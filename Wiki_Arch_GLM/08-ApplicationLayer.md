# Application Layer — Node Facade, Protocols, Bindings

> **Focus**: Facade pattern, protocol exposure, language bindings  
> **Source**: `ipfrs/src/`, `ipfrs-interface/src/`, bindings

---

## 1. Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                    PRESENTATION / HOST                              │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  CLI · gRPC · GraphQL · HTTP · WebSocket · FFI · Python · WASM      │
│                                                                     │
│                          │                                          │
│                          ▼                                          │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    APPLICATION FACADE                        │   │
│  │                       (Node)                                 │   │
│  │  ─────────────────────────────────────────────────────────── │   │
│  │  storage · network · semantic(OnceCell) · tensorlogic(Once)  │   │
│  │  auth · tls · pin · metrics                                  │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
│                          │                                          │
│                          ▼                                          │
│                                                                     │
│              STORAGE · NETWORK · SEMANTIC · LOGIC · TRANSPORT       │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 2. Node Orchestrator

### 2.1 Structure

```rust
pub struct Node {
    pub config: NodeConfig,
    
    // Domain contexts
    pub network: Option<NetworkNode>,
    pub storage: Option<Arc<NodeStore>>,
    
    // Lazy init (zero-cost if unused)
    pub semantic: OnceCell<Arc<SemanticRouter>>,
    pub tensorlogic: OnceCell<Arc<TensorLogicStore<NodeStore>>>,
    
    // Cross-cutting
    pub auth_manager: Option<Arc<AuthManager>>,
    pub tls_manager: Option<Arc<TlsManager>>,
    pub pin_manager: Arc<PinManager>,
    pub metrics: Arc<IpfrsMetrics>,
}
```

### 2.2 Use Cases

```rust
impl Node {
    // === Storage ===
    pub async fn put_block(&self, block: &Block) -> Result<()>;
    pub async fn get_block(&self, cid: &Cid) -> Result<Option<Block>>;
    pub async fn has_block(&self, cid: &Cid) -> Result<bool>;
    
    // === Semantic ===
    pub async fn index_content(&self, cid: &Cid, embedding: Vec<f32>) -> Result<()>;
    pub async fn search_similar(&self, query: Vec<f32>, k: usize) -> Result<Vec<SearchResult>>;
    
    // === Logic ===
    pub async fn add_fact(&self, predicate: Predicate) -> Result<()>;
    pub async fn infer(&self, goal: &Predicate) -> Result<Vec<Substitution>>;
    
    // === Pin ===
    pub async fn pin_add(&self, cid: &Cid, recursive: bool) -> Result<()>;
    pub async fn pin_ls(&self) -> Result<Vec<(Cid, PinInfo)>>;
    pub async fn pin_rm(&self, cid: &Cid) -> Result<()>;
    
    // === DAG ===
    pub async fn dag_export(&self, root: &Cid) -> Result<CarWriter>;
    pub async fn dag_import(&self, car: CarReader) -> Result<Cid>;
}
```

### 2.3 Lazy Initialization

```rust
impl Node {
    pub fn new(config: NodeConfig) -> Result<Self> {
        // Only essential contexts
        let storage = config.storage.enabled.then(|| Arc::new(NodeStore::new(&config.storage)?));
        let network = config.network.enabled.then(|| NetworkNode::new(&config.network)?);
        
        Ok(Self {
            config,
            storage,
            network,
            semantic: OnceCell::new(),      // Not initialized
            tensorlogic: OnceCell::new(),   // Not initialized
            // ...
        })
    }
}
```

---

## 3. Interface Protocols

### 3.1 gRPC

```protobuf
service BlockService {
    rpc PutBlock(stream PutBlockRequest) returns (PutBlockResponse);
    rpc GetBlock(GetBlockRequest) returns (stream GetBlockResponse);
}

service TensorService {
    rpc GetTensorSlice(TensorSliceRequest) returns (TensorSliceResponse);
}
```

**Zero-copy tensor path**: Direct slice extraction from `TensorBlock`.

---

### 3.2 GraphQL

```rust
#[Object]
impl QueryRoot {
    async fn block(&self, cid: String) -> Result<Option<BlockType>>;
    async fn semantic_search(&self, query: Vec<f32>, k: i32) -> Result<Vec<SearchResultType>>;
    async fn infer(&self, goal: PredicateInput) -> Result<Vec<SubstitutionType>>;
}

#[Object]
impl MutationRoot {
    async fn put_block(&self, data: Vec<u8>) -> Result<String>;
    async fn index_content(&self, cid: String, embedding: Vec<f32>) -> Result<bool>;
}
```

---

### 3.3 HTTP Gateway

```rust
// Kubo-compatible API
router.route("/api/v0/add", post(add_handler));
router.route("/api/v0/cat", get(cat_handler));
router.route("/api/v0/pin/add", post(pin_add_handler));

// Content gateway
router.route("/ipfs/:cid", get(content_handler));

// Tensor API
router.route("/v1/tensor/:cid", get(tensor_handler));
```

**Byte ranges**: Content handler supports `Range` header.

---

### 3.4 WebSocket

```rust
pub enum RealtimeEvent {
    BlockAdded { cid: Cid },
    PeerConnected { peer_id: String },
    DhtQueryCompleted { query_id: u64 },
    SessionCompleted { session_id: u64 },
}
```

---

## 4. Authentication

### 4.1 JWT

```rust
pub struct Claims {
    pub sub: String,
    pub role: Role,
    pub exp: usize,
}

pub enum Role {
    Admin,
    Writer,
    Reader,
}

pub enum Permission {
    BlockRead,
    BlockWrite,
    PinManage,
    AdminAccess,
}
```

### 4.2 OAuth2 with PKCE

```rust
pub struct OAuth2Config {
    pub client_id: String,
    pub auth_url: String,
    pub token_url: String,
}
```

---

## 5. Bindings

### 5.1 FFI

```rust
#[repr(C)]
pub struct IpfrsClient {
    inner: *mut c_void,
}

#[repr(C)]
pub enum IpfrsErrorCode {
    Success = 0,
    NotFound = 1,
    InvalidInput = 2,
    // ...
}

#[no_mangle]
pub extern "C" fn ipfrs_client_new(config_json: *const c_char) -> IpfrsClient;
#[no_mangle]
pub extern "C" fn ipfrs_client_add(client: *mut IpfrsClient, data: *const u8, len: usize) -> IpfrsBlock;
```

---

### 5.2 Python (PyO3)

```rust
#[pyclass]
pub struct PyClient {
    node: Arc<Node>,
}

#[pymethods]
impl PyClient {
    #[new]
    fn new(config_path: Option<&str>) -> PyResult<Self>;
    
    fn add(&self, data: &[u8]) -> PyResult<String>;
    fn get(&self, cid: &str) -> PyResult<Option<Vec<u8>>>;
    fn has(&self, cid: &str) -> PyResult<bool>;
}
```

---

## 6. Performance

| Protocol | P50 | P99 |
|----------|-----|-----|
| gRPC (get) | 50µs | 100µs |
| GraphQL | 100µs | 500µs |
| HTTP | 100µs | 500µs |
| WebSocket | 10µs | 50µs |
| FFI (Python) | 200µs | 1ms |

---

## 7. Key Files

| File | Lines | Purpose |
|------|-------|---------|
| `ipfrs/src/node/mod.rs` | 400+ | Node orchestrator |
| `interface/grpc.rs` | 500+ | gRPC services |
| `interface/graphql.rs` | 400+ | GraphQL schema |
| `interface/gateway/mod.rs` | 600+ | HTTP gateway |
| `interface/ffi.rs` | 400+ | C FFI |
| `interface/python.rs` | 350+ | PyO3 bindings |

---

## 8. Design Decisions

### 8.1 Why Facade?

**Decision**: Single `Node` composing all contexts.

**Rationale**:
- Single entry point
- Hide context complexity
- Lazy initialization

---

### 8.2 Why Multiple Protocols?

**Decision**: gRPC, GraphQL, HTTP, WS all supported.

**Rationale**:
- gRPC: High-performance, streaming
- GraphQL: Flexible queries
- HTTP: Browser compatible
- WS: Real-time

---

## 9. Context Integration

```
┌─────────────────────────────────────────────────────────────────────┐
│                    APPLICATION INTEGRATION                          │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Presentation → Application → Domain → Shared Kernel                │
│                                                                     │
│  All protocols funnel into Node use cases                           │
│  Never touch domain aggregates directly                             │
│                                                                     │
│  Lazy init: semantic, tensorlogic (OnceCell)                        │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

**Next**: [09-ContextIntegration.md](09-ContextIntegration.md) — Cross-context flows, ACL patterns
