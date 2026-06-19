# IPFRS — Deep Domain-Driven Design Analysis

**Version analyzed**: 0.2.0 "Network Release"
**Date**: 2026-06-18
**Audience**: Architects and senior engineers
**Scope**: Code-grounded DDD model derived from the actual sources under `crates/`. Where the prior `ARCHITECTURE_DDD.md` is aspirational/idealized, this document reflects the *real* type definitions, signatures, and invariants found in the tree, with `file:line` anchors.

---

## 0. How to read this document

IPFRS is a Cargo workspace of 12 crates. From a DDD standpoint it is a **modular monolith** organized into bounded contexts, with one **Shared Kernel** (`ipfrs-core`), five **domain contexts** (storage, network, semantic, logic/tensorlogic, transport), and two **presentation/host contexts** (`ipfrs` facade + `ipfrs-interface`, plus `ipfrs-cli` and the language-binding crates).

```
crates/
  ipfrs-core         → SHARED KERNEL (Block, Cid, Ipld, Tensor, Manifest, Codec, Hash)
  ipfrs-storage      → Storage context        (BlockStore port + adapters, GC, tiering, dedup)
  ipfrs-network      → Network context        (Peer aggregate, reputation, DHT, routing)
  ipfrs-semantic     → Semantic context       (HNSW/DiskANN index, quantization, search)
  ipfrs-tensorlogic  → Logic context          (Term/Rule/Fact IR, inference, neural-symbolic)
  ipfrs-transport    → Transport context      (Session aggregate, Bitswap, want-list, multi-transport)
  ipfrs-interface    → Presentation           (gRPC/GraphQL/HTTP/WS, auth, TLS, FFI, Python)
  ipfrs              → APPLICATION FACADE      (Node orchestrator composing all contexts)
  ipfrs-cli          → Presentation (CLI)
  ipfrs-wasm/-nodejs/-python → host/binding adapters
```

The single unifying domain concept is **content-addressing**: every artifact (block, tensor, logic rule, proof, manifest) reduces to a `Cid` computed from its bytes. This is the *ubiquitous language* token that crosses every context boundary, and it is what makes the ACLs between contexts cheap (you pass a `Cid`, not a foreign aggregate).

---

## 1. Strategic design — context map

### 1.1 Context relationships

```
                    ┌──────────────────────────────────────────────┐
                    │          PRESENTATION / HOST                  │
                    │  ipfrs-cli · ipfrs-interface (gRPC/GraphQL/   │
                    │  HTTP/WS) · wasm · nodejs · python · FFI      │
                    └───────────────────────┬──────────────────────┘
                                            │  (depends only on facade)
                    ┌───────────────────────▼──────────────────────┐
                    │       APPLICATION FACADE  (crate: ipfrs)      │
                    │  Node { storage, network, semantic,           │
                    │         tensorlogic, auth, tls, pin, metrics }│
                    └───┬──────────┬──────────┬──────────┬──────────┘
                        │          │          │          │
          ┌─────────────▼───┐ ┌────▼─────┐ ┌──▼───────┐ ┌▼──────────────┐
          │  STORAGE        │ │ NETWORK  │ │ SEMANTIC │ │ LOGIC          │
          │  BlockStore     │ │ Peer     │ │ HNSW/    │ │ KnowledgeBase  │
          │  port+adapters  │ │ reputat. │ │ DiskANN  │ │ Term/Rule/Fact │
          └────────▲────────┘ └────▲─────┘ └────▲─────┘ └──────▲────────┘
                   │               │            │              │
                   │          ┌────┴────────────┴──────────────┘
                   │          │   TRANSPORT (Session, Bitswap, want-list)
                   │          │   uses Network (peers) + Storage (blocks)
                   └──────────┴────────────────────────────────────────┐
                                                                        │
          ┌─────────────────────────────────────────────────────────────────┐
          │              SHARED KERNEL  (crate: ipfrs-core)                   │
          │   Cid · Block · Ipld · TensorBlock · ContentManifest · Codec ·    │
          │   HashEngine · CAR · MerkleTree — re-exported, used by ALL above   │
          └───────────────────────────────────────────────────────────────────┘
```

### 1.2 Context-mapping patterns (Evans/Vernon taxonomy)

| Relationship | Pattern | How it manifests in code |
|---|---|---|
| `ipfrs-core` → all contexts | **Shared Kernel** | `Cid`, `Block`, `Ipld`, `Result/Error` imported by every crate. Single source of domain truth. |
| Storage ← all | **Conformist / Open Host Service** | `BlockStore` trait (`storage/traits.rs`) is a published port; everyone conforms to it. |
| Transport → Storage | **Customer/Supplier + ACL** | `BitswapExchange<S: BlockStore>` calls `store.get/put`; it knows only the trait, never Sled. |
| Transport → Network | **Customer/Supplier** | Transport's `PeerManager` consumes `PeerId`s discovered by Network; reputation is duplicated, not shared (two scoring models — see §11). |
| All domain → libp2p | **Anti-Corruption Layer** | Network wraps `libp2p::PeerId`/`Multiaddr` into `String`-backed domain VOs (`network/peer.rs`, `identity.rs`). |
| Logic → Storage | **Published Language (IPLD)** | `tensorlogic/ipld_codec.rs` serializes `Rule`/`Term` into content-addressed `Block`s. |
| Presentation → Application | **Open Host Service / Facade** | gRPC/GraphQL/CLI all funnel into `Node`; they never touch domain aggregates directly. |
| Bindings (FFI/Python) → Application | **Anti-Corruption Layer** | Opaque `#[repr(C)]` pointers + error-code enums (`interface/ffi.rs`); PyO3 `PyClient` (`interface/python.rs`). |

A noteworthy strategic decision: **reputation/peer-scoring is *not* a shared kernel concept.** Network and Transport each maintain their own scoring model (graph-EMA vs. composite-EWMA). This is deliberate Customer/Supplier separation — Transport scores peers for *this transfer session*, Network scores them for *long-term routing trust*. The duplication is the cost of context autonomy.

---

## 2. The Shared Kernel — `ipfrs-core`

This crate is unambiguously a Shared Kernel: `lib.rs` re-exports `Block, Cid, Ipld, TensorBlock, ContentManifest, MerkleTree, Codec, Error, Result, Car*` etc. (`lib.rs:108–161`), and every other crate imports from it.

### 2.1 `Cid` — the central Value Object

```rust
// core/cid.rs — wraps the external `cid` crate
pub use ::cid::Cid;

pub enum HashAlgorithm {           // cid.rs:12–31
    Sha256, Sha512, Sha3_256, Sha3_512,
    Blake2b256, Blake2b512, Blake2s256, Blake3,
}

pub struct CidBuilder {            // cid.rs:268–283
    version: cid::Version,         // V0 | V1
    codec: u64,                    // 0x55 raw, 0x71 dag-cbor, 0x70 dag-pb, 0x0129 dag-json
    hash_algorithm: HashAlgorithm, // default Sha256
}

#[serde(transparent)]
pub struct SerializableCid(pub Cid); // cid.rs:520–539 — JSON-safe wrapper
```

**Why a Value Object, not an Entity**: identity *is* the value. Two CIDs with the same bytes are the same CID; there is no lifecycle, no mutable state. It is `Copy`-cheap, `Hash`, `Eq`.

**Invariants** (enforced in `CidBuilder::build`):
- CIDv0 ⟹ must be SHA2-256 + dag-pb, else `Error::InvalidInput` (cid.rs:341–350).
- Hash output length fixed by algorithm (32 or 64 bytes; cid.rs:64–77).
- Multibase prefix uniquely determines decode path (`b`=base32, `z`=base58btc; cid.rs:214–259).
- **Determinism axiom**: `hash(d) == hash(d') ⟺ d == d'` (collision-resistant; ~2⁻²⁵⁶).

### 2.2 `Block` — the foundational Aggregate Root

```rust
// core/block.rs:57–63
pub struct Block { cid: Cid, data: Bytes }   // both private — no mutation API

pub const MAX_BLOCK_SIZE: usize = 2 * 1024 * 1024;  // block.rs:37
pub const MIN_BLOCK_SIZE: usize = 1;                // block.rs:40

pub fn new(data: Bytes) -> Result<Self> {           // block.rs:70–74
    Self::validate_size(data.len())?;               // INVARIANT 1
    let cid = CidBuilder::new().build(&data)?;       // INVARIANT 2 (CID = H(data))
    Ok(Self { cid, data })
}

pub fn verify(&self) -> Result<bool> {              // block.rs:117–120
    Ok(CidBuilder::new().build(&self.data)? == self.cid)
}
```

**Aggregate invariants:**
1. **Size bound** — `1 ≤ len ≤ 2 MiB`. The 2 MiB cap is the classic IPFS block limit (network efficiency, predictable memory, fits one QUIC/Bitswap message).
2. **Content-addressing** — the CID is *computed*, never supplied, in the `new()` path. `from_parts(cid, data)` exists for rehydration-without-rehash (block.rs:94), which is the one place the invariant is trusted rather than enforced — a deliberate performance escape hatch for data already validated on the wire.
3. **Immutability** — both fields private; `data()` returns `&Bytes`; `Bytes` is ref-counted so `Clone` is O(1) and never mutates.

This is the model invariant the whole system rests on: **a Block cannot exist in an inconsistent state** (its CID always matches its bytes unless deliberately constructed via `from_parts`).

### 2.3 `Ipld` — the data-model Value Object

```rust
// core/ipld.rs:18–38
pub enum Ipld {
    Null, Bool(bool), Integer(i128), Float(f64),
    String(String), Bytes(Vec<u8>),
    List(Vec<Ipld>),
    Map(BTreeMap<String, Ipld>),     // BTreeMap ⟹ sorted keys ⟹ canonical CBOR
    Link(SerializableCid),           // DAG link, CBOR tag 42
}
```

The `BTreeMap` choice is load-bearing: it guarantees deterministic DAG-CBOR encoding (sorted keys), which guarantees deterministic CIDs, which guarantees the content-addressing axiom across nodes. The `Link` variant is what turns a flat block store into a Merkle-DAG.

### 2.4 Other kernel aggregates

- **`TensorBlock`** (tensor.rs:192–262): `Block + TensorMetadata{shape, dtype}`. Invariant: `data.len() == shape.element_count() * dtype.size_bytes()`. Bridges storage and ML.
- **`ContentManifest`** (manifest.rs:245–300): multi-file aggregate. Invariants: entries canonically sorted by `(path, chunk_index)`; `manifest_id` = FNV-1a of sorted CIDs; `root_cid` = Merkle root → deterministic, verifiable, supports partial retrieval.
- **Chunking** (chunking.rs): `FixedSize` vs `ContentDefined` (Rabin, polynomial `0x3DA3358B4DC173`, 48-byte window). `MAX_LINKS_PER_NODE = 174` bounds DAG fan-out. Encodes the overhead-vs-dedup trade-off.
- **`MerkleBatchProver`** (merkle_batch.rs): O(n + log n) batch proofs instead of O(n log n) — efficient partial verification.
- **CAR** (car.rs): CARv1 archive (header = CBOR `{version, roots}`, then varint-prefixed blocks) for portable DAG transfer.

### 2.5 Kernel domain services & errors

- **`HashEngine` trait** (hash.rs:37–53): `digest/code/name/is_simd_enabled`, runtime CPU-feature detection (AVX2/NEON).
- **`Codec` trait** (codec_registry.rs:29–44): `encode/decode/code/name`; global registry singleton; implementors `DagCborCodec`, `DagJsonCodec`, `RawCodec`.
- **`Error`** (error.rs:38–102): 16 variants with predicate helpers (`is_not_found`, `is_recoverable`). `Result<T> = std::result::Result<T, Error>` is the cross-crate exception boundary.

---

## 3. Storage context — `ipfrs-storage`

**Responsibility**: durable, content-addressed block persistence. ~150 files; the domain core is small, the periphery (tiering, dedup, GC, replication, integrity) is large.

### 3.1 The port — `BlockStore` (Hexagonal architecture)

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
    fn len(&self) -> usize; fn is_empty(&self) -> bool;
    async fn flush(&self) -> Result<()>;
    async fn close(&self) -> Result<()>;
}
// blanket impl for Arc<S: BlockStore>
```

This is the **Repository pattern** + the central **port** of a Ports-and-Adapters design. It is the Open Host Service that Transport and the Application facade conform to.

### 3.2 Adapters (driven side) and decorators

**Backend adapters** (each `impl BlockStore`): `SledBlockStore` (default, blockstore.rs), `ParityDbBlockStore` (paritydb.rs, SSD-optimized), `S3BlockStore` (s3.rs, multipart + semaphore), `MemoryBlockStore` (memory.rs), `StorageObjectStore` (object_store.rs, versioned objects).

**Decorator stack** (each wraps `inner: Arc<S: BlockStore>` and re-implements the trait — Decorator pattern):
```
TtlBlockStore< QuotaBlockStore< CachedBlockStore< DedupBlockStore< CompressionBlockStore< EncryptedBlockStore< SledBlockStore >>>>>>
```
- `CachedBlockStore` (cache.rs): LRU/LFU/TTL hot cache.
- `DedupBlockStore` (dedup.rs): FastCDC chunk-level dedup; `chunk_index: DashMap<Cid, ChunkMeta>`.
- `QuotaBlockStore` (quota.rs): per-tenant byte/block/bandwidth limits with soft/hard thresholds; atomic counters.
- `CompressionBlockStore` (Zstd/Lz4/Snappy via OxiARC), `EncryptedBlockStore` (ChaCha20Poly1305/AES-GCM).

This composition is the elegant heart of the storage design: cross-cutting concerns are **stackable BlockStores**, not flags inside one monolith.

### 3.3 Storage aggregates, value objects, services

| DDD element | Type | Source | Notes |
|---|---|---|---|
| Entity | `PinInfo{pin_type, ref_count}` | pinning.rs | Direct/Recursive/Indirect; GC roots |
| Entity | `SsmSnapshot` + `SnapshotDelta` | snapshot_manager.rs | Chained incremental snapshots |
| Entity | `Transaction{id, status, operations}` | transaction_log.rs | ACID, rollback via `prev_value` |
| Entity | `IndexEntry`, `BlockRecord` | block_index.rs, blockstore_sharding.rs | secondary index, shard record |
| Value Object | `Tier{Hot,Warm,Cold,Archive}` + `TierPolicy` | tiering.rs | classify by access rate |
| Value Object | `QuotaConfig/Usage`, `EvictionPolicy`, `ChunkingConfig` | quota.rs, block_cache.rs, dedup.rs | |
| Domain Service | `GarbageCollector` (mark-and-sweep from pins, incremental batches) | gc.rs | `GcPhase{Mark,Sweep}` |
| Domain Service | `Replicator<S,T>` (`SyncStrategy`, `ConflictStrategy`) | replication.rs | |
| Domain Service | `StorageTierManager`, integrity checker, compactor | tier_manager.rs, integrity_checker.rs | |
| Domain Event | `StorageEvent{event_type, severity, tick, block_cid}` | event_log.rs | bounded queryable log |

### 3.4 Storage invariants

1. **Content-addressing / integrity** — `integrity_checker.rs` recomputes CID; `IntegrityError::CidMismatch{stored, computed}` detects bit-rot. Corruption can only be deleted, never "fixed".
2. **Immutability** — write-once/read-many/delete-once; no update op.
3. **Pin protection** — GC mark starts from pin roots; recursive pins transitively protect a whole DAG.
4. **WAL durability** — entry + checksum written before apply; replay on restart (wal.rs).
5. **Quota** — soft = alert, hard = reject.

### 3.5 Scalability & bottlenecks
Sharding (FNV-1a CID → shard, reduces lock contention), tiering, replication, multi-layer caching, dedup, incremental GC. **Bottlenecks**: serial mark-sweep GC (no cross-shard parallelism), per-store dedup (no cluster-wide chunk index), single-machine caches, master-slave replication only.

---

## 4. Network context — `ipfrs-network`

**Responsibility**: peer identity, discovery, reputation, DHT content routing. ~180 files.

### 4.1 The `Peer` aggregate

```rust
// network/peer.rs:22–41
pub struct PeerInfo {
    pub peer_id: String,            // VO: libp2p::PeerId stringified (ACL)
    pub addrs: Vec<String>,         // Multiaddrs
    pub protocols: Vec<String>,
    pub last_seen: u64,
    pub connection_count: u64,
    pub avg_latency_ms: Option<u64>,
    pub reputation: u8,             // 0..=100
}

struct PeerRecord {                 // peer.rs:64–76 (internal aggregate state)
    info: PeerInfo,
    addrs: HashSet<Multiaddr>,
    connected: bool,
    connected_at: Option<Instant>,
    latency_samples: Vec<Duration>, // bounded
}
```

Repository: **`PeerStore`** (peer.rs:175+) backed by `DashMap<PeerId, PeerRecord>` + `RwLock<HashSet<PeerId>>` for connected set. Methods: `add_peer, get_peer, peer_connected, update_latency, increase/decrease_reputation, peers_by_reputation, peers_by_latency`.

### 4.2 Reputation — a two-tier model (the most interesting domain service here)

**Tier 1 — composite EWMA** (reputation.rs:215–279):
```rust
pub struct ReputationScore {
    transfer_success_rate: f64,        // EWMA
    latency_score, protocol_compliance_score, uptime_score: f64,
    successful_transfers, failed_transfers, protocol_violations: u64,
}
// overall = Σ dimension * weight   (weights sum to 1.0)
// update:  s_new = α·signal + (1-α)·s_old      ; α ≈ 0.2–0.4
// decay:   s *= (1 - 0.1)  per evaluation tick
```
Profiles: Strict (threshold 0.85), Lenient (0.5), Performance (latency_weight 0.5).

**Tier 2 — trust graph** (peer_reputation_graph.rs:115–130):
```rust
pub struct ReputationScore {
    direct_score: f64,       // EMA from direct interactions
    propagated_score: f64,   // BFS multi-hop trust, damping 0.5/hop, depth 3
    combined_score: f64,     // 0.6*direct + 0.4*propagated
    confidence, percentile: f64,
}
// edges weight ∈ [0,1]; trust_decay 0.99/tick; prune edges < 0.01
```

The simple `u8` reputation on `PeerInfo` is the cheap in-band signal; the graph is the heavyweight analytic. **Invariants**: `PeerId` unique (DashMap key), `u8` reputation clamped via `saturating_add/sub` to `[0,100]`, all `f64` scores clamped `[0,1]`.

### 4.3 DHT & content routing

- **`DhtProvider` trait** (dht_provider.rs:138–191): `bootstrap, provide, find_providers, find_peer, get_closest_peers, is_healthy` — pluggable, registry-based. Kademlia params: k-bucket 20, α=3, replication 20, 60s timeout, re-announce every 12h.
- **`ContentRoutingTable`** (routing_table.rs): `add_provider/get_providers/evict_expired`, sharded (routing_table_sharding.rs) to cut lock contention.
- **Semantic DHT** (semantic_dht.rs): LSH maps embeddings → key space; ANN "find peers with similar embeddings"; multi-namespace (text/image/audio); BFS converges at 0.95 agreement. This is what makes routing *semantic* rather than purely hash-based.

### 4.4 Events & ACL
`NebNetworkEvent{id, topic, payload, source_peer}` over a topic event bus with `EventFilter` and a 10k replay buffer (network_event_bus.rs). The **ACL with libp2p** lives in `identity.rs`/`message_codec.rs`: `PeerId↔String`, `Multiaddr` parsing, keypair load/rotate with atomic temp-file+rename, swarm-event → `NetworkEvent` translation.

---

## 5. Semantic context — `ipfrs-semantic`

**Responsibility**: approximate nearest-neighbor vector search and the surrounding retrieval pipeline. ~140 files.

### 5.1 The `VectorIndex` (HNSW) aggregate

```rust
// semantic/hnsw.rs
pub struct VectorIndex {
    index: Arc<RwLock<Hnsw<'static, f32, DistL2>>>,  // hnsw_rs backend
    id_to_cid: Arc<RwLock<HashMap<usize, Cid>>>,
    cid_to_id: Arc<RwLock<HashMap<Cid, usize>>>,
    vectors:   Arc<RwLock<HashMap<Cid, Vec<f32>>>>,  // originals for rebuild/migrate
    next_id:   Arc<RwLock<usize>>,
    dimension: usize,
    metric:    DistanceMetric,                        // L2 | Cosine | DotProduct
    tracker:   Arc<RwLock<IncrementalTracker>>,       // dirty-set for snapshots
}
```

**HNSW structure & params**: multi-layer navigable small-world graph. `M` (max bi-directional links/layer), `ef_construction`, `ef_search`; probabilistic layer assignment `ml = -ln(U(0,1))/ln(2)`; single top-layer entry point; layer 0 holds all nodes. Auto-tuned by size (hnsw.rs:442): `<10k → (M=16, ef_c=200)`, `<100k → (32,400)`, else `(48,600)`.

**Algorithms**:
- *Insert*: validate dim → dedup CID → next id → normalize per-metric → insert into HNSW → store original → update mappings → mark dirty.
- *Search*: greedy descent from entry point per layer, then `SEARCH_LAYER(query, ef_search)` at layer 0; convert internal ids → CIDs; convert distance → score.
- *Delete*: soft (unmap CID; node stays in graph — HNSW has no true delete).

**Value Objects**: `SearchResult{cid, score}`, `DistanceMetric`, `QuantizerCode(Vec<u8>)`, `Codebook{centroids, subspace_dim, num_codes}`.

**Invariants**: dimension consistency (reject mismatched vectors), CID uniqueness, no NaN/Inf (normalize guards `norm>0`), cosine score ∈ [0,1] / L2 ∈ [0,∞), monotonic internal ids, `created_at ≤ updated_at` in metadata.

### 5.2 Alternative index — DiskANN

`DiskANNIndex` (diskann.rs): memory-mapped flat **Vamana** graph (params R=max_degree, L=queue_size, α≈1.2, multiple entry points). Trades per-query latency (page faults) for billion-scale capacity with constant RAM. Contrast: HNSW is in-memory, hierarchical, ~10M-vector practical ceiling.

### 5.3 Domain services
- **`EmbeddingPipeline`** (embedding_pipeline.rs): `EmbeddingInput{RawBytes|Text|Structured|Embedding}` → normalized `Vec<f32>` (`None|L2|MinMax|ZScore`).
- **`VectorQuantizer`** (vector_quantizer.rs): Product Quantization — split D into M subspaces, k-means K≤256 centroids each, encode → M bytes, `asymmetric_distance` for query-to-code. (OPQ rotation is structurally anticipated via the compression codec but the shipped quantizer uses deterministic init, not a learned rotation.)
- **`NearestNeighborQueryPlanner`** (query_planner.rs): `ExecutionStrategy{LocalOnly|RemoteFanout|Hybrid|Cached}`, filters shards by latency budget, prefer-local, fanout cap.
- **`SemanticSearchPipeline`** (search_pipeline.rs): vector + BM25 → fusion (`RRF|LinearCombination|CombSUM`) → rerank.
- **`ReRanker`** (reranking.rs): `WeightedCombination|ReciprocalRankFusion|LearnToRank` over `ScoreComponent{VectorSimilarity, Metadata, Recency, Popularity, Diversity}`.
- **Distribution**: `ShardCoordinator` (consistent-hash ring, FNV-1a, 150 virtual nodes/shard), `AdaptiveIndexPartitioner` (`RebalanceAction{Split|Merge|Migrate}`), `SemanticRoutingTable` DHT.
- **SIMD** (simd.rs): runtime NEON/AVX dispatch for `l2/dot/cosine`.

### 5.4 Bottlenecks & trade-offs
HNSW memory ≈ `n·(dim·4 + M·8)` (768-d, M=16, 10M ≈ 30 GB) → mitigated by PQ (≈12,000× compression, ~0.5–1% recall loss) or DiskANN. Classic recall↔latency knobs: M / ef_construction / ef_search.

---

## 6. Logic context — `ipfrs-tensorlogic`

**Responsibility**: content-addressed symbolic reasoning fused with tensor computation. ~190 files. This is the most conceptually distinctive context.

### 6.1 The IR — Term / Predicate / Rule / KnowledgeBase

```rust
// tensorlogic/ir.rs
pub enum Term {                       // :13–22
    Var(String),                      // ?X
    Const(Constant),
    Fun(String, Vec<Term>),           // f(X,Y)
    Ref(TermRef),                     // CID-addressed external term
}
pub enum Constant { String(String), Int(i64), Bool(bool), Float(String) } // Float-as-string ⟹ deterministic hash
pub struct TermRef { pub cid: Cid, pub hint: Option<String> }             // :38–63

pub struct Predicate { pub name: String, pub args: Vec<Term> }            // :163
pub struct Rule { pub head: Predicate, pub body: Vec<Predicate> }         // Horn clause :216
pub struct KnowledgeBase { pub facts: Vec<Predicate>, pub rules: Vec<Rule> } // aggregate root :277
```

`Substitution = HashMap<String, Term>` is the key Value Object (variable bindings). `KnowledgeBase` is the aggregate root; `Term`/`Predicate`/`Rule` are structural Value Objects (equality by structure → identical rule ⟹ identical CID).

### 6.2 Inference services
```rust
// reasoning.rs — backward chaining (SLD resolution)
pub struct InferenceEngine { max_depth, max_solutions: usize, cycle_detection: bool }
fn query(goal, kb)  -> Result<Vec<Substitution>>
fn prove(goal, kb)  -> Result<Option<Proof>>      // Proof{goal, rule, subproofs}
fn verify(proof,kb) -> Result<bool>
// + unify_predicates, rename_rule_vars (capture avoidance), apply_subst_predicate
```
Variants: `TabledInferenceEngine`/`FixpointEngine` (recursive_reasoning.rs, SLG tabling), temporal (Allen's 13 relations), `FuzzyLogicEngine` (Mamdani/Sugeno + defuzzification), `epistemic_logic.rs` (S5 Kripke, `Knows/CommonKnowledge`), `ProbabilisticLogicNetwork` (`TruthValue{strength, confidence}`, OpenCog-style deduction/induction/abduction), `BayesianNetwork` (VarElim/BeliefProp/Sampling).

### 6.3 The neural-symbolic fusion (the "TensorLogic" thesis)
```rust
// neural_symbolic.rs
pub struct Symbol { id, name, embedding: Vec<f64>, confidence: f64 }
pub struct LogicalRule { head: SymbolId, body: Vec<SymbolId>, weight: f64, rule_type: RuleType }
pub enum RuleType { Definite, Probabilistic, Soft{temperature} }
pub enum InferenceMode { PureSymbolic, PureNeural, Hybrid{neural_weight} }
// confidence = neural_weight·cosine(embeddings) + (1-neural_weight)·forward_chain_confidence
```
Two engines run in parallel — symbolic forward-chaining and neural embedding-similarity — and blend. Plus deterministic tensor execution: `ComputationGraph{TensorOp::{MatMul,Einsum,Softmax,LayerNorm,Fused*}}` with `validate_dag()`/`infer_shapes()`, and reverse-mode `AutogradGraph` (`backward()`, topological gradient accumulation). This is the bridge that lets *learned* concepts and *explicit* rules co-exist in one knowledge base.

### 6.4 Content-addressed logic & distribution
`ipld_codec.rs` maps `RuleIpld`/`TermIpld`/`KnowledgeBaseIpld` ↔ `Block` via `rule_cid()`/`rule_to_block()` — so rules are deduplicated, shareable over Bitswap, and resolvable by IPLD path (`ipld_path.rs`, e.g. `/rule/<cid>/head/args/0`). `DistributedBackwardChainer` (distributed_backward_chainer.rs) delegates unsolved sub-goals to peers (DHT find-providers → remote query), producing a `ProofTree` with **per-peer attribution** — proofs are themselves first-class, cacheable (`proof_cache.rs`, LFU+TTL), verifiable (`proof_verifier.rs`), streamable (`proof_tree_streaming.rs`).

**Invariants**: KB facts ground (no free vars), rule-dependency graph acyclic, head vars bound by body (`rule_validator.rs` → `ValidationError::UnboundVariable/CircularDependency`), identical rule ⟹ identical CID, proof soundness (every node ↔ KB rule/fact) and acyclicity, ComputationGraph is a DAG with shape-consistent ops.

---

## 7. Transport context — `ipfrs-transport`

**Responsibility**: reliable block exchange protocol coordination. ~45 files.

### 7.1 The `Session` aggregate & state machine

```rust
// transport/session.rs
pub struct Session {
    id: SessionId,                       // = u64
    config: SessionConfig,
    state: Arc<RwLock<SessionState>>,
    blocks: Arc<DashMap<Cid, BlockRequest>>,
    stats: Arc<RwLock<SessionStats>>,
    event_tx: Option<mpsc::UnboundedSender<SessionEvent>>,
    state_tx/state_rx: watch::Sender/Receiver<SessionState>,  // reactive state
}

pub enum SessionState { Active, Paused, Completing, Completed, Cancelled }
```

**Valid transitions**: `Active→{Paused, Completing, Completed, Cancelled}`, `Paused→Active`, `*→Cancelled`. **Terminal**: `Completed`, `Cancelled` (no revival). **Completion invariant**: a session enters `Completed` only when `blocks_received + blocks_failed ≥ total_blocks` (`SessionStats::is_complete()`) — *no early completion*. A block can be marked received/failed exactly once.

Repository: **`SessionManager`** (session.rs:462+) over `DashMap<SessionId, Arc<Session>>` with `create/get/remove/active_sessions/cleanup_completed/recv_event`.

### 7.2 Want-list — priority-queue Value Object/service
```rust
// want_list.rs
pub enum Priority { Low=0, Normal=50, High=100, Urgent=200, Critical=300 }
pub struct WantEntry { cid, priority, created_at, expires_at, retry_count,
                       send_dont_have, deadline, tag }   // effective_priority() boosts near deadline
pub struct WantList { heap: BinaryHeap<HeapEntry>, entries: HashMap<Cid,(WantEntry,u64)>, version_counter, config }
```
O(log n) push/pop + O(1) dedup via lazy deletion (version markers skip stale heap entries). Retry backoff `base·2^min(retry,10)` (cap 2^10) then clamped to `max`, with 10% jitter (`want_list.rs:437–459`). **Invariants**: one entry per CID, heap ≤ `max_wants`, FIFO within priority, deadline-elevated priority.

### 7.3 Bitswap, GraphSync, multi-transport
- **Bitswap** (`messages.rs`, `bitswap.rs`): wire `Message{WantList|Block|Have|DontHave|Cancel}` (oxicode/bincode); `BitswapExchange<S: BlockStore>` — `want/receive_block(store.put)/send_block(store.get)/select_peers_for_request`. This is the ACL to Storage (knows only the trait) and Network (consumes `PeerId`).
- **GraphSync** (graphsync.rs): selector-based DAG sync (`Selector{All|Fields|RecursiveDepth|Index|Sequence|Matcher}`, BFS/DFS `TraversalState`) — request *structure*, not enumerated CIDs; resumable.
- **Multi-transport** (transport.rs, multi_transport.rs): `Transport`/`Connection` traits (Strategy) over `TransportType{Quic,Tcp,WebSocket,WebTransport}` with `TransportCapabilities`; `MultiTransportManager` remembers the working transport per peer, falls back Quic→WebTransport→Tcp→WebSocket, caps connection attempts.
- **`PeerManager`** (peer_manager.rs): transport-local peer scoring (`PeerMetrics` EWMA latency/bandwidth/reliability + circuit-breaker blacklist), `SelectionStrategy{FastestFirst|HighestBandwidth|BestScore|RoundRobin|Random|LeastLoaded}`.
- **`RequestCoalescer`** (request_coalescing.rs): dedup concurrent requests for the same CID via broadcast; erasure coding (k+m shards) and `RecoveryManager` (`RecoveryMode{Normal,Degraded,Emergency}`) for resilience.

**Events**: `SessionEvent{Started, BlockReceived, BlockFailed, Progress, Completed, Cancelled}` (in-session) and `TransportEvent` (observability, incl. `CircuitBreakerOpened`, `PartitionDetected/Recovered`).

---

## 8. Application facade & presentation

### 8.1 The `Node` orchestrator (`crate: ipfrs`)
```rust
// ipfrs/src/node/mod.rs:34–49
pub struct Node {
    config: NodeConfig,
    network: Option<NetworkNode>,
    storage: Option<Arc<NodeStore>>,                         // = CachedBlockStore<SledBlockStore>
    semantic: OnceCell<Arc<SemanticRouter>>,                 // lazy
    tensorlogic: OnceCell<Arc<TensorLogicStore<NodeStore>>>, // lazy
    auth_manager: Option<Arc<AuthManager>>,
    tls_manager: Option<Arc<TlsManager>>,
    pin_manager: Arc<PinManager>,
    metrics: Arc<IpfrsMetrics>,
}
```
This is the **Application Layer**: it composes the five domain contexts (plus auth/tls/pin/metrics) behind one struct. `OnceCell` gives zero-cost lazy init for semantic/logic when unused. Use cases: `put_block/get_block/has_block`, `index_content/search_similar`, `add_fact/infer`, `pin_add/ls/rm`, `dag_export/import`.

### 8.2 Interface protocols (`ipfrs-interface`)
Open Host Service exposing the *same* domain operations across many wire formats — all converge on `BlockStore::get/put`, `SemanticRouter`, `TensorLogicStore`:
- **gRPC** (proto/ + grpc.rs): `BlockService`, `FileService`, `DagService`, `TensorService` (streaming get/put, zero-copy slice).
- **GraphQL** (graphql.rs): `QueryRoot{block, has_block, semantic_search, infer}`, `MutationRoot{put_block, index_content, add_fact}`.
- **HTTP/REST** (gateway): Kubo-compatible `/api/v0/add`, `/ipfs/<CID>` content gateway with byte-ranges, `/v1/tensor/<CID>?slice=...`.
- **WebSocket** (websocket.rs): topic subscriptions, `RealtimeEvent{BlockAdded, PeerConnected, DhtQuery*}`.

**Presentation-only concerns (not domain)**: auth (`auth.rs` JWT `Claims`, `Role`, `Permission` RBAC; `oauth2.rs` PKCE), TLS, CORS/rate-limit middleware, `FlowController` backpressure, Prometheus metrics. The zero-copy tensor path (tensor.rs `TensorSlice::extract_data`, safetensors.rs, mmap.rs, arrow.rs, zerocopy.rs) bridges to core `TensorBlock` and tensorlogic.

### 8.3 Bindings as ACL
`interface/ffi.rs`: opaque `#[repr(C)]` `IpfrsClient`/`IpfrsBlock`, `IpfrsErrorCode` enum, `catch_unwind` panic barrier, thread-local `LAST_ERROR`. `interface/python.rs`: PyO3 `PyClient` (`add/get/has`), automatic GIL + `PyErr` mapping. CLI (`ipfrs-cli`) maps verbs → facade calls (`add→put_block`, `query→semantic+logic`, `pin add→PinManager::pin`) via `dispatch.rs`.

**Dependency direction is clean**: presentation depends on the facade/traits only; the gateway state holds `Arc<SledBlockStore>` + trait facades and never imports domain aggregates directly (verified in `gateway/mod.rs`). No reverse dependency from domain → presentation.

---

## 9. Cross-cutting: data flow & event model

### 9.1 `add_file` (write path)
```
CLI/HTTP → Node.add_file
  → tokio::fs::read → (chunking if large) → Block::new(bytes)        [core invariant: CID=H(data)]
  → storage.put(block)        [decorator stack: cache→dedup→compress→sled]
  → StorageEvent::BlockAdded (event_log)
  → (optional) semantic.index_content(cid, embedding)               [HNSW insert]
  → network.provide(cid)      [DHT announce]  → NebNetworkEvent
```

### 9.2 `get(cid)` (read path with miss → network)
```
Node.get(cid)
  → storage.get(cid)  ──hit──► return Block
        └─miss─► transport.SessionManager.create_session([cid])
                  → WantList.add(cid, priority)
                  → PeerManager.select_peers(cid)  ← Network providers (DHT)
                  → Bitswap WantList message → peer
                  → receive Block → block.verify()  [core invariant re-checked on wire]
                  → storage.put(block) → SessionEvent::BlockReceived
                  → session completes when received+failed ≥ total
```

### 9.3 Messaging vs. state, sourcing vs. mutation
- **Within a context**: direct method calls on `Arc<...>` aggregates; concurrency via `DashMap` (lock-free) + `RwLock`/atomics.
- **Across async boundaries**: `tokio::mpsc`/`watch`/`broadcast` channels (session events, WS subscriptions, network event bus).
- **Persistence is state-mutation, not event-sourcing.** The WAL (storage) and transaction log are *recovery/audit* journals layered over a mutable store, not the source of truth. Event logs (`StorageEvent`, `TransportEvent`, `NebNetworkEvent`) are **observability streams**, not an event-sourced aggregate-rebuild mechanism. The closest thing to event sourcing is the snapshot-delta chain (`SnapshotDelta`) and the proof-tree streaming in logic.
- **Immutability everywhere it matters**: blocks, CIDs, terms, manifests are immutable; mutation lives in indexes, peer state, sessions, caches.

---

## 10. Patterns & principles inventory

| Pattern | Where |
|---|---|
| **Shared Kernel** | `ipfrs-core` |
| **Ports & Adapters (Hexagonal)** | `BlockStore` trait + Sled/ParityDb/S3/Memory adapters; `Transport`/`Connection`; `DhtProvider` |
| **Repository** | `BlockStore`, `PeerStore`, `SessionManager`, `KnowledgeBase`, `MetadataStore` |
| **Decorator** | storage decorator stack (cache/dedup/quota/compress/encrypt/ttl) |
| **Strategy** | `DistanceMetric`, `SelectionStrategy`, `EvictionPolicy`, `SyncStrategy`, `TransportSelectionStrategy`, `ChunkingStrategy`, `FusionMethod`, `InferenceMode` |
| **Factory/Builder** | `CidBuilder`, `BlockBuilder`, `Session` creation, gateway `with_*` injection |
| **Observer / Event bus** | network event bus, session events, WS subscription manager |
| **Facade** | `Node` (application), `ipfrs-transport::facade`, `ipfrs-network::facade` |
| **Circuit Breaker** | network + transport peer blacklisting |
| **Anti-Corruption Layer** | libp2p wrapping, FFI/PyO3 boundaries, IPLD codec for logic |
| **Aggregate / Value Object / Domain Service / Domain Event** | per §3–§7 |

**Entity vs Value Object decisions** (the recurring judgment call):
- `Cid`, `Ipld`, `Term`, `SearchResult`, `Priority`, `Tier`, `DistanceMetric` → **VO** (identity = value, immutable).
- `Block`, `TensorBlock`, `Peer`, `Session`, `KnowledgeBase`, `PinInfo`, `Snapshot`, `VectorIndex` → **Aggregate Root / Entity** (have identity + lifecycle/invariants spanning sub-objects).
- Borderline: `Block`'s identity *is* its content hash, which looks VO-ish, but it is treated as an aggregate because it has construction invariants, a `verify()` lifecycle operation, and is the root the repository stores/retrieves. The codebase resolves this by making fields private and exposing no mutation — a "frozen aggregate".

**Aggregate composition**: aggregates stay small and reference others by `Cid`, never by embedding foreign aggregates. A `ContentManifest` holds CIDs, not Blocks; a `Session` holds `Cid → BlockRequest`, not Blocks; a `KnowledgeBase` rule holds `TermRef{cid}`. This keeps aggregate boundaries crisp and makes the whole system distributable — the CID *is* the cross-aggregate reference, and it is content-addressed so references never dangle into mutable state.

---

## 11. Notable design decisions & trade-offs (the "why")

1. **CID as the universal boundary token.** Every ACL is "pass a CID." This is why the contexts compose so cleanly and why distribution is natural — but it also means *everything* must be hashable/serializable to a block, which forced `Float`-as-`String` in the logic IR (deterministic hashing) and `BTreeMap` in `Ipld` (canonical encoding).

2. **Storage cross-cutting concerns as stacked BlockStores** (Decorator) rather than config flags. Trade-off: composition is elegant and testable, but a deep stack adds per-op indirection and makes "where did latency go" harder to reason about.

3. **Two reputation models, intentionally not shared.** Network (long-term routing trust, graph+EMA) vs Transport (per-session transfer quality, EWMA). Duplicated logic is the price of bounded-context autonomy; the alternative (shared kernel reputation) would couple routing policy to transfer policy.

4. **Frozen aggregates + ref-counted `Bytes`.** Immutability is enforced by privacy, not by language-level `const`; `Bytes` makes clones cheap. The `from_parts` escape hatch trades a re-hash for trusting wire-validated data.

5. **HNSW in-memory vs DiskANN on-disk** offered as alternative index aggregates — explicit recall/latency/scale trade-off surfaced to the operator, plus PQ as the compression lever.

6. **Neural-symbolic as a blend, not a replacement.** `InferenceMode::Hybrid{neural_weight}` lets operators dial between interpretable rules and learned similarity — the system commits to neither paradigm.

7. **State-mutation + journals, not event sourcing.** Pragmatic for a storage/network system where throughput matters; WAL/transaction-log give crash recovery without the rebuild cost of full event sourcing.

8. **Lazy context init (`OnceCell`).** A node that only stores/serves blocks pays nothing for the semantic index or logic engine until first use.

### Per-context scalability summary
| Context | Scales via | Primary bottleneck |
|---|---|---|
| Storage | sharding, tiering, decorator caches, dedup, incremental GC | serial mark-sweep GC; per-store dedup |
| Network | sharded routing table, connection pool, churn adaptation, DHT caching | reputation-graph BFS O(E); DHT query latency |
| Semantic | HNSW sub-linear search, PQ compression, consistent-hash sharding, SIMD | HNSW RAM footprint; recall↔latency |
| Logic | rule/term indexing, proof caching, SLG tabling, distributed backward chaining | exponential proof-tree growth; remote round-trips |
| Transport | concurrent sessions (DashMap), request coalescing, multi-transport | O(n) peer scan; per-connection single-threaded I/O |

---

## 12. Discrepancies between `ARCHITECTURE_DDD.md` (idealized) and the code

The shipped code is **richer** than the prior doc and differs in specifics worth flagging:
- `Block` fields are private (`cid`, `data`) — the doc shows a `metadata` field that does not exist on `Block` itself; metadata is a separate `BlockMetadata` VO.
- Reputation is **not** a single `update_score(success)` with fixed REWARD/0.95 decay as the doc sketches; it is a two-tier EWMA + trust-graph model, and the in-band score is a clamped `u8`.
- `BlockExchangeSession` in the doc = `Session` in code; states are `{Active, Paused, Completing, Completed, Cancelled}` (doc omits `Completing`).
- Semantic similarity range `[0,1]` holds only for cosine; L2/DotProduct have different ranges (the doc over-generalizes).
- Logic `Term` variants are `Var/Const/Fun/Ref` (with CID-addressed `Ref`), not `Constant/Variable/Compound` as the doc states.
- The neural-symbolic tensor side (`ComputationGraph`, `Autograd`) is entirely absent from the prior doc but is the defining feature of the Logic context.

---

## 13. One-screen reference

```
SHARED KERNEL  Cid(VO) · Block(AR, CID=H(data), ≤2MiB) · Ipld(VO, sorted-map) ·
               TensorBlock(AR) · ContentManifest(AR, Merkle) · Codec/HashEngine(svc)

STORAGE        port BlockStore ; adapters Sled/ParityDb/S3/Memory ;
               decorators Cache/Dedup/Quota/Compress/Encrypt/Ttl ;
               AR Pin/Snapshot/Transaction ; svc GC/Replicator/Tiering/Integrity ;
               evt StorageEvent ; inv content-addr, immutable, pin-protect, WAL

NETWORK        AR Peer(PeerStore) ; VO PeerId/Multiaddr/Score/Capability ;
               reputation = EWMA ⊕ trust-graph(0.6 direct+0.4 propagated) ;
               DhtProvider(port) + semantic DHT(LSH) ; ACL→libp2p ; evt NebNetworkEvent

SEMANTIC       AR VectorIndex(HNSW: M/ef_c/ef_s, prob layers) | DiskANN(Vamana,mmap) ;
               VO SearchResult/DistanceMetric/Codebook ; svc EmbeddingPipeline/
               Quantizer(PQ)/QueryPlanner/ReRanker ; ShardCoordinator(consistent-hash)

LOGIC          IR Term(Var/Const/Fun/Ref) · Predicate · Rule(Horn) · KnowledgeBase(AR) ;
               svc InferenceEngine(SLD)/Tabling/Temporal/Fuzzy/Epistemic/PLN/Bayes ;
               neural-symbolic blend + ComputationGraph/Autograd ;
               IPLD codec → CID-addressed rules ; ProofTree(AR, peer-attributed)

TRANSPORT      AR Session(states Active/Paused/Completing/Completed/Cancelled,
               complete ⟺ recv+fail≥total) ; VO SessionId/WantEntry/Priority/Message ;
               svc WantList(heap+lazy-del)/PeerManager/Bitswap/GraphSync/MultiTransport/
               Coalescer/Erasure/Recovery ; evt SessionEvent/TransportEvent

FACADE         Node{storage,network,semantic(OnceCell),tensorlogic(OnceCell),auth,tls,pin,metrics}
PRESENTATION   gRPC·GraphQL·HTTP·WS·CLI·FFI·PyO3 → all converge on facade use-cases
```

---

*Generated from source inspection of IPFRS 0.2.0. All `file:line` anchors refer to `crates/<crate>/src/`.*
