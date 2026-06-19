# Evolution Guide — Extension Points, Future Directions

> **Focus**: How to extend IPFRS, planned evolution, contribution areas

---

## 1. Extension Points

### 1.1 BlockStore Backends (Adapter Pattern)

```rust
// Implement this trait
#[async_trait]
impl BlockStore for MyBlockStore {
    async fn put(&self, block: &Block) -> Result<()>;
    async fn get(&self, cid: &Cid) -> Result<Option<Block>>;
    // ...
}
```

**Existing adapters**: Sled, ParityDB, S3, Memory  
**Potential adapters**: RocksDB, PostgreSQL, IPFS remote, Custom cloud

---

### 1.2 Storage Decorators (Decorator Pattern)

```rust
pub struct MyDecorator<S: BlockStore> {
    inner: Arc<S>,
}

#[async_trait]
impl<S: BlockStore> BlockStore for MyDecorator<S> {
    // Wrap inner operations
}
```

**Existing decorators**: Cache, Dedup, Quota, Compression, Encryption, TTL  
**Potential decorators**: Audit log, Rate limiter, Content filter, Analytics

---

### 1.3 Distance Metrics (Strategy Pattern)

```rust
pub enum DistanceMetric {
    L2,
    Cosine,
    DotProduct,
    // Add custom: MyMetric
}
```

**Implementation**: Add to `simd.rs` for vectorized version.

---

### 1.4 Inference Engines (Strategy Pattern)

```rust
// Implement custom inference
impl InferenceEngine for MyEngine {
    fn query(&self, goal: &Predicate, kb: &KnowledgeBase) -> Result<Vec<Substitution>>;
}
```

**Existing engines**: SLD, Tabling, Temporal, Fuzzy, Epistemic, PLN, Bayesian  
**Potential engines**: Description logic, Modal logic, Non-monotonic

---

### 1.5 Transports (Strategy Pattern)

```rust
#[async_trait]
impl Transport for MyTransport {
    async fn connect(&self, addr: &Multiaddr) -> Result<Box<dyn Connection>>;
}
```

**Existing transports**: QUIC, TCP, WebSocket, WebTransport  
**Potential transports**: Tor, I2P, Bluetooth, WebRTC

---

### 1.6 Peer Selection Strategies (Strategy Pattern)

```rust
pub enum SelectionStrategy {
    FastestFirst,
    HighestBandwidth,
    BestScore,
    RoundRobin,
    Random,
    LeastLoaded,
    // Add custom: MyStrategy
}
```

---

### 1.7 Protocol Handlers (Open Host Service)

Add new protocols to `ipfrs-interface`:

```rust
// Example: Add MQTT protocol
pub struct MqttServer {
    node: Arc<Node>,
}
```

**Existing protocols**: gRPC, GraphQL, HTTP, WebSocket  
**Potential protocols**: MQTT, AMQP, GraphQL subscriptions, gRPC-web

---

## 2. Planned Evolution

### 2.1 Short-term (0.3.0)

| Feature | Description |
|---------|-------------|
| Multi-master replication | Write anywhere, conflict resolution |
| Batch GC parallelism | Cross-shard parallel mark-sweep |
| OPQ quantization | Learned rotation for better recall |
| GraphQL subscriptions | Real-time updates |

### 2.2 Medium-term (0.4.0)

| Feature | Description |
|---------|-------------|
| Distributed query planner | Cross-shard query optimization |
| Proof compression | Compact proof trees |
| Bandwidth market | Incentivized peer participation |
| Browser WASM | Full browser support |

### 2.3 Long-term (1.0.0)

| Feature | Description |
|---------|-------------|
| Sharding v2 | Dynamic resharding |
| Consensus | Optional consensus layer |
| Formal verification | Proof-carrying rules |
| Multi-chain bridges | Ethereum, Solana, etc. |

---

## 3. Contribution Areas

### 3.1 High Priority

| Area | Skills Needed |
|------|---------------|
| New BlockStore backends | Rust, databases |
| SIMD optimizations | Rust, AVX2/NEON |
| Performance profiling | Benchmarking |
| Documentation | Technical writing |

### 3.2 Medium Priority

| Area | Skills Needed |
|------|---------------|
| New inference engines | Logic, Rust |
| New transports | Networking, Rust |
| Protocol implementations | Web, gRPC |
| Testing infrastructure | QA, Rust |

### 3.3 Nice to Have

| Area | Skills Needed |
|------|---------------|
| Visualization tools | Frontend, D3 |
| CLI improvements | Rust, UX |
| Examples/tutorials | Documentation |
| Benchmark suite | Performance testing |

---

## 4. Architecture Evolution Principles

### 4.1 Backward Compatibility

- New features must not break existing APIs
- `BlockStore` trait is stable
- `Cid` format is immutable

### 4.2 Bounded Context Integrity

- New features belong to existing contexts or new contexts
- No cross-context coupling without ACL
- Shared Kernel changes require consensus

### 4.3 Performance First

- No regression on P99 latencies
- Memory footprint must not grow unbounded
- New features should be opt-in (lazy)

### 4.4 Testability

- All new code requires unit tests
- Integration tests for cross-context features
- Property-based tests for invariants

---

## 5. Extension Checklist

When adding a new feature:

- [ ] Identify target bounded context
- [ ] Define extension point (trait, enum, struct)
- [ ] Implement with tests
- [ ] Add to facade if user-facing
- [ ] Update documentation
- [ ] Add to CLI/API if applicable
- [ ] Benchmark performance impact

---

## 6. File Structure for Extensions

```
ipfrs_source/crates/
├── ipfrs-core/              # Shared Kernel (changes carefully)
├── ipfrs-storage/
│   └── src/
│       ├── traits.rs        # BlockStore trait
│       ├── my_backend.rs    # ← Add new backend
│       └── my_decorator.rs  # ← Add new decorator
├── ipfrs-semantic/
│   └── src/
│       ├── distance.rs      # DistanceMetric enum
│       └── my_metric.rs     # ← Add new metric
├── ipfrs-tensorlogic/
│   └── src/
│       ├── reasoning.rs     # InferenceEngine trait
│       └── my_engine.rs     # ← Add new engine
├── ipfrs-transport/
│   └── src/
│       ├── transport.rs     # Transport trait
│       └── my_transport.rs  # ← Add new transport
└── ipfrs-interface/
    └── src/
        ├── mod.rs           # Protocol registry
        └── my_protocol.rs   # ← Add new protocol
```

---

## 7. Getting Help

- **Documentation**: `/Wiki`, `/Wiki_GLM`, `/Wiki_Arch_GLM`
- **Architecture**: `ARCHITECTURE_DDD_DEEP.md`
- **Examples**: `ipfrs-cli/`, `ipfrs-python/`
- **Issues**: GitHub issues for bugs, features

---

## 8. Design Principles for Extensions

1. **Content-addressing first** — New types should be CID-addressable
2. **Port/Adapter** — External dependencies behind traits
3. **Immutability** — Prefer immutable data structures
4. **Lazy evaluation** — Expensive operations on demand
5. **Graceful degradation** — Handle failures without crash
6. **Observability** — Emit events, metrics for monitoring

---

**End of Architecture Research**

This completes the IPFRS Architecture documentation. For implementation details, see `/Wiki_GLM`. For high-level understanding, see `/Wiki`.
