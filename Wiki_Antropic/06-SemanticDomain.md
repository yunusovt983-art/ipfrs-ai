---
title: 06-SemanticDomain
type: domain
summary: Поиск по смыслу — HNSW VectorIndex, k-NN, квантование, reranking
tags: [ipfrs, semantic, ddd, hnsw, embeddings]
source: crates/ipfrs-semantic/src/
related: ["[[03-BoundedContexts]]", "[[07-LogicDomain]]", "[[09-DataFlows]]"]
read_time: 40 мин
updated: 2026-06-18
---

# Semantic Domain: Поиск по смыслу через HNSW

**Краткое резюме**: Semantic Domain отвечает на вопрос "Что это означает?" с помощью векторной индексации. Используется HNSW (Hierarchical Navigable Small World) для быстрого поиска похожих документов.

---

## Язык домена

| Термин | Значение |
|--------|----------|
| **Embedding** | Вектор, представляющий смысл (768-dim) |
| **HNSW** | Иерархический индекс для k-NN поиска |
| **Vector Distance** | Косинусная, L2, или Jaccard метрика |
| **Query Cache** | Кеш результатов поиска (~85% hit rate) |
| **ML Model** | Encoder (e.g., BERT, embeddings) |

---

## Domain Model: VectorIndex (HNSW)

**Source**: `crates/ipfrs-semantic/src/hnsw.rs`

### Структура

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

### HNSW структура & параметры

Auto-tuned по размеру (hnsw.rs:442):
- `<10k → (M=16, ef_c=200)`
- `<100k → (32,400)`
- else `(48,600)`

**Инварианты**:
1. Dimension consistency (reject mismatched vectors)
2. CID uniqueness
3. No NaN/Inf (normalize guards `norm>0`)
4. Cosine score ∈ [0,1] / L2 ∈ [0,∞)
5. Monotonic internal ids, `created_at ≤ updated_at`

---

## Алгоритм: Поиск k-NN

```rust
pub fn search(&self, query: Vec<f32>, k: usize) -> Vec<(Cid, f32)> {
    // HNSW paper algorithm
    
    // 1. Start at top layer entry point
    let mut nearest = vec![self.entry_point];
    
    // 2. Layer-by-layer descent
    for layer in self.layers.iter().rev() {  // Top to bottom
        // Expand search radius on this layer
        nearest = self.expand_search(&nearest, &query, self.ef_search, layer);
    }
    
    // 3. Return top-k closest vectors
    return nearest
        .iter()
        .map(|idx| (self.cids[idx].clone(), self.distance(&query, &self.vectors[idx])))
        .sorted_by(|a, b| b.1.partial_cmp(&a.1))  // High to low
        .take(k)
        .collect()
}

// Time complexity: O(log N) on average for N vectors
// For 100k vectors: typically 1-10ms search
```

### Пример: Ищем "машинное обучение"

```
Input query: "machine learning"
    ↓
ML Model: embed("machine learning") → [0.14, -0.09, 0.23, ...]
    ↓
HNSW.search([0.14, -0.09, ...], k=10)
    ↓
Layer 0 (top, sparse):     Check ~50 nodes
Layer 1:                   Check ~150 nodes
Layer 2:                   Check ~500 nodes
...
Layer N (bottom, dense):   Check ~10k nodes
    ↓
Converge on 10 closest
    ↓
Results:
  1. "Deep Learning Fundamentals" (similarity: 0.95)
  2. "Neural Networks Explained" (similarity: 0.92)
  3. "Transformers Architecture" (similarity: 0.88)
  ...
```

---

## Distance Metrics

### Cosine Similarity (Default)

```
similarity(A, B) = (A · B) / (|A| * |B|)
Range: [-1, 1] (usually [0, 1] for normalized embeddings)
Properties: Invariant to magnitude, good for text
```

### L2 (Euclidean Distance)

```
distance(A, B) = sqrt(Σ(A_i - B_i)²)
Range: [0, ∞]
Properties: Exact geometric distance
```

### Jaccard Distance (Sets)

```
distance(A, B) = 1 - |A ∩ B| / |A ∪ B|
Range: [0, 1]
Properties: For sparse categorical data
```

---

## Domain Services

**Source**: `crates/ipfrs-semantic/src/{embedding_pipeline,vector_quantizer,query_planner,search_pipeline,reranking,simd}.rs`

### Основные сервисы

- **`EmbeddingPipeline`** (embedding_pipeline.rs): `EmbeddingInput{RawBytes|Text|Structured|Embedding}` → normalized `Vec<f32>`
  - Normalization: `None|L2|MinMax|ZScore`

- **`VectorQuantizer`** (vector_quantizer.rs): Product Quantization
  - Split D into M subspaces, k-means K≤256 centroids each
  - Encode → M bytes, `asymmetric_distance` for query-to-code

- **`NearestNeighborQueryPlanner`** (query_planner.rs):
  - `ExecutionStrategy{LocalOnly|RemoteFanout|Hybrid|Cached}`
  - Фильтрует shards по latency budget, prefer-local

- **`SemanticSearchPipeline`** (search_pipeline.rs): vector + BM25 → fusion (`RRF|LinearCombination|CombSUM`)

- **`ReRanker`** (reranking.rs): `WeightedCombination|ReciprocalRankFusion|LearnToRank`

- **Distribution**: `ShardCoordinator` (consistent-hash ring, FNV-1a, 150 virtual nodes/shard)

- **SIMD** (simd.rs): runtime NEON/AVX dispatch для `l2/dot/cosine`

### Узкие места

HNSW память ≈ `n·(dim·4 + M·8)` (768-d, M=16, 10M ≈ 30 GB)
- Mitigation: PQ (≈12,000× compression, ~0.5–1% recall loss) или DiskANN
- Classic recall↔latency knobs: M / ef_construction / ef_search

---

## Query Cache

```rust
pub struct QueryCache {
    cache: Arc<DashMap<EmbeddingHash, Vec<SearchResult>>>,
    config: CacheConfig,
}

impl QueryCache {
    pub async fn get(&self, embedding_hash: EmbeddingHash) 
        -> Option<Vec<SearchResult>> {
        // O(1) lookup
        // Hit rate: ~85% for typical workloads
        self.cache.get(&embedding_hash).map(|r| r.clone())
    }
    
    pub async fn put(&self, embedding_hash: EmbeddingHash, 
                     results: Vec<SearchResult>) {
        self.cache.insert(embedding_hash, results);
        // Auto-expire old entries (TTL-based)
    }
}

// Why not 100% hit rate?
// - Model retraining → different embeddings
// - Index updates → slightly different results
// - User typos → similar but different queries
```

---

## Insert Algorithm

```rust
pub fn insert(&mut self, cid: Cid, vector: Vec<f32>) -> Result<()> {
    // 1. Assign to random layer (exponential probability)
    let layer = self.assign_layer();
    
    // 2. Expand layers if needed
    if layer > self.max_layer {
        self.max_layer = layer;
    }
    
    // 3. Find nearest neighbors at each layer
    let mut nearest = self.entry_point;
    for L in (layer..=max_layer).rev() {
        nearest = self.search_layer(&vector, vec![nearest], ef_construction, L);
    }
    
    // 4. Add to layers and create bidirectional links
    for L in (0..=layer).rev() {
        let M = if L == 0 { 2 * max_connections } else { max_connections };
        let neighbors = self.find_neighbors(&vector, nearest, ef_construction, M);
        
        // Add edges: this node ← neighbors
        for neighbor_id in &neighbors {
            self.add_edge(neighbor_id, self.new_id, L);
        }
        
        // Prune neighbors if needed (keep M closest)
        for neighbor_id in &neighbors {
            self.connections[neighbor_id][L].truncate(M);
        }
    }
    
    self.cids.push(cid);
    self.vectors.push(vector);
    Ok(())
}

// Time complexity: O(log N * connections)
// For 100k vectors: ~100µs per insert
```

---

## Metrics & Performance

| Operation | Latency | Notes |
|-----------|---------|-------|
| Search (cache hit) | ~1µs | Memory lookup only |
| Search (cache miss) | 1-10ms | HNSW traversal |
| Insert | ~100µs | Add to layers, link neighbors |
| Rebuild | ~5min | For 1M vectors (background) |
| Memory per vector | ~3KB | 768-dim + metadata |

**Recall vs Speed Trade-off**:
```
ef_search = 50  → 1-5ms search, ~98% recall
ef_search = 200 → 5-10ms search, ~99% recall
ef_search = 500 → 10-20ms search, ~99.5% recall
```

---

## Batch Indexing

```rust
pub async fn index_batch(&self, blocks: Vec<(Cid, Bytes)>) -> Result<()> {
    // 1. Extract text from blocks (parallel)
    let texts = blocks.par_iter()
        .map(|(_, bytes)| extract_text(bytes))
        .collect::<Vec<_>>();
    
    // 2. Encode all embeddings (parallel batching)
    let embeddings = ml_model.encode_batch(&texts)?;
    
    // 3. Insert into HNSW (sequential for safety)
    for (cid, embedding) in blocks.iter().zip(embeddings) {
        hnsw.insert(cid, embedding)?;
    }
    
    // 4. Invalidate query cache (results may have changed)
    query_cache.clear();
    
    Ok(())
}

// For 100 blocks: ~500ms (100ms ML + 20ms HNSW + 10ms cache clear)
```

---

## Взаимодействие с другими доменами

### Semantic ← Storage
```
On BlockAdded event:
  1. Extract text from block
  2. Encode to embedding
  3. Insert into HNSW
```

### Semantic → Storage
```
On search query:
  1. HNSW returns [CID1, CID2, ...]
  2. Storage.get(CID) for each
  3. Return with metadata
```

### Semantic ← Logic (Future)
```
On logic inference:
  If no symbolic solutions:
    Embed predicate
    Query semantic index as fallback
    Return similar facts
```

---

## Важные свойства

| Свойство | Значение |
|----------|----------|
| **Approximate** | ~99% recall vs exact k-NN |
| **Efficient** | O(log N) complexity |
| **Incremental** | Can insert vectors online |
| **Configurable** | Multiple distance metrics |
| **Scalable** | 1M+ vectors on single node |

---

## Типичные use cases

1. **Document Search**: "Find papers similar to this"
2. **Recommendation**: "What else might user like?"
3. **Clustering**: "Group similar blocks"
4. **Anomaly Detection**: "Is this embedding an outlier?"
5. **Fallback for Logic**: "If rules fail, search semantically"

---

## Что дальше?

→ [03-Bounded Contexts](03-BoundedContexts.md) для обзора  
→ [09-Data Flows](09-DataFlows.md) для сценария "Semantic search"  
→ `/Volumes/Kingston/cool-japan/Vendor/ipfrs/crates/ipfrs-semantic/` для кода

---

**Связанные**: [02-Architecture Stack](02-ArchitectureStack.md) | [03-Bounded Contexts](03-BoundedContexts.md) | [09-Data Flows](09-DataFlows.md) | [07-LogicDomain](07-LogicDomain.md)
