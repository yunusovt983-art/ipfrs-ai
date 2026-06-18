# Network Domain: Распределённая сеть и DHT

**Краткое резюме**: Network Domain управляет peer-to-peer сетью. Его задача: "Кто имеет данные и где они находятся?" Использует Kademlia DHT для распределённого поиска.

---

## Язык домена

| Термин | Значение |
|--------|----------|
| **Peer** | Удалённый узел с идентичностью |
| **PeerId** | Уникальный ID = hash(public_key) |
| **DHT** | Distributed Hash Table (Kademlia) |
| **Multiaddr** | Адрес для достижения peer'а |
| **Provider** | Узел, который имеет блок с CID |
| **Reputation** | Оценка надёжности peer'а |

---

## Агрегат: Peer

**Source**: `crates/ipfrs-network/src/peer.rs:22–76`

### Структура

```rust
// network/peer.rs:22–41
pub struct PeerInfo {
    pub peer_id: String,            // VO: libp2p::PeerId stringified (ACL)
    pub addrs: Vec<String>,         // Multiaddrs
    pub protocols: Vec<String>,
    pub last_seen: u64,
    pub connection_count: u64,
    pub avg_latency_ms: Option<u64>,
    pub reputation: u8,             // 0..=100
}

struct PeerRecord {                 // peer.rs:64–76 (internal aggregate state)
    info: PeerInfo,
    addrs: HashSet<Multiaddr>,
    connected: bool,
    connected_at: Option<Instant>,
    latency_samples: Vec<Duration>, // bounded
}
```

**Repository**: `PeerStore` (peer.rs:175+) backed by `DashMap<PeerId, PeerRecord>` + `RwLock<HashSet<PeerId>>`
Methods: `add_peer, get_peer, peer_connected, update_latency, increase/decrease_reputation, peers_by_reputation, peers_by_latency`

### Инварианты

```
1. PeerId = hash(public_key)     (ВСЕГДА, immutable)
2. multiaddrs не пусты если connected
3. reputation ∈ [0, 100]
4. known_blocks = blocks we've heard peer announced
```

---

## Domain Service: DHT (Kademlia)

### Операция: Announce

```
User: I just stored CID X
    ↓
dht.put_provider(X, my_peer_id)
    ↓
Find k=20 peers closest to X (by XOR distance)
    ↓
Send them: "I, {my_peer_id}, have block X"
    ↓
They store: (X → [my_peer_id, ...])
    ↓
Other peers can now discover me for X
```

### Операция: Lookup

```
User: Who has CID Y?
    ↓
dht.find_providers(Y)
    ↓
Iterative search:
  1. Ask bootstrap peers: "Who's close to Y?"
  2. They respond: [peer1, peer2, ...]
  3. Ask those: "Who's even closer?"
  4. They respond with closer peers
  5. Repeat until converging
    ↓
Result: [PeerId1, PeerId2, ...]  (typically 20-100)

Time: 150-300ms (network RTT × hops)
```

### Метрика: XOR Distance

```
distance(A, B) = A ^ B  (bitwise XOR)

Properties:
- Metric space (triangle inequality)
- All peers agree on ordering
- Deterministic: same input → same ordering
```

---

## Reputation Scoring (Network Context)

**Вопрос**: "Является ли этот peer'надёжным для долгосрочной маршрутизации?"

```rust
pub struct ReputationScore {
    success_count: u64,
    total_queries: u64,
    last_updated: Instant,
    weight: f64,  // Exponential decay by age
}

impl ReputationScore {
    pub fn score(&self) -> f64 {
        let base_rate = self.success_count as f64 / (self.total_queries as f64 + 1.0);
        let age_seconds = self.last_updated.elapsed().as_secs_f64();
        let decay = (-age_seconds / HALF_LIFE).exp();
        base_rate * decay
    }
}

// Example:
// Peer A: 95 successes / 100 queries, recent → score = 0.95 * 0.99 = 0.94
// Peer B: 70 successes / 100 queries, stale  → score = 0.70 * 0.50 = 0.35
// Network prefers Peer A
```

---

## Provider Store

```rust
pub struct ProviderStore {
    providers: Arc<DashMap<Cid, Vec<PeerId>>>,
    expiration: Arc<DashMap<(Cid, PeerId), Instant>>,
    replication_factor: usize,  // k=20
}

impl ProviderStore {
    pub async fn put_provider(&self, cid: Cid, peer_id: PeerId) {
        // Store on this node
        self.providers.entry(cid)
            .or_insert(Vec::new())
            .push(peer_id);
        
        // Set TTL (default: 24 hours)
        self.expiration.insert((cid, peer_id), now() + 24h);
    }
    
    pub async fn get_providers(&self, cid: &Cid) -> Vec<PeerId> {
        // Return all non-expired providers
        let providers = self.providers.get(cid)?;
        providers.iter()
            .filter(|p| !self.is_expired(cid, p))
            .collect()
    }
}
```

---

## Content Routing Protocol

### Phase 1: Announcement

```
┌─ Storage Domain ────┐
│ Block added: CID    │
└──────────┬──────────┘
           │
┌──────────▼──────────────────────┐
│ Network: announce(cid)          │
│  1. Locate k=20 peers closest   │
│  2. Send Put-Provider message   │
│  3. They store in their tables  │
└──────────┬──────────────────────┘
           │
└──────────▶ DHT Replicas know CID location
```

### Phase 2: Discovery

```
┌─ Application ───────────────────┐
│ Need: get(cid)                  │
└──────────┬──────────────────────┘
           │
┌──────────▼──────────────────────┐
│ Network: find_providers(cid)    │
│  1. Ask bootstrap peers         │
│  2. Iterative refinement        │
│  3. Converge on best k peers    │
└──────────┬──────────────────────┘
           │
└──────────▶ Return [peer1, peer2, ...]
                for Transport to contact
```

---

## Anti-Corruption Layer: libp2p Wrapping

Network domain **wraps** low-level libp2p types:

```rust
// External (libp2p):
use libp2p::PeerId as Libp2pPeerId;

// Domain (ipfrs-network):
pub struct PeerId(String);  // Opaque wrapper

impl From<Libp2pPeerId> for PeerId {
    fn from(libp2p_id: Libp2pPeerId) -> Self {
        PeerId(libp2p_id.to_string())
    }
}

// Benefit: Network domain independent of libp2p details
// If we migrate to libp2p v1.0, only this function changes
```

---

## Peer Discovery Methods

### Method 1: mDNS (Local Network)

```
Broadcast: "I'm here!" on multicast UDP
Other peers on LAN respond
Useful for: Local testing, LAN deployment
```

### Method 2: Bootstrap Nodes

```
Known list of "entry points" (e.g., cloud-hosted)
Every new node connects to bootstrap
Learns about rest of network
Time: <1 second
```

### Method 3: DHT Walk

```
Periodically query random keys
Discover peers not in our routing table
Time: ~5 minutes per walk
```

---

## Metrics & Performance

| Operation | Latency | Notes |
|-----------|---------|-------|
| Connect to peer | 50-200ms | Depends on network conditions |
| DHT lookup | 150-300ms | Iterative search (α=3 concurrency) |
| Provider announcement | ~100ms | Async, runs in background |
| Peer discovery | Variable | Depends on network topology |
| Reputation update | <1ms | Atomic counter |

**Network utilization**:
- DHT: ~10 KB/lookup
- Provider messages: ~1 KB per announcement
- Peer state: ~500 bytes per peer

---

## Взаимодействие с другими доменами

### Network → Storage
```
On bootstrap:
  Ask older peers: "What CIDs do you have?"
  Sync known_blocks set
```

### Network → Transport
```
Application asks: find_providers(cid)
Network returns: [peer1, peer2, ...]
Transport chooses best peer by reputation
```

### Network ← Application
```
Application: "Announce I have CID X"
Network: DHT.put_provider(X, my_peer_id)
```

---

## Важные свойства

| Свойство | Значение |
|----------|----------|
| **Decentralized** | Нет центрального сервера |
| **Self-Healing** | Mesh automatically repairs |
| **Byzantine-Tolerant** | Gossip protocol resists malice |
| **Deterministic Routing** | Same CID → same order of peers |
| **Scalable** | O(log n) hops for n peers |

---

## Что дальше?

→ [03-Bounded Contexts](03-BoundedContexts.md) для обзора  
→ [08-TransportDomain](08-TransportDomain.md) для выбора лучшего peer'а из списка  
→ `/Volumes/Kingston/cool-japan/Vendor/ipfrs/crates/ipfrs-network/` для кода

---

**Связанные**: [02-Architecture Stack](02-ArchitectureStack.md) | [03-Bounded Contexts](03-BoundedContexts.md) | [08-TransportDomain](08-TransportDomain.md)
