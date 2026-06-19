# Performance Model — Bottlenecks, Scaling, Optimization

> **Focus**: Performance characteristics, bottlenecks, optimization strategies

---

## 1. Latency Distribution

### 1.1 Storage Operations

| Operation | P50 | P99 | Throughput |
|-----------|-----|-----|------------|
| GET (cache hit) | 30µs | 50µs | 33k ops/s |
| GET (cache miss) | 500µs | 2ms | 2k ops/s |
| PUT | 50µs | 80µs | 20k ops/s |
| PUT (dedup) | 100µs | 500µs | 10k ops/s |
| DELETE | 200µs | 1ms | 5k ops/s |

### 1.2 Network Operations

| Operation | P50 | P99 | Notes |
|-----------|-----|-----|-------|
| DHT lookup | 150ms | 300ms | Kademlia |
| Peer connect | 50ms | 200ms | QUIC handshake |
| Bitswap message | 10µs | 50µs | Encode/decode |

### 1.3 Semantic Operations

| Operation | P50 | P99 | Notes |
|-----------|-----|-----|-------|
| HNSW search (k=10) | 1ms | 10ms | 10M vectors |
| HNSW insert | 2ms | 20ms | |
| DiskANN search | 5ms | 50ms | Page faults |
| PQ encode | 0.1ms | 0.5ms | |

### 1.4 Logic Operations

| Operation | P50 | P99 | Notes |
|-----------|-----|-----|-------|
| Simple query | 1ms | 5ms | Local KB |
| Recursive (tabling) | 5ms | 50ms | |
| Distributed query | 100ms | 1000ms | Network latency |

### 1.5 Transport Operations

| Operation | P50 | P99 | Notes |
|-----------|-----|-----|-------|
| WantList push/pop | 1µs | 5µs | Heap |
| Full network fetch | 200ms | 1000ms | Depends on peers |

---

## 2. Memory Model

### 2.1 HNSW Memory

```
Memory ≈ n × (dim × 4 + M × 8) bytes

Example (10M vectors, 768-dim, M=16):
  = 10,000,000 × (768 × 4 + 16 × 8)
  = 10,000,000 × 3,200
  ≈ 30 GB
```

### 2.2 Component Memory

| Component | Memory |
|-----------|--------|
| Node (minimal) | ~50 MB |
| + Semantic (100k vectors) | ~500 MB |
| + Logic (10k rules) | ~100 MB |
| PeerStore (10k peers) | ~50 MB |
| Trust graph (100k edges) | ~20 MB |
| Event replay buffer | ~100 MB |

### 2.3 PQ Compression

```
Original: D × 4 bytes per vector
PQ code:  M bytes per vector

Compression ratio: D × 4 / M

Example (D=768, M=32):
  Ratio = 768 × 4 / 32 = 96×

With codebook sharing:
  Up to 12,000× (30 GB → 2.5 MB)
```

---

## 3. Bottlenecks

### 3.1 Storage

| Bottleneck | Mitigation |
|------------|------------|
| Serial mark-sweep GC | Incremental batches |
| Per-store dedup | Sharded chunk index |
| Single-machine caches | Distributed cache layer |
| Master-slave replication | Multi-master (future) |

### 3.2 Network

| Bottleneck | Mitigation |
|------------|------------|
| Trust graph BFS O(E) | Prune edges < 0.01 |
| DHT query latency | Cache providers |
| Connection single-threaded | Connection pooling |

### 3.3 Semantic

| Bottleneck | Mitigation |
|------------|------------|
| HNSW RAM footprint | PQ, DiskANN |
| Recall vs latency | Tune M, ef |
| Index rebuild time | Incremental updates |

### 3.4 Logic

| Bottleneck | Mitigation |
|------------|------------|
| Exponential proof-tree | Tabling, depth limits |
| Remote round-trips | Proof caching |
| Rule indexing | Hash index on head |

### 3.5 Transport

| Bottleneck | Mitigation |
|------------|------------|
| O(n) peer scan | Index by score |
| WantList memory | Bounded size |
| Retry storms | Backoff + jitter |

---

## 4. Scaling Strategies

### 4.1 Vertical Scaling

| Resource | Scaling |
|----------|---------|
| CPU | SIMD (AVX2/NEON) |
| RAM | PQ compression, DiskANN |
| Disk | Tiering, SSD |

### 4.2 Horizontal Scaling

| Strategy | Implementation |
|----------|----------------|
| Sharding | `ShardCoordinator` (consistent hash) |
| Replication | `Replicator` (full/incremental sync) |
| Load balancing | `SelectionStrategy` |

---

## 5. Optimization Techniques

### 5.1 Zero-Copy

- `Bytes` for block data (ref-counted)
- Tensor slice extraction without copy
- mmap for DiskANN

### 5.2 SIMD

```rust
// Runtime detection
AVX2: l2, dot, cosine
NEON: l2, dot, cosine
Scalar: fallback
```

### 5.3 Lazy Evaluation

- `OnceCell` for semantic/logic contexts
- Lazy DAG traversal in GraphSync

### 5.4 Batching

- `put_many` for bulk inserts
- `get_many` for bulk reads
- Query batcher for DHT

### 5.5 Caching

- LRU/LFU block cache
- Proof cache (LFU + TTL)
- DHT provider cache

---

## 6. Performance Tuning

### 6.1 HNSW Parameters

| Parameter | Effect | Trade-off |
|-----------|--------|-----------|
| M ↑ | Higher recall | More memory |
| ef_construction ↑ | Better index | Slower build |
| ef_search ↑ | Higher recall | Slower search |

### 6.2 Storage Decorators

| Decorator | Effect |
|-----------|--------|
| Cache | Reduces latency |
| Dedup | Saves space |
| Compression | Saves space |
| Encryption | Adds latency |

### 6.3 Network

| Parameter | Effect |
|-----------|--------|
| bucket_size | DHT fanout |
| query_timeout | DHT latency budget |
| max_peers | Connection pool size |

---

## 7. Benchmarking

### 7.1 Block Store

```
put_block:     20k ops/s (P99: 80µs)
get_block:     33k ops/s (P99: 50µs)  [cache hit]
get_block:     2k ops/s  (P99: 2ms)   [cache miss]
```

### 7.2 Semantic Search

```
HNSW search (k=10, 10M vectors):  1k q/s (P99: 10ms)
HNSW insert:                       500/s (P99: 20ms)
DiskANN search:                    200 q/s (P99: 50ms)
```

### 7.3 End-to-End

```
add_file (1MB):         50ms  [local]
get_block (network):    200ms [P50]
semantic_search:        5ms   [local]
inference (simple):     2ms   [local]
```

---

## 8. Monitoring

### 8.1 Metrics

```rust
pub struct IpfrsMetrics {
    pub blocks_stored: Counter,
    pub blocks_retrieved: Counter,
    pub cache_hits: Counter,
    pub cache_misses: Counter,
    pub dht_queries: Counter,
    pub network_bytes: Counter,
    pub semantic_queries: Counter,
    pub inference_time: Histogram,
}
```

### 8.2 Health Checks

- BlockStore: `is_empty()`, `len()`
- DHT: `is_healthy()`
- Session: `is_complete()`
- Index: `len()`, memory usage

---

**Next**: [12-EvolutionGuide.md](12-EvolutionGuide.md) — Extension points, future directions
