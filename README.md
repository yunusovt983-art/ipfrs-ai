<p align="center">
  <img src="assets/ipfrs-banner.jpg" alt="IPFRS — Planetary-Scale Distributed Brain for AI" width="100%">
</p>

# IPFRS — Inter-Planetary File Rust System

> Распределённая контент-адресуемая файловая система, объединяющая хранение с ML-интеллектом.  
> Файлы идентифицируются по хешу содержимого (CID). Каждый блок — сам себе адрес.

[![License: AGPL v3](https://img.shields.io/badge/License-AGPL%20v3-blue.svg)](LICENSE)

> **Примечание о происхождении.** IPFRS — форк [cool-japan/ipfrs](https://github.com/cool-japan/ipfrs),
> исходно под лицензией Apache-2.0. Объединённая работа в этом репозитории лицензируется
> под AGPL-3.0; атрибуция и условия Apache-2.0 для исходных частей сохраняются
> (см. заголовки файлов в `ipfrs_source/`). AGPL-3.0 требует раскрытия исходного кода
> при предоставлении доступа к ПО по сети.

---

## Архитектура — вид с высоты

```mermaid
graph TD
    User(["👤 Пользователь"])
    CLI["💻 CLI\nipfrs-cli"]
    PY["🐍 Python"]
    JS["📦 Node.js"]
    WB["🌐 WASM"]
    GW["🔌 gRPC / GraphQL\n/ REST / WebSocket"]

    User --> CLI
    User --> GW
    User --> PY
    User --> JS
    User --> WB

    subgraph IPFRS ["Система IPFRS"]
        direction TB

        IF["🚪 ipfrs-interface  ·  Шлюз\n─────────────────────────────\nauth · TLS · backpressure\ngRPC · GraphQL · WS · FFI"]

        NODE["🧠 ipfrs  ·  Узел / Оркестратор\n─────────────────────────────\nadd · get · search · query · pin · dag"]

        subgraph DOMAINS ["5 Bounded Contexts"]
            direction LR
            ST["💾 Storage\nSled + WAL\nGC · tiers\ndecorators"]
            NW["🌐 Network\nlibp2p · DHT\nGossip · NAT\nreputation"]
            SM["🔍 Semantic\nHNSW · DiskANN\nembeddings\nre-ranking"]
            LG["🤖 TensorLogic\n20+ движков вывода\nautograd · RL\nneuro-symbolic"]
            TR["📡 Transport\nBitswap · Session\nWantList · QUIC\nTensorSwap"]
        end

        CORE["⚙️ ipfrs-core  ·  Shared Kernel\nCid · Block · Ipld · TensorBlock · IpfrsError"]

        IF --> NODE
        NODE --> ST
        NODE --> NW
        NODE --> SM
        NODE --> LG
        NODE --> TR
        ST & NW & SM & LG & TR --> CORE
    end

    CLI --> IF
    GW  --> IF
    PY  --> IF
    JS  --> IF
    WB  --> IF
```

---

## CID — универсальный токен границы

```mermaid
flowchart LR
    B["📄 Байты"] --> H{"hash()"} --> CID["🔑 CID\nидентификатор содержимого"]

    CID -->|"ключ хранения"| ST["💾 Storage\nSled B+ tree"]
    CID -->|"запись в DHT"| NW["🌐 Network\nCID → пиры"]
    CID -->|"ссылка вектора"| SM["🔍 Semantic\nузел HNSW"]
    CID -->|"содержимое правила"| LG["🤖 Logic\nIPLD-блок"]
    CID -->|"запрос пиру"| TR["📡 Transport\nWant(CID)"]
```

> Любое межконтекстное взаимодействие сводится к «передай CID».

---

## Потоки данных

### ADD — сохранение файла

```mermaid
sequenceDiagram
    actor User as Пользователь
    participant Node
    participant Storage
    participant Network
    participant Semantic

    User->>Node: add(file, 100 MB)
    Node->>Node: chunk → 391 блок (по 256 KB)
    loop для каждого блока
        Node->>Storage: put(block) → CID
        Node-)Network: announce(CID)
        Node->>Semantic: index(CID, embed)
    end
    Node-->>User: root_CID
    Note over User,Semantic: ~300 мс (без семантики) · ~900 мс (с семантикой)
```

### GET — получение файла

```mermaid
sequenceDiagram
    actor User as Пользователь
    participant Node
    participant Storage
    participant Network
    participant Transport
    participant Peer as Удалённый пир

    User->>Node: get(CID)
    Node->>Storage: get(CID)
    alt Локальное попадание
        Storage-->>Node: Block ✓
        Note right of Storage: LRU: 30 мкс · Sled: 100 мкс
    else Локальный промах
        Node->>Network: find_providers(CID)
        Note right of Network: Kademlia DHT · 150–300 мс
        Network-->>Node: [PeerId₁, PeerId₂, ...]
        Node->>Transport: create_session([CID])
        Transport->>Peer: Want(CID)
        Peer-->>Transport: Block(CID, data)
        Transport->>Transport: проверка hash(data)==CID ✓
        Transport->>Storage: put(block)
        Transport-->>Node: Block ✓
    end
    Node-->>User: байты
```

### SEARCH — семантический запрос

```mermaid
sequenceDiagram
    actor User as Пользователь
    participant Node
    participant Model as ML-модель
    participant HNSW
    participant Storage

    User->>Node: search("deep learning", k=10)
    Node->>Model: embed(query) → vec[768]
    Node->>HNSW: search(vec, k=10)
    Note right of HNSW: Послойный спуск · ~99% recall · 1–10 мс
    HNSW-->>Node: [(CID₁, 0.92) ... (CID₁₀, 0.71)]
    loop для каждого результата
        Node->>Storage: get_metadata(CIDᵢ)
    end
    Node-->>User: [{cid, score, title, preview}]
```

---

## Storage — стек декораторов

```mermaid
graph TD
    REQ["📥 put / get / has"]
    D1["🔒 EncryptedBlockStore · AES-GCM"]
    D2["📦 CompressionBlockStore · zstd/lz4"]
    D3["🔍 DedupBlockStore · BF + точный хеш"]
    D4["💾 CachedBlockStore · LRU в памяти"]
    D5["📊 QuotaBlockStore · лимит размера"]
    D6["⏱️ TtlBlockStore · авто-протухание"]
    IMPL["🗄️ SledBlockStore · B+ tree · ACID · WAL"]
    DISK["💿 NVMe SSD"]

    REQ --> D1 --> D2 --> D3 --> D4 --> D5 --> D6 --> IMPL --> DISK
```

> Это иллюстративный полный стек. Реально собираемый по умолчанию стек —
> **`Bloom → Cache → Sled`** (`helpers.rs:136`). Подробности в [Wiki/04-StorageContext.md](Wiki/04-StorageContext.md).

---

## Network — топология P2P

```mermaid
graph TD
    BS["🏗️ Bootstrap-пиры"]

    subgraph LAN ["Локальная сеть (mDNS)"]
        A["Узел A"] <-->|"mDNS"| B["Узел B"]
        A <-->|"mDNS"| C["Узел C"]
    end

    subgraph WAN ["Интернет (Kademlia DHT)"]
        D["Узел D"] <-->|"XOR-маршрутизация"| E["Узел E"]
        E <-->|"XOR-маршрутизация"| F["Узел F"]
    end

    subgraph NAT ["За NAT"]
        H["Узел H (скрытый)"]
    end

    BS --> A & D
    A <-->|"QUIC / TCP"| D
    E -->|"Circuit Relay"| H
    D -->|"DCuTR Hole Punch"| H
```

---

## Граф зависимостей крейтов

```mermaid
graph TD
    %% Слой FFI
    PY["🐍 ipfrs-python\nPyO3 · 590 строк"]
    NJS["📦 ipfrs-nodejs\nnapi-rs · 1K строк"]
    WASM["🌐 ipfrs-wasm\nwasm-bindgen · 2K строк"]

    %% Интерфейс + CLI
    CLI["💻 ipfrs-cli\nclap · ratatui · 12K строк"]
    IF["🚪 ipfrs-interface\nШлюз · gRPC · GraphQL · 17K строк"]

    %% Приложение
    NODE["🧠 ipfrs\nУзел / Оркестратор · 15K строк"]

    %% Доменный слой
    ST["💾 ipfrs-storage\nSled · WAL · GC · 135K строк"]
    NW["🌐 ipfrs-network\nlibp2p · Kademlia · 156K строк"]
    SM["🔍 ipfrs-semantic\nHNSW · DiskANN · 142K строк"]
    TL["🤖 ipfrs-tensorlogic\n20+ движков · autograd · 156K строк"]
    TR["📡 ipfrs-transport\nBitswap · QUIC · 34K строк"]

    %% Shared Kernel
    CORE["⚙️ ipfrs-core\nCid · Block · Ipld · 23K строк"]

    %% FFI → node
    PY  --> NODE
    PY  --> TL
    NJS --> NODE
    NJS --> TL
    WASM -.->|"нет внутр. зависимостей"| CORE

    %% CLI
    CLI --> NODE
    CLI --> IF
    CLI --> TL

    %% Интерфейс → домены
    IF --> NODE
    IF --> ST
    IF --> NW
    IF --> SM
    IF --> TL

    %% Узел → все домены
    NODE --> ST
    NODE --> NW
    NODE --> SM
    NODE --> TL
    NODE --> TR

    %% Кросс-зависимости доменов
    NW --> TL
    SM --> ST
    SM --> NW
    SM --> TL
    TR --> ST
    TR --> NW
    TR --> TL
    TL --> ST

    %% Все → core
    ST  --> CORE
    NW  --> CORE
    SM  --> CORE
    TL  --> CORE
    TR  --> CORE
    NODE --> CORE
    IF  --> CORE
    CLI --> CORE

    %% Стили
    classDef ffi      fill:#fdf4ff,stroke:#d8b4fe,color:#581c87
    classDef app      fill:#d4edda,stroke:#10b981,color:#064e3b
    classDef gateway  fill:#cffafe,stroke:#06b6d4,color:#164e63
    classDef domain   fill:#dbeafe,stroke:#60a5fa,color:#1e3a8a
    classDef tl       fill:#fff7ed,stroke:#f97316,color:#c2410c
    classDef storage  fill:#fee2e2,stroke:#f87171,color:#7f1d1d
    classDef semantic fill:#ede9fe,stroke:#a78bfa,color:#4c1d95
    classDef transport fill:#d1fae5,stroke:#34d399,color:#064e3b
    classDef core     fill:#fef3c7,stroke:#f59e0b,color:#78350f

    class PY,NJS,WASM ffi
    class NODE app
    class IF gateway
    class CLI gateway
    class NW domain
    class TL tl
    class ST storage
    class SM semantic
    class TR transport
    class CORE core
```

> **Ключевое наблюдение:** `ipfrs-tensorlogic` — самый «горизонтальный» крейт:  
> его импортируют 8 из 12 крейтов (network, semantic, transport, interface, cli, node, nodejs, python).

---

## Строки кода

| Крейт | Файлы | Строки |
|-------|------:|------:|
| `ipfrs-tensorlogic` | 215 | 156,899 |
| `ipfrs-network` | 225 | 156,501 |
| `ipfrs-storage` | 165 | 135,684 |
| `ipfrs-semantic` | 169 | 142,392 |
| `ipfrs-transport` | 61 | 34,299 |
| `ipfrs-core` | 51 | 23,949 |
| `ipfrs-interface` | 29 | 17,511 |
| `ipfrs` (узел) | 46 | 15,420 |
| `ipfrs-cli` | 36 | 12,821 |
| `ipfrs-wasm` | 5 | 2,726 |
| `ipfrs-nodejs` | 2 | 1,060 |
| `ipfrs-python` | 1 | 590 |
| **Итого** | **1,005** | **699,852** |

> **699,852 строк** Rust в **1,005 файлах** в 12 крейтах (без артефактов сборки).  
> **724 файла** содержат `#[cfg(test)]` — обширное инлайн-покрытие тестами.  
> **~193 внешних зависимости** в 15 файлах `Cargo.toml`.  
> **Расположение**: `ipfrs_source/` (перенесено из `Vendor/ipfrs`).  
> _Пересчитано 2026-06-19 — выверено по рабочему дереву._

---

## Документация архитектуры

| Папка / документ | Файлы | Строки |
|------------------|------:|-------:|
| `Wiki_Antropic/` | 20 | 8,853 |
| `Wiki_Arch_GLM/` | 13 | 7,188 |
| `Wiki/` (новая, DDD) | 13 | 1,890 |
| `IPFRS_ARCHITECTURE.md` | 1 | 1,599 |
| `Wiki_GLM/` | 2 | 1,544 |
| `RoadMap/` | 7 | 1,536 |
| `README.md` (корень) | 1 | 472 |
| `CONTRIBUTORS.md` | 1 | 18 |
| **Итого (архитектура)** | **58** | **23,100** |

> **23,100 строк** документации архитектуры в **58 Markdown-файлах** (7 баз знаний + корневые доки).  
> **3 «живые» вики**: [`Wiki/`](Wiki/00-INDEX.md) (DDD, выверена по коду), [`Wiki_Antropic/`](Wiki_Antropic/INDEX.md) (доменные статьи), [`Wiki_Arch_GLM/`](Wiki_Arch_GLM/00-INDEX.md) (GLM-вариант — **все 6 доменных контекстов выверены по коду**, `06-LogicContext` = 1356 строк).  
> Всего Markdown в репозитории (без `target/`, `Vendor/`): **132 файла, 57,479 строк** — остальное это доки исходников и служебное.  
> _Пересчитано 2026-06-19 — выверено по рабочему дереву._

---

## Технологический стек

| Слой | Технология |
|------|-----------|
| Рантайм | Tokio async |
| Движок хранения | Sled (B+ tree, ACID, WAL) |
| Сеть | libp2p (QUIC, TCP, Kademlia, Gossip, mDNS) |
| Векторный индекс | hnsw_rs + DiskANN |
| Вывод | Собственный Datalog + 20+ типов движков |
| TLS | rustls |
| Сериализация | DAG-CBOR (IPLD), Apache Arrow, SafeTensors |
| gRPC | tonic |
| GraphQL | async-graphql |
| Python FFI | PyO3 |
| Node.js FFI | napi-rs |
| CLI | clap |

---

## Ключевые архитектурные решения

| Решение | Выбор | Почему |
|---------|-------|--------|
| Контент-адресация | CID = hash(data) | Дедупликация, целостность, кешируемость, неизменяемость |
| Хранение | Sled B+ tree | Чистый Rust, ACID, без C-зависимостей |
| Сеть | libp2p | Проверенная, протокол-агностичная, обход NAT |
| Векторный индекс | HNSW | Запросы O(log n), ~99% recall, в памяти |
| Вывод | Horn-clause Datalog | Разрешимый, композируемый, нейро-символическая фузия |
| Транспорт | Bitswap + WantList | Параллельный обмен блоками с несколькими пирами |

---

## Известные слабые места

> Обновлено 2026-06-19 по итогам глубокого исследования (7 агентов, привязка `file:line`).
> Полный реестр — [Wiki/11-RealityCheck.md](Wiki/11-RealityCheck.md).

**Опровергнутые ранее «критические баги»** (по факту кода уже корректны):
- JWT — это **HMAC-HS256, а не MD5** — `ipfrs/src/auth.rs:461`
- Backpressure-семафор **корректно** освобождает permits — `ipfrs-interface/src/backpressure.rs:185`
- «Баг FedAvg-таймаута» в `tensorlogic_ops.rs:1131` — **файла/строки не существует**

**Реальные заглушки и слабые места:**
- **Выкачка блоков по swarm — заглушка** → `NotFound` — `ipfrs-network/src/node.rs:1311` (блокер P2P GET)
- **Целостность при чтении НЕ проверяется** (`get` без `verify()`) — `ipfrs-storage/src/blockstore.rs:350`
- **`VectorIndex::rebuild`** молча опустошает индекс — `ipfrs-semantic/src/hnsw.rs:586`
- **TLS-генератор в node-крейте — заглушка** (rcgen только в комментарии) — `ipfrs/src/tls.rs:314`
- **GraphSync / erasure (Reed-Solomon) / NAT-STUN** — заглушки — `ipfrs-transport/src/{graphsync,erasure,nat_traversal}.rs`
- **Bitswap дублирован** в `ipfrs-transport` и `ipfrs-network` (несовместимые типы)
- GC-порог `min_age` enforced только в одной из трёх реализаций GC

---

## Документация

### Структура вики (3 «живые» базы знаний)

```
Wiki/                       — НОВАЯ: DDD «как функционирует IPFRS», выверена по коду
├── 00-INDEX · README       — навигация, карта зависимостей
├── 01-DomainOverview       — домен, единый язык, инварианты
├── 02-StrategicDesign      — 7 bounded contexts, карта контекстов
├── 03-SharedKernel         — Cid / Block / Ipld
├── 04-08 контексты         — Storage · Network · Semantic · TensorLogic · Transport
├── 09-ApplicationLayer     — Node-фасад + Gateway
├── 10-DataFlows            — ADD/GET/SEARCH/INFER/FedAvg сквозь контексты
└── 11-RealityCheck         — реестр заглушек, расхождения модели с кодом

Wiki_Antropic/              — подробные доменные статьи (паттерн Карпати, RU)
├── 01-15 + 12-RealityCheck — обзор, домены, потоки, HLD, проверка реальности
└── INDEX · README · WIKI_SCHEMA · log

Wiki_Arch_GLM/              — GLM-вариант DDD-анализа (13 файлов)
```

### Исходный код

- **`ipfrs_source/`** — полная кодовая база IPFRS (перенесена в корень)
  - `crates/` — 12 Rust-крейтов (storage, network, semantic, tensorlogic и др.)
  - `Cargo.toml` — конфигурация workspace
  - `book/`, `CRATE_DOCS.md` — документация исходников

---

## Контрибьюторы

| Контрибьютор | E-mail | Роль |
|--------------|--------|------|
| Temur | rust.istio@gmail.com | Maintainer · архитектура · документация |

Полный список — в [CONTRIBUTORS.md](CONTRIBUTORS.md). Вклад приветствуется — открывайте
issue или pull request.

---

## Лицензия

Этот проект распространяется под лицензией **GNU Affero General Public License v3.0
(AGPL-3.0)** — см. файл [LICENSE](LICENSE).

> **Примечание о происхождении.** IPFRS — форк [cool-japan/ipfrs](https://github.com/cool-japan/ipfrs),
> исходно под лицензией Apache-2.0. Объединённая работа в этом репозитории лицензируется
> под AGPL-3.0; атрибуция и условия Apache-2.0 для исходных частей сохраняются
> (см. заголовки файлов в `ipfrs_source/`). AGPL-3.0 требует раскрытия исходного кода
> при предоставлении доступа к ПО по сети.

---

*Глубокий анализ: Claude Opus 4.8 · параллельный workflow из 7 агентов · 2026-06-19*
*Первичный анализ: Claude Sonnet 4.6 · 6 агентов · 2026-06-18*
