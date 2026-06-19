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

## 1bis. Глубокое погружение по коду (выверено 2026-06-19)

> Точные `file:line`-якоря и исправление расхождений по реальному коду
> `ipfrs-transport`. Подсекции 2–13 ниже остаются концептуальными.

### 1bis.1 Что реально работает, а что заглушка

| Подсистема | Статус | Источник |
|------------|--------|----------|
| `Session` + `SessionManager` (жизненный цикл, события) | ✅ работает | `session.rs:199,463` |
| Bitswap want/have/block, приоритеты, выбор пиров | ✅ работает | `bitswap.rs:75,106,184` |
| **TensorSwap** (стриминг, einsum-граф, safetensors, градиенты) | ✅ самый завершённый | `tensorswap/core.rs:57` |
| Prefetch / request coalescing | ✅ работает | `request_coalescing.rs:99` |
| **GraphSync DAG-обход** | ⚠️ заглушка: `extract_links` → `Ok(Vec::new())`, обходит только корень | `graphsync.rs:377` |
| **Erasure (Reed-Solomon)** | ⚠️ заглушка: decode → `DecodingFailed`; encode — взвешенный XOR | `erasure.rs:299` |
| **NAT traversal (STUN/TURN/ICE)** | ⚠️ симуляция: dummy-адреса, «успех» захардкожен | `nat_traversal.rs:367` |
| `MultiTransportManager::find_transport` | ⚠️ баг: fallback не выбирает непредпочтённый транспорт | `multi_transport.rs:215` |

### 1bis.2 Session — корректировки

```rust
pub struct Session {                          // session.rs:199
    id: SessionId,                            // = u64
    state: Arc<RwLock<SessionState>>,
    blocks: Arc<DashMap<Cid, BlockRequest>>,  // приватная внутренняя сущность
    stats, state_tx/rx: watch::*,             // реактивный сигнал завершения
}
```
- `BlockRequest` — **приватная** внутренняя сущность (граница агрегата соблюдена); мутации только
  через методы `Session` (`add_block`/`mark_received`/`mark_failed`).
- **Инвариант завершения**: сессия завершена ⟺ `recv + failed ≥ total` (`session.rs:151`); переход
  состояния делается **вне stats-lock** (`session.rs:323`) — осознанное избегание дедлока.
- ⚠️ Состояние `SessionState::Completing` **объявлено, но никогда не присваивается** (мёртвое).

### 1bis.3 Два Bitswap (ключевое расхождение)

⚠️ **Bitswap реализован дважды и несовместимо**:
| | `ipfrs-transport/bitswap.rs` | `ipfrs-network/bitswap.rs` |
|---|---|---|
| Тип | `BitswapExchange<S>` (`:75`) | `Bitswap` (`:15`) |
| Want-list | приоритетная куча + дедуп | неупорядоченный `HashSet<Cid>` |
| PeerId | `String` (`peer_manager.rs:40`) | `libp2p::PeerId` |
| Зрелость | богатое планирование | минимальный каркас |

Это **параллельные реализации**, не два слоя одного дизайна; ни один не импортирует другой.

- ⚠️ В transport-Bitswap **нет различия want-have vs want-block** и **нет сообщения `WantHave`**;
  присутствие узнаётся лишь из входящих `Have`/`DontHave`.
- ⚠️ `PeerLedger`/«fair-leeching» (из старых диаграмм) **не существуют**.

### 1bis.4 Приоритеты want-list и backpressure

- Полосы приоритета `Low=0/Normal=50/High=100/Urgent=200/Critical=300` (`want_list.rs:70`);
  `effective_priority` поднимает по близости дедлайна (`:170`); куча: приоритет ↓, затем
  `created_at` ↑ → **FIFO внутри полосы**; backoff = `base·2^min(retry,10)` + 10% jitter (`:437`).
- ⚠️ «Баг backpressure-семафора» из старых заметок — **не здесь**: семафорный backpressure живёт в
  `ipfrs-interface/src/backpressure.rs` и (по коду+тестам) **корректен**. У transport свой
  watermark-счётчик (`tensorswap/streaming.rs:328`), без семафора. Полный реестр: `[[../Wiki/11-RealityCheck]]`.

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
