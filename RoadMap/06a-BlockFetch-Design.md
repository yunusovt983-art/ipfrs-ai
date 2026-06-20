---
title: 06a-BlockFetch-Design
type: design
summary: Детальный дизайн Фазы 1.1 — реальная выкачка блоков по swarm через libp2p request-response (/ipfrs/blockfetch/1.0.0)
tags: [ipfrs, geo-inference, bitswap, request-response, libp2p, design]
updated: 2026-06-19
---

# Фаза 1.1 — Block-fetch по swarm (детальный дизайн)

> Решение из [[ADR-GeoInference]] ADR-001: **libp2p `request-response`** протокол
> `/ipfrs/blockfetch/1.0.0` (не полный Bitswap-behaviour на старте). Планирование «кому
> слать» берём из `ipfrs-transport::BitswapExchange`; здесь — только тонкий wire.

---

## 1. Что меняем (карта правок)

| Файл | Правка |
|------|--------|
| `ipfrs-network/Cargo.toml` | feature `request-response` у libp2p (если не включён) |
| `ipfrs-network/src/blockfetch.rs` (новый) | кодек протокола + типы `BlockRequest`/`BlockResponse` |
| `ipfrs-network/src/node.rs:288` | добавить поле `blockfetch` в `IpfrsBehaviour` |
| `ipfrs-network/src/node.rs:364` | вариант `SwarmCommand::FetchBlock{peer,cid,reply}` |
| `ipfrs-network/src/node.rs:828` | обработка событий `request_response` (входящие запросы + ответы) |
| `ipfrs-network/src/node.rs:1298` | переписать `fetch_block_from_peer` на отправку команды |

---

## 2. Протокол и типы

```rust
// ipfrs-network/src/blockfetch.rs (новый файл)
use ipfrs_core::Cid;

/// Имя протокола для libp2p request-response.
pub const PROTOCOL: &str = "/ipfrs/blockfetch/1.0.0";

/// Запрос блока по CID.
#[derive(Debug, Clone)]
pub struct BlockRequest {
    pub cid: Cid,                 // что хотим
    pub max_size: u32,            // защита: не принимать ответ больше N (деф. 2 MiB = MAX_BLOCK_SIZE)
}

/// Ответ: либо данные блока, либо «нет».
#[derive(Debug, Clone)]
pub enum BlockResponse {
    Block { data: Vec<u8> },      // получатель проверит cid == hash(data)
    NotFound,
    TooLarge,                     // запрошенный блок > max_size
}
```

**Кодек** — реализовать `request_response::Codec` (read/write request/response). Формат
кадра: `varint(len) ++ payload`; CID и enum-теги — через уже используемый в проекте
`oxicode`/serde (как в `ipfrs-transport/src/messages.rs`). Лимит чтения = `max_size + заголовок`.

---

## 3. Включение в IpfrsBehaviour

```rust
// node.rs:288 — добавить поле
#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "IpfrsBehaviourEvent")]
pub struct IpfrsBehaviour {
    pub kademlia: kad::Behaviour<kad::store::MemoryStore>,
    pub identify: identify::Behaviour,
    pub ping: ping::Behaviour,
    pub autonat: autonat::Behaviour,
    pub dcutr: dcutr::Behaviour,
    pub mdns: mdns::tokio::Behaviour,
    pub relay_client: relay::client::Behaviour,
    pub blockfetch: request_response::Behaviour<BlockFetchCodec>,   // ← НОВОЕ
}
```
В `build_swarm` (`node.rs:599`) создать behaviour:
```rust
let blockfetch = request_response::Behaviour::new(
    [(StreamProtocol::new(PROTOCOL), ProtocolSupport::Full)],
    request_response::Config::default().with_request_timeout(Duration::from_secs(30)),
);
```

---

## 4. Поток данных (sequence)

```
Node A (нужен cid)                         Node B (держит cid)
   │  fetch_block_from_peer(B, cid)
   │  ── SwarmCommand::FetchBlock{B,cid,reply} ─▶ swarm-loop A
   │                                   blockfetch.send_request(B, BlockRequest{cid,2MiB})
   │                                            ──── /ipfrs/blockfetch/1.0.0 ───▶ swarm B
   │                                                          InboundRequest{cid}
   │                                                          store.get(cid) → Some(block)
   │                                            ◀─── BlockResponse::Block{data} ────
   │           OutboundResponse{data}
   │           verify: Block::from_parts(cid,data).verify()? == true
   │  reply.send(Ok(block))  ──▶ await в fetch_block_from_peer
   ▼  Ok(Block)
```

---

## 5. Реализация по шагам (DoD на каждый)

### Шаг 1 — типы + кодек (`blockfetch.rs`)
`BlockRequest`/`BlockResponse` + `BlockFetchCodec: request_response::Codec`.
**DoD:** unit-тест round-trip кодека (encode→decode идемпотентен).

### Шаг 2 — встроить behaviour
Добавить поле `blockfetch` (`node.rs:288`) и его создание в `build_swarm`.
**DoD:** `cargo build -p ipfrs-network` зелёный; узел стартует.

### Шаг 3 — исходящий запрос
- `SwarmCommand::FetchBlock { peer: PeerId, cid: Cid, reply: oneshot::Sender<Result<Block>> }`
  (`node.rs:364`).
- В match-цикле (`node.rs:1083`): `let id = swarm.behaviour_mut().blockfetch.send_request(&peer, BlockRequest{...}); pending_fetch.insert(id, (cid, reply));`
- Хранить `pending_fetch: HashMap<OutboundRequestId, (Cid, oneshot::Sender<...>)>` в swarm-loop
  (по аналогии с `provider_waiters`, но не лоссивный ключ — id типизирован, см. баг
  `node.rs:1234`).
**DoD:** команда доходит до swarm-цикла.

### Шаг 4 — обработка событий request-response
В `handle_swarm_event` (`node.rs:828`) добавить ветку `IpfrsBehaviourEvent::Blockfetch(...)`:
- `Message::Request { request, channel }` → `store.get(request.cid)`:
  - есть и ≤ max_size → `send_response(channel, BlockResponse::Block{data})`;
  - нет → `NotFound`; больше лимита → `TooLarge`.
- `Message::Response { request_id, response }` → достать `(cid, reply)` из `pending_fetch`:
  - `Block{data}` → `let b = Block::from_parts(cid, data.into()); if b.verify()? { reply.send(Ok(b)) } else { reply.send(Err(Verification)) }`
  - `NotFound`/`TooLarge` → `reply.send(Err(NotFound))`.
- `OutboundFailure`/`InboundFailure` → `reply.send(Err(Network))`.
**DoD:** интеграционный тест из двух узлов: A забирает блок у B, `verify()` проходит;
несуществующий CID → `NotFound`.

### Шаг 5 — переписать `fetch_block_from_peer`
```rust
// node.rs:1298 — было: всегда NotFound
pub async fn fetch_block_from_peer(&mut self, peer: &PeerId, cid: &Cid) -> IpfrsResult<Block> {
    if !self.connected_peers.contains(peer) {
        return Err(Error::Network(format!("Peer {peer} is not connected")));
    }
    let (tx, rx) = oneshot::channel();
    self.send_swarm_cmd(SwarmCommand::FetchBlock { peer: *peer, cid: *cid, reply: tx })?;
    tokio::time::timeout(Duration::from_secs(30), rx)
        .await
        .map_err(|_| Error::Network("block fetch timeout".into()))?
        .map_err(|_| Error::Network("fetch channel closed".into()))?
}
```
**DoD:** `Node::get` на промахе локально + наличии провайдера реально возвращает блок
(закрывает заглушку и чинит P2P GET в [[../Wiki/10-DataFlows]] §2 шаг 3).

---

## 6. Инварианты и безопасность

| Инвариант | Как обеспечиваем |
|-----------|------------------|
| Целостность: `cid == hash(data)` | `Block::from_parts(cid,data).verify()` на стороне получателя (`block.rs:117`) |
| Защита от oversize | `max_size` в запросе + лимит чтения в кодеке (деф. 2 MiB = `MAX_BLOCK_SIZE`) |
| Не висим вечно | `request_timeout=30s` + внешний `tokio::timeout` |
| Только подключённым | проверка `connected_peers` (как сейчас, `node.rs:1300`) |
| Типизированный ключ запроса | `OutboundRequestId` (НЕ lossy-UTF-8, в отличие от провайдер-вейтера) |

---

## 7. Тест-план

1. **unit**: кодек round-trip; verify-fail при подменённых данных.
2. **integration (2 ноды, in-memory transport)**: успешная выкачка; `NotFound`; `TooLarge`;
   таймаут (B не отвечает).
3. **e2e**: `Node::get(cid)` с пустым локальным стором и одним удалённым провайдером →
   блок выкачан и записан в локальный стор (backfill, `block_ops.rs:159`).

---

## 8. Что дальше после 1.1

- **1.2** gossipsub-behaviour (анонс `model_cid`).
- **1.3** транспорт семантического DHT (тот же request-response паттерн для top-k).
- Позже — миграция на полноценный Bitswap-behaviour (ADR-001, вариант A): want-list батчи,
  дедуп дубликатов, IPFS-совместимость; API `Node::get` не меняется.

---

**Связанные**: [[06-GeoInference]] | [[ADR-GeoInference]] | [[../Wiki/05-NetworkContext]] | [[../Wiki/08-TransportContext]]
**Источник кода**: `ipfrs-network/src/node.rs:288,364,828,1298`, `ipfrs-transport/src/bitswap.rs:75`
