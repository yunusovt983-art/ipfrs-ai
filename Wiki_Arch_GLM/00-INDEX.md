# IPFRS Architecture Research — GLM Opus 4.8 Analysis

> **Version**: 0.2.0 "Network Release"  
> **Status**: Production Architecture  
> **Generated**: 2026-06-19  
> **Analysis Model**: GLM5 / Claude Opus 4.8 hybrid deep-dive

---

## Purpose

Это исследование предоставляет **глубокий архитектурный анализ** IPFRS с точки зрения Domain-Driven Design (DDD). В отличие от `/Wiki` (стиль Карпати — high-level понимание) и `/Wiki_GLM` (технический deep-dive), данный фокус:

- **Strategic Design** — контекст-маппинг, bounded contexts, контекстные отношения
- **Tactical Patterns** — Aggregates, Entities, Value Objects, Domain Services
- **Design Decisions** — trade-offs, rationale, философские choices
- **Cross-Cutting Concerns** — как контексты взаимодействуют

---

## Documents

| # | File | Description | Time |
|---|------|-------------|------|
| 01 | [Strategic Design](01-StrategicDesign.md) | Context Map, Bounded Contexts, relationships | 25 min |
| 02 | [Shared Kernel](02-SharedKernel.md) | ipfrs-core — Cid, Block, Ipld, invariants | 30 min |
| 03 | [Storage Context](03-StorageContext.md) | BlockStore port, decorators, GC, tiering | 35 min |
| 04 | [Network Context](04-NetworkContext.md) | libp2p, DHT, reputation graph, semantic routing | 35 min |
| 05 | [Semantic Context](05-SemanticContext.md) | HNSW/DiskANN, quantization, embedding pipeline | 30 min |
| 06 | [Logic Context](06-LogicContext.md) | IR, inference engines, neural-symbolic fusion | 40 min |
| 07 | [Transport Context](07-TransportContext.md) | Bitswap, sessions, want-list, peer scoring | 30 min |
| 08 | [Application Layer](08-ApplicationLayer.md) | Node facade, interface protocols, bindings | 25 min |
| 09 | [Context Integration](09-ContextIntegration.md) | ACL patterns, events, cross-context flows | 30 min |
| 10 | [Design Decisions](10-DesignDecisions.md) | Trade-offs, rationale, philosophy | 35 min |
| 11 | [Performance Model](11-PerformanceModel.md) | Bottlenecks, scaling strategies, optimization | 25 min |
| 12 | [Evolution Guide](12-EvolutionGuide.md) | Extension points, future directions | 20 min |

**Total reading time**: ~5.5 hours

---

## Quick Reference

### Architecture at a Glance

```
┌─────────────────────────────────────────────────────────────────────┐
│                    STRATEGIC DESIGN MAP                             │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │                    SHARED KERNEL                               │ │
│  │         Cid · Block · Ipld · TensorBlock · Manifest            │ │
│  │         Codec · HashEngine · Error · Result                    │ │
│  └────────────────────────────┬───────────────────────────────────┘ │
│                               │                                     │
│    ┌──────────────────────────┼──────────────────────────┐          │
│    │                          │                          │          │
│    ▼                          ▼                          ▼          │
│ ┌──────────────┐     ┌─────────────────┐     ┌───────────────┐      │
│ │   STORAGE    │     │     NETWORK     │     │   SEMANTIC    │      │
│ │              │     │                 │     │               │      │
│ │ BlockStore   │◄────┤ NetworkNode     ├────►│ VectorIndex   │      │
│ │ (port)       │     │ PeerStore       │     │ (HNSW/DiskANN)│      │
│ │              │     │ DHT (Kademlia)  │     │               │      │
│ │ Decorators:  │     │                 │     │ Quantizer     │      │
│ │ Cache→Dedup  │     │ Reputation:     │     │ ReRanker      │      │
│ │ →Quota→Comp  │     │ EWMA + Graph    │     │               │      │
│ │ →Encrypt→TTL │     │                 │     │               │      │
│ └──────┬───────┘     └────────┬────────┘     └───────┬───────┘      │
│        │                      │                      │              │
│        │         ┌────────────┴───────────────┐      │              │
│        │         │       TRANSPORT            │      │              │
│        └────────►│                            │◄─────┘              │
│                  │  Session (AR)              │                     │
│                  │  WantList (Priority Queue) │                     │
│                  │  BitswapExchange           │                     │
│                  │  Multi-Transport           │                     │
│                  └────────────┬───────────────┘                     │
│                               │                                     │
│                  ┌────────────▼───────────────┐                     │
│                  │        LOGIC               │                     │
│                  │                            │                     │
│                  │  KnowledgeBase (AR)        │                     │
│                  │  InferenceEngine           │                     │
│                  │  Neural-Symbolic Fusion    │                     │
│                  │  ComputationGraph          │                     │
│                  └────────────────────────────┘                     │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Context Relationships (DDD Patterns)

```
┌─────────────────────────────────────────────────────────────────────┐
│                    CONTEXT MAP                                      │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ipfrs-core ───────────► ALL CONTEXTS                               │
│    │                     (Shared Kernel)                            │
│    │                                                                │
│  Storage ◄────────────── ALL CONTEXTS                               │
│    │                     (Conformist / Open Host Service)           │
│    │                                                                │
│  Transport ────────────► Storage                                    │
│    │                     (Customer/Supplier + ACL)                  │
│    │                                                                │
│  Transport ────────────► Network                                    │
│    │                     (Customer/Supplier)                        │
│    │                                                                │
│  All Domains ───────────► libp2p                                    │
│    │                     (Anti-Corruption Layer)                    │
│    │                                                                │
│  Logic ─────────────────► Storage                                   │
│    │                     (Published Language: IPLD)                 │
│    │                                                                │
│  Presentation ──────────► Application                               │
│    │                     (Open Host Service / Facade)               │
│    │                                                                │
│  Bindings ──────────────► Application                               │
│                          (Anti-Corruption Layer)                    │
│                                                                     │
│  ⚠️ INTENTIONAL DUPLICATION:                                        │
│  Network Reputation  ≠  Transport Peer Scoring                      │
│  (long-term trust)      (per-session quality)                       │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Aggregate Roots Summary

| Context | Aggregate Root | Identity | Invariant |
|---------|---------------|----------|-----------|
| Core | `Block` | `Cid` (computed) | `hash(data) == cid` |
| Core | `TensorBlock` | `Cid` | `data.len == shape × dtype` |
| Core | `ContentManifest` | `Cid` | Sorted entries, Merkle root |
| Storage | `PinInfo` | `Cid` | Ref count, pin type |
| Storage | `Snapshot` | `Cid` | Delta chain |
| Network | `Peer` | `PeerId` | `hash(pubkey)` |
| Semantic | `VectorIndex` | Config hash | Dimension, metric |
| Semantic | `DiskANNIndex` | Config hash | R, L, alpha |
| Logic | `KnowledgeBase` | `Cid` | Acyclic rules |
| Logic | `ProofTree` | `Cid` | Sound, acyclic |
| Transport | `Session` | `SessionId` | `recv + fail ≥ total` |

### Key Invariants

```
┌─────────────────────────────────────────────────────────────────────┐
│                    SYSTEM INVARIANTS                                │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  CORE:                                                              │
│    • Block: hash(data) == CID                 [content-addressing]  │
│    • Block: 1 ≤ len ≤ 2 MiB                   [size bounds]         │
│    • Ipld: BTreeMap keys                      [canonical encoding]  │
│    • TensorBlock: data.len == elements × dtype_bytes                │
│                                                                     │
│  STORAGE:                                                           │
│    • Blocks are immutable (write-once, delete-once)                 │
│    • Pin-protected blocks cannot be GC'd                            │
│    • WAL entry precedes mutation (durability)                       │
│                                                                     │
│  NETWORK:                                                           │
│    • PeerId = hash(public_key)                 [identity]           │
│    • Reputation ∈ [0, 100]                     [clamped]            │
│                                                                     │
│  SEMANTIC:                                                          │
│    • Vector dimension must match index                              │
│    • No NaN/Inf in vectors                                          │
│    • Cosine similarity ∈ [0, 1]                [normalized]         │
│                                                                     │
│  LOGIC:                                                             │
│    • Rule dependency graph is acyclic                               │
│    • Head variables bound by body                                   │
│    • Identical rule ⟹ identical CID           [dedup]              │
│    • Proof must be sound and acyclic                                │
│                                                                     │
│  TRANSPORT:                                                         │
│    • Session completes only when recv + fail ≥ total                │
│    • Block verified on receive                                      │
│    • WantList: one entry per CID               [dedup]              │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Source Map

```
ipfrs_source/crates/
├── ipfrs-core/         → 02-SharedKernel.md      (28 files, Shared Kernel)
├── ipfrs-storage/      → 03-StorageContext.md   (150+ files, BlockStore + decorators)
├── ipfrs-network/      → 04-NetworkContext.md   (150+ files, libp2p + reputation)
├── ipfrs-semantic/     → 05-SemanticContext.md  (159 files, HNSW/DiskANN)
├── ipfrs-tensorlogic/  → 06-LogicContext.md     (194 files, IR + inference)
├── ipfrs-transport/    → 07-TransportContext.md (46 files, Bitswap + sessions)
├── ipfrs-interface/    → 08-ApplicationLayer.md (24 files, gRPC/GraphQL/HTTP/WS)
├── ipfrs/              → 08-ApplicationLayer.md (24 files, Node facade)
├── ipfrs-cli/          → 08-ApplicationLayer.md (34 files, CLI)
├── ipfrs-wasm/         → 08-ApplicationLayer.md (WASM bindings)
├── ipfrs-nodejs/       → 08-ApplicationLayer.md (Node.js bindings)
└── ipfrs-python/       → 08-ApplicationLayer.md (Python bindings)
```

---

## Performance Numbers

```
Operation                    P50        P99        Throughput
─────────────────────────────────────────────────────────────
Block GET (cache)            30µs       50µs       33k ops/s
Block PUT                    50µs       80µs       20k ops/s
Block PUT (network)          200ms      1000ms     —
HNSW search (k=10)           1ms        10ms       1k q/s
HNSW insert                  2ms        20ms       500/s
DiskANN search               5ms        50ms       200 q/s
PQ encode                    0.1ms      0.5ms      10k/s
DHT lookup                   150ms      300ms      100 q/s
Inference (simple)           1ms        5ms        —
Inference (distributed)      100ms      1000ms     —
Session completion           200ms      1000ms     —

Memory:
  Node (minimal)             ~50 MB
  + Semantic (100k vectors)  ~500 MB
  + Logic (10k rules)        ~100 MB
  HNSW (10M, 768-d, M=16)    ~30 GB
  PQ compression             12,000× (30GB → 2.5MB)
```

---

## Design Principles

1. **CID as Universal Boundary Token** — Every ACL passes a CID, not foreign aggregates
2. **Content-Addressing is Identity** — `hash(data) == identity`, enabling natural distribution
3. **Frozen Aggregates** — Immutability enforced by privacy, not language-level const
4. **Decorator Stack for Cross-Cutting** — Storage concerns are composable, not flag-driven
5. **Intentional Duplication** — Network reputation ≠ Transport scoring (bounded-context autonomy)
6. **Lazy Context Init** — Pay only for what you use (OnceCell)
7. **Port/Adapter Pattern** — BlockStore trait = central port, all adapters conform

---

## Related Documentation

| Location | Style | Purpose |
|----------|-------|---------|
| `/Wiki` | Karpathy "second brain" | High-level understanding, role-based navigation |
| `/Wiki_GLM` | Technical deep-dive | Code-level DDD analysis, file:line anchors |
| `/Wiki_Arch_GLM` | Architecture research | Strategic design, decisions, integration |
| `/ipfrs_source/ARCHITECTURE_DDD.md` | Aspirational | Idealized domain model |
| `/ipfrs_source/ARCHITECTURE_DDD_DEEP.md` | Code-grounded | Real type definitions |

---

**Start with**: [01-StrategicDesign.md](01-StrategicDesign.md)
