# Storage Context — BlockStore Port & Adapters

> **Focus**: Ports & Adapters pattern, decorator stack, GC, tiering  
> **Source**: `ipfrs_source/crates/ipfrs-storage/src/` (~150 files)

---

## 1. Context Overview

Storage Context отвечает за **durable, content-addressed block persistence**.

```
┌─────────────────────────────────────────────────────────────────────┐
│                    STORAGE CONTEXT                                  │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    PORT (BlockStore trait)                   │   │
│  │  ─────────────────────────────────────────────────────────── │   │
│  │  put / put_many / get / get_many / has / delete / list_cids  │   │
│  │  len / is_empty / flush / close                              │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                              │                                      │
│                              ▼                                      │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    ADAPTERS (impl BlockStore)                │   │
│  │  ─────────────────────────────────────────────────────────── │   │
│  │  SledBlockStore      — Primary (embedded DB)                 │   │
│  │  ParityDbBlockStore  — SSD-optimized                         │   │
│  │  S3BlockStore        — Cloud object storage                  │   │
│  │  MemoryBlockStore    — Testing                               │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                              │                                      │
│                              ▼                                      │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    DECORATORS (wrap BlockStore)              │   │
│  │  ─────────────────────────────────────────────────────────── │   │
│  │  CachedBlockStore    — LRU/LFU/TTL cache                     │   │
│  │  DedupBlockStore     — Content-defined dedup                 │   │
│  │  QuotaBlockStore     — Per-tenant limits                     │   │
│  │  CompressionBlockStore — Zstd/Lz4/Snappy                     │   │
│  │  EncryptedBlockStore — ChaCha20/AES-GCM                      │   │
│  │  TtlBlockStore       — Time-to-live expiration               │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    DOMAIN SERVICES                           │   │
│  │  ─────────────────────────────────────────────────────────── │   │
│  │  GarbageCollector    — Mark-sweep from pin roots             │   │
│  │  PinManager          — Direct/Recursive/Indirect pins        │   │
│  │  TieredStore         — Hot/Warm/Cold/Archive tiering         │   │
│  │  Replicator          — Sync (Full/Incremental)               │   │
│  │  IntegrityChecker    — CID verification                      │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    AGGREGATES                                │   │
│  │  ─────────────────────────────────────────────────────────── │   │
│  │  PinInfo             — Pin state (type, ref_count)           │   │
│  │  Snapshot            — Incremental snapshots                 │   │
│  │  Transaction         — ACID transaction log                  │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 2. The Port — BlockStore Trait

### 2.1 Definition

```rust
// storage/traits.rs
#[async_trait]
pub trait BlockStore: Send + Sync {
    // Write operations
    async fn put(&self, block: &Block) -> Result<()>;
    async fn put_many(&self, blocks: &[Block]) -> Result<()>;
    
    // Read operations
    async fn get(&self, cid: &Cid) -> Result<Option<Block>>;
    async fn get_many(&self, cids: &[Cid]) -> Result<Vec<Option<Block>>>;
    async fn has(&self, cid: &Cid) -> Result<bool>;
    
    // Delete operations
    async fn delete(&self, cid: &Cid) -> Result<()>;
    
    // Metadata
    fn list_cids(&self) -> Result<Vec<Cid>>;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool { self.len() == 0 }
    
    // Lifecycle
    async fn flush(&self) -> Result<()>;
    async fn close(&self) -> Result<()>;
}

// Blanket impl for Arc<S: BlockStore>
#[async_trait]
impl<S: BlockStore> BlockStore for Arc<S> {
    // Delegates to inner
}
```

### 2.2 Why a Port?

**Hexagonal Architecture**: `BlockStore` — это порт, который:
- Определяет contract для storage operations
- Позволяет любой backend impl
- Изолирует domain от infrastructure

**All contexts conform to this trait** — они знают только interface, не implementation.

---

## 3. Adapters — Backend Implementations

### 3.1 SledBlockStore (Primary)

```rust
// storage/blockstore.rs
pub struct SledBlockStore {
    db: Db,
    tree: Tree,
    cache: LruCache<Cid, Block>,
    config: SledConfig,
    dedup_stats: DedupStats,
}

impl SledBlockStore {
    pub fn new(path: &Path, config: SledConfig) -> Result<Self>;
    
    // Additional methods
    pub async fn put_if_absent(&self, block: &Block) -> Result<bool>;
    pub async fn put_batch_dedup(&self, blocks: &[Block]) -> Result<DedupStats>;
}
```

**Sled**: Embedded, ACID, high-performance key-value store.

---

### 3.2 ParityDbBlockStore

```rust
// storage/paritydb.rs
pub struct ParityDbBlockStore {
    db: parity_db::Db,
    config: ParityDbConfig,
}
```

**ParityDB**: SSD-optimized, used in blockchain nodes.

---

### 3.3 S3BlockStore

```rust
// storage/s3.rs
pub struct S3BlockStore {
    client: S3Client,
    bucket: String,
    prefix: String,
    semaphore: Semaphore,  // Concurrent request limit
}

impl S3BlockStore {
    // Multipart upload for large blocks
    async fn put_multipart(&self, block: &Block) -> Result<()>;
}
```

---

### 3.4 MemoryBlockStore

```rust
// storage/memory.rs
pub struct MemoryBlockStore {
    blocks: DashMap<Cid, Block>,
}

// Used for testing
```

---

## 4. Decorators — Cross-Cutting Concerns

### 4.1 The Stack

```
┌────────────────────────────────────────────────────────────────────┐
│                    DECORATOR STACK                                 │
├────────────────────────────────────────────────────────────────────┤
│                                                                    │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │  TtlBlockStore                                               │  │
│  │  — Expire blocks after TTL                                   │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                              │                                     │
│                              ▼                                     │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │  QuotaBlockStore                                             │  │
│  │  — Enforce per-tenant limits                                 │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                              │                                     │
│                              ▼                                     │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │  CachedBlockStore                                            │  │
│  │  — LRU/LFU hot cache                                         │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                              │                                     │
│                              ▼                                     │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │  DedupBlockStore                                             │  │
│  │  — Content-defined dedup                                     │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                              │                                     │
│                              ▼                                     │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │  CompressionBlockStore                                       │  │
│  │  — Zstd/Lz4/Snappy compression                               │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                              │                                     │
│                              ▼                                     │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │  EncryptedBlockStore                                         │  │
│  │  — ChaCha20/AES-GCM encryption                               │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                              │                                     │
│                              ▼                                     │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │  SledBlockStore (inner)                                      │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                                                                    │
└────────────────────────────────────────────────────────────────────┘
```

### 4.2 Decorator Pattern

```rust
pub struct CachedBlockStore<S: BlockStore> {
    inner: Arc<S>,
    cache: LruCache<Cid, Block>,
    config: CacheConfig,
}

#[async_trait]
impl<S: BlockStore> BlockStore for CachedBlockStore<S> {
    async fn get(&self, cid: &Cid) -> Result<Option<Block>> {
        // 1. Check cache
        if let Some(block) = self.cache.get(cid) {
            return Ok(Some(block));
        }
        
        // 2. Delegate to inner
        let block = self.inner.get(cid).await?;
        
        // 3. Cache result
        if let Some(ref b) = block {
            self.cache.put(cid.clone(), b.clone());
        }
        
        Ok(block)
    }
    
    async fn put(&self, block: &Block) -> Result<()> {
        self.inner.put(block).await?;
        self.cache.put(block.cid().clone(), block.clone());
        Ok(())
    }
    
    // ... other methods delegate
}
```

### 4.3 Why Decorators?

**Benefits**:
- **Composable** — mix and match concerns
- **Testable** — test each in isolation
- **Configurable** — enable/disable at runtime

**Trade-off**: Per-op indirection adds latency; deep stacks harder to debug.

---

## 5. Pin Management

### 5.1 Pin Types

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
```

### 5.2 PinManager

```rust
pub struct PinManager {
    pins: DashMap<Cid, PinInfo>,
    store: Arc<dyn BlockStore>,
    config: PinConfig,
}

impl PinManager {
    pub async fn pin(&self, cid: &Cid, pin_type: PinType) -> Result<()>;
    pub async fn unpin(&self, cid: &Cid) -> Result<()>;
    pub async fn is_pinned(&self, cid: &Cid) -> bool;
    
    // Recursive pin traverses DAG
    async fn pin_recursive(&self, cid: &Cid) -> Result<Vec<Cid>>;
}
```

### 5.3 Invariant

**Pin-protected blocks cannot be GC'd** — GC mark phase starts from pin roots.

---

## 6. Garbage Collector

### 6.1 Mark-Sweep Algorithm

```rust
pub struct GarbageCollector<S: BlockStore> {
    store: Arc<S>,
    pin_manager: Arc<PinManager>,
    config: GcConfig,
}

pub struct GcConfig {
    pub incremental: bool,      // Batch deletion
    pub batch_size: usize,
    pub dry_run: bool,
}

pub enum GcPhase {
    Mark,
    Sweep,
}

impl<S: BlockStore> GarbageCollector<S> {
    pub async fn run(&self) -> Result<GcStats> {
        // Phase 1: Mark
        let marked = self.mark_phase().await?;
        
        // Phase 2: Sweep
        let deleted = self.sweep_phase(marked).await?;
        
        Ok(GcStats { marked, deleted })
    }
    
    async fn mark_phase(&self) -> Result<HashSet<Cid>> {
        let mut marked = HashSet::new();
        
        // Start from pin roots
        for (cid, _) in self.pin_manager.list().await? {
            self.mark_recursive(&cid, &mut marked).await?;
        }
        
        Ok(marked)
    }
    
    async fn sweep_phase(&self, marked: HashSet<Cid>) -> Result<usize> {
        let mut deleted = 0;
        let all_cids = self.store.list_cids()?;
        
        for cid in all_cids {
            if !marked.contains(&cid) {
                self.store.delete(&cid).await?;
                deleted += 1;
            }
        }
        
        Ok(deleted)
    }
}
```

### 6.2 Incremental GC

For large stores, GC runs in batches:

```rust
pub struct GcScheduler<S: BlockStore> {
    gc: GarbageCollector<S>,
    interval: Duration,
    running: AtomicBool,
}

impl<S: BlockStore> GcScheduler<S> {
    pub fn start(&self) {
        tokio::spawn(async move {
            loop {
                sleep(self.interval).await;
                if !self.running.swap(true, Ordering::SeqCst) {
                    self.gc.run().await.ok();
                    self.running.store(false, Ordering::SeqCst);
                }
            }
        });
    }
}
```

---

## 7. Tiered Storage

### 7.1 Tiers

```rust
pub enum Tier {
    Hot,      // Fast SSD, frequently accessed
    Warm,     // Slower SSD, less frequent
    Cold,     // HDD, rarely accessed
    Archive,  // Tape/cloud, long-term
}

pub struct TierPolicy {
    pub access_threshold: u64,   // Accesses per day
    pub age_threshold: Duration, // Time since last access
}
```

### 7.2 TieredStore

```rust
pub struct TieredStore {
    stores: HashMap<Tier, Arc<dyn BlockStore>>,
    tracker: AccessTracker,
    policies: HashMap<Tier, TierPolicy>,
}

impl TieredStore {
    pub async fn classify_tier(&self, cid: &Cid) -> Tier {
        let stats = self.tracker.get_stats(cid);
        
        if stats.access_rate > self.policies[&Tier::Hot].access_threshold {
            Tier::Hot
        } else if stats.access_rate > self.policies[&Tier::Warm].access_threshold {
            Tier::Warm
        } else {
            Tier::Cold
        }
    }
    
    pub async fn migrate_cold_blocks(&self) -> Result<usize> {
        // Move cold blocks to archive tier
    }
}
```

---

## 8. Replication

### 8.1 Sync Strategies

```rust
pub enum SyncStrategy {
    Full,          // Complete sync
    Incremental,   // Delta-based
}

pub enum ConflictStrategy {
    SourceWins,
    TargetWins,
    KeepBoth,
    Error,
}
```

### 8.2 Replicator

```rust
pub struct Replicator<S: BlockStore, T: BlockStore> {
    source: Arc<S>,
    target: Arc<T>,
    config: ReplicatorConfig,
}

impl<S: BlockStore, T: BlockStore> Replicator<S, T> {
    pub async fn sync_full(&self) -> Result<SyncStats>;
    pub async fn sync_incremental(&self, since: u64) -> Result<SyncStats>;
    pub async fn verify(&self) -> Result<bool>;
}
```

---

## 9. WAL — Write-Ahead Log

### 9.1 Purpose

**Durability**: Write to WAL before mutation.

```rust
pub struct WalEntry {
    pub sequence: u64,
    pub operation: WalOp,
    pub timestamp: u64,
    pub checksum: u32,
}

pub enum WalOp {
    Put { cid: Cid, data: Vec<u8> },
    Delete { cid: Cid },
}
```

### 9.2 Recovery

On restart:
1. Read WAL entries
2. Replay operations
3. Apply to store

---

## 10. Integrity Checker

### 10.1 Verification

```rust
pub struct IntegrityChecker<S: BlockStore> {
    store: Arc<S>,
}

impl<S: BlockStore> IntegrityChecker<S> {
    pub async fn verify_block(&self, cid: &Cid) -> Result<IntegrityResult> {
        let block = self.store.get(cid).await?.ok_or(Error::NotFound)?;
        
        let computed = CidBuilder::new().build(block.data())?;
        
        if computed != *cid {
            Ok(IntegrityResult::CidMismatch { stored: *cid, computed })
        } else {
            Ok(IntegrityResult::Valid)
        }
    }
    
    pub async fn verify_all(&self) -> Result<Vec<IntegrityResult>>;
}
```

### 10.2 Handling Corruption

**Only deletion** — corrupted blocks cannot be "fixed".

```rust
match self.verify_block(cid).await? {
    IntegrityResult::CidMismatch { .. } => {
        self.store.delete(cid).await?;
        // Log corruption event
    }
    _ => {}
}
```

---

## 11. Invariants

| Invariant | Enforcement |
|-----------|-------------|
| Blocks are immutable | No update operation |
| Pin protection | GC mark from pin roots |
| WAL durability | Write before mutation |
| Content integrity | `verify_block()` |
| Quota limits | Soft = alert, hard = reject |
| TTL expiration | Decorator checks on get |

---

## 12. Performance Characteristics

### Latency

| Operation | P50 | P99 | Notes |
|-----------|-----|-----|-------|
| `get` (cache hit) | 30µs | 50µs | LRU cache |
| `get` (cache miss) | 500µs | 2ms | Sled read |
| `put` | 50µs | 80µs | Write-through |
| `put` (with dedup) | 100µs | 500µs | Chunk index |
| `delete` | 200µs | 1ms | Tombstone |
| GC sweep | 100ms | 1s | Per 10k blocks |

### Memory

| Component | Memory |
|-----------|--------|
| Sled (1M blocks) | ~2 GB |
| Cache (10k entries) | ~200 MB |
| Dedup index | ~50 MB |
| Pin map | ~10 MB |

---

## 13. Scalability

### 13.1 Sharding

```rust
// storage/blockstore_sharding.rs
pub struct ShardedBlockStore {
    shards: Vec<Arc<dyn BlockStore>>,
    hash: fn(&Cid) -> usize,
}

impl ShardedBlockStore {
    fn shard(&self, cid: &Cid) -> &Arc<dyn BlockStore> {
        let idx = (self.hash)(cid) % self.shards.len();
        &self.shards[idx]
    }
}
```

**Reduces lock contention** — each shard has independent locks.

---

## 14. Key Files

| File | Lines | Purpose |
|------|-------|---------|
| `traits.rs` | 100+ | BlockStore trait |
| `blockstore.rs` | 400+ | SledBlockStore |
| `paritydb.rs` | 300+ | ParityDB adapter |
| `s3.rs` | 350+ | S3 adapter |
| `cache.rs` | 250+ | CachedBlockStore |
| `dedup.rs` | 300+ | DedupBlockStore |
| `quota.rs` | 200+ | QuotaBlockStore |
| `pinning.rs` | 350+ | PinManager |
| `gc.rs` | 400+ | GarbageCollector |
| `tiering.rs` | 300+ | TieredStore |
| `replication.rs` | 350+ | Replicator |
| `wal.rs` | 250+ | Write-ahead log |
| `integrity_checker.rs` | 200+ | CID verification |

---

## 15. Design Decisions

### 15.1 Why Decorator Stack?

**Decision**: Cross-cutting concerns as stacked BlockStores.

**Rationale**:
- Composable, testable
- Enable/disable independently
- Follow Open/Closed Principle

**Trade-off**: Per-op indirection; deep stacks harder to debug.

---

### 15.2 Why Write-Ahead Log?

**Decision**: WAL for durability, not event sourcing.

**Rationale**:
- Crash recovery without full rebuild
- High throughput for storage operations

**Trade-off**: No audit trail by default.

---

### 15.3 Why Mark-Sweep GC?

**Decision**: Mark-sweep from pin roots.

**Rationale**:
- Simple, correct
- Works with any backend

**Trade-off**: Serial mark-sweep; no cross-shard parallelism.

---

## 16. Context Integration

```
┌─────────────────────────────────────────────────────────────────────┐
│                    STORAGE INTEGRATION                               │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  Consumed by (Conformist):                                          │
│    • Transport — BitswapExchange<S: BlockStore>                     │
│    • Logic — TensorLogicStore<S: BlockStore>                        │
│    • Semantic — VectorIndex stores embeddings as blocks            │
│    • Application — Node orchestrator                                │
│                                                                      │
│  Publishes:                                                          │
│    • StorageEvent — Observability (not sourcing)                    │
│                                                                      │
│  Shared Kernel usage:                                               │
│    • Block, Cid, Error, Result                                      │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

---

**Next**: [04-NetworkContext.md](04-NetworkContext.md) — libp2p, DHT, reputation graph
