---
title: 10-DataFlows
type: dataflow
summary: Сквозные сценарии «как функционирует IPFRS» — ADD, GET (cache→DHT→backfill), SEARCH, INFER, FedAvg — с цитатами кода
tags: [ipfrs, ddd, dataflow, sequence, add, get, search, infer]
related: ["[[01-DomainOverview]]", "[[09-ApplicationLayer]]", "[[11-RealityCheck]]"]
read_time: 14 мин
updated: 2026-06-19
---

# Потоки данных: как IPFRS функционирует

**Краткое резюме**: эта страница — сердце вики. Она прослеживает реальные запросы
через все контексты: **ADD**, **GET**, **SEARCH**, **INFER** и **FedAvg**. Каждый
шаг привязан к коду. Шаги-заглушки помечены ⚠️.

---

## 1. ADD — добавление контента

```
Пользователь → Gateway → Node.add_bytes → Storage(put) → Network(provide)
```

```
1. POST /api/v0/add (multipart)            gateway/routes.rs:385
2. ACL: bytes → Block::new(Bytes)           block.rs:70  (cid = hash(data), I1)
       └─ если > 2 MiB: chunking → DAG       chunking.rs:459 (фан-аут ≤174)
3. Node.add_bytes                            block_ops.rs:45
   ├─ storage.put_if_absent(block)           block_ops.rs:53 (дедуп I5)
   │    └─ стек: Bloom → Cache → Sled         helpers.rs:136
   │         └─ db.insert(cid.to_bytes(), data); flush_async  blockstore.rs:223,241
   └─ network.provide(cid)  (best-effort)    block_ops.rs:65
        └─ kademlia.start_providing(key)      node.rs:1198
4. ответ Kubo {Name, Hash, Size}             routes.rs:385
```

**Что произошло по DDD**: Gateway (ACL) перевёл HTTP в доменный `Block`; Shared Kernel
присвоил идентичность (CID); Storage (Repository) сохранил с дедупом; Network объявил
провайдинг в DHT. Четыре контекста, один сквозной сценарий.

---

## 2. GET — получение контента (cache → DHT → backfill)

Это каноничный пример оркестрации фасадом (`block_ops.rs:127`):

```
Node.get(cid):
1. storage.get(cid)                          block_ops.rs:131
   └─ Bloom «нет»? → сразу None              bloom.rs:560
   └─ L1 cache hit? → вернуть                cache.rs:234
   └─ Sled: Block::from_parts(*cid, data)    blockstore.rs:350  ⚠️ БЕЗ verify() (I9)
   ▼ если найдено — вернуть
2. ПРОМАХ → network.find_providers(cid)      block_ops.rs:146
   └─ kademlia.get_providers(key)            node.rs:1214
   └─ ожидание провайдеров с таймаутом       node.rs:1229
3. для ≤3 провайдеров: fetch_block_from_peer  block_ops.rs:154
   └─ ⚠️ В ipfrs-network это ЗАГЛУШКА → NotFound  node.rs:1311
   └─ реальная выкачка — забота Transport (Session/Bitswap)  [[08-TransportContext]]
4. при успехе: storage.put(block) (backfill) block_ops.rs:159
5. метрики на каждом шаге                     block_ops.rs:133,145,165
```

> ⚠️ **Критично для понимания «как это работает»**: связка Network→Transport для
> выкачки блока по проводу **не завершена** (`node.rs:1311`). Node находит
> провайдеров через DHT, но реальная межпировая выкачка через swarm — заглушка.
> Локальный GET (cache/Sled) работает полностью. См. [[11-RealityCheck]].

---

## 3. Обмен блоками (Transport, изолированно)

Когда выкачка инициируется через Transport напрямую:

```
1. SessionManager.create_session             session.rs:484
2. session.add_block(cid)                     session.rs:252 (один WantEntry/CID)
3. BitswapExchange.want(cid, priority)        bitswap.rs:106
   └─ ConcurrentWantList: куча приоритетов     want_list.rs:220
4. select_peers_for_request (провайдеры 1-ми) bitswap.rs:184
5. (входящий) receive_block(peer, block)      bitswap.rs:203
   ├─ store.put(block)                         (Storage)
   ├─ учёт латентности пира
   └─ session.mark_received(cid)               session.rs:290
6. сессия завершена ⟺ recv+failed ≥ total      session.rs:151
```

TensorSwap (`tensorswap/core.rs`) добавляет поверх: запрос зависимостей, прогрессивный
стриминг чанков, einsum-граф приоритетов, safetensors-парсинг.

---

## 4. SEARCH — семантический поиск

```
Node → SemanticRouter → VectorIndex(HNSW)
```

```
ИНДЕКСАЦИЯ:
1. Node.index_content(cid, data)             semantic_ops.rs (ленивый OnceCell init)
2. EmbeddingPipeline.process → Vec<f32>       embedding_pipeline.rs:212
3. VectorIndex.insert(cid, embedding)         hnsw.rs:158
   ├─ проверка размерности (I6)                hnsw.rs:159
   ├─ проверка уникальности CID                hnsw.rs:167
   ├─ нормализация (для cosine/dot)            hnsw.rs:379
   └─ hnsw_rs.insert (делегирование)           hnsw.rs:126

ПОИСК:
4. Node.search_similar(query_vec, k)
5. VectorIndex.search                          hnsw.rs:235
   ├─ нормализация запроса
   ├─ hnsw_rs.search → внутренние id           (через DistL2)
   ├─ convert_distance → score                 hnsw.rs:399
   └─ id → Cid (отсутствующие молча отброшены) hnsw.rs:261
6. (опц.) reranking / гибрид с BM25            dense_retriever.rs:204
```

> ⚠️ Не вызывайте `VectorIndex::rebuild` на заполненном индексе — он молча опустошает
> граф (`hnsw.rs:586`). Распределённый поиск через семантический DHT — заглушка
> (`dht_node.rs:409`).

---

## 5. INFER — логический/нейро-символический вывод

```
Node → TensorLogicStore → InferenceEngine / NeuralSymbolicIntegrator
```

```
ПОДГОТОВКА KB:
1. Node.add_fact / add_rule                   tensorlogic_ops.rs
   └─ KnowledgeBase.add_fact (push, без проверки согласованности)  ir.rs:292
   └─ (опц.) rule_to_block → CID               ipld_codec.rs:171 (детерминизм I7)

ВЫВОД (символический):
2. Node.infer(goal)
3. InferenceEngine.query(goal, kb)            reasoning.rs:259
   ├─ solve_goal (SLD-резолюция)               reasoning.rs:275
   ├─ unify_predicates (+ occurs-check)        reasoning.rs:555
   └─ → Vec<Substitution> + ProofTree

ВЫВОД (нейро-символический):
4. NeuralSymbolicIntegrator.infer(query)      neural_symbolic.rs:474
   ├─ symbolic = forward_chain(...)
   ├─ neural   = cosine по эмбеддингам
   └─ Hybrid: nw*neural + (1-nw)*symbolic      neural_symbolic.rs:487

РАСПРЕДЕЛЁННЫЙ ВЫВОД:
5. DistributedBackwardChainer.prove_with_tree  distributed_backward_chainer.rs:66
   ├─ локальные факты → локальные правила
   └─ делегирование пирам по CID (через gossip) node.rs:1539
```

Семантический fallback: если точная унификация пуста, `solver.backward_chain`
доказывает семантически похожие предикаты выше порога 0.8 (`solver.rs:340`).

---

## 6. FedAvg — федеративное обучение

```
1. локальный тренинг → градиент Vec<f32>       (autograd.rs / per-layer backward)
2. commit_local → broadcast CID градиента      gradient/federated.rs:1054
3. пиры: add_peer_gradient(cid)
   └─ fetch блока, декод Arrow IPC
4. is_ready(min_peers)?                         federated.rs:1157
5. aggregate() → FedAvg (невзвешенное среднее)  federated.rs:1125 / backward_pass.rs:19
   └─ проверка размерности (I8 совместимо)
6. (опц.) RoundConsensusTracker: кворум раунда  consensus.rs
```

> ⚠️ `collect_updates` прерывается по таймауту и **молча усредняет по меньшему кругу
> пиров** (`federated.rs:1014`) — не зависание, а тихая частичная агрегация.

---

## 7. Сводная диаграмма контекстов в потоке

```
        ADD ──────────────┐   ┌────────────── GET
                          ▼   ▼
              ┌───────────────────────────┐
              │      ipfrs::Node (Facade)  │
              └─┬────┬────────┬────────┬───┘
        Storage │    │Network │Semantic│ TensorLogic
       (put/get)│    │(DHT)   │(HNSW)  │ (infer/FedAvg)
                │    │        │        │
                ▼    ▼        ▼        ▼
            [Sled] [Kademlia][hnsw_rs][20+ движков]
                     │
                  Transport (Session/Bitswap/TensorSwap) ── выкачка блоков
```

---

## Что дальше?

- **Что из показанного — заглушка** → [[11-RealityCheck]]
- **Единый язык и инварианты** → [[01-DomainOverview]]

**Связанные**: [[01-DomainOverview]] | [[09-ApplicationLayer]] | [[11-RealityCheck]]
**Источник кода**: `ipfrs/src/node/{block_ops,semantic_ops,tensorlogic_ops}.rs` + соответствующие контексты
