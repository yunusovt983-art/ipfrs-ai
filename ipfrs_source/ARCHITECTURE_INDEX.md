# IPFRS Architecture Documentation Index

**Version**: 0.2.0 "Network Release"  
**Last Updated**: 2026-06-18

---

## Quick Navigation

### 📚 Main Documents

| Document | Focus | Audience | Status |
|----------|-------|----------|--------|
| [ARCHITECTURE_DDD.md](./ARCHITECTURE_DDD.md) | **Domain-Driven Design** (NEW) | Architects, Senior Engineers | ✅ Complete |
| [README.md](./README.md) | Quick start & overview | All levels | ✅ Complete |
| [crates/ipfrs-transport/ARCHITECTURE.md](./crates/ipfrs-transport/ARCHITECTURE.md) | Protocol layers & messaging | Protocol engineers | ✅ Complete |

---

## Architecture Highlights

### 🏗️ Five Bounded Contexts (DDD)

```
1. STORAGE DOMAIN
   └─ Immutable blocks, CID, DAG, Sled database
   
2. NETWORK DOMAIN  
   └─ Peers, DHT, libp2p, peer discovery
   
3. SEMANTIC DOMAIN
   └─ Vector search, HNSW, similarity, embedding
   
4. LOGIC DOMAIN
   └─ Terms, predicates, rules, inference
   
5. TRANSPORT DOMAIN
   └─ Sessions, Bitswap, messaging, reliability
```

### 🎯 Key Principles

- **Content-Addressed**: Every block has a cryptographic identity (CID)
- **Distributed**: No central authority; P2P via libp2p
- **Intelligent**: Semantic search + Logic programming
- **Pure Rust**: Memory-safe, high-performance
- **Zero-Copy**: Apache Arrow for tensor data

### 📊 Architecture Layers

```
Presentation  → HTTP API, CLI, WASM, Node.js, Python
              ↓
Application   → Use cases, orchestration
              ↓
Domain        → 5 bounded contexts (Storage, Network, Semantic, Logic, Transport)
              ↓
Infrastructure → Sled, libp2p, HNSW, Tokio
```

---

## Document Descriptions

### ARCHITECTURE_DDD.md (NEW)

**Purpose**: Comprehensive Domain-Driven Design blueprint

**Covers**:
- Bounded contexts (5 domains)
- Core domain models (Block, CID, DAG, Peer)
- Aggregate design (roots, invariants)
- Repository pattern
- Application services
- Event-driven architecture
- DDD patterns used (Aggregate, Repository, Service, Observer, Factory, Strategy)
- Scalability & testing strategies

**Best For**:
- Understanding overall system design
- Making architectural decisions
- Adding new features
- Reviewing code across domains

**Read Time**: 30-40 minutes

---

### crates/ipfrs-transport/ARCHITECTURE.md

**Purpose**: Deep dive into protocol layers and message flow

**Covers**:
- Component overview (Want lists, peer manager, message handler)
- Core protocol layers (Transport, Message, Block Exchange, Application)
- Message flow diagrams (6 scenarios)
- State machines (5 types: Peer, Want, Session, Circuit Breaker, Partition Detection)
- Data flow diagrams
- Concurrency model (Tokio threads, shared state, lock hierarchy)
- Error handling flow

**Best For**:
- Understanding network communication
- Implementing transport features
- Debugging peer interactions
- Performance optimization

**Read Time**: 20-30 minutes

---

### README.md

**Purpose**: Quick start guide and feature overview

**Covers**:
- Installation & setup
- CLI commands
- Rust API examples
- HTTP API endpoints
- Features list (6 main features)
- Performance benchmarks
- Roadmap (v0.1 through v1.0)

**Best For**:
- Getting started quickly
- Learning by example
- API reference
- Understanding capabilities

**Read Time**: 10-15 minutes

---

## Key Concepts

### Content Identifier (CID)

**What**: Cryptographic hash of content  
**Why**: Enables content-addressing without central registry  
**Formula**: `CID = Hash(content, algorithm)`  
**Properties**: Deterministic, immutable, collision-resistant

### Block

**What**: Immutable unit of storage  
**Size**: Typically 256KB (configurable)  
**Invariant**: `CID = Hash(block.data)`  
**Lifecycle**: Create → Store → Announce → (maybe) Request from peers

### Peer Reputation

**What**: Score reflecting peer reliability  
**Range**: [0, ∞)  
**Update**: Success → +reward; Failure → ×0.95  
**Use**: Peer selection in block requests

### Semantic Index (HNSW)

**What**: Hierarchical Navigable Small World index  
**Purpose**: Fast approximate k-nearest-neighbor search  
**Query Time**: O(log N) on average  
**Space**: ~1-2KB per indexed vector

### Logic Rules

**What**: Datalog-style predicates and rules  
**Example**: `ancestor(X, Y) :- parent(X, Y) | ancestor(X, Z), parent(Z, Y)`  
**Purpose**: Distributed reasoning and inference

---

## Domain Interaction Patterns

### Pattern 1: Adding a File

```
User → Application → Storage (create Block) 
                  → Network (announce CID)
                  → Semantic (index if needed)
```

### Pattern 2: Retrieving a File

```
User → Application → Transport (find peers)
                  → Network (select best peer)
                  → Storage (store received block)
                  → Semantic (update index)
```

### Pattern 3: Semantic Search

```
User → Application → Semantic (query HNSW)
                  → (maybe) Network (fetch blocks)
```

### Pattern 4: Logic Query

```
User → Application → Logic (unify & infer)
                  → (maybe) Network (fetch rules)
```

---

## Implementation Details

### Repository Structure

```
ipfrs/
├── crates/
│   ├── ipfrs-core/           # Core types (Block, CID)
│   ├── ipfrs-storage/        # Storage domain
│   ├── ipfrs-network/        # Network domain
│   ├── ipfrs-semantic/       # Semantic domain
│   ├── ipfrs-tensorlogic/    # Logic domain
│   ├── ipfrs-transport/      # Transport domain
│   ├── ipfrs-interface/      # HTTP API
│   ├── ipfrs/                # Unified library
│   ├── ipfrs-cli/            # CLI
│   ├── ipfrs-wasm/           # WebAssembly
│   ├── ipfrs-nodejs/         # Node.js bindings
│   └── ipfrs-python/         # Python bindings
```

### Key Dependencies

```
Runtime:  tokio (async), libp2p (networking)
Storage:  sled (embedded DB)
Search:   hnsw_rs (vector indexing)
Arrow:    Zero-copy I/O
Crypto:   blake3, sha2, sha3
Compress: oxiarc-* (pure Rust)
```

---

## Performance Baseline (v0.2.0)

| Operation | Time | Throughput |
|-----------|------|------------|
| Block PUT | ~50µs | 20k ops/sec |
| Block GET | ~30µs | 33k ops/sec |
| DAG PUT | ~80µs | 12.5k ops/sec |
| Semantic search | ~1ms | 1k queries/sec |
| HNSW insert | ~100µs | 10k inserts/sec |

*Tested on: AMD Ryzen 9 5900X, NVMe SSD*

---

## Roadmap

- ✅ **v0.1.0**: Foundation (content storage, semantic search, logic, HTTP API)
- ✅ **v0.2.0**: Network Release (libp2p, DHT, Bitswap, distributed inference)
- 🚧 **v0.3.0**: Intelligence (persistent indexes, advanced reasoning)
- 📅 **v0.4.0**: Ecosystem (GraphQL, monitoring)
- 📅 **v1.0.0**: Stable (API guarantees)

---

## Contributing Guidelines

### Code Quality

- ✅ Follow Rust style (`rustfmt`)
- ✅ Zero compiler warnings
- ✅ Add tests for features
- ✅ Update documentation

### Commits

- Use conventional commits: `feat:`, `fix:`, `docs:`, `test:`
- Link related issues: `Closes #123`
- Reference domains: `[storage]`, `[network]`, `[semantic]`

### Testing

```bash
# Run all tests
cargo nextest run --workspace --all-features

# Run tests with logging
RUST_LOG=debug cargo test

# Run specific crate
cargo test -p ipfrs-storage
```

### Documentation

- Update ARCHITECTURE_DDD.md for design changes
- Update crate READMEs for API changes
- Add examples for new features

---

## Decision Log

### D1: Why Content-Addressing (CID)?

**Decision**: Use cryptographic content hashing for identity  
**Rationale**:
- Enables deduplication (same content → same CID)
- Detects corruption (verify CID matches data)
- Supports P2P without naming authority
- Enables incentive alignment (valuable content = valuable CID)

### D2: Why Five Bounded Contexts?

**Decision**: Split into Storage, Network, Semantic, Logic, Transport  
**Rationale**:
- Each domain has distinct language & concepts
- Orthogonal scaling (storage independent of network)
- Independent testing (mock other domains)
- Clear integration points (repositories & events)

### D3: Why Tokio (async/await)?

**Decision**: Use Tokio for async runtime  
**Rationale**:
- High concurrency (millions of peers)
- Non-blocking I/O (network & disk)
- Ecosystem (libp2p, Axum, etc.)

### D4: Why Sled (embedded DB)?

**Decision**: Use Sled for block storage  
**Rationale**:
- Embedded (no external database needed)
- ACID transactions
- Good performance for our access patterns
- Pure Rust

---

## FAQ

**Q: How does IPFRS differ from IPFS/Kubo?**

A: IPFRS adds:
- Semantic search (HNSW vector indexing)
- Logic programming (TensorLogic)
- Better performance (Pure Rust)
- Modern networking (QUIC, distributed inference)

**Q: Can I use IPFRS without semantic search?**

A: Yes! Disable `semantic_config` in `NodeConfig`.

**Q: How many blocks can IPFRS store?**

A: Limited only by disk space. HNSW index scales to millions of vectors.

**Q: Is IPFRS production-ready?**

A: v0.2.0 is "Production Ready" for core storage & networking. Semantic search & logic programming are beta.

**Q: How do I contribute?**

A: See [CONTRIBUTING.md](./CONTRIBUTING.md). PRs welcome! Start with issues labeled `good first issue`.

---

## Related Resources

- **IPFS Spec**: https://spec.ipfs.tech/
- **libp2p Spec**: https://github.com/libp2p/specs
- **HNSW Paper**: https://arxiv.org/abs/1802.02413
- **TensorLogic**: [In repository docs]

---

## Support

- **Issues**: [GitHub Issues](https://github.com/cool-japan/ipfrs/issues)
- **Discussions**: [GitHub Discussions](https://github.com/cool-japan/ipfrs/discussions)
- **Docs**: [docs.rs/ipfrs](https://docs.rs/ipfrs)
- **Community**: #ipfrs on COOLJAPAN Discord

---

**Maintained by**: COOLJAPAN OU (Team Kitasan)  
**License**: Apache-2.0  
**Latest Release**: v0.2.0 (2026-06-15)
