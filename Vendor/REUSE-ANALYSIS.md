# Vendor Reuse Analysis — DDD lens

> Companion to [`ARCHITECTURE.md`](ARCHITECTURE.md). That file maps the vendored contexts;
> **this file decides what we take from them and *by which DDD integration pattern*.** Reuse is
> never "copy a file" — it's a choice of relationship with an upstream Bounded Context.

**Produced:** 2026-06-21, from a 5-way parallel source read (one explorer per context).
**Host:** IPFRS (`ipfrs_source/`), Core differentiators = geo-distributed inference + proof-carrying
responses; Generic substrate = tensor numerics, storage, network.

> ⚠️ **Verify before depending.** Findings come from excerpt-level reads, not a compile. Treat all
> file paths / line numbers / type names below as *leads to confirm*, not facts. Heavy upstreams
> (esp. trustformers → tch/candle/onnx) must be weighed against our lean P2P crate graph and the
> known `network ↔ tensorlogic` dependency-cycle risk. Anything touching **SciRS2 is a Shared
> Kernel** decision — not a unilateral bump.

---

## 0. Decisions taken

- **Tensor substrate (Phase 5):** keep our own lightweight `NumTensor`, **hidden behind a backend
  trait** (Conformist on `TlExecutor`'s *interface*, not its impl). No heavy supplier dependency,
  no SciRS2 inheritance, no cycle risk. A real engine can be swapped in behind the trait later.
- **Op set:** grow it by **porting pure-`f32` kernels** (ACL), not by adopting a tensor crate.

---

## 1. Integration patterns (the DDD legend)

| Pattern | Meaning here | Cost / guardrail |
|---------|--------------|------------------|
| **Conformist** | adopt upstream's *model/interface*, speak its language | cheap; we don't control its evolution |
| **ACL port** | copy the *algorithm*, translate types into our Ubiquitous Language | upstream types must not leak past the edge |
| **Supplier dependency** | `depend on` the crate directly | inherits its dep graph; only for Generic subdomains |
| **Shared Kernel** | jointly-governed dep (SciRS2) | change = multi-context decision |
| **Published Language** | enrich our `model_manifest` (DAG-CBOR) with upstream conventions | keep it ours, versioned by CID |

**Strategic rule:** *we own Core, we buy Generic.* Port Core mechanics carefully through an ACL;
depend on suppliers only for replaceable substrate.

---

## 2. Reuse map — by host subdomain

### 🔴 Core — distributed graph execution (Phase 5)  ·  *highest leverage*
ACL ports, low dependency weight; these are the actual unblock.

| Take | From | File (verify) | Pattern | Rel / Effort |
|------|------|---------------|---------|--------------|
| `PartitioningStrategy` + `PartitionedGraph` with **per-stage comm schedule** | torsh | `torsh-fx/src/graph_partitioning.rs` | ACL port | HIGH / Low-Med |
| `ShardInfo` (peer owns which slice) → add to `NumTensor` | torsh | `torsh-distributed/src/tensor_parallel.rs` | ACL port | HIGH / Low |
| `CollectiveOp{AllReduce,AllGather,ReduceScatter,Barrier}` over libp2p | torsh | `torsh-distributed/src/collectives.rs` | ACL port | HIGH / Med |
| einsum contraction analysis + `placement`/`scheduling`/`partitioned` | tensorlogic | `tensorlogic-infer/src/{join_order,placement,scheduling,partitioned}` | ACL port | HIGH / Med |
| pipeline/tensor-parallel as reference ("layers across peers") | trustformers | `trustformers-core/src/parallel/{tensor,pipeline}_parallel.rs` | study only | Med-High / High |

### 🟠 Generic — numeric engine  ·  *decision locked: own-it-behind-trait*

| Take | From | File (verify) | Pattern | Rel / Effort |
|------|------|---------------|---------|--------------|
| `TlExecutor` trait + `ElemOp`/`ReduceOp` enums → wrap `NumTensor` | tensorlogic | `tensorlogic-infer/src/{traits.rs,ops.rs}` | **Conformist (interface)** | HIGH / Low |
| pure-`f32` kernels: `softmax`(log-sum-exp), `layer_norm`, `rms_norm`, `gelu`, `silu`, subnormal-detect | oxigaf | `oxigaf-diffusion/src/numerics.rs` | ACL port | HIGH / Low |
| `EinsumGraph`/`OpType` + graph `validation` as IR reference | tensorlogic | `tensorlogic-ir/src/graph/{node,optype,validation}.rs` | Conformist (optional) | Med / Low-Med |
| _full `Tensor<T>` / autograd / mmap_ | torsh-core / trustformers-core | — | **Supplier — deferred** (SciRS2 + heavy deps) | — |

### 🟠 Core-ish — federated / distributed training

| Take | From | File (verify) | Pattern | Rel / Effort |
|------|------|---------------|---------|--------------|
| `Optimizer`/`Loss` traits → FedAvg implements `Optimizer` | tensorlogic | `tensorlogic-train/src/{optimizers,loss}` | Conformist | Med / Low |
| Byzantine-robust `AggregationStrategy{Krum,Median}` + client selection | torsh | `torsh-autograd/src/federated_learning/` | ACL port | Med / Med |
| gradient accumulation + mixed-precision loss scaling + flow tracking | oxigaf | `oxigaf-trainer/src/{gradient_accumulation,mixed_precision,gradient_flow}.rs` | ACL port | HIGH / Low-Med |

### 🟢 Supporting — semantic search / retrieval (we already have HNSW)  ·  *quick wins*

> ✅ **Verified 2026-06-21 — SIMD and RRF already exist in the host; do NOT port from oxirag.**
> `ipfrs-semantic::simd` already has `cosine_distance`/`l2_distance`/`dot_product` with runtime
> AVX2/AVX/SSE/NEON detection + scalar fallback. `ipfrs-semantic::result_aggregator` already has
> `ResultAggregator` + `AggregationStrategy::RankFusion` + `aggregate_rrf()` (RRF, k=60 default).
> The real gap was that `semantic_search_distributed` did a naive best-score CID merge instead of
> using them — **now fixed by wiring the existing `ResultAggregator` (RRF) into the distributed
> fan-out** (`ipfrs/src/node/semantic_ops.rs`), each peer + local index as its own ranked source.

| Take | From | File (verify) | Pattern | Rel / Effort |
|------|------|---------------|---------|--------------|
| ~~`SimilarityEngine` (auto AVX2/NEON cosine)~~ | oxirag | `simd_similarity.rs` | ❌ **redundant** — host already has `ipfrs-semantic::simd` | DONE (pre-existing) |
| ~~**RRF fusion** for merging peer results~~ | oxirag | `hybrid_search.rs` | ✅ **DONE** — wired existing `ipfrs-semantic::result_aggregator` into `semantic_search_distributed` | DONE 2026-06-21 |
| reranker pipeline + query expansion + relevance feedback | oxirag | `reranker.rs`, `query_expansion.rs`, `relevance_feedback.rs` | ACL port | Med / Low-Med |
| KG overlay over CID blocks | oxirag | `layer4_graph/` | ACL port | Med / Med |

### 🔴 Core — proof-carrying inference (we already emit `proof_json`)

| Take | From | File (verify) | Pattern | Rel / Effort |
|------|------|---------------|---------|--------------|
| `ClaimExtractor` (NL → predicates) + `SmtVerifier` (SMT-LIB2, peer-checkable) | oxirag | `layer3_judge/` | ACL port + supplier (SMT) | HIGH / High |

### ⚪ Cross-cutting — resilience of distributed peer queries

> ✅ **Verified 2026-06-21 — circuit breaker already exists in the host; do NOT port from oxirag.**
> `ipfrs-network::circuit_breaker` already has `CircuitBreakerRegistry` + `PeerCircuitBreaker`
> (Closed/Open/HalfOpen, sliding window, slow-call detection). It just wasn't wired into the
> semantic fan-out. **Now fixed:** `Node` holds a `Mutex<CircuitBreakerRegistry>` and
> `semantic_search_distributed` guards each peer query (`can_call` → skip Open peers;
> `record_result` Success/Failure/Timeout).

| Take | From | File (verify) | Pattern | Rel / Effort |
|------|------|---------------|---------|--------------|
| ~~circuit breaker (Closed/Open/HalfOpen per peer)~~ | oxirag | `circuit_breaker.rs` | ✅ **DONE** — wired existing `ipfrs-network::circuit_breaker` into peer fan-out | DONE 2026-06-21 |
| retry+backoff+jitter + libp2p conn pool | oxirag | `retry.rs`, `connection_pool.rs` | ACL port (host has `ipfrs-storage::retry` — check reuse first) | Med / Low |

### ⚫ oxigaf domain — **not reusable**
~80% is FLAME / Gaussian-splat rendering / diffusion-domain logic. Take only the infra kernels and
training patches listed above; ignore `oxigaf-{render,flame}` and the avatar trainer orchestration.

---

## 3. Serving (Supporting, when we run real transformer inference)

From **trustformers** (only if/when we move past `serve_inference` stub) — adopt patterns, avoid the
heavy crate: dynamic batching (`trustformers-serve/src/batching/`), KVCache + eviction +
prefix-cache (`trustformers-serve/src/{kv_cache,prefix_cache}/`), load balancer + health checks.
**Published Language angle:** bridge `model_manifest` CID-addressed layers → a `Model`-style loader
so weights load *from content-addressed blocks* instead of HF Hub.

---

## 4. Recommended sequence (by leverage)

1. ✅ **Spike 1 — retrieval quick wins** — **DONE 2026-06-21.** Outcome differed from the plan:
   SIMD, RRF and the per-peer circuit breaker *already existed* in the host (`ipfrs-semantic::simd`,
   `ipfrs-semantic::result_aggregator`, `ipfrs-network::circuit_breaker`) — nothing was ported from
   oxirag. The win was **wiring**: `semantic_search_distributed` now RRF-fuses local + per-peer
   ranked lists via `ResultAggregator` and guards each peer with a persistent
   `CircuitBreakerRegistry` (skip Open peers, record Success/Failure/Timeout). Lesson: the
   "verify before depending" guardrail paid off — the analysis's excerpt-based "port from oxirag"
   leads were redundant with mature host code. Remaining optional retrieval items: reranker /
   query-expansion / KG overlay (genuinely absent → still candidate ACL ports).
2. **Spike 2 — unblock Phase 5**: ACL-port torsh partitioning + `ShardInfo` + collectives →
   `graph_partitioner` gains a communication schedule (= partition graph + stream activations).
3. **Spike 3 — Conformist engine**: wrap `NumTensor` in `TlExecutor`; add missing ops from
   `oxigaf/numerics.rs` (softmax / layernorm / gelu / silu).
4. **Mid-term**: SMT judge → proof-carrying; Krum aggregation → FedAvg; gradient flow tracking.

**Guardrail (from `ARCHITECTURE.md` §4):** keep upstream types behind our ACL
(`ipfrs_source/crates/ipfrs-tensorlogic` and the network edge) — they must not leak into Core
domain types.
