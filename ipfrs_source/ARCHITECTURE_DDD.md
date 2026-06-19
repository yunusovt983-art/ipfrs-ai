# IPFRS Architecture - Domain-Driven Design (DDD)

**Version**: 0.2.0 "Network Release"  
**Status**: Production Ready — P2P Networking Available  
**Last Updated**: 2026-06-18

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Domain Overview](#domain-overview)
3. [Bounded Contexts](#bounded-contexts)
4. [Core Domain Models](#core-domain-models)
5. [Subdomain Relationships](#subdomain-relationships)
6. [Architecture Layers](#architecture-layers)
7. [Key Concepts & Invariants](#key-concepts--invariants)
8. [Aggregate Design](#aggregate-design)
9. [Repository Pattern](#repository-pattern)
10. [Services & Application Layer](#services--application-layer)
11. [Event-Driven Architecture](#event-driven-architecture)
12. [Technical Implementation](#technical-implementation)

---

## Executive Summary

IPFRS is a **distributed file system** that unifies **intelligent storage** with **distributed reasoning** via content-addressing and semantic capabilities. It follows Domain-Driven Design principles to clearly separate concerns between:

- **Storage Domain**: Block persistence, content-addressing, and zero-copy I/O
- **Network Domain**: P2P communication, peer discovery, and block exchange
- **Semantic Domain**: Vector search, similarity matching, and query optimization
- **Logic Domain**: Content-addressed reasoning, inference, and rule evaluation
- **Transport Domain**: Protocol-level coordination, session management, and reliability

### Key Principles

- **Content-Addressed**: Every piece of data has a cryptographic identity (CID)
- **Distributed**: No central authority; peer-to-peer communication via libp2p
- **Intelligent**: Semantic search (HNSW) + Logic Programming (TensorLogic)
- **Pure Rust**: Memory-safe, zero-unsafe-code-where-possible, high-performance
- **Zero-Copy**: Apache Arrow integration for tensor streaming

---

## Domain Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Problem Space                                 │
├─────────────────────────────────────────────────────────────────────┤
│  How do we build a global distributed knowledge mesh that unifies    │
│  human data (storage) with machine intelligence (reasoning)?         │
└─────────────────────────────────────────────────────────────────────┘

Answered by Five Interconnected Domains:

┌─────────────────────────────────────────────────────────────────────┐
│                      STORAGE DOMAIN                                  │
│  "Immutable blocks identified by cryptographic hash"                 │
│  - Core entities: Block, CID, Dag                                    │
│  - Invariant: Block data = Hash(content)                             │
│  - Impl: Sled embedded DB, Apache Arrow                              │
└─────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────┐
│                      NETWORK DOMAIN                                  │
│  "Decentralized peer discovery and content routing"                  │
│  - Core entities: Peer, PeerId, Multiaddr                            │
│  - Invariant: Every node has a unique PeerId                         │
│  - Impl: libp2p, DHT, QUIC transport                                 │
└─────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────┐
│                     SEMANTIC DOMAIN                                  │
│  "Intelligent search via vector similarity"                          │
│  - Core entities: Embedding, Similarity, SearchResult                │
│  - Invariant: Similarity in [0, 1] range                             │
│  - Impl: HNSW index, distance metrics, LRU cache                     │
└─────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────┐
│                      LOGIC DOMAIN                                    │
│  "Content-addressed reasoning with rules and predicates"             │
│  - Core entities: Term, Predicate, Rule, Fact                        │
│  - Invariant: Rule consistency (no contradictions)                   │
│  - Impl: TensorLogic IR, inference engine                            │
└─────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────┐
│                     TRANSPORT DOMAIN                                 │
│  "Protocol-level coordination of block exchange and reliability"     │
│  - Core entities: Session, Message, WantList, Peer                   │
│  - Invariant: FIFO message delivery per peer                         │
│  - Impl: Bitswap protocol, sessions, circuit breakers                │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Bounded Contexts

### 1. Storage Bounded Context

**Responsibility**: Persisting and retrieving immutable blocks with content-addressing

**Language**: Block, CID, DAG, IPLD, Sled

**Key Abstractions**:
```rust
// Core Aggregate: Block
pub struct Block {
    cid: Cid,           // Content Identifier (hash identity)
    data: Bytes,        // Immutable payload
    metadata: Metadata, // Size, type, timestamp
}

// Repository
pub trait Blockstore {
    async fn put(&self, block: Block) -> Result<Cid>;
    async fn get(&self, cid: &Cid) -> Result<Option<Block>>;
    async fn has(&self, cid: &Cid) -> Result<bool>;
}

// Value Objects
pub struct Cid { /* Content Identifier */ }
pub struct Metadata { /* Block metadata */ }
```

**Relationships**:
- ← **Network**: Receives block requests from peers
- → **Semantic**: Indexes blocks for search
- → **Logic**: Stores logical terms and rules
- ← **Transport**: Fulfills block retrieval requests

**Example Flow**:
```
User.add_file("document.pdf")
    └─→ Storage.compute_cid(data)
    └─→ Storage.put(Block { cid, data })
    └─→ Network.announce(cid)  // Content routing
```

---

### 2. Network Bounded Context

**Responsibility**: Decentralized peer discovery, DHT routing, and peer management

**Language**: Peer, PeerId, Multiaddr, Capability, Peer Discovery, DHT

**Key Abstractions**:
```rust
// Core Aggregate: Peer
pub struct Peer {
    peer_id: PeerId,
    multiaddrs: Vec<Multiaddr>,
    reputation: Score,
    known_blocks: HashSet<Cid>,
}

// Repository
pub trait PeerStore {
    async fn add_peer(&self, peer: Peer) -> Result<()>;
    async fn get_peer(&self, peer_id: &PeerId) -> Result<Option<Peer>>;
    async fn find_peers_with(&self, cid: &Cid) -> Result<Vec<Peer>>;
}

// Value Objects
pub struct PeerId { /* Unique peer identifier */ }
pub struct Score { /* Reputation score */ }
```

**Responsibilities**:
- **Peer Discovery**: Bootstrap, DHT, mDNS, relay discovery
- **Content Routing**: "Who has block X?"
- **Reputation Management**: Track peer performance
- **Connection Management**: Establish/maintain connections

**Relationships**:
- ← **Storage**: Gets announced blocks for indexing
- → **Transport**: Provides peer list for block requests
- ↔ **Other Peers**: P2P communication via libp2p

**Example Flow**:
```
Network.bootstrap()
    └─→ DHT.lookup(content_hash)
    └─→ Peer.record_capability(cid)
    └─→ Transport.establish_connection(peer)
```

---

### 3. Semantic Bounded Context

**Responsibility**: Vector search, similarity matching, and semantic indexing

**Language**: Embedding, Similarity, Vector, Index, SearchResult, Metric

**Key Abstractions**:
```rust
// Core Aggregate: SemanticIndex
pub struct SemanticIndex {
    hnsw: HnswIndex<Vec<f32>>,
    dimension: usize,
    cache: LruCache<Vec<f32>, Vec<SearchResult>>,
}

// Repository
pub trait SemanticStore {
    async fn index(&self, cid: &Cid, embedding: Vec<f32>) -> Result<()>;
    async fn search(&self, query: Vec<f32>, k: usize) -> Result<Vec<SearchResult>>;
    async fn get_stats(&self) -> Result<SemanticStats>;
}

// Value Objects
pub struct Embedding(Vec<f32>);
pub struct SearchResult {
    cid: Cid,
    similarity: f32,  // [0.0, 1.0]
}
```

**Capabilities**:
- **HNSW Indexing**: Fast approximate k-NN search
- **Distance Metrics**: Cosine, L2, etc.
- **Query Caching**: LRU cache for hot queries
- **Filtering**: Score thresholds, prefix filters

**Relationships**:
- ← **Storage**: Receives blocks to index
- → **Application**: Provides search results
- → **Transport**: Semantically guides peer selection

**Example Flow**:
```
User.search_similar(embedding)
    └─→ SemanticIndex.search(embedding, k=10)
    └─→ Cache.lookup() or
    └─→ Hnsw.knn_search()
    └─→ return SearchResults
```

---

### 4. Logic Bounded Context

**Responsibility**: Content-addressed reasoning with terms, predicates, rules, and inference

**Language**: Term, Predicate, Rule, Fact, Substitution, Unification

**Key Abstractions**:
```rust
// Core Aggregate: LogicProgram
pub struct LogicProgram {
    facts: HashSet<Fact>,
    rules: Vec<Rule>,
    terms: HashMap<Cid, Term>,
}

// Repository
pub trait LogicStore {
    async fn put_term(&self, term: &Term) -> Result<Cid>;
    async fn get_term(&self, cid: &Cid) -> Result<Option<Term>>;
    async fn assert_rule(&self, rule: Rule) -> Result<()>;
    async fn query(&self, goal: &Goal) -> Result<Vec<Substitution>>;
}

// Value Objects
pub enum Term {
    Constant(String),
    Variable(String),
    Compound(String, Vec<Term>),
}

pub struct Predicate {
    name: String,
    args: Vec<Term>,
}

pub struct Rule {
    head: Predicate,
    body: Vec<Predicate>,
}
```

**Capabilities**:
- **Unification**: Pattern matching on terms
- **Inference**: Forward/backward chaining
- **Temporal Reasoning**: Rules with time constraints
- **Abductive Reasoning**: Hypothetical explanation

**Relationships**:
- ← **Storage**: Stores serialized terms
- ↔ **Semantic**: Can combine logic with vector search
- → **Network**: Distributes inference across peers

**Example Flow**:
```
User.assert_rule(parent(X, Y) :- father(X, Y) | mother(X, Y))
User.query(ancestor(alice, ?))
    └─→ LogicEngine.unify()
    └─→ LogicEngine.infer()
    └─→ return Substitutions
```

---

### 5. Transport Bounded Context

**Responsibility**: Protocol-level coordination, session management, and reliable block exchange

**Language**: Session, Message, WantList, Block Exchange, Bitswap, Peer Scoring

**Key Abstractions**:
```rust
// Core Aggregate: BlockExchangeSession
pub struct BlockExchangeSession {
    session_id: SessionId,
    cids: Vec<Cid>,
    peers: Vec<Peer>,
    want_list: WantList,
    progress: Progress,
    state: SessionState,
}

// Repository
pub trait SessionStore {
    async fn create(&self, session: BlockExchangeSession) -> Result<SessionId>;
    async fn get(&self, id: &SessionId) -> Result<Option<BlockExchangeSession>>;
    async fn update(&self, session: BlockExchangeSession) -> Result<()>;
}

// Value Objects
pub enum SessionState {
    Created,
    Active,
    Paused,
    Completed,
    Cancelled,
}

pub struct Message {
    msg_type: MessageType,
    payload: Bytes,
}
```

**Responsibilities**:
- **Session Management**: Batching, prioritization, lifecycle
- **Want List Management**: Priority queues, deduplication, timeouts
- **Peer Scoring**: Select best peers for requests
- **Message Handling**: Serialize/deserialize, routing
- **Reliability**: Retries, circuit breakers, error handling
- **Concurrency**: Async message passing, shared state

**Relationships**:
- → **Storage**: Retrieves/stores blocks
- → **Network**: Selects peers, manages connections
- ← **Application**: Receives block requests, sends progress events

**Example Flow**:
```
User.request_blocks([cid1, cid2, cid3])
    └─→ TransportSession.create()
    └─→ WantListManager.add_to_priority_queue()
    └─→ PeerManager.select_best_peer()
    └─→ Transport.send(want_list)
    └─→ (network communication)
    └─→ MessageHandler.process_block()
    └─→ User.on_block_received()
```

---

## Core Domain Models

### Central Value Object: Content Identifier (CID)

**Purpose**: Cryptographic identity of content

```rust
pub struct Cid {
    version: CidVersion,           // v0 or v1
    codec: Codec,                  // raw, cbor, protobuf, etc.
    multihash: Multihash,          // hash algorithm + digest
}

impl Cid {
    // Invariants
    // 1. CID is deterministic: same content → same CID
    // 2. CID is immutable: cannot change without changing identity
    // 3. CID is unique: collision-resistant (SHA-256)
}
```

**Axioms**:
- **Determinism**: `hash(data) == hash(data')` iff `data == data'`
- **Uniqueness**: Collision probability ≈ 1 in 2^256
- **Immutability**: CID cannot change without being a different CID

### Central Aggregate: Block

**Purpose**: Immutable unit of storage with content-addressing

```rust
pub struct Block {
    cid: Cid,
    data: Bytes,
    metadata: BlockMetadata,
}

impl Block {
    // Invariants
    // 1. CID matches data: verify_cid(self.cid, self.data)?
    // 2. Data is immutable: &self.data (no &mut)
    // 3. Metadata is accurate: size = data.len()
}
```

**Lifecycle**:
```
User Input → Validation → CID Computation → Block Creation → Storage
```

### Central Aggregate: DAG (Directed Acyclic Graph)

**Purpose**: Structured data representation with content-addressing

```rust
pub enum Ipld {
    Null,
    Bool(bool),
    Integer(i128),
    Float(f64),
    String(String),
    Bytes(Vec<u8>),
    List(Vec<Ipld>),
    Map(BTreeMap<String, Ipld>),
    Link(Cid),  // Reference to another block
}

pub struct DagNode {
    data: Ipld,
    cid: Cid,
    children: Vec<Cid>,  // Child block references
}
```

**Invariants**:
- Each link points to a valid block
- No circular references
- Deterministic CBOR encoding

---

## Subdomain Relationships

### Context Map

```
                    ┌─────────────────────────────────────┐
                    │     Application/User Interface       │
                    └──────────────┬──────────────────────┘
                                   │
                    ┌──────────────┴──────────────────────┐
                    │                                      │
        ┌───────────▼──────────┐          ┌──────────────▼─────────┐
        │   STORAGE DOMAIN     │          │   SEMANTIC DOMAIN      │
        │  (Blockstore, CID)   │◄────────▶│  (Vector Search, HNSW) │
        └─────────┬──────┬─────┘          └──────────┬────────┬────┘
                  │      │                           │        │
                  │      │        ┌──────────────────┘        │
                  │      │        │                           │
        ┌─────────▼──────▼────┐   │         ┌─────────────────▼──┐
        │  TRANSPORT DOMAIN   │   │         │   LOGIC DOMAIN     │
        │  (Sessions, Blocks) │   │         │  (Rules, Inference)│
        │  ◄────────────────────┐ │         └─────────┬──────────┘
        └────────┬──────┬───────┘ │                   │
                 │      │         │                   │
                 │      │         └───────────────────┘
                 │      │
        ┌────────▼──────▼─────┐
        │   NETWORK DOMAIN    │
        │  (Peers, DHT, libp2p)│
        └─────────────────────┘
```

### Anti-Corruption Layers (ACL)

**Storage ↔ Network**:
- Storage exposes `Blockstore` interface
- Network uses this interface without knowing implementation
- CID is the language bridge

**Storage ↔ Semantic**:
- Semantic consumes `Block` from Storage
- Converts block to `Embedding` (own domain)
- Stores embedding in `SemanticIndex`

**Transport ↔ Network**:
- Transport provides `SessionManager` interface
- Network provides `PeerProvider` interface
- Message types are marshaled via codecs

---

## Architecture Layers

```
┌──────────────────────────────────────────────────────────────────┐
│                    PRESENTATION LAYER                             │
│   HTTP API (Axum), CLI (Clap), WASM, Node.js, Python bindings    │
├──────────────────────────────────────────────────────────────────┤
│                   APPLICATION LAYER                               │
│   Use Cases: add_file, get_file, search, query, stream_tensor    │
│   Orchestration: Combine storage + semantic + logic + network     │
├──────────────────────────────────────────────────────────────────┤
│                    DOMAIN LAYER                                   │
│  ┌─────────────┐ ┌──────────────┐ ┌──────────┐ ┌────────────┐  │
│  │   Storage   │ │   Semantic   │ │  Logic   │ │  Transport │  │
│  │   Domain    │ │   Domain     │ │  Domain  │ │  Domain    │  │
│  └─────────────┘ └──────────────┘ └──────────┘ └────────────┘  │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │           Network Domain (Peers, DHT, P2P)               │   │
│  └──────────────────────────────────────────────────────────┘   │
├──────────────────────────────────────────────────────────────────┤
│                  INFRASTRUCTURE LAYER                             │
│  Sled (Embedded DB), libp2p, HNSW, Tokio (Async Runtime)         │
└──────────────────────────────────────────────────────────────────┘
```

### Layer Responsibilities

**Presentation**:
- REST API endpoints
- CLI commands
- WASM/bindings marshaling
- HTTP serialization

**Application**:
- Use case orchestration
- Transaction handling
- Input validation
- Response assembly

**Domain**:
- Business logic encapsulation
- Entity/aggregate definitions
- Invariant enforcement
- Value object definitions

**Infrastructure**:
- Database persistence
- Network I/O
- Index management
- Cache implementation

---

## Key Concepts & Invariants

### Content-Addressing Invariant

**Axiom**: `Hash(data) = CID`

```rust
fn add_block(data: &[u8]) -> Result<Cid> {
    let computed_cid = compute_cid(data)?;
    assert_eq!(computed_cid, block.cid);  // Invariant
    storage.store(block)?;
    Ok(computed_cid)
}
```

### Immutability Invariant

**Axiom**: Blocks cannot be mutated after creation

```rust
pub struct Block {
    data: Bytes,  // Immutable, no &mut access
}

// Cannot do: block.data[0] = 42;  (compile error)
```

### Distributed Consensus Invariant

**Axiom**: Same block must have same CID across all nodes

This is enforced by deterministic hashing and content-addressed storage.

### Peer Reputation Invariant

**Axiom**: Peer score reflects data delivery reliability

```rust
pub struct Peer {
    reputation: Score,  // Updated on success/failure
    successful_blocks: u64,
    failed_blocks: u64,
}

impl Peer {
    fn update_score(&mut self, success: bool) {
        if success {
            self.reputation += REWARD;
        } else {
            self.reputation = (self.reputation * 0.95).max(0);
        }
    }
}
```

### Semantic Index Invariant

**Axiom**: Search result similarity ∈ [0.0, 1.0]

```rust
pub struct SearchResult {
    cid: Cid,
    similarity: f32,  // Normalized to [0.0, 1.0]
}

assert!(similarity >= 0.0 && similarity <= 1.0);
```

---

## Aggregate Design

### Storage Aggregate: Block

**Root**: `Block`

**Invariants**:
- CID matches content hash
- Data is immutable
- Metadata is consistent

```rust
pub struct Block {
    cid: Cid,
    data: Bytes,
    metadata: BlockMetadata,
}

impl Block {
    pub fn new(cid: Cid, data: Bytes) -> Result<Self> {
        let computed = compute_cid(&data)?;
        ensure!(computed == cid, "CID mismatch");
        
        Ok(Block {
            cid,
            data,
            metadata: BlockMetadata::new(data.len()),
        })
    }
}
```

### Network Aggregate: Peer

**Root**: `Peer`

**Invariants**:
- PeerId is unique
- Reputation score is non-negative
- Multiaddrs are valid

```rust
pub struct Peer {
    peer_id: PeerId,
    multiaddrs: Vec<Multiaddr>,
    reputation: Score,
    last_seen: Instant,
}

impl Peer {
    pub fn is_healthy(&self) -> bool {
        self.reputation > REPUTATION_THRESHOLD
    }
}
```

### Transport Aggregate: BlockExchangeSession

**Root**: `BlockExchangeSession`

**Invariants**:
- Session ID is unique
- Requested CIDs are fixed
- State transitions are valid

```rust
pub struct BlockExchangeSession {
    session_id: SessionId,
    requested_cids: Vec<Cid>,
    received_blocks: HashSet<Cid>,
    state: SessionState,
}

impl BlockExchangeSession {
    pub fn complete(&mut self) -> Result<()> {
        ensure!(
            self.received_blocks.len() == self.requested_cids.len(),
            "Not all blocks received"
        );
        self.state = SessionState::Completed;
        Ok(())
    }
}
```

---

## Repository Pattern

### Storage Repository

```rust
#[async_trait]
pub trait Blockstore: Send + Sync {
    async fn put(&self, block: Block) -> Result<Cid>;
    async fn get(&self, cid: &Cid) -> Result<Option<Block>>;
    async fn has(&self, cid: &Cid) -> Result<bool>;
    async fn delete(&self, cid: &Cid) -> Result<()>;
}

// Implementation
pub struct SledBlockstore {
    db: sled::Db,
    cache: LruCache<Cid, Block>,
}
```

### Network Repository

```rust
#[async_trait]
pub trait PeerRepository: Send + Sync {
    async fn add_peer(&self, peer: Peer) -> Result<()>;
    async fn get_peer(&self, id: &PeerId) -> Result<Option<Peer>>;
    async fn find_peers_with(&self, cid: &Cid) -> Result<Vec<Peer>>;
    async fn update_reputation(&self, id: &PeerId, score: Score) -> Result<()>;
}
```

### Semantic Repository

```rust
#[async_trait]
pub trait SemanticIndex: Send + Sync {
    async fn index(&self, cid: &Cid, embedding: Vec<f32>) -> Result<()>;
    async fn search(&self, query: Vec<f32>, k: usize) -> Result<Vec<SearchResult>>;
    async fn remove(&self, cid: &Cid) -> Result<()>;
}
```

---

## Services & Application Layer

### Node Service (Main Orchestrator)

```rust
pub struct Node {
    storage: Arc<dyn Blockstore>,
    network: Arc<NetworkManager>,
    semantic: Arc<SemanticIndex>,
    logic: Arc<LogicEngine>,
    transport: Arc<TransportManager>,
}

impl Node {
    pub async fn add_file(&self, path: &Path) -> Result<Cid> {
        let data = tokio::fs::read(path).await?;
        let block = Block::from_data(data)?;
        let cid = self.storage.put(block.clone()).await?;
        
        // Announce to network
        self.network.announce(cid.clone()).await?;
        
        Ok(cid)
    }
    
    pub async fn search_similar(&self, embedding: Vec<f32>, k: usize) -> Result<Vec<SearchResult>> {
        self.semantic.search(embedding, k).await
    }
    
    pub async fn get(&self, cid: &Cid) -> Result<Option<Block>> {
        // Try local storage first
        if let Some(block) = self.storage.get(cid).await? {
            return Ok(Some(block));
        }
        
        // Request from network
        self.transport.request_from_network(cid).await
    }
}
```

### Application Use Cases

**UC1: Add File**
```
User Input: file_path
    ↓
Read File → Compute CID → Create Block → Store → Announce
    ↓
Output: CID
```

**UC2: Retrieve File**
```
User Input: CID
    ↓
Check Local → (if miss) Discover Peers → Request → Receive → Return
    ↓
Output: File Data
```

**UC3: Search Similar**
```
User Input: embedding vector
    ↓
Normalize → Query HNSW → Rank → Filter → Return
    ↓
Output: [SearchResult]
```

**UC4: Store Logic Rule**
```
User Input: Rule { head, body }
    ↓
Validate → Serialize → Create Block → Store → Announce
    ↓
Output: CID (Rule Reference)
```

---

## Event-Driven Architecture

### Domain Events

**Storage Domain**:
```rust
pub enum StorageEvent {
    BlockAdded(Cid),
    BlockRemoved(Cid),
    StorageQuotaExceeded,
}
```

**Network Domain**:
```rust
pub enum NetworkEvent {
    PeerDiscovered(Peer),
    PeerDisconnected(PeerId),
    ContentRoutingUpdated(Cid, Vec<PeerId>),
    PeerReputationChanged(PeerId, Score),
}
```

**Transport Domain**:
```rust
pub enum TransportEvent {
    SessionCreated(SessionId),
    BlockReceived(SessionId, Cid),
    SessionCompleted(SessionId),
    PeerFailed(SessionId, PeerId),
}
```

**Semantic Domain**:
```rust
pub enum SemanticEvent {
    IndexUpdated(Cid, Embedding),
    QueryExecuted(Query),
    CacheHit,
    CacheMiss,
}
```

### Event Publishing

```rust
impl Node {
    async fn on_block_received(&self, cid: Cid) -> Result<()> {
        // Event 1: Storage
        self.publish(StorageEvent::BlockAdded(cid.clone())).await?;
        
        // Event 2: Semantic indexing
        if let Some(embedding) = self.extract_embedding(&cid).await? {
            self.semantic.index(&cid, embedding).await?;
            self.publish(SemanticEvent::IndexUpdated(cid, embedding)).await?;
        }
        
        Ok(())
    }
}
```

---

## Technical Implementation

### Workspace Structure

```
ipfrs/                              # Main workspace
├── crates/
│   ├── ipfrs-core/                 # Core types (Block, CID, Error)
│   ├── ipfrs-storage/              # Storage domain (Sled backend)
│   ├── ipfrs-network/              # Network domain (libp2p, DHT)
│   ├── ipfrs-semantic/             # Semantic domain (HNSW, search)
│   ├── ipfrs-tensorlogic/          # Logic domain (TensorLogic)
│   ├── ipfrs-transport/            # Transport domain (Sessions, Bitswap)
│   ├── ipfrs-interface/            # HTTP gateway (Axum)
│   ├── ipfrs/                      # Main library (unified API)
│   ├── ipfrs-cli/                  # CLI (Clap)
│   ├── ipfrs-wasm/                 # WebAssembly bindings
│   ├── ipfrs-nodejs/               # Node.js bindings
│   └── ipfrs-python/               # Python bindings
└── Cargo.toml
```

### Key Dependencies (Pure Rust Policy)

```toml
# Storage
sled = "0.34"           # Embedded database

# Networking
libp2p = "0.56"         # P2P protocols
quinn = "0.11"          # QUIC protocol

# Semantic Search
hnsw_rs = "0.3"         # HNSW algorithm

# Async Runtime
tokio = "1.52"          # Async executor

# Compression (Pure Rust via COOLJAPAN OxiARC)
oxiarc-zstd = "0.3.3"   # Zstd compression
oxiarc-lz4 = "0.3.3"    # LZ4 compression

# Zero-Copy I/O
arrow = "59"            # Apache Arrow
```

### Error Handling Strategy

```rust
// Domain-specific errors
#[derive(thiserror::Error)]
pub enum StorageError {
    #[error("CID mismatch: expected {expected}, got {actual}")]
    CidMismatch { expected: Cid, actual: Cid },
    
    #[error("Block not found: {0}")]
    BlockNotFound(Cid),
    
    #[error("Storage error: {0}")]
    DatabaseError(#[from] sled::Error),
}

// Error propagation
pub type Result<T> = std::result::Result<T, Error>;

pub async fn add_file(path: &Path) -> Result<Cid> {
    let data = tokio::fs::read(path)
        .await
        .map_err(|e| Error::IoError(e))?;
    
    let block = Block::from_data(data)?;
    self.storage.put(block).await?;
    
    Ok(block.cid)
}
```

### Concurrency Model

**Thread Architecture** (via Tokio):
```
Worker Threads (8-16 based on CPU)
    ↓
    ├─ Accept Loop (new connections)
    ├─ Send Loop (outgoing messages)
    └─ Receive Loop (incoming messages)
       ↓
       └─ Spawned tasks (per-connection handlers)
```

**Shared State** (via Arc + DashMap):
```rust
pub struct Node {
    storage: Arc<dyn Blockstore>,           // Arc for sharing
    peers: Arc<DashMap<PeerId, Peer>>,      // Lock-free hashmap
    sessions: Arc<DashMap<SessionId, BlockExchangeSession>>,
    metrics: Arc<Metrics>,                  // Atomic counters
}
```

**Message Passing** (via mpsc):
```rust
let (tx, mut rx) = tokio::sync::mpsc::channel(1000);

// Producer
tx.send(BlockRequest { cid }).await?;

// Consumer
while let Some(request) = rx.recv().await {
    handle_request(request).await?;
}
```

---

## Design Patterns Used

### Aggregate Pattern
- `Block` (Storage domain)
- `Peer` (Network domain)
- `BlockExchangeSession` (Transport domain)

### Repository Pattern
- `Blockstore` (Storage)
- `PeerRepository` (Network)
- `SemanticIndex` (Semantic)

### Service Pattern
- `Node` (Main orchestrator)
- `NetworkManager` (Peer coordination)
- `TransportManager` (Session management)

### Observer Pattern
- `EventPublisher` (Domain events)
- `Subscriber` (Event listeners)

### Factory Pattern
- `BlockFactory.create(data)` → Block with CID
- `SessionFactory.create(cids)` → BlockExchangeSession

### Strategy Pattern
- `PeerScoring` (different scoring algorithms)
- `DistanceMetric` (L2, Cosine, etc.)
- `TransportSelector` (QUIC, TCP, WebSocket)

---

## Scalability Considerations

### Horizontal Scaling

1. **Peer Network**: Any number of peers (no central authority)
2. **Block Storage**: Limited only by disk space
3. **Semantic Index**: HNSW scales to millions of vectors
4. **Session Concurrency**: Tokio async handles thousands

### Vertical Scaling

1. **Cache**: LRU cache for hot blocks/queries
2. **Indexing**: HNSW provides sub-linear search
3. **Connection Pooling**: Reuse TCP/QUIC connections
4. **Batch Operations**: Session-based batching

### Performance Optimizations

1. **Zero-Copy**: Apache Arrow for tensor data
2. **Lock-Free**: DashMap for concurrent access
3. **Async I/O**: Non-blocking network/disk
4. **Caching**: Multiple cache layers (block, query)

---

## Testing Strategy

### Unit Tests
- Domain model invariants
- Repository implementations
- Error handling

### Integration Tests
- Multi-domain workflows
- Network communication
- Semantic search accuracy

### Property-Based Tests
- CID determinism (same data → same CID)
- Embedding similarity transitivity
- Content routing consistency

### Load Tests
- Throughput (blocks/sec, queries/sec)
- Latency (p50, p95, p99)
- Resource usage (memory, CPU, disk)

---

## Security Considerations

### Content Integrity
- **CID verification**: Detect corrupted blocks
- **Deterministic hashing**: Prevent hash collisions

### Peer Trust
- **Reputation scoring**: Trust high-performing peers
- **Circuit breakers**: Isolate misbehaving peers

### Network Security
- **TLS/Noise Protocol**: Encrypted connections via libp2p
- **Message validation**: Deserialize safely with bounds

### Storage Security
- **Immutability**: Prevent unauthorized modifications
- **Access control**: File permissions (OS-level)

---

## Conclusion

IPFRS follows Domain-Driven Design to elegantly separate concerns:

- **Storage Domain**: "What data do we have?"
- **Network Domain**: "Where are peers?"
- **Semantic Domain**: "What does data mean?"
- **Logic Domain**: "What can we infer?"
- **Transport Domain**: "How do we exchange data reliably?"

This architecture provides:
✅ **Clarity**: Clear boundaries between concerns  
✅ **Scalability**: Each domain scales independently  
✅ **Maintainability**: Changes isolated to domains  
✅ **Extensibility**: New features add new bounded contexts  
✅ **Testability**: Domains can be tested in isolation  

---

**For questions or contributions, see**: [GitHub Discussions](https://github.com/cool-japan/ipfrs/discussions)
