---
title: Features (Weeks 5+)
summary: High-impact feature proposals for v0.3.0 and beyond
tags: [features, roadmap, proposals]
---

# Features (Weeks 5+)

> Pick 1-2 of these approved features to help the original project reach v0.3.0.

---

## Feature Ranking

| Feature | Complexity | Time | Impact | Status |
|---------|-----------|------|--------|--------|
| WebRTC signaling server | Hard | 40h | ⭐⭐⭐⭐ | Proposed |
| S3-compatible REST gateway | Medium | 30h | ⭐⭐⭐ | Proposed |
| Prometheus exporter service | Medium | 20h | ⭐⭐⭐ | Proposed |
| SQLite semantic index alternative | Hard | 50h | ⭐⭐ | Research |
| Production deployment guide | Easy | 10h | ⭐⭐⭐⭐ | Doc-only |
| Benchmarking vs. IPFS | Medium | 15h | ⭐⭐⭐ | Proposed |
| Streaming GraphQL subscriptions | Medium | 35h | ⭐⭐ | Proposed |

---

## Feature #1: WebRTC Signaling Server ⭐⭐⭐⭐

**Impact:** Enable browser-based IPFRS clients (WASM → WebRTC → P2P network)  
**Complexity:** Hard (40h)  
**Prerequisite:** ipfrs-wasm already has WebRTC stub

### Overview

```
┌─────────────┐
│   Browser   │
│  (WASM node)│
└──────┬──────┘
       │ WebRTC
   ┌───▼────────┐
   │  Signaling │
   │   Server   │  ← Implement this
   └───┬────────┘
       │
   ┌───▼──────────────┐
   │  P2P Network     │
   │  (libp2p peers)  │
   └──────────────────┘
```

### Tasks

1. **Signaling server** (`signaling-server/` crate)
   - REST API for ICE candidate exchange
   - SDP offer/answer relay
   - Session management (WebSocket)
   
2. **Update WASM bindings**
   - Export `set_signaling_server_url()`
   - Implement `IceCandidate` collection
   - Export `WebRtcSignal` enum

3. **Docker container**
   - Pre-built image for easy deployment
   - Environment variables for configuration

4. **Documentation**
   - "Run browser node" tutorial
   - Security considerations (CORS, auth)

### Example Code (Signaling Server)

```rust
// signaling-server/src/main.rs
#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/signal/:session_id/offer", post(handle_offer))
        .route("/signal/:session_id/answer", post(handle_answer))
        .route("/signal/:session_id/ice", post(handle_ice_candidate))
        .with_state(AppState::default());
    
    axum::Server::bind(&"0.0.0.0:3001".parse().unwrap())
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .unwrap();
}
```

### Timeline

- Week 5-6: Signaling server REST API
- Week 7: WASM bindings update
- Week 8: Docker & docs
- Week 9: Testing & deployment

---

## Feature #2: S3-Compatible REST Gateway ⭐⭐⭐

**Impact:** Drop-in replacement for S3 (object storage); enterprise adoption  
**Complexity:** Medium (30h)  
**Tools:** `s3-server` crate or custom Axum handlers

### Overview

```
┌──────────────────┐
│  S3 Client       │  (boto3, AWS CLI, etc)
│  (any language)  │
└────────┬─────────┘
         │ HTTP PUT/GET/DELETE
    ┌────▼──────────────────┐
    │  IPFRS S3 Gateway      │
    │  (Axum server)         │
    └────┬──────────────────┘
         │
    ┌────▼──────────────────┐
    │  IPFRS Node            │
    │  (ipfrs core)          │
    └───────────────────────┘
```

### API Endpoints

```bash
# S3-compatible REST API
PUT /bucket/object → ipfrs add
GET /bucket/object → ipfrs get
DELETE /bucket/object → ipfrs delete (pin removal)
HEAD /bucket/object → check if exists
LIST /bucket → list blocks with metadata
```

### Tasks

1. **HTTP handlers** (Axum)
   - `PUT /bucket/key` → `ipfrs add` → return S3-format XML
   - `GET /bucket/key` → `ipfrs get` → stream bytes
   - `DELETE /bucket/key` → `ipfrs pin unpin` → remove
   - `LIST /bucket` → enumerate blocks

2. **Authentication**
   - AWS SigV4 signature verification (optional)
   - Simple bearer token for MVP
   
3. **Metadata storage**
   - IPFRS blocks → S3 object metadata (size, content-type, ETag)
   - Stored as IPLD DAG-CBOR

4. **Docker & docs**
   - Easy deployment
   - Compatibility matrix (tested with boto3, AWS CLI, s3cmd)

### Example Code

```rust
// ipfrs-interface/src/s3_gateway.rs
pub async fn handle_put(
    bucket: String,
    key: String,
    body: Bytes,
) -> Result<Response<String>, ApiError> {
    let cid = node.add_bytes(&body).await?;
    
    // Store metadata
    let metadata = S3Metadata {
        etag: format!("\"{:x}\"", cid.hash()),
        content_type: "application/octet-stream".to_string(),
        size: body.len() as u64,
    };
    
    Ok(Response::builder()
        .status(200)
        .header("ETag", &metadata.etag)
        .body(format!(r#"<PutObjectResponse>{}</PutObjectResponse>"#, cid))?)
}
```

### Timeline

- Week 5: Basic PUT/GET/DELETE handlers
- Week 6: Authentication & metadata
- Week 7: Compatibility testing
- Week 8: Docker & docs

---

## Feature #3: Prometheus Exporter Service ⭐⭐⭐

**Impact:** Enable enterprise monitoring (Prometheus + Grafana dashboards)  
**Complexity:** Medium (20h)  
**Prerequisite:** IpfrsMetrics already implemented

### Overview

IPFRS already collects metrics internally. Expose them as Prometheus-compatible endpoint.

```
┌─────────────┐
│   IPFRS     │
│   Metrics   │ (Arc<IpfrsMetrics>)
└────┬────────┘
     │
┌────▼──────────────────┐
│  Prometheus Exporter  │ ← Implement this
│  (HTTP endpoint)      │
└────┬──────────────────┘
     │
┌────▼──────────────────┐
│  Prometheus Server    │
│  (scrapes metrics)    │
└────┬──────────────────┘
     │
┌────▼──────────────────┐
│  Grafana Dashboard    │
│  (visualizes)         │
└───────────────────────┘
```

### Metrics to Export

```
# Storage
ipfrs_blocks_total{shard="0"}          # Total blocks stored
ipfrs_blocks_bytes{shard="0"}          # Total bytes stored
ipfrs_gc_runs_total                    # GC runs completed
ipfrs_gc_duration_seconds              # GC duration

# Network
ipfrs_peers_connected                  # Current peer count
ipfrs_dht_queries_total                # DHT queries
ipfrs_dht_query_latency_ms             # DHT latency

# Semantic
ipfrs_hnsw_vectors_indexed             # Vectors in index
ipfrs_semantic_searches_total          # Search queries
ipfrs_search_latency_ms                # Search latency

# TensorLogic
ipfrs_inferences_total                 # Inference calls
ipfrs_inference_cache_hits             # Cache hit rate

# HTTP
http_requests_total{method,path}       # HTTP requests
http_request_duration_seconds          # HTTP latency
```

### Tasks

1. **Metrics registry** (if not exists)
   - Counter, Gauge, Histogram for each metric
   - Update in real-time as IPFRS operates

2. **HTTP endpoint**
   - `GET /metrics` → Prometheus text format
   - Efficient encoding (no unnecessary allocation)

3. **Grafana dashboards**
   - Pre-built JSON dashboards
   - Show storage growth, network health, performance

4. **Documentation**
   - Prometheus scrape config example
   - Grafana import guide

### Example Code

```rust
// ipfrs-interface/src/metrics.rs
pub struct PrometheusExporter {
    metrics: Arc<IpfrsMetrics>,
}

impl PrometheusExporter {
    pub async fn handle_metrics(&self) -> String {
        let mut output = String::new();
        
        // Storage metrics
        output.push_str(&format!(
            "ipfrs_blocks_total {{}} {}\n",
            self.metrics.blocks_total.load(Ordering::Relaxed)
        ));
        
        output.push_str(&format!(
            "ipfrs_blocks_bytes {{}} {}\n",
            self.metrics.blocks_bytes.load(Ordering::Relaxed)
        ));
        
        // Network metrics
        output.push_str(&format!(
            "ipfrs_peers_connected {{}} {}\n",
            self.metrics.peers_connected.load(Ordering::Relaxed)
        ));
        
        output
    }
}
```

### Timeline

- Week 5: Metrics registry implementation
- Week 6: HTTP endpoint + export format
- Week 7: Grafana dashboards
- Week 8: Testing & docs

---

## Feature #4: Production Deployment Guide ⭐⭐⭐⭐

**Impact:** Enterprise adoption (documentation-only, high ROI)  
**Complexity:** Easy (10h)  
**Prerequisite:** None

### Content

```markdown
RoadMap/05-Deployment-Guide.md
├── 1. Pre-flight Checklist
│   ├── Hardware requirements (CPU, RAM, disk)
│   ├── Network (ports, firewall)
│   └── Security (TLS certs, auth)
│
├── 2. Docker Deployment
│   ├── Single-node setup
│   ├── docker-compose.yml with persistence
│   └── Health checks
│
├── 3. Kubernetes Deployment
│   ├── StatefulSet (maintains persistent storage)
│   ├── ConfigMap (configuration)
│   ├── Service (networking)
│   └── Examples (helm chart draft)
│
├── 4. Monitoring & Observability
│   ├── Prometheus scrape config
│   ├── Grafana dashboard import
│   ├── Alert rules
│   └── Log aggregation (ELK stack example)
│
├── 5. Backup & Recovery
│   ├── Sled snapshot strategy
│   ├── WAL archival
│   └── Restore procedures
│
├── 6. Performance Tuning
│   ├── Sled configuration (cache size, compaction)
│   ├── Network settings (peer limits, backpressure)
│   ├── Storage sharding
│   └── Benchmarking methodology
│
├── 7. Troubleshooting
│   ├── "Disk full" errors
│   ├── "Too many open files"
│   ├── "Peers not connecting"
│   └── "Memory leaks?"
│
└── 8. Security Hardening
    ├── Network isolation
    ├── TLS configuration
    ├── Access control (gRPC auth)
    └── Audit logging
```

### Timeline

- Week 5-6: Write deployment guide
- Week 7: Test with sample deployment
- Week 8: Publish & get community feedback

---

## Feature #5: Benchmarking vs. IPFS ⭐⭐⭐

**Impact:** Marketing + performance validation  
**Complexity:** Medium (15h)  
**Tools:** Criterion.rs, hyperfine

### Benchmarks

```
┌──────────────────────────────────────────┐
│  Benchmark: ADD 1GB file                 │
├──────────────────────────────────────────┤
│  IPFS:  12.3s                            │
│  IPFRS: 8.1s  ⭐ 33% faster              │
│                                          │
│  Benchmark: GET (cached, 1000 blocks)    │
├──────────────────────────────────────────┤
│  IPFS:  2.5ms/block                      │
│  IPFRS: 0.03ms/block ⭐ 80x faster       │
└──────────────────────────────────────────┘
```

### Tasks

1. **Setup both systems**
   - Install IPFS daemon
   - Install IPFRS daemon
   - Standardize hardware & conditions

2. **Benchmark suite** (Criterion.rs)
   - ADD: 100MB, 1GB, 10GB files
   - GET: local cache, network fetch
   - SEARCH: 1000 vectors, 100K vectors
   - QUERY: simple rules, complex reasoning

3. **Report**
   - Graph comparison
   - Analysis of differences (why IPFRS is faster)
   - Caveats & fairness notes

### Timeline

- Week 5: Benchmark infrastructure setup
- Week 6: Run benchmarks, collect data
- Week 7: Analysis & visualization
- Week 8: Publish report

---

## Decision: Which to Pick?

**Recommendation:** Start with **Feature #4** (Deployment Guide) **+ Feature #2** (S3 Gateway)

**Rationale:**
- Feature #4 is low-effort, high-impact (enables real-world use)
- Feature #2 is enterprise-friendly, differentiates from IPFS
- Together: ~40 hours of work (4 weeks)
- Both help original project reach v0.3.0

**Alternative:** If you prefer ML, choose Feature #1 (WebRTC) for browser-based inference.

---

## Contributing Features Upstream

Once a feature is complete:

1. Create PR in original repository (cool-japan/ipfrs)
2. Link to your implementation as reference
3. Work with maintainers to integrate
4. Both repos benefit (you get credit, they get feature)

See [05-Upstream-Contribution.md](05-Upstream-Contribution.md) for details.

---

**Next:** [05-Upstream-Contribution.md](05-Upstream-Contribution.md) — How to contribute features & fixes upstream.
