---
title: 14-HLD
type: architecture
summary: Helicopter View — высокоуровневый дизайн IPFRS (HLD): зачем, что, как взаимодействует, ключевые решения
tags: [ipfrs, hld, architecture, overview, helicopter-view]
source: cool-japan/Vendor/ipfrs/
related: ["[[01-Overview]]", "[[02-ArchitectureStack]]", "[[03-BoundedContexts]]", "[[12-MasterArchitecture]]"]
read_time: 15 мин
updated: 2026-06-18
---

# IPFRS: Helicopter View (HLD)

> **Формат**: High-Level Design — «что и зачем», не «как»  
> **Аудитория**: новый инженер, PM, архитектор другой системы  
> **Цель**: понять всю систему за 15 минут

---

## 1. Проблема одним предложением

> Традиционные файловые системы хранят **байты**. IPFRS хранит **смысл** — данные становятся самоописывающими, связанными и способными рассуждать о себе.

---

## 2. Что такое IPFRS — системные границы

```
╔══════════════════════════════════════════════════════════════╗
║                       IPFRS                                  ║
║                                                              ║
║   ┌──────────┐   ┌──────────┐   ┌──────────┐                 ║
║   │  Хранит  │   │  Ищет    │   │ Рассуждает│                ║
║   │  блоки   │   │  смысл   │   │ о данных  │                ║
║   │  по CID  │   │ (векторы)│   │  (логика) │                ║
║   └──────────┘   └──────────┘   └──────────┘                 ║
║                                                              ║
║   ┌────────────────────────────────────────┐                 ║
║   │     Распределяет всё это по сети       │                 ║
║   │        peer-to-peer, без центра        │                 ║
║   └────────────────────────────────────────┘                 ║
╚══════════════════════════════════════════════════════════════╝

Снаружи:
  Пользователь / CLI / gRPC / GraphQL / Python / WASM / Node.js
```

**В системе**: хранение, семантика, логика, сеть, транспорт  
**Вне системы**: ML-модели для embeddings, бизнес-логика приложений

---

## 3. Архитектура одним взглядом

```
┌─────────────────────────────────────────────────────────────────┐
│                    КТО ГОВОРИТ С IPFRS                          │
│                                                                 │
│   CLI        gRPC       GraphQL    REST/WS    Python  WASM      │
│    │           │           │          │          │       │      │
└────┼───────────┼───────────┼──────────┼──────────┼───────┼──────┘
     └───────────┴───────────┴──────────┴──────────┴───────┘
                                  │
                    ┌─────────────▼──────────────┐
                    │      ipfrs-interface       │
                    │   (Gateway — Уровень 0)    │
                    │  auth · backpressure · TLS │
                    └─────────────┬──────────────┘
                                  │
                    ┌─────────────▼───────────────┐
                    │         ipfrs (Node)        │
                    │    Оркестратор — Уровень 1  │
                    │  add · get · search · query │
                    └──┬──────┬───────┬──────┬────┘
                       │      │       │      │
          ┌────────────┘      │       │      └────────────┐
          │                   │       │                   │
    ┌─────▼──────┐    ┌───────▼───┐  ┌▼──────────┐  ┌─────▼──────┐
    │  STORAGE   │    │ NETWORK   │  │ SEMANTIC  │  │  LOGIC /   │
    │            │    │           │  │           │  │ TENSORLOGIC│
    │ Sled  +    │    │ libp2p    │  │ HNSW  +   │  │            │
    │ decorators │    │ Kademlia  │  │ DiskANN   │  │ Backward   │
    │ WAL  GC    │    │ Gossip    │  │ Embeddings│  │ chaining   │
    │ Tiering    │    │ Reputation│  │ Re-ranking│  │ Neural-sym │
    └─────┬──────┘    └───────────┘  └───────────┘  └────────────┘
          │                │
          └────────────────┘
                    │
          ┌─────────▼──────────┐
          │    TRANSPORT       │
          │                    │
          │ Bitswap · Session  │
          │ WantList · Ledger  │
          │ TensorSwap  QUIC   │
          └────────────────────┘

          Всё зависит от ──▶  ipfrs-core  (CID, Block, Ipld)
```

---

## 4. Шесть слоёв за 30 секунд

```
  СЛОЙ       ЧТО ДЕЛАЕТ                          КРЕЙТ
  ───────────────────────────────────────────────────────
  L0  UI     HTTP, CLI, WASM, FFI                interface, cli, wasm, python, nodejs
  L1  App    Оркестрирует use-cases              ipfrs (Node)
  L2  Domain 5 BC: хранение/сеть/смысл/логика/   storage, network, semantic,
             транспорт                            tensorlogic, transport
  L3  Port   Трейты-интерфейсы                   BlockStore, NetworkNode, ...
  L4  Impl   Движки: Sled, libp2p, HNSW, Tokio   (внутри каждого BC-крейта)
  L5  HW     NVMe, Ethernet, CPU                 ОС / железо
```

---

## 5. Ключевой токен: CID

```
        ┌──────────┐
        │  Bytes   │
        └────┬─────┘
             │ hash()
             ▼
        ┌──────────┐
        │   CID    │◄─── Детерминирован. Глобально уникален.
        └──┬──┬──┬─┘     Проверяется на каждом чтении.
           │  │  │
     ┌─────┘  │  └──────┐
     ▼        ▼         ▼
  Storage  Network   Semantic
  (ключ)   (DHT)    (связь с
                    вектором)
```

> **Правило**: всё межконтекстное взаимодействие — это «передай CID».  
> Если два контекста должны общаться, они делают это через CID — никаких прямых объектных ссылок.

---

## 6. Пять доменов — одна строка каждый

| Домен | Вопрос | Ядро |
|-------|--------|------|
| **Storage** | «Что хранится и где?» | Sled + стек декораторов (кэш → тиеринг → WAL) |
| **Network** | «Кто есть в сети и у кого что?» | Kademlia DHT + репутация пиров |
| **Semantic** | «Что это означает?» | HNSW-индекс + embedding pipeline |
| **TensorLogic** | «Что можно вывести?» | 8 движков вывода + нейро-символьное слияние |
| **Transport** | «Как надёжно перенести блок?» | Bitswap + приоритетная WantList |

---

## 7. Три главных потока данных

### 7.1 ADD — положить в систему

```
Пользователь                                              Сеть
    │                                                       │
    │ add(file)                                             │
    ▼                                                       │
  Node ──► chunk ──► Storage.put(block)×N ──► Network.announce(CID)×N
                          │                         │
                          ▼                         ▼
                    Sled (диск)              DHT (20 пиров
                    LRU (память)             знают, что у нас есть)
                          │
                          ▼
                    Semantic.index(CID, embedding)  ← опционально
                          │
                          ▼
                    HNSW (поиск по смыслу доступен)

  Время: 300 мс (без семантики) / 900 мс (с семантикой)
```

### 7.2 GET — получить из системы

```
Пользователь
    │ get(CID)
    ▼
  Node ──► Storage.get(CID)
               │
        ┌──────┴──────┐
       HIT            MISS
        │              │
    30 мкс         Network.find_providers(CID)  ← DHT 150-300 мс
   (кэш)               │
                    [peer1, peer2, ...]
                        │
                   Transport.session(CID)  ← Bitswap
                        │
                   Block(CID, data) ← пир отдаёт
                        │
                   hash(data)==CID? ✓  ← верификация
                        │
                   Storage.put(block)  ← кэшируем
                        │
                   вернуть байты

  Время: 30 мкс (локально) / 250–1000 мс (сеть)
```

### 7.3 SEARCH — найти по смыслу

```
Пользователь
    │ search("глубокое обучение", k=10)
    ▼
  Node ──► embed(query) → вектор [0.14, -0.09, ...]
               │
          Semantic.search(вектор, k=10)
               │
        ┌──────┴──────┐
       HIT             MISS
        │               │
    кэш (85%)        HNSW k-NN ← 1–10 мс
        │               │
        └───────┬────────┘
                │
           [(CID1, 0.92), (CID2, 0.88), ...]
                │
           Storage.get(CID) × k  ← метаданные
                │
           вернуть [{cid, score, title}, ...]
```

---

## 8. Топология сети

```
          ┌─────────────┐
          │  Bootstrap  │  ← известные точки входа
          │    Peers    │
          └──────┬──────┘
                 │
    ┌────────────┼────────────┐
    │            │            │
┌───▼──┐    ┌───▼──┐    ┌───▼──┐
│Node A│────│Node B│────│Node C│   Peer-to-peer mesh
└───┬──┘    └───┬──┘    └───┬──┘   без центрального сервера
    │            │            │
    └────────────┼────────────┘
                 │
         ┌───────▼──────┐
         │   Kademlia   │  DHT: O(log N) хопов
         │     DHT      │  Каждый знает k=20 ближайших
         └──────────────┘

Обнаружение пиров:
  1. mDNS     → локальная сеть (LAN), мгновенно
  2. Bootstrap → первые известные узлы (<1 сек)
  3. DHT walk  → случайные запросы, ~5 мин на полный обход
```

---

## 9. Стек технологий

```
┌─────────────────────────────────────────────────────┐
│  Язык:      Rust 2021 (edition), версия 1.90+       │
│  Async:     Tokio (multi-thread runtime)            │
│  Сеть:      libp2p (QUIC → TCP → WebSocket)        │
│  DHT:       Kademlia (k=20, α=3)                   │
│  Storage:   Sled (embedded B+ tree, ACID)           │
│  Vectors:   hnsw_rs + DiskANN (mmap)               │
│  Logic IR:  Custom Datalog-like (Horn clauses)      │
│  API:       gRPC (tonic) + GraphQL (async-graphql)  │
│  FFI:       PyO3 (Python), napi-rs (Node.js)        │
│  WASM:      wasm-bindgen                            │
│  Auth:      JWT + OAuth2 + TLS (rustls)             │
│  Formats:   IPLD/DAG-CBOR, Arrow, SafeTensors       │
└─────────────────────────────────────────────────────┘
```

---

## 10. Ключевые архитектурные решения

### 10.1 CID как граничный токен

```
❌ Плохо:  Storage передаёт Block-объект в Transport
✅ Хорошо: Storage → CID → Transport запрашивает Block по CID

Почему: контексты остаются независимыми. Замена Storage не ломает Transport.
```

### 10.2 Намеренное дублирование репутации

```
Network  ──► долгосрочная репутация (EWMA, граф доверия)
Transport ──► session-local скор (задержка, доступность)

Почему: автономность > DRY.
  Если Network упал — Transport продолжает работать.
  Если Transport использует другую метрику — не надо менять Network.
```

### 10.3 Декораторы вместо монолита Storage

```
Request ──► TtlBlockStore
              └──► QuotaBlockStore
                     └──► CachedBlockStore
                            └──► DedupBlockStore
                                   └──► CompressionBlockStore
                                          └──► EncryptedBlockStore
                                                 └──► SledBlockStore

Почему: каждая задача (кэш, квота, дедупликация) тестируется изолированно.
  Новый движок = только реализовать трейт BlockStore.
```

### 10.4 Мутация состояния, не Event Sourcing

```
Блок после записи никогда не меняется → CID = hash(data) = иммутабелен.
Event sourcing был бы избыточен: нечего «воспроизводить».
Журналы аудита существуют только для метрик и наблюдаемости.
```

### 10.5 Нейро-символьное слияние в TensorLogic

```
Вопрос: «Кто является другом Алисы?»

Чистая логика:    friend(alice, X) :- known_facts only
                  → быстро, но знает только то, что явно записано

Гибридный режим:  сначала логика → если нет ответов
                  → embed("friend of alice") → HNSW → семантически похожие факты
                  → объединить результаты

Почему: данные реального мира неполны. Нейро-символьный fallback
  позволяет выводить знания, которых нет явно.
```

---

## 11. Нефункциональные требования

| Свойство | Значение | Как обеспечивается |
|---------|---------|-------------------|
| **Масштабируемость** | Storage → PB; Semantic → 1M+ векторов; Network → 1K+ пиров | Горизонтальное масштабирование per domain |
| **Надёжность** | Блоки верифицируются при каждом чтении | `hash(data)==CID` invariant |
| **Отказоустойчивость** | Данные на k=20 DHT-пирах | Kademlia replication factor |
| **Производительность** | GET из кэша < 30 мкс; HNSW search < 10 мс | LRU + DashMap + SIMD |
| **Безопасность** | TLS, JWT, OAuth2, capability model | rustls + auth middleware |
| **Расширяемость** | Новый бэкенд = реализовать трейт | BlockStore, NetworkNode traits |
| **Наблюдаемость** | Metrics, tracing, profiling | Arc<IpfrsMetrics>, opentelemetry |

---

## 12. Слабые места системы (честно)

```
⚠️  JWT подписывается MD5 вместо HS256          → auth.rs:449
⚠️  TLS-генератор возвращает заглушку           → tls.rs:314
⚠️  Backpressure не отзывает permits            → backpressure.rs:182
⚠️  GC параметр min_age принимается, игнорируется → gc.rs
⚠️  Federated Learning: timeout при min_peers>0 → tensorlogic_ops.rs:1131
⚠️  "Zero-copy" Arrow = на самом деле 3 копии   → arrow.rs

Производительные ловушки:
🐢  gc_stats() = O(V+E) BFS при каждом вызове
🐢  DiskANN sort в O(n² log n) при больших графах
🐢  WebSocket 10мс polling × 10K соединений = 10K спящих задач
```

> Подробнее: [[IPFRS_ARCHITECTURE_SONNET.md]] → Приложение A

---

## 13. Где что живёт

```
cool-japan/
├── Vendor/ipfrs/            ← Исходный код (источник истины)
│   └── crates/
│       ├── ipfrs-core/      ← Shared Kernel: CID, Block, Ipld
│       ├── ipfrs-storage/   ← Storage BC: 162 файла
│       ├── ipfrs-network/   ← Network BC: 187 файлов
│       ├── ipfrs-semantic/  ← Semantic BC: 159 файлов
│       ├── ipfrs-tensorlogic/ ← Logic BC: 194 файла
│       ├── ipfrs-transport/ ← Transport BC: 46 файлов
│       ├── ipfrs-interface/ ← Gateway: 24 файла
│       ├── ipfrs/           ← Node (App Layer): 24 файла
│       ├── ipfrs-cli/       ← CLI: 34 файла
│       └── ipfrs-{wasm,nodejs,python}/  ← Биндинги
│
├── Wiki/                    ← Документация (этот файл)
│   ├── 01 – 13 .md          ← 13 статей по системе
│   ├── WIKI_SCHEMA.md       ← Правила поддержки Wiki
│   └── log.md               ← Хроника изменений
│
├── IPFRS_ARCHITECTURE_SONNET.md  ← Полный DDD (Sonnet 4.6, 1722 стр.)
├── IPFRS_ARCHITECTURE_MASTER.md  ← Мастер-документ (Opus 4.8, 872 стр.)
└── IPFRS_DEEP_ARCHITECTURE.md    ← Глубокое погружение (1910 стр.)
```

---

## 14. С чего начать чтение

```
Цель                         Читай
─────────────────────────────────────────────────────────
Новый инженер (первый час)   Этот файл → [[01-Overview]] → [[03-BoundedContexts]]
Понять Storage               [[04-StorageDomain]]
Понять поиск                 [[06-SemanticDomain]]
Понять логику/AI             [[07-LogicDomain]]
Полный DDD-анализ            [[12-MasterArchitecture]]
Полная глубокая архитектура  [[13-DeepArchitecture]]
Производительность           [[10-Performance]]
Ошибки и recovery            [[11-ErrorHandling]]
Конкретный поток данных      [[09-DataFlows]]
```

---

**Связанные**: [[01-Overview]] | [[02-ArchitectureStack]] | [[03-BoundedContexts]] | [[12-MasterArchitecture]] | [[13-DeepArchitecture]]
