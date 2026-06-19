# Transport Context — Bitswap, Sessions, Want-List

> **Focus**: Reliable block exchange, session management, peer scoring  
> **Source**: `ipfrs_source/crates/ipfrs-transport/src/` (46 files)

---

## 1. Context Overview

Transport Context отвечает за **reliable block exchange protocol coordination**.

```
┌─────────────────────────────────────────────────────────────────────┐
│                    TRANSPORT CONTEXT                                │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    SESSION AGGREGATE                         │   │
│  │  ─────────────────────────────────────────────────────────── │   │
│  │  States: Active → Paused → Completing → Completed            │   │
│  │          Active → Cancelled                                  │   │
│  │  Invariant: complete ⟺ recv + fail ≥ total                  │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    WANTLIST (Priority Queue)                 │   │
│  │  ─────────────────────────────────────────────────────────── │   │
│  │  O(log n) push/pop + O(1) dedup                              │   │
│  │  Priority: Low → Normal → High → Urgent → Critical           │   │
│  │  Deadline elevation                                          │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    BITSWAP EXCHANGE                          │   │
│  │  ─────────────────────────────────────────────────────────── │   │
│  │  Message: WantList | Block | Have | DontHave | Cancel        │   │
│  │  ACL: BlockStore trait, PeerId                               │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    MULTI-TRANSPORT                           │   │
│  │  ─────────────────────────────────────────────────────────── │   │
│  │  QUIC → WebTransport → TCP → WebSocket                       │   │
│  │  Transport trait + Connection trait                          │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    PEER MANAGER                              │   │
│  │  ─────────────────────────────────────────────────────────── │   │
│  │  Per-session EWMA scoring                                    │   │
│  │  Circuit breaker                                             │   │
│  │  Selection: FastestFirst | BestScore | RoundRobin | ...      │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    RECOVERY                                  │   │
│  │  ─────────────────────────────────────────────────────────── │   │
│  │  Request coalescing                                          │   │
│  │  Erasure coding (k+m shards)                                 │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 2. Session Aggregate

### 2.1 State Machine

```
                    ┌──────────────┐
                    │    Active    │
                    └──────┬───────┘
                           │
         ┌─────────────────┼─────────────────┐
         │                 │                 │
         ▼                 ▼                 ▼
    ┌─────────┐      ┌───────────┐     ┌───────────┐
    │  Paused │      │ Completing│     │ Cancelled │
    └────┬────┘      └─────┬─────┘     └───────────┘
         │                 │                 (terminal)
         │                 ▼
         │           ┌───────────┐
         └──────────►│ Completed │
                     └───────────┘
                      (terminal)
```

### 2.2 Structure

```rust
pub struct Session {
    pub id: SessionId,                       // u64
    pub config: SessionConfig,
    pub state: Arc<RwLock<SessionState>>,
    pub blocks: Arc<DashMap<Cid, BlockRequest>>,
    pub stats: Arc<RwLock<SessionStats>>,
    pub event_tx: Option<mpsc::UnboundedSender<SessionEvent>>,
    
    // Reactive watching
    pub state_tx: watch::Sender<SessionState>,
    pub state_rx: watch::Receiver<SessionState>,
}

pub enum SessionState {
    Active,
    Paused,
    Completing,
    Completed,   // Terminal
    Cancelled,   // Terminal
}
```

### 2.3 Completion Invariant

```rust
impl SessionStats {
    pub fn is_complete(&self) -> bool {
        self.blocks_received + self.blocks_failed >= self.total_blocks
    }
}
```

**No early completion**: Session enters `Completed` only when all blocks resolved.

---

## 3. WantList — Priority Queue

### 3.1 Structure

```rust
pub enum Priority {
    Low      = 0,
    Normal   = 50,
    High     = 100,
    Urgent   = 200,
    Critical = 300,
}

pub struct WantEntry {
    pub cid: Cid,
    pub priority: Priority,
    pub created_at: Instant,
    pub expires_at: Option<Instant>,
    pub retry_count: u32,
    pub deadline: Option<Instant>,
}

pub struct WantList {
    heap: BinaryHeap<HeapEntry>,
    entries: HashMap<Cid, (WantEntry, u64)>,  // CID → (entry, version)
    version_counter: u64,
}
```

### 3.2 Operations

```rust
impl WantList {
    // O(log n) push
    pub fn push(&mut self, entry: WantEntry) {
        self.version_counter += 1;
        self.entries.insert(entry.cid.clone(), (entry.clone(), self.version_counter));
        self.heap.push(HeapEntry {
            priority: entry.effective_priority(),
            cid: entry.cid,
            version: self.version_counter,
        });
    }
    
    // O(log n) pop with lazy deletion
    pub fn pop(&mut self) -> Option<WantEntry> {
        while let Some(HeapEntry { cid, version, .. }) = self.heap.pop() {
            if let Some((entry, current)) = self.entries.get(&cid) {
                if *current == version {
                    self.entries.remove(&cid);
                    return Some(entry.clone());
                }
            }
        }
        None
    }
    
    // O(1) dedup
    pub fn contains(&self, cid: &Cid) -> bool {
        self.entries.contains_key(cid)
    }
}
```

### 3.3 Deadline Elevation

```rust
impl WantEntry {
    pub fn effective_priority(&self) -> u32 {
        let base = self.priority as u32;
        
        if let Some(deadline) = self.deadline {
            let remaining = deadline.duration_since(Instant::now());
            let urgency = match remaining {
                d if d < Duration::from_secs(10) => 100,
                d if d < Duration::from_secs(60) => 50,
                _ => 0,
            };
            base + urgency
        } else {
            base
        }
    }
}
```

### 3.4 Retry Backoff

```rust
pub fn next_retry_delay(&self) -> Duration {
    let base = self.config.retry_base_delay;
    let max = self.config.retry_max_delay;
    
    // base × 2^min(retry, 10)
    let exponent = self.retry_count.min(10);
    let delay = base * 2u32.pow(exponent);
    
    // Clamp + 10% jitter
    (delay.min(max)).mul_f64(1.0 + 0.1 * random::<f64>())
}
```

---

## 4. Bitswap Exchange

### 4.1 Messages

```rust
pub enum Message {
    WantList(WantListMessage),
    Block(BlockMessage),
    Have(HaveMessage),
    DontHave(DontHaveMessage),
    Cancel(CancelMessage),
}
```

### 4.2 BitswapExchange

```rust
pub struct BitswapExchange<S: BlockStore> {
    store: Arc<S>,
    peer_manager: Arc<PeerManager>,
    sessions: Arc<SessionManager>,
}

impl<S: BlockStore> BitswapExchange<S> {
    pub async fn want(&self, cid: &Cid, priority: Priority) -> Result<()> {
        // 1. Check local
        if self.store.has(cid).await? {
            return Ok(());
        }
        
        // 2. Add to session
        let session = self.sessions.get_active()?;
        session.add_want(cid, priority)?;
        
        // 3. Select peers
        let peers = self.peer_manager.select_peers_for_request(cid)?;
        
        // 4. Send want-list
        for peer in peers {
            self.send_want_list(peer, &[WantEntry { cid, priority, .. }]).await?;
        }
        
        Ok(())
    }
    
    pub async fn receive_block(&self, peer: &PeerId, block: Block) -> Result<()> {
        // 1. Verify (core invariant)
        block.verify()?;
        
        // 2. Store
        self.store.put(&block).await?;
        
        // 3. Update session
        if let Some(session) = self.sessions.find_session_for_cid(&block.cid)? {
            session.mark_received(&block.cid)?;
        }
        
        Ok(())
    }
}
```

---

## 5. Multi-Transport

### 5.1 Transport Trait

```rust
pub enum TransportType {
    Quic,
    Tcp,
    WebSocket,
    WebTransport,
}

#[async_trait]
pub trait Transport: Send + Sync {
    async fn connect(&self, addr: &Multiaddr) -> Result<Box<dyn Connection>>;
    fn transport_type(&self) -> TransportType;
}

#[async_trait]
pub trait Connection: Send + Sync {
    async fn send(&self, data: &[u8]) -> Result<()>;
    async fn recv(&self) -> Result<Vec<u8>>;
}
```

### 5.2 Fallback Chain

```rust
pub struct MultiTransportManager {
    transports: HashMap<TransportType, Box<dyn Transport>>,
    working_transport: DashMap<PeerId, TransportType>,
    fallback_order: Vec<TransportType>,  // [Quic, WebTransport, Tcp, WebSocket]
}

impl MultiTransportManager {
    pub async fn connect(&self, peer: &PeerId, addrs: &[Multiaddr]) -> Result<Box<dyn Connection>> {
        // 1. Check cache
        if let Some(transport) = self.working_transport.get(peer) {
            if let Ok(conn) = self.try_transport(*transport, addrs).await {
                return Ok(conn);
            }
        }
        
        // 2. Fallback chain
        for transport in &self.fallback_order {
            if let Ok(conn) = self.try_transport(*transport, addrs).await {
                self.working_transport.insert(*peer, *transport);
                return Ok(conn);
            }
        }
        
        Err(Error::AllTransportsFailed)
    }
}
```

---

## 6. Peer Manager

### 6.1 Metrics

```rust
pub struct PeerMetrics {
    pub latency_ewma: f64,
    pub bandwidth_ewma: f64,
    pub reliability: f64,
    pub consecutive_failures: u32,
}
```

### 6.2 Circuit Breaker

```rust
pub struct CircuitBreaker {
    state: CircuitState,
    failure_threshold: u32,
    recovery_timeout: Duration,
}

pub enum CircuitState {
    Closed,      // Normal
    Open,        // Failing, reject
    HalfOpen,    // Testing recovery
}
```

### 6.3 Selection Strategies

```rust
pub enum SelectionStrategy {
    FastestFirst,
    HighestBandwidth,
    BestScore,
    RoundRobin,
    Random,
    LeastLoaded,
}
```

---

## 7. Request Coalescing

```rust
pub struct RequestCoalescer {
    pending: DashMap<Cid, broadcast::Sender<Block>>,
}

impl RequestCoalescer {
    pub async fn request(&self, cid: &Cid, fetch: impl Future<Output = Result<Block>>) -> Result<Block> {
        // 1. Check if in progress
        if let Some(sender) = self.pending.get(cid) {
            return sender.subscribe().recv().await?;
        }
        
        // 2. Create channel
        let (tx, _) = broadcast::channel(1);
        self.pending.insert(cid.clone(), tx.clone());
        
        // 3. Fetch and broadcast
        let block = fetch.await?;
        tx.send(block.clone()).ok();
        
        Ok(block)
    }
}
```

---

## 8. Erasure Coding

```rust
pub struct RecoveryManager {
    k: usize,  // Data shards
    m: usize,  // Parity shards
}

pub enum RecoveryMode {
    Normal,     // k shards sufficient
    Degraded,   // Using parity
    Emergency,  // Below k
}

impl RecoveryManager {
    pub fn encode(&self, data: &[u8]) -> Result<Vec<Shard>>;
    pub fn decode(&self, shards: &[Option<Shard>]) -> Result<Vec<u8>>;
}
```

---

## 9. Invariants

| Invariant | Enforcement |
|-----------|-------------|
| Session completes when `recv + fail ≥ total` | `is_complete()` |
| Block verified on receive | `block.verify()` |
| WantList: one entry per CID | HashMap dedup |
| Circuit breaker prevents cascading | State machine |

---

## 10. Performance

| Operation | P50 | P99 |
|-----------|-----|-----|
| WantList push/pop | 1µs | 5µs |
| Bitswap message | 10µs | 50µs |
| Peer selection | 100µs | 500µs |
| Full network fetch | 200ms | 1000ms |

---

## 11. Key Files

| File | Lines | Purpose |
|------|-------|---------|
| `session.rs` | 600+ | Session aggregate |
| `want_list.rs` | 500+ | Priority queue |
| `bitswap.rs` | 400+ | Bitswap protocol |
| `transport.rs` | 250+ | Transport trait |
| `peer_manager.rs` | 400+ | Peer scoring |
| `request_coalescing.rs` | 200+ | Deduplication |

---

## 12. Design Decisions

### 12.1 Why Intentional Duplication?

**Network reputation ≠ Transport peer scoring**

**Rationale**:
- Network: Long-term routing trust
- Transport: Per-session transfer quality
- Bounded-context autonomy

---

### 12.2 Why Multi-Transport Fallback?

**Decision**: QUIC → WebTransport → TCP → WebSocket.

**Rationale**:
- QUIC: Best performance
- WebTransport: Browser support
- TCP: Universal
- WebSocket: Firewall traversal

---

## 13. Context Integration

```
┌─────────────────────────────────────────────────────────────────────┐
│                    TRANSPORT INTEGRATION                            │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Consumes (Customer/Supplier):                                      │
│    • Storage — BlockStore.get/put                                   │
│    • Network — PeerId, Multiaddr                                    │
│                                                                     │
│  Shared Kernel:                                                     │
│    • Block, Cid, Error, Result                                      │
│                                                                     │
│  Publishes:                                                         │
│    • SessionEvent — Observability                                   │
│    • TransportEvent — Health monitoring                             │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

**Next**: [08-ApplicationLayer.md](08-ApplicationLayer.md) — Node facade, protocols, bindings
