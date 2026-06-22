# Анализ переиспользования вендоров — через призму DDD

> Дополнение к [`ARCHITECTURE.md`](ARCHITECTURE.md). Тот файл картирует вендорённые контексты;
> **этот файл решает, что мы из них берём и *каким именно DDD-паттерном интеграции*.** Переиспользование
> — это никогда не «скопировать файл», а выбор отношения с вышестоящим Bounded Context.

**Подготовлено:** 2026-06-21, по результатам параллельного чтения 5 источников (по одному исследователю на контекст).
**Хост:** IPFRS (`ipfrs_source/`), Core-дифференциаторы = гео-распределённый вывод + ответы с доказательствами
(proof-carrying); Generic-субстрат = тензорная численность, хранилище, сеть.

> ⚠️ **Проверяй, прежде чем зависеть.** Выводы получены из чтения фрагментов, а не из компиляции. Все
> пути к файлам / номера строк / имена типов ниже считай *зацепками для подтверждения*, а не фактами.
> Тяжёлые апстримы (особенно trustformers → tch/candle/onnx) нужно взвешивать против нашего лёгкого
> графа P2P-крейтов и известного риска цикла зависимостей `network ↔ tensorlogic`. Всё, что затрагивает
> **SciRS2, — это решение уровня Shared Kernel**, а не односторонний апгрейд.

---

## 0. Принятые решения

- **Тензорный субстрат (Phase 5):** оставляем собственный лёгкий `NumTensor`, **спрятанный за бэкенд-трейтом**
  (Conformist на *интерфейс* `TlExecutor`, а не на его реализацию). Никакой тяжёлой supplier-зависимости,
  никакого наследования SciRS2, никакого риска цикла. Реальный движок можно подставить за трейт позже.
- **Набор операций:** наращиваем его **портом чистых `f32`-kernel'ов** (ACL), а не принятием тензорного крейта.

---

## 1. Паттерны интеграции (легенда DDD)

| Паттерн | Что означает здесь | Стоимость / ограничитель |
|---------|--------------------|--------------------------|
| **Conformist** | принять *модель/интерфейс* апстрима, говорить на его языке | дёшево; мы не управляем его эволюцией |
| **ACL-порт** | скопировать *алгоритм*, перевести типы в наш Ubiquitous Language | типы апстрима не должны протекать за границу |
| **Supplier-зависимость** | `depend on` крейт напрямую | наследуется его граф зависимостей; только для Generic-поддоменов |
| **Shared Kernel** | совместно управляемая зависимость (SciRS2) | изменение = решение нескольких контекстов |
| **Published Language** | обогатить наш `model_manifest` (DAG-CBOR) соглашениями апстрима | оставляем своим, версионируем по CID |

**Стратегическое правило:** *Core — владеем, Generic — покупаем.* Механику Core портируем аккуратно через ACL;
от suppliers зависим только ради заменяемого субстрата.

---

## 2. Карта переиспользования — по поддоменам хоста

### 🔴 Core — распределённое исполнение графа (Phase 5)  ·  *наивысший рычаг*
ACL-порты, малый вес зависимостей; именно это и есть разблокировка.

> ✅ **Spike 2 ГОТОВ 2026-06-22 — vocabulary партиционирования, sharding и collectives ACL-портированы в новый
> модуль `ipfrs-tensorlogic::distributed` (local/pure/synchronous, без dep на `ipfrs-network` → нет цикла).**
> *Vocabulary* заимствован у torsh; *алгоритмы — наши собственные и реально считают* —
> тела collectives у torsh оказались mock-заглушками («skip the averaging to avoid type issues»),
> поэтому брать стоило только формы типов. Три сабмодуля (23 unit-теста, все зелёные):
> • `sharding` — `ShardStrategy{Row,Column}Parallel` + `ShardSpec`, `plan_shards`/`shard_tensor`/
>   `gather_shards` (2-D `NumTensor`, ровное разбиение с остатком-вперёд, проверено roundtrip'ом).
> • `collectives` — `ReduceOp{Sum,Mean,Max,Min,Product}` + реальные `all_reduce`/`all_gather`/
>   `reduce_scatter` над `NumTensor`.
> • `schedule` — `build_communication_schedule(partitions, edges)` → `CommunicationSchedule`
>   (`CommunicationStage` / `DataTransfer`); строит DAG уровня партиций из рёбер-разрезов и
>   раскладывает трансферы по слоям longest-path по готовности исходной партиции (= «partition graph +
>   stream activations»). Это и есть постадийное расписание связи, которого не хватало партиционеру.
> ✅ **Spike 2b ГОТОВ 2026-06-22 — libp2p wire-протокол `/ipfrs/activation/1.0.0` + проводка через
> инверсию зависимостей; DoD достигнут.** Три слоя, цикл `network ↔ tensorlogic` остался невозможен:
> • `ipfrs-tensorlogic::distributed::wire` — serde `StageRequest`/`StageResponse` + чистый
>   `execute_stage` (то, что считает пир); `transport` — трейт `ActivationTransport` + оркестратор
>   `execute_pipeline` + in-process `LocalTransport` (single-node fallback). Чистые, тестируются без
>   сети. Попутно починен serde-roundtrip `ComputationGraph` (поле `cid` без `#[serde(default)]`).
> • `ipfrs-network` — протокол `/ipfrs/activation/1.0.0` (паттерн `semsearch`: behaviour, event loop,
>   SwarmCommand, provider-callback). Транспорт реализован на Send+Sync `ActivationHandle` (клон
>   `cmd_tx` + `connected_peers`), т.к. `NetworkNode` держит libp2p `Swarm` и `!Sync`; стадия,
>   закреплённая за локальным пиром, считается in-process без сетевого round-trip.
> • `ipfrs::Node` — `enable_distributed_execution` (сервер: provider → `execute_stage`) +
>   `run_distributed_pipeline` (клиент: `execute_pipeline` поверх `ActivationHandle`).
> **DoD выполнен:** интеграционный тест `distributed_exec_integration.rs` — 2-stage граф исполняется
> на 2 живых libp2p-нодах (stage 1 на узле B по сети, stage 2 на узле A локально; активация `h`
> стримится B→A). 32 + 2 юнит-теста + интеграционный — зелёные, clippy чист.
> **Остаётся (Spike 2c, опц.):** связать `graph_partitioner`/`build_communication_schedule` →
> авто-построение `PipelineStage` из партиций (сейчас стадии задаёт вызывающий вручную); тюнинг
> размера CBOR-сообщений под крупные активации.

| Взять | Откуда | Файл (проверить) | Паттерн | Связь / Трудоёмкость |
|-------|--------|------------------|---------|----------------------|
| ~~`PartitioningStrategy` + `PartitionedGraph` с **постадийным comm-расписанием**~~ | torsh | `torsh-fx/src/graph_partitioning.rs` | ✅ **ГОТОВО** — `distributed::schedule::build_communication_schedule` над нашими `Partition`/`GraphEdge` | ГОТОВО 2026-06-22 |
| ~~`ShardInfo` (пир владеет каким срезом) → добавить в `NumTensor`~~ | torsh | `torsh-distributed/src/tensor_parallel.rs` | ✅ **ГОТОВО** — `distributed::sharding` (`ShardSpec`/`ShardStrategy`) | ГОТОВО 2026-06-22 |
| ~~`CollectiveOp{AllReduce,AllGather,ReduceScatter,Barrier}` поверх libp2p~~ | torsh | `torsh-distributed/src/collectives.rs` | ✅ **ГОТОВО (локально)** — `distributed::collectives` реальные редукции; слой libp2p = Spike 2b | ГОТОВО 2026-06-22 (wire отложен) |
| анализ свёртки einsum + `placement`/`scheduling`/`partitioned` | tensorlogic | `tensorlogic-infer/src/{join_order,placement,scheduling,partitioned}` | ACL-порт | ВЫС / Сред |
| pipeline/tensor-parallel как референс («слои по пирам») | trustformers | `trustformers-core/src/parallel/{tensor,pipeline}_parallel.rs` | только изучить | Сред-Выс / Выс |

### 🟠 Generic — численный движок  ·  *решение зафиксировано: владеем-за-трейтом*

> ✅ **Spike 3 ГОТОВ 2026-06-22 — Conformist-трейт `TlExecutor` + ACL-порт kernel'ов из oxigaf.**
> • `ipfrs-tensorlogic::tl_executor` — трейт `TlExecutor` (assoc. `Tensor`/`Error`; `einsum`/`elem_op`/
>   `elem_op_binary`/`reduce`) + enum'ы `ElemOp`/`ReduceOp` по форме `tensorlogic-infer` (Conformist
>   на интерфейс) + бэкенд `NumExecutor` над `NumTensor` (наша реализация; einsum пока matmul
>   `ab,bc->ac`, reduce — целиком/2-D по оси). • `ipfrs-tensorlogic::numerics` — ACL-порт чистых f32:
>   `softmax`/`log_softmax`/`layer_norm`/`rms_norm`/`gelu`/`silu` (ошибки → `GraphError`, не отдельный
>   `NumericsError`). • Граф-исполнитель `numeric_exec` теперь считает `Softmax{axis}`, `LayerNorm`,
>   новый op `SiLU`, а `GELU` идёт через общий kernel. 10 + 7 + 5 тестов зелёные, clippy чист.
> Решение «владеем-за-трейтом» в силе: тяжёлый бэкенд (SciRS2/GPU) можно подставить за `TlExecutor`.

| Взять | Откуда | Файл (проверить) | Паттерн | Связь / Трудоёмкость |
|-------|--------|------------------|---------|----------------------|
| ~~трейт `TlExecutor` + enum'ы `ElemOp`/`ReduceOp` → обернуть `NumTensor`~~ | tensorlogic | `tensorlogic-infer/src/{traits.rs,ops.rs}` | ✅ **ГОТОВО** — `tl_executor` (трейт + `NumExecutor`) | ГОТОВО 2026-06-22 |
| ~~чистые `f32`-kernel'ы: `softmax`(log-sum-exp), `layer_norm`, `rms_norm`, `gelu`, `silu`~~ | oxigaf | `oxigaf-diffusion/src/numerics.rs` | ✅ **ГОТОВО** — `numerics` (ACL-порт) + проводка в граф | ГОТОВО 2026-06-22 |
| `EinsumGraph`/`OpType` + `validation` графа как референс IR | tensorlogic | `tensorlogic-ir/src/graph/{node,optype,validation}.rs` | Conformist (опц.) | Сред / Низк-Сред |
| _полный `Tensor<T>` / autograd / mmap_ | torsh-core / trustformers-core | — | **Supplier — отложено** (SciRS2 + тяжёлые deps) | — |

### 🟠 Core-ish — федеративное / распределённое обучение

| Взять | Откуда | Файл (проверить) | Паттерн | Связь / Трудоёмкость |
|-------|--------|------------------|---------|----------------------|
| трейты `Optimizer`/`Loss` → FedAvg реализует `Optimizer` | tensorlogic | `tensorlogic-train/src/{optimizers,loss}` | Conformist | Сред / Низк |
| византийско-устойчивые `AggregationStrategy{Krum,Median}` + отбор клиентов | torsh | `torsh-autograd/src/federated_learning/` | ACL-порт | Сред / Сред |
| аккумуляция градиентов + loss-scaling смешанной точности + трекинг потока | oxigaf | `oxigaf-trainer/src/{gradient_accumulation,mixed_precision,gradient_flow}.rs` | ACL-порт | ВЫС / Низк-Сред |

### 🟢 Supporting — семантический поиск / retrieval (HNSW у нас уже есть)  ·  *быстрые победы*

> ✅ **Проверено 2026-06-21 — SIMD и RRF уже есть в хосте; НЕ портировать из oxirag.**
> В `ipfrs-semantic::simd` уже есть `cosine_distance`/`l2_distance`/`dot_product` с рантайм-детектом
> AVX2/AVX/SSE/NEON + скалярный фолбэк. В `ipfrs-semantic::result_aggregator` уже есть
> `ResultAggregator` + `AggregationStrategy::RankFusion` + `aggregate_rrf()` (RRF, k=60 по умолчанию).
> Реальный пробел был в том, что `semantic_search_distributed` делал наивный merge по лучшему score
> на CID вместо их использования — **теперь исправлено проводкой существующего `ResultAggregator` (RRF)
> в распределённый fan-out** (`ipfrs/src/node/semantic_ops.rs`), где каждый пир + локальный индекс —
> отдельный ранжированный источник.

| Взять | Откуда | Файл (проверить) | Паттерн | Связь / Трудоёмкость |
|-------|--------|------------------|---------|----------------------|
| ~~`SimilarityEngine` (авто AVX2/NEON cosine)~~ | oxirag | `simd_similarity.rs` | ❌ **избыточно** — в хосте уже есть `ipfrs-semantic::simd` | ГОТОВО (уже было) |
| ~~**RRF-слияние** для merge результатов пиров~~ | oxirag | `hybrid_search.rs` | ✅ **ГОТОВО** — существующий `ipfrs-semantic::result_aggregator` подключён в `semantic_search_distributed` | ГОТОВО 2026-06-21 |
| reranker-конвейер + расширение запроса + relevance feedback | oxirag | `reranker.rs`, `query_expansion.rs`, `relevance_feedback.rs` | ACL-порт | Сред / Низк-Сред |
| KG-оверлей поверх CID-блоков | oxirag | `layer4_graph/` | ACL-порт | Сред / Сред |

### 🔴 Core — вывод с доказательствами (мы уже отдаём `proof_json`)

| Взять | Откуда | Файл (проверить) | Паттерн | Связь / Трудоёмкость |
|-------|--------|------------------|---------|----------------------|
| `ClaimExtractor` (NL → предикаты) + `SmtVerifier` (SMT-LIB2, проверяемо пиром) | oxirag | `layer3_judge/` | ACL-порт + supplier (SMT) | ВЫС / Выс |

### ⚪ Сквозное — устойчивость распределённых запросов к пирам

> ✅ **Проверено 2026-06-21 — circuit breaker уже есть в хосте; НЕ портировать из oxirag.**
> В `ipfrs-network::circuit_breaker` уже есть `CircuitBreakerRegistry` + `PeerCircuitBreaker`
> (Closed/Open/HalfOpen, скользящее окно, детект медленных вызовов). Он просто не был подключён к
> семантическому fan-out. **Теперь исправлено:** `Node` держит `Mutex<CircuitBreakerRegistry>`, а
> `semantic_search_distributed` защищает каждый запрос к пиру (`can_call` → пропускать Open-пиров;
> `record_result` Success/Failure/Timeout).

| Взять | Откуда | Файл (проверить) | Паттерн | Связь / Трудоёмкость |
|-------|--------|------------------|---------|----------------------|
| ~~circuit breaker (Closed/Open/HalfOpen на пира)~~ | oxirag | `circuit_breaker.rs` | ✅ **ГОТОВО** — существующий `ipfrs-network::circuit_breaker` подключён в fan-out по пирам | ГОТОВО 2026-06-21 |
| retry+backoff+jitter + пул соединений libp2p | oxirag | `retry.rs`, `connection_pool.rs` | ACL-порт (в хосте есть `ipfrs-storage::retry` — сперва проверить переиспользование) | Сред / Низк |

### ⚫ Домен oxigaf — **не переиспользуем**
~80% — это логика FLAME / рендеринга Gaussian-splat / диффузионного домена. Берём только инфра-kernel'ы и
патчи обучения, перечисленные выше; игнорируем `oxigaf-{render,flame}` и оркестрацию тренера аватаров.

---

## 3. Serving (Supporting, когда запустим реальный инференс трансформеров)

Из **trustformers** (только если/когда выйдем за пределы заглушки `serve_inference`) — перенять паттерны,
избегая тяжёлого крейта: динамический батчинг (`trustformers-serve/src/batching/`), KVCache + вытеснение +
prefix-cache (`trustformers-serve/src/{kv_cache,prefix_cache}/`), балансировщик нагрузки + health-check'и.
**Угол Published Language:** мост от CID-адресуемых слоёв `model_manifest` → загрузчик в стиле `Model`,
чтобы веса грузились *из контент-адресуемых блоков* вместо HF Hub.

---

## 4. Рекомендуемая последовательность (по рычагу)

1. ✅ **Spike 1 — быстрые победы в retrieval** — **ГОТОВО 2026-06-21.** Результат отличался от плана:
   SIMD, RRF и circuit breaker на пира *уже существовали* в хосте (`ipfrs-semantic::simd`,
   `ipfrs-semantic::result_aggregator`, `ipfrs-network::circuit_breaker`) — из oxirag не портировали
   ничего. Победа была в **проводке**: `semantic_search_distributed` теперь RRF-сливает локальный +
   per-peer ранжированные списки через `ResultAggregator` и защищает каждого пира персистентным
   `CircuitBreakerRegistry` (пропуск Open-пиров, запись Success/Failure/Timeout). Урок:
   ограничитель «проверяй, прежде чем зависеть» окупился — зацепки анализа «port from oxirag», основанные
   на фрагментах, оказались избыточны при зрелом коде хоста. Оставшиеся опциональные retrieval-пункты:
   reranker / расширение запроса / KG-оверлей (реально отсутствуют → по-прежнему кандидаты на ACL-порт).
2. ✅ **Spike 2 — разблокировка Phase 5** — **ГОТОВО 2026-06-22.** ACL-порт партиционирования torsh + `ShardInfo` +
   collectives в новый модуль `ipfrs-tensorlogic::distributed`: у `graph_partitioner` теперь есть
   спутник `build_communication_schedule` (= partition graph + stream activations), плюс примитивы
   `sharding` + `collectives` (23 теста, все зелёные; без dep на `ipfrs-network`). Тот же урок, что в
   Spike 1: collectives у torsh были mock-заглушками, поэтому портирован только *vocabulary* — алгоритмы
   наши собственные.
   ✅ **Spike 2b — ГОТОВО 2026-06-22.** libp2p-протокол `/ipfrs/activation/1.0.0` + проводка через
   инверсию зависимостей (`wire`/`transport` в tensorlogic, протокол в network, оркестрация в `Node`).
   DoD достигнут: 2-stage граф на 2 живых нодах (интеграционный тест `distributed_exec_integration.rs`).
   Деталь — `NetworkNode` `!Sync` (держит `Swarm`), поэтому транспорт реализован на Send+Sync
   `ActivationHandle`, а не на `&NetworkNode`. **Spike 2c (опц.):** авто-`PipelineStage` из партиций.
3. ✅ **Spike 3 — Conformist-движок** — **ГОТОВО 2026-06-22.** `NumTensor` за трейтом `TlExecutor`
   (`tl_executor`: трейт + `ElemOp`/`ReduceOp` по форме `tensorlogic-infer` + бэкенд `NumExecutor`) +
   ACL-порт чистых f32-kernel'ов из `oxigaf/numerics.rs` (`numerics`: softmax/log_softmax/layer_norm/
   rms_norm/gelu/silu); граф-исполнитель считает `Softmax`/`LayerNorm`/новый `SiLU`. 22 теста зелёные.
   **Spike 3b (опц.):** обобщить einsum/reduce в `NumExecutor`; переключить `numeric_exec` на `TlExecutor`.
4. **Среднесрочное**: SMT-судья → proof-carrying; агрегация Krum → FedAvg; трекинг потока градиентов.

**Ограничитель (из `ARCHITECTURE.md` §4):** держать типы апстрима за нашим ACL
(`ipfrs_source/crates/ipfrs-tensorlogic` и граница сети) — они не должны протекать в доменные типы Core.
