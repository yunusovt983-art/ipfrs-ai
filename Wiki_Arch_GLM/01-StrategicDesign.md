# Strategic Design — Context Map & Bounded Contexts

> **Focus**: DDD Strategic Patterns, context boundaries, integration patterns  
> **Sources**: `ipfrs_source/ARCHITECTURE_DDD_DEEP.md`, crate analysis

---

## 1. The Big Picture

IPFRS — это **modular monolith**, организованный как Cargo workspace из 12 crates. С точки зрения DDD:

- **1 Shared Kernel** (`ipfrs-core`)
- **5 Domain Contexts** (storage, network, semantic, logic, transport)
- **2 Presentation/Host Contexts** (`ipfrs` facade + `ipfrs-interface`, bindings)
- **Ubiquitous Language Token**: `Cid` (content identifier)

```
┌─────────────────────────────────────────────────────────────────────┐
│                    IPFRS STRATEGIC MAP                              │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│                    ┌──────────────────────────┐                     │
│                    │   PRESENTATION / HOST    │                     │
│                    │                          │                     │
│                    │  CLI · gRPC · GraphQL    │                     │
│                    │  HTTP · WS · FFI · Py    │                     │
│                    └────────────┬─────────────┘                     │
│                                 │                                   │
│                                 │ Open Host Service                 │
│                                 ▼                                   │
│                    ┌──────────────────────────┐                     │
│                    │   APPLICATION FACADE     │                     │
│                    │        (ipfrs)           │                     │
│                    │                          │                     │
│                    │  Node { storage,         │                     │
│                    │    network, semantic,    │                     │
│                    │    tensorlogic, ... }    │                     │
│                    └─┬────────┬────────┬──────┘                     │
│                      │        │        │                            │
│        ┌─────────────┘        │        └─────────────┐              │
│        │                      │                      │              │
│        ▼                      ▼                      ▼              │
│  ┌───────────┐        ┌───────────────┐      ┌───────────────┐      │
│  │  STORAGE  │        │   NETWORK     │      │   SEMANTIC    │      │
│  │           │        │               │      │               │      │
│  │ BlockStore│        │ NetworkNode   │      │ VectorIndex   │      │
│  │ (port)    │        │ PeerStore     │      │ DiskANN       │      │
│  │           │        │ DHT           │      │ Quantizer     │      │
│  └─────┬─────┘        └───────┬───────┘      └───────┬───────┘      │
│        │                      │                      │              │
│        └──────────────────────┼──────────────────────┘              │
│                               │                                     │
│                    ┌──────────▼──────────┐                          │
│                    │     TRANSPORT       │                          │
│                    │                     │                          │
│                    │  Session            │                          │
│                    │  BitswapExchange    │                          │
│                    │  WantList           │                          │
│                    └──────────┬──────────┘                          │
│                               │                                     │
│                    ┌──────────▼──────────┐                          │
│                    │       LOGIC         │                          │
│                    │                     │                          │
│                    │  KnowledgeBase      │                          │
│                    │  InferenceEngine    │                          │
│                    │  Neural-Symbolic    │                          │
│                    └─────────────────────┘                          │
│                                                                     │
│                    ┌──────────────────────────┐                     │
│                    │     SHARED KERNEL        │                     │
│                    │       (ipfrs-core)       │                     │
│                    │                          │                     │
│                    │  Cid · Block · Ipld ·    │                     │
│                    │  TensorBlock · Manifest  │                     │
│                    └──────────────────────────┘                     │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 2. Bounded Contexts

### 2.1 Shared Kernel — `ipfrs-core`

**Ответственность**: Domain primitives, value objects, cross-context types

**Ключевые типы**:
- `Cid` — Content Identifier (Value Object)
- `Block` — Content-addressed block (Aggregate Root)
- `Ipld` — IPLD data model (Value Object)
- `TensorBlock` — ML tensor wrapper (Aggregate Root)
- `ContentManifest` — Multi-file manifest (Aggregate Root)
- `HashEngine` — Hash abstraction (Domain Service)
- `Codec` — Encoding/decoding (Domain Service)

**Invariants**:
- `hash(data) == CID`
- `1 ≤ block.len ≤ 2 MiB`
- `Ipld::Map` uses `BTreeMap` (canonical encoding)

---

### 2.2 Storage Context — `ipfrs-storage`

**Ответственность**: Durable, content-addressed block persistence

**Ключевые типы**:
- `BlockStore` trait — Port (Repository pattern)
- `SledBlockStore` — Primary adapter
- `CachedBlockStore`, `DedupBlockStore`, etc. — Decorators
- `PinManager` — Pin management
- `GarbageCollector` — Mark-sweep GC
- `TieredStore` — Hot/Warm/Cold/Archive tiering

**Invariants**:
- Blocks are immutable
- Pin-protected blocks cannot be GC'd
- WAL durability for crash recovery

---

### 2.3 Network Context — `ipfrs-network`

**Ответственность**: Peer identity, discovery, reputation, DHT content routing

**Ключевые типы**:
- `NetworkNode` — libp2p swarm wrapper
- `PeerStore` — Peer repository
- `ReputationManager` — EWMA scoring
- `PeerReputationGraph` — Trust graph propagation
- `DhtProvider` trait — DHT abstraction
- `SemanticDht` — LSH-based semantic routing

**Invariants**:
- `PeerId = hash(pubkey)`
- Reputation ∈ [0, 100]
- Trust propagation depth ≤ 3

---

### 2.4 Semantic Context — `ipfrs-semantic`

**Ответственность**: Approximate nearest neighbor vector search

**Ключевые типы**:
- `VectorIndex` — HNSW in-memory index
- `DiskANNIndex` — Disk-based billion-scale index
- `VectorQuantizer` — Product Quantization
- `EmbeddingPipeline` — Normalization pipeline
- `ReRanker` — Result fusion

**Invariants**:
- Vector dimension must match index
- No NaN/Inf in vectors
- Cosine similarity ∈ [0, 1]

---

### 2.5 Logic Context — `ipfrs-tensorlogic`

**Ответственность**: Content-addressed symbolic reasoning + tensor computation

**Ключевые типы**:
- `Term`, `Predicate`, `Rule`, `KnowledgeBase` — IR
- `InferenceEngine` — SLD resolution
- `NeuralSymbolicIntegrator` — Hybrid inference
- `ComputationGraph` — Tensor DAG
- `ProofTree` — Peer-attributed proofs

**Invariants**:
- Rule dependency graph is acyclic
- Identical rule ⟹ identical CID
- Proof must be sound and acyclic

---

### 2.6 Transport Context — `ipfrs-transport`

**Ответственность**: Reliable block exchange protocol coordination

**Ключевые типы**:
- `Session` — Transfer session aggregate
- `WantList` — Priority queue
- `BitswapExchange` — Bitswap protocol
- `PeerManager` — Transport-local scoring
- `MultiTransport` — QUIC/TCP/WS fallback

**Invariants**:
- Session completes only when `recv + fail ≥ total`
- Block verified on receive
- WantList: one entry per CID

---

### 2.7 Application Facade — `ipfrs`

**Ответственность**: Compose all contexts, expose use cases

**Ключевые типы**:
- `Node` — Application orchestrator
- `NodeConfig` — Configuration

**Pattern**: Facade + lazy initialization

---

### 2.8 Presentation — `ipfrs-interface`, bindings

**Ответственность**: Expose domain via protocols/languages

**Ключевые типы**:
- gRPC, GraphQL, HTTP, WebSocket servers
- FFI, Python, Node.js, WASM bindings
- Auth, TLS, metrics

**Pattern**: Open Host Service + ACL

---

## 3. Context Mapping Patterns

По классификации Evans/Vernon:

### 3.1 Shared Kernel

```
ipfrs-core ──────► ALL CONTEXTS
```

**Реализация**: `Cid`, `Block`, `Ipld`, `Error` импортируются каждым crate. Single source of domain truth.

---

### 3.2 Customer/Supplier + ACL

```
Transport ──────► Storage
Transport ──────► Network
```

**Реализация**: 
- `BitswapExchange<S: BlockStore>` знает только trait, никогда Sled
- Transport потребляет `PeerId` из Network, но дублирует scoring (intentional)

---

### 3.3 Conformist / Open Host Service

```
ALL CONTEXTS ──────► Storage (BlockStore trait)
```

**Реализация**: `BlockStore` — published port. Все conform to trait interface.

---

### 3.4 Anti-Corruption Layer

```
All Domains ──────► libp2p
Bindings ──────► Application
```

**Реализация**:
- Network: `PeerId` wrapped as `String` (domain VO)
- FFI: opaque `#[repr(C)]` pointers
- Python: PyO3 wrappers with GIL management

---

### 3.5 Published Language (IPLD)

```
Logic ──────► Storage
```

**Реализация**: `Rule`/`Term` сериализуются в `Block` через IPLD codec. Rules = content-addressed.

---

### 3.6 Facade

```
Presentation ──────► Application (Node)
```

**Реализация**: gRPC/GraphQL/CLI funnel into `Node`. Never touch domain aggregates directly.

---

## 4. Intentional Duplication

### 4.1 Reputation vs Peer Scoring

**Network Context**:
- `ReputationManager` + `PeerReputationGraph`
- Long-term routing trust
- EWMA + graph-based propagation
- Scope: days/weeks

**Transport Context**:
- `PeerManager` with `PeerMetrics`
- Per-session transfer quality
- Simple EWMA
- Scope: minutes/hours

**Rationale**: Дублирование logic — цена за bounded-context autonomy. Network scoring для routing decisions, Transport scoring для immediate peer selection.

---

### 4.2 Why Not Shared?

Если бы reputation был в Shared Kernel:
- Network и Transport coupled
- Изменение scoring policy влияет на оба
- Тестирование сложнее
- Context boundaries размыты

С дублированием:
- Каждый context owns свою scoring model
- Independent evolution
- Clear responsibilities

---

## 5. Cross-Context Communication

### 5.1 Synchronous (Method Calls)

```
Application ──► Node.get_block()
                   │
                   ▼
             Storage.get()
                   │
                   ▼
             Cache.get() ──► Sled.get()
```

**Pattern**: Direct method calls на `Arc<...>` aggregates

---

### 5.2 Asynchronous (Channels)

```
Transport ──► SessionManager.create_session()
                 │
                 ▼
           Session (DashMap)
                 │
                 ├─► event_tx.send(SessionEvent::BlockReceived)
                 │
                 └─► state_tx.send(SessionState::Completing)
```

**Pattern**: `tokio::mpsc`/`watch`/`broadcast` channels

---

### 5.3 Events (Observability)

```
Storage ──► StorageEvent::BlockAdded
Network ──► NebNetworkEvent::PeerDiscovered
Transport ──► TransportEvent::PartitionDetected
```

**Pattern**: Event bus, NOT event sourcing

---

## 6. Dependency Direction

**Golden Rule**: Dependency always points inward.

```
Presentation ──► Application ──► Domain ──► Shared Kernel
                   │
                   └─► Cross-cutting (auth, tls, metrics)
```

**Violations**: None detected. Clean architecture.

---

## 7. Team Organization Implications

| Context | Team Size | Skills | Coupling |
|---------|-----------|--------|----------|
| Core | 2-3 | Rust, crypto | High (all depend on it) |
| Storage | 3-4 | Rust, databases | Low (port/adapter) |
| Network | 4-5 | Rust, p2p, libp2p | Medium (libp2p ACL) |
| Semantic | 3-4 | Rust, ML, ANN | Low (well-isolated) |
| Logic | 4-5 | Rust, logic, ML | Medium (neural-symbolic) |
| Transport | 2-3 | Rust, protocols | Medium (Network, Storage) |
| Interface | 3-4 | Rust, web, FFI | Low (facade only) |

---

## 8. Strategic Decisions

### 8.1 CID as Universal Token

**Decision**: `Cid` — единственный cross-context reference mechanism.

**Rationale**:
- Content-addressing = natural distribution
- No foreign aggregates
- ACLs are cheap (pass a CID)

**Trade-off**: Everything must be hashable/serializable to a block.

---

### 8.2 Modular Monolith

**Decision**: Single deployable unit, multiple crates.

**Rationale**:
- Simpler deployment
- Shared memory, no network overhead
- Cargo workspace manages dependencies

**Trade-off**: Cannot scale contexts independently.

---

### 8.3 State Mutation + Journals

**Decision**: Mutable stores with WAL/transaction-log, NOT event sourcing.

**Rationale**:
- Throughput matters for storage/network
- Crash recovery without rebuild cost

**Trade-off**: No audit trail by default (events are observability, not sourcing).

---

## 9. Extension Points

| Extension Point | Pattern | Example |
|-----------------|---------|---------|
| BlockStore backends | Adapter | `S3BlockStore`, `ParityDbBlockStore` |
| Storage decorators | Decorator | `QuotaBlockStore`, `EncryptedBlockStore` |
| Distance metrics | Strategy | `L2`, `Cosine`, `DotProduct` |
| Inference engines | Strategy | `TabledInferenceEngine`, `FuzzyLogicEngine` |
| Transports | Strategy | `QuicTransport`, `WebSocketTransport` |
| Peer selection | Strategy | `FastestFirst`, `RoundRobin` |
| Protocols | Open Host Service | gRPC, GraphQL, HTTP, WS |

---

## 10. Key Takeaways

1. **Shared Kernel is minimal** — Only truly shared types
2. **BlockStore is the central port** — Everyone conforms to it
3. **Intentional duplication exists** — Network ≠ Transport reputation
4. **CID is the ubiquitous language token** — Crosses all boundaries
5. **Clean dependency direction** — No cycles, inward-pointing
6. **Lazy initialization** — Pay only for what you use
7. **Events are observability, not sourcing** — Pragmatic choice

---

**Next**: [02-SharedKernel.md](02-SharedKernel.md) — Deep dive into ipfrs-core
