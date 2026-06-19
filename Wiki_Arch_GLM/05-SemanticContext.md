# Semantic Context — HNSW, DiskANN, Quantization

> **Focus**: Approximate Nearest Neighbor search, embedding pipeline  
> **Source**: `ipfrs_source/crates/ipfrs-semantic/src/` (159 files)

---

## 1. Context Overview

Semantic Context отвечает за **vector similarity search** и **embedding pipeline**.

```
┌─────────────────────────────────────────────────────────────────────┐
│                    SEMANTIC CONTEXT                                 │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    INDEX AGGREGATES                          │   │
│  │  ─────────────────────────────────────────────────────────── │   │
│  │  VectorIndex (HNSW)    — In-memory, <10M vectors             │   │
│  │  DiskANNIndex          — Disk-based, billion-scale           │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    VALUE OBJECTS                             │   │
│  │  ─────────────────────────────────────────────────────────── │   │
│  │  SearchResult { cid, score }                                 │   │
│  │  DistanceMetric — L2/Cosine/DotProduct                       │   │
│  │  QuantizerCode — Compressed vector                           │   │
│  │  Codebook — PQ centroids                                     │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    DOMAIN SERVICES                           │   │
│  │  ─────────────────────────────────────────────────────────── │   │
│  │  EmbeddingPipeline   — Input → normalized vector             │   │
│  │  VectorQuantizer     — Product Quantization                  │   │
│  │  QueryPlanner        — Execution strategy                    │   │
│  │  ReRanker            — Fusion + reranking                    │   │
│  │  ShardCoordinator    — Consistent-hash sharding              │   │
│  │  SemanticRouter      — Cross-shard routing                   │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    SIMD LAYER                                │   │
│  │  ─────────────────────────────────────────────────────────── │   │
│  │  Runtime detection: AVX2 / NEON / Scalar                     │   │
│  │  Vectorized: l2, dot, cosine                                 │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 1bis. Глубокое погружение по коду (выверено 2026-06-19)

> Точные `file:line`-якоря и **исправление расхождений** между идеализированной моделью
> и реальным кодом (19 пунктов, проверено по исходникам). Подсекции 2–13 ниже остаются
> как концептуальное описание; здесь — выверенные факты.

### 1bis.1 VectorIndex (HNSW) — ключевые факты

```rust
// hnsw.rs:84 — реальные поля
pub struct VectorIndex {
    index: Arc<RwLock<Hnsw<'static, f32, DistL2>>>,   // ВСЕГДА DistL2 (hnsw.rs:86)
    id_to_cid, cid_to_id, vectors: HashMap<Cid, Vec<f32>>,  // оригиналы хранятся
    dimension, metric, tracker,
}
```
- ⚠️ **Алгоритм HNSW НЕ реализован в репозитории** — делегирован крейту **`hnsw_rs`**
  (`hnsw.rs:6`); бэкенд жёстко на `DistL2`. Cosine/DotProduct **эмулируются** нормализацией
  входа + пересчётом результата.
- **`convert_distance`** (`hnsw.rs:399`), реальные формулы: `L2 → distance`;
  `Cosine → 1 - distance²/2`; `DotProduct → -distance`. (В прежнем тексте формулы были иные.)
- Нормализация (`hnsw.rs:379`): только **Cosine** нормирует к единичной длине; **DotProduct
  НЕ нормализуется** (вопреки распространённому ожиданию).
- **insert**: повторная вставка существующего CID → **ошибка** `Error::InvalidInput`
  (`hnsw.rs:168`), НЕ no-op.
- **delete** (`hnsw.rs:275`): **soft-delete** — узел остаётся в графе `hnsw_rs`, чистятся
  только маппинги (накапливается фрагментация).
- ⚠️ **`rebuild` (`hnsw.rs:570`) — БАГ**: ставит пустой `Hnsw`, **не переинициализирует
  векторы** (`vectors_reinserted: 0`, `hnsw.rs:615`) → граф опустошается, поиск перестаёт
  находить, хотя `len()` (по `cid_to_id`) врёт.
- ⚠️ **snapshot/from_snapshot не сохраняют топологию** HNSW (`layer_connections: Vec::new()`,
  `max_layer: 0`, `hnsw.rs:791,801`); `from_snapshot` пересобирает граф пере-вставкой.
- `with_defaults` (`hnsw.rs:149`): M=16, ef_construction=200, **метрика L2** (не cosine).

### 1bis.2 DiskANNIndex — ключевые факты (`diskann.rs`)

```rust
pub struct DiskANNConfig { dimension:768, max_degree:64(R), queue_size:100(L), alpha:1.2, num_entry_points:4 } // :22
```
- Vamana реализован **сам** (`vamana_insert:593`, `robust_prune:631`). Условие отбраковки
  (`diskann.rs:670`): кандидат отбрасывается при `alpha * dist(cand, selected) < dist(cand, node)`.
- ⚠️ **Хранит raw f32 через mmap, БЕЗ PQ и кодбука**; adjacency-граф `Vec<Vec<usize>>` —
  **в RAM** (`diskann.rs:201`). «Константная память» — лишь частично (векторы пагинируются,
  граф нет).
- Расстояние — **свой скалярный `l2_distance`** (`diskann.rs:689`), **без SIMD**.
- ⚠️ **`compact` (`diskann.rs:936`) — no-op** (`bytes_saved: 0`); зато `prune_graph:965` реален.

### 1bis.3 Value Objects — корректировки

- **`DistanceMetric`** (`hnsw.rs:15`) = `{ L2, Cosine, DotProduct }`. ⚠️ **`Jaccard` отсутствует**
  (он только в несвязанном `text_similarity_scorer::SimilarityMetric`).
- ⚠️ **Product Quantization есть, но НЕ подключён к индексам.** Две независимые реализации:
  `vector_quantizer::VectorQuantizer` (f64, `Codebook`/`QuantizerCode`, `num_subspaces=8`,
  `codes_per_subspace=255`) и `quantization::ProductQuantizer` (f32, `num_centroids=1<<bits`).
  Обе **standalone**. В индексный путь встроен только **скалярный INT8** (`router.rs`), не PQ.

### 1bis.4 Domain Services — корректировки

| Сервис (в обзоре) | Реальность | Источник |
|-------------------|------------|----------|
| EmbeddingPipeline | FNV-фолд только для `RawBytes`; `Text`/`Structured` → Unicode code-point `/0x10FFFF`; `Embedding` passthrough. Это **детерминированные хеши, не ML** | `embedding_pipeline.rs:40,212` |
| VectorQuantizer | PQ standalone (см. 1bis.3) | `vector_quantizer.rs:209` |
| QueryPlanner | ⚠️ реальное имя **`NearestNeighborQueryPlanner`**; только планирует (поиск не исполняет); `ExecutionStrategy::Cached` не порождается | `query_planner.rs:111,131` |
| ReRanker | `WeightedCombination` + `ReciprocalRankFusion` — реальны; ⚠️ **`LearnToRank` и `Custom` — заглушки** | `reranking.rs:14,103` |
| ShardCoordinator | реальное consistent hashing, `virtual_nodes` деф. **150** | `shard_coordinator.rs:164,212` |
| SemanticRouter | `IndexHandle{Hnsw\|DiskAnn}` + LRU query-кэш; consistent hashing **сам не использует**; DiskAnn-бэкенд не поддерживает `remove`/`contains` | `router.rs:39,347` |

### 1bis.5 SIMD и Semantic DHT — корректировки

- **SIMD** (`simd.rs:24,53,82`): runtime-детект NEON / SSE/AVX/AVX2 для `l2/dot/cosine`.
  ⚠️ **AVX-512 упомянут в комментарии, но НЕ реализован.** ⚠️ SIMD **не используется ни HNSW**
  (там `hnsw_rs`), **ни DiskANN** (свой скаляр) — только DHT и аналитические модули.
- ⚠️ **Semantic DHT — распределённый поиск это заглушка**: `replicate_to_peer` (`dht_node.rs:409`)
  → no-op, `query_peer` (`:430`) → всегда `None` → `search_distributed` вырождается в локальный.
  Векторная маршрутизация (`SemanticRoutingTable`, greedy `find_nearest_peers`) и локальный
  поиск — реальны; межузловой обмен — нет. Полный реестр заглушек: `[[../Wiki/11-RealityCheck]]`.

---

## 2. HNSW — Hierarchical Navigable Small World

### 2.1 Structure

```rust
pub struct VectorIndex {
    index: Arc<RwLock<Hnsw<'static, f32, DistL2>>>,
    
    // CID ↔ internal-ID mappings
    id_to_cid: Arc<RwLock<HashMap<usize, Cid>>>,
    cid_to_id: Arc<RwLock<HashMap<Cid, usize>>>,
    
    // Original vectors (for rebuild)
    vectors: Arc<RwLock<HashMap<Cid, Vec<f32>>>>,
    
    next_id: Arc<RwLock<usize>>,
    dimension: usize,
    metric: DistanceMetric,
    tracker: Arc<RwLock<IncrementalTracker>>,
}
```

### 2.2 HNSW Parameters

Auto-tuned by collection size:

| Size | M | ef_construction | ef_search |
|------|---|-----------------|-----------|
| < 10k | 16 | 200 | 50 |
| < 100k | 32 | 400 | 100 |
| ≥ 100k | 48 | 600 | 200 |

### 2.3 Layer Assignment

```
layer = -ln(U(0,1)) / ln(2)

Probability of layer l:
  P(layer = l) = 1/2^(l+1)

Layer 0: ~50% of nodes
Layer 1: ~25% of nodes
Layer 2: ~12.5% of nodes
...
```

### 2.4 Operations

```rust
impl VectorIndex {
    pub async fn insert(&self, cid: &Cid, vector: Vec<f32>) -> Result<()> {
        // 1. Validate dimension
        if vector.len() != self.dimension {
            return Err(Error::DimensionMismatch);
        }
        
        // 2. Dedup
        if self.cid_to_id.read().contains_key(cid) {
            return Ok(());
        }
        
        // 3. Normalize per metric
        let normalized = self.normalize(&vector);
        
        // 4. Insert into HNSW
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        self.index.write().insert(&normalized, id);
        
        // 5. Update mappings
        self.id_to_cid.write().insert(id, cid.clone());
        self.cid_to_id.write().insert(cid.clone(), id);
        self.vectors.write().insert(cid.clone(), vector);
        
        // 6. Mark dirty
        self.tracker.write().mark_dirty(cid);
        
        Ok(())
    }
    
    pub async fn search(&self, query: &[f32], k: usize) -> Result<Vec<SearchResult>> {
        let normalized = self.normalize(query);
        
        let results = self.index.read()
            .search(&normalized, k, self.ef_search());
        
        Ok(results.into_iter()
            .filter_map(|(id, distance)| {
                let cid = self.id_to_cid.read().get(&id)?.clone();
                let score = self.convert_distance(distance);
                Some(SearchResult { cid, score, distance })
            })
            .collect())
    }
    
    pub async fn delete(&self, cid: &Cid) -> Result<()> {
        // Soft delete: unmap CID, node stays in graph
        if let Some(id) = self.cid_to_id.write().remove(cid) {
            self.id_to_cid.write().remove(&id);
            self.vectors.write().remove(cid);
        }
        Ok(())
    }
}
```

### 2.5 Score Normalization

```rust
fn convert_distance(&self, distance: f32) -> f64 {
    match self.metric {
        DistanceMetric::Cosine => {
            // Cosine ∈ [0, 2] → score ∈ [0, 1]
            (1.0 - distance as f64 / 2.0).max(0.0)
        }
        DistanceMetric::L2 => {
            // L2 ∈ [0, ∞) → score ∈ [0, 1]
            1.0 / (1.0 + distance as f64)
        }
        DistanceMetric::DotProduct => {
            // Dot ∈ [-1, 1] for normalized → score ∈ [0, 1]
            ((distance as f64) + 1.0) / 2.0
        }
    }
}
```

---

## 3. DiskANN — Billion-Scale Index

### 3.1 Structure

```rust
pub struct DiskANNIndex {
    // Memory-mapped Vamana graph
    graph: MmapMut,
    
    // Configuration
    r: usize,           // Max degree
    l: usize,           // Queue size for search
    alpha: f64,         // Approximation factor (~1.2)
    
    // Multiple entry points
    entry_points: Vec<usize>,
    
    // PQ-encoded vectors
    pq_codes: Mmap,
    codebook: Codebook,
}
```

### 3.2 Vamana Graph

**Properties**:
- Flat graph (not hierarchical)
- Each node has ≤ R neighbors
- Multiple entry points for robustness
- Constructed via greedy search

### 3.3 Search Algorithm

```
search(query):
  1. PQ-encode query (asymmetric distance)
  2. Start from entry points
  3. Greedy BFS with priority queue
  4. Visit at most L nodes
  5. Return top-k by distance
```

### 3.4 Trade-offs

| Aspect | HNSW | DiskANN |
|--------|------|---------|
| Memory | O(n × (dim + M)) | O(1) constant |
| Latency | Lower | Higher (page faults) |
| Scale | ~10M vectors | Billion+ |
| Recall | Higher | Slightly lower |

---

## 4. Product Quantization

### 4.1 Concept

Compress D-dimensional vector into M bytes:

```
Vector (D × 4 bytes)
    ↓ Split into M subvectors
Subvectors (D/M × 4 bytes each)
    ↓ Quantize each to nearest centroid
QuantizerCode (M bytes)

Compression: D×4 → M bytes
Example: 768×4 = 3072 → 32 bytes = 96×
```

### 4.2 VectorQuantizer

```rust
pub struct VectorQuantizer {
    codebook: Codebook,
    m: usize,           // Number of subspaces
    k: usize,           // Centroids per subspace (≤256)
}

pub struct Codebook {
    pub centroids: Vec<Vec<f32>>,   // M × K centroids
    pub subspace_dim: usize,
}

impl VectorQuantizer {
    pub fn encode(&self, vector: &[f32]) -> QuantizerCode {
        let mut code = Vec::with_capacity(self.m);
        
        for i in 0..self.m {
            let start = i * self.codebook.subspace_dim;
            let end = start + self.codebook.subspace_dim;
            let subvector = &vector[start..end];
            
            // Find nearest centroid
            let nearest = self.find_nearest_centroid(i, subvector);
            code.push(nearest as u8);
        }
        
        QuantizerCode(code)
    }
    
    pub fn asymmetric_distance(&self, query: &[f32], code: &QuantizerCode) -> f64 {
        let mut distance = 0.0;
        
        for (i, &byte) in code.0.iter().enumerate() {
            let centroid_idx = i * self.k + byte as usize;
            let centroid = &self.codebook.centroids[centroid_idx];
            
            let start = i * self.codebook.subspace_dim;
            let end = start + self.codebook.subspace_dim;
            let subquery = &query[start..end];
            
            distance += l2_squared(subquery, centroid);
        }
        
        distance.sqrt()
    }
}
```

### 4.3 Compression Ratio

```
Original: D × 4 bytes
PQ code:  M bytes
Codebook: M × K × (D/M) × 4 = D × K × 4 bytes (shared)

Effective compression (for n vectors):
  n × D × 4 → n × M + D × K × 4
  
For large n:
  Ratio ≈ D × 4 / M
  
Example: D=768, M=32
  Ratio ≈ 768 × 4 / 32 = 96×
  
With codebook sharing across indices:
  Up to 12,000×
```

---

## 5. Embedding Pipeline

### 5.1 Input Types

```rust
pub enum EmbeddingInput {
    RawBytes(Vec<u8>),              // Pass-through
    Text(String),                   // Text → embedding model
    Structured(serde_json::Value),  // JSON → structured embedding
    Embedding(Vec<f32>),            // Pre-computed
}
```

### 5.2 Normalization

```rust
pub enum Normalization {
    None,
    L2,          // x / ||x||₂
    MinMax,      // (x - min) / (max - min)
    ZScore,      // (x - μ) / σ
}

impl EmbeddingPipeline {
    pub fn normalize(&self, vector: &mut [f32]) -> Result<()> {
        match self.normalizer {
            Normalization::None => Ok(()),
            Normalization::L2 => {
                let norm = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
                if norm > 0.0 {
                    for x in vector.iter_mut() {
                        *x /= norm;
                    }
                }
                Ok(())
            }
            // ...
        }
    }
}
```

---

## 6. Query Planner

### 6.1 Execution Strategies

```rust
pub enum ExecutionStrategy {
    LocalOnly,
    RemoteFanout { max_peers: usize },
    Hybrid { local_ratio: f64 },
    Cached { ttl: Duration },
}
```

### 6.2 Planning Logic

```rust
impl NearestNeighborQueryPlanner {
    pub fn plan(&self, query: &Query) -> QueryPlan {
        // 1. Check cache
        if let Some(cached) = self.cache.get(&query.cache_key()) {
            return QueryPlan::Cached(cached);
        }
        
        // 2. Identify shards
        let shards = self.shard_coordinator.target_shards(&query);
        
        // 3. Filter by latency budget
        let shards = shards.into_iter()
            .filter(|s| s.latency < query.latency_budget)
            .collect();
        
        // 4. Select strategy
        let strategy = if shards.iter().all(|s| s.is_local) {
            ExecutionStrategy::LocalOnly
        } else if query.prefer_local {
            ExecutionStrategy::Hybrid { local_ratio: 0.7 }
        } else {
            ExecutionStrategy::RemoteFanout { max_peers: 10 }
        };
        
        QueryPlan { strategy, shards, .. }
    }
}
```

---

## 7. ReRanker

### 7.1 Fusion Methods

```rust
pub enum FusionMethod {
    WeightedCombination { weights: Vec<f64> },
    ReciprocalRankFusion { k: usize },   // RRF: Σ 1/(k + rank)
    CombSUM,
    LearnToRank { model: LtrModel },
}
```

### 7.2 Score Components

```rust
pub struct ScoreComponent {
    pub vector_similarity: f64,
    pub metadata_match: f64,
    pub recency: f64,
    pub popularity: f64,
    pub diversity: f64,
}

impl ReRanker {
    pub fn rerank(&self, results: Vec<SearchResult>, components: Vec<ScoreComponent>) -> Vec<SearchResult> {
        match self.method {
            FusionMethod::ReciprocalRankFusion { k } => {
                let mut scores = HashMap::new();
                for component in components {
                    for (rank, result) in results.iter().enumerate() {
                        *scores.entry(result.cid.clone()).or_insert(0.0) += 
                            1.0 / (k as f64 + rank as f64);
                    }
                }
                // Sort by RRF score
            }
            // ...
        }
    }
}
```

---

## 8. Shard Coordinator

### 8.1 Consistent Hashing

```rust
pub struct ShardCoordinator {
    ring: BTreeMap<u64, ShardId>,    // FNV-1a hash → shard
    virtual_nodes: usize,            // 150 per shard
    health: DashMap<ShardId, ShardHealth>,
}

impl ShardCoordinator {
    pub fn assign_shard(&self, cid: &Cid) -> ShardId {
        let hash = fnv1a_hash(cid.to_bytes());
        
        self.ring.range(hash..).next()
            .map(|(_, id)| *id)
            .unwrap_or_else(|| *self.ring.values().next().unwrap())
    }
}
```

### 8.2 Adaptive Partitioning

```rust
pub enum RebalanceAction {
    Split { shard: ShardId, new_shards: Vec<ShardId> },
    Merge { shards: Vec<ShardId>, into: ShardId },
    Migrate { from: ShardId, to: ShardId, cids: Vec<Cid> },
}

impl AdaptiveIndexPartitioner {
    pub fn check_rebalance(&self) -> Vec<RebalanceAction> {
        // Monitor shard size/latency
        // Emit rebalance actions if thresholds exceeded
    }
}
```

---

## 9. SIMD Optimization

### 9.1 Runtime Detection

```rust
pub enum SimdLevel {
    Scalar,
    Sse41,
    Avx2,
    Neon,
}

pub fn detect_simd_support() -> SimdLevel {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") { SimdLevel::Avx2 }
        else if is_x86_feature_detected!("sse4.1") { SimdLevel::Sse41 }
        else { SimdLevel::Scalar }
    }
    #[cfg(target_arch = "aarch64")]
    {
        if std::arch::is_aarch64_feature_detected!("neon") { SimdLevel::Neon }
        else { SimdLevel::Scalar }
    }
}
```

### 9.2 Vectorized Distance

```rust
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn l2_distance_avx2(a: &[f32], b: &[f32]) -> f32 {
    let mut sum = _mm256_setzero_ps();
    
    for i in (0..a.len()).step_by(8) {
        let va = _mm256_loadu_ps(a.as_ptr().add(i));
        let vb = _mm256_loadu_ps(b.as_ptr().add(i));
        let diff = _mm256_sub_ps(va, vb);
        sum = _mm256_fmadd_ps(diff, diff, sum);
    }
    
    // Horizontal sum
    let result = _mm256_extractf128_ps(sum, 0);
    let result = _mm_add_ps(result, _mm256_extractf128_ps(sum, 1));
    // ...
}
```

---

## 10. Performance Model

### 10.1 HNSW Memory

```
Memory ≈ n × (dim × 4 + M × 8) bytes

Example (10M vectors, 768-dim, M=16):
  = 10,000,000 × (768 × 4 + 16 × 8)
  = 10,000,000 × 3,200
  ≈ 30 GB
```

### 10.2 Latency

| Operation | P50 | P99 |
|-----------|-----|-----|
| HNSW search (k=10) | 1ms | 10ms |
| HNSW insert | 2ms | 20ms |
| DiskANN search | 5ms | 50ms |
| PQ encode | 0.1ms | 0.5ms |

---

## 11. Key Files

| File | Lines | Purpose |
|------|-------|---------|
| `hnsw.rs` | 600+ | HNSW index |
| `diskann.rs` | 500+ | DiskANN index |
| `vector_quantizer.rs` | 400+ | Product Quantization |
| `embedding_pipeline.rs` | 300+ | Normalization |
| `query_planner.rs` | 350+ | Execution planning |
| `reranking.rs` | 250+ | Fusion/reranking |
| `sharding.rs` | 400+ | Shard coordination |
| `simd.rs` | 200+ | AVX2/NEON |

---

## 12. Design Decisions

### 12.1 Why HNSW + DiskANN?

**Decision**: Two alternative index aggregates.

**Rationale**:
- HNSW: Best for <10M vectors, in-memory
- DiskANN: Best for billion+ vectors, disk-based
- Operator chooses based on scale

---

### 12.2 Why Soft Delete in HNSW?

**Decision**: Unmap CID, keep node in graph.

**Rationale**:
- True delete requires graph rewiring (O(M²))
- Breaks concurrent search
- Tombstones are rare in content-addressed systems

---

### 12.3 Why Product Quantization?

**Decision**: PQ over OPQ/ScaNN.

**Rationale**:
- PQ: Simple, effective, 12,000× compression
- OPQ: Requires learned rotation (complexity)
- ScaNN: TensorFlow dependency

---

## 13. Context Integration

```
┌─────────────────────────────────────────────────────────────────────┐
│                    SEMANTIC INTEGRATION                             │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Consumes (Shared Kernel):                                          │
│    • Cid (vector identity)                                          │
│    • TensorBlock (tensor storage)                                   │
│    • Error, Result                                                  │
│                                                                     │
│  Consumes (Customer/Supplier):                                      │
│    • Storage — BlockStore.get (embedding retrieval)                 │
│    • Network — SemanticDht (semantic routing)                       │
│                                                                     │
│  Publishes:                                                         │
│    • VectorAnnotatedRecord — for Network semantic DHT               │
│    • SearchResult — to Application                                  │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

**Next**: [06-LogicContext.md](06-LogicContext.md) — IR, inference, neural-symbolic
