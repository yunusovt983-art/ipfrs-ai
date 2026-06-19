---
title: 11-RealityCheck
type: reference
summary: Реестр реальности — что в IPFRS реально работает, что заглушка (⚠️), и где старые вики расходятся с кодом
tags: [ipfrs, ddd, reality-check, stubs, discrepancies, tech-debt]
related: ["[[00-INDEX]]", "[[02-StrategicDesign]]", "[[10-DataFlows]]"]
read_time: 12 мин
updated: 2026-06-19
---

# Реестр реальности: модель vs код

**Краткое резюме**: эта страница — главное отличие данной вики от предыдущих. Она
честно фиксирует, **что в IPFRS реально работает, а что заглушка**, и собирает
расхождения между ранними вики (`Wiki_Antropic`, `Wiki_Arch_GLM`) и фактическим
кодом. Основано на глубоком анализе 7 агентов с привязкой `file:line`.

---

## 1. Карта зрелости по контекстам

| Контекст | Ядро реально | Главные заглушки/баги |
|----------|--------------|------------------------|
| Shared Kernel | ✅ полностью | — (но `verify()` чувствителен к алгоритму) |
| Storage | ✅ стор + декораторы | ⚠️ нет verify при чтении; 3 разных GC; min_age не везде |
| Network | ✅ swarm + DHT | ⚠️ Bitswap-выкачка, gossip-по-проводу, `KademliaDhtProvider` — заглушки; Tier B не подключён |
| Semantic | ✅ HNSW/DiskANN/поиск | ⚠️ `rebuild` ломает индекс; снапшот топологии; `compact` no-op; DHT-transport заглушка |
| TensorLogic | ✅ 20+ движков + автоград + FedAvg | ⚠️ распределённое исполнение графа заглушка; тензорный backward отсутствует |
| Transport | ✅ Session/Bitswap/TensorSwap | ⚠️ GraphSync, erasure, NAT — заглушки; fallback-баг мульти-транспорта |
| Application/Gateway | ✅ Node/Gateway/Auth/JWT | ⚠️ TLS-генератор в node заглушка; auth-модель дублирована |

---

## 2. Полный реестр заглушек (⚠️) и багов

### Network
- **Выкачка блоков по swarm** — `fetch_block_from_peer` → `NotFound` («pending Task E»,
  `node.rs:1311`).
- **Gossipsub** — только in-process; нет `libp2p::gossipsub` behaviour; `validate_message`
  всегда `true` (`gossipsub.rs:468`).
- **`KademliaDhtProvider`** — все 12 методов порта DHT заглушены (`dht_provider.rs:388`).
- **`DhtManager`** — это кэш, не DHT; `record_provide`/`mark_reannounced` — no-op (`dht.rs:646`).
- **Tier B** (~180 модулей репутации/банов/пулов) не получает событий swarm → не влияет
  на реальные дайлы/прюнинг.
- Хрупкий ключ ожидания провайдера (lossy UTF-8 от бинарного CID, `node.rs:1234`).

### Semantic
- **`VectorIndex::rebuild`** — ставит пустой граф, `vectors_reinserted: 0`; на заполненном
  индексе **молча опустошает** граф, при этом `len()` врёт (`hnsw.rs:586,611`). Реальный баг.
- **Снапшот HNSW** не сохраняет adjacency (`layer_connections: Vec::new()`); restore
  пересобирает граф пере-вставкой, топология приближённо теряется (`hnsw.rs:791`).
- **`DiskANNIndex::compact`** — no-op, `bytes_saved: 0` (`diskann.rs:947`).
- **DHT-транспорт** — `replicate_to_peer`/`query_peer` ничего не передают (`dht_node.rs:409,437`)
  → распределённый поиск только локальный.

### TensorLogic
- **Распределённое исполнение графа** — `execute_distributed` → `Err("requires
  ipfrs-network integration")` (`computation_graph.rs:1667`). Партиционирование работает,
  исполнение — нет.
- **Тензорный автоград** — `ComputationGraph` только forward; backward над `TensorOp` нет;
  градиенты — рукописные per-layer. Скалярный `AutogradGraph` от тензорного графа оторван.
- **FedAvg при таймауте** — `collect_updates` прерывается и молча усредняет по меньшему
  кругу (`federated.rs:1014`).
- `consensus.rs` содержит `panic!` на некорректных переходах раунда (строки 701/807/887).

### Transport
- **GraphSync** — `extract_links` → `Ok(Vec::new())`; обходит только корневой блок
  (`graphsync.rs:377`). Селекторы парсятся, но не применяются.
- **Erasure (Reed-Solomon)** — decode → `DecodingFailed`; encode — взвешенный XOR; нулевая
  устойчивость к потерям (`erasure.rs:299`).
- **NAT traversal** — STUN-рефлексивные адреса фейковые, «успех» захардкожен
  (`nat_traversal.rs:367,665`).
- **`MultiTransportManager::find_transport`** — fallback не может выбрать непредпочтённый
  транспорт (`multi_transport.rs:215`).
- `BitswapStats.total_bytes_*` захардкожены в 0 (`bitswap.rs:324`).

### Storage
- **Целостность при чтении НЕ проверяется** — `get` → `from_parts(*cid, data)` без `verify()`
  (`blockstore.rs:350`). Тихая порча диска отдаётся как валидные данные.
- **`StorageGarbageCollector` игнорирует возраст** — нет `min_age` (`garbage_collector.rs:140`);
  блок с `ref_count==0` собираем немедленно (гонка с записью).
- **Пины не транзакционны с данными** — `PinManager` это in-memory `DashMap` с файловой
  сериализацией; крах между удалением блока и обновлением пина оставляет рассинхрон.
- Фабрикованные метрики: «коэффициент сжатия» в `cold_storage` выводится из хеша CID
  (`cold_storage.rs:378`).

### Application / Gateway
- **TLS-генератор в node** — `SelfSignedCertGenerator::generate()` пишет фейковый PEM,
  rcgen только в комментарии (`ipfrs/src/tls.rs:314`). (Gateway-TLS через rustls — реальный.)
- **Дублированная Auth-модель** — два разных enum `Role`/`Permission`/`User` в `ipfrs` и
  `ipfrs-interface`; риск расхождения авторизации.
- Дефолтный секрет `"default_secret_change_in_production"` (`auth.rs:574`).
- In-memory `UserStore`/`ApiKeyStore` (не персистятся).
- `GcConfig.min_age_seconds` существует, но **не enforced** в цикле сборки (`gc.rs`).
- Indirect-пины не персистятся между рестартами (риск over-collection).
- `ShutdownCoordinator::wait_internal` — `sleep(100ms)` placeholder (`shutdown.rs:103`).
- Несколько `network_ops` — placeholder'ы (`bitswap_stats`, `ping`, `find_peer`).

---

## 3. Опровергнутые «факты» из старых заметок

| Утверждение (старое) | Реальность | Источник |
|----------------------|------------|----------|
| JWT подписан **MD5** (auth.rs:449) | ✅ реальный **HMAC-HS256** (`jsonwebtoken`) | `ipfrs/src/auth.rs:461` |
| Backpressure-семафор «не освобождает permits» (backpressure.rs:182) | ✅ корректен (eager + deferred forget), покрыт тестами | `ipfrs-interface/src/backpressure.rs:185` |
| Баг FedAvg-таймаута в `tensorlogic_ops.rs:1131` | файла/строки **не существует**; реальный FL — `gradient/federated.rs` | `gradient/federated.rs:1014` |
| `gossipsub`/`bitswap` — поля `IpfrsBehaviour` | их там **нет** (7 behaviour'ов) | `node.rs:288` |

> Часть этих пунктов раньше числилась «критическими багами» в RoadMap. По факту кода
> два из трёх security-пунктов (JWT, backpressure) **уже корректны**, а третий (TLS-stub)
> локализован в node-крейте, тогда как gateway-TLS реален. RoadMap стоит обновить.

---

## 4. Расхождения старых вики с кодом (ключевые)

### Shared Kernel (`Wiki_Arch_GLM/02-SharedKernel.md` — ≥12 ошибок)
`CidBuilder::build`, `Block::from_parts` (на деле infallible, без валидации),
`TensorBlock` (поля `{block, metadata}`, не `{block,shape,dtype}`), `Error` (16 других
вариантов), коды хешей, `Priority` (4 уровня, не 5), формат CAR (не чистый CARv1).

### Storage
Реальный стек — `Bloom→Cache→Sled` (не `Cache→Tiering→Sled`); `SledBlockStore` — 3 поля;
`ShardedBlockStore`/circuit-breaker-декоратор не существуют; кэш это `Mutex<LruCache>` (не
DashMap+VecDeque).

### Network
`NetworkEvent` не содержит `DhtQueryCompleted`/`GossipsubMessage`; `peer_store` живёт в
facade, не в `NetworkNode`; единого newtype `PeerId(String)` нет.

### Semantic
Jaccard-метрики нет; дефолт метрики L2 в `with_defaults` (не cosine везде); `convert_distance`
формулы иные; DiskANN без PQ; логико-семантический fallback **реализован** (вики: «будущее»).

### TensorLogic (крупнейший пробел)
Вики документируют 2-3 движка — реально **20+** (см. [[07-TensorLogicContext]] §2). IPLD-term
`Tensor` как буквальный мост символика↔тензор не описан.

### Transport
`WantHave`/want-have-vs-want-block, `PeerLedger`, «fair/leeching» — **не существуют**;
`BitswapExchange` не держит `SessionManager`; erasure — XOR-заглушка, не Reed-Solomon.

---

## 5. Технический долг (приоритеты для RoadMap)

1. **Завершить Network→Transport выкачку блоков по swarm** (`node.rs:1311`) — без неё P2P
   GET по проводу не работает. **Блокер.**
2. **Включить целостность при чтении** в Storage (`verify()` или опция) — безопасность данных.
3. **Починить `VectorIndex::rebuild`** — иначе компактизация индекса опустошает его.
4. **Консолидировать дубли**: два Bitswap, 3 GC, 4 тиринга, 4 модели репутации, 2 RL/fuzzy.
5. **Подключить Tier B network к swarm-событиям** или удалить как мёртвый код.
6. **Унифицировать Auth-модель** между `ipfrs` и `ipfrs-interface`.
7. **Реализовать GraphSync `extract_links`**, реальный Reed-Solomon, реальный STUN/TURN.

---

## Что дальше?

- **Вернуться к карте** → [[00-INDEX]]
- **Стратегические выводы (выделение контекстов)** → [[02-StrategicDesign]] §4
- **Как это влияет на сценарии** → [[10-DataFlows]]

**Связанные**: [[00-INDEX]] | [[02-StrategicDesign]] | [[10-DataFlows]]
**Источник кода**: весь `ipfrs_source/crates/` (анализ 7 агентов, 2026-06-19)
