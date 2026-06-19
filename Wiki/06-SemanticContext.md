---
title: 06-SemanticContext
type: domain
summary: Semantic Context (ipfrs-semantic) — VectorIndex (HNSW), DiskANNIndex (Vamana), SemanticRouter, семантический DHT, мост к TensorLogic
tags: [ipfrs, ddd, semantic, hnsw, diskann, vector-search, embedding]
source: ipfrs_source/crates/ipfrs-semantic/src/
related: ["[[03-SharedKernel]]", "[[07-TensorLogicContext]]", "[[11-RealityCheck]]"]
read_time: 13 мин
updated: 2026-06-19
---

# Semantic Context — `ipfrs-semantic`

**Краткое резюме**: Semantic отвечает на вопрос «**что это значит и что на это
похоже?**». Идентичность берётся из ядра (`Cid`), смысл — плотные `Vec<f32>`-эмбеддинги,
индексируемые для приближённого поиска соседей (ANN). Это один из двух **Core Domain**
контекстов IPFRS.

---

## 1. Структура: малое ядро под огромным слоем сервисов

Настоящее доменное ядро невелико:
- `hnsw.rs` — in-memory ANN-агрегат (`VectorIndex`)
- `diskann.rs` — дисковый ANN-агрегат (`DiskANNIndex`)
- `router.rs` — фасад/оркестратор (`SemanticRouter`)
- `simd.rs`, `quantization.rs`, `dht.rs`, `solver.rs`, `persistence.rs`

Остальные ~100 модулей (`topic_modeler`, `sentiment_analyzer`, `concept_graph`…) —
в основном самодостаточные сервисы. ⚠️ **Только 25 из ~135 файлов вообще импортируют
`ipfrs_core`** — большая часть спутникового слоя слабо связана с доменом IPFRS и
читается скорее как векторная библиотека-тулкит.

---

## 2. Корневой агрегат: `VectorIndex` (HNSW)

```rust
// hnsw.rs:84 (сокращено)
pub struct VectorIndex {
    index: Arc<RwLock<Hnsw<'static, f32, DistL2>>>,   // backend hnsw_rs, ВСЕГДА DistL2
    id_to_cid: …, cid_to_id: …,                       // двусторонний маппинг
    vectors: Arc<RwLock<HashMap<Cid, Vec<f32>>>>,     // оригиналы сохраняются
    dimension: usize, metric: DistanceMetric,
}
```

**Ключевой факт**: backend монтирован на `DistL2` на уровне типа (`hnsw.rs:86`).
Cosine и DotProduct **эмулируются**: нормализация при вставке/поиске (`hnsw.rs:379`)
+ конверсия L2-расстояния обратно в score (`hnsw.rs:399`). Отдельного cosine-графа нет.

Сам алгоритм HNSW **делегирован крейту `hnsw_rs`** (`hnsw.rs:6,126`) — IPFRS не пишет
сам назначение слоёв/`search_layer`/прунинг. IPFRS добавляет: нормализацию,
CID-маппинг, тюнинг параметров, персистентность.

---

## 3. Второй агрегат: `DiskANNIndex` (Vamana, собственная реализация)

`diskann.rs:186` — векторы лежат на диске через `MmapMut`, **но граф (adjacency)
держится в RAM** (`diskann.rs:201`). Реально реализованы:
- `vamana_insert` (`diskann.rs:592`): greedy search → robust prune → двунаправленные
  рёбра.
- `robust_prune` (`diskann.rs:630`): классический Vamana RobustPrune, `alpha` деф. 1.2.

⚠️ «Константная память» (как в вики) верна лишь частично — adjacency не на диске. PQ
(product quantization) в `diskann.rs` **нет** — хранятся сырые f32.

---

## 4. Фасад: `SemanticRouter`

`router.rs:347` — владеет `IndexHandle` (`Hnsw | DiskAnn`) + LRU-кэш результатов.
Высокоуровневая точка входа и ACL между вызывающими и выбранным backend. Прочие
индексы-агрегаты: `HybridIndex` (vector+metadata), `MultiModalIndex`, `DynamicIndex`,
`LearnedIndex`.

---

## 5. Доменные сервисы

| Сервис | Ответственность | Источник |
|--------|-----------------|----------|
| `EmbeddingPipeline` | вход (`RawBytes/Text/Structured/Embedding`) → нормализованный `Vec<f32>` | `embedding_pipeline.rs:212` |
| `DenseRetriever` | гибрид точный cosine + BM25, min-max нормализация и слияние | `dense_retriever.rs:204` |
| `CorpusIndexer` | инвертированный индекс, BM25, фасетные фильтры | `corpus_indexer.rs:218` |
| `CrossEncoder` / реранкеры | переоценка пар query-doc (несколько реализаций) | `cross_encoder.rs:210` |
| SIMD-ядра | NEON/SSE/AVX/AVX2 для l2/dot/cosine | `simd.rs:24` |

---

## 6. Ключевые инварианты

1. **Согласованность размерности** — каждая вставка/поиск проверяет
   `vector.len() == dimension` (`hnsw.rs:159`).
2. **Уникальность CID** — повторная вставка существующего CID это **ошибка**, не
   upsert (`hnsw.rs:167`). ⚠️ Вики GLM показывают `return Ok(())` — неверно.
3. **CID-связность** — эмбеддинги адресуются по `Cid` (мост к ядру).
4. **Soft delete** — `delete` убирает только маппинги, узел графа остаётся
   (`hnsw.rs:275`) → со временем фрагментация.

---

## 7. Интеграция

### 7.1 Мост к TensorLogic (`solver.rs`)
`PredicateEmbedder` (`solver.rs:49`) детерминированно отображает предикат в вектор.
`backward_chain` (`solver.rs:340`): сначала точная унификация, при пустом результате —
**семантический fallback** через похожие предикаты выше порога сходства (деф. 0.8).
> ⚠️ Вики Antropic помечают «логико-семантический fallback» как *будущее* — на деле
> он **реализован**.

### 7.2 Семантический DHT ≠ сетевой DHT
`dht.rs` — отдельный DHT **в пространстве эмбеддингов**: маршрутизация по векторному
расстоянию до агрегатных эмбеддингов пиров (`dht.rs:148`). Использует `PeerId` из
network лишь для идентичности.

---

## 8. Что реально работает, а что заглушка

| Подсистема | Статус |
|------------|--------|
| HNSW insert/search (через `hnsw_rs`) | ✅ работает |
| DiskANN/Vamana insert + robust_prune | ✅ работает |
| Гибридный поиск (dense + BM25) | ✅ работает |
| Логико-семантический `backward_chain` | ✅ работает |
| `VectorIndex::rebuild` | ⚠️ **баг**: ставит пустой граф, `vectors_reinserted: 0` — молча опустошает индекс (`hnsw.rs:586,611`) |
| Снапшот топологии HNSW | ⚠️ нельзя сохранить adjacency — restore пересобирает граф (`hnsw.rs:791`) |
| `DiskANNIndex::compact` | ⚠️ no-op (`diskann.rs:947`) |
| Распределённый семантический поиск (DHT transport) | ⚠️ заглушка (`dht_node.rs:409,437`) |

Полностью — [[11-RealityCheck]].

---

## Что дальше?

- **Мост к движкам вывода** → [[07-TensorLogicContext]]
- **Сценарий SEARCH целиком** → [[10-DataFlows]]

**Связанные**: [[03-SharedKernel]] | [[07-TensorLogicContext]] | [[10-DataFlows]] | [[11-RealityCheck]]
**Источник кода**: `ipfrs-semantic/src/{hnsw,diskann,router,solver,dht,simd,embedding_pipeline}.rs`
