---
title: "IPFRS: Полная Архитектура (Sonnet 4.6)"
date: 2026-06-18
version: "0.2.0"
status: "Comprehensive Reference — supersedes IPFRS_ARCHITECTURE_MASTER.md"
crates: 12
source_files: "~1005 .rs files"
tags: [ipfrs, ddd, architecture, rust, distributed-systems]
---

# IPFRS: Deep Architecture (Sonnet 4.6)
*Полный код-обоснованный анализ — ~1005 файлов, 12 крейтов*

> Этот документ получен путём параллельного глубокого анализа всех 12 крейтов через 6 независимых агентов и является наиболее полным описанием архитектуры IPFRS на данный момент.

---

## Содержание

1. [[#1. Strategic Context Map]]
2. [[#2. Shared Kernel — ipfrs-core]]
3. [[#3. Storage Domain]]
4. [[#4. Network Domain]]
5. [[#5. Semantic Domain]]
6. [[#6. TensorLogic Domain]]
7. [[#7. Transport Domain]]
8. [[#8. Interface / Gateway Domain]]
9. [[#9. Application Layer — ipfrs Node]]
10. [[#10. CLI — ipfrs-cli]]
11. [[#11. Cross-Cutting Concerns]]
12. [[#12. Data Flows]]
13. [[#13. Invariants & Guarantees]]
14. [[#14. Performance Architecture]]
15. [[#15. DDD Pattern Inventory]]

---

## 1. Strategic Context Map

### Workspace Overview

```
/Volumes/Kingston/cool-japan/Vendor/ipfrs/
├── Cargo.toml              — workspace root, version 0.2.0, rust-version 1.90
├── crates/
│   ├── ipfrs-core          — Shared Kernel: primitives
│   ├── ipfrs-storage       — Storage Bounded Context
│   ├── ipfrs-network       — Network Bounded Context
│   ├── ipfrs-semantic      — Semantic Bounded Context
│   ├── ipfrs-tensorlogic   — TensorLogic Bounded Context
│   ├── ipfrs-transport     — Transport Bounded Context
│   ├── ipfrs-interface     — Interface / Gateway Bounded Context
│   ├── ipfrs               — Application Layer (Node facade)
│   ├── ipfrs-cli           — CLI presentation layer
│   ├── ipfrs-python        — Python FFI bindings (PyO3)
│   ├── ipfrs-nodejs        — Node.js FFI bindings (napi-rs)
│   └── ipfrs-wasm          — WebAssembly bindings
```

### Context Map с зависимостями

```
                    ┌─────────────────┐
                    │   ipfrs-core    │  ◄── Shared Kernel
                    │  (Block, Cid,   │      (imported by ALL crates)
                    │   Ipld, Error)  │
                    └────────┬────────┘
                             │ upstream
          ┌──────────────────┼───────────────────────┐
          │                  │                       │
   ┌──────▼──────┐   ┌───────▼──────┐   ┌────────────▼──────┐
   │ipfrs-storage│   │ipfrs-network │   │ipfrs-tensorlogic  │
   │  (BlockStore│   │(NetworkNode, │   │(KnowledgeBase,    │
   │   WAL, GC,  │   │ Gossip, DHT, │   │ InferenceEngine,  │
   │   Tiering)  │   │ NAT, Relay)  │   │ Neural-Symbolic)  │
   └──────┬──────┘   └───────┬──────┘   └────────────┬──────┘
          │                  │                       │
   ┌──────▼───────┐          │           ┌───────────▼───────┐
   │ipfrs-semantic│          │           │  ipfrs-transport  │
   │  (HNSW,      │          │           │  (Bitswap,        │
   │  DiskANN,    │◄─────────┘           │  GraphSync, QUIC) │
   │  LogicSolver │                      └───────────┬───────┘
   └──────┬───────┘                                  │
          │                                          │
          └────────────────┬─────────────────────────┘
                           │ imports from all domains
                   ┌───────▼───────┐
                   │     ipfrs     │  ◄── Application Service Layer
                   │   (Node,      │      (DI root, lifecycle mgmt)
                   │   GC, Auth,   │
                   │   PinManager) │
                   └───────┬───────┘
                    ┌──────┴───────┐
                    │              │
             ┌──────▼────────┐ ┌───▼────────────┐
             │ipfrs-interface│ │   ipfrs-cli    │
             │ (gRPC, HTTP,  │ │  (CLI cmds,    │
             │  GraphQL,     │ │   TUI, shell)  │
             │  OAuth2, FFI) │ └────────────────┘
             └───────┬───────┘
              ┌──────┴──────┐
              │             │
        ┌─────▼────┐ ┌──────▼──────┐
        │ipfrs-wasm│ │ipfrs-python │  ipfrs-nodejs
        └──────────┘ └─────────────┘
```

### Bounded Context Relationships

| Upstream | Downstream | Тип отношения |
|----------|------------|---------------|
| `ipfrs-core` | все остальные | Shared Kernel |
| `ipfrs-storage` | `ipfrs` (Node), `ipfrs-interface`, `ipfrs-semantic` | Customer-Supplier |
| `ipfrs-network` | `ipfrs` (Node), `ipfrs-interface`, `ipfrs-tensorlogic` | Customer-Supplier |
| `ipfrs-tensorlogic` | `ipfrs` (Node), `ipfrs-interface`, `ipfrs-semantic` | Customer-Supplier |
| `ipfrs-semantic` | `ipfrs` (Node), `ipfrs-interface` | Customer-Supplier |
| `ipfrs-transport` | `ipfrs` (Node), `ipfrs-interface` | Customer-Supplier |
| `ipfrs` | `ipfrs-cli`, `ipfrs-interface` | Published Language |

**Ключевое наблюдение:** `ipfrs-tensorlogic` зависит от `ipfrs-storage` (через `BlockStore`) и от `ipfrs-network` (через `GossipSub` topics). `ipfrs-network` в свою очередь осведомлён о `ipfrs-tensorlogic` через `inference_waiters` на `NetworkNode` — это bidirectional coupling, нарушающий чистую иерархию.

---

## 2. Shared Kernel — ipfrs-core

### Файловая структура (29 модулей)

```
ipfrs-core/src/
├── block.rs, cid.rs, ipld.rs      — Фундаментальные типы данных
├── tensor.rs, chunking.rs          — Специализированные типы
├── error.rs, types.rs             — Системные примитивы
├── hash.rs, codec_registry.rs     — Crypto + сериализация
├── compression.rs, car.rs         — Хранение
├── dag.rs, streaming.rs           — Обходы и потоки
├── batch.rs, pool.rs              — Batching и pooling
├── metrics.rs, config.rs          — Наблюдаемость
└── wasm_compat.rs, jose.rs        — Специфичные таргеты
```

### Block — фундаментальный тип

```rust
// block.rs:37-40
pub const MAX_BLOCK_SIZE: usize = 2 * 1024 * 1024;  // 2 MiB
pub const MIN_BLOCK_SIZE: usize = 1;

// block.rs:58
pub struct Block {
    cid: Cid,    // вычисляется при создании
    data: Bytes, // reference-counted bytes
}
```

**Инварианты Block:**
- `data.len() ∈ [1, 2 MiB]`
- `cid == CidBuilder::new().build(&data)` — проверяется через `Block::verify()`
- Равенство и хеширование делегируются только `Cid` — два блока с одинаковым CID взаимозаменяемы
- `Block::from_parts(cid, data)` — доверенный путь импорта, обходит проверку

### Cid — идентичность содержимого

```rust
// cid.rs:13
pub enum HashAlgorithm {
    #[default] Sha256, Sha512, Sha3_256, Sha3_512,
    Blake2b256, Blake2b512, Blake2s256, Blake3,
}
// cid.rs:185
pub enum MultibaseEncoding {
    #[default] Base32Lower, Base58Btc, Base64, Base64Url, Base32Upper,
}
// cid.rs:269 — Default: V1, codec=0x55 (RAW), SHA256
pub struct CidBuilder { version: cid::Version, codec: u64, hash_algorithm: HashAlgorithm }

// Codec constants:
// RAW=0x55, DAG_CBOR=0x71, DAG_JSON=0x0129, DAG_PB=0x70
```

**Важное замечание:** `Blake2b256Engine::digest()` вычисляет `blake2b_512(data)[..32]` — это усечённый Blake2b-512, а **не** native Blake2b-256 (hash.rs). Это приводит к несовместимости CID с другими IPFS реализациями.

### Ipld — унифицированное дерево данных

```rust
// ipld.rs:19
pub enum Ipld {
    Null, Bool(bool), Integer(i128), Float(f64),
    String(String), Bytes(Vec<u8>), List(Vec<Ipld>),
    Map(BTreeMap<String, Ipld>), Link(SerializableCid),
}
```

**DAG-CBOR особенности:**
- CID links: CBOR tag 42 + multibase identity prefix `0x00`
- `Ipld::Integer` хранит `i128`; JSON-сериализация использует `i64` если влезает, иначе quoted string
- f16→f64 конверсия реализована вручную в декодере

### TensorBlock — специализированный агрегат

```rust
// tensor.rs:193
pub struct TensorBlock { block: Block, metadata: TensorMetadata }
// Инвариант: data.len() == shape.element_count() * dtype.size_bytes()
// reshape() — zero-copy, переиспользует тот же Block
```

### Chunking / Merkle DAG

```rust
// chunking.rs:31
pub enum ChunkingStrategy { #[default] FixedSize, ContentDefined }
pub struct ChunkingConfig {
    strategy: ChunkingStrategy,
    chunk_size: usize,      // default 256 KiB
    min_chunk_size: usize,  // 1 KiB
    max_chunk_size: usize,  // 1 MiB
}
pub const MAX_LINKS_PER_NODE: usize = 174;

// Rabin-polynomial content-defined chunker:
struct RabinChunker {
    polynomial: u64,    // 0x3DA3358B4DC173
    window_size: usize, // 48 bytes
    mask: u64,
}
```

**Примечание:** `out_table()` — precomputed 256-entry dispatch table — помечен `#[allow(dead_code)]`. Chunker работает без таблицы, что значительно медленнее.

### CAR Format (Custom Extension)

```
[header: varint-len + CBOR]
[block: varint(cid_len+1+data_len) | cid_bytes | compression_flag:u8 | data]
```

Byte `compression_flag` — нестандартное расширение IPFRS (не CARv1). Если первый байт блока `== 0x00` или `== 0x01`, он будет ошибочно интерпретирован как флаг компрессии (car.rs:500-512).

### Error Hierarchy

```rust
// error.rs:38 — 16 вариантов
pub enum Error {
    Io(#[from] std::io::Error),
    BlockNotFound(String), Cid(String), Serialization(String),
    Deserialization(String), Network(String), Storage(String),
    Encryption(String), InvalidData(String), InvalidInput(String),
    NotFound(String), Protocol(String), NotImplemented(String),
    Internal(String), Initialization(String), Verification(String),
}
// Predicates: is_io(), is_not_found(), is_recoverable(), is_internal()
```

### Trait Hierarchy (ipfrs-core)

```
HashEngine (hash.rs)
  ├── Sha256Engine
  ├── Blake3Engine
  └── ... (8 алгоритмов)

Codec (codec_registry.rs)
  ├── DagCborCodec
  ├── DagJsonCodec
  └── RawCodec

BlockStore (ipfrs-storage/traits.rs — использует core types)
  ├── SledBlockStore
  ├── CachedBlockStore<S>
  ├── TieredStore<H,C>
  ├── MmapStore (feature-gated)
  ├── S3Store (feature-gated)
  └── ParityDB (feature-gated)
```

---

## 3. Storage Domain

**Crate:** `ipfrs-storage` — 163 файла

### BlockStore Trait (traits.rs:9)

```rust
#[async_trait]
pub trait BlockStore: Send + Sync {
    async fn put(&self, block: &Block) -> Result<()>;
    async fn put_many(&self, blocks: &[Block]) -> Result<()>;
    async fn get(&self, cid: &Cid) -> Result<Option<Block>>;
    async fn get_many(&self, cids: &[Cid]) -> Result<Vec<Option<Block>>>;
    async fn has(&self, cid: &Cid) -> Result<bool>;
    async fn has_many(&self, cids: &[Cid]) -> Result<Vec<bool>>;
    async fn delete(&self, cid: &Cid) -> Result<()>;
    async fn delete_many(&self, cids: &[Cid]) -> Result<()>;
    fn list_cids(&self) -> Result<Vec<Cid>>;  // синхронный!
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool;
    async fn flush(&self) -> Result<()>;      // default: no-op
    async fn close(&self) -> Result<()>;      // default: flush
}
// Arc<S: BlockStore> — blanket impl (traits.rs:87)
```

### Три несовместимых реализации WAL

| Тип | Файл | Стратегия переполнения | Транзакции |
|-----|------|------------------------|------------|
| `WriteAheadLog` | wal.rs | `Err` при переполнении | нет |
| `StorageWriteAheadLog` | wal.rs | кольцевой буфер (старые вытесняются) | нет |
| `WalWriteAheadLog` | write_ahead_log.rs | сегмент 64 MiB, ACID | `Begin/Commit/Rollback` |

```rust
// write_ahead_log.rs:22
pub const WAL_MAGIC: [u8; 4] = *b"WALX";
const MIN_ENTRY_BYTES: usize = 45;  // фиксированный overhead

// Бинарный формат:
// [magic(4)] [seq_num(8)] [tx_id(8)] [op_type(1)] [key_len(4)] [key(n)]
// [value_len(4)] [value(m)] [timestamp(8)] [checksum(8)]

pub struct WalWriteAheadLog {
    segment: Vec<u8>,
    entries: Vec<WalEntry>,
    transactions: HashMap<u64, Transaction>,
    next_seq: u64, next_tx_id: u64,
    config: WalConfig,
    last_checkpoint_seq: u64,
    writes_since_checkpoint: usize,
}
```

**Конфликт имён:** `lib.rs` алиасирует типы из `write_ahead_log.rs` как `WalWalConfig`, `WalWalEntry`, `WalWalStats` — уродливые двойные префиксы из-за коллизии с `wal.rs`.

### Tiering System (tiering.rs)

```rust
pub enum Tier { Hot, Warm, Cold, Archive }

pub struct TierConfig {
    pub hot_threshold: f64,     // 10.0 accesses/hr
    pub warm_threshold: f64,    // 1.0 accesses/hr
    pub cold_threshold: f64,    // 0.1 accesses/hr
    pub time_window_secs: u64,  // 3600
    pub decay_factor: f64,      // 0.9 (10% decay per cleanup)
    pub cleanup_interval_secs: u64,  // 300
}

pub struct TieredStore<H: BlockStore, C: BlockStore> {
    hot_store: H, cold_store: C, tracker: AccessTracker, config: TierConfig,
}
```

**Алгоритм классификации:**
```
access_rate = weighted_accesses * 3600.0 / time_window_secs
Hot   if rate >= 10.0/hr
Warm  if rate >= 1.0/hr
Cold  if rate >= 0.1/hr
Archive otherwise
```

**Критично:** `delete()` удаляет из обоих хранилищ без проверки существования — тихий no-op на каждом.

### Garbage Collector (block_garbage_collector.rs)

```rust
pub enum BgcGcPolicy {
    MarkAndSweep,        // BFS от root_set
    ReferenceCounting,   // sweep где ref_count == 0
    TriColor,            // Dijkstra tri-color
    Generational,        // только young-gen (gen==0)
}

pub struct BgcCollectorConfig {
    pub dry_run: bool,
    pub min_age_secs: u64,                    // default 300
    pub batch_size: usize,                    // default 1024
    pub generational_threshold_secs: u64,    // default 3600
}
```

**Скрытая проблема:** `gc_stats()` внутри вызывает `mark_phase()` — O(V+E) BFS при каждом запросе статистики.

**Jitter в часах:** `now_secs()` XOR-ит реальные часы с младшими 4 битами xorshift64 — ошибка до 15 секунд в продакшене.

### Data Integrity Checker (data_integrity_checker.rs)

```rust
pub enum IntegrityStatus {
    Valid,
    Corrupted,           // И размер И checksum неверны
    Missing,             // блок не найден в записях
    SizeMismatch,        // размер не тот, checksum ок
    ChecksumMismatch,    // checksum не тот, размер ок
}
// auto_quarantine = true по умолчанию — quarantine даже Missing
```

### Retention Policies (retention_policy.rs)

Два независимых движка:
- `StorageRetentionPolicyEngine` — wall-clock, priority-sorted rules
- `StorageRetentionPolicy` — tick-based

**Защита от случайного удаления:**
```rust
// Правило с max_age_ticks=None и max_size_bytes=None НИКОГДА не срабатывает
// Явная null-guard защита от "удали всё" при misconfigured catch-all rule
```

### Pinning (pinning.rs)

```rust
pub enum PinType { Direct, Recursive, Indirect }
pub struct PinManager {
    pins: DashMap<Vec<u8>, PinInfo>,  // key = cid.to_bytes()
    stats: PinStats,
}
```

**TOCTOU race в `unpin()`:** `remove_if` и последующий `get_mut` — два отдельных DashMap-операции, между которыми другой поток может изменить запись.

### Corruption Repair (corruption_repair.rs)

```rust
pub enum CorruptionType {
    BitFlip,
    Truncation,
    ZeroFill,       // 8+ consecutive zeros
    HeaderDamage,   // corruption в первых 16 байтах
    Unknown,
}
```

**Алгоритм:** cyclic XOR parity — `parity[i % 256] ^= data[i]`. Исправляет только одиночные ошибки в группе. `FetchedFromPeer` в `RepairAction` — dead code, не возвращается ни одним путём.

### Replication Manager (storage_replication_manager.rs)

```rust
pub enum ReplicationPolicy {
    Synchronous,           // fail-fast
    Asynchronous,          // queue, continue regardless
    BestEffort,            // no retries
    QuorumWrite(usize),    // требует N успехов
    PriorityFirst,         // стоп на первой ошибке
}
// Симулированные success rates: Put/Delete = 7/8, Sync = 3/4
// MAX_PENDING_OPS = 10_000, MAX_LOG_ENTRIES = 500
```

**Производительность:** `replication_log` использует `Vec::remove(0)` — O(n) shift вместо deque.

### Auto-Tuner (auto_tuner.rs)

**6 областей настройки:**
1. Cache — прокси через `success_rate * 0.7` (задокументировано как аппроксимация!)
2. Bloom filter — 2x размер при `read_write_ratio > 0.7`
3. Concurrency — 8-16 при `avg_latency > 10ms`
4. Compression — Zstd level 3 при `avg_block_size > 16KB`
5. Deduplication — 16KB chunks для WriteHeavy
6. Backend selection — Sled+WriteHeavy→ParityDB (2x throughput), ParityDB+ReadHeavy→Sled (1.5x reads)

---

## 4. Network Domain

**Crate:** `ipfrs-network` — 164 файла

### NetworkNode (node.rs)

```rust
// node.rs:288 — libp2p #[derive(NetworkBehaviour)]
pub struct IpfrsBehaviour {
    pub kademlia: kad::Behaviour<kad::store::MemoryStore>,
    pub identify: identify::Behaviour,
    pub ping: ping::Behaviour,
    pub autonat: autonat::Behaviour,
    pub dcutr: dcutr::Behaviour,
    pub mdns: mdns::tokio::Behaviour,
    pub relay_client: relay::client::Behaviour,
}

pub struct NetworkNode {
    pub gossipsub: Arc<GossipSubManager>,
    pub inference_waiters: InferenceWaiters,  // ML-специфично!
    pub active_relay_reservations: Arc<...>,
    connected_peers: Arc<DashSet<PeerId>>,
    swarm_cmd_tx: Option<mpsc::Sender<SwarmCommand>>,
    // ...
}
```

**Ключевой факт:** `NetworkNode` несёт `inference_waiters` — артефакт зависимости network layer от ML домена.

### GossipSub + ML-специфичные топики (gossipsub.rs:588-601)

```rust
// Стандартные IPFS топики:
pub const BLOCK_ANNOUNCE: &str = "/ipfrs/block/announce/1.0.0";
pub const PROVIDER_ANNOUNCE: &str = "/ipfrs/provider/announce/1.0.0";

// IPFRS-специфичные ML топики:
pub const INFERENCE_REQUEST: &str = "/ipfrs/inference/request/1.0.0";
pub const INFERENCE_RESULT: &str = "/ipfrs/inference/result/1.0.0";
pub const GRADIENT_SYNC: &str = "/ipfrs/gradient/sync/1.0.0";  // FedAvg
pub const KB_DELTA: &str = "/ipfrs/kb/delta/1.0.0";            // Knowledge Base sync
```

**Нетривиально:** сетевой слой содержит первоклассный метод `publish_gradient_cid()` для трансляции Arrow IPC gradient блоков — domain knowledge о ML training встроен в transport.

### GossipSubManager (gossipsub.rs:280)

```rust
pub struct GossipSubManager {
    config: GossipSubConfig,
    subscriptions: Arc<DashMap<TopicId, TopicSubscription>>,
    peer_scores: Arc<DashMap<PeerId, PeerScore>>,
    seen_messages: Arc<DashMap<MessageId, SeenCacheEntry>>,
    sequence_counter: Arc<RwLock<u64>>,
}
// mesh_n_low=4, mesh_n=6, mesh_n_high=12, heartbeat=1s
// max_message_size=1MB, dedup_cache=10000 entries, TTL=120s
```

**Проблема: `dummy_local_peer()`** — при публикации менеджер не хранит локальный `PeerId`, синтезирует его из zero-seed Ed25519 keypair при каждом вызове. Source identifier всегда одинаковый фиктивный peer ID.

### GossipProtocolEngine (gossip_protocol_engine.rs)

```rust
pub enum FanoutStrategy {
    Fixed(usize),
    Adaptive { min: usize, max: usize },  // min + (active_count/10)
    Epidemic(f64),                         // probability per peer
    Random(u64),                           // Fisher-Yates via xorshift64
    PriorityBased,                         // sort by fanout_score desc
}
// fanout_size=6, max_hops=7, dedup_cache=2048 (VecDeque — O(n) lookup!)
// EMA peer scoring: α=0.1, score = 0.9*score + 0.1*signal
```

### Три параллельных NAT traversal системы

1. **`nat_traversal.rs`** — простой STUN-like hole punching
2. **`nat_traversal_manager.rs`** — полный ICE (RFC 5245) с CandidateType, IcePair, PairState
3. **`IpfrsBehaviour::autonat + dcutr`** — libp2p встроенный AutoNAT + DCUtR

Экспортируются с prefix aliases (`Ntm*`) для разрешения коллизий.

### Kademlia Routing (routing_table_manager.rs)

```rust
pub struct NodeId(pub [u8; 32]);  // SHA-256 of public key

pub struct KBucket {
    entries: Vec<BucketEntry>,              // LRU ordered
    replacement_cache: VecDeque<BucketEntry>,
    max_size: usize,                        // DEFAULT_K=20
}
// NUM_BUCKETS=256, DEFAULT_ALPHA=3
// MAX_FAILED_QUERIES=3 — eviction threshold
// find_closest(): O(N) scan — практично до ~20K peers
```

### Stream Multiplexer (stream_multiplexer.rs)

```rust
pub struct MultiplexerConfig {
    pub max_streams: usize,         // default 256
    pub default_window_size: u32,  // default 65536
    pub max_frame_size: usize,     // default 16384
    pub idle_timeout_us: u64,      // default 30s
}
// Wire header: 25 bytes (stream_id:4 + seq:8 + payload_len:4 + flags:1 + timestamp:8)
// Priority: BinaryHeap<PrioritizedFrame> — Background=0, Low=1, Normal=2, High=4, Critical=8
```

### Overlay Network (overlay_network_manager/)

```rust
pub enum RoutingPolicy {
    ShortestPath,        // unit-weight Dijkstra
    MaxBandwidth,        // max-min bottleneck Dijkstra
    MaxReliability,      // min -ln(r) accumulation
    LoadBalanced,        // weight = load/capacity
    GeographicProximity, // 0.1 same-region, 1.0 cross-region
}
pub enum OverlayTopology {
    FullMesh, Ring, Star { center_id },
    Hypercube(u8), Custom,
}
```

Реализован **полный алгоритм Yen's** для k-кратчайших путей (не заглушка).

### Anti-Entropy (anti_entropy.rs)

```rust
pub struct MerkleDigest {
    entries: Vec<DigestEntry>,   // sorted by key
    root_hash: u64,              // XOR-fold(FNV1a(key) XOR value_hash XOR version)
}
// 3-way reconciliation: sent / requested / conflict
// max_diff_keys=100 per round — bounded message size
```

### Connection Management

```rust
// connection_manager.rs
pub struct ConnectionLimitsConfig {
    pub max_connections: usize,  // 256
    pub max_inbound: usize,      // 128
    pub max_outbound: usize,     // 128
    pub reserved_slots: usize,   // 8
    pub idle_timeout: Duration,  // 300s
}
// Low-memory preset: max=16, k=10, alpha=2
// IoT preset: max=32
// Mobile preset: max=64
```

---

## 5. Semantic Domain

**Crate:** `ipfrs-semantic` — 161 файл

### HNSW Vector Index (hnsw.rs)

```rust
pub struct VectorIndex {
    index: Arc<RwLock<Hnsw<'static, f32, DistL2>>>,  // hnsw_rs backing
    id_to_cid: Arc<RwLock<HashMap<usize, Cid>>>,
    cid_to_id: Arc<RwLock<HashMap<Cid, usize>>>,
    vectors: Arc<RwLock<HashMap<Cid, Vec<f32>>>>,     // оригиналы для snapshot
    tracker: Arc<RwLock<IncrementalTracker>>,
    dimension: usize,
    metric: DistanceMetric,
}
```

**Критические особенности:**
- `delete()` — только tombstone: удаляет CID mapping, вектор остаётся в HNSW графе
- `layer_connections` в snapshot **всегда пуст** — топология не сериализуется
- Восстановление из snapshot: O(n log n) полная переинсертация
- Cosine metric: `1.0 - (L2_dist² / 2)` на нормализованных векторах (корректно)

**Параметры настройки:**

| Use case | M | ef_construction | ef_search | Recall@10 |
|----------|---|-----------------|-----------|-----------|
| LowLatency (<10k) | 8 | 100 | 32 | 0.90 |
| Balanced (<10k) | 16 | 200 | 50 | 0.95 |
| HighRecall (<10k) | 32 | 400 | 200 | 0.99 |
| Balanced (>100k) | 32 | 400 | 150 | 0.93 |
| HighRecall (>100k) | 64 | 600 | 400 | 0.97 |

### DiskANN / Vamana (diskann.rs)

```rust
pub struct DiskANNConfig {
    pub max_degree: usize,    // R=64
    pub queue_size: usize,    // L=100
    pub alpha: f32,           // pruning ratio=1.2
}
// Файлы: *.dat (header+graph, oxicode) + *.dat.vectors (mmap, VectorFileHeader + raw f32)
// VectorFileHeader::SIZE = 24 bytes: magic(8) + num_vectors(8) + dimension(8)
```

**Производительная проблема:** `greedy_search_internal` использует `Vec::sort_by` после каждой вставки — O(n² log n) вместо BinaryHeap O(n log n). Критично при 100M векторах.

### Две реализации Product Quantization

1. **`vector_quantizer.rs`** — f64, stride-based k-means, Welford error tracking
2. **`quantization/product.rs`** — f32, nalgebra OPQ rotation, distance table precomputation

```rust
// Scalar INT8:
pub struct QuantizedVectorStore {
    data: Vec<i8>,           // flat layout
    scales: Vec<f32>,        // per-vector
    zero_points: Vec<f32>,   // per-vector
}
// 4× compression vs f32

// Binary:
pub struct BinaryVectorStore {
    data: Vec<u64>,    // ceil(dim/64) words per vector
}
// Hamming via popcount; ~32× compression
```

### SIMD Distance (simd.rs)

Runtime dispatch: AVX2 → AVX → SSE → scalar (x86_64); NEON → scalar (aarch64).

**Нетривиальный факт:** все три `*_avx2` функции немедленно делегируют к `*_avx` с комментарием. AVX2 = AVX для f32 операций — бранч существует, но не даёт прироста.

### Embedding Pipeline (embedding_pipeline.rs)

```rust
pub enum EmbeddingInput {
    RawBytes { data: Vec<u8>, mime_type: String },
    Text { content: String, language: Option<String> },  // ЗАГЛУШКА!
    Structured { fields: HashMap<String, String> },
    Embedding { vector: Vec<f32> },
}
```

**Важно:** `EmbeddingInput::Text` конвертирует chars в `char as f32 / 0x10FFFF` — это не embedding модель, это разработческая заглушка (embedding_pipeline.rs:265-271).

### Query Planning (query_planner.rs)

```rust
pub struct PlannerConfig {
    pub max_fanout: usize,          // 8
    pub latency_budget_ms: f64,    // 100.0
    pub min_vectors_per_shard: u64, // 100
    pub prefer_local: bool,         // true
}
// query_id = FNV-1a hash of query bytes
```

### LogicSolver — мост к TensorLogic (solver.rs)

```rust
pub struct SolverConfig {
    pub max_depth: usize,
    pub similarity_threshold: f32,  // 0.8
    pub top_k_similar: usize,
    pub embedding_dim: usize,
    pub detect_cycles: bool,
}
pub struct LogicSolver {
    config: SolverConfig,
    facts: Vec<(Predicate, Cid)>,
    rules: Vec<(Predicate, Vec<Predicate>)>,
    index: VectorIndex,  // семантический поиск внутри rule evaluation
}
```

---

## 6. TensorLogic Domain

**Crate:** `ipfrs-tensorlogic` — ~170 файлов. Самый объёмный домен.

### Фундаментальная IR (ir.rs)

```rust
// ir.rs:13-22
pub enum Term {
    Var(String),
    Const(Constant),
    Fun(String, Vec<Term>),
    Ref(TermRef),   // CID-addressed cross-reference — considered "ground"
}

pub enum Constant {
    String(String), Int(i64), Bool(bool),
    Float(String),  // ИНВАРИАНТ: хранится как String для детерминированного хеширования
}
// Float(String) решает: (1) обходит Eq/Hash запрет для f64, (2) портируемость CID через платформы

pub struct TermRef {
    #[serde(serialize_with = "crate::serialize_cid",
            deserialize_with = "crate::deserialize_cid")]
    pub cid: Cid,
    pub hint: Option<String>,
}

pub struct KnowledgeBase { pub facts: Vec<Predicate>, pub rules: Vec<Rule> }
```

**Правило Хорна:** `Rule { head: Predicate, body: Vec<Predicate> }`. `Rule::is_fact()` iff `body.is_empty()`.

### SLD-Resolution Backward Chaining (reasoning.rs)

```rust
pub struct InferenceEngine {
    max_depth: usize,       // default 100
    max_solutions: usize,   // default 100
    cycle_detection: bool,
}

pub struct CycleDetector {
    goal_stack: Vec<String>,    // для unwind
    goal_set: HashSet<String>,  // O(1) membership
}
```

**Критический факт (reasoning.rs ~goal_to_key):** ключ обнаружения цикла — только `"name(arity)"`. `parent(alice, X)` и `parent(bob, Y)` дают одинаковый ключ. Это conservative over-approximation — пропускает реальные циклы никогда, но может ложно остановить взаимную рекурсию на разных константах.

### Neural-Symbolic Fusion (neural_symbolic.rs)

```rust
pub struct NeuralSymbolicIntegrator {
    pub symbols: Vec<Symbol>,    // embedding: Vec<f64>, len=128
    pub rules: Vec<LogicalRule>,
    total_inferences: u64,       // мутирует → NOT Sync
}

pub enum RuleType {
    Definite,
    Probabilistic,           // confidence = weight * body_satisfaction²
    Soft { temperature: f64 }, // confidence = weight * sigmoid(body/T)
}
```

**Семантика "AND":** произведение confidences тела. 5-atom body с каждым по 0.9 = 0.59; по 0.7 = 0.17. `Probabilistic` правила квадратично консервативнее — нигде не задокументировано.

**Инициализация весов:**
- `AttentionMechanism::new()` инициализирует все проекции `1/sqrt(model_dim)` — детерминированно, но патологически (все головы идентичны до обучения).
- `dropout_rate` хранится, но никогда не применяется — поле для будущей совместимости API.

### Fuzzy Logic Engine (fuzzy_logic.rs)

```rust
pub enum InferenceMethod { Mamdani, Sugeno }
pub enum DefuzzMethod { Centroid, MeanOfMax, LargestOfMax }
pub const CENTROID_STEPS: usize = 100;  // числовое интегрирование

// Mamdani: minimum T-norm, clip MF at activation α, aggregate с max, defuzzify
// Sugeno: weighted centroid консеквентных множеств
```

### Epistemic Logic — S5 Kripke (epistemic_logic.rs)

```rust
pub enum EpistemicFormula {
    Atom(String), Not(Box<Self>), And(Box<Self>, Box<Self>), Or(Box<Self>, Box<Self>),
    Knows { agent: AgentId, phi: Box<Self> },
    Possible { agent: AgentId, phi: Box<Self> },
    EveryoneKnows(Box<Self>),
    CommonKnowledge(Box<Self>),
}

pub struct KripkeModel {
    pub worlds: Vec<PossibleWorld>,
    pub relations: Vec<AccessibilityRelation>,
    pub actual_world: WorldId,
}
```

`common_knowledge_worlds()`: итеративная fixed-point shrink. Проверяются аксиомы T, 4, B, 5 (S5).

### Probabilistic Logic Networks (probabilistic_logic_network.rs)

```rust
pub struct TruthValue { pub strength: f64, pub confidence: f64 }
// count() = c/(1-c) — effective sample count

pub struct ProbabilisticLogicNetwork {
    atoms: HashMap<String, PlnAtom>,
    links: HashMap<String, PlnLink>,
    adjacency: HashMap<String, Vec<OutEdge>>,  // только от ПЕРВОГО source_id!
    config: PlnConfig,  // max_atoms=100_000, inference_depth=6
}
```

**Ревизия:** `s_rev = (s1*n1 + s2*n2)/(n1+n2)`, `c_rev = n_total/(n_total+1)`

**Важно:** adjacency добавляется только от первого `source_id` — обратный поиск от B или C в link [A,B,C] невозможен через adjacency map.

### Bayesian Network Inference (bayesian_network_inference.rs)

```rust
pub enum InferenceAlgorithm {
    VariableElimination,
    BeliefPropagation,         // ФАКТИЧЕСКИ — это тоже VE!
    Sampling { n_samples: usize, seed: u64 },
}
// seed=0 → 0xDEAD_BEEF_CAFE
// max_variables=256, max_states=1024
```

**Критическое несоответствие:** `InferenceAlgorithm::BeliefPropagation` реализован как per-query Variable Elimination. Настоящий loopy BP не реализован. Вызывающий код, рассчитывающий на BP для цикличных графов, получит точный VE (который может завершиться ошибкой на плотных графах).

### Causal Inference (causal_inference.rs)

Pearl's do-calculus над Gaussian linear SCM:
```rust
pub struct CausalEdge {
    pub strength: f64,      // может быть отрицательным (inhibitory)
    pub edge_type: CausalEdgeType,  // Direct, Confounded, Backdoor, Instrumental
}
```

**Variance cap:** `(1 - total_explained_variance.min(0.99))` — 1% floor предотвращает нулевую вариацию при полном объяснении. Детерминированные уравнения никогда не дадут нулевой вариации.

### Temporal Knowledge Graph (temporal_knowledge_graph.rs)

```rust
pub struct TemporalKnowledgeGraph {
    nodes: HashMap<NodeId, TkgNode>,
    edges: HashMap<EdgeId, TkgEdge>,
    timeline: BTreeMap<u64, Vec<TkgEvent>>,  // append-only event log
    rng_state: u64,  // xorshift64, seed=0xcafe_babe_dead_beef
}
// NodeId = [u8; 16], генерируется через xorshift64 + FNV-1a
```

**PropertyAt query:** O(total events) — линейный скан всего timeline. Нет вторичного индекса по (node_id, key).

**Распределённая проблема:** два свежесозданных TemporalKnowledgeGraph стартуют с одинаковым RNG seed — коллизии ID при merge.

### Autograd (autograd.rs)

```rust
pub enum AutogradOp {
    Input, Add{lhs,rhs}, Mul{lhs,rhs}, Neg{input},
    Exp{input}, Ln{input}, Pow{base,exponent:f64},  // scalar exponent only
}
pub struct AutogradGraph {
    nodes: HashMap<NodeId, AutogradNode>,
    next_id: NodeId,
}
// backward(): iterative topo sort с HashMap<NodeId,bool>
// scalar f64 only — без батчинга, без tensor ops
```

### Inference Scheduler (inference_scheduler.rs)

```rust
pub struct SchedulerConfig {
    pub max_concurrent: usize,    // default 4
    pub max_queue_size: usize,    // default 256
}
// tick(current_tick): expire → sort by (priority DESC, job_id ASC) → promote
// submit() → None при pending_count >= max_queue_size (back-pressure)
```

### Knowledge Base Federation (kb_federation.rs)

```rust
// merge_knowledge_bases(): FNV-1a deduplication
// export_kb_as_cid(): каждое правило → отдельный IPLD block
// import_remote_kb(): fetch root + rule blocks → merge

// КРИТИЧНО (line ~308): local_fact_hashes пересчитывается в ЦИКЛЕ →
// O(local_facts × remote_facts) вместо O(local_facts + remote_facts)
```

**Обнаружение конфликтов:** использует `rule_body_repr = "name1(arity1),name2(arity2),..."` — только predicates+arities, не значения аргументов. Структурно разные правила с теми же функторами тихо мержатся без конфликта.

---

## 7. Transport Domain

**Crate:** `ipfrs-transport` (файлы не прочитаны напрямую, описание из ipfrs crate зависимостей)

Упоминается в `node/tensorlogic_ops.rs`:
```rust
use ipfrs_transport::tensorswap::GradientStreamSession;
```

Также упоминается `TensorSwap` protocol для streaming градиентов в chunks.

**Протоколы (из context):**
- Bitswap — стандартный IPFS block exchange
- GraphSync — DAG traversal protocol
- QUIC, TCP, WebSocket, WebTransport транспорты
- `TensorSwap` — кастомный IPFRS protocol для gradient streaming

---

## 8. Interface / Gateway Domain

**Crate:** `ipfrs-interface` — 28 файлов

### gRPC Services (grpc.rs)

```rust
// Только BlockServiceImpl реально работает со storage!
pub struct BlockServiceImpl<S> { storage: Arc<S> }
// DagServiceImpl, FileServiceImpl, TensorServiceImpl — ЗАГЛУШКИ с Arc<()>
// stream_blocks → возвращает mock data vec![1,2,3,4]

// gRPC validation limits:
// MAX_BLOCK_SIZE = 256 MB (grpc.rs:72)
// MAX_BATCH_SIZE = 1000 (grpc.rs:75)
// MAX_PATH_LENGTH = 4096 chars (grpc.rs:79)
// MAX_TENSOR_DIMS = 8 (grpc.rs:167)
// Validation: CID должен начинаться с "Qm", "bafy", "bafk", или "bafz"
```

**Interceptor chain** (order: metrics → logging → auth):
```rust
pub struct ChainedInterceptor {
    auth: Option<AuthInterceptor>,  // JwtManager
    logging,                         // Instant в request extensions
    metrics,                         // AtomicU64 request/error count
}
```

### GradientSyncService (grpc.rs:1384-1494)

```rust
// ВАЖНО: это loopback stub!
// "In a live deployment peer CIDs would be discovered via the network layer..."
// min_peers и timeout_secs — принимаются но игнорируются
// Chunk size: 65_536 bytes
// Вызывает: load_gradient_from_arrow, store_gradient_as_arrow
//            DistributedGradientAccumulator::commit_local
```

### Auth (auth.rs)

```rust
pub enum Role { Admin, User, ReadOnly }
pub struct JwtManager { encoding_key, decoding_key, validation }  // HS256
// ApiKey format: "ipfrs_" + hex::encode(32 random bytes)
// ApiKey prefix = первые 12 chars raw key — для быстрого отсева при bcrypt
```

**КРИТИЧЕСКИЙ ИЗЪЯН (auth.rs:449-453):**
```rust
// JWT подписывается MD5 !!
let signature = format!("{:x}", md5::compute(format!("{}{}", payload, self.jwt_secret)));
// Комментарий: "Simplified JWT encoding — in production, use jsonwebtoken crate"
// Default secret: "default_secret_change_in_production"
```

### OAuth2 Server (oauth2.rs)

```rust
pub enum GrantType { AuthorizationCode, ClientCredentials, RefreshToken, Implicit }
pub struct OAuth2Server {
    clients: DashMap<...>, authorization_codes: DashMap<...>,
    access_tokens: DashMap<...>, refresh_tokens: DashMap<...>,
    // Всё in-memory! Нет персистентности.
}
// TTLs: access=1h, refresh=30d, code=10min
// client_secret == строковое сравнение (НЕ constant-time!) — timing attack
```

### WebSocket (websocket.rs)

```rust
pub enum RealtimeEvent {
    BlockAdded{cid,size,timestamp}, BlockRemoved,
    PeerConnected, PeerDisconnected,
    DhtQueryStarted/Progress/Completed,
}
// Channel capacity: 100 per topic
// Polling loop: try_recv all topics + sleep(10ms) — НЕ push!
// Задержка события до 10ms; 10k connections × 1 sub = 10k polling tasks
```

### Streaming & Flow Control (streaming.rs)

```rust
pub struct FlowController {
    // AIMD: +10% каждые 100ms, -50% при on_congestion()
    initial_window: 256KB, max_window: 1MB, min_window: 64KB,
}
// ResumeToken: base64(JSON{operation_id, offset, cid}) — stateless, client-owned
// batch_put_atomic: two-phase (validate all → store all → rollback on failure)
```

### Backpressure (backpressure.rs)

```rust
pub struct BackpressureController {
    semaphore: Arc<Semaphore>,
    window_size: Arc<AtomicUsize>,
    // ...
}
// КРИТИЧНО: decrease_window обновляет только AtomicUsize!
// Semaphore permits НЕ отзываются при уменьшении окна.
// Backpressure фактически не работает после decrease.
```

### Binary Protocol (binary_protocol.rs)

```
MAGIC(4)="IPFS" | VERSION(1) | MSG_TYPE(1) | MSG_ID(4) | PAYLOAD
// 10-byte fixed header
// MAX_MESSAGE_SIZE = 16 MB
```

### FFI Layer (ffi.rs)

```rust
// C-ABI exports: ipfrs_client_new, free, add, get, has, get_last_error, string_free, data_free, version
// Все функции: catch_unwind(AssertUnwindSafe(...))
// LATENT BUG: ipfrs_get_last_error возвращает raw pointer на String в RefCell
// Указатель инвалидируется при следующем FFI вызове на этом треде
```

### Tensor Serving (tensor.rs, arrow.rs)

```rust
// N-dim slicing: row-major strides, Cartesian product iteration, element-by-element copy
// "Zero-copy" Arrow path = BlockStore read + data.to_vec() + Buffer::from copy + StreamWriter
// Реально 3 копии — комментарий "zero-copy" некорректен
```

---

## 9. Application Layer — ipfrs Node

### Node Aggregate (node/mod.rs)

```rust
pub(super) type NodeStore = CachedBlockStore<SledBlockStore>;

pub struct Node {
    pub(super) config: NodeConfig,
    pub(super) network: Option<NetworkNode>,
    pub(super) storage: Option<Arc<NodeStore>>,
    pub(super) semantic: OnceCell<Arc<SemanticRouter>>,      // lazy init
    pub(super) tensorlogic: OnceCell<Arc<TensorLogicStore<NodeStore>>>, // lazy
    pub(super) auth_manager: Option<Arc<AuthManager>>,
    pub(super) tls_manager: Option<Arc<TlsManager>>,
    pub(super) pin_manager: Arc<PinManager>,
    pub(super) startup_time: Option<SystemTime>,
    pub metrics: Arc<IpfrsMetrics>,
}

pub struct NodeConfig {
    pub network: NetworkConfig,
    pub storage: BlockStoreConfig,
    pub semantic: RouterConfig,
    pub enable_semantic: bool,       // default: true
    pub enable_tensorlogic: bool,    // default: true
    pub auth_jwt_secret: Option<String>,
    pub tls: Option<TlsConfig>,
    pub fetch_timeout_secs: Option<u64>,  // None = 30s
}
```

### Операции разделены по 9 файлам

| Файл | Операции |
|------|----------|
| `core.rs` | start, stop, status, config |
| `block_ops.rs` | add, get, has, delete; `get(&mut self)` — network fetch |
| `dag_ops.rs` | import_car, export_car, dag_stat, dag_resolve |
| `network_ops.rs` | connect, disconnect, find_peers, provide |
| `pin_ops.rs` | pin, unpin, list_pins |
| `repo_ops.rs` | repo_stat, fsck, gc |
| `semantic_ops.rs` | semantic_add, semantic_search, semantic_build_index |
| `tensorlogic_ops.rs` | infer, infer_streaming, prove_distributed, accumulate_gradients |
| `auth_ops.rs` | create_user, authenticate, create_api_key |

**Несимметрия:** `get()` принимает `&mut self` (network fetch мутирует), `get_block()` принимает `&self` (только local). Вызывающий код должен держать мутабельную ссылку на весь Node для сетевого получения.

### Garbage Collector (gc.rs)

```rust
pub struct GcConfig {
    pub min_age_seconds: u64,      // default 3600
    pub max_blocks_per_run: u64,   // default 0 = unlimited
    pub dry_run: bool,
}
```

**КРИТИЧНО:** `min_age_seconds` задокументировано, принимается CLI, но **не применяется в `collect()`**. Все недостижимые блоки удаляются независимо от возраста.

### Distributed Inference (tensorlogic_ops.rs)

1. Локальный fast-path через `infer()`
2. При отсутствии результатов: publish `InferenceRequest` в GossipSub
3. Ждать ответов через `register_inference_waiter()` до deadline
4. Агрегировать `RemoteResult` bindings

**`accumulate_gradients()` broken (tensorlogic_ops.rs:1131-1136):** busy-polls `acc.is_ready(min_peers)` через `sleep(50ms)`, но нет механизма добавления peer градиентов в `acc` внутри этого метода. При `min_peers > 0` — всегда timeout.

**`prove_distributed()` (tensorlogic_ops.rs:1024-1031):** `find_providers` и `remote_query` callbacks возвращают пустые векторы. DHT peer queries — заглушка.

### PinManager (pin.rs)

```rust
pub struct PinManager {
    pins: Arc<RwLock<HashMap<Cid, PinInfo>>>,
    indirect_pins: Arc<RwLock<HashSet<Cid>>>,
    recursive_deps: Arc<RwLock<HashMap<Cid, HashSet<Cid>>>>,
}
// pin_recursive(): iterative DFS (не рекурсивный — нет stack overflow)
// unpin(): decrements ref_count, removes когда == 0
// Persistence: oxicode CBOR serialization
```

### TLS Manager (tls.rs)

```rust
pub struct TlsConfig {
    pub min_version: TlsVersion,              // default: Tls13
    pub reload_interval_secs: Option<u64>,    // default: Some(3600)
}
// SelfSignedCertGenerator::generate() → ЗАГЛУШКА
// Возвращает PEM с человекочитаемыми комментариями, не реальный X.509
// В production использовать crate rcgen
```

---

## 10. CLI — ipfrs-cli

**Version:** `0.3.0 "The Fast & The Wise"`

### Exit Codes

| Код | Константа | Значение |
|-----|-----------|----------|
| 0 | SUCCESS | ок |
| 1 | ERROR | общая ошибка |
| 2 | USAGE_ERROR | неверный синтаксис |
| 3 | NOT_FOUND | ресурс не найден |
| 4 | PERMISSION_DENIED | |
| 5 | NETWORK_ERROR | |
| 6 | IO_ERROR | |
| 7 | TIMEOUT | |
| 8 | CONFIG_ERROR | |

### Команды → Domain mapping

| CLI команда | Domain | Ключевой тип |
|-------------|--------|--------------|
| `block add/get/has/del/stat` | Storage | `Node::add()`, `Node::get()` |
| `dag import/export/stat/resolve` | Storage/Core | `Node::import_car()`, `dag_resolve()` |
| `pin add/rm/ls` | Application | `PinManager` |
| `repo stat/gc/fsck` | Application | `GarbageCollector`, `FsckResult` |
| `semantic add/search/index` | Semantic | `SemanticRouter` |
| `logic infer/prove/add-fact` | TensorLogic | `InferenceEngine`, `KnowledgeBase` |
| `tensor add/get/slice` | Core+Interface | `TensorBlock`, `TensorSlice` |
| `gradient sync/push/pull` | TensorLogic+Network | `GradientSyncService` |
| `network connect/peers/dht` | Network | `NetworkNode` |
| `model add/list/infer` | TensorLogic | `TensorLogicStore` |
| `query semantic/logic` | Semantic+TensorLogic | `LogicSolver` |
| `gateway start/stop` | Interface | `Gateway` |
| `daemon start/stop/status` | Application | `Node` lifecycle |
| `diag health/metrics` | Cross-cutting | `IpfrsMetrics` |

---

## 11. Cross-Cutting Concerns

### Error Handling Philosophy

```rust
// ipfrs-core/error.rs — единый Error тип для всей системы
// Конвертации: #[from] std::io::Error
// Категории: is_recoverable(), is_io(), is_not_found()
// Все crates используют ipfrs_core::Result<T> = Result<T, ipfrs_core::Error>
```

**Паттерн:** `thiserror` для domain-specific ошибок, конверсия в `ipfrs_core::Error` на границе.

### Observability

**Prometheus метрики** (`ipfrs-interface/metrics.rs`):
- Request counts per service
- Latency histograms
- Block store operations
- Network peer counts

**Structured tracing:**
- `tracing` crate везде
- `tracing_setup.rs` в ipfrs crate

**Логирование:** `xorshift64` + `FNV-1a` паттерн в 10+ модулях — каждый модуль реализует свой PRNG inline, всегда один и тот же алгоритм, разные seed-константы.

### Security

**Проблемные места (по степени серьёзности):**

1. **КРИТИЧНО:** JWT подписывается MD5 (`auth.rs:449`). Не использовать в production.
2. **ВЫСОКОЕ:** OAuth2 client_secret сравнивается без constant-time (`oauth2.rs:109`).
3. **СРЕДНЕЕ:** `ApiKeyStore` lookup — O(n) linear scan (`auth.rs:517-534`).
4. **СРЕДНЕЕ:** `ipfrs_get_last_error` возвращает dangling pointer при следующем FFI вызове (`ffi.rs:413-419`).
5. **СРЕДНЕЕ:** `TieredStore::delete` тихо no-op если блок не найден.
6. **НИЗКОЕ:** `SelfSignedCertGenerator` — заглушка, не реальный X.509.
7. **НИЗКОЕ:** xorshift64 с hardcoded seed — предсказуемые ID при распределённом создании TKG.

### Concurrency Model

- **Tokio async runtime** — основа всей async работы
- **parking_lot::RwLock** предпочитается std::sync
- **DashMap** для hot concurrent maps
- **Arc<Semaphore>** для backpressure (но decrease не работает — см. секцию 8)
- **OnceCell** для lazy initialization semantic/tensorlogic в Node

### xorshift64 Pattern

Используется в 10+ модулях как inline PRNG без внешних зависимостей:
```rust
// Паттерн везде:
fn xorshift64(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}
```

Seed-константы: `0xcafe_babe_dead_beef` (TKG), `0xDEAD_BEEF_CAFE` (BNI sampling), `0xdeadbeef_cafebabe XOR fanout_size` (gossip).

---

## 12. Data Flows

### Flow 1: Add File → Searchable

```
User: ipfrs add <file>
  │
  ├─ Chunker::chunk_file() → Vec<Block>
  │    ├─ FixedSize(256KiB) или ContentDefined(Rabin)
  │    └─ Build Merkle DAG, MAX_LINKS_PER_NODE=174
  │
  ├─ Node::add_blocks() → SledBlockStore::put_many()
  │    └─ WAL: WalWriteAheadLog::append(Put)
  │
  ├─ Network: NetworkNode::provide(root_cid)
  │    └─ Kademlia::start_providing(cid)
  │
  ├─ Semantic (optional): SemanticRouter::insert(cid, embedding)
  │    └─ VectorIndex::insert(vec, cid) → HNSW graph update
  │
  └─ GossipSub: publish BLOCK_ANNOUNCE
```

### Flow 2: Get File (с сетью)

```
Node::get(&mut self, cid)
  │
  ├─ storage.get(cid) → Some(block)?  ──► return block
  │
  └─ None → network path:
       ├─ NetworkNode::get_providers(cid) [timeout=30s]
       │    └─ DHT Kademlia lookup → Vec<PeerId>
       │
       ├─ Bitswap/TensorSwap request to providers
       │
       └─ On receive: storage.put(block) → return block
```

### Flow 3: Semantic Search

```
CLI: ipfrs semantic search "query text"
  │
  ├─ EmbeddingInput::Text{ content } (заглушка в dev, реальный model в prod)
  │
  ├─ VectorIndex::search(vec, k=10, ef_search=50)
  │    └─ HNSW greedy search → Vec<(Cid, f32 score)>
  │
  ├─ ReRanker::rerank(results, strategy=RRF{k=60})
  │    └─ ScoreComponents: VectorSimilarity, Recency, Popularity
  │
  └─ return Vec<SearchResult{cid, score}>
```

### Flow 4: Logic Query — Distributed

```
CLI: ipfrs logic infer "ancestor(X, Y)"
  │
  ├─ Local fast-path:
  │    InferenceEngine::solve(&kb, goal)
  │    ├─ SLD-resolution backward chaining
  │    ├─ cycle detect via "name(arity)" key
  │    └─ max_depth=100, max_solutions=100
  │
  ├─ If no results + network available:
  │    GossipSubManager::publish(INFERENCE_REQUEST, json)
  │    │
  │    └─ Peers receive → run local inference → publish INFERENCE_RESULT
  │         │
  │         └─ Node::inference_waiters receives → aggregate RemoteResult
  │
  └─ Merge local + remote bindings → DistributedInferResult
```

### Flow 5: gRPC Call → Storage

```
Client: gRPC BlockService.GetBlock(cid)
  │
  ├─ ChainedInterceptor: metrics → logging → AuthInterceptor(JWT)
  │    └─ validate: starts_with("Qm"|"bafy"|"bafk"|"bafz")
  │
  ├─ BlockServiceImpl<SledBlockStore>::get_block()
  │    └─ storage.get(cid)
  │
  └─ return Block{cid, data} or NOT_FOUND
```

### Flow 6: Gradient Sync (Federated Learning)

```
Node::accumulate_gradients(cid, min_peers, timeout)
  │
  ├─ Encode local gradient → Arrow IPC bytes
  ├─ storage.put(gradient_block) → gradient_cid
  │
  ├─ GossipSub::publish(GRADIENT_SYNC, gradient_cid)
  │
  ├─ Wait DistributedGradientAccumulator::is_ready(min_peers)
  │    └─ BROKEN: нет механизма добавления peer-градиентов → всегда timeout
  │
  └─ FedAvg on collected → aggregated gradient block
```

---

## 13. Invariants & Guarantees

### Per-Domain Invariants

**ipfrs-core:**
- `Block::cid = hash(data)` — детерминированная идентичность
- `TensorBlock`: `data.len() == shape.element_count() * dtype.size_bytes()`
- `Constant::Float(String)`: IEEE 754 identity != content-hash identity — `1.0` и `1.0000000000001` дают разные CIDs

**Storage:**
- WAL entry: `checksum = FNV-1a(serialized_op)` — corruption detection
- Tier promotion: All новые блоки → Hot
- GC: pinned блоки иммунны ко всем retention actions
- Chunk ID encoding: `chunk_id = object_id << 32 | chunk_index` — предел 4 млрд объектов и 4 млрд чанков на объект
- WAL (`write_ahead_log.rs`): partial transactions (Begin без Commit) — discarded при recovery

**Network:**
- GossipSub: подписка должна существовать перед publish (TopicNotFound)
- DHT: `MAX_FAILED_QUERIES = 3` перед eviction из K-Bucket
- Stream mux: flow control window не может превысить `default_window_size` (65536)
- Кольцевой dedup в gossip engine — мягкий: вытесненные старые сообщения повторно принимаются

**Semantic:**
- VectorIndex: dimension consistency enforced при insert
- HNSW delete: tombstone-only, вектор остаётся в графе
- Snapshot: topology loss — восстановление меняет топологию

**TensorLogic:**
- `Term::Ref` считается ground (CID = opaque constant)
- `Rule::is_fact()` iff `body.is_empty()`
- KnowledgeBase: facts + rules — единственный авторитет, модифицируется только через `add_fact`/`add_rule`
- PLN TruthValue: strength, confidence ∈ [0, 1]
- Neural-Symbolic: body satisfaction = product (не min, не Łukasiewicz)

### System-Level Guarantees

| Guarantee | Status |
|-----------|--------|
| Content-addressing | Работает (SHA256 по умолчанию) |
| Block dedup | Работает (CID-based) |
| Distributed inference | Частично (remote callbacks — заглушки) |
| TLS security | НЕ работает (SelfSigned — заглушка) |
| JWT security | НЕБЕЗОПАСНО (MD5 вместо HS256) |
| OAuth2 PKCE | Работает (S256 SHA-256 корректно) |
| Federated learning | НЕ работает (accumulate_gradients broken) |
| GC min_age | НЕ применяется |
| Backpressure decrease | НЕ работает (semaphore не уменьшается) |

---

## 14. Performance Architecture

### Scale Targets (задекларированы)

| Метрика | Цель |
|---------|------|
| Vectors (semantic) | 100M+ (DiskANN) |
| Query latency | < 1ms (cached), < 5ms (uncached) |
| Index build | < 10min / 1M vectors |
| Memory | < 2GB / 1M × 768-dim vectors (реально ~3.2GB при M=16) |
| Recall@10 | > 95% |
| gRPC simple GET | < 10ms |
| HTTP throughput | > 1 GB/s (range requests) |
| Concurrent connections | 10,000+ |
| Memory per connection | < 100KB |

### Настроечные параметры по доменам

**Storage:**
```
WAL max segment: 64 MiB          WAL checkpoint: каждые 10,000 записей
Chunk size: 1 MiB (default)      GC batch: 1024 blocks
Tier cleanup: каждые 300s        Decay factor: 0.9 per cycle
Replication pending: 10,000 ops  Parity block size: 256 bytes
```

**Semantic:**
```
HNSW M: 8–64                     ef_construction: 100–600
ef_search: 32–400                PQ subspaces: 8 (default)
LSH hash functions: 8            LSH tables: 4
Semantic DHT dim: 384 (default)  Cache TTL: 300s
```

**TensorLogic:**
```
SLD max_depth: 100               max_solutions: 100
PLN max_atoms: 100,000           inference_depth: 6
BNI max_variables: 256           Scheduler max_concurrent: 4
NS embedding_dim: 128            NS max_symbols: 10,000
Fuzzy centroid steps: 100
```

**Network:**
```
Gossip fanout: 6                 max_hops: 7
K-Bucket size: 20                Alpha: 3
Mesh D/D_low/D_high: 6/4/12     Stream max: 256
Anti-entropy interval: 30s       max_diff_keys: 100
```

### Критические производительные проблемы

| Проблема | Местоположение | Severity |
|----------|----------------|----------|
| DiskANN greedy search: Vec::sort_by вместо BinaryHeap | diskann.rs:783-823 | HIGH |
| HNSW: topology loss при snapshot, O(n log n) restore | persistence.rs | HIGH |
| TKG PropertyAt: O(total events) без индекса | temporal_knowledge_graph.rs | MEDIUM |
| KB federation import: O(n×m) hash rebuild в цикле | kb_federation.rs:308 | MEDIUM |
| GC stats: O(V+E) BFS при каждом запросе | block_garbage_collector.rs | MEDIUM |
| Replication log: Vec::remove(0) O(n) | storage_replication_manager.rs | LOW |
| ApiKey lookup: O(n) scan всех ключей | auth.rs:517-534 | LOW |
| Rabin chunker: нет precomputed table | chunking.rs | LOW |
| Attention mechanism: чистый Rust Vec<f64>, нет BLAS | attention_mechanism.rs | LOW |
| Autograd: scalar only, нет батчинга | autograd.rs | LOW |

### Release Profile

```toml
[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
strip = true
```

---

## 15. DDD Pattern Inventory

### Aggregates

| Aggregate | Crate | Файл | Инварианты |
|-----------|-------|------|------------|
| `Block` | ipfrs-core | block.rs:58 | CID=hash(data); иммутабельный |
| `TensorBlock` | ipfrs-core | tensor.rs:193 | data.len()==shape×dtype |
| `Node` | ipfrs | node/mod.rs | lifecycle; DI root; 9 impl-блоков |
| `KnowledgeBase` | ipfrs-tensorlogic | ir.rs:277 | единственный авторитет над facts+rules |
| `BayesianNetwork` | ipfrs-tensorlogic | bni.rs | CPTs+variables+cardinality в sync |
| `CausalGraph` | ipfrs-tensorlogic | causal_inference.rs | DAG CausalNodes+edges |
| `KripkeModel` | ipfrs-tensorlogic | epistemic_logic.rs | worlds+relations+actual_world |
| `TemporalKnowledgeGraph` | ipfrs-tensorlogic | temporal_knowledge_graph.rs | append-only event log |
| `PLN AtomSpace` | ipfrs-tensorlogic | pln.rs | atoms+links+adjacency+config |
| `NeuralSymbolicIntegrator` | ipfrs-tensorlogic | neural_symbolic.rs | &mut self — not Sync |
| `DistributedOptimizer` | ipfrs-tensorlogic | distributed_optimizer.rs | workers+pending gradients |
| `TensorInferenceScheduler` | ipfrs-tensorlogic | inference_scheduler.rs | tick-driven state machine |
| `VectorIndex` | ipfrs-semantic | hnsw.rs:84 | dimension consistency; CID bijection |
| `DiskANNIndex` | ipfrs-semantic | diskann.rs:186 | mmap handles; graph+CID mapping |
| `GossipSubManager` | ipfrs-network | gossipsub.rs:280 | подписка до publish |
| `RoutingTableManager` | ipfrs-network | routing_table_manager.rs | 256 K-Buckets |
| `OverlayNetworkManager` | ipfrs-network | overlay_network_manager | Routing policies |
| `GossipAntiEntropy` | ipfrs-network | anti_entropy.rs | local state map |
| `OAuth2Server` | ipfrs-interface | oauth2.rs:297 | in-memory DashMaps |
| `PinManager` | ipfrs | pin.rs | ref_count > 0 → GC immunity |

### Value Objects

| Value Object | Crate | Тип |
|--------------|-------|-----|
| `Cid` | ipfrs-core | re-export `::cid::Cid` |
| `HashAlgorithm` | ipfrs-core | cid.rs:13 enum |
| `Term` | ipfrs-tensorlogic | ir.rs:13 enum; Eq+Hash через String |
| `Constant` | ipfrs-tensorlogic | ir.rs:25 enum; Float как String |
| `Predicate` | ipfrs-tensorlogic | ir.rs:164 struct |
| `TruthValue` | ipfrs-tensorlogic | pln.rs struct |
| `SearchResult` | ipfrs-semantic | hnsw.rs:26 `{cid, score: f32}` |
| `QueryPlan` | ipfrs-semantic | query_planner.rs:37 |
| `IndexSnapshot` | ipfrs-semantic | persistence.rs |
| `MerkleDigest` | ipfrs-network | anti_entropy.rs |
| `PathQuality` | ipfrs-network | multipath_quic.rs |
| `VirtualRoute` | ipfrs-network | overlay_network_manager/types.rs |
| `TkgSnapshot` | ipfrs-tensorlogic | temporal_knowledge_graph.rs |
| `KbMergeDiff` | ipfrs-tensorlogic | kb_federation.rs |
| `GradientUpdate` | ipfrs-tensorlogic | distributed_optimizer.rs |
| `ResumeToken` | ipfrs-interface | streaming.rs:313 |
| `TuningRecommendation` | ipfrs-storage | auto_tuner.rs |
| `WalEntry` | ipfrs-storage | write_ahead_log.rs |
| `RetentionRule` | ipfrs-storage | retention_policy.rs |

### Domain Services

| Service | Crate | Назначение |
|---------|-------|-----------|
| `InferenceEngine` | ipfrs-tensorlogic | reasoning.rs — SLD-resolution |
| `MemoizedInferenceEngine` | ipfrs-tensorlogic | reasoning.rs — caching wrapper |
| `DistributedReasoner` | ipfrs-tensorlogic | reasoning.rs — distributed decomp |
| `FuzzyLogicEngine` | ipfrs-tensorlogic | fuzzy_logic.rs |
| `CausalInferenceEngine` | ipfrs-tensorlogic | causal_inference.rs |
| `KnowledgeGraphTraverser` | ipfrs-tensorlogic | kg_traversal.rs |
| `merge_knowledge_bases()` | ipfrs-tensorlogic | kb_federation.rs — stateless |
| `export_kb_as_cid()` | ipfrs-tensorlogic | kb_federation.rs — async IPLD I/O |
| `GarbageCollector<S>` | ipfrs | gc.rs — mark-sweep GC |
| `RepoAnalyzer<S>` | ipfrs | repo.rs — read-only analytics |
| `Chunker` | ipfrs-core | chunking.rs — file→DAG |
| `HashRegistry` | ipfrs-core | hash.rs — algorithm dispatch |
| `CodecRegistry` | ipfrs-core | codec_registry.rs |
| `AccessTracker` | ipfrs-storage | tiering.rs — tier classification |
| `AutoTuner` | ipfrs-storage | auto_tuner.rs — recommendations |
| `DataIntegrityChecker` | ipfrs-storage | data_integrity_checker.rs |
| `CorruptionRepairer` | ipfrs-storage | corruption_repair.rs |
| `NatTraversalManager` | ipfrs-network | nat_traversal.rs |
| `GossipProtocolEngine` | ipfrs-network | gossip_protocol_engine.rs |
| `ParameterTuner` | ipfrs-semantic | hnsw.rs:1109 — HNSW config |
| `NearestNeighborQueryPlanner` | ipfrs-semantic | query_planner.rs |
| `ReRanker` | ipfrs-semantic | reranking.rs |
| `JwtManager` | ipfrs-interface | auth.rs:268 |
| `FlowController` | ipfrs-interface | streaming.rs:175 |
| `BackpressureController` | ipfrs-interface | backpressure.rs:46 |

### Repositories

| Repository | Crate | Backend |
|------------|-------|---------|
| `BlockStore` trait | ipfrs-storage | traits.rs — primary persistence port |
| `SledBlockStore` | ipfrs-storage | Sled embedded B-tree |
| `CachedBlockStore<S>` | ipfrs-storage | LRU wrapper over S |
| `TieredStore<H,C>` | ipfrs-storage | two-store composite |
| `MmapStore` | ipfrs-storage | memory-mapped files (feature) |
| `S3Store` | ipfrs-storage | S3-compatible (feature) |
| `ParityDB` | ipfrs-storage | parity-db (feature) |
| `TensorLogicStore<S>` | ipfrs-tensorlogic | storage.rs — KB over BlockStore |
| `VectorIndex` | ipfrs-semantic | hnsw.rs — HNSW index |
| `DiskANNIndex` | ipfrs-semantic | diskann.rs — disk-based |
| `UserStore` | ipfrs-interface | auth.rs:320 — DashMap in-memory |
| `ApiKeyStore` | ipfrs-interface | auth.rs:479 — DashMap in-memory |
| `OAuth2Server` stores | ipfrs-interface | oauth2.rs:297 — DashMap in-memory |
| `PinManager` | ipfrs | pin.rs — HashMap+oxicode persistence |
| `CacheManager` | ipfrs-tensorlogic | Arc<LRU> over inference results |

### Domain Events

| Event | Crate | Файл | Когда |
|-------|-------|------|-------|
| `TkgEvent::NodeAdded` | ipfrs-tensorlogic | temporal_knowledge_graph.rs | add_node() |
| `TkgEvent::NodeDeleted` | ipfrs-tensorlogic | temporal_knowledge_graph.rs | delete_node() |
| `TkgEvent::EdgeAdded/Deleted` | ipfrs-tensorlogic | temporal_knowledge_graph.rs | add/delete_edge() |
| `TkgEvent::PropertyChanged` | ipfrs-tensorlogic | temporal_knowledge_graph.rs | set_property() |
| `NetworkEvent::PeerConnected` | ipfrs-network | node.rs | libp2p swarm event |
| `NetworkEvent::ContentFound` | ipfrs-network | node.rs | DHT provider response |
| `NetworkEvent::DhtBootstrapCompleted` | ipfrs-network | node.rs | |
| `MuxEvent::StreamOpened/Closed` | ipfrs-network | stream_multiplexer.rs | |
| `GossipEvent::MessageReceived` | ipfrs-network | gossip_protocol_engine.rs | |
| `RealtimeEvent::BlockAdded` | ipfrs-interface | websocket.rs:58 | broadcast |
| `ProgressEvent` | ipfrs-interface | streaming.rs:377 | upload/download |
| `DriftSignal` | ipfrs-semantic | drift_monitor.rs | concept drift detected |
| `ScalingAction` | ipfrs-semantic | auto_scaling.rs | scaling recommendation |

### Anti-Corruption Layers

| ACL | Crate | Расположение | Назначение |
|-----|-------|--------------|-----------|
| `SerializableCid` | ipfrs-core | cid.rs | Newtype для serde контроля над external Cid |
| `ipfrs_core::Ipld` | ipfrs-core | ipld.rs | Собственный IPLD — изоляция от libipld экосистемы |
| `serialize_cid`/`deserialize_cid` | ipfrs-tensorlogic | ir.rs:TermRef | Изоляция бинарного формата ipfrs_core::Cid от IR serde |
| `ipld_codec.rs` | ipfrs-tensorlogic | kb_federation.rs | IPLD wire ↔ KnowledgeBase domain |
| `Float(String)` в Constant | ipfrs-tensorlogic | ir.rs | Изоляция IEEE 754 от content-addressed domain |
| `IpfrsBehaviour` + `IpfrsBehaviourEvent` | ipfrs-network | node.rs:288 | libp2p types → IPFRS NetworkEvent |
| `DhtProvider` trait | ipfrs-network | dht_provider.rs | Pluggable Kademlia implementations |
| `ipfs_compat.rs` | ipfrs-network | ipfs_compat.rs | IPFRS ↔ public IPFS network shim |
| `adapters.rs` (`VectorBackend` trait) | ipfrs-semantic | adapters.rs | Изоляция external vector DBs |
| `LocalIndexAdapter` | ipfrs-semantic | federated.rs | Local VectorIndex → QueryableIndex trait |
| `grpc::validation` | ipfrs-interface | grpc.rs:68-243 | CID format + size validation perimeter |
| `streaming::validation` | ipfrs-interface | streaming.rs:1156 | HTTP tier validation perimeter |
| FFI layer (`ffi.rs`) | ipfrs-interface | ffi.rs | Rust panic → C error code; catch_unwind |
| `Arc<S: BlockStore>` blanket impl | ipfrs-storage | traits.rs:87 | DI seam: Arc<Concrete> → trait object |

### Published Languages

| Published Language | Описание |
|--------------------|----------|
| CARv1 + compression extension | `/api/v0/dag/export`, `/api/v0/dag/import` |
| Arrow IPC for tensors | `/v1/tensor/:cid/arrow`, GradientSync gRPC |
| Safetensors format | `/v1/tensor/:cid/safetensors` |
| IPLD DAG-CBOR | Все block storage, KB federation |
| gRPC proto (ipfrs.block.v1, etc.) | BlockService, DagService, FileService, TensorService |
| GossipSub topics (INFERENCE_REQUEST, GRADIENT_SYNC, etc.) | Распределённые ML операции |
| Binary protocol (MAGIC="IPFS", 10-byte header) | Lightweight binary framing |

---

## Приложение A: Критические Баги и Производственные Ограничения

### Не работает в текущей версии

1. **JWT Security** (auth.rs:449): MD5 вместо HS256 — все JWT небезопасны
2. **TLS** (tls.rs:314): SelfSignedCertGenerator возвращает заглушку
3. **Federated Learning** (tensorlogic_ops.rs:1131): accumulate_gradients всегда timeout при min_peers>0
4. **Distributed Proofs** (tensorlogic_ops.rs:1024): find_providers/remote_query — пустые callbacks
5. **GC min_age** (gc.rs:collect): параметр принимается, не применяется
6. **HTTP Compression** (gateway/mod.rs:385): COOLJAPAN policy запрещает tower-http C compression; не подключено
7. **Backpressure decrease** (backpressure.rs:182): semaphore permits не отзываются

### Скрытые производительные ловушки

1. **gc_stats() = O(V+E) BFS** — каждый вызов запускает GC mark phase
2. **DiskANN sort в цикле** — O(n² log n) при large graphs
3. **WebSocket 10ms polling × 10K connections** — 10K sleeping tasks
4. **CAR compression flag collision** — block data начинающийся с 0x00/0x01 = silent corruption
5. **KB federation import O(n×m)** — local_fact_hashes rebuild в цикле

### Intentional Design Decisions (не баги)

1. `Float(String)` в Term: детерминированный хеш важнее арифметической идентичности
2. `xorshift64` везде без `rand` crate: воспроизводимость тестов + zero dependencies
3. GossipSub ML topics в network crate: network layer is ML-aware by design
4. Conservative cycle detection (name+arity): безопасность важнее completeness
5. `BeliefPropagation` = VE: документированная alias; loopy BP запланирован но не реализован

---

*Документ создан Sonnet 4.6 на основе параллельного анализа 6 агентов.*
*Дата: 2026-06-18. Версия кодовой базы: 0.2.0.*
*Supersedes: [[IPFRS_ARCHITECTURE_MASTER.md]], [[IPFRS_DEEP_ARCHITECTURE.md]]*
