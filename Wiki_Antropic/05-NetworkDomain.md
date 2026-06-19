---
title: 05-NetworkDomain
type: domain
summary: Распределённая сеть — Peer-агрегат, Kademlia DHT, два уровня репутации, ACL libp2p
tags: [ipfrs, network, ddd, dht, kademlia]
source: crates/ipfrs-network/src/
related: ["[[03-BoundedContexts]]", "[[08-TransportDomain]]", "[[09-DataFlows]]"]
read_time: 40 мин
updated: 2026-06-18
---

# Network Domain: Распределённая сеть и DHT

**Краткое резюме**: Network Domain управляет peer-to-peer сетью. Его задача: "Кто имеет данные и где они находятся?" Использует Kademlia DHT для распределённого поиска.

> **🔎 Уточнение по коду (2026-06-19).** Контекст состоит из **двух тиров**: Tier A —
> живое ядро `NetworkNode` + `libp2p::Swarm` (Kademlia DHT, QUIC/TCP, NAT — работают); Tier B —
> ~180 модулей (репутация, бан-листы, пулы), ⚠️ **почти не подключённых к событиям swarm**.
> `IpfrsBehaviour` содержит **7 behaviour'ов** — **без** `gossipsub` и `bitswap` (вопреки старым
> диаграммам). Network находит провайдеров через DHT, но ⚠️ **выкачка блоков по swarm — заглушка**
> (`fetch_block_from_peer` → `NotFound`, `node.rs:1311`); реальную выкачку делает Transport-домен.
> `KademliaDhtProvider` — все методы заглушены (`dht_provider.rs:388`); gossipsub только in-process.
> Детали: `[[12-RealityCheck]]`, `[[../Wiki/05-NetworkContext]]`.

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

**Вопрос**: "Является ли этот peer надёжным для долгосрочной маршрутизации?"

Network использует **несколько** скореров репутации (а не одну формулу). Их три плюс дешёвый in-band сигнал:

**Сигнал `u8`** (`peer.rs:40`) на `PeerInfo` — дешёвый, clamp `[0,100]` через `saturating_add(..).min(100)` / `saturating_sub(..)`.

**Композитный EWMA** (`reputation.rs:140+`, scoring `:215–279`):

```rust
// reputation.rs — ReputationScore
pub struct ReputationScore {
    transfer_success_rate: f64,        // EWMA
    latency_score, protocol_compliance_score, uptime_score: f64,
    successful_transfers, failed_transfers, protocol_violations: u64,
}
// overall = Σ dimension * weight     (weights sum to 1.0)
// update:  s_new = α·signal + (1-α)·s_old   ; α ≈ 0.2–0.4
// decay:   s *= (1 - 0.1)  per tick
// Profiles: Strict (0.85), Lenient (0.5), Performance (latency_weight 0.5)
```

**Граф доверия** (`peer_reputation_graph.rs:115–130`):

```rust
// peer_reputation_graph.rs — ReputationScore
pub struct ReputationScore {
    direct_score: f64,       // EMA от прямых взаимодействий
    propagated_score: f64,   // BFS многошаговое доверие, damping 0.5/hop, depth 3
    combined_score: f64,     // 0.6*direct + 0.4*propagated
    confidence, percentile: f64,
}
// edges weight ∈ [0,1]; trust_decay 0.99/tick; prune edges < 0.01
```

Плюс упрощённый `PrReputationScore {score, total_events, violations}` в `peer_reputation.rs`.

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
