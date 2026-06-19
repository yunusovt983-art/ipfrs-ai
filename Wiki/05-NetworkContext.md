---
title: 05-NetworkContext
type: domain
summary: Network Context (ipfrs-network) — NetworkNode + libp2p swarm, Kademlia DHT, репутация; ключевое разделение Tier A (живой swarm) vs Tier B (неподключённые модули)
tags: [ipfrs, ddd, network, libp2p, dht, kademlia, reputation]
source: ipfrs_source/crates/ipfrs-network/src/
related: ["[[02-StrategicDesign]]", "[[08-TransportContext]]", "[[11-RealityCheck]]"]
read_time: 14 мин
updated: 2026-06-19
---

# Network Context — `ipfrs-network`

**Краткое резюме**: Network отвечает на вопрос «**кто и где хранит данные?**» —
P2P-слой на libp2p + QUIC. Его корневой агрегат — `NetworkNode`, владеющий реальным
`libp2p::Swarm`. Ключевая архитектурная правда: крейт состоит из **двух тиров,
которые почти не соприкасаются**.

---

## 1. Главный факт: Tier A vs Tier B

| | **Tier A — живое ядро** | **Tier B — спутниковый слой** |
|---|---|---|
| Что | `node.rs`: `NetworkNode` + `IpfrsBehaviour`, владеет `libp2p::Swarm` | ~180 модулей: репутация, бан-листы, пулы соединений, таблицы маршрутизации |
| I/O | реальный сетевой ввод-вывод | in-memory автоматы, тикающие на логических счётчиках |
| Связь с swarm | `node.rs` подтягивает ровно **один** модуль — `gossipsub::GossipSubManager` (`node.rs:428`) | `node.rs` не ссылается ни на `ReputationManager`, ни на `ConnectionManager`, ни на `PeerBanList` |

> Мост между тирами — `facade.rs::NetworkFacade` (`facade.rs:93`): держит `NetworkNode`
> + ~17 подсистем как соседние поля `Arc<RwLock<…>>`. Но он их **со-размещает**, а не
> кормит событиями swarm. ⚠️ **Большая часть «доменной логики» сейчас декоративна
> относительно живого пути данных** — главный нюанс этого контекста ([[11-RealityCheck]]).

---

## 2. Корневой агрегат: `NetworkNode`

```rust
// node.rs:404 (сокращено)
pub struct NetworkNode {
    config: NetworkConfig, peer_id: PeerId,
    swarm: Option<Swarm<IpfrsBehaviour>>,            // забирается на start()
    swarm_cmd_tx: Option<mpsc::Sender<SwarmCommand>>,
    event_tx: mpsc::Sender<NetworkEvent>,            // mpsc(1024)
    connected_peers: Arc<DashSet<PeerId>>,
    provider_waiters: ProviderWaiters,
    pub gossipsub: Arc<GossipSubManager>,            // единственный Tier-B модуль внутри
    pub inference_waiters: InferenceWaiters,         // для распределённого вывода
    // ...
}
```

Композитное поведение (`node.rs:288`) содержит **7 behaviour'ов**: kademlia,
identify, ping, autonat, dcutr, mdns, relay_client.

> ⚠️ Обе старые вики утверждают, что `gossipsub` и `bitswap` — поля `IpfrsBehaviour`.
> **Это не так**: их там нет. Значит, обмен блоками и gossip по проводу через swarm
> сейчас не активны на уровне behaviour. См. [[11-RealityCheck]].

---

## 3. Доменные сервисы

### 3.1 Маршрутизация контента (DHT) — живой путь
- `provide(cid)` → `kademlia.start_providing(RecordKey::new(&cid.to_bytes()))`
  (`node.rs:1198`).
- `find_providers(cid)` → `kademlia.get_providers(key)` (`node.rs:1214`);
  `find_providers_await` регистрирует oneshot-ожидание по CID с таймаутом (`node.rs:1229`).
- Kademlia настроен: query_timeout 60 c, replication 20, α=3, `OnConnected`
  (`node.rs:655`).

⚠️ **Хрупкость ключа ожидания** (`node.rs:1234` vs `864`): ключ строится
`String::from_utf8_lossy(&cid.to_bytes())` — бинарные байты CID не UTF-8, любой не-UTF-8
байт → U+FFFD, риск коллизии. Работает по совпадению, но хрупко.

### 3.2 Обнаружение пиров
mDNS (`node.rs:692`), bootstrap-дайлинг (`node.rs:753`), identify (`node.rs:846`).
`BootstrapManager` с экспоненциальным backoff (`bootstrap.rs`) существует, но **не
вызывается** из `node.rs` — он лишь возвращает адреса для внешнего вызова.

### 3.3 Репутация — четыре(!) сосуществующие модели
1. In-band `u8` на `PeerInfo`, старт 50, клэмп [0,100] (`peer.rs:40,314`).
2. EWMA-композит `ReputationManager` (transfer/latency/protocol/uptime) (`reputation.rs:300`).
3. Клэмп-трекер [-100,100] с временным затуханием (`reputation.rs:606`).
4. Граф доверия `PeerReputationGraph` — BFS-распространение, глубина 3, демпфирование
   0.5/шаг (`peer_reputation_graph.rs:239`) — параметры **совпадают** с вики.

⚠️ Модель №2 не реклэмпится после затухания/нарушений (`reputation.rs:256,272`):
композит может уйти ниже задуманного пола.

---

## 4. Доменные события

`NetworkEvent` (`node.rs:449`): `PeerConnected`, `PeerDisconnected`, `ContentFound{cid,providers}`,
`PeerDiscovered`, `DhtBootstrapCompleted`, `NatStatusChanged`, …

Поток: `SwarmEvent` → `handle_swarm_event` (`node.rs:828`) → маппинг в `NetworkEvent` →
`event_tx` mpsc(1024) → потребитель через `take_event_receiver`.

> ⚠️ Вики упоминают `DhtQueryCompleted{query_id}` и `GossipsubMessage` — таких
> вариантов в `NetworkEvent` **нет**.

---

## 5. Стек технологий (build_swarm, `node.rs:597`)

- **QUIC** (основной) + **TCP** (fallback), оба через **Noise** + **Yamux**.
- **Relay client** всегда вкомпилирован (`node.rs:610`).
- Behaviour'ы: Kademlia (`MemoryStore`), Identify (`/ipfrs/1.0.0`), Ping (15 c),
  AutoNAT, DCUtR, mDNS. Idle timeout 60 c.
- Неожиданная зависимость от `ipfrs-tensorlogic` для распределённого вывода через
  gossip (`InferenceWaiters`, `node.rs:32`).

---

## 6. Интеграция и границы

- **Shared Kernel**: `Cid` напрямую как ключ DHT (`RecordKey::new(&cid.to_bytes())`),
  `Block`, `Error`.
- **Transport / Bitswap**: предполагаемая граница — `fetch_block_from_peer`
  (`node.rs:1298`), но ⚠️ она **возвращает `NotFound`** — обмен по проводу это явная
  заглушка («pending Task E», `node.rs:1311`). Значит **Network находит провайдеров
  (DHT), но не качает блоки** — это делает Transport-контекст ([[08-TransportContext]]).
- **ACL к libp2p**: частичный. Tier-B модули используют `String`/`NodeId` (де-факто
  ACL), Tier A — сырой `libp2p::PeerId`. Единого newtype `PeerId(String)` (как в вики)
  **нет**.

---

## 7. Что реально работает, а что заглушка

| Подсистема | Статус |
|------------|--------|
| Kademlia DHT (provide/find_providers) | ✅ работает (живой путь) |
| QUIC/TCP/Noise/Yamux/NAT (AutoNAT/DCUtR/relay) | ✅ работает |
| mDNS/identify обнаружение | ✅ работает |
| Bitswap-выкачивание блоков через swarm | ⚠️ заглушка (`node.rs:1311`) |
| Gossipsub по проводу | ⚠️ только in-process (нет libp2p-gossipsub behaviour) |
| `KademliaDhtProvider` (порт DHT) | ⚠️ все 12 методов — заглушки (`dht_provider.rs:388`) |
| Репутация/бан-листы влияют на реальные дайлы | ⚠️ нет (Tier B не подключён к swarm) |

Полностью — [[11-RealityCheck]].

---

## Что дальше?

- **Кто реально качает блоки** → [[08-TransportContext]]
- **Как `Node` зовёт DHT в сценарии GET** → [[10-DataFlows]]

**Связанные**: [[02-StrategicDesign]] | [[08-TransportContext]] | [[10-DataFlows]] | [[11-RealityCheck]]
**Источник кода**: `ipfrs-network/src/{node,facade,bitswap,reputation,peer_reputation_graph,dht,dht_provider,bootstrap}.rs`
