---
title: 08-TransportDomain
type: domain
summary: Надёжный обмен блоками — Session-агрегат, Bitswap, WantList с приоритетами, peer scoring
tags: [ipfrs, transport, ddd, bitswap, session]
source: crates/ipfrs-transport/src/
related: ["[[03-BoundedContexts]]", "[[05-NetworkDomain]]", "[[09-DataFlows]]"]
read_time: 40 мин
updated: 2026-06-18
---

# Transport Domain: Надёжный обмен блоками

**Краткое резюме**: Transport Domain отвечает на вопрос "Как обмениваться блоками надёжно?" Использует Bitswap протокол с приоритетными очередями и peer scoring.

---

## Язык домена

| Термин | Значение |
|--------|----------|
| **Session** | Батч запросов блоков с состояниями |
| **WantList** | Приоритетная очередь блоков |
| **Bitswap** | Протокол обмена (запрос/ответ) |
| **Ledger** | Per-peer учёт (я им дал / они мне дали) |
| **Reputation** | Оценка скорости peer'а (session-local) |

---

## Агрегат: Session

**Source**: `crates/ipfrs-transport/src/session.rs`

### Структура

```rust
// transport/session.rs
pub struct Session {
    id: SessionId,                       // = u64
    config: SessionConfig,
    state: Arc<RwLock<SessionState>>,
    blocks: Arc<DashMap<Cid, BlockRequest>>,
    stats: Arc<RwLock<SessionStats>>,
    event_tx: Option<mpsc::UnboundedSender<SessionEvent>>,
    state_tx/state_rx: watch::Sender/Receiver<SessionState>,  // reactive state
}

pub enum SessionState { 
    Active, Paused, Completing, 
    Completed, Cancelled 
}
```

### Инварианты & State Machine

- **Valid transitions**: `Active→{Paused, Completing, Completed, Cancelled}`, `Paused→Active`, `*→Cancelled`
- **Terminal**: `Completed`, `Cancelled` (no revival)
- **Completion invariant**: session enters `Completed` только when `blocks_received + blocks_failed ≥ total_blocks` (`SessionStats::is_complete()`)
- A block marked received/failed exactly once

**Repository**: `SessionManager` (session.rs:462+) over `DashMap<SessionId, Arc<Session>>`
Methods: `create/get/remove/active_sessions/cleanup_completed/recv_event`

---

## WantList & Priority

**Source**: `crates/ipfrs-transport/src/want_list.rs`

```rust
// want_list.rs
pub enum Priority { 
    Low=0, Normal=50, 
    High=100, Urgent=200, 
    Critical=300 
}

pub struct WantEntry { 
    cid, priority, created_at, 
    expires_at, retry_count,
    send_dont_have, deadline, tag 
}   // effective_priority() boosts near deadline

pub struct WantList { 
    heap: BinaryHeap<HeapEntry>, 
    entries: HashMap<Cid,(WantEntry,u64)>, 
    version_counter, config 
}
```

**Сложность**: O(log n) push/pop + O(1) dedup via lazy deletion (version markers skip stale heap entries)

**Retry backoff**: `base·2^min(retry,6)` capped 32×, with jitter

**Инварианты**:
- one entry per CID
- heap ≤ `max_wants`
- FIFO within priority
- deadline-elevated priority

### Priority Semantics

```
Scenario: Fetch tensor with 1000 chunks

Assign priorities:
  - First chunk (needed for header): priority 100
  - Next 10 chunks (needed soon): priority 90-80
  - Middle chunks: priority 50
  - Tail chunks (nice to have): priority 10

Bitswap to peer:
  "I want [chunk1@100, chunk2@90, ..., chunk1000@10]"
  
Peer respects priority:
  Sends chunk1 immediately
  Then chunk2-10
  Then middle chunks
  Tail only if bandwidth available
```

---

## Peer Scoring (Transport Perspective)

**Different from Network's long-term scoring:**

```rust
pub struct TransportScore {
    success_in_session: u64,
    total_requests_in_session: u64,
    avg_latency_ms: f64,
    connected: bool,
    connection_age: Duration,
}

impl TransportScore {
    pub fn score(&self) -> f64 {
        let success_rate = self.success_in_session as f64 
                         / (self.total_requests_in_session as f64 + 1.0);
        let latency_factor = 1.0 / (self.avg_latency_ms + 1.0);
        let availability = if self.connected { 1.0 } else { 0.1 };
        let age_bonus = (self.connection_age.as_secs() as f64).min(100.0) / 100.0;
        
        success_rate * latency_factor * availability * age_bonus
    }
}

// Example session scoring:
// Peer A: 50/50 success, 10ms latency, connected 5min
//         → 1.0 * 0.091 * 1.0 * 1.0 = 0.091
// Peer B: 40/50 success, 50ms latency, connected 1sec
//         → 0.8 * 0.019 * 1.0 * 0.01 = 0.00015
// → Transport prefers Peer A strongly (600x better)
```

---

## Bitswap Protocol

### Message Flow

```
┌─ Peer A (us) ────────────┐
│ Want blocks: [CID1, CID2]│
│ Priority: [100, 50]      │
└────────┬─────────────────┘
         │
      [Want message]
         ↓ (50-100ms RTT)
         │
┌────────▼──────────────────┐
│ Peer B (remote)           │
│ Receives Want             │
│ Checks storage:           │
│   CID1: found ✓           │
│   CID2: found ✓           │
│ Respects priority → CID1  │
└────────┬──────────────────┘
         │
      [Block(CID1, data)]
         ↓ (50-100ms RTT)
         │
┌────────▼──────────────────┐
│ Peer A (us)               │
│ Receives Block(CID1)      │
│ Verify: hash(data)==CID1 ✓│
│ Storage.put(block)        │
│ Update peer reputation    │
│ Remove CID1 from want_list│
│ Send next: CID2           │
└────────┬──────────────────┘
         │
         ... repeat until all blocks received or timeout
```

### Message Format

```rust
pub enum BitswapMessage {
    Want {
        block_presences: Vec<(Cid, Priority)>,
        cancel_blocks: Vec<Cid>,
        send_dont_have: bool,
    },
    Block {
        cid: Cid,
        data: Bytes,
    },
    WantHave {
        cids: Vec<Cid>,
    },
    Have {
        blocks: Vec<Cid>,
    },
    DontHave {
        blocks: Vec<Cid>,
    },
}
```

---

## Per-Peer Ledger

```rust
pub struct PeerLedger {
    peer_id: PeerId,
    we_owe: u64,             // Bytes we promised to send
    they_owe: u64,           // Bytes they promised to send
    balance: i64,            // they_owe - we_owe (positive = fair)
    last_interaction: Instant,
}

impl PeerLedger {
    pub fn is_fair(&self) -> bool {
        // Don't send more than ratio
        if self.we_owe > 0 {
            self.they_owe as f64 / self.we_owe as f64 >= 0.5
        } else {
            true
        }
    }
}

// Use case: Prevent leeching
// If a peer always takes without giving back:
//   ledger.balance < -10MB → throttle or disconnect
```

---

## Session State Machine

```
         ┌─────────┐
         │ Created │
         └────┬────┘
              │
              ↓
        ┌──────────┐
        │  Active  │◄─────┐
        └────┬─────┘      │
             │            │ (pause/resume)
             │            │
        (got all blocks)  Paused
             │            │
             ↓            │
        ┌──────────┐      │
        │Completed │      │
        └──────────┘      │
                          │
             (on error)   │
             ↓            │
        ┌──────────┐      │
        │  Failed  │◄─────┘
        └──────────┘

Timeouts:
- Active → Failed: 30s without progress
- Paused → Failed: 5m inactivity
```

---

## Example: Fetch 100MB File

```
Scenario: User wants file with 391 blocks
Network found: [peer1, peer2, peer3]
Transport scoring:
  peer1: 0.95 (fast, reliable)
  peer2: 0.45 (medium)
  peer3: 0.20 (slow)

Session created:
  requested_blocks = [block1...block391]
  state = Active

WantList assigned:
  Priority 100: [block1] (first chunk)
  Priority 90: [block2-20]
  Priority 50: [block21-200]
  Priority 10: [block201-391]

Round 1: Contact peer1
  Send: Want([block1@100, block2-20@90, ...])
  Receive: Block(block1) + Block(block2-5)  (6 blocks, ~1.5MB)
  Latency: 150ms
  received_blocks.len() = 6

Round 2: Contact peer1 again
  Send: Want([block6-20@90, ...]) (removed arrived blocks)
  Receive: Block(block6-15)  (10 blocks, ~2.5MB)
  Latency: 100ms
  received_blocks.len() = 16

... continue until all 391 blocks arrive ...

Total time: ~10-30 seconds depending on peer bandwidth
  DHT lookup: 150-300ms
  Actual transfer: 100-500ms per round × ~30 rounds
  Wait time: 5-20s (peer slow/throttling)
```

---

## Metrics & Performance

| Operation | Latency | Notes |
|-----------|---------|-------|
| Session creation | <1ms | Just allocate structs |
| Peer selection | <1ms | Score lookup |
| Bitswap message | 50-100ms | Network RTT |
| Block verification | <1µs | hash() SIMD |
| Ledger update | <1µs | Atomic counter |
| Full block fetch (100MB) | 10-30s | Depends on peer bandwidth |

**Throughput**:
```
Single peer: 1-10 MB/s (depends on network)
Multiple peers (3): 3-30 MB/s
Parallel sessions: 10-100 MB/s
```

---

## Взаимодействие с другими доменами

### Transport ← Network
```
Network: find_providers(cid) → [peer1, peer2, ...]
Transport: Create session, contact peers
```

### Transport → Storage
```
Transport: Received block(cid, data)
Storage: put(block) → verify hash
```

### Transport ← Application
```
App: Need blocks [cid1, cid2, ...]
Transport: Create session, manage fetching
```

---

## Важные свойства

| Свойство | Значение |
|----------|----------|
| **Reliable** | Verify every block (hash check) |
| **Priority** | Earlier blocks first |
| **Fair** | Per-peer ledger prevents leeching |
| **Efficient** | Batch requests, parallel peers |
| **Timeout** | Session fails gracefully |

---

## Что дальше?

→ [03-Bounded Contexts](03-BoundedContexts.md) для обзора  
→ [05-NetworkDomain](05-NetworkDomain.md) для peer discovery  
→ [04-StorageDomain](04-StorageDomain.md) для хранения полученных блоков  
→ [09-Data Flows](09-DataFlows.md) для сценария "Get file from network"  
→ `/Volumes/Kingston/cool-japan/Vendor/ipfrs/crates/ipfrs-transport/` для кода

---

**Связанные**: [02-Architecture Stack](02-ArchitectureStack.md) | [03-Bounded Contexts](03-BoundedContexts.md) | [05-NetworkDomain](05-NetworkDomain.md) | [09-Data Flows](09-DataFlows.md)
