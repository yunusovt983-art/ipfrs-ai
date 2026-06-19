# IPFRS — Inter-Planetary File Rust System

> Distributed content-addressed file system that unifies storage with ML intelligence.  
> Files are identified by their content hash (CID). Every block is its own address.

---

## Architecture — Helicopter View

```mermaid
graph TD
    User(["👤 User"])
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

    subgraph IPFRS ["IPFRS System"]
        direction TB

        IF["🚪 ipfrs-interface  ·  Gateway\n─────────────────────────────\nauth · TLS · backpressure\ngRPC · GraphQL · WS · FFI"]

        NODE["🧠 ipfrs  ·  Node / Orchestrator\n─────────────────────────────\nadd · get · search · query · pin · dag"]

        subgraph DOMAINS ["5 Bounded Contexts"]
            direction LR
            ST["💾 Storage\nSled + WAL\nGC · tiers\ndecorators"]
            NW["🌐 Network\nlibp2p · DHT\nGossip · NAT\nreputation"]
            SM["🔍 Semantic\nHNSW · DiskANN\nembeddings\nre-ranking"]
            LG["🤖 TensorLogic\n8 inference engines\nautograd · RL\nneuro-symbolic"]
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

## CID — Universal Boundary Token

```mermaid
flowchart LR
    B["📄 Bytes"] --> H{"hash()"} --> CID["🔑 CID\ncontent identifier"]

    CID -->|"storage key"| ST["💾 Storage\nSled B+ tree"]
    CID -->|"DHT record"| NW["🌐 Network\nCID → peers"]
    CID -->|"vector link"| SM["🔍 Semantic\nHNSW node"]
    CID -->|"rule content"| LG["🤖 Logic\nIPLD block"]
    CID -->|"peer request"| TR["📡 Transport\nWant(CID)"]
```

> All cross-context communication is just "pass a CID".

---

## Data Flows

### ADD — store a file

```mermaid
sequenceDiagram
    actor User
    participant Node
    participant Storage
    participant Network
    participant Semantic

    User->>Node: add(file, 100 MB)
    Node->>Node: chunk → 391 blocks (256 KB each)
    loop for each block
        Node->>Storage: put(block) → CID
        Node-)Network: announce(CID)
        Node->>Semantic: index(CID, embed)
    end
    Node-->>User: root_CID
    Note over User,Semantic: ~300 ms (no semantic) · ~900 ms (with semantic)
```

### GET — retrieve a file

```mermaid
sequenceDiagram
    actor User
    participant Node
    participant Storage
    participant Network
    participant Transport
    participant Peer as Remote Peer

    User->>Node: get(CID)
    Node->>Storage: get(CID)
    alt Local hit
        Storage-->>Node: Block ✓
        Note right of Storage: LRU: 30 µs · Sled: 100 µs
    else Local miss
        Node->>Network: find_providers(CID)
        Note right of Network: Kademlia DHT · 150–300 ms
        Network-->>Node: [PeerId₁, PeerId₂, ...]
        Node->>Transport: create_session([CID])
        Transport->>Peer: Want(CID)
        Peer-->>Transport: Block(CID, data)
        Transport->>Transport: verify hash(data)==CID ✓
        Transport->>Storage: put(block)
        Transport-->>Node: Block ✓
    end
    Node-->>User: bytes
```

### SEARCH — semantic query

```mermaid
sequenceDiagram
    actor User
    participant Node
    participant Model as ML Model
    participant HNSW
    participant Storage

    User->>Node: search("deep learning", k=10)
    Node->>Model: embed(query) → vec[768]
    Node->>HNSW: search(vec, k=10)
    Note right of HNSW: Layered descent · ~99% recall · 1–10 ms
    HNSW-->>Node: [(CID₁, 0.92) ... (CID₁₀, 0.71)]
    loop for each result
        Node->>Storage: get_metadata(CIDᵢ)
    end
    Node-->>User: [{cid, score, title, preview}]
```

---

## Storage — Decorator Stack

```mermaid
graph TD
    REQ["📥 put / get / has"]
    D1["🔒 EncryptedBlockStore · AES-GCM"]
    D2["📦 CompressionBlockStore · zstd/lz4"]
    D3["🔍 DedupBlockStore · BF + exact hash"]
    D4["💾 CachedBlockStore · LRU in-memory"]
    D5["📊 QuotaBlockStore · size limit"]
    D6["⏱️ TtlBlockStore · auto-expiry"]
    IMPL["🗄️ SledBlockStore · B+ tree · ACID · WAL"]
    DISK["💿 NVMe SSD"]

    REQ --> D1 --> D2 --> D3 --> D4 --> D5 --> D6 --> IMPL --> DISK
```

---

## Network — Peer-to-Peer Topology

```mermaid
graph TD
    BS["🏗️ Bootstrap Peers"]

    subgraph LAN ["Local Network (mDNS)"]
        A["Node A"] <-->|"mDNS"| B["Node B"]
        A <-->|"mDNS"| C["Node C"]
    end

    subgraph WAN ["Internet (Kademlia DHT)"]
        D["Node D"] <-->|"XOR routing"| E["Node E"]
        E <-->|"XOR routing"| F["Node F"]
    end

    subgraph NAT ["Behind NAT"]
        H["Node H (hidden)"]
    end

    BS --> A & D
    A <-->|"QUIC / TCP"| D
    E -->|"Circuit Relay"| H
    D -->|"DCuTR Hole Punch"| H
```

---

## Crate Dependency Graph

```mermaid
graph TD
    %% FFI layer
    PY["🐍 ipfrs-python\nPyO3 · 590 lines"]
    NJS["📦 ipfrs-nodejs\nnapi-rs · 1K lines"]
    WASM["🌐 ipfrs-wasm\nwasm-bindgen · 2K lines"]

    %% Interface + CLI
    CLI["💻 ipfrs-cli\nclap · ratatui · 12K lines"]
    IF["🚪 ipfrs-interface\nGateway · gRPC · GraphQL · 17K lines"]

    %% Application
    NODE["🧠 ipfrs\nNode / Orchestrator · 15K lines"]

    %% Domain layer
    ST["💾 ipfrs-storage\nSled · WAL · GC · 135K lines"]
    NW["🌐 ipfrs-network\nlibp2p · Kademlia · 156K lines"]
    SM["🔍 ipfrs-semantic\nHNSW · DiskANN · 142K lines"]
    TL["🤖 ipfrs-tensorlogic\n8 engines · autograd · 156K lines"]
    TR["📡 ipfrs-transport\nBitswap · QUIC · 34K lines"]

    %% Shared Kernel
    CORE["⚙️ ipfrs-core\nCid · Block · Ipld · 23K lines"]

    %% FFI → node
    PY  --> NODE
    PY  --> TL
    NJS --> NODE
    NJS --> TL
    WASM -.->|"no internal deps"| CORE

    %% CLI
    CLI --> NODE
    CLI --> IF
    CLI --> TL

    %% Interface → domains
    IF --> NODE
    IF --> ST
    IF --> NW
    IF --> SM
    IF --> TL

    %% Node → all domains
    NODE --> ST
    NODE --> NW
    NODE --> SM
    NODE --> TL
    NODE --> TR

    %% Domain cross-deps
    NW --> TL
    SM --> ST
    SM --> NW
    SM --> TL
    TR --> ST
    TR --> NW
    TR --> TL
    TL --> ST

    %% All → core
    ST  --> CORE
    NW  --> CORE
    SM  --> CORE
    TL  --> CORE
    TR  --> CORE
    NODE --> CORE
    IF  --> CORE
    CLI --> CORE

    %% Styles
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

## Lines of Code

| Crate | Files | Lines |
|-------|------:|------:|
| `ipfrs-tensorlogic` | 215 | 156,899 |
| `ipfrs-network` | 225 | 156,501 |
| `ipfrs-storage` | 165 | 135,684 |
| `ipfrs-semantic` | 169 | 142,392 |
| `ipfrs-transport` | 61 | 34,299 |
| `ipfrs-core` | 51 | 23,949 |
| `ipfrs-interface` | 29 | 17,511 |
| `ipfrs` (node) | 46 | 15,420 |
| `ipfrs-cli` | 36 | 12,821 |
| `ipfrs-wasm` | 5 | 2,726 |
| `ipfrs-nodejs` | 2 | 1,060 |
| `ipfrs-python` | 1 | 590 |
| **Total** | **1,005** | **699,852** |

> **702,768 lines** total including workspace root files.  
> **504,850 lines** of actual code (excluding blank lines and comments).  
> **724 files** contain `#[cfg(test)]` — extensive inline test coverage.  
> **~193 external dependencies** across 15 `Cargo.toml` files.  
> **Location**: `ipfrs_source/` (moved from `Vendor/ipfrs`)

---

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Runtime | Tokio async |
| Storage engine | Sled (B+ tree, ACID, WAL) |
| Networking | libp2p (QUIC, TCP, Kademlia, Gossip, mDNS) |
| Vector index | hnsw_rs + DiskANN |
| Inference | Custom Datalog + 8 engine types |
| TLS | rustls |
| Serialization | DAG-CBOR (IPLD), Apache Arrow, SafeTensors |
| gRPC | tonic |
| GraphQL | async-graphql |
| Python FFI | PyO3 |
| Node.js FFI | napi-rs |
| CLI | clap |

---

## Key Architectural Decisions

| Decision | Choice | Why |
|----------|--------|-----|
| Content addressing | CID = hash(data) | Deduplication, integrity, cacheable, immutable |
| Storage | Sled B+ tree | Pure Rust, ACID, no C deps |
| Network | libp2p | Battle-tested, protocol-agnostic, NAT traversal |
| Vector index | HNSW | O(log n) queries, ~99% recall, in-memory |
| Inference | Horn clause Datalog | Decidable, composable, neuro-symbolic fusion |
| Transport | Bitswap + WantList | Parallel multi-peer block exchange |

---

## Known Weaknesses

- JWT uses **MD5** instead of HS256 — `interface/src/auth.rs:449`
- TLS cert generator returns a **stub** — `interface/src/tls.rs:314`
- Backpressure semaphore permits **not revoked** on window decrease — `backpressure.rs:182`
- Storage GC `min_age` parameter accepted but **never applied** — `gc.rs:collect`
- FedAvg **always times out** when `min_peers > 0` — `tensorlogic_ops.rs:1131`
- Arrow "zero-copy" path performs **3 actual copies** — `interface/src/arrow.rs`

---

## Documentation

### Wiki Structure

```
Wiki/ (or Wiki_Arch_Claude/)
├── 01-Overview.md          — What is IPFRS?
├── 02-ArchitectureStack.md — 6-layer stack
├── 03-BoundedContexts.md   — 5 bounded contexts (DDD)
├── 04-StorageDomain.md     — Sled, blocks, decorators, GC
├── 05-NetworkDomain.md     — libp2p, DHT, peer discovery
├── 06-SemanticDomain.md    — HNSW, vector search
├── 07-LogicDomain.md       — Backward chaining, inference
├── 08-TransportDomain.md   — Bitswap, sessions
├── 09-DataFlows.md         — 4 end-to-end flows
├── 10-Performance.md       — P50/P99/P999 latency table
├── 11-ErrorHandling.md     — Recovery strategies
├── 12-MasterArchitecture.md — Full DDD analysis (RU)
├── 13-DeepArchitecture.md  — Deep architecture (RU)
├── 14-HLD.md               — Helicopter view (ASCII)
└── 15-HLD-Mermaid.md       — Helicopter view (Mermaid)
```

### Source Code

- **`ipfrs_source/`** — Complete IPFRS codebase (moved to root)
  - `crates/` — 12 Rust crates (storage, network, semantic, tensorlogic, etc.)
  - `Cargo.toml` — Workspace configuration
  - `ARCHITECTURE_*.md` — Original architecture docs

---

*Analyzed with Claude Sonnet 4.6 · 6-agent parallel workflow · 2026-06-18*
