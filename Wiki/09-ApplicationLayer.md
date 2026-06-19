---
title: 09-ApplicationLayer
type: domain
summary: Application Layer (ipfrs::Node как Facade) + Interface/Gateway (gRPC/GraphQL/WS/HTTP, Auth, TLS); как контексты собираются вместе
tags: [ipfrs, ddd, application, facade, gateway, auth, tls]
source: ipfrs_source/crates/ipfrs/, ipfrs-interface/
related: ["[[02-StrategicDesign]]", "[[10-DataFlows]]", "[[11-RealityCheck]]"]
read_time: 13 мин
updated: 2026-06-19
---

# Application Layer + Gateway — `ipfrs`, `ipfrs-interface`

**Краткое резюме**: `ipfrs::Node` — это **Facade/Application Service**, собирающий 5
доменных контекстов в сценарии. `ipfrs-interface` — **Open Host Service**:
экспонирует этот фасад через 4 протокола и переводит внешние форматы в доменные
операции (ACL-граница).

---

## 1. `Node` как композиционный корень

```rust
// ipfrs/src/node/mod.rs:34 (сокращено)
pub struct Node {
    network: Option<NetworkNode>,                          // Network
    storage: Option<Arc<NodeStore>>,                       // Storage
    semantic: OnceCell<Arc<SemanticRouter>>,              // Semantic (лениво)
    tensorlogic: OnceCell<Arc<TensorLogicStore<NodeStore>>>, // TensorLogic (лениво)
    auth_manager: Option<Arc<AuthManager>>,               // cross-cutting
    tls_manager: Option<Arc<TlsManager>>,                 // cross-cutting
    pin_manager: Arc<PinManager>,                          // GC-safety
    pub metrics: Arc<IpfrsMetrics>,
}
// NodeStore = CachedBlockStore<SledBlockStore>  (mod.rs:31)
```

Фасад **разбит на op-модули** (по одному `impl Node` на заботу) — чистое разделение:
`block_ops`, `dag_ops`, `pin_ops`, `semantic_ops`, `tensorlogic_ops` (самый богатый,
1290 строк), `network_ops`, `repo_ops`, `auth_ops`.

**Ленивая инициализация**: Semantic и TensorLogic поднимаются через
`OnceCell::get_or_try_init` (`mod.rs:69`) под флагами конфигурации; `warmup()`
форсирует их (`core.rs:345`).

---

## 2. Опубликованные API (Gateway)

| Протокол | Точка входа | Источник |
|----------|-------------|----------|
| **HTTP-gateway** (Axum) | `Gateway::router()`, `/ipfs/{cid}`, Kubo v0, v1-stream | `gateway/mod.rs:283` |
| **gRPC** (Tonic, feature) | Block/Dag/File/Tensor services + `GradientSyncService` | `grpc.rs:247,1384` |
| **GraphQL** (async-graphql) | Query: `block/semantic_search/infer/prove`; Mutation: `add_block/index_content/add_fact` | `graphql.rs:82,276` |
| **WebSocket** pub/sub | топик→`broadcast::Sender<RealtimeEvent>` | `websocket.rs:112` |

Опциональные сервисы инжектятся через `Option`/`.data()` только при наличии — feature
gating по `Option`.

---

## 3. Anti-Corruption Layer (примеры)

- multipart-форма → `Block::new(...)` → Kubo-ответ `{Name,Hash,Size}` (`routes.rs:385`).
- HTTP `Range` → `206 Partial Content`/`multipart/byteranges` (`routes.rs:66`).
- gRPC `AuthInterceptor` (`grpc.rs:1081`): метаданные `Bearer` → `validate_token` →
  `Status::unauthenticated` при провале.
- FFI: наружу только непрозрачные `#[repr(C)]`-указатели, каждый `extern "C"` обёрнут
  в `catch_unwind` (`ffi.rs:54,82`).

---

## 4. Сквозные заботы (cross-cutting)

| Забота | Реализация |
|--------|------------|
| **Auth** | `AuthManager` (node) + `JwtManager`/`UserStore`/OAuth2 (interface) |
| **TLS** | `TlsManager` (node) + rustls `from_pem_file` (interface) |
| **Метрики** | `IpfrsMetrics`, Prometheus `/metrics`, gRPC `MetricsInterceptor` |
| **Rate limiting** | token-bucket (`middleware.rs:286`), gRPC `RateLimitInterceptor` |
| **Health** | `/health` + `HealthChecker` liveness/readiness (`health.rs:48`) |
| **Graceful shutdown** | `ShutdownCoordinator` (broadcast + SIGTERM/SIGINT) |

---

## 5. Ключевые инварианты прикладного слоя

1. **Pin-safety vs GC** (центральный инвариант долговечности): mark-фаза засевает
   достижимое из `PinManager::list()` (`gc.rs:134`), sweep пропускает достижимое
   (`gc.rs:100`). Direct/Recursive/Indirect-пин никогда не собирается.
2. **Валидность токена**: `AuthToken::is_expired` + JWT `validate_exp` + lookup в
   реестре (отзыв = `tokens.remove`).
3. **Целостность репозитория**: `fsck` пересчитывает CID из байтов и сравнивает
   (`fsck.rs:138`).
4. **Предусловие пина**: нельзя запинить несуществующий блок (`pin_ops.rs:15`).

---

## 6. Три проверки безопасности (по факту кода)

| Пункт | Вердикт | Источник |
|-------|---------|----------|
| **JWT-подпись** | ✅ **реальный HMAC-HS256, НЕ MD5** (вопреки старым заметкам) | `ipfrs/src/auth.rs:461`, `ipfrs-interface/src/auth.rs:278` |
| **TLS-сертификаты** | ⚠️ **заглушка в node-крейте**: `SelfSignedCertGenerator::generate()` пишет фейковый PEM, rcgen лишь в комментарии. Но gateway-TLS через rustls — реальный | `ipfrs/src/tls.rs:314` vs `ipfrs-interface/src/tls.rs:49` |
| **Backpressure-семафор** | ✅ корректен: forget permits при сжатии окна (eager + deferred через RAII drop), покрыт тестами | `ipfrs-interface/src/backpressure.rs:185-266` |

Остаточные риски: дефолтный секрет `"default_secret_change_in_production"`
(`auth.rs:574`); **дублированная модель Auth** (два разных enum `Role`/`Permission` в
node и interface); in-memory хранилища пользователей; `min_age_seconds` GC не enforced;
indirect-пины не персистятся между рестартами. Подробно — [[11-RealityCheck]].

---

## Что дальше?

- **Сквозные сценарии работы** → [[10-DataFlows]]
- **Карта контекстов** → [[02-StrategicDesign]]
- **Полный реестр заглушек** → [[11-RealityCheck]]

**Связанные**: [[02-StrategicDesign]] | [[10-DataFlows]] | [[11-RealityCheck]]
**Источник кода**: `ipfrs/src/node/`, `ipfrs/src/{auth,tls,gc,pin,fsck}.rs`, `ipfrs-interface/src/{gateway/,grpc,graphql,websocket,auth,oauth2,backpressure}.rs`
