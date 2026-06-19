---
title: 12-RealityCheck
type: reference
summary: Проверка реальности — что в IPFRS реально работает, что заглушка, и опровергнутые «факты» о безопасности (по глубокому исследованию 2026-06-19)
tags: [ipfrs, reality-check, stubs, security, tech-debt]
source: crates/**
related: ["[[03-BoundedContexts]]", "[[07-LogicDomain]]", "[[11-ErrorHandling]]"]
read_time: 12 мин
updated: 2026-06-19
---

# Проверка реальности: модель vs код

**Краткое резюме**: Эта страница добавлена после **глубокого параллельного
исследования всех 7 контекстов (7 агентов, привязка `file:line`)**. Она честно
разделяет **спроектированную модель** и **фактическую реализацию**. Главный вывод:
ядро каждого контекста работает, но многие «фичи» — заглушки, а часть «критических
багов» из RoadMap по факту кода уже корректны.

> Эта страница — конденсат. Полный сквозной разбор «как функционирует IPFRS» с DDD-
> структурой см. в соседней базе `[[../Wiki/00-INDEX|Wiki «Как это работает»]]` и её
> странице `[[../Wiki/11-RealityCheck]]`.

---

## 1. Опровергнутые «критические баги» безопасности

| Утверждение (RoadMap/старые заметки) | Реальность по коду | Источник |
|--------------------------------------|--------------------|----------|
| JWT подписан **MD5** (нужен HS256) | ✅ Уже **HMAC-HS256** через `jsonwebtoken` | `ipfrs/src/auth.rs:461`, `ipfrs-interface/src/auth.rs:278` |
| Backpressure-семафор не освобождает permits | ✅ Корректен (eager + deferred forget, покрыт тестами) | `ipfrs-interface/src/backpressure.rs:185-266` |
| Баг FedAvg-таймаута в `tensorlogic_ops.rs:1131` | Файла/строки **нет**; FL живёт в `gradient/federated.rs` | `gradient/federated.rs:1014` |

**Реальный security-долг** (а не мнимый):
- ⚠️ **TLS-генератор в node-крейте — заглушка**: `SelfSignedCertGenerator::generate()`
  пишет фейковый PEM, rcgen только в комментарии (`ipfrs/src/tls.rs:314`). Gateway-TLS
  через rustls — реальный (`ipfrs-interface/src/tls.rs:49`).
- ⚠️ Дефолтный секрет `"default_secret_change_in_production"` (`auth.rs:574`).
- ⚠️ **Дублированная Auth-модель**: два разных enum `Role`/`Permission` в `ipfrs` и
  `ipfrs-interface` — риск расхождения авторизации.
- ⚠️ In-memory `UserStore`/`ApiKeyStore` (не персистятся).

---

## 2. Реестр заглушек по контекстам

### Network
- Выкачка блоков по swarm — `fetch_block_from_peer` → `NotFound` (`node.rs:1311`).
- Gossipsub только in-process (нет libp2p-gossipsub behaviour; `validate_message` всегда true).
- `KademliaDhtProvider` — все 12 методов заглушены (`dht_provider.rs:388`).
- Tier B (~180 модулей репутации/банов) не подключён к событиям swarm.

### Semantic
- `VectorIndex::rebuild` молча опустошает граф (`hnsw.rs:586`). **Реальный баг.**
- Снапшот HNSW не сохраняет adjacency (`hnsw.rs:791`).
- `DiskANNIndex::compact` — no-op (`diskann.rs:947`).
- DHT-транспорт ничего не передаёт (`dht_node.rs:409`).
- ✅ Зато логико-семантический fallback **реализован** (вики ранее: «будущее») — `solver.rs:340`.

### TensorLogic
- Распределённое исполнение графа — заглушка (`computation_graph.rs:1667`).
- Тензорный backward отсутствует (только forward; градиенты рукописные per-layer).
- FedAvg при таймауте молча усредняет по меньшему кругу (`federated.rs:1014`).

### Transport
- GraphSync `extract_links` → пустой вектор; обходит только корень (`graphsync.rs:377`).
- Erasure — XOR-заглушка, не Reed-Solomon; decode не восстанавливает (`erasure.rs:299`).
- NAT/STUN/TURN — симуляция (`nat_traversal.rs:367`).
- `MultiTransportManager::find_transport` — fallback-баг (`multi_transport.rs:215`).
- ⚠️ **Два разных Bitswap** (transport + network), несовместимые типы PeerId/сообщений.

### Storage
- Целостность при чтении **не проверяется** (`get` → `from_parts` без `verify`, `blockstore.rs:350`).
- `min_age` GC enforced только в одной из трёх реализаций GC.
- Пины не транзакционны с данными; indirect-пины не персистятся между рестартами.

---

## 3. Технический долг (приоритеты)

1. Завершить Network→Transport выкачку блоков по swarm (`node.rs:1311`) — **блокер P2P GET**.
2. Включить verify целостности при чтении (Storage).
3. Починить `VectorIndex::rebuild`.
4. Консолидировать дубли: 2 Bitswap, 3 GC, 4 тиринга, 4 репутации, 2 RL/fuzzy.
5. Унифицировать Auth-модель между крейтами.
6. Реализовать GraphSync, реальный Reed-Solomon, реальный STUN/TURN.

---

## Что дальше?

- **Каталог движков вывода (20+)** → [[07-LogicDomain]]
- **Границы контекстов** → [[03-BoundedContexts]]
- **Сквозная DDD-вики** → [[../Wiki/00-INDEX]]

**Связанные**: [[03-BoundedContexts]] | [[07-LogicDomain]] | [[11-ErrorHandling]] | [[../Wiki/11-RealityCheck]]
**Источник кода**: весь `crates/` (анализ 7 агентов, 2026-06-19)
