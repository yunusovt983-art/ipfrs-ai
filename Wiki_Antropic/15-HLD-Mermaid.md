---
title: 15-HLD-Mermaid
type: architecture
summary: Helicopter View HLD в формате Mermaid — контекст системы, домены, потоки данных, стек хранилища, топология сети
tags: [ipfrs, hld, mermaid, architecture, diagram, helicopter-view]
source: cool-japan/Vendor/ipfrs/
related: ["[[14-HLD]]", "[[03-BoundedContexts]]", "[[09-DataFlows]]"]
read_time: 10 мин
updated: 2026-06-18
---

# IPFRS: Helicopter View (Mermaid HLD)

> Все диаграммы рендерятся нативно в Obsidian.  
> Текстовая версия: [[14-HLD]]

---

## 1. Контекст системы (C4 Level 1)

```mermaid
graph TD
    User(["👤 Пользователь"])
    CLI["💻 CLI\nipfrs-cli"]
    PY["🐍 Python\nбиндинг"]
    JS["📦 Node.js\nбиндинг"]
    WB["🌐 WASM\nбраузер"]
    GW["🔌 gRPC / GraphQL\n/ REST / WebSocket"]

    User --> CLI
    User --> GW
    User --> PY
    User --> JS
    User --> WB

    subgraph IPFRS ["🗄 IPFRS — Inter-Planetary File Rust System"]
        direction TB

        IF["🚪 ipfrs-interface\nGateway\n────────────────\nauth · TLS · backpressure\ngRPC · GraphQL · WS · FFI"]

        NODE["🧠 ipfrs (Node)\nОркестратор\n────────────────\nadd · get · search\nquery · pin · dag"]

        subgraph DOMAINS ["Доменный слой (5 Bounded Contexts)"]
            direction LR
            ST["💾 Storage\nSled + WAL\n+ GC + тиеринг\n+ декораторы"]
            NW["🌐 Network\nlibp2p · Kademlia\nGossip · NAT\n+ репутация"]
            SM["🔍 Semantic\nHNSW · DiskANN\nembeddings\nre-ranking"]
            LG["🤖 TensorLogic\n8 движков вывода\nautograd · RL\nнейро-символьный"]
            TR["📡 Transport\nBitswap · Session\nWantList · QUIC\nTensorSwap"]
        end

        CORE["⚙️ ipfrs-core (Shared Kernel)\nCid · Block · Ipld · TensorBlock · IpfrsError"]

        IF --> NODE
        NODE --> ST
        NODE --> NW
        NODE --> SM
        NODE --> LG
        NODE --> TR
        ST --> CORE
        NW --> CORE
        SM --> CORE
        LG --> CORE
        TR --> CORE
    end

    CLI --> IF
    GW  --> IF
    PY  --> IF
    JS  --> IF
    WB  --> IF

    style CORE fill:#fff3cd,stroke:#ffc107
    style IF   fill:#cce5ff,stroke:#004085
    style NODE fill:#d4edda,stroke:#155724
    style ST   fill:#f8d7da,stroke:#721c24
    style NW   fill:#d1ecf1,stroke:#0c5460
    style SM   fill:#e2d9f3,stroke:#4a235a
    style LG   fill:#fde8d8,stroke:#a04000
    style TR   fill:#d6eaf8,stroke:#1a5276
```

---

## 2. CID — универсальный граничный токен

```mermaid
flowchart LR
    B["📄 Bytes\n(любые данные)"]
    H{"hash()"}
    CID["🔑 CID\nContent Identifier\n= hash(bytes)"]

    B --> H --> CID

    CID -->|"ключ хранения"| ST["💾 Storage\nSled B+ tree"]
    CID -->|"DHT-запись"| NW["🌐 Network\n(CID → [peers])"]
    CID -->|"связь с вектором"| SM["🔍 Semantic\nHNSW node"]
    CID -->|"контент правила"| LG["🤖 Logic\nIPLD block"]
    CID -->|"запрос пиру"| TR["📡 Transport\nWant(CID)"]

    note["⚡ Правило:\nвсе межконтекстные\nвзаимодействия —\nэто «передай CID»"]

    style CID fill:#fff3cd,stroke:#ffc107,font-weight:bold
    style note fill:#f8f9fa,stroke:#adb5bd
```

---

## 3. Карта контекстов (Context Map DDD)

```mermaid
graph TD
    CORE["⚙️ ipfrs-core\nShared Kernel"]

    subgraph BC ["Bounded Contexts"]
        ST["💾 Storage"]
        NW["🌐 Network"]
        SM["🔍 Semantic"]
        LG["🤖 TensorLogic"]
        TR["📡 Transport"]
    end

    APP["🧠 Node\nApplication Facade"]

    %% Shared Kernel
    ST -->|"использует"| CORE
    NW -->|"использует"| CORE
    SM -->|"использует"| CORE
    LG -->|"использует"| CORE
    TR -->|"использует"| CORE

    %% Context relationships
    TR -->|"Customer/Supplier + ACL\nBlockStore trait"| ST
    TR -->|"Customer/Supplier\n(реплицирует скоринг)"| NW
    LG -->|"Published Language\n(правила = IPLD блоки)"| ST
    NW -->|"ACL\nlibp2p::PeerId → String"| EXT["🔧 libp2p\n(внешний)"]

    %% App orchestrates all
    APP -->|"OHS"| ST
    APP -->|"OHS"| NW
    APP -->|"OHS"| SM
    APP -->|"OHS"| LG
    APP -->|"OHS"| TR

    style CORE fill:#fff3cd,stroke:#ffc107
    style APP  fill:#d4edda,stroke:#155724
    style EXT  fill:#f8d7da,stroke:#721c24
```

---

## 4. Стек декораторов Storage

```mermaid
graph TD
    REQ["📥 Запрос\nput / get / has"]

    D1["🔒 EncryptedBlockStore\nAES-GCM шифрование"]
    D2["📦 CompressionBlockStore\nzstd / lz4 / snappy"]
    D3["🔍 DedupBlockStore\nBF + точный хеш"]
    D4["💾 CachedBlockStore\nLRU in-memory"]
    D5["📊 QuotaBlockStore\nограничение размера"]
    D6["⏱️ TtlBlockStore\nавто-истечение"]
    IMPL["🗄️ SledBlockStore\nB+ tree, ACID, WAL"]

    REQ --> D1 --> D2 --> D3 --> D4 --> D5 --> D6 --> IMPL

    IMPL -->|"диск"| DISK["💿 NVMe SSD"]

    note["➕ Каждый слой реализует\nBlockStore trait независимо.\nЛегко добавить/убрать\nне ломая остальное."]

    style IMPL fill:#f8d7da,stroke:#721c24
    style DISK fill:#e2e3e5,stroke:#383d41
    style note fill:#f8f9fa,stroke:#adb5bd
```

---

## 5. Поток ADD (добавить файл)

```mermaid
sequenceDiagram
    actor User as 👤 Пользователь
    participant Node as 🧠 Node
    participant Chunker as ✂️ Chunker
    participant Storage as 💾 Storage
    participant Network as 🌐 Network
    participant Semantic as 🔍 Semantic

    User->>Node: add(file.pdf, 100 MB)
    activate Node

    Node->>Chunker: chunk(bytes)
    Chunker-->>Node: [block₁..block₃₉₁] (по 256 КБ)

    loop для каждого из 391 блоков
        Node->>Storage: put(blockᵢ)
        Note right of Storage: CID = hash(data)<br/>Sled + LRU cache
        Storage-->>Node: CID

        Node-)Network: announce(CID)  [async]
        Note right of Network: DHT.put_provider<br/>(CID → my_peer_id)

        opt если semantic включён
            Node->>Semantic: index(CID, embed(text))
            Note right of Semantic: HNSW.insert<br/>(вектор 768 dim)
            Semantic-->>Node: ok
        end
    end

    Node-->>User: root_CID = bafybeig...
    deactivate Node

    Note over User,Semantic: ⏱ ~300 мс (без semantic) / ~900 мс (с semantic)
```

---

## 6. Поток GET (получить файл)

```mermaid
sequenceDiagram
    actor User as 👤 Пользователь
    participant Node as 🧠 Node
    participant Storage as 💾 Storage
    participant Network as 🌐 Network
    participant Transport as 📡 Transport
    participant Peer as 🖥️ Удалённый пир

    User->>Node: get(CID)
    activate Node

    Node->>Storage: get(CID)

    alt Локальное попадание
        Storage-->>Node: Block ✓
        Note right of Storage: LRU: 30 мкс<br/>Sled: 100 мкс
        Node-->>User: bytes
    else Локальный промах
        Storage-->>Node: None

        Node->>Network: find_providers(CID)
        Note right of Network: Kademlia DHT<br/>итеративный XOR-поиск
        Network-->>Node: [PeerId₁, PeerId₂, ...]
        Note over Network: ⏱ 150–300 мс

        Node->>Transport: create_session([CID])
        activate Transport

        Transport->>Peer: Want(CID, priority=100)
        Note right of Peer: Storage.get(CID)<br/>на стороне пира
        Peer-->>Transport: Block(CID, data)
        Note over Transport,Peer: ⏱ 50–200 мс RTT × 2

        Transport->>Transport: verify: hash(data)==CID ✓
        Transport->>Storage: put(block)
        Transport-->>Node: Block ✓
        deactivate Transport

        Node-->>User: bytes
    end

    deactivate Node
```

---

## 7. Поток SEARCH (семантический поиск)

```mermaid
sequenceDiagram
    actor User as 👤 Пользователь
    participant Node as 🧠 Node
    participant Model as 🤖 ML Model
    participant Cache as ⚡ Query Cache
    participant HNSW as 🔍 HNSW Index
    participant Storage as 💾 Storage

    User->>Node: search("глубокое обучение", k=10)
    activate Node

    Node->>Model: embed(query)
    Model-->>Node: вектор [0.14, -0.09, ...] (768 dim)

    Node->>Cache: lookup(hash(вектор))

    alt Кэш-попадание (~85%)
        Cache-->>Node: [(CID₁, 0.92), ..., (CID₁₀, 0.71)]
        Note right of Cache: ⏱ ~1 мс
    else Кэш-промах (~15%)
        Cache-->>Node: miss

        Node->>HNSW: search(вектор, k=10)
        Note right of HNSW: Послойный спуск:<br/>Layer 2 → 1 → 0<br/>~99% recall
        HNSW-->>Node: [(CID₁, 0.92), ..., (CID₁₀, 0.71)]
        Note over HNSW: ⏱ 1–10 мс (100k векторов)

        Node->>Cache: store(hash(вектор), results)
    end

    loop для каждого из top-k CID
        Node->>Storage: get_metadata(CIDᵢ)
        Storage-->>Node: {title, preview}
    end

    Node-->>User: [{cid, score, title, preview}, ...]
    deactivate Node
```

---

## 8. Поток QUERY (логический запрос)

```mermaid
sequenceDiagram
    actor User as 👤 Пользователь
    participant Node as 🧠 Node
    participant KB as 📚 KnowledgeBase
    participant Engine as ⚙️ InferenceEngine
    participant Semantic as 🔍 Semantic (fallback)

    User->>Node: query(ancestor(alice, ?))
    activate Node

    Node->>KB: get_facts_and_rules()
    KB-->>Node: {facts: [...], rules: [...]}

    Node->>Engine: infer(ancestor(alice, ?))
    activate Engine

    loop Backward Chaining
        Engine->>Engine: unify(goal, rule_head)
        Engine->>Engine: prove(subgoals...)
        Note right of Engine: Глубина: max 1000<br/>Обнаружение циклов
    end

    alt Решения найдены
        Engine-->>Node: [{X: bob}, {X: charlie}]
    else Решений нет + Hybrid режим
        Engine-->>Node: []

        Node->>Semantic: search(embed("ancestor alice"), k=5)
        Note right of Semantic: Векторный fallback:<br/>ищем семантически похожие факты
        Semantic-->>Node: [(CID₁, 0.88), ...]
        Node->>Node: merge(symbolic=[], neural=[...])
    end

    deactivate Engine
    Node-->>User: [{substitution}, ...]
    deactivate Node
```

---

## 9. Топология сети peer-to-peer

```mermaid
graph TD
    BS["🏗️ Bootstrap\nПиры\n(известные точки входа)"]

    subgraph LAN ["🏠 Локальная сеть (mDNS)"]
        A["📦 Node A"] <-->|"mDNS broadcast"| B["📦 Node B"]
        A <-->|"mDNS broadcast"| C["📦 Node C"]
    end

    subgraph INTERNET ["🌍 Интернет (Kademlia DHT)"]
        D["📦 Node D"] <-->|"XOR routing"| E["📦 Node E"]
        E <-->|"XOR routing"| F["📦 Node F"]
        D <-->|"XOR routing"| G["📦 Node G"]
    end

    BS -->|"первичное\nобнаружение"| A
    BS -->|"первичное\nобнаружение"| D

    A <-->|"QUIC / TCP"| D
    B <-->|"QUIC / TCP"| F

    subgraph NAT ["🔒 За NAT"]
        H["📦 Node H\n(hidden)"]
    end

    E -->|"Circuit Relay\n(если DCuTR не прошёл)"| H
    D -->|"DCuTR\nHole Punching"| H

    style BS fill:#fff3cd,stroke:#ffc107
    style LAN fill:#d4edda,stroke:#155724
    style INTERNET fill:#cce5ff,stroke:#004085
    style NAT fill:#f8d7da,stroke:#721c24
```

---

## 10. Машина состояний Transport-сессии

```mermaid
stateDiagram-v2
    [*] --> Active : create_session()

    Active --> Paused       : pause()
    Paused --> Active       : resume()

    Active --> Completing   : все блоки получены\n(blocks_received + blocks_failed ≥ total)
    Completing --> Completed : финальная верификация ✓

    Active --> Cancelled    : cancel() / timeout
    Paused --> Cancelled    : timeout (5 мин)
    Completing --> Cancelled : верификация провалилась

    Completed --> [*]
    Cancelled --> [*]

    note right of Active
        Активно запрашивает блоки
        по приоритетам WantList
    end note

    note right of Completing
        received + failed ≥ total
        SessionStats::is_complete()
    end note
```

---

## 11. Репутация пиров — два уровня

```mermaid
graph LR
    subgraph NETWORK ["🌐 Network — долгосрочная репутация"]
        NR1["transfer_success_rate\n(EWMA)"]
        NR2["latency_score\n(EWMA)"]
        NR3["protocol_compliance\n(EWMA)"]
        NR4["uptime_score\n(EWMA)"]
        NC["Composite Score\nWeighted sum"]
        NG["Trust Graph\ndirect × 0.6\n+ propagated × 0.4"]

        NR1 & NR2 & NR3 & NR4 --> NC
        NC --> NG
    end

    subgraph TRANSPORT ["📡 Transport — session-local репутация"]
        TS["success_in_session\n/ total_requests"]
        TL["avg_latency_ms\n(EWMA)"]
        TC["connected?\n× 1.0 : 0.1"]
        TA["connection_age\nbonus"]
        SCORE["Session Score\nproduct"]

        TS & TL & TC & TA --> SCORE
    end

    WHY["💡 Почему дублировать?\n• Автономность\n• Устойчивость к сбоям\n• Разные метрики\n• Независимое тестирование"]

    style NETWORK  fill:#d1ecf1,stroke:#0c5460
    style TRANSPORT fill:#d6eaf8,stroke:#1a5276
    style WHY fill:#fff3cd,stroke:#ffc107
```

---

## 12. Нейро-символьное слияние (TensorLogic)

```mermaid
flowchart TD
    GOAL["❓ Запрос: ancestor(alice, ?)"]

    SYM["⚙️ Symbolic Engine\nBackward Chaining\nSLD Resolution"]
    GOAL --> SYM

    SYM --> HAS{Решения\nнайдены?}

    HAS -->|"✅ Да"| RES["📋 Результаты\n{X: bob, X: charlie}"]

    HAS -->|"❌ Нет + Hybrid"| EMB["🔢 Embed query\n→ вектор 768 dim"]
    EMB --> HNSW["🔍 HNSW поиск\nk-NN в семантическом пространстве"]
    HNSW --> NEAR["📌 Похожие факты\n(CID₁, 0.88), (CID₂, 0.81)"]
    NEAR --> MERGE["🔀 Merge\nsymbolic ∪ neural"]
    MERGE --> RES

    HAS -->|"❌ Нет + PureNeural"| EMB2["🧠 Полностью нейронный\nвекторное пространство\nкак единственный источник"]
    EMB2 --> RES

    MODES["Режимы:\n• PureSymbolic\n• Hybrid(neural_weight: f32)\n• PureNeural"]

    style GOAL fill:#fff3cd,stroke:#ffc107
    style RES  fill:#d4edda,stroke:#155724
    style MODES fill:#f8f9fa,stroke:#adb5bd
```

---

## 13. Стек технологий

```mermaid
graph TB
    subgraph UI ["L0 · Интерфейсы"]
        CLI2["ipfrs-cli\nclap"]
        GRPC2["gRPC\ntonic"]
        GQL["GraphQL\nasync-graphql"]
        WS["WebSocket\ntokio-tungstenite"]
        FFI2["FFI\nPyO3 / napi-rs"]
        WASM2["WASM\nwasm-bindgen"]
    end

    subgraph APP2 ["L1 · Приложение"]
        NODE2["ipfrs Node\nOrchestrator"]
    end

    subgraph DOMAIN2 ["L2 · Домены"]
        SLED["Sled\nB+ tree ACID"]
        LP["libp2p\nQUIC/TCP"]
        HNSW2["hnsw_rs\n+ DiskANN"]
        DATALOG["Custom Datalog\nHorn clauses"]
        TOKIO["Tokio async\nruntime"]
    end

    subgraph INFRA ["L3–L4 · Инфраструктура"]
        RUSTLS["rustls\nTLS"]
        ARROW2["Apache Arrow\nSafeTensors"]
        CBOR["DAG-CBOR\nIPLD"]
        DASHMAP["DashMap\nlock-free"]
    end

    subgraph HW2 ["L5 · Железо"]
        NVME["NVMe SSD"]
        NET2["Ethernet / WiFi"]
        CPU2["CPU (SIMD AVX2/NEON)"]
    end

    UI --> APP2 --> DOMAIN2 --> INFRA --> HW2

    style UI     fill:#cce5ff,stroke:#004085
    style APP2   fill:#d4edda,stroke:#155724
    style DOMAIN2 fill:#e2d9f3,stroke:#4a235a
    style INFRA  fill:#fff3cd,stroke:#ffc107
    style HW2    fill:#e2e3e5,stroke:#383d41
```

---

## 14. NFR — нефункциональные требования

```mermaid
mindmap
  root((IPFRS NFR))
    Масштабируемость
      Storage → петабайты
      Semantic → 1M+ векторов
      Network → 1K+ пиров
      Горизонтальное per-domain
    Производительность
      GET кэш < 30 мкс
      HNSW search < 10 мс
      DHT lookup 150–300 мс
      Block fetch 200–1000 мс
    Надёжность
      hash == CID на каждом чтении
      k=20 DHT реплик
      Retry + fallback peers
      Circuit breaker
    Безопасность
      TLS (rustls)
      JWT + OAuth2
      Capability model
      Content hash verification
    Расширяемость
      BlockStore trait → любой бэкенд
      InferenceEngine trait → 8+ движков
      Transport plugins
    Наблюдаемость
      IpfrsMetrics (Arc)
      OpenTelemetry tracing
      Profiler per-domain
```

---

## Навигация

| Хочу узнать | Читай |
|-------------|-------|
| Только текстовые диаграммы | [[14-HLD]] |
| Полный DDD-анализ | [[12-MasterArchitecture]] |
| Глубокое погружение | [[13-DeepArchitecture]] |
| Домен Storage | [[04-StorageDomain]] |
| Домен Network | [[05-NetworkDomain]] |
| Семантический поиск | [[06-SemanticDomain]] |
| Логика и вывод | [[07-LogicDomain]] |
| Транспорт Bitswap | [[08-TransportDomain]] |
| Все потоки данных | [[09-DataFlows]] |

---

**Связанные**: [[14-HLD]] | [[01-Overview]] | [[03-BoundedContexts]] | [[09-DataFlows]] | [[12-MasterArchitecture]]
