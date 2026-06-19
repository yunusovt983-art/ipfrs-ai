# Context Integration — Cross-Context Flows, ACL Patterns

> **Focus**: How bounded contexts communicate, ACL patterns, event flows

---

## 1. Integration Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                    CONTEXT INTEGRATION MAP                          │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│                      ┌──────────────┐                               │
│                      │  SHARED      │                               │
│                      │  KERNEL      │◄───────────────────────┐      │
│                      └──────┬───────┘                        │      │
│                             │                                │      │
│    ┌────────────────────────┼────────────────────────┐       │      │
│    │                        │                        │       │      │
│    ▼                        ▼                        ▼       │      │
│ ┌──────────┐          ┌───────────┐          ┌───────────┐   │      │
│ │ STORAGE  │◄─────────│  NETWORK  │─────────►│ SEMANTIC  │   │      │
│ │          │   ACL    │           │   ACL    │           │   │      │
│ │BlockStore│          │ PeerId    │          │ VectorIdx │   │      │
│ │  trait   │          │ DHT       │          │           │   │      │
│ └────┬─────┘          └─────┬─────┘          └─────┬─────┘   │      │
│      │                      │                      │         │      │
│      │         ┌────────────┴────────────┐         │         │      │
│      │         │       TRANSPORT          │        │         │      │
│      └────────►│                          │◄───────┘         │      │
│                │  Session, Bitswap        │                  │      │
│                │  ACL: BlockStore, PeerId │                  │      │
│                └────────────┬─────────────┘                  │      │
│                             │                                │      │
│                  ┌──────────▼──────────┐                     │      │
│                  │       LOGIC         │                     │      │
│                  │                     │                     │      │
│                  │  KB, Inference      │─────────────────────┘      │
│                  │  ACL: BlockStore    │   (neural-symbolic)        │
│                  │       IPLD codec    │                            │
│                  └─────────────────────┘                            │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 2. ACL Patterns

### 2.1 libp2p → Domain (Network ACL)

```rust
// network/identity.rs
pub fn peer_id_to_string(peer_id: &libp2p::PeerId) -> String {
    peer_id.to_base58()
}

pub fn multiaddr_to_string(addr: &Multiaddr) -> String {
    addr.to_string()
}
```

**Why**: Domain types should not leak infrastructure (libp2p).

---

### 2.2 Storage → Transport (BlockStore ACL)

```rust
// transport/bitswap.rs
pub struct BitswapExchange<S: BlockStore> {
    store: Arc<S>,  // Knows only trait, not Sled
}

// Transport never imports SledBlockStore
```

**Why**: Transport can use any BlockStore implementation.

---

### 2.3 Logic → Storage (IPLD Published Language)

```rust
// tensorlogic/ipld_codec.rs
impl Rule {
    pub fn to_block(&self) -> Result<Block> {
        let ipld = self.to_ipld();
        let bytes = DagCborCodec::encode(&ipld)?;
        Block::new(bytes.into())
    }
    
    pub fn from_block(block: &Block) -> Result<Self> {
        let ipld = DagCborCodec::decode(block.data())?;
        Self::from_ipld(&ipld)
    }
}
```

**Why**: Rules are content-addressed blocks, shareable over Bitswap.

---

### 2.4 Bindings → Application (FFI ACL)

```rust
// interface/ffi.rs
#[repr(C)]
pub struct IpfrsClient {
    inner: *mut c_void,  // Opaque Arc<Node>
}

#[no_mangle]
pub extern "C" fn ipfrs_client_add(client: *mut IpfrsClient, ...) -> IpfrsBlock {
    catch_unwind(|| {
        let node = unsafe { &*(client.inner as *const Arc<Node>) };
        // ...
    }).unwrap_or(IpfrsBlock::null())
}
```

**Why**: Isolate FFI boundary, catch panics, opaque pointers.

---

## 3. Cross-Context Data Flows

### 3.1 Add File Flow

```
CLI/HTTP → Node.add_file
    │
    ├─► tokio::fs::read(file)
    │
    ├─► Chunker.chunk(data) → Vec<Bytes>
    │
    ├─► Block::new(bytes) → Block (CID computed)
    │
    ├─► Storage.put(block)
    │       │
    │       └─► Decorator stack: Cache → Dedup → Sled
    │
    ├─► StorageEvent::BlockAdded (observability)
    │
    ├─► (optional) Semantic.index_content(cid, embedding)
    │
    └─► (optional) Network.provide(cid) → DHT announce
```

---

### 3.2 Get Block Flow (Cache Miss → Network)

```
Node.get(cid)
    │
    ├─► Storage.get(cid)
    │       │
    │       ├─► Cache miss
    │       │
    │       └─► Sled miss
    │
    ├─► Transport.SessionManager.create_session([cid])
    │       │
    │       ├─► WantList.add(cid, priority)
    │       │
    │       ├─► Network.find_providers(cid) → peers
    │       │
    │       ├─► PeerManager.select_peers(cid)
    │       │
    │       ├─► Bitswap WantList → peers
    │       │
    │       └─► receive Block
    │               │
    │               ├─► block.verify() (core invariant)
    │               │
    │               ├─► Storage.put(block)
    │               │
    │               └─► Session.mark_received(cid)
    │
    └─► Return Block
```

---

### 3.3 Semantic Search Flow

```
Node.search_similar(query, k)
    │
    ├─► EmbeddingPipeline.normalize(query)
    │
    ├─► SemanticRouter.search(query, k)
    │       │
    │       ├─► QueryPlanner.plan(query)
    │       │       │
    │       │       └─► Strategy: LocalOnly | Hybrid | RemoteFanout
    │       │
    │       ├─► VectorIndex.search(query, k)
    │       │       │
    │       │       └─► HNSW greedy descent
    │       │
    │       ├─► (optional) RemoteFanout
    │       │       │
    │       │       └─► Network.SemanticDht.query(embedding)
    │       │
    │       └─► ReRanker.rerank(results)
    │
    └─► Return Vec<SearchResult>
```

---

### 3.4 Logic Inference Flow

```
Node.infer(goal)
    │
    ├─► TensorLogicStore.query(goal)
    │       │
    │       ├─► InferenceEngine.query(goal, kb)
    │       │       │
    │       │       ├─► Local KB lookup
    │       │       │
    │       │       └─► (optional) DistributedBackwardChainer
    │       │               │
    │       │               ├─► Network.find_providers(rule_cid)
    │       │               │
    │       │               ├─► Remote query_peer
    │       │               │
    │       │               └─► Merge proofs
    │       │
    │       └─► (optional) NeuralSymbolicIntegrator
    │               │
    │               ├─► Semantic.search_similar(embedding)
    │               │
    │               └─► Blend symbolic + neural
    │
    └─► Return Vec<Substitution>
```

---

## 4. Event Flows

### 4.1 Storage Events

```rust
pub enum StorageEvent {
    BlockAdded { cid: Cid, size: usize },
    BlockDeleted { cid: Cid },
    GcCompleted { deleted: usize },
}
```

**Consumers**: Observability, metrics, WebSocket subscriptions.

---

### 4.2 Network Events

```rust
pub enum NebNetworkEvent {
    PeerDiscovered { peer_id: String },
    PeerDisconnected { peer_id: String },
    DhtQueryCompleted { query_id: u64 },
}
```

**Consumers**: PeerStore updates, metrics.

---

### 4.3 Transport Events

```rust
pub enum SessionEvent {
    BlockReceived { cid: Cid, peer: String },
    SessionCompleted { session_id: u64 },
}

pub enum TransportEvent {
    PartitionDetected { peers: Vec<String> },
    CircuitBreakerOpened { peer: String },
}
```

**Consumers**: SessionManager, metrics, WebSocket.

---

## 5. Intentional Duplication

### 5.1 Reputation

| Context | Model | Scope |
|---------|-------|-------|
| Network | EWMA + Trust Graph | Days/weeks |
| Transport | EWMA | Minutes/hours |

**Rationale**: Different concerns, bounded-context autonomy.

---

## 6. Integration Patterns Summary

| Pattern | From → To | Example |
|---------|-----------|---------|
| Shared Kernel | Core → All | `Cid`, `Block` |
| Conformist | All → Storage | `BlockStore` trait |
| Customer/Supplier | Transport → Network | `PeerId` |
| ACL | Network → libp2p | String wrappers |
| Published Language | Logic → Storage | IPLD codec |
| Facade | Presentation → Application | `Node` |

---

**Next**: [10-DesignDecisions.md](10-DesignDecisions.md) — Trade-offs, rationale, philosophy
