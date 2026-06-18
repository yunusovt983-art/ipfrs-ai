# Storage Domain: Хранилище блоков и CID

**Краткое резюме**: Storage Domain управляет хранением неизменяемых блоков. Каждый блок идентифицируется криптографическим хешем (CID). Инвариант: `hash(data) == cid` **всегда** верен.

---

## Язык домена

| Термин | Значение |
|--------|----------|
| **Block** | Неизменяемая единица данных (≤2 МiB) |
| **CID** | Content Identifier = hash(data) |
| **DAG** | Направленный граф блоков с ссылками |
| **IPLD** | Формат сериализации (CBOR, JSON) с детерминизмом |
| **Sled** | Встроенная B+ tree база данных |
| **LRU Cache** | Кеш горячих блоков в памяти |

---

## Агрегат: Block

**Source**: `crates/ipfrs-core/src/block.rs:57–63`

### Структура

```rust
// core/block.rs:57–63
pub struct Block { 
    cid: Cid,                    // Content Identifier (hash)
    data: Bytes                  // Неизменяемые данные (both private, no mutation API)
}

pub const MAX_BLOCK_SIZE: usize = 2 * 1024 * 1024;  // block.rs:37
pub const MIN_BLOCK_SIZE: usize = 1;                // block.rs:40

pub fn new(data: Bytes) -> Result<Self> {           // block.rs:70–74
    Self::validate_size(data.len())?;               // INVARIANT 1
    let cid = CidBuilder::new().build(&data)?;      // INVARIANT 2 (CID = H(data))
    Ok(Self { cid, data })
}

pub fn verify(&self) -> Result<bool> {              // block.rs:117–120
    Ok(CidBuilder::new().build(&self.data)? == self.cid)
}
```

### Инварианты

```
1. hash(block.data) == block.cid      (ВСЕГДА)
2. block.data NOT MUTATED after creation
3. block.size ≤ 2 MiB
4. CID computed with SHA256, SHA3, Blake3 (configurable)
```

### Жизненный цикл

```
User adds data
    ↓
Block::new(bytes) → computes CID
    ↓
Storage.put(&block) → verify hash, persist to Sled, cache in LRU
    ↓
User queries Storage.get(&cid) → verify hash(retrieved) == cid
    ↓
If unpinned + old → Garbage Collector deletes
```

---

## Published Port: BlockStore Trait

**Source**: `crates/ipfrs-storage/src/traits.rs`

```rust
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

**Реализации** (adapters):
- `SledBlockStore` — default, embedded B+ tree (blockstore.rs)
- `ParityDbBlockStore` — SSD-optimized, blockchain-tuned (paritydb.rs)
- `S3BlockStore` — cloud storage with multipart + semaphore (s3.rs)
- `MemoryBlockStore` — for testing (memory.rs)
- `StorageObjectStore` — versioned objects (object_store.rs)

---

## Паттерн: Stacked Decorators

Каждый уровень — отдельная забота:

```
┌─────────────────────────────────────┐
│ User Code (Application Layer)       │
└──────────────┬──────────────────────┘
               │ (calls)
┌──────────────▼──────────────────────────┐
│ Decorator 1: Corruption Repair          │
│  • Verify CID on every read             │
│  • Detect corrupted blocks              │
│  • Auto-repair from replication         │
└──────────────┬──────────────────────────┘
               │ (delegates)
┌──────────────▼──────────────────────────┐
│ Decorator 2: LRU Cache Layer            │
│  • Hot-path optimization (99% hits)     │
│  • Configurable cache size (default 2GB)│
│  • Async-safe concurrent access         │
└──────────────┬──────────────────────────┘
               │ (delegates)
┌──────────────▼──────────────────────────┐
│ Decorator 3: Tiering / Hot-Cold         │
│  • Frequent → in-memory/SSD             │
│  • Old/cold → slower storage            │
│  • Transparent to upper layers          │
└──────────────┬──────────────────────────┘
               │ (delegates)
┌──────────────▼──────────────────────────┐
│ Implementation: SledBlockStore          │
│  • Embedded B+ tree database            │
│  • ACID transactions                    │
│  • RocksDB-style performance            │
└─────────────────────────────────────────┘
```

**Преимущества**:
- Каждый level независимо тестируем
- Easy to swap implementations
- Open-Closed Principle

---

## Domain Services

### CID Computation

```rust
pub fn compute_cid(data: &[u8], hash_algo: HashAlgorithm) -> Cid {
    let hash = match hash_algo {
        Sha256 => sha256(data),
        Sha512 => sha512(data),
        Blake3 => blake3(data),
        Sha3_256 => sha3_256(data),
    };
    
    Cid::new(CidVersion::V1, Codec::Raw, hash)
}

// Детерминизм: одни данные → один CID (всегда)
// Различные алгоритмы → разные CID (можно использовать оба)
```

### LRU Cache Management

```rust
pub struct LRUCache {
    cache: Arc<DashMap<Cid, Arc<Block>>>,
    max_size: u64,           // bytes
    current_size: AtomicU64,
    access_order: VecDeque<Cid>,
}

impl LRUCache {
    pub async fn get(&self, cid: &Cid) -> Option<Arc<Block>> {
        // O(1) lookup, update access order
        // Hit rate: ~99% for repeated access patterns
    }
    
    pub async fn put(&self, block: Arc<Block>) -> Result<()> {
        // Evict LRU if full
        // Move accessed to end
    }
}
```

### Garbage Collection

**Trigger**: Unpinned + created > TTL (default: 7 days)

```rust
pub async fn gc_collect(&self) -> Result<GCStats> {
    let all_cids = storage.all_cids().await?;
    let mut deleted_count = 0;
    
    for cid in all_cids {
        let block = storage.get(&cid).await?;
        if !block.is_pinned() && is_old(&block) {
            storage.delete(&cid).await?;
            deleted_count += 1;
        }
    }
    
    Ok(GCStats { deleted_count })
}
```

---

## Metrics & Performance

| Operation | Latency | Throughput | Notes |
|-----------|---------|-----------|-------|
| Block PUT | ~50µs | 20k ops/s | SSD I/O limited |
| Block GET (cache hit) | ~30µs | 33k ops/s | L3 cache hit |
| Block GET (SSD hit) | ~100µs | 10k ops/s | One SSD round-trip |
| Block verification | <1µs | 1M+ ops/s | SIMD hash |
| GC scan (1TB) | ~30s | - | Background task |

**Memory for 1TB**:
- Sled DB: ~1.2 GB (on-disk index)
- LRU Cache: ~2.0 GB (hot blocks)
- Metadata: ~0.2 GB
- Total: ~3.4 GB

---

## Взаимодействие с другими доменами

### Storage → Network
```
Emit event: BlockAdded(cid)
Network subscribes → announces in DHT
```

### Storage → Semantic
```
On block_added:
  Extract text if applicable
  Encode embedding
  Index in HNSW
```

### Storage ← Transport
```
Transport requests: storage.get(&cid)
Storage verifies, returns
```

---

## Важные свойства

| Свойство | Значение |
|----------|----------|
| **Immutability** | Блоки не меняются после создания |
| **Content Addressing** | Идентичность определена содержимым |
| **Replication** | Несколько копий одного блока имеют один CID |
| **Deduplication** | Идентичные данные → один блок |
| **Fault Detection** | hash(retrieved) != cid → обнаружена коррупция |

---

## Что дальше?

→ [03-Bounded Contexts](03-BoundedContexts.md) для обзора всех доменов  
→ [09-Data Flows](09-DataFlows.md) для примера: "Как добавляется файл?"  
→ `/Volumes/Kingston/cool-japan/Vendor/ipfrs/crates/ipfrs-storage/` для кода

---

**Связанные**: [02-Architecture Stack](02-ArchitectureStack.md) | [03-Bounded Contexts](03-BoundedContexts.md) | [09-Data Flows](09-DataFlows.md)
