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


---

## 3. Shared Kernel — ipfrs-core

**Shared Kernel** — центральный bounded context, содержащий типы и абстракции, используемые всеми остальными контекстами. Изоляция ядра гарантирует согласованность ubiquitous language.

### 3.1 Агрегаты и Value Objects

```
┌─────────────────────────────────────────────────────────────────────┐
│                    SHARED KERNEL (ipfrs-core)                       │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  AGGREGATES:                                                        │
│  ─────────────────────────────────────────────────────────────────  │
│  Block           { cid: Cid, data: Bytes }    [content-addressed]   │
│  TensorBlock     { cid: Cid, tensor: Tensor } [ML workloads]        │
│  ContentManifest { cid: Cid, entries: Vec<ManifestEntry> }          │
│  CAR             { version: u1, roots: Vec<Cid>, blocks: Vec<Block>}│
│                                                                     │
│  VALUE OBJECTS:                                                     │
│  ─────────────────────────────────────────────────────────────────  │
│  Cid             — Multibase + Multicodec + Hash                    │
│  Ipld            — JSON/CBOR-compatible recursive data              │
│  Hash            — blake3 | sha2-256 | sha2-512                     │
│  Codec           — Raw | Json | Cbor | DagCbor | DagJson | Custom   │
│                                                                     │
│  DOMAIN SERVICES:                                                   │
│  ─────────────────────────────────────────────────────────────────  │
│  HashEngine      — blake3::hash, sha2::Sha256, incremental hashing  │
│  CidBuilder      — Cid::new_v1(codec, hash)                         │
│  IpldCodec       — encode/decode Ipld ↔ Vec<u8>                     │
│                                                                     │
│  PORTS (traits):                                                    │
│  ─────────────────────────────────────────────────────────────────  │
│  BlockStore      — put/get/has/delete (см. Storage Context)         │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 3.2 CID — Content Identifier

**CID** — центральный value object, идентификатор контента:

```rust
// core/src/cid.rs
pub struct Cid {
    version: u64,        // CIDv1
    codec: Codec,        // DagCbor, Raw, Json...
    hash: Hash,          // blake3, sha2-256...
    bytes: Bytes,        // Cached multibase encoding
}

impl Cid {
    pub fn new_v1(codec: Codec, hash: Hash) -> Self;
    pub fn from_bytes(bytes: &[u8]) -> Result<Self>;
    pub fn to_bytes(&self) -> &[u8];
    pub fn to_string(&self) -> String;  // base32btc
}
```

**Инвариант**: `hash(data) == cid.hash` — CID вычисляется из контента, не назначается.

### 3.3 Block — Content-Addressed Aggregate

```rust
// core/src/block.rs
pub struct Block {
    cid: Cid,
    data: Bytes,         // Zero-copy via bytes::Bytes
}

impl Block {
    pub fn new(data: Bytes) -> Result<Self> {
        let hash = HashEngine::blake3(&data);
        let cid = Cid::new_v1(Codec::DagCbor, hash);
        Ok(Self { cid, data })
    }
    
    // Размер: 1 B ≤ len ≤ 2 MiB
    pub const MAX_SIZE: usize = 2 * 1024 * 1024;
    pub const MIN_SIZE: usize = 1;
}
```

**Инварианты**:
- `hash(self.data) == self.cid.hash`
- `MIN_SIZE ≤ self.data.len() ≤ MAX_SIZE`
- Блоки иммутабельны — нет метода `set_data()`

### 3.4 Ipld — InterPlanetary Linked Data

```rust
// core/src/ipld.rs
#[derive(Clone, Debug, PartialEq)]
pub enum Ipld {
    Null,
    Bool(bool),
    Integer(i128),
    Float(f64),
    String(String),
    Bytes(Vec<u8>),
    List(Vec<Ipld>),
    Map(BTreeMap<String, Ipld>),  // BTreeMap → canonical ordering
    Link(Cid),
}

impl Ipld {
    pub fn encode(&self, codec: Codec) -> Result<Vec<u8>>;
    pub fn decode(data: &[u8], codec: Codec) -> Result<Self>;
}
```

**Published Language**: IPLD — стандарт сериализации для межконтекстного обмена.

### 3.5 Ключевые файлы

| Файл | LOC | Назначение |
|------|-----|------------|
| `cid.rs` | 400+ | CID construction, parsing, multibase |
| `block.rs` | 300+ | Block aggregate, zero-copy |
| `ipld.rs` | 250+ | Recursive data structure |
| `hash.rs` | 200+ | blake3, sha2, multihash |
| `codec.rs` | 150+ | Multicodec registry |
| `arrow.rs` | 300+ | Apache Arrow integration |
| `car.rs` | 400+ | CAR file format (archive) |

---

## 4. Storage Bounded Context

Storage Context отвечает за **durable, content-addressed block persistence** через порты и адаптеры.

### 4.1 Port — BlockStore Trait

```rust
// storage/traits.rs
#[async_trait]
pub trait BlockStore: Send + Sync {
    async fn put(&self, block: &Block) -> Result<()>;
    async fn put_many(&self, blocks: &[Block]) -> Result<()>;
    async fn get(&self, cid: &Cid) -> Result<Option<Block>>;
    async fn get_many(&self, cids: &[Cid]) -> Result<Vec<Option<Block>>>;
    async fn has(&self, cid: &Cid) -> Result<bool>;
    async fn delete(&self, cid: &Cid) -> Result<()>;
    fn list_cids(&self) -> Result<Vec<Cid>>;
    fn len(&self) -> usize;
    async fn flush(&self) -> Result<()>;
    async fn close(&self) -> Result<()>;
}
```

**Hexagonal Architecture**: `BlockStore` — порт, любой адаптер может его реализовать.

### 4.2 Adapters — Backend Implementations

```
┌─────────────────────────────────────────────────────────────────────┐
│                    BLOCKSTORE ADAPTERS                              │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  SledBlockStore        — Primary (embedded ACID KV)                 │
│  ParityDbBlockStore    — SSD-optimized                              │
│  S3BlockStore          — Cloud object storage (multipart upload)    │
│  MemoryBlockStore      — Testing (DashMap<Cid, Block>)              │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 4.3 Decorators — Cross-Cutting Concerns

```rust
// Decorator stack (порядок важен!)
pub type ProductionBlockStore = TtlBlockStore<
    QuotaBlockStore<
        CachedBlockStore<
            DedupBlockStore<
                CompressionBlockStore<
                    EncryptedBlockStore<SledBlockStore>
                >
            >
        >
    >
>;
```

| Decorator | Назначение | Файл |
|-----------|------------|------|
| `CachedBlockStore` | LRU/LFU hot cache | `cache.rs` |
| `DedupBlockStore` | Content-defined dedup | `dedup.rs` |
| `QuotaBlockStore` | Per-tenant limits | `quota.rs` |
| `CompressionBlockStore` | Zstd/Lz4/Snappy | `compression.rs` |
| `EncryptedBlockStore` | ChaCha20/AES-GCM | `encryption.rs` |
| `TtlBlockStore` | Time-to-live expiration | `ttl.rs` |

### 4.4 Pin Management

```rust
pub enum PinType {
    Direct,      // Single block
    Recursive,   // Entire DAG
    Indirect,    // Referenced by recursive pin
}

pub struct PinInfo {
    pub cid: Cid,
    pub pin_type: PinType,
    pub ref_count: u64,
    pub pinned_at: u64,
}

// Инвариант: Pin-protected blocks cannot be GC'd
```

### 4.5 Garbage Collector

```rust
pub struct GarbageCollector<S: BlockStore> {
    store: Arc<S>,
    pin_manager: Arc<PinManager>,
}

impl<S: BlockStore> GarbageCollector<S> {
    pub async fn run(&self) -> Result<GcStats> {
        // Phase 1: Mark — traverse from pin roots
        let marked = self.mark_phase().await?;
        
        // Phase 2: Sweep — delete unmarked
        let deleted = self.sweep_phase(marked).await?;
        
        Ok(GcStats { marked, deleted })
    }
}
```

### 4.6 Ключевые файлы

| Файл | LOC | Назначение |
|------|-----|------------|
| `traits.rs` | 100+ | BlockStore trait |
| `blockstore.rs` | 400+ | SledBlockStore |
| `paritydb.rs` | 300+ | ParityDB adapter |
| `s3.rs` | 350+ | S3 adapter |
| `cache.rs` | 250+ | CachedBlockStore |
| `dedup.rs` | 300+ | DedupBlockStore |
| `pinning.rs` | 350+ | PinManager |
| `gc.rs` | 400+ | GarbageCollector |
| `tiering.rs` | 300+ | TieredStore |
| `wal.rs` | 250+ | Write-ahead log |

---

## 5. Network Bounded Context

Network Context отвечает за **peer identity, discovery, reputation, DHT content routing**.

### 5.1 NetworkNode — libp2p Wrapper

```rust
// network/node.rs
pub struct NetworkNode {
    swarm: Swarm<NetworkBehaviour>,
    peer_store: Arc<PeerStore>,
    config: NetworkConfig,
    event_tx: mpsc::UnboundedSender<NetworkEvent>,
}

#[derive(NetworkBehaviour)]
pub struct NetworkBehaviour {
    kademlia: Kademlia<MemoryStore>,
    identify: Identify,
    ping: Ping,
    autonat: AutoNAT,
    dcutr: DCUtR,          // Hole punching
    mdns: TokioMdns,       // Local discovery
    relay: Relay,
    gossipsub: Gossipsub,
}
```

### 5.2 Peer Aggregate

```rust
// network/peer.rs
pub struct PeerInfo {
    pub peer_id: String,            // VO: libp2p::PeerId stringified (ACL)
    pub addrs: Vec<String>,         // Multiaddrs
    pub protocols: Vec<String>,
    pub last_seen: u64,
    pub connection_count: u64,
    pub avg_latency_ms: Option<u64>,
    pub reputation: u8,             // 0..=100
}

pub struct PeerStore {
    peers: DashMap<PeerId, PeerRecord>,
    connected: RwLock<HashSet<PeerId>>,
}
```

### 5.3 Two-Tier Reputation Model

```
┌─────────────────────────────────────────────────────────────────────┐
│                    REPUTATION (2-TIER)                              │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Tier 1: EWMA ReputationScore                                       │
│  ─────────────────────────────────────────────────────────────────  │
│  transfer_success_rate: f64      // EWMA                            │
│  latency_score: f64              // EWMA                            │
│  protocol_compliance_score: f64  // EWMA                            │
│  uptime_score: f64               // EWMA                            │
│                                                                     │
│  Tier 2: PeerReputationGraph (Trust Graph)                          │
│  ─────────────────────────────────────────────────────────────────  │
│  BFS propagation, damping 0.5/hop, depth 3                          │
│  combined = 0.6×direct + 0.4×propagated                             │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

```rust
// network/peer_reputation_graph.rs
impl PeerReputationGraph {
    pub fn compute_propagated_score(&self, peer_id: &PeerId) -> f64 {
        // BFS with damping factor 0.5^depth
        // Max depth: 3 hops
    }
}
```

### 5.4 DHT Abstraction

```rust
// network/dht_provider.rs
#[async_trait]
pub trait DhtProvider: Send + Sync {
    async fn bootstrap(&self) -> Result<()>;
    async fn provide(&self, cid: &Cid) -> Result<()>;
    async fn find_providers(&self, cid: &Cid) -> Result<Vec<PeerId>>;
    async fn find_peer(&self, peer_id: &PeerId) -> Result<Vec<Multiaddr>>;
}
```

### 5.5 Semantic DHT

**LSH-based routing** для кластеризации похожего контента:

```rust
// network/semantic_dht.rs
pub struct SemanticDht {
    lsh_projections: Vec<Vec<f32>>,
    namespaces: EnumMap<NamespaceId, NamespaceConfig>,
}

pub enum NamespaceId {
    Text,
    Image,
    Audio,
    Custom(u64),
}
```

### 5.6 Ключевые файлы

| Файл | LOC | Назначение |
|------|-----|------------|
| `node.rs` | 400+ | NetworkNode |
| `peer.rs` | 350+ | PeerStore, PeerInfo |
| `reputation.rs` | 300+ | EWMA reputation |
| `peer_reputation_graph.rs` | 400+ | Trust graph |
| `dht_provider.rs` | 250+ | DHT trait |
| `semantic_dht.rs` | 350+ | LSH routing |
| `facade.rs` | 500+ | NetworkFacade builder |

---

## 6. Semantic Bounded Context

Semantic Context отвечает за **vector similarity search** и **embedding pipeline**.

### 6.1 Index Aggregates

```
┌─────────────────────────────────────────────────────────────────────┐
│                    SEMANTIC CONTEXT                                 │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  VectorIndex (HNSW)       — In-memory, <10M vectors                 │
│  DiskANNIndex             — Disk-based, billion-scale               │
│                                                                     │
│  PARAMETERS:                                                        │
│  ─────────────────────────────────────────────────────────────────  │
│  M (max neighbors)        — 16/32/48 (auto-tuned by size)           │
│  ef_construction          — 200/400/600                             │
│  ef_search                — 50/100/200                              │
│                                                                     │
│  LAYER ASSIGNMENT:                                                  │
│  layer = -ln(U(0,1)) / ln(2)                                        │
│  Layer 0: ~50%, Layer 1: ~25%, Layer 2: ~12.5%...                   │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 6.2 HNSW Operations

```rust
// semantic/hnsw.rs
impl VectorIndex {
    pub async fn insert(&self, cid: &Cid, vector: Vec<f32>) -> Result<()>;
    pub async fn search(&self, query: &[f32], k: usize) -> Result<Vec<SearchResult>>;
    pub async fn delete(&self, cid: &Cid) -> Result<()>;  // Soft delete
}
```

### 6.3 Product Quantization

```
Compression: D×4 bytes → M bytes

Vector (D × 4 bytes)
    ↓ Split into M subvectors
Subvectors (D/M × 4 bytes each)
    ↓ Quantize each to nearest centroid
QuantizerCode (M bytes)

Example: 768×4 = 3072 → 32 bytes = 96× compression
With codebook sharing: up to 12,000×
```

### 6.4 Query Pipeline

```rust
pub enum ExecutionStrategy {
    LocalOnly,
    RemoteFanout { max_peers: usize },
    Hybrid { local_ratio: f64 },
    Cached { ttl: Duration },
}

pub enum FusionMethod {
    WeightedCombination { weights: Vec<f64> },
    ReciprocalRankFusion { k: usize },
    LearnToRank { model: LtrModel },
}
```

### 6.5 Ключевые файлы

| Файл | LOC | Назначение |
|------|-----|------------|
| `hnsw.rs` | 600+ | HNSW index |
| `diskann.rs` | 500+ | DiskANN index |
| `vector_quantizer.rs` | 400+ | Product Quantization |
| `embedding_pipeline.rs` | 300+ | Normalization |
| `query_planner.rs` | 350+ | Execution planning |
| `reranking.rs` | 250+ | Fusion/reranking |
| `sharding.rs` | 400+ | Shard coordination |
| `simd.rs` | 200+ | AVX2/NEON |

---

## 7. Logic Bounded Context

Logic Context — самый сложный bounded context, объединяющий **symbolic reasoning**, **tensor computation**, и **neural-symbolic integration**.

### 7.1 Архитектура Context

```
┌─────────────────────────────────────────────────────────────────────┐
│                    LOGIC CONTEXT (ipfrs-tensorlogic)                │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  KNOWLEDGE REPRESENTATION:                                          │
│  ─────────────────────────────────────────────────────────────────  │
│  KnowledgeBase (AR)     — Term/Rule/Fact с CID-identity             │
│  ProofTree              — Sound, acyclic proof structure            │
│  IpldCodec              — Published Language для сериализации       │
│                                                                     │
│  TENSOR EXECUTION:                                                  │
│  ─────────────────────────────────────────────────────────────────  │
│  ComputationGraph (DAG)  — Nodes contain TensorOp                   │
│  AutogradGraph           — Reverse-mode automatic diff              │
│  TensorArena             — Bump allocator для inference             │
│  TensorPool              — Slab-based buffer pool                   │
│  TensorGC                — Mark-and-sweep garbage collector         │
│  TensorQuantizer         — INT8/INT4/FP16/BF16 compression          │
│  TensorDiffEngine        — Federated learning diff                  │
│  TensorChecksumEngine    — Corruption detection                     │
│                                                                     │
│  INFERENCE ENGINES:                                                 │
│  ─────────────────────────────────────────────────────────────────  │
│  InferenceEngine         — SLD Resolution                           │
│  TabledInferenceEngine   — SLG Tabling (avoids infinite loops)      │
│  TemporalReasoningEngine — Allen's 13 relations                     │
│  FuzzyLogicEngine        — Mamdani/Sugeno inference                 │
│  EpistemicLogicReasoner  — S5 Kripke semantics                      │
│  ProbabilisticLogicNetwork — PLN uncertain reasoning                │
│  BayesianNetworkInference — VE/BP/Gibbs sampling                    │
│  NeuralSymbolicIntegrator — Hybrid neural-symbolic                  │
│  DistributedBackwardChainer — Cross-peer reasoning                  │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 7.2 Tensor Memory Management

#### 7.2.1 TensorArena — Bump Allocator

```rust
// tensorlogic/src/tensor_arena.rs
pub struct TensorArena {
    pub regions: Vec<ArenaRegion>,    // Fixed-size slabs
    pub region_size: usize,           // Default: 1 MiB
    pub stats: ArenaStats,
}

pub struct ArenaRegion {
    slab: Vec<u8>,       // Pre-allocated memory
    offset: usize,       // Bump pointer
    capacity: usize,
}

pub struct ArenaSlice {
    pub region_index: usize,
    pub start: usize,
    pub end: usize,
}
```

**Операции**:
- `allocate(size)` → O(1) — bump pointer increment
- `reset_all()` → O(regions) — вернуть всю память

**Alignment**: 8-byte для всех allocations

#### 7.2.2 TensorPool — Slab-based Buffer Pool

```rust
// tensorlogic/src/tensor_pool.rs
pub struct TensorPool {
    free_lists: [Mutex<Vec<Vec<u8>>>; 8],  // 8 buckets
    stats: TensorPoolStats,
}

// Size classes (power-of-two):
// Bucket 0: 0-255 B    → allocate 256 B
// Bucket 1: 256-511 B  → allocate 512 B
// Bucket 2: 512-1023 B → allocate 1 KiB
// ...
// Bucket 7: >16 KiB    → allocate exact size (cap 32 MiB)
```

**Thread-safe**: каждый bucket под отдельным Mutex

**Статистика**: `total_acquired`, `total_released`, `total_allocs`, `total_reuses`

#### 7.2.3 TensorGC — Mark-and-Sweep Garbage Collector

```rust
// tensorlogic/src/tensor_gc.rs
pub struct TensorGarbageCollector {
    pub tensors: HashMap<u64, TensorRef>,
    pub roots: Vec<u64>,
}

pub struct TensorRef {
    pub tensor_id: u64,
    pub name: Option<String>,
    pub size_bytes: u64,
    pub ref_count: u32,          // External references
    pub dependencies: Vec<u64>,  // Outgoing edges
    pub pinned: bool,            // Never collect
}

pub enum GcPhase {
    MarkRoots,  // Seed from roots + pinned
    Trace,      // BFS through dependency edges
    Sweep,      // Remove unreachable with ref_count=0
}
```

**Инварианты**:
- `pinned == true` → никогда не собирается
- `ref_count > 0` → никогда не собирается
- BFS max depth: не ограничено (граф должен быть acyclic)

#### 7.2.4 TensorQuantizer — Multi-Precision Compression

```rust
// tensorlogic/src/tensor_quantizer.rs
pub enum QuantizationMode {
    Int8Symmetric,   // scale = percentile(|x|) / 127
    Int8Asymmetric,  // scale + zero_point, range [0, 255]
    Int4,            // scale = percentile(|x|) / 7
    Fp16,            // 5-bit exp, 10-bit mantissa
    Bf16,            // Top 16 bits of f32
}

pub struct QuantizerConfig {
    pub mode: QuantizationMode,
    pub per_channel: bool,           // Per-channel scale/zp
    pub channel_dim: usize,          // Axis for channels
    pub calibration_percentile: f64, // e.g. 99.9 (outlier suppression)
}
```

**Compression Ratio**:
- INT8: 64/8 = 8×
- INT4: 64/4 = 16×
- FP16/BF16: 64/16 = 4×

#### 7.2.5 TensorDiffEngine — Federated Learning Diff

```rust
// tensorlogic/src/tensor_diff.rs
pub enum DiffKind {
    Added,
    Removed,
    ShapeChanged { old_shape: Vec<usize>, new_shape: Vec<usize> },
    ValueChanged { max_abs_diff: f32, mean_abs_diff: f32, changed_elements: usize },
    Unchanged,
}

pub struct TensorDiffEngine {
    pub threshold: f32,  // Value difference threshold
}

impl TensorDiffEngine {
    pub fn diff_tensors(&self, old: &TensorSnapshot, new: &TensorSnapshot) -> TensorDiff;
    pub fn diff_snapshots(&self, old: &[TensorSnapshot], new: &[TensorSnapshot]) -> Vec<TensorDiff>;
    pub fn summarize(&self, diffs: &[TensorDiff]) -> DiffSummary;
}
```

**Use-case**: Change detection в federated learning checkpoints

#### 7.2.6 TensorChecksumEngine — Corruption Detection

```rust
// tensorlogic/src/tensor_checksum.rs
pub enum ChecksumAlgorithm {
    Fnv1a64,     // Fast non-cryptographic
    Adler32,     // zlib-compatible
    Fletcher16,  // Lightweight
    XorFold,     // Ultra-fast for large tensors
}

pub struct TensorChecksumEngine {
    pub records: HashMap<u64, ChecksumRecord>,
    pub stats: ChecksumEngineStats,
}

// Pure-Rust implementations:
pub fn fnv1a64(data: &[u8]) -> u64;
pub fn adler32(data: &[u8]) -> u64;
pub fn fletcher16(data: &[u8]) -> u64;
pub fn xor_fold(data: &[u8]) -> u64;
```

### 7.3 ComputationGraph — DAG Execution

```rust
// tensorlogic/src/computation_graph.rs (1723 lines)
pub struct ComputationGraph {
    nodes: HashMap<NodeId, ComputationNode>,
    outputs: Vec<NodeId>,
}

pub struct ComputationNode {
    pub id: NodeId,
    pub op: TensorOp,
    pub inputs: Vec<NodeId>,
    pub shape: Option<Vec<usize>>,
}

pub enum TensorOp {
    Input { name: String },
    Constant { data: Vec<f32> },
    MatMul { a: NodeId, b: NodeId },
    Add { a: NodeId, b: NodeId },
    Einsum { equation: String, inputs: Vec<NodeId> },
    Softmax { input: NodeId, axis: usize },
    LayerNorm { input: NodeId, eps: f32 },
    // ... 40+ operations
    FusedLinear { input: NodeId, weight: NodeId, bias: Option<NodeId> },
}
```

**Optimizations**:
- Topological sort (Kahn's algorithm)
- CSE (Common Subexpression Elimination)
- Constant folding
- Operation fusion (FusedLinear, FusedAddReLU, etc.)

### 7.4 AutogradGraph — Reverse-Mode AD

```rust
// tensorlogic/src/autograd.rs
pub struct AutogradGraph {
    nodes: HashMap<NodeId, AutogradNode>,
}

pub enum AutogradOp {
    Input { requires_grad: bool },
    Add { a: NodeId, b: NodeId },
    Mul { a: NodeId, b: NodeId },
    MatMul { a: NodeId, b: NodeId },
    // ...
}

impl AutogradGraph {
    pub fn backward(&self, loss: NodeId) -> HashMap<NodeId, Vec<f32>>;
}
```

**Алгоритм**: Iterative post-order DFS для backward pass

### 7.5 Inference Engines Deep Dive

#### 7.5.1 InferenceEngine — SLD Resolution

```rust
// tensorlogic/src/reasoning.rs
pub struct InferenceEngine {
    rules: Vec<Rule>,
    facts: HashSet<Fact>,
}

impl InferenceEngine {
    pub fn query(&self, goal: &Term) -> Result<Vec<Substitution>>;
    
    // SLD: Select → Literal → Derive
    fn sld_resolution(&self, goals: Vec<Term>, subst: Substitution) 
        -> Result<Vec<Substitution>>;
}
```

#### 7.5.2 TabledInferenceEngine — SLG Tabling

```rust
// tensorlogic/src/recursive_reasoning.rs
pub struct TabledInferenceEngine {
    tables: HashMap<Predicate, Table>,
    worklist: VecDeque<Goal>,
}

// Табулирование предотвращает infinite loops в рекурсивных правилах
```

#### 7.5.3 TemporalReasoningEngine — Allen's 13 Relations

```rust
// tensorlogic/src/temporal_reasoning.rs
pub enum TemporalRelation {
    Before, After, Meets, MetBy,
    Overlaps, OverlappedBy, Starts, StartedBy,
    Finishes, FinishedBy, During, Contains, Equals,
}

impl TemporalReasoningEngine {
    pub fn infer_relations(&self, intervals: &[Interval]) -> Vec<(Interval, Interval, TemporalRelation)>;
}
```

#### 7.5.4 FuzzyLogicEngine — Mamdani/Sugeno

```rust
// tensorlogic/src/fuzzy_logic.rs
pub struct FuzzyLogicEngine {
    membership_functions: HashMap<String, MembershipFunction>,
    rules: Vec<FuzzyRule>,
}

pub enum MembershipFunction {
    Triangular { a: f64, b: f64, c: f64 },
    Trapezoidal { a: f64, b: f64, c: f64, d: f64 },
    Gaussian { mean: f64, sigma: f64 },
}
```

#### 7.5.5 EpistemicLogicReasoner — S5 Kripke Semantics

```rust
// tensorlogic/src/epistemic_logic.rs
pub struct EpistemicLogicReasoner {
    possible_worlds: Vec<World>,
    accessibility: HashMap<Agent, Vec<(World, World)>>,
}

// Modal operators: K (knows), B (believes), ◻ (necessarily), ◇ (possibly)
```

#### 7.5.6 ProbabilisticLogicNetwork — PLN

```rust
// tensorlogic/src/probabilistic_logic_network.rs
pub struct ProbabilisticLogicNetwork {
    atoms: HashMap<Atom, TruthValue>,
    rules: Vec<InferenceRule>,
}

pub struct TruthValue {
    pub strength: f64,   // [0, 1]
    pub confidence: f64, // [0, 1]
}
```

#### 7.5.7 BayesianNetworkInference

```rust
// tensorlogic/src/bayesian_network_inference.rs
pub struct BayesianNetworkInference {
    nodes: HashMap<String, BnNode>,
    algorithm: InferenceAlgorithm,
}

pub enum InferenceAlgorithm {
    VariableElimination,
    BeliefPropagation,
    GibbsSampling { iterations: usize },
}
```

#### 7.5.8 NeuralSymbolicIntegrator

```rust
// tensorlogic/src/neural_symbolic.rs
pub struct NeuralSymbolicIntegrator {
    neural_encoder: NeuralEncoder,
    symbolic_reasoner: InferenceEngine,
    neural_decoder: NeuralDecoder,
}

impl NeuralSymbolicIntegrator {
    pub fn forward(&self, input: &Tensor) -> Result<Tensor>;
    pub fn explain(&self, output: &Tensor) -> Result<Vec<Explanation>>;
}
```

### 7.6 Gradient System

**Файл**: `gradient/mod.rs` (~500 LOC)

Gradient types для federated learning и distributed training.

```rust
// Sparse gradient — CSR format
pub struct SparseGradient {
    pub indices: Vec<usize>,
    pub values: Vec<f32>,
    pub shape: Vec<usize>,
}

// Compressed gradient transmission
pub struct QuantizedGradient {
    pub data: Vec<i8>,
    pub scale: f32,
    pub zero_point: i32,
    pub shape: Vec<usize>,
}

// Delta encoding for bandwidth reduction
pub struct GradientDelta {
    pub base_version: u64,
    pub deltas: Vec<GradientDeltaEntry>,
}

pub struct GradientDeltaEntry {
    pub param_name: String,
    pub indices: Vec<usize>,
    pub values: Vec<f32>,
}

// Differential Privacy
pub struct DifferentialPrivacy {
    pub mechanism: DpMechanism,
    pub epsilon: f64,
    pub delta: f64,
}

pub enum DpMechanism {
    Gaussian { sigma: f64 },
    Laplace { scale: f64 },
}

// Secure Aggregation
pub struct SecureAggregation {
    pub secret_shares: Vec<Vec<u8>>,
    pub public_keys: HashMap<PeerId, Vec<u8>>,
}
```

**Compression**: Delta encoding + quantization → до **100×** bandwidth reduction

---

### 7.7 Proof System

**Файлы**: `proof_tree.rs`, `proof_cache.rs`, `provenance.rs`

```rust
// Proof tree — sound, acyclic structure
pub struct ProofTree {
    pub goal: Predicate,
    pub rule: Option<Rule>,
    pub subproofs: Vec<ProofTree>,
    pub attribution: Option<PeerAttribution>,
}

pub struct ProofNode {
    pub id: ProofNodeId,
    pub goal: Predicate,
    pub rule_cid: Option<Cid>,
    pub children: Vec<ProofNodeId>,
}

// LRU cache for proven goals
pub struct ProofCache {
    cache: LruCache<Predicate, ProofTree>,
    max_size: usize,
}

// Source attribution
pub struct Provenance {
    pub source: ProvenanceSource,
    pub confidence: f64,
    pub timestamp: u64,
}

pub enum ProvenanceSource {
    Local { kb_version: u64 },
    Remote { peer_id: PeerId, signature: Vec<u8> },
    Derived { from: Vec<Cid> },
}
```

**Инварианты**:
- Proof is sound (каждый node ↔ KB rule)
- Proof is acyclic (tree structure)
- ProofTree cacheable по goal CID

---

### 7.8 IR — Intermediate Representation

**Файлы**: `ir.rs`, `term_index.rs`, `rule_index.rs`, `rule_dependency.rs`

```rust
// Core IR types (value objects)
pub enum Term {
    Var(String),                    // ?X, ?Y
    Const(Constant),                // Ground term
    Fun(String, Vec<Term>),         // f(X, g(Y))
    Ref(TermRef),                   // CID-addressed external
}

pub struct Predicate {
    pub name: String,
    pub args: Vec<Term>,
}

pub struct Rule {
    pub head: Predicate,
    pub body: Vec<Predicate>,
}

// Hash-consing for interning
pub struct TermIndex {
    terms: HashMap<Term, TermId>,
    by_id: HashMap<TermId, Term>,
}

pub struct RuleIndex {
    rules: HashMap<Rule, RuleId>,
    head_index: HashMap<Predicate, Vec<RuleId>>,  // Fast lookup by head
}

// Dependency analysis
pub struct RuleDependencyGraph {
    nodes: HashMap<RuleId, RuleNode>,
    edges: Vec<(RuleId, RuleId)>,
}

impl RuleDependencyGraph {
    pub fn topological_sort(&self) -> Result<Vec<RuleId>>;
    pub fn detect_cycles(&self) -> Vec<Vec<RuleId>>;
}
```

---

### 7.9 OpFusion — Pattern Matching

**Файл**: `op_fusion.rs` (~300 LOC)

Greedy pattern matching для operation fusion.

**Fusion Patterns**:

| Pattern | Before | After |
|---------|--------|-------|
| ScaleBias | `Mul + Add` | `FusedScaleBias` |
| ScaleBiasRelu | `Mul + Add + Relu` | `FusedScaleBiasRelu` |
| ClampNormalize | `Clamp + Div` | `FusedClampNorm` |
| Linear | `MatMul + Add` | `FusedLinear` |

```rust
pub struct OpFusion;

impl OpFusion {
    pub fn fuse(graph: &mut ComputationGraph) -> Result<usize>;
}
```

**Performance gain**: 15-30% latency reduction за счёт kernel fusion

---

### 7.10 OpDispatcher — Backend Routing

**Файл**: `op_dispatcher.rs` (~250 LOC)

```rust
pub enum Backend {
    Cpu,
    Gpu,
    Remote { endpoint: String },
    Simulated { latency_ms: u64 },
}

pub struct OpDispatcher {
    backends: HashMap<BackendType, Box<dyn BackendExecutor>>,
    fallback: BackendType,
}

pub trait BackendExecutor {
    fn execute(&self, op: &TensorOp, inputs: &[Vec<f32>]) -> Result<Vec<f32>>;
    fn is_available(&self) -> bool;
}
```

**Routing Logic**:
1. Check primary backend availability
2. Fall back to `fallback` if unavailable
3. Return error if both fail

---

### 7.11 Performance Metrics

#### 7.11.1 Tensor Memory Performance

| Operation | P50 | P99 | Notes |
|-----------|-----|-----|-------|
| Arena allocate | 10 ns | 50 ns | O(1) bump pointer |
| Pool acquire | 100 ns | 1 µs | Lock contention |
| Pool release | 100 ns | 1 µs | Lock contention |
| GC collect | 1 ms | 10 ms | Graph size dependent |
| Quantize (1M) | 5 ms | 20 ms | Mode dependent |
| Checksum | 100 µs | 1 ms | Algorithm dependent |

#### 7.11.2 Inference Performance

| Operation | P50 | P99 | Notes |
|-----------|-----|-----|-------|
| Simple query | 1 ms | 5 ms | SLD Resolution |
| Recursive (tabling) | 5 ms | 50 ms | Depth dependent |
| Distributed query | 100 ms | 1000 ms | Network dominant |
| Proof verify | 0.5 ms | 5 ms | Tree size dependent |

#### 7.11.3 Compression Ratios

| Mode | Compression | Use-case |
|------|-------------|----------|
| INT8 | 8× | General quantization |
| INT4 | 16× | Aggressive compression |
| FP16 | 4× | Mixed precision |
| BF16 | 4× | Training stability |
| Gradient Delta | up to 100× | Federated bandwidth |

---

### 7.12 Ключевые файлы Logic Context

| Категория | Файл | LOC | Назначение |
|-----------|------|-----|------------|
| **IR** | ir.rs | 350+ | Term, Predicate, Rule, Fact |
| **IR** | term_index.rs | 200+ | HashCons for Terms |
| **IR** | rule_index.rs | 200+ | HashCons for Rules |
| **IR** | rule_dependency.rs | 300+ | DAG dependency analysis |
| **Memory** | tensor_arena.rs | 300+ | Bump allocator |
| **Memory** | tensor_pool.rs | 450+ | Slab-based buffer pool |
| **Memory** | tensor_gc.rs | 400+ | Mark-and-sweep GC |
| **Memory** | tensor_quantizer.rs | 1250+ | Multi-precision quantization |
| **Memory** | tensor_diff.rs | 350+ | Federated learning diff |
| **Memory** | tensor_checksum.rs | 400+ | Corruption detection |
| **Execution** | computation_graph.rs | 1723+ | DAG execution |
| **Execution** | autograd.rs | 400+ | Reverse-mode AD |
| **Execution** | op_fusion.rs | 300+ | Pattern matching fusion |
| **Execution** | op_dispatcher.rs | 250+ | Backend routing |
| **Inference** | reasoning.rs | 800+ | SLD Resolution |
| **Inference** | recursive_reasoning.rs | 600+ | SLG Tabling |
| **Inference** | temporal_reasoning.rs | 500+ | Allen's algebra |
| **Inference** | fuzzy_logic.rs | 400+ | Mamdani/Sugeno |
| **Inference** | epistemic_logic.rs | 450+ | S5 Kripke |
| **Inference** | probabilistic_logic_network.rs | 700+ | PLN |
| **Inference** | bayesian_network_inference.rs | 500+ | BN inference |
| **Inference** | neural_symbolic.rs | 600+ | Hybrid integration |
| **Inference** | distributed_backward_chainer.rs | 400+ | Cross-peer reasoning |
| **Gradient** | gradient/mod.rs | 500+ | Gradient types |
| **Gradient** | backward_pass.rs | 300+ | Backward pass |
| **Gradient** | federated.rs | 400+ | Federated aggregation |
| **Proof** | proof_tree.rs | 400+ | Proof structure |
| **Proof** | proof_cache.rs | 200+ | LRU cache |
| **Proof** | provenance.rs | 250+ | Source attribution |

---

## 8. Transport Bounded Context

Transport Context отвечает за **block exchange protocols** между пирами.

### 8.1 Session Aggregate

```rust
// transport/session.rs
pub struct Session {
    pub session_id: SessionId,
    pub want_list: WantList,
    pub blocks_received: usize,
    pub blocks_failed: usize,
    pub total_blocks: usize,
    pub started_at: Instant,
}

pub struct WantList {
    entries: BTreeMap<Cid, WantEntry>,
}

pub struct WantEntry {
    pub cid: Cid,
    pub priority: u8,
    pub cancel: bool,
}
```

**Инвариант**: Session completes only when `blocks_received + blocks_failed >= total_blocks`

### 8.2 BitswapExchange

```rust
// transport/bitswap.rs
pub struct BitswapExchange<S: BlockStore> {
    store: Arc<S>,
    sessions: DashMap<SessionId, Session>,
    peer_stats: DashMap<PeerId, PeerStats>,
    config: BitswapConfig,
}

impl<S: BlockStore> BitswapExchange<S> {
    pub async fn want(&self, cid: &Cid) -> Result<Block>;
    pub async fn want_many(&self, cids: &[Cid]) -> Result<Vec<Block>>;
    pub async fn cancel(&self, cid: &Cid);
}
```

### 8.3 TensorSwap — Distributed Gradient Exchange

```rust
// transport/tensorswap/mod.rs
pub struct TensorSwap<S: BlockStore> {
    store: Arc<S>,
    gradient_cache: GradientCache,
    coordinator: BackwardPassCoordinator,
}

pub struct BackwardPassCoordinator {
    steps: Vec<BackwardPassStep>,
    aggregation: AggregationMethod,
}

pub enum AggregationMethod {
    Sum,
    Mean,
    WeightedMean,
    FedAvg,
}
```

### 8.4 Ключевые файлы

| Файл | LOC | Назначение |
|------|-----|------------|
| `session.rs` | 300+ | Session aggregate |
| `bitswap.rs` | 400+ | Bitswap protocol |
| `tensorswap/mod.rs` | 250+ | Tensor exchange |
| `tensorswap/coordinator.rs` | 300+ | Backward pass coordination |

---

## 9. Inference Engines — Глубокий Анализ

### 9.1 Таксономия Inference Engines

```
┌─────────────────────────────────────────────────────────────────────┐
│                    INFERENCE ENGINE TAXONOMY                        │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  DEDUCTIVE REASONING:                                               │
│  ─────────────────────────────────────────────────────────────────  │
│  InferenceEngine         → SLD Resolution (Prolog-style)            │
│  TabledInferenceEngine   → SLG Tabling (recursion-safe)             │
│  DistributedBackwardChainer → Cross-peer reasoning                  │
│                                                                     │
│  TEMPORAL REASONING:                                                │
│  ─────────────────────────────────────────────────────────────────  │
│  TemporalReasoningEngine → Allen's 13 interval relations            │
│                                                                     │
│  UNCERTAIN REASONING:                                               │
│  ─────────────────────────────────────────────────────────────────  │
│  FuzzyLogicEngine        → Mamdani/Sugeno fuzzy inference           │
│  EpistemicLogicReasoner  → S5 Kripke (knowledge/belief)             │
│  ProbabilisticLogicNetwork → PLN (strength + confidence)            │
│  BayesianNetworkInference → Variable Elimination / Gibbs            │
│                                                                     │
│  HYBRID REASONING:                                                  │
│  ─────────────────────────────────────────────────────────────────  │
│  NeuralSymbolicIntegrator → Neural encoder + symbolic reasoner      │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 9.2 Сравнительная таблица

| Engine | Paradigm | Use-case | Complexity |
|--------|----------|----------|------------|
| InferenceEngine | Deductive | Expert systems | O(goals × rules) |
| TabledInferenceEngine | Deductive | Recursive rules | O(table_size) |
| TemporalReasoningEngine | Temporal | Event ordering | O(n²) intervals |
| FuzzyLogicEngine | Fuzzy | Control systems | O(rules × fuzzification) |
| EpistemicLogicReasoner | Modal | Multi-agent | O(worlds² × agents) |
| ProbabilisticLogicNetwork | Probabilistic | Uncertain KB | O(atoms × rules) |
| BayesianNetworkInference | Probabilistic | Causal reasoning | O( treewidth × n) |
| NeuralSymbolicIntegrator | Hybrid | Explainable AI | O(neural) + O(symbolic) |
| DistributedBackwardChainer | Distributed | P2P reasoning | O(hops × local_inference) |

### 9.3 Integration Patterns

```
                    ┌───────────────────────────────────────────────┐
                    │          NeuralSymbolicIntegrator             │
                    │  ───────────────────────────────────────────  │
                    │  NeuralEncoder → SymbolicReasoner → Decoder   │
                    └───────────────────────┬───────────────────────┘
                                            │
          ┌─────────────────────────────────┼─────────────────────────────────┐
          │                                 │                                 │
          ▼                                 ▼                                 ▼
┌─────────────────┐             ┌─────────────────────┐           ┌─────────────────┐
│ InferenceEngine │             │ TemporalReasoning   │           │ FuzzyLogicEngine│
│ (Deductive)     │             │ (Temporal)          │           │ (Fuzzy)         │
└─────────────────┘             └─────────────────────┘           └─────────────────┘
          │                                 │                                 │
          └─────────────────────────────────┼─────────────────────────────────┘
                                            │
                                            ▼
                              ┌─────────────────────────┐
                              │   KnowledgeBase (AR)    │
                              │   Term/Rule/Fact → CID  │
                              └─────────────────────────┘
```

---

## 10. Паттерны DDD в IPFRS

### 10.1 Strategic Patterns

| Паттерн | Реализация | Описание |
|---------|------------|----------|
| **Shared Kernel** | `ipfrs-core` | Cid, Block, Ipld — общие типы |
| **Context Map** | Architecture diagram | Явные границы и отношения |
| **Open Host Service** | `BlockStore` trait | Публикация порта |
| **Anti-Corruption Layer** | libp2p wrappers | Изоляция external libs |
| **Published Language** | IPLD | Стандарт сериализации |
| **Customer/Supplier** | Transport → Network | Upstream dependency |
| **Conformist** | Storage consumers | Downstream conform to port |

### 10.2 Tactical Patterns

| Паттерн | Пример | Инвариант |
|---------|--------|-----------|
| **Aggregate Root** | `Block` | `hash(data) == cid` |
| **Aggregate Root** | `Peer` | `peer_id = hash(pubkey)` |
| **Aggregate Root** | `Session` | `recv + fail ≥ total` |
| **Value Object** | `Cid` | Immutable, identity by value |
| **Value Object** | `Ipld` | Recursive, copy-on-write |
| **Entity** | `PinInfo` | Mutable ref_count |
| **Domain Service** | `HashEngine` | Stateless operation |
| **Domain Service** | `GarbageCollector` | Cross-aggregate operation |
| **Repository** | `PeerStore` | Collection-like interface |
| **Factory** | `CidBuilder` | Complex object creation |
| **Specification** | `IntegrityChecker` | Business rule encapsulation |

### 10.3 Intentional Duplication

**Network Reputation ≠ Transport Peer Scoring**

```
Network Reputation:
  - Long-term trust
  - EWMA + Trust Graph
  - Used for DHT routing decisions

Transport Peer Scoring:
  - Per-session quality
  - Immediate transfer metrics
  - Used for Bitswap peer selection
```

**Rationale**: Разные bounded contexts имеют разные модели одного доменного понятия.

### 10.4 Event-Driven Integration

```rust
// Cross-context events (not event sourcing)
pub enum StorageEvent {
    BlockStored { cid: Cid, size: usize },
    BlockDeleted { cid: Cid },
    PinAdded { cid: Cid, pin_type: PinType },
    PinRemoved { cid: Cid },
}

pub enum NetworkEvent {
    PeerConnected { peer_id: PeerId },
    PeerDisconnected { peer_id: PeerId },
    DhtProviderAdded { cid: Cid },
}
```

### 10.5 ACL Patterns

```rust
// Anti-Corruption Layer к libp2p
pub fn peer_id_to_string(peer_id: &libp2p::PeerId) -> String {
    peer_id.to_base58()
}

pub fn string_to_peer_id(s: &str) -> Result<libp2p::PeerId> {
    libp2p::PeerId::from_str(s).map_err(|_| Error::InvalidPeerId)
}
```

**Цель**: Domain layer не зависит от libp2p типов напрямую.

---

## 11. Performance Model

### 11.1 Latency Targets

| Operation | P50 | P99 | Throughput |
|-----------|-----|-----|------------|
| Block GET (cache) | 30µs | 50µs | 33k ops/s |
| Block PUT | 50µs | 80µs | 20k ops/s |
| HNSW search (k=10) | 1ms | 10ms | 1k q/s |
| DiskANN search | 5ms | 50ms | 200 q/s |
| DHT lookup | 150ms | 300ms | 100 q/s |
| Inference (simple) | 1ms | 5ms | — |
| Inference (distributed) | 100ms | 1000ms | — |

### 11.2 Memory Model

| Component | Memory |
|-----------|--------|
| Node (minimal) | ~50 MB |
| + Semantic (100k vectors) | ~500 MB |
| + Logic (10k rules) | ~100 MB |
| HNSW (10M, 768-d, M=16) | ~30 GB |
| PQ compression | 12,000× (30GB → 2.5MB) |
| TensorArena | Configurable region_size |
| TensorPool | 8 buckets × max_per_bucket |

### 11.3 Scalability

```
Vertical Scaling:
  - SledBlockStore: ~1M blocks per node
  - HNSW: ~10M vectors in-memory
  - DiskANN: Billion+ vectors on-disk

Horizontal Scaling:
  - DHT Kademlia: O(log n) lookup
  - Semantic DHT: LSH-based clustering
  - DistributedBackwardChainer: Cross-peer reasoning
```

---

## 12. Extension Points

### 12.1 Adding New BlockStore Backend

```rust
#[async_trait]
impl BlockStore for MyCustomStore {
    async fn put(&self, block: &Block) -> Result<()> { /* ... */ }
    async fn get(&self, cid: &Cid) -> Result<Option<Block>> { /* ... */ }
    // ... implement all trait methods
}
```

### 12.2 Adding New Inference Engine

```rust
pub struct MyInferenceEngine {
    // ... engine state
}

impl MyInferenceEngine {
    pub fn query(&self, goal: &Term) -> Result<Vec<Substitution>> { /* ... */ }
}
```

### 12.3 Adding New Quantization Mode

```rust
pub enum QuantizationMode {
    Int8Symmetric,
    Int8Asymmetric,
    Int4,
    Fp16,
    Bf16,
    MyCustomMode,  // Add here
}
```

---

## 13. Summary

### 13.1 Key Architectural Decisions

| Decision | Rationale | Trade-off |
|----------|-----------|-----------|
| Shared Kernel (ipfrs-core) | Ubiquitous language consistency | Coupling cost |
| Port/Adapter (BlockStore) | Backend flexibility | Indirection overhead |
| Decorator Stack | Composable cross-cutting | Deep stack debugging |
| Two-Tier Reputation | Separate trust models | Duplicate logic |
| ACL to libp2p | Domain isolation | Wrapper maintenance |
| Tensor Arena | O(1) allocation | No individual free |
| Mark-Sweep GC | Simple, correct | Serial phases |

### 13.2 Design Principles

1. **CID as Universal Boundary Token** — Every ACL passes a CID
2. **Content-Addressing is Identity** — `hash(data) == identity`
3. **Frozen Aggregates** — Immutability enforced by design
4. **Decorator Stack for Cross-Cutting** — Storage concerns are composable
5. **Intentional Duplication** — Context autonomy over DRY
6. **Lazy Context Init** — Pay only for what you use
7. **Port/Adapter Pattern** — BlockStore trait = central port

---

**Документ завершён.** 

Для углублённого изучения см. `/Wiki_Arch_GLM/` — детальные документы по каждому bounded context.
