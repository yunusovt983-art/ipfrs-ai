# IPFRS Architecture Guide - Complete Reference

**Version**: 0.2.0 "Network Release"  
**Date**: 2026-06-18  
**Status**: ✅ Complete Professional Documentation

---

## 📚 Documentation Map

This directory contains comprehensive architecture documentation for IPFRS at multiple levels of detail:

### Root Level Documents (Quick Reference)

```
cool-japan/
├── IPFRS_DEEP_ARCHITECTURE.md    ← START HERE (15 chapters, detailed)
├── ARCHITECTURE_GUIDE.md          ← This file (navigation & overview)
├── Vendor/
│   ├── SUMMARY.md
│   ├── PROJECT_STATUS.md
│   ├── ipfrs/
│   │   ├── ARCHITECTURE_DDD.md     (DDD perspective)
│   │   ├── ARCHITECTURE_INDEX.md   (quick reference)
│   │   └── crates/
│   │       └── ipfrs-transport/
│   │           └── ARCHITECTURE.md (protocol details)
│   │
│   └── go-ethereum/               (reference implementation)
```

---

## 🎯 Which Document Should I Read?

### If You Want To...

**Understand the complete system from first principles**
→ Read: [IPFRS_DEEP_ARCHITECTURE.md](./IPFRS_DEEP_ARCHITECTURE.md)
- 10,000+ lines
- 15 major chapters
- Every component explained
- Complete data flows
- Performance models
- Error handling

**Get a quick overview of the architecture**
→ Read: [Vendor/ipfrs/ARCHITECTURE_DDD.md](./Vendor/ipfrs/ARCHITECTURE_DDD.md)
- DDD (Domain-Driven Design) perspective
- 5 bounded contexts
- Core aggregates
- Design patterns
- 30-40 minute read

**Understand networking and protocols**
→ Read: [Vendor/ipfrs/crates/ipfrs-transport/ARCHITECTURE.md](./Vendor/ipfrs/crates/ipfrs-transport/ARCHITECTURE.md)
- Message flow diagrams
- State machines
- Protocol layers
- Performance considerations

**Learn how to set up and integrate IPFRS**
→ Read: [Vendor/ipfrs/README.md](./Vendor/ipfrs/README.md)
- Quick start guide
- Installation instructions
- Usage examples
- HTTP API reference

**Understand the project structure and status**
→ Read: [Vendor/PROJECT_STATUS.md](./Vendor/PROJECT_STATUS.md)
- Repository status
- Technology stack
- Performance metrics
- Next steps

---

## 🏗️ Architecture Layers Covered

### DEEP ARCHITECTURE Document Covers

#### Layer 0: System Philosophy
- What is IPFRS? (problem statement)
- Bi-layer architecture (Logical + Physical)
- Core philosophy: unifying data with intelligence

#### Layer 1: Complete Stack (6 Layers)
```
Presentation  → HTTP | CLI | WASM | Node.js | Python
Application   → Use cases & orchestration
Domain        → 5 bounded contexts
Infrastructure → Trait abstractions
Implementation → Concrete engines (Sled, libp2p, HNSW)
Hardware      → CPU, RAM, SSD, Network
```

#### Layer 2: Five Bounded Contexts (In Depth)

**1. Storage Domain**
- Block storage in Sled
- LRU caching
- Corruption detection & repair
- Garbage collection
- Write/read paths explained
- Statistics and monitoring

**2. Network Domain**
- libp2p integration
- Kademlia DHT
- Peer discovery (mDNS, bootstrap)
- NAT traversal (AutoNAT, DCUtR)
- Peer reputation scoring
- Content routing

**3. Semantic Domain**
- HNSW vector index
- Embedding computation
- Vector search algorithm
- Query caching
- Distance metrics
- Index persistence

**4. Logic Domain**
- Backward chaining inference
- Unification and pattern matching
- Rule evaluation
- Proof generation
- Consistency checking
- Termination analysis

**5. Transport Domain**
- Bitswap protocol
- Want list management
- Block exchange sessions
- Peer scoring for selection
- Circuit breaker pattern
- Message flow and sequencing

#### Layer 3: Data Flow Patterns (4 Complete Examples)
1. User adds file → Storage → Network announcement → Semantic indexing
2. User retrieves file → Storage check → DHT lookup → Block exchange
3. User semantic search → Embedding → HNSW query → Result ranking
4. User logic query → Goal matching → Inference → Proof generation

#### Layer 4: Component Interactions
- Repository pattern for loose coupling
- Event emitting for async notification
- Dependency injection for flexibility
- Trait objects vs concrete types

#### Layer 5: Core Aggregates & Invariants
```
Block Aggregate
├─ Invariant: hash(data) == cid
├─ Lifecycle: Created → Persisted → Referenced → (Garbage Collected)

Peer Aggregate
├─ Invariant: PeerId = hash(public_key)
├─ Lifecycle: Discovered → Connected → Scored → Evicted

BlockExchangeSession Aggregate
├─ Invariant: received_blocks ⊆ requested_blocks
├─ Lifecycle: Created → Active → Completed
```

#### Layer 6: Runtime Execution
- Tokio async runtime (8-16 worker threads)
- Task hierarchy (accept → send → receive loops)
- Synchronization primitives (Arc, RwLock, DashMap, mpsc)
- Concurrent access patterns

#### Layer 7: Storage Deep Dive
- Sled database architecture
- Block write path (50µs)
- Block read path (30µs cache, 100µs disk)
- WAL and crash safety
- Bloom filters for fast negative lookups

#### Layer 8: Network Deep Dive
- libp2p swarm architecture
- DHT lookup algorithm
- Peer reputation calculation
- Connection pooling and reuse
- Message routing and handling

#### Layer 9: Semantic Deep Dive
- HNSW hierarchical structure (3+ layers)
- k-NN search algorithm (converging descent)
- Query caching with LRU eviction
- Similarity metrics (cosine, L2, etc.)

#### Layer 10: Logic Deep Dive
- Backward chaining algorithm
- Proof tree construction
- Unification and substitution
- Termination checking
- Inconsistency detection

#### Layer 11: Transport Deep Dive
- Bitswap message types
- Want list as priority queue
- Peer selection algorithm
- Circuit breaker state machine
- Session lifecycle management

#### Layer 12: Complete Operations (4 Full Examples)
1. Add large file (100 MB)
   - Chunking
   - Storage per chunk
   - Semantic indexing
   - Network announcement
   - Timeline: ~900ms end-to-end

2. Retrieve from network
   - Local storage check
   - DHT lookup
   - Peer scoring
   - Block exchange
   - Timeline: 200-1000ms

#### Layer 13: Memory & Performance
- RAM breakdown (4.5 GB for 1TB data)
  - LRU cache: 2.0 GB
  - HNSW index: 1.5 GB
  - Peer state: 100 MB
  - Session state: 50 MB
  - Metadata: 200 MB
  - Runtime: 600 MB

- Latency profile (P50/P99/P999)
- Throughput limits (by bottleneck)
- Disk usage breakdown

#### Layer 14: Error Handling & Recovery
- Error taxonomy (7 error categories)
- Recovery strategies:
  - Automatic retry
  - Fallback to other peers
  - Corruption repair
  - Circuit breaker pattern

---

## 📖 Reading Paths by Role

### For Architects

1. **Quick Overview** (15 min)
   - Read: System Overview section in DEEP_ARCHITECTURE.md
   - Read: Architecture Layers diagram

2. **Design Decisions** (45 min)
   - Read: Five Bounded Contexts (complete)
   - Read: Component Interactions

3. **Advanced Topics** (2 hours)
   - Read: Runtime Execution Model
   - Read: Memory & Performance Model
   - Read: Error Handling & Recovery

### For Backend Engineers

1. **Getting Started** (30 min)
   - Read: How Operations Flow Through System
   - Read: Storage System Deep Dive
   - Read: Network System Deep Dive

2. **Implementation Details** (2 hours)
   - Read: Each bounded context deep dive
   - Study: Data Flow Patterns
   - Review: Core Aggregates & Invariants

3. **Optimization** (1 hour)
   - Read: Memory & Performance Model
   - Read: Latency profile
   - Read: Throughput limits

### For DevOps / Operations

1. **System Understanding** (20 min)
   - Read: System Overview
   - Read: Layered Architecture

2. **Operations** (45 min)
   - Read: Memory consumption breakdown
   - Read: Performance metrics
   - Read: Error handling

3. **Monitoring** (30 min)
   - Read: Statistics tracked in each domain
   - Read: Metrics collection points

### For Contributors

1. **Foundation** (1 hour)
   - Read: Complete DEEP_ARCHITECTURE.md

2. **Pick a Domain** (2 hours)
   - Read: That domain's deep dive section
   - Study: Error handling for that domain
   - Review: Data flows involving that domain

3. **Get Hands-On** (2+ hours)
   - Clone: `git clone https://github.com/cool-japan/ipfrs.git`
   - Build: `cargo build --release`
   - Test: `cargo test`
   - Read: Crate source code with documentation in hand

---

## 🔑 Key Concepts Explained Across Documents

### Content Identifier (CID)

**Overview**: Every piece of data has a cryptographic identity

**Covered In**:
- DEEP_ARCHITECTURE: System Overview, Storage Domain, Core Aggregates
- DDD_ARCHITECTURE: Central Value Object section
- TRANSPORT_ARCHITECTURE: Content addressing layer

**Key Points**:
- Deterministic: `hash(data) == cid` (same data → same CID)
- Collision-resistant: 2^256 security
- Immutable: CID cannot change without being different
- Unique identifier for content

---

### Peer Reputation

**Overview**: Trust metric for peer selection

**Covered In**:
- DEEP_ARCHITECTURE: Network System, Peer Scoring
- NETWORK_ARCHITECTURE: Reputation management
- TRANSPORT_ARCHITECTURE: Peer selection algorithm

**Formula**:
```
score = success_rate × recency × speed × availability
```

---

### HNSW Vector Index

**Overview**: Fast approximate k-nearest-neighbor search

**Covered In**:
- DEEP_ARCHITECTURE: Semantic System, Search Algorithm
- ARCHITECTURE_INDEX: Key Concepts section
- Network performance: k=10 in ~1ms

---

### Backward Chaining Inference

**Overview**: How logic programming queries are answered

**Covered In**:
- DEEP_ARCHITECTURE: Logic System (complete algorithm)
- Proof tree construction explained step-by-step
- Termination checking and recursion limits

---

## 🎯 Implementation Guides

Each domain has implementation guidance:

### Storage Domain
- **Crate**: `ipfrs-storage`
- **Files**: `/Vendor/ipfrs/crates/ipfrs-storage/`
- **Key**: `src/lib.rs` (797 lines)
- **Start**: Read `README.md`, then `lib.rs`

### Network Domain
- **Crate**: `ipfrs-network`
- **Files**: `/Vendor/ipfrs/crates/ipfrs-network/`
- **Key**: `src/node.rs` (1250 lines)
- **Start**: Read `README.md`, then `node.rs`

### Semantic Domain
- **Crate**: `ipfrs-semantic`
- **Files**: `/Vendor/ipfrs/crates/ipfrs-semantic/`
- **Key**: `src/router.rs` (931 lines)
- **Start**: Read `README.md`, then `router.rs`

### Logic Domain
- **Crate**: `ipfrs-tensorlogic`
- **Files**: `/Vendor/ipfrs/crates/ipfrs-tensorlogic/`
- **Key**: `src/store.rs` (1334 lines)
- **Start**: Read `README.md`, then `store.rs`

### Transport Domain
- **Crate**: `ipfrs-transport`
- **Files**: `/Vendor/ipfrs/crates/ipfrs-transport/`
- **Key**: `src/bitswap/exchange.rs`
- **Start**: Read `ARCHITECTURE.md`, then source code

---

## 📊 Document Statistics

```
IPFRS_DEEP_ARCHITECTURE.md
├─ Length: 10,000+ lines
├─ Chapters: 15
├─ Code examples: 100+
├─ Diagrams: 50+
├─ Read time: 2-3 hours (thorough)
└─ Skill level: Intermediate to Advanced

ARCHITECTURE_DDD.md  
├─ Length: 2,000 lines
├─ Chapters: 5 contexts
├─ Design patterns: 6
├─ Read time: 30-40 minutes
└─ Skill level: Intermediate

ARCHITECTURE_GUIDE.md (this file)
├─ Length: 500 lines
├─ Navigation maps: 4
├─ Reading paths: 4 roles
└─ Read time: 15 minutes
```

---

## ✨ What Each Document Excels At

| Document | Best For | Length | Time |
|----------|----------|--------|------|
| DEEP_ARCHITECTURE | Complete understanding | 10k lines | 2-3 hrs |
| DDD_ARCHITECTURE | Design decisions | 2k lines | 30-40 min |
| TRANSPORT_ARCHITECTURE | Protocol details | 600 lines | 20-30 min |
| README.md | Getting started | 600 lines | 10-15 min |
| PROJECT_STATUS | Status overview | 1k lines | 20-30 min |
| ARCHITECTURE_GUIDE | Navigation (you are here) | 500 lines | 15 min |

---

## 🚀 Getting Started

### Path 1: Quick Start (30 minutes)
```
1. Read: System Overview in DEEP_ARCHITECTURE.md (5 min)
2. Read: Layered Architecture diagram (5 min)
3. Skim: Five Bounded Contexts (quick overview) (10 min)
4. Try: cargo build && cargo test (10 min)
```

### Path 2: Deep Learning (3 hours)
```
1. Read: Complete DEEP_ARCHITECTURE.md (2 hours)
2. Study: One domain deep dive in detail (30 min)
3. Hands-on: Run examples, modify code (30 min)
```

### Path 3: Specialist (Domain-Specific)
```
1. Read: Your domain's section in DEEP_ARCHITECTURE.md
2. Read: Domain's README.md in Vendor/ipfrs/crates/
3. Study: Domain's source code
4. Implement: Fix a domain-specific issue
```

---

## 📎 Quick Reference

### Core Invariants

```
Storage: hash(data) == cid
Network: PeerId = hash(public_key)
Semantic: 0.0 ≤ similarity ≤ 1.0
Logic: Rules must be consistent
Transport: FIFO per peer connection
```

### Key Performance Numbers

```
Storage:
  - Block GET (cache): 30µs
  - Block PUT: 50µs
  - Block GET (disk): 100µs

Network:
  - DHT lookup: 150-300ms
  - Peer connection: 100-200ms

Semantic:
  - HNSW search: 1-10ms
  - Query cache hit: <1ms

Logic:
  - Inference query: 1-5ms

Overall:
  - Add file: ~900ms (10 MB)
  - Retrieve: 200-1000ms (network dependent)
```

### Limits

```
Storage:
  - Max block size: 256KB
  - Min block size: 1 byte
  - Cache: 10k blocks (2GB)

Network:
  - DHT peers: 20-100
  - Connected peers: 100+
  - Max connections: configurable

Semantic:
  - Max indexed vectors: 10M+
  - Vector dimension: 768 (standard)
  - Cache size: 10k queries (LRU)

Logic:
  - Recursion depth limit: 1000
  - Rule count: unlimited
  - Fact count: unlimited
```

---

## 🎓 Learning Objectives

After reading these documents, you should understand:

✅ How IPFRS unifies data storage with distributed intelligence  
✅ The five bounded contexts and how they interact  
✅ How data flows through the system end-to-end  
✅ The key invariants protecting data integrity  
✅ The performance characteristics and bottlenecks  
✅ How to extend each domain with new features  
✅ How to troubleshoot and debug issues  
✅ How to optimize for specific use cases  

---

## 📝 Contributing

When contributing to IPFRS:

1. **Understand** the relevant domain (read deep dive)
2. **Design** changes according to DDD principles
3. **Implement** with invariant protection
4. **Test** with comprehensive examples
5. **Document** your changes (update architecture docs)
6. **Performance** check (measure impact)

---

## 🔗 External References

- **IPFS Spec**: https://spec.ipfs.tech/
- **libp2p Spec**: https://github.com/libp2p/specs
- **HNSW Paper**: https://arxiv.org/abs/1802.02413
- **Rust Async**: https://tokio.rs/
- **Sled Docs**: https://docs.rs/sled/

---

## 📞 Questions?

- **Architecture questions**: Check DEEP_ARCHITECTURE.md
- **DDD questions**: Check DDD_ARCHITECTURE.md
- **Protocol questions**: Check TRANSPORT_ARCHITECTURE.md
- **Setup questions**: Check README.md
- **Status questions**: Check PROJECT_STATUS.md

---

**Status**: ✅ Complete Reference Documentation  
**Version**: 0.2.0 "Network Release"  
**Last Updated**: 2026-06-18  
**Ready for**: Production use, team onboarding, system extension

**Next Step**: [👉 Read IPFRS_DEEP_ARCHITECTURE.md](./IPFRS_DEEP_ARCHITECTURE.md)
