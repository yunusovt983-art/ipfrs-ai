---
title: 06-GeoInference
type: design
summary: Пошаговый план реализации геораспределённого инференса на базе IPFRS — фазы, привязка к коду (file:line), диаграммы, критерии готовности
tags: [ipfrs, roadmap, geo-inference, distributed, design]
updated: 2026-06-19
---

# RoadMap 06 — Геораспределённый инференс на IPFRS

> **Цель.** Превратить IPFRS в платформу **геораспределённого инференса**: запрос
> приходит в ближайший регион → маршрутизируется к пиру, у которого есть нужная модель
> (адресуемая по CID) → недостающие веса/активации дотягиваются у ближайшей реплики →
> результат возвращается с проверкой целостности и происхождением (provenance).
>
> Документ — **пошаговый**. Каждая фаза: что делаем, где в коде, критерий готовности (DoD).
> Все `file:line` выверены по `ipfrs_source/crates/**` на 2026-06-19.

---

## ✅ Статус реализации (2026-06-19)

Первые срезы уже в коде, компилируются и покрыты тестами:

| Что | Где | Коммит |
|-----|-----|--------|
| **Фаза 1.1** — block-fetch по swarm (`/ipfrs/blockfetch/1.0.0`) + inbound-обслуживание из стора | `ipfrs-network/src/blockfetch.rs`, `node.rs`, `ipfrs/src/node/core.rs` | `da50774`, `26ffbe5` |
| **Фаза 2** — `ModelManifest` (DAG-CBOR, CID слоёв) | `ipfrs-tensorlogic/src/model_manifest.rs` | `1d158ca` |
| **Фаза 4.2** — планировщик маршрутизации/хеджирования + `Node::geo_fetch_block` | `ipfrs-network/src/geo.rs`, `ipfrs/src/node/geo_ops.rs` | `51471a1`, `26ffbe5` |
| **Проброс в GraphQL** — `geo_fetch(cid, hedge_k)` + `NetworkNode::geo_fetch_block` | `ipfrs-interface/src/graphql.rs`, `ipfrs-network/src/node.rs` | `7bbcae8` |
| **Фаза 3 (RTT)** — реальный per-peer RTT из ping → ранжирование кандидатов | `ipfrs-network/src/node.rs` (`peer_rtt`) | `9ec56b4` |
| **Фаза 3 (регион)** — coarse-регион из Multiaddr на connect → region-affinity | `ipfrs-network/src/node.rs` (`peer_region`, `region_from_multiaddr`) | `33fbc68` |
| **Фаза 1.2** — реальный libp2p gossipsub по проводу (subscribe/publish/event) | `ipfrs-network/src/node.rs` (`gossipsub` behaviour) | `13c278a` |
| **Анонс моделей** — `announce_model` (provide + gossip `/ipfrs/models`) / `subscribe_models` | `ipfrs-network/src/models.rs`, `ipfrs/src/node/models_ops.rs` | `dd58458` |
| **Фоновый consumer** — `start_model_consumer` → реестр `known_models` из gossip | `ipfrs/src/node/models_ops.rs` | `24d0d0a` |

Тесты: geo 6/6, blockfetch 3/3, model_manifest 4/4, region 4/4, models 3/3; `cargo check --workspace` зелёный.
Полный цикл замкнут: `announce_model` → gossip `/ipfrs/models` → `start_model_consumer` →
`known_models` → `geo_fetch_block(cid)`.
Осталось по фазам: 1.3 semantic-DHT transport, 3 (нагрузка/load), 5 (исполнение),
6 (proof/FedAvg/residency); проброс в **gRPC** (GraphQL — готово); переключить
`distributed_infer` со in-process на wire-gossipsub.

---

## 0. Почему IPFRS уже почти готов к этому

| Готовый кирпич | Где | Зачем для гео-инференса |
|----------------|-----|--------------------------|
| Контент-адресация (CID) | `ipfrs-core` | Модели/веса/правила/активации — по CID: дедуп, версии, проверяемость |
| `DistributedBackwardChainer` | `distributed_backward_chainer.rs:66` | Делегирование под-целей пирам по CID (символический инференс) |
| `InferenceWaiters` + gossip | `node.rs:32, 1529` | Каркас распределённого вывода поверх pub/sub |
| Семантический DHT | `dht.rs:148` (`find_nearest_peers`) | Маршрутизация в пространстве эмбеддингов (MoE/эксперты) |
| TensorSwap | `tensorswap/core.rs:57` | Потоковая передача тензоров с дедлайнами + einsum-граф зависимостей |
| Федеративка | `gradient/federated.rs:1054` | FedAvg, secure aggregation, DP, `GradientDelta` |
| **Гео-модули (объявлены!)** | `facade.rs:100-103` | `GeoRouter`, `QualityPredictor`, `PeerSelector`, `MultipathQuicManager` — **написаны, но не подключены** |
| Provenance | `provenance.rs:239` | `TrainingProvenance{model_cid, datasets, hyperparams}` — воспроизводимость |
| Квантизация | `tensor_quantizer.rs:349` | INT8/INT4/FP16/BF16 — адаптация к полосе канала |

**Вывод:** это не «строить inference-сервер с нуля», а **подключить и достроить** уже
существующее. Главное препятствие — несколько ключевых заглушек (Фаза 1).

---

## Дорожная карта (обзор фаз)

```
Фаза 1  Закрыть блокеры        ── block-fetch · gossipsub · semantic-DHT transport
   │
Фаза 2  Модели как контент     ── CID-registry слоёв · lazy fetch · дедуп версий
   │
Фаза 3  Гео-маршрутизация      ── оживить GeoRouter/PeerSelector/QualityPredictor
   │
Фаза 4  MVP гео-инференс       ── routing + model-by-CID + hedged requests
   │
Фаза 5  Параллелизм по регионам── pipeline/tensor split · distributed graph exec
   │
Фаза 6  Доверие и приватность  ── proof-carrying results · FedAvg по регионам · data residency
```

---

## Фаза 1 — Закрыть блокеры (фундамент «по проводу»)

Без этого распределённый инференс не поедет. Это пересечение с [[01-Critical-Bugs]].

### Шаг 1.1 — Выкачка блоков по swarm ⚠️ БЛОКЕР №1
- **Сейчас:** `fetch_block_from_peer` → `Error::NotFound("...pending Task E")`
  (`ipfrs-network/src/node.rs:1315`).
- **Сделать:** реальный Bitswap-обмен по libp2p. Варианты: (а) добавить `bitswap` как
  `NetworkBehaviour` в `IpfrsBehaviour` (`node.rs:288`); (б) request-response протокол
  `/ipfrs/bitswap/1.0.0`. Связать с уже готовым `ipfrs-transport::BitswapExchange`
  (`bitswap.rs:75`).
- **DoD:** узел A забирает блок по CID у узла B через swarm; `Block::verify()` проходит.

### Шаг 1.2 — Реальный gossipsub по проводу
- **Сейчас:** `GossipSubManager` — только in-process (`gossipsub.rs:280`),
  `validate_message` всегда `true` (`gossipsub.rs:468`); в `IpfrsBehaviour` gossipsub нет.
- **Сделать:** добавить `libp2p::gossipsub::Behaviour` в swarm; топики
  `/ipfrs/models`, `/ipfrs/inference`. Реализовать `validate_message` (подпись + CID).
- **DoD:** публикация `model_cid` в одном регионе доходит до подписчиков в другом.

### Шаг 1.3 — Транспорт семантического DHT
- **Сейчас:** `replicate_to_peer` → no-op (`dht_node.rs:409`), `query_peer` → всегда `None`
  (`dht_node.rs:430`) → `search_distributed` вырождается в локальный.
- **Сделать:** подключить запрос к пиру через request-response (Шаг 1.1) — отдать соседу
  эмбеддинг, получить top-k.
- **DoD:** `search_distributed` возвращает результаты от удалённых пиров.

---

## Фаза 2 — Модели как контент (CID-addressed model registry)

### Шаг 2.1 — Публикация модели послойно
- Разбить модель на блоки: по слою/шарду → `TensorBlock` → `Block` (CID).
  Использовать `safetensors_support.rs` + `gradient/arrow_ipc.rs` для сериализации.
- Манифест модели = IPLD-узел `{ layers: [cid...], arch, dtype, version }` → один корневой CID
  (по аналогии с `KnowledgeBaseIpld`, `ipld_codec.rs:88`).
- **DoD:** `model_cid` → можно перечислить и достать любой слой по CID.

### Шаг 2.2 — Ленивая дозагрузка весов
- Нода тянет **только нужные слои** у ближайшей реплики через TensorSwap
  (`tensorswap/core.rs:98`, `want_tensor` уже умеет приоритеты + зависимости).
- Дедуп между версиями модели через `GradientDelta` (`gradient_sparsify.rs:267`) —
  передавать только дельту весов.
- **DoD:** загрузка v2 модели поверх v1 тянет ≈ только изменённые слои.

### Шаг 2.3 — Анонс провайдеров модели в DHT
- `provide(model_cid)` + `provide(layer_cid)` (`node.rs:1198`) → кто где хранит.
- **DoD:** `find_providers(model_cid)` возвращает реальных держателей.

---

## Фаза 3 — Гео-маршрутизация (оживить Tier B)

Модули **уже написаны** в `ipfrs-network`, но не подключены к живому пути (Tier B).

### Шаг 3.1 — Подключить `GeoRouter` + `PeerSelector` + `QualityPredictor`
- **Сейчас:** объявлены как поля `NetworkFacade` (`facade.rs:100-103`), но события swarm в
  них не поступают.
- **Сделать:** кормить их `NetworkEvent` (`node.rs:448`): `PeerConnected`/RTT из ping,
  регион из identify/Multiaddr; `QualityPredictor` оценивает латентность/полосу пира;
  `PeerSelector` выбирает пир по {RTT, регион, загрузка, есть ли `model_cid`}.
- **DoD:** `select_peer(model_cid, region)` возвращает ближайшего здорового держателя.

### Шаг 3.2 — `MultipathQuicManager` для устойчивости
- Подключить мультипуть QUIC (`facade.rs:103`) — несколько путей до удалённого региона.
- **DoD:** потеря одного пути не рвёт инференс-сессию.

---

## Фаза 4 — MVP: гео-инференс на одном узле-исполнителе

Минимальный сквозной сценарий (собирает Фазы 1–3).

```
Запрос(EU) → Node.geo_infer(model_cid, input)
  1. embedding(input)                         semantic
  2. peer = GeoRouter.select(model_cid, EU)   network (Фаза 3)
  3. если peer == self → локально; иначе:
     hedged: отправить N ближайшим репликам    transport (want-list multi-peer)
  4. peer тянет недостающие слои по CID         TensorSwap (Фаза 2)
  5. исполнить → результат + verify(result_cid)
  6. первый валидный ответ, остальные cancel
```

### Шаг 4.1 — API `geo_infer`
- Новый метод `Node::geo_infer(model_cid, input, policy)` в op-модуле (рядом с
  `tensorlogic_ops.rs`). Публиковать через gRPC/GraphQL (`grpc.rs`, `graphql.rs`).
### Шаг 4.2 — Хеджированные запросы
- Слать k ближайшим репликам, брать первый CID-проверенный ответ, остальные `cancel`
  (want-list уже поддерживает несколько пиров, `bitswap.rs:184`).
- **DoD MVP:** запрос из региона EU обслуживается ближайшим пиром с моделью; p99 латентность
  ниже, чем single-region; ответ проверен по CID.

---

## Фаза 5 — Параллелизм инференса между регионами

### Шаг 5.1 — Распределённое исполнение графа
- **Сейчас:** `execute_distributed` → `Err("requires ipfrs-network integration")`
  (`computation_graph.rs:1668`). Партиционирование (`partition_graph`) уже работает.
- **Сделать:** исполнять стадии графа там, где лежат веса; стримить активации между
  регионами через TensorSwap (einsum-граф зависимостей, `tensorswap/einsum.rs`).
- **DoD:** граф из 2+ стадий исполняется на 2 географически разнесённых нодах.

### Шаг 5.2 — Latency-aware размещение слоёв
- Партиционирование с учётом RTT (из `QualityPredictor`) — минимизировать кросс-региональные
  хопы (наибольший трафик активаций — на быстрых линках).

### Шаг 5.3 — Mixture-of-Experts через семантический DHT
- Эксперт = пир; гейтинг = `find_nearest_peers(embedding)` (`dht.rs:148`).
- **DoD:** токен/запрос маршрутизируется к эксперту, ближайшему в эмбеддинг-пространстве.

---

## Фаза 6 — Доверие, приватность, комплаенс

### Шаг 6.1 — Proof-carrying inference
- К ответу прикладывать `ProofTree` (`proof_tree.rs:199`) + `model_cid` + `input_cid` +
  `TrainingProvenance` (`provenance.rs:239`) → воспроизводимо и аудируемо.
- Опц.: избыточное исполнение на N пирах + голосование (BFT-lite доверие к чужому инференсу).

### Шаг 6.2 — Федеративное дообучение по регионам
- Локальный fine-tune → делиться только дельтами (`GradientDelta`) + secure aggregation
  (`federated.rs:328`) + DP (`federated.rs:97`). Данные не покидают регион.

### Шаг 6.3 — Data residency
- Пиннинг model/rule CID к региональным нодам (`ipfrs-storage/datacenter.rs`); ограничивать,
  где исполняется инференс и куда течёт ввод (GDPR-like).

### Шаг 6.4 — Адаптивное квантование под канал
- INT8/INT4 по медленным гео-линкам, FP16 по LAN; точность выбирает `QualityPredictor`
  (`TensorQuantizer`, `tensor_quantizer.rs:377`).

---

## Карта «фича → код»

| Фаза | Главный файл(ы) | Что меняем |
|------|-----------------|-----------|
| 1.1 | `ipfrs-network/src/node.rs:1298`, `bitswap.rs` | реальный Bitswap по swarm |
| 1.2 | `ipfrs-network/src/gossipsub.rs:280,468`, `node.rs:288` | libp2p gossipsub + validate |
| 1.3 | `ipfrs-semantic/src/dht_node.rs:409,430` | транспорт семантического DHT |
| 2 | `tensorlogic/safetensors_support.rs`, `tensorswap/core.rs:98`, `gradient_sparsify.rs:267` | model-by-CID + lazy fetch + delta |
| 3 | `ipfrs-network/src/facade.rs:100-103`, `node.rs:448` | оживить GeoRouter/PeerSelector/QualityPredictor |
| 4 | `ipfrs/src/node/*_ops.rs`, `grpc.rs`, `graphql.rs` | API `geo_infer` + hedged |
| 5 | `tensorlogic/computation_graph.rs:1668`, `tensorswap/einsum.rs` | распределённое исполнение графа |
| 6 | `proof_tree.rs`, `provenance.rs`, `gradient/federated.rs`, `storage/datacenter.rs` | доверие + приватность |

---

## Рекомендация по старту

**MVP = Фаза 1 (шаги 1.1–1.3) → Фаза 2 → Фаза 3 → Фаза 4.**
Это даёт работающий гео-инференс «на одном узле-исполнителе» и опирается на сильные стороны
IPFRS (контент-адресация + семантическая маршрутизация), а не строит сервис с нуля.
Фазы 5–6 — дифференциаторы (параллелизм между регионами, верифицируемость, приватность).

> ⚠️ Заглушки, перечисленные здесь, — те же, что в `[[../Wiki/11-RealityCheck]]`. Закрытие
> блокеров Фазы 1 полезно само по себе (чинит P2P GET), независимо от гео-инференса.

---

**Связанные**: [[01-Critical-Bugs]] | [[04-Features]] | [[../Wiki/10-DataFlows]] | [[../Wiki/11-RealityCheck]]
**Источник кода**: `ipfrs_source/crates/` (выверено 2026-06-19)
