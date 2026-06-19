---
title: 08-TransportContext
type: domain
summary: Transport Context (ipfrs-transport) — агрегат Session, Bitswap, TensorSwap, GraphSync, приоритетные want-list; вопрос двух Bitswap
tags: [ipfrs, ddd, transport, bitswap, tensorswap, session, graphsync]
source: ipfrs_source/crates/ipfrs-transport/src/
related: ["[[05-NetworkContext]]", "[[04-StorageContext]]", "[[11-RealityCheck]]"]
read_time: 13 мин
updated: 2026-06-19
---

# Transport Context — `ipfrs-transport`

**Краткое резюме**: Transport отвечает за **обмен блоками между пирами** — Bitswap,
GraphSync, приоритетные want-list и ML-специализированный TensorSwap. Корневой
агрегат — `Session`. Самое важное наблюдение: **Bitswap реализован дважды** — здесь
и в `ipfrs-network` — несовместимо.

---

## 1. Корневой агрегат: `Session`

```rust
// session.rs:199 (сокращено)
pub struct Session {
    id: SessionId,                            // = u64
    state: Arc<RwLock<SessionState>>,
    blocks: Arc<DashMap<Cid, BlockRequest>>,  // внутренние сущности
    stats: Arc<RwLock<SessionStats>>,
    state_tx/rx: watch::*,                     // реактивный сигнал завершения
}
```

- **Граница агрегата**: `BlockRequest` (`session.rs:159`) — приватная внутренняя
  сущность, наружу не утекает; внешний код мутирует её только через методы `Session`
  (`add_block`, `mark_received`, `mark_failed`).
- **Репозиторий**: `SessionManager` (`session.rs:463`) над `DashMap<SessionId, Arc<Session>>`.
- **События**: `SessionEvent` (`Started/BlockReceived/BlockFailed/Progress/Completed/Cancelled`).

Инвариант завершения: сессия завершена ⟺ `recv + failed ≥ total` (`session.rs:151`).
Состояние меняется **вне** stats-lock (`session.rs:323`) — осознанное избегание
дедлока. ⚠️ Состояние `Completing` объявлено, но **никогда не присваивается** (мёртвое).

---

## 2. Bitswap-движок

`BitswapExchange<S: BlockStore>` (`bitswap.rs:75`) композирует `ConcurrentWantList`,
`ConcurrentPeerManager` и `pending_requests`. Операции:
- `want(cid, priority)` (`bitswap.rs:106`) — добавить в want-list; при наличии —
  только повышение приоритета.
- `select_peers_for_request` (`bitswap.rs:184`) — стратегия «провайдеры первыми».
- `receive_block` (`bitswap.rs:203`) — `store.put`, учёт латентности, отмена want.

⚠️ В этом крейте **нет различия want-have vs want-block** и **нет сообщения `WantHave`**.
Реальный IPFS Bitswap 1.2 их разделяет; здесь присутствие узнаётся лишь из *входящих*
`Have`/`DontHave`. `PeerLedger`/«fair-leeching» из вики тоже **не существуют**.

---

## 3. TensorSwap — ML-расширение (самая полная часть)

`TensorSwap<S>` (`tensorswap/core.rs:57`) **оборачивает** `BitswapExchange` (композиция,
не замена). Реально реализованы:
- `want_tensor` (`core.rs:98`) — запрос тензора **+ его зависимостей** с повышенным
  приоритетом.
- Прогрессивный стриминг: ранние чанки получают выше приоритет (`core.rs:167`).
- `einsum.rs` — граф зависимостей из einsum-выражения управляет порядком выкачки.
- `gradient.rs` — чанкованная передача градиентов с CRC-32 (Arrow IPC).
- `streaming.rs:424` — **реальный** парсинг safetensors-заголовка.
- Backpressure: счётчик-watermark (`streaming.rs:328`), pause на high (48), resume на
  low (16).

> TensorSwap — наиболее доменно-специфичная и **наиболее завершённая** часть контекста
> (в отличие от GraphSync/erasure/NAT — см. §6).

---

## 4. Приоритеты и протокол

- Полосы приоритета: `Low=0, Normal=50, High=100, Urgent=200, Critical=300`
  (`want_list.rs:70`); `effective_priority` поднимает по близости дедлайна (`want_list.rs:170`).
- Куча: приоритет ↓, затем `created_at` ↑ → **FIFO внутри полосы** (`want_list.rs:203`).
- Сообщения Bitswap (`messages.rs:59`): `WantList | Block | Have | DontHave | Cancel`;
  кодирование `oxicode`/serde, CID как строка.

---

## 5. Ключевые инварианты

| Инвариант | Источник |
|-----------|----------|
| Один `WantEntry` на CID (дедуп) | `want_list.rs:252` |
| `WantList` ≤ `max_wants` (деф. 1024) | `want_list.rs:257` |
| Приоритет только повышается на дубль | `bitswap.rs:109` |
| FIFO внутри равного приоритета | `want_list.rs:208` |
| Сессия завершена ⟺ recv+failed ≥ total | `session.rs:151` |
| Переход состояния вне stats-lock | `session.rs:323` |
| Backoff = `base·2^min(retry,10)` + 10% jitter | `want_list.rs:437` |

---

## 6. Что реально работает, а что заглушка

| Подсистема | Статус |
|------------|--------|
| `Session` + `SessionManager` (жизненный цикл, события) | ✅ работает |
| Bitswap want/have/block, приоритеты, выбор пиров | ✅ работает |
| TensorSwap (стриминг, einsum-граф, safetensors, градиенты) | ✅ работает |
| Prefetch / request coalescing | ✅ работает |
| Мульти-транспорт (QUIC/TCP/WS) выбор | ✅ работает (но ⚠️ fallback-баг, см. ниже) |
| **GraphSync DAG-обход** | ⚠️ заглушка: `extract_links` возвращает `Ok(Vec::new())` → обходит только корень (`graphsync.rs:377`) |
| **Erasure (Reed-Solomon)** | ⚠️ заглушка: decode возвращает `DecodingFailed`; encode — взвешенный XOR (`erasure.rs:299`) |
| **NAT traversal (STUN/TURN/ICE)** | ⚠️ симуляция: dummy-адреса, «успех» захардкожен (`nat_traversal.rs:367,665`) |
| `MultiTransportManager::find_transport` | ⚠️ баг: fallback не может выбрать непредпочтённый транспорт (`multi_transport.rs:215`) |

---

## 7. Вопрос двух Bitswap (важно)

Есть **две независимые реализации Bitswap**:

| | `ipfrs-transport/bitswap.rs` | `ipfrs-network/bitswap.rs` |
|---|---|---|
| Тип | `BitswapExchange<S>` (`:75`) | `Bitswap` (`:15`) |
| Want-list | приоритетная куча + дедуп | неупорядоченный `HashSet<Cid>` |
| PeerId | `String` (`peer_manager.rs:40`) | `libp2p::PeerId` |
| Зрелость | богатое планирование, выбор пиров | минимальный каркас |

⚠️ Это **не два слоя одного дизайна, а параллельные несовместимые реализации**
(разные типы PeerId и сообщений). Transport-версия — продвинутый движок планирования;
network-версия — тонкая заглушка. Ни один не импортирует другой. Это самый
значимый кросс-контекстный запах ([[11-RealityCheck]]).

> Про «баг backpressure-семафора» из старых заметок: он **не здесь**. Семафорный
> backpressure живёт в `ipfrs-interface/src/backpressure.rs` и, судя по коду+тестам,
> **уже исправлен**. У transport свой watermark-счётчик, без семафора.

---

## Что дальше?

- **Кто находит провайдеров для выкачки** → [[05-NetworkContext]]
- **Куда складываются полученные блоки** → [[04-StorageContext]]
- **Сценарий GET целиком** → [[10-DataFlows]]

**Связанные**: [[05-NetworkContext]] | [[04-StorageContext]] | [[10-DataFlows]] | [[11-RealityCheck]]
**Источник кода**: `ipfrs-transport/src/{session,bitswap,want_list,messages,graphsync,tensorswap/,multi_transport,erasure,nat_traversal}.rs`
