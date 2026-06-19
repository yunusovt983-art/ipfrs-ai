# IPFRS — Глубокая Архитектура (Domain-Driven Design)

**Версия**: 0.2.0 "Network Release"  
**Статус**: Production Ready — P2P Networking Available  
**Дата анализа**: 2026-06-19  
**Аудитория**: Архитекторы и старшие инженеры

---

## Содержание

1. [Executive Summary](#1-executive-summary)
2. [Стратегический дизайн — Context Map](#2-стратегический-дизайн--context-map)
3. [Shared Kernel — ipfrs-core](#3-shared-kernel--ipfrs-core)
4. [Storage Bounded Context](#4-storage-bounded-context)
5. [Network Bounded Context](#5-network-bounded-context)
6. [Semantic Bounded Context](#6-semantic-bounded-context)
7. [Logic Bounded Context](#7-logic-bounded-context)
8. [Transport Bounded Context](#8-transport-bounded-context)
9. [Inference Engines — Глубокий Анализ](#9-inference-engines--глубокий-анализ)
10. [Паттерны DDD в IPFRS](#10-паттерны-ddd-в-ipfrs)

---

## 1. Executive Summary

IPFRS (InterPlanetary File Replication System) — **распределённая файловая система**, объединяющая **интеллектуальное хранилище** с **распределённым выводом** через content-addressing и семантические возможности.

### Ключевые принципы

| Принцип | Реализация | Файл |
|---------|------------|------|
| **Content-Addressed** | CID = Hash(content) | `ipfrs-core/src/cid.rs` |
| **Distributed** | libp2p P2P, DHT | `ipfrs-network/src/` |
| **Intelligent** | HNSW + TensorLogic | `ipfrs-semantic/`, `ipfrs-tensorlogic/` |
| **Pure Rust** | Memory-safe, zero-unsafe | Весь workspace |
| **Zero-Copy** | Apache Arrow, Bytes | `ipfrs-core/src/arrow.rs`, `block.rs` |

### Workspace Structure (12 crates)

```
ipfrs_source/crates/
├── ipfrs-core/         → SHARED KERNEL (Block, CID, Ipld, Tensor)
├── ipfrs-storage/      → Storage Context (BlockStore, GC, Tiering)
├── ipfrs-network/      → Network Context (Peer, DHT, Reputation)
├── ipfrs-semantic/     → Semantic Context (HNSW, DiskANN, Search)
├── ipfrs-tensorlogic/  → Logic Context (IR, Inference, Neural-Symbolic)
├── ipfrs-transport/    → Transport Context (Session, Bitswap, TensorSwap)
├── ipfrs-interface/    → Presentation (HTTP, gRPC, GraphQL)
├── ipfrs/              → APPLICATION FACADE (Node orchestrator)
├── ipfrs-cli/          → CLI (Clap)
├── ipfrs-wasm/         → WebAssembly bindings
├── ipfrs-nodejs/       → Node.js bindings
└── ipfrs-python/       → Python bindings
```

### Статистика кодовой базы

| Crate | Файлов | LOC | Ключевые модули |
|-------|--------|-----|-----------------|
| ipfrs-core | 31 | ~17,600 | block.rs, cid.rs, ipld.rs, hash.rs |
| ipfrs-storage | 150+ | ~100,000+ | blockstore.rs, tiering.rs, gc.rs |
| ipfrs-network | 180+ | ~80,000+ | peer.rs, dht_provider.rs, reputation.rs |
| ipfrs-semantic | 140+ | ~70,000+ | hnsw.rs, diskann.rs, search_pipeline.rs |
| ipfrs-tensorlogic | 190+ | ~129,000+ | reasoning.rs, neural_symbolic.rs, ir.rs |
| ipfrs-transport | 45+ | ~25,000+ | session.rs, bitswap.rs, tensorswap/ |


---

## 2. Стратегический дизайн — Context Map

### 2.1 Диаграмма контекстов

```
                    ┌───────────────────────────────────────────────┐
                    │          PRESENTATION / HOST                  │
                    │  ipfrs-cli · ipfrs-interface (gRPC/GraphQL)   │
                    │  ipfrs-wasm · ipfrs-nodejs · ipfrs-python     │
                    └───────────────────────┬───────────────────────┘
                                            │
                    ┌───────────────────────▼───────────────────────┐
                    │       APPLICATION FACADE  (crate: ipfrs)      │
                    │  Node { storage, network, semantic,           │
                    │         tensorlogic, transport, metrics }     │
                    └───┬──────────┬──────────┬──────────┬──────────┘
                        │          │          │          │
          ┌─────────────▼───┐ ┌────▼─────┐ ┌──▼───────┐ ┌▼───────────────┐
          │  STORAGE        │ │ NETWORK  │ │ SEMANTIC │ │ LOGIC          │
          │  BlockStore     │ │ Peer     │ │ HNSW/    │ │ KnowledgeBase  │
          │  port+adapters  │ │ DHT      │ │ DiskANN  │ │ Term/Rule/Fact │
          └────────▲────────┘ └────▲─────┘ └────▲─────┘ └──────▲─────────┘
                   │               │            │              │
                   │          ┌────┴────────────┴──────────────┘
                   │          │   TRANSPORT (Session, Bitswap, TensorSwap)
                   └──────────┴─────────────────────────────────────────┐
                                                                        │
          ┌──────────────────────────────────────────────────────────────────┐
          │              SHARED KERNEL  (crate: ipfrs-core)                  │
          │   Cid · Block · Ipld · TensorBlock · Codec · HashEngine · CAR    │
          └──────────────────────────────────────────────────────────────────┘
```

### 2.2 Паттерны отношений между контекстами

| Отношение | Паттерн (Evans/Vernon) | Реализация |
|-----------|------------------------|------------|
| `ipfrs-core` → все контексты | **Shared Kernel** | `Cid`, `Block` импортируются каждым crate |
| Storage ← все | **Conformist / Open Host Service** | `BlockStore` trait — опубликованный порт |
| Transport → Storage | **Customer/Supplier + ACL** | `BitswapExchange<S: BlockStore>` знает только trait |
| Transport → Network | **Customer/Supplier** | PeerId из Network; репутация дублируется |
| All domain → libp2p | **Anti-Corruption Layer** | `libp2p::PeerId` → `String` domain VO |
| Logic → Storage | **Published Language (IPLD)** | `ipld_codec.rs` сериализует Rule/Term → Block |
| Presentation → Application | **Open Host Service / Facade** | gRPC/GraphQL/CLI через `Node` |

### 2.3 Ubiquitous Language

**CID (Content Identifier)** — центральный токен ubiquitous language:

```
Storage: "Храни блок по CID"
Network: "Найди пиров с CID"  
Semantic: "Проиндексируй embedding для CID"
Logic: "Правило имеет CID"
Transport: "Запроси CID у пира"
```

