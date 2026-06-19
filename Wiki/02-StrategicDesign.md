---
title: 02-StrategicDesign
type: architecture
summary: Стратегический DDD — 7 bounded contexts IPFRS, карта контекстов и типы отношений между ними
tags: [ipfrs, ddd, strategic-design, bounded-context, context-map]
related: ["[[01-DomainOverview]]", "[[03-SharedKernel]]", "[[09-ApplicationLayer]]"]
read_time: 14 мин
updated: 2026-06-19
---

# Стратегический дизайн: границы контекстов

**Краткое резюме**: IPFRS делится на **один Shared Kernel + 5 доменных bounded
contexts + 1 прикладной/шлюзовой контекст**. Связь между ними идёт почти
исключительно через один тип — **`Cid`**. Эта страница описывает карту контекстов
и тип каждого отношения (Shared Kernel / Conformist / ACL / Open Host Service).

---

## 1. Карта bounded contexts

| # | Bounded Context | Крейт | Роль DDD | Корневой агрегат |
|---|-----------------|-------|----------|------------------|
| 0 | **Shared Kernel** | `ipfrs-core` | Общее ядро типов | `Block` |
| 1 | **Storage** | `ipfrs-storage` | Supporting | `Block` через `BlockStore` |
| 2 | **Network** | `ipfrs-network` | Supporting | `NetworkNode` |
| 3 | **Semantic** | `ipfrs-semantic` | **Core** | `VectorIndex` / `DiskANNIndex` |
| 4 | **TensorLogic** | `ipfrs-tensorlogic` | **Core** | `KnowledgeBase`, `ComputationGraph` |
| 5 | **Transport** | `ipfrs-transport` | Supporting | `Session` |
| 6 | **Interface + Application** | `ipfrs-interface`, `ipfrs` | Generic + Facade | `Node`, `Gateway` |

Прочие крейты — это **языковые привязки** (`ipfrs-wasm`, `ipfrs-nodejs`,
`ipfrs-python`) и CLI (`ipfrs-cli`); они не вводят нового домена, а лишь экспонируют
существующий.

---

## 2. Карта контекстов (Context Map)

```
                          ┌────────────────────────────────────────┐
                          │  Interface / Gateway (ipfrs-interface) │
                          │  Open Host Service + Published Language│
                          │  gRPC · GraphQL · WebSocket · HTTP     │
                          └───────────────────┬────────────────────┘
                                              │ переводит wire → домен (ACL)
                          ┌───────────────────▼─────────────────────┐
                          │        Application (ipfrs::Node)        │
                          │            Facade / оркестратор         │
                          └──┬──────────┬─────────┬─────────┬──────┘
            Conformist       │          │         │         │   Conformist
        ┌────────────────────▼┐  ┌──────▼─────┐ ┌─▼────────┐      ┌▼──────────────────┐
        │   Storage           │  │  Network   │ │ Semantic │ │   TensorLogic      │
        │   (ipfrs-storage)   │  │(ipfrs-net.)│ │(semantic)│ │ (tensorlogic)      │
        └──────────┬──────────┘  └─────┬──────┘ └────┬─────┘ └─────────┬──────────┘
                   │                   │             │                 │
                   │            ┌──────▼─────────────▼──┐              │
                   │            │      Transport         │             │
                   │            │   (ipfrs-transport)    │             │
                   │            └───────────┬────────────┘             │
                   │  Shared Kernel         │ Shared Kernel            │
                   └───────────┬────────────┴─────────────┬───────────┘
                       ┌───────▼─────────────────────────▼───────┐
                       │       Shared Kernel (ipfrs-core)          │
                       │   Cid · Block · Ipld · Error · DAG        │
                       └───────────────────────────────────────────┘
```

---

## 3. Типы отношений между контекстами

### 3.1 Shared Kernel — `ipfrs-core`

Все контексты **разделяют** минимальный набор типов: `Cid`, `Block`, `Ipld`,
`Error`/`Result`, сервисы хеширования и кодеков, примитивы DAG. Это сознательно
**минимальное** ядро — всё специфичное (репутация, сессии, эмбеддинги) живёт вне
`ipfrs-core`.

> **Почему `Cid` — идеальный токен границы.** Он `Copy + Eq + Hash + Ord`
> (значимостная семантика, без жизненного цикла), самоописывающийся и
> самопроверяемый, и детерминированный. Это позволяет контекстам **доверять
> данным, а не друг другу**. Подробно — [[03-SharedKernel]].

### 3.2 Conformist: доменные контексты → Shared Kernel

Storage, Network, Semantic, TensorLogic, Transport **конформны** к `ipfrs-core`:
они принимают `Cid`/`Block` как есть, не оборачивая. Пример: Storage хранит блок по
ключу `cid.to_bytes()` (`blockstore.rs:223`); Semantic адресует эмбеддинги по `Cid`
(`hnsw.rs:88`); TensorLogic адресует правила по `Cid` (`ipld_codec.rs:171`).

### 3.3 Anti-Corruption Layer (ACL): шлюз → внешний мир

`ipfrs-interface` переводит внешние форматы в доменные типы, не пропуская
доменные внутренности наружу:
- multipart-форма → `Block::new(...)` (`gateway/routes.rs:385`);
- HTTP `Range` → срез байтов (`routes.rs:66`);
- gRPC-метаданные `Bearer` → доменное решение auth (`grpc.rs:1095`);
- C-ABI: наружу выходят только непрозрачные указатели (`ffi.rs:54`).

### 3.4 Open Host Service + Published Language

Шлюз — это **Open Host Service**: один домен (`Node`) опубликован через четыре
протокола (HTTP-gateway, gRPC, GraphQL, WebSocket). **Published Language** —
DAG-CBOR для связанных данных (`dag_ops.rs:38`) и версионированный бинарный кадр
(`binary_protocol.rs:19`).

### 3.5 Facade: `Node` оркеструет контексты

`ipfrs::Node` (`node/mod.rs:34`) держит по одному хэндлу на каждый контекст и
собирает их в сценарии. Подробно — [[09-ApplicationLayer]].

---

## 4. Важное наблюдение: «большой» крейт ≠ «много домена»

Глубокий анализ ([[11-RealityCheck]]) выявил структурную особенность всех крупных
контекстов: **небольшое нагруженное ядро + огромный спутниковый слой**.

| Контекст | Реальное ядро | Спутниковый слой |
|----------|---------------|------------------|
| Storage | `BlockStore` + ~20 декораторов | ~120 сервисов на отдельных реестрах, не подключённых к живому стору |
| Network | `NetworkNode` + swarm (Tier A) | ~180 модулей (репутация, бан-листы, пулы) почти не подключены к swarm (Tier B) |
| Semantic | HNSW/DiskANN/Router | ~100 модулей-сервисов, большинство даже не ссылается на `Cid` |
| TensorLogic | IR + 20+ движков + автоград | дублирование примитивов (несколько PRNG, 2 RL, 2 fuzzy) |

**Следствие для DDD**: некоторые контексты фактически выполняют работу нескольких
bounded contexts, втиснутых в один крейт. Storage, например, содержит ещё и
consensus (Raft), мульти-ДЦ координацию и tensor-VCS — кандидаты на выделение в
отдельные границы. Это зафиксировано как технический долг в [[11-RealityCheck]].

---

## 5. Направления зависимостей (кто кого знает)

```
ipfrs-interface ──▶ ipfrs (Node) ──▶ { storage, network, semantic, tensorlogic, transport }
                                            │         │         │            │            │
                                            └─────────┴────┬────┴────────────┴────────────┘
                                                           ▼
                                                      ipfrs-core
```

- **Никаких циклов**: всё течёт вниз к `ipfrs-core`.
- `transport` зависит от `storage` (порт `BlockStore`) и `ipfrs-core`.
- `network` неожиданно зависит от `tensorlogic` (для распределённого вывода через
  gossip — `node.rs:32`) — единственная «восходящая» по смыслу связь, её стоит
  держать в поле зрения.
- `semantic` зависит от `tensorlogic` (`solver.rs:12`) для логико-семантического
  моста и от `network` (для типа `PeerId` семантического DHT, `dht.rs:12`).

---

## Что дальше?

- **Фундамент, на котором всё держится** → [[03-SharedKernel]]
- **Как контексты собираются в узел** → [[09-ApplicationLayer]]
- **Где модель расходится с кодом** → [[11-RealityCheck]]

**Связанные**: [[01-DomainOverview]] | [[03-SharedKernel]] | [[09-ApplicationLayer]] | [[11-RealityCheck]]
**Источник кода**: `crates/*` (границы), `crates/ipfrs/src/node/` (оркестрация)
