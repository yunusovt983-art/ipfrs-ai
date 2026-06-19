# Design Decisions — Trade-offs, Rationale, Philosophy

> **Focus**: Why decisions were made, trade-offs, alternatives considered

---

## 1. CID as Universal Boundary Token

### Decision

`Cid` — единственный cross-context reference mechanism.

### Rationale

- Content-addressing = natural distribution
- No foreign aggregates
- ACLs are cheap (pass a CID)

### Trade-off

Everything must be hashable/serializable to a block.

### Alternatives Considered

| Alternative | Why Rejected |
|-------------|--------------|
| Foreign keys (database-style) | Requires centralized registry |
| UUID references | No content verification |
| Object IDs | Not content-addressed |

---

## 2. Modular Monolith

### Decision

Single deployable unit, multiple crates.

### Rationale

- Simpler deployment
- Shared memory, no network overhead
- Cargo workspace manages dependencies

### Trade-off

Cannot scale contexts independently.

### Alternatives Considered

| Alternative | Why Rejected |
|-------------|--------------|
| Microservices | Network overhead, complexity |
| Plugin architecture | Rust dynamic loading limitations |

---

## 3. Decorator Stack for Storage

### Decision

Cross-cutting concerns as stacked `BlockStore`s.

### Rationale

- Composable, testable
- Enable/disable independently
- Open/Closed Principle

### Trade-off

Per-op indirection; deep stacks harder to debug.

### Alternatives Considered

| Alternative | Why Rejected |
|-------------|--------------|
| Config flags | Monolithic, hard to test |
| Middleware chain | Less type-safe |

---

## 4. Two-Tier Reputation

### Decision

Network (EWMA + Trust Graph) ≠ Transport (EWMA).

### Rationale

- Different concerns: routing trust vs transfer quality
- Bounded-context autonomy
- Independent evolution

### Trade-off

Duplicate logic, higher complexity.

### Alternatives Considered

| Alternative | Why Rejected |
|-------------|--------------|
| Shared reputation | Couples contexts |
| Single scoring model | Conflates concerns |

---

## 5. Frozen Aggregates

### Decision

`Block`, `TensorBlock` are immutable by privacy.

### Rationale

- Content-addressing requires immutability
- `Bytes` makes `Clone` O(1)
- Thread-safe by construction

### Trade-off

`from_parts()` escape hatch requires trust.

### Alternatives Considered

| Alternative | Why Rejected |
|-------------|--------------|
| `const` types | Rust limitations |
| Builder pattern | Doesn't enforce invariants |

---

## 6. HNSW + DiskANN Alternatives

### Decision

Two index aggregates: in-memory and disk-based.

### Rationale

- HNSW: Best recall/latency for <10M
- DiskANN: Billion-scale with constant RAM
- Operator chooses based on scale

### Trade-off

Two code paths, maintenance overhead.

### Alternatives Considered

| Alternative | Why Rejected |
|-------------|--------------|
| ScaNN only | TensorFlow dependency |
| Only HNSW | RAM-bounded |
| Only DiskANN | Higher latency |

---

## 7. Neural-Symbolic Fusion

### Decision

Blend rules + embeddings in `Hybrid` mode.

### Rationale

- Rules: Interpretable, verifiable
- Embeddings: Approximate, fuzzy
- Hybrid: Best of both

### Trade-off

Complexity, two inference paths.

### Alternatives Considered

| Alternative | Why Rejected |
|-------------|--------------|
| Pure symbolic | No fuzzy matching |
| Pure neural | Not interpretable |

---

## 8. State Mutation + Journals

### Decision

Mutable stores with WAL/transaction-log, NOT event sourcing.

### Rationale

- Throughput matters for storage/network
- Crash recovery without rebuild cost

### Trade-off

No audit trail by default.

### Alternatives Considered

| Alternative | Why Rejected |
|-------------|--------------|
| Event sourcing | Rebuild cost, complexity |
| Append-only | Storage overhead |

---

## 9. Lazy Context Init

### Decision

`OnceCell` for semantic, tensorlogic.

### Rationale

- Pay only for what you use
- Node that only stores blocks pays nothing for semantic/logic

### Trade-off

First-use latency spike.

### Alternatives Considered

| Alternative | Why Rejected |
|-------------|--------------|
| Eager init | Unnecessary overhead |
| Dynamic loading | Complexity |

---

## 10. BTreeMap in Ipld

### Decision

`Ipld::Map` uses `BTreeMap`, not `HashMap`.

### Rationale

- Sorted keys → canonical encoding
- Two maps with same entries = same CID
- Critical for cross-node consistency

### Trade-off

Slightly slower insert than `HashMap`.

### Alternatives Considered

| Alternative | Why Rejected |
|-------------|--------------|
| HashMap | Non-deterministic order |
| IndexMap | Extra dependency |

---

## 11. Soft Delete in HNSW

### Decision

Unmap CID, keep node in graph.

### Rationale

- True delete requires graph rewiring (O(M²))
- Breaks concurrent search
- Tombstones are rare

### Trade-off

Memory leak for heavy delete workloads.

### Alternatives Considered

| Alternative | Why Rejected |
|-------------|--------------|
| Hard delete | Complexity, breaks search |
| Tombstone markers | Same effect |

---

## 12. Product Quantization over OPQ

### Decision

PQ for compression, not OPQ.

### Rationale

- PQ: Simple, effective, 12,000× compression
- OPQ: Requires learned rotation (complexity)

### Trade-off

Slightly lower recall than OPQ.

### Alternatives Considered

| Alternative | Why Rejected |
|-------------|--------------|
| OPQ | Complexity |
| ScaNN | TensorFlow dependency |
| No compression | RAM-limited |

---

## 13. Multi-Transport Fallback

### Decision

QUIC → WebTransport → TCP → WebSocket.

### Rationale

- QUIC: Best performance
- WebTransport: Browser support
- TCP: Universal
- WebSocket: Firewall traversal

### Trade-off

Complexity, multiple code paths.

### Alternatives Considered

| Alternative | Why Rejected |
|-------------|--------------|
| QUIC only | Firewall issues |
| TCP only | Performance |

---

## 14. Float-as-String in IR

### Decision

`Constant::Float(String)` for deterministic hash.

### Rationale

- IEEE 754 bit variations break content-addressing
- Two floats that print the same = same CID

### Trade-off

Parsing overhead.

### Alternatives Considered

| Alternative | Why Rejected |
|-------------|--------------|
| f64 directly | Non-deterministic |
| Decimal type | Extra dependency |

---

## 15. Principles Summary

| Principle | Manifestation |
|-----------|---------------|
| Content-addressing | CID as universal token |
| Immutability | Frozen aggregates, `Bytes` |
| Bounded-context autonomy | Intentional duplication |
| Composition | Decorator stack |
| Lazy evaluation | `OnceCell` |
| Canonical encoding | `BTreeMap`, Float-as-String |
| Graceful degradation | Multi-transport fallback |

---

**Next**: [11-PerformanceModel.md](11-PerformanceModel.md) — Bottlenecks, scaling, optimization
