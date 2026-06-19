# Shared Kernel — ipfrs-core

> **Focus**: Domain primitives, value objects, invariants  
> **Source**: `ipfrs_source/crates/ipfrs-core/src/` (28 files)

---

## 1. Purpose

Shared Kernel — это **минимальный набор типов**, которые:
- Используются всеми bounded contexts
- Представляют ubiquitous language системы
- Гарантируют invariants на границах contexts

```
┌─────────────────────────────────────────────────────────────────────┐
│                    SHARED KERNEL LAYER                              │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    VALUE OBJECTS                             │   │
│  │  ─────────────────────────────────────────────────────────── │   │
│  │  Cid          — Content identifier (identity = value)        │   │
│  │  Ipld         — IPLD data model (9 variants)                 │   │
│  │  HashAlgorithm — 8 hash algorithms (enum)                    │   │
│  │  DistanceMetric — L2/Cosine/DotProduct                       │   │
│  │  Priority      — Low/Normal/High/Urgent/Critical             │   │
│  │  Tier          — Hot/Warm/Cold/Archive                       │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    AGGREGATE ROOTS                           │   │
│  │  ─────────────────────────────────────────────────────────── │   │
│  │  Block         — Content-addressed block                     │   │
│  │  TensorBlock   — ML tensor wrapper                           │   │
│  │  ContentManifest — Multi-file manifest                       │   │
│  │  MerkleTree    — Batch-provable Merkle structure             │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    DOMAIN SERVICES                           │   │
│  │  ─────────────────────────────────────────────────────────── │   │
│  │  HashEngine    — Hash abstraction (trait + 8 impls)          │   │
│  │  Codec         — Encoding/decoding (trait + registry)        │   │
│  │  Chunker       — Fixed/Content-defined chunking              │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    CROSS-CUTTING                             │   │
│  │  ─────────────────────────────────────────────────────────── │   │
│  │  Error         — 16 variants, thiserror                      │   │
│  │  Result<T>     — std::result::Result<T, Error>               │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 2. Cid — The Central Value Object

### 2.1 Definition

```rust
// core/cid.rs — wraps external `cid` crate
pub use ::cid::Cid;

pub enum HashAlgorithm {
    Sha256, Sha512, Sha3_256, Sha3_512,
    Blake2b256, Blake2b512, Blake2s256, Blake3,
}
```

### 2.2 Why Value Object?

**Identity = Value**: Two CIDs with same bytes are the same CID.

- `Copy` — cheap to pass around
- `Hash` — usable as HashMap key
- `Eq` — structural equality
- No lifecycle, no mutation

### 2.3 Invariants

| Invariant | Enforcement |
|-----------|-------------|
| CIDv0 ⟹ SHA2-256 + dag-pb | `CidBuilder::build()` validates |
| Hash length fixed by algorithm | 32 or 64 bytes |
| Multibase prefix determines decode | `b` = base32, `z` = base58btc |

### 2.4 CidBuilder

```rust
pub struct CidBuilder {
    version: cid::Version,         // V0 | V1
    codec: u64,                    // 0x55 raw, 0x71 dag-cbor, etc.
    hash_algorithm: HashAlgorithm, // default Sha256
}

impl CidBuilder {
    pub fn build(&self, data: &[u8]) -> Result<Cid> {
        // INVARIANT: CID = H(data)
        let hash = self.hash_algorithm.digest(data);
        Ok(Cid::new_v1(self.codec, hash))
    }
}
```

---

## 3. Block — The Foundational Aggregate Root

### 3.1 Definition

```rust
// core/block.rs:57–63
pub struct Block {
    cid: Cid,      // private — no mutation
    data: Bytes,   // private — ref-counted
}

pub const MAX_BLOCK_SIZE: usize = 2 * 1024 * 1024;  // 2 MiB
pub const MIN_BLOCK_SIZE: usize = 1;
```

### 3.2 Invariants

| Invariant | Enforcement |
|-----------|-------------|
| `1 ≤ len ≤ 2 MiB` | `Block::validate_size()` |
| `hash(data) == cid` | `new()` computes CID |
| Immutability | Fields private, no mutation API |

### 3.3 Construction

```rust
impl Block {
    // Primary constructor — INVARIANT enforced
    pub fn new(data: Bytes) -> Result<Self> {
        Self::validate_size(data.len())?;
        let cid = CidBuilder::new().build(&data)?;
        Ok(Self { cid, data })
    }
    
    // Escape hatch for rehydration — INVARIANT trusted
    pub fn from_parts(cid: Cid, data: Bytes) -> Result<Self> {
        Self::validate_size(data.len())?;
        Ok(Self { cid, data })
    }
    
    // Verify invariant
    pub fn verify(&self) -> Result<bool> {
        Ok(CidBuilder::new().build(&self.data)? == self.cid)
    }
}
```

### 3.4 Why Aggregate Root?

**Block** — это Aggregate Root, потому что:
1. Имеет construction invariants
2. Имеет lifecycle (`verify()`)
3. Repository (`BlockStore`) stores/retrieves by Block

**Но** — frozen aggregate: fields immutable, `Clone` is O(1) via `Bytes`.

---

## 4. Ipld — The Data Model Value Object

### 4.1 Definition

```rust
// core/ipld.rs:18–38
pub enum Ipld {
    Null,
    Bool(bool),
    Integer(i128),
    Float(f64),
    String(String),
    Bytes(Vec<u8>),
    List(Vec<Ipld>),
    Map(BTreeMap<String, Ipld>),  // BTreeMap → sorted keys
    Link(SerializableCid),        // DAG link
}
```

### 4.2 Why BTreeMap?

**Determinism**: `BTreeMap` guarantees sorted keys → canonical CBOR encoding → deterministic CIDs.

```rust
// This matters:
let map1 = BTreeMap::from([("b", 1), ("a", 2)]);
let map2 = BTreeMap::from([("a", 2), ("b", 1)]);
// map1 == map2 (sorted)
// CBOR(map1) == CBOR(map2) (canonical)
// CID(map1) == CID(map2) (deterministic)
```

### 4.3 Link — The DAG Mechanism

```rust
Ipld::Link(SerializableCid)  // CBOR tag 42
```

**Это** превращает плоский block store в Merkle-DAG. `Link` — это cross-aggregate reference.

---

## 5. TensorBlock — ML Integration

### 5.1 Definition

```rust
// core/tensor.rs:192–262
pub struct TensorBlock {
    block: Block,
    shape: Vec<usize>,
    dtype: DType,
}

pub enum DType {
    F32, F64, I32, I64, U8, U16, U32, U64,
}
```

### 5.2 Invariant

```rust
impl TensorBlock {
    pub fn new(data: Bytes, shape: Vec<usize>, dtype: DType) -> Result<Self> {
        let expected_len = shape.iter().product::<usize>() * dtype.size_bytes();
        if data.len() != expected_len {
            return Err(Error::ShapeMismatch);
        }
        let block = Block::new(data)?;
        Ok(Self { block, shape, dtype })
    }
}
```

**Bridges** storage and ML: block semantics + tensor metadata.

---

## 6. ContentManifest — Multi-File Aggregate

### 6.1 Definition

```rust
// core/manifest.rs:245–300
pub struct ContentManifest {
    entries: Vec<ManifestEntry>,  // sorted by (path, chunk_index)
    manifest_id: Cid,             // FNV-1a of sorted CIDs
    root_cid: Cid,                // Merkle root
}

pub struct ManifestEntry {
    path: String,
    chunk_index: usize,
    cid: Cid,
    size: u64,
}
```

### 6.2 Invariants

| Invariant | Enforcement |
|-----------|-------------|
| Entries sorted | `entries.sort()` on construction |
| `manifest_id` = FNV-1a(sorted CIDs) | Computed, not supplied |
| `root_cid` = Merkle root | Computed from entries |

### 6.3 Use Case

Multi-file uploads (e.g., datasets):
1. Chunk files → Blocks
2. Create manifest with all CIDs
3. Store manifest as Block
4. Share single manifest CID

---

## 7. HashEngine — Domain Service

### 7.1 Trait

```rust
// core/hash.rs:37–53
pub trait HashEngine: Send + Sync {
    fn digest(&self, data: &[u8]) -> Vec<u8>;
    fn code(&self) -> u64;
    fn name(&self) -> &'static str;
    fn is_simd_enabled(&self) -> bool;
}
```

### 7.2 Implementations

| Algorithm | Code | SIMD |
|-----------|------|------|
| Sha256 | 0x12 | AVX2/NEON |
| Sha512 | 0x13 | — |
| Sha3-256 | 0x16 | — |
| Blake2b-256 | 0xb220 | AVX2 |
| Blake3 | 0x1e | AVX2/NEON |

### 7.3 Registry Pattern

```rust
lazy_static! {
    static ref HASH_REGISTRY: RwLock<HashMap<u64, Box<dyn HashEngine>>> = ...;
}

pub fn register(engine: Box<dyn HashEngine>) {
    HASH_REGISTRY.write().insert(engine.code(), engine);
}

pub fn get(code: u64) -> Option<&'static dyn HashEngine> {
    HASH_REGISTRY.read().get(&code).map(|e| e.as_ref())
}
```

---

## 8. Codec — Encoding Domain Service

### 8.1 Trait

```rust
// core/codec_registry.rs:29–44
pub trait Codec: Send + Sync {
    fn encode(&self, ipld: &Ipld) -> Result<Vec<u8>>;
    fn decode(&self, data: &[u8]) -> Result<Ipld>;
    fn code(&self) -> u64;
    fn name(&self) -> &'static str;
}
```

### 8.2 Implementations

| Codec | Code | Use Case |
|-------|------|----------|
| Raw | 0x55 | Raw bytes |
| DagCbor | 0x71 | Canonical encoding |
| DagJson | 0x0129 | Human-readable |
| DagPb | 0x70 | Protocol Buffers |

### 8.3 Global Registry

```rust
lazy_static! {
    static ref CODEC_REGISTRY: RwLock<HashMap<u64, Box<dyn Codec>>> = ...;
}
```

---

## 9. Chunker — File Splitting

### 9.1 Strategies

```rust
pub enum ChunkingStrategy {
    FixedSize { size: usize },
    ContentDefined { min: usize, max: usize, target: usize },
}

pub struct RabinChunker {
    polynomial: u64,         // 0x3DA3358B4DC173
    window_size: usize,      // 48 bytes
    mask: u64,               // For boundary detection
}
```

### 9.2 Trade-offs

| Strategy | Pros | Cons |
|----------|------|------|
| Fixed | Simple, predictable | Poor dedup |
| Content-defined | Good dedup | Variable sizes |

**MAX_LINKS_PER_NODE = 174** — bounds DAG fan-out.

---

## 10. Error — Cross-Context Exception Boundary

### 10.1 Definition

```rust
// core/error.rs:38–102
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Block not found: {0}")]
    NotFound(String),
    
    #[error("Invalid block size: {0}")]
    InvalidBlockSize(usize),
    
    #[error("CID mismatch: stored {stored}, computed {computed}")]
    CidMismatch { stored: Cid, computed: Cid },
    
    #[error("Hash error: {0}")]
    HashError(String),
    
    #[error("Codec error: {0}")]
    CodecError(String),
    
    // ... 16 total variants
}

pub type Result<T> = std::result::Result<T, Error>;
```

### 10.2 Predicate Helpers

```rust
impl Error {
    pub fn is_not_found(&self) -> bool {
        matches!(self, Error::NotFound(_))
    }
    
    pub fn is_recoverable(&self) -> bool {
        matches!(self, Error::NotFound(_) | Error::Timeout(_))
    }
}
```

---

## 11. MerkleTree — Batch Proofs

### 11.1 Definition

```rust
// core/merkle_batch.rs
pub struct MerkleBatchProver {
    tree: MerkleTree,
}

impl MerkleBatchProver {
    // O(n + log n) batch proofs instead of O(n log n)
    pub fn prove_batch(&self, indices: &[usize]) -> Vec<Proof>;
    
    pub fn verify_batch(root: &Cid, proofs: &[Proof]) -> bool;
}
```

### 11.2 Use Case

Partial verification: prove specific leaves without entire tree.

---

## 12. CAR — Archive Format

### 12.1 Definition

```rust
// core/car.rs
pub struct CarHeader {
    version: u64,
    roots: Vec<Cid>,
}

pub struct CarWriter {
    header: CarHeader,
    blocks: Vec<Block>,
}

pub struct CarReader {
    header: CarHeader,
    // Stream blocks
}
```

### 12.2 Format

```
[ header (CBOR, varint-prefixed) ]
[ block1 (varint-prefixed) ]
[ block2 (varint-prefixed) ]
...
```

**Portable DAG transfer**: single file = entire graph.

---

## 13. Key Invariants Summary

| Type | Invariant | Enforcement |
|------|-----------|-------------|
| `Cid` | `hash(data) == cid` | `CidBuilder::build()` |
| `Block` | `1 ≤ len ≤ 2 MiB` | `validate_size()` |
| `Block` | `cid = H(data)` | `new()` computes |
| `Ipld::Map` | Sorted keys | `BTreeMap` |
| `TensorBlock` | `data.len == shape × dtype` | Constructor validates |
| `Manifest` | Entries sorted | `sort()` on construction |
| `Manifest` | `manifest_id` deterministic | FNV-1a computed |

---

## 14. Design Decisions

### 14.1 Why Minimal Shared Kernel?

**Decision**: Only truly shared types in `ipfrs-core`.

**Rationale**:
- Shared Kernel = coordination cost
- Every change affects all contexts
- Keep it minimal

**What's NOT in Shared Kernel**:
- Reputation (duplicated in Network/Transport)
- Session state (Transport-specific)
- Embedding vectors (Semantic-specific)

---

### 14.2 Why Frozen Aggregates?

**Decision**: `Block`, `TensorBlock` are immutable.

**Rationale**:
- Content-addressing requires immutability
- `Bytes` makes `Clone` O(1)
- Thread-safe by construction

**Escape hatch**: `from_parts()` for wire-validated data (avoid re-hash).

---

### 14.3 Why BTreeMap in Ipld?

**Decision**: `Ipld::Map` uses `BTreeMap`, not `HashMap`.

**Rationale**:
- Canonical encoding = deterministic CIDs
- Two maps with same entries = same CID
- Critical for cross-node consistency

---

## 15. Files Reference

| File | Lines | Purpose |
|------|-------|---------|
| `cid.rs` | 540+ | CID, CidBuilder, HashAlgorithm |
| `block.rs` | 300+ | Block aggregate |
| `ipld.rs` | 250+ | IPLD data model |
| `tensor.rs` | 350+ | TensorBlock |
| `manifest.rs` | 400+ | ContentManifest |
| `hash.rs` | 200+ | HashEngine trait |
| `codec_registry.rs` | 180+ | Codec trait + registry |
| `chunking.rs` | 250+ | Chunker implementations |
| `error.rs` | 150+ | Error enum |
| `merkle_batch.rs` | 200+ | Batch Merkle proofs |
| `car.rs` | 300+ | CAR format |
| `pool.rs` | 150+ | Bytes/CidString pools |

---

## 16. Cross-Context Usage

```
┌─────────────────────────────────────────────────────────────────────┐
│                    SHARED KERNEL USAGE                              │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Storage:                                                           │
│    • Block (put/get)                                                │
│    • Cid (key)                                                      │
│    • Error/Result                                                   │
│                                                                     │
│  Network:                                                           │
│    • Cid (DHT key, content routing)                                 │
│    • Block (wire protocol)                                          │
│    • HashEngine (peer ID)                                           │
│                                                                     │
│  Semantic:                                                          │
│    • Cid (vector identity)                                          │
│    • TensorBlock (tensor storage)                                   │
│                                                                     │
│  Logic:                                                             │
│    • Cid (rule identity)                                            │
│    • Block (rule storage)                                           │
│    • Ipld (serialization)                                           │
│                                                                     │
│  Transport:                                                         │
│    • Block (Bitswap messages)                                       │
│    • Cid (want-list key)                                            │
│                                                                     │
│  Interface:                                                         │
│    • All of the above (gateway)                                     │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

**Next**: [03-StorageContext.md](03-StorageContext.md) — BlockStore port, decorators, GC
