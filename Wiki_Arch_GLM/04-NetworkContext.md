# Network Context — libp2p, DHT, Reputation

> **Focus**: Peer identity, DHT content routing, reputation graph  
> **Source**: `ipfrs_source/crates/ipfrs-network/src/` (~150 files)

---

## 1. Context Overview

Network Context отвечает за **peer identity, discovery, reputation, DHT content routing**.

```
┌─────────────────────────────────────────────────────────────────────┐
│                    NETWORK CONTEXT                                  │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    NETWORK NODE                              │   │
│  │  ─────────────────────────────────────────────────────────── │   │
│  │  libp2p::Swarm with:                                         │   │
│  │    • Kademlia DHT                                            │   │
│  │    • Identify protocol                                       │   │
│  │    • Ping                                                    │   │
│  │    • AutoNAT (NAT detection)                                 │   │
│  │    • DCUtR (hole punching)                                   │   │
│  │    • mDNS (local discovery)                                  │   │
│  │    • Relay                                                   │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    PEER AGGREGATE                            │   │
│  │  ─────────────────────────────────────────────────────────── │   │
│  │  PeerStore — DashMap<PeerId, PeerRecord>                     │   │
│  │  PeerInfo { peer_id, addrs, protocols, reputation }          │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    REPUTATION (2-TIER)                       │   │
│  │  ─────────────────────────────────────────────────────────── │   │
│  │  Tier 1: ReputationManager (EWMA)                            │   │
│  │    • transfer_success_rate, latency_score, etc.              │   │
│  │                                                              │   │
│  │  Tier 2: PeerReputationGraph (Trust Graph)                   │   │
│  │    • BFS propagation, damping 0.5/hop, depth 3               │   │
│  │    • combined = 0.6×direct + 0.4×propagated                  │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    DHT ABSTRACTION                           │   │
│  │  ─────────────────────────────────────────────────────────── │   │
│  │  DhtProvider trait (port)                                    │   │
│  │  KademliaDhtProvider (adapter)                               │   │
│  │  SemanticDht — LSH-based semantic routing                    │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    EVENT BUS                                 │   │
│  │  ─────────────────────────────────────────────────────────── │   │
│  │  NebNetworkEvent { id, topic, payload, source_peer }         │   │
│  │  EventFilter, 10k replay buffer                              │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 1bis. Глубокое погружение по коду (выверено 2026-06-19)

> Точные `file:line`-якоря и исправление расхождений по реальному коду `ipfrs-network`.
> Подсекции 2–15 ниже остаются концептуальными.

### 1bis.1 Главный факт: Tier A vs Tier B

⚠️ Крейт состоит из **двух тиров, которые почти не соприкасаются**:
- **Tier A — живое ядро**: `NetworkNode` (`node.rs:404`) + `libp2p::Swarm`. `node.rs` импортирует
  из `crate::` **по сути только `gossipsub`** (`node.rs:428,518`). `ConnectionManager`, `PeerStore`,
  `BootstrapManager`, `ban_list`, `routing_table_manager` в `node.rs` **не упоминаются вообще**.
- **Tier B — ~150 модулей** (репутация, бан-листы, пулы, маршрутизация) — изолированы, **не
  получают событий swarm**.
- **`NetworkFacade`** (`facade.rs:93`) со-локализует `NetworkNode` + ровно **17 подсистем**
  (14 `Option<Arc<RwLock>>` + 3 всегда-доступных), но ⚠️ **ни одно `NetworkEvent` им не доставляется**
  (в `facade.rs` нет обработки `event_rx`). Это и есть разрыв Tier A/Tier B.

### 1bis.2 NetworkNode + IpfrsBehaviour

- **`NetworkNode`** (`node.rs:404`): ⚠️ **нет поля `peer_store`** (он живёт только в `NetworkFacade`,
  `facade.rs:114`); канал событий — **bounded `mpsc::channel(1024)`** (`node.rs:511`), не unbounded.
- **`IpfrsBehaviour`** (`node.rs:288`) — ровно **7 behaviour'ов**: `kademlia, identify, ping, autonat,
  dcutr, mdns, relay_client`. ⚠️ **Ни `gossipsub`, ни `bitswap`** как libp2p-behaviour нет → pub/sub и
  обмен блоками не интегрированы в swarm.

### 1bis.3 Что реально работает, а что заглушка

- ✅ **DHT content-routing** — живой путь: `provide` → `kademlia.start_providing` (`node.rs:1200`),
  `find_providers` → `get_providers` (`node.rs:1246`), результат через `provider_waiters`.
  ⚠️ Ключ ожидания строится `String::from_utf8_lossy(&cid.to_bytes())` (`node.rs:864,1234`) — бинарные
  байты CID не UTF-8 → хрупко.
- ⚠️ **Выкачка блоков — заглушка**: `fetch_block_from_peer` → `Error::NotFound("...pending Task E")`
  (`node.rs:1314`). `bitswap.rs::Bitswap` — in-process структура, не libp2p-behaviour.
- ⚠️ **Gossipsub — только in-process router** (`GossipSubManager`, `gossipsub.rs:280`); нет
  `libp2p::gossipsub::Behaviour`; `validate_message` → **всегда `true`** (`gossipsub.rs:468`).
- ⚠️ **`KademliaDhtProvider`** (`dht_provider.rs:367`) — **все методы заглушены** (provide → `Ok(())`,
  find_providers → пустой результат). Не связан с реальным Kademlia в `node.rs`.

### 1bis.4 Репутация и события — корректировки

- ⚠️ **4 независимые модели репутации**: `peer.rs` (`u8` 0–100), `reputation.rs` (EWMA
  `ReputationManager`), `peer_reputation_graph.rs` (граф доверия), `peer_reputation.rs` (`f64` [0,1]).
  Параметры графа **подтверждены**: depth=3, damping=0.5, веса 0.6/0.4, decay=0.99, prune=0.01
  (`peer_reputation_graph.rs:178`). ⚠️ `reputation.rs::apply_decay` (`:272`) — **без `.clamp()`**
  (остальные клампят).
- **`NetworkEvent`** (`node.rs:448`): `PeerConnected/PeerDisconnected/ContentFound{cid,providers}/
  PeerDiscovered/ListeningOn/ConnectionError/DhtBootstrapCompleted/NatStatusChanged`. ⚠️ Вариантов
  `DhtQueryCompleted{query_id}` и `GossipsubMessage` **нет**.

### 1bis.5 Транспорт

`build_swarm` (`node.rs:599`): **QUIC (`quic-v1`) + TCP + relay-client**, аутентификация **Noise**,
мультиплексирование **Yamux**; idle timeout **60 c** (`:723`); identify `"/ipfrs/1.0.0"` (`:671`);
ping **15 c** (`:677`); Kademlia `set_mode(Server)` + `bootstrap()` на старте. ⚠️ Зависит от
**`ipfrs-tensorlogic`** — для распределённого вывода поверх gossip-router (`InferenceWaiters`,
`node.rs:32,1529`). Полный реестр заглушек: `[[../Wiki/11-RealityCheck]]`.

---

## 2. NetworkNode — libp2p Swarm Wrapper

### 2.1 Structure

```rust
pub struct NetworkNode {
    swarm: Swarm<NetworkBehaviour>,
    peer_store: Arc<PeerStore>,
    config: NetworkConfig,
    
    // Event channels
    event_tx: mpsc::UnboundedSender<NetworkEvent>,
    event_rx: mpsc::UnboundedReceiver<NetworkEvent>,
}
```

### 2.2 libp2p Behaviours

```rust
#[derive(NetworkBehaviour)]
pub struct NetworkBehaviour {
    kademlia: Kademlia<MemoryStore>,
    identify: Identify,
    ping: Ping,
    autonat: AutoNAT,
    dcutr: DCUtR,
    mdns: TokioMdns,
    relay: Relay,
    gossipsub: Gossipsub,
}
```

### 2.3 Swarm Events

```rust
enum NetworkEvent {
    PeerConnected { peer_id: PeerId },
    PeerDisconnected { peer_id: PeerId },
    PeerDiscovered { peer_id: PeerId, addrs: Vec<Multiaddr> },
    DhtQueryCompleted { query_id: u64, results: Vec<PeerId> },
    GossipsubMessage { topic: String, data: Vec<u8> },
}
```

---

## 3. Peer Aggregate

### 3.1 PeerInfo

```rust
// network/peer.rs:22–41
pub struct PeerInfo {
    pub peer_id: String,            // VO: libp2p::PeerId stringified (ACL)
    pub addrs: Vec<String>,         // Multiaddrs
    pub protocols: Vec<String>,
    pub last_seen: u64,
    pub connection_count: u64,
    pub avg_latency_ms: Option<u64>,
    pub reputation: u8,             // 0..=100, in-band signal
}
```

### 3.2 PeerRecord (Internal)

```rust
struct PeerRecord {
    info: PeerInfo,
    addrs: HashSet<Multiaddr>,
    connected: bool,
    connected_at: Option<Instant>,
    latency_samples: Vec<Duration>,  // bounded ring buffer
}
```

### 3.3 PeerStore (Repository)

```rust
pub struct PeerStore {
    peers: DashMap<PeerId, PeerRecord>,
    connected: RwLock<HashSet<PeerId>>,
    config: PeerStoreConfig,
}

impl PeerStore {
    pub fn add_peer(&self, info: PeerInfo);
    pub fn get_peer(&self, peer_id: &PeerId) -> Option<PeerInfo>;
    pub fn peer_connected(&self, peer_id: &PeerId);
    pub fn update_latency(&self, peer_id: &PeerId, latency: Duration);
    
    pub fn increase_reputation(&self, peer_id: &PeerId, delta: u8);
    pub fn decrease_reputation(&self, peer_id: &PeerId, delta: u8);
    
    pub fn peers_by_reputation(&self) -> Vec<(PeerId, u8)>;
    pub fn peers_by_latency(&self) -> Vec<(PeerId, Duration)>;
    
    // Persistence
    pub fn save(&self, path: &Path) -> Result<()>;
    pub fn load(path: &Path) -> Result<Self>;
}
```

### 3.4 Config Presets

```rust
pub enum PeerStorePreset {
    LowMemory { max_peers: usize },    // ~1000
    IoT { max_peers: usize },           // ~100
    Mobile { max_peers: usize },        // ~500
    Server { max_peers: usize },        // ~10000
}
```

---

## 4. Two-Tier Reputation Model

### 4.1 Why Two Tiers?

**Problem**: Single reputation score conflates:
- Short-term transfer quality
- Long-term routing trust

**Solution**: Two independent models:
1. **EWMA** — Per-dimensional scoring (strict/lenient/performance profiles)
2. **Trust Graph** — Social trust propagation

---

### 4.2 Tier 1 — EWMA ReputationScore

```rust
// network/reputation.rs:215–279
pub struct ReputationScore {
    pub transfer_success_rate: f64,     // EWMA
    pub latency_score: f64,             // EWMA
    pub protocol_compliance_score: f64, // EWMA
    pub uptime_score: f64,              // EWMA
    
    pub successful_transfers: u64,
    pub failed_transfers: u64,
    pub protocol_violations: u64,
}

impl ReputationScore {
    pub fn overall(&self, weights: &ReputationWeights) -> f64 {
        weights.transfer_success * self.transfer_success_rate +
        weights.latency * self.latency_score +
        weights.protocol_compliance * self.protocol_compliance_score +
        weights.uptime * self.uptime_score
    }
    
    // EWMA update: s_new = α·signal + (1-α)·s_old
    pub fn update(&mut self, alpha: f64, signal: f64) {
        self.transfer_success_rate = alpha * signal + (1.0 - alpha) * self.transfer_success_rate;
    }
    
    // Decay: s *= (1 - decay_rate)
    pub fn decay(&mut self, decay_rate: f64) {
        self.transfer_success_rate *= (1.0 - decay_rate);
    }
}
```

### 4.3 Reputation Profiles

```rust
pub enum ReputationProfile {
    Strict {
        threshold: f64,           // 0.85
        weights: ReputationWeights,
    },
    Lenient {
        threshold: f64,           // 0.5
        weights: ReputationWeights,
    },
    Performance {
        latency_weight: f64,      // 0.5
        weights: ReputationWeights,
    },
}
```

---

### 4.4 Tier 2 — Trust Graph

```rust
// network/peer_reputation_graph.rs:115–130
pub struct PeerReputationGraph {
    edges: DashMap<PeerId, HashMap<PeerId, f64>>,  // trust → trust_weight ∈ [0,1]
    scores: DashMap<PeerId, ReputationScore>,
    config: GraphConfig,
}

pub struct ReputationScore {
    pub direct_score: f64,        // EMA from direct interactions
    pub propagated_score: f64,    // BFS multi-hop trust
    pub combined_score: f64,      // 0.6×direct + 0.4×propagated
    pub confidence: f64,          // Based on interaction count
    pub percentile: f64,          // Relative to other peers
}

impl PeerReputationGraph {
    // BFS trust propagation
    pub fn compute_propagated_score(&self, peer_id: &PeerId) -> f64 {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        let mut total_trust = 0.0;
        
        // Start from direct trust edges
        for (neighbor, weight) in self.edges.get(peer_id).unwrap().iter() {
            queue.push_back((*neighbor, *weight, 0));  // (peer, weight, depth)
        }
        
        while let Some((current, weight, depth)) = queue.pop_front() {
            if depth >= 3 || visited.contains(&current) {
                continue;
            }
            visited.insert(current);
            
            // Damping: 0.5^depth
            total_trust += weight * 0.5_f64.powi(depth as i32);
            
            // Propagate to neighbors
            for (neighbor, edge_weight) in self.edges.get(&current).unwrap().iter() {
                queue.push_back((*neighbor, weight * edge_weight * 0.5, depth + 1));
            }
        }
        
        total_trust
    }
    
    pub fn combined_score(&self, peer_id: &PeerId) -> f64 {
        let direct = self.direct_score(peer_id);
        let propagated = self.compute_propagated_score(peer_id);
        0.6 * direct + 0.4 * propagated
    }
    
    // Prune edges < 0.01 (noise)
    pub fn prune_isolated(&self);
    
    // Decay all edges: weight *= 0.99
    pub fn decay(&self);
}
```

### 4.5 Graph Parameters

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| Propagation depth | 3 | Beyond 3 hops, trust dilutes |
| Damping factor | 0.5/hop | Trust decreases with distance |
| Prune threshold | 0.01 | Remove noise |
| Decay rate | 0.99/tick | Forget old trust |

---

## 5. DHT Abstraction

### 5.1 DhtProvider Trait (Port)

```rust
// network/dht_provider.rs:138–191
#[async_trait]
pub trait DhtProvider: Send + Sync {
    async fn bootstrap(&self) -> Result<()>;
    async fn provide(&self, cid: &Cid) -> Result<()>;
    async fn find_providers(&self, cid: &Cid) -> Result<Vec<PeerId>>;
    async fn find_peer(&self, peer_id: &PeerId) -> Result<Vec<Multiaddr>>;
    async fn get_closest_peers(&self, cid: &Cid) -> Result<Vec<PeerId>>;
    async fn put_value(&self, key: &[u8], value: &[u8]) -> Result<()>;
    async fn get_value(&self, key: &[u8]) -> Result<Option<Vec<u8>>>;
    
    fn stats(&self) -> DhtStats;
    fn is_healthy(&self) -> bool;
}
```

### 5.2 KademliaDhtProvider (Adapter)

```rust
pub struct KademliaDhtProvider {
    kademlia: Arc<Mutex<Kademlia<MemoryStore>>>,
    config: KademliaConfig,
}

pub struct KademliaConfig {
    pub bucket_size: usize,           // 20 (k-bucket)
    pub replication_factor: usize,    // 20
    pub query_timeout: Duration,      // 60s
    pub reannounce_interval: Duration, // 12h
    pub alpha: usize,                 // 3 (parallel queries)
}
```

### 5.3 DhtProviderRegistry

```rust
pub struct DhtProviderRegistry {
    providers: DashMap<String, Box<dyn DhtProvider>>,
    default: String,
}

impl DhtProviderRegistry {
    pub fn register(&self, name: &str, provider: Box<dyn DhtProvider>);
    pub fn get(&self, name: &str) -> Option<&dyn DhtProvider>;
    pub fn default_provider(&self) -> &dyn DhtProvider;
}
```

---

## 6. Semantic DHT

### 6.1 Concept

**Problem**: Traditional DHT routes by hash → random peer distribution.

**Solution**: Semantic DHT routes by **embedding similarity** → cluster similar content.

### 6.2 LSH-Based Routing

```rust
// network/semantic_dht.rs
pub struct SemanticDht {
    lsh_projections: Vec<Vec<f32>>,   // Locality-sensitive hash
    namespaces: EnumMap<NamespaceId, NamespaceConfig>,
    shard_balancer: ShardBalancer,
}

pub enum NamespaceId {
    Text,
    Image,
    Audio,
    Custom(u64),
}

pub struct VectorAnnotatedRecord {
    pub cid: Cid,
    pub lsh_hash: LshHash,
    pub embedding_dim: usize,
    pub namespace: NamespaceId,
    pub timestamp: u64,
}
```

### 6.3 Operations

```rust
impl SemanticDht {
    // Index content with embedding
    pub async fn index_content(&self, cid: &Cid, embedding: &[f32]) -> Result<()>;
    
    // Query: find peers with similar embeddings
    pub async fn query(&self, embedding: &[f32], k: usize) -> Result<Vec<PeerId>>;
    
    // LSH hash → DHT key mapping
    fn lsh_hash(&self, embedding: &[f32]) -> LshHash;
}
```

### 6.4 Distributed Search

```
Query(embedding)
  │
  ├─► LSH hash → DHT key
  │
  ├─► DHT lookup → peers with similar embeddings
  │
  ├─► BFS from seed peers
  │
  └─► Converge at 0.95 agreement
```

---

## 7. Anti-Corruption Layer

### 7.1 libp2p Wrapping

Network context **wraps** libp2p types into domain VOs:

```rust
// network/identity.rs
pub fn peer_id_to_string(peer_id: &libp2p::PeerId) -> String {
    peer_id.to_base58()
}

pub fn string_to_peer_id(s: &str) -> Result<libp2p::PeerId> {
    libp2p::PeerId::from_str(s).map_err(|_| Error::InvalidPeerId)
}

// network/message_codec.rs
pub fn multiaddr_to_string(addr: &Multiaddr) -> String {
    addr.to_string()
}
```

### 7.2 Why ACL?

- **Domain isolation**: Network domain doesn't leak libp2p types
- **Testability**: Can mock `PeerId`, `Multiaddr`
- **Flexibility**: Could swap libp2p for another p2p stack

---

## 8. Event Bus

### 8.1 NebNetworkEvent

```rust
pub struct NebNetworkEvent {
    pub id: u64,
    pub topic: String,
    pub payload: Vec<u8>,
    pub source_peer: Option<PeerId>,
    pub timestamp: u64,
}
```

### 8.2 Event Bus

```rust
pub struct NetworkEventBus {
    subscribers: DashMap<String, broadcast::Sender<NebNetworkEvent>>,
    replay_buffer: VecDeque<NebNetworkEvent>,  // 10k events
    config: EventBusConfig,
}

impl NetworkEventBus {
    pub fn subscribe(&self, topic: &str) -> broadcast::Receiver<NebNetworkEvent>;
    pub fn publish(&self, event: NebNetworkEvent);
    pub fn replay(&self, filter: &EventFilter) -> Vec<NebNetworkEvent>;
}
```

---

## 9. NetworkFacade — Builder Pattern

### 9.1 Builder

```rust
pub struct NetworkFacadeBuilder {
    // Required
    keypair: Keypair,
    listen_addrs: Vec<Multiaddr>,
    
    // Optional modules
    semantic_dht: Option<SemanticDhtConfig>,
    gossipsub: Option<GossipsubConfig>,
    geo_routing: Option<GeoRoutingConfig>,
    quality_predictor: Option<QualityPredictorConfig>,
    peer_selector: Option<PeerSelectorConfig>,
    multipath_quic: Option<MultipathQuicConfig>,
    tor: Option<TorConfig>,
    bandwidth_throttle: Option<BandwidthThrottleConfig>,
    adaptive_polling: Option<AdaptivePollingConfig>,
    background_mode: Option<BackgroundModeConfig>,
    offline_queue: Option<OfflineQueueConfig>,
    memory_monitor: Option<MemoryMonitorConfig>,
    network_monitor: Option<NetworkMonitorConfig>,
    query_batcher: Option<QueryBatcherConfig>,
}

impl NetworkFacadeBuilder {
    pub fn build(self) -> Result<NetworkFacade>;
}
```

### 9.2 15+ Optional Modules

| Module | Purpose |
|--------|---------|
| `semantic_dht` | LSH-based routing |
| `gossipsub` | Pub/sub messaging |
| `geo_routing` | Latency-based routing |
| `quality_predictor` | Predict peer quality |
| `peer_selector` | Custom selection logic |
| `multipath_quic` | Multi-path QUIC |
| `tor` | Tor transport |
| `bandwidth_throttle` | Rate limiting |
| `adaptive_polling` | Dynamic poll intervals |
| `background_mode` | Low-power mode |
| `offline_queue` | Queue operations offline |
| `memory_monitor` | Memory limits |
| `network_monitor` | Health metrics |
| `query_batcher` | Batch DHT queries |

---

## 10. Invariants

| Invariant | Enforcement |
|-----------|-------------|
| `PeerId = hash(pubkey)` | libp2p identity module |
| `reputation ∈ [0, 100]` | `saturating_add/sub` |
| Trust weight ∈ [0, 1] | Clamped on edge add |
| Propagation depth ≤ 3 | BFS termination |
| DHT re-announce every 12h | Scheduler |

---

## 11. Performance Characteristics

### Latency

| Operation | P50 | P99 | Notes |
|-----------|-----|-----|-------|
| DHT lookup | 150ms | 300ms | Kademlia |
| Peer connect | 50ms | 200ms | QUIC handshake |
| Reputation update | 1µs | 10µs | EWMA update |
| Trust graph BFS | 10ms | 100ms | Depth 3 |
| Semantic DHT query | 200ms | 500ms | + ANN latency |

### Memory

| Component | Memory |
|-----------|--------|
| PeerStore (10k peers) | ~50 MB |
| Trust graph (100k edges) | ~20 MB |
| DHT routing table | ~10 MB |
| Event replay buffer | ~100 MB |

---

## 12. Scalability

### 12.1 Sharded Routing Table

```rust
// network/routing_table_sharding.rs
pub struct ShardedRoutingTable {
    shards: Vec<RwLock<LocalRoutingTable>>,
    shard_count: usize,
}
```

**Reduces lock contention** — each shard has independent lock.

### 12.2 Connection Pool

```rust
pub struct ConnectionPool {
    connections: DashMap<PeerId, Connection>,
    max_per_peer: usize,
}
```

---

## 13. Key Files

| File | Lines | Purpose |
|------|-------|---------|
| `node.rs` | 400+ | NetworkNode |
| `peer.rs` | 350+ | PeerStore, PeerInfo |
| `reputation.rs` | 300+ | EWMA reputation |
| `peer_reputation_graph.rs` | 400+ | Trust graph |
| `dht_provider.rs` | 250+ | DHT trait |
| `semantic_dht.rs` | 350+ | LSH routing |
| `identity.rs` | 200+ | Key management |
| `network_event_bus.rs` | 200+ | Event bus |
| `facade.rs` | 500+ | NetworkFacade builder |

---

## 14. Design Decisions

### 14.1 Why Two-Tier Reputation?

**Decision**: Separate EWMA + Trust Graph.

**Rationale**:
- EWMA for operational quality
- Trust Graph for social trust
- Combined = best of both

**Trade-off**: Duplicate logic, higher complexity.

---

### 14.2 Why ACL to libp2p?

**Decision**: Wrap libp2p types as domain VOs.

**Rationale**:
- Domain isolation
- Testability
- Flexibility (swap libp2p)

---

### 14.3 Why Semantic DHT?

**Decision**: LSH-based routing for embeddings.

**Rationale**:
- Cluster similar content
- Faster ANN search
- Natural sharding

**Trade-off**: Additional complexity, not needed for all use cases.

---

## 15. Context Integration

```
┌─────────────────────────────────────────────────────────────────────┐
│                    NETWORK INTEGRATION                              │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Consumed by (Customer/Supplier):                                   │
│    • Transport — PeerId, Multiaddr                                  │
│    • Semantic — SemanticDht                                         │
│    • Application — Node orchestrator                                │
│                                                                     │
│  Publishes:                                                         │
│    • NebNetworkEvent — Observability                                │
│                                                                     │
│  Shared Kernel usage:                                               │
│    • Cid (DHT key)                                                  │
│    • Error, Result                                                  │
│                                                                     │
│  ACL:                                                               │
│    • libp2p → String wrappers                                       │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

**Next**: [05-SemanticContext.md](05-SemanticContext.md) — HNSW, DiskANN, quantization
