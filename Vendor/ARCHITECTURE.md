# Vendor Layer — Domain-Driven Architecture

> **Role of this folder.** `Vendor/` is the host project's **integration boundary** with the
> external [cool-japan](https://github.com/cool-japan) ecosystem. In DDD terms each sub-folder is
> an **upstream Bounded Context** with its own Ubiquitous Language, lifecycle and release cadence.
> The host system (IPFRS / cool-japan) never reaches into their internals directly — it consumes
> them through **published interfaces** behind an **Anti-Corruption Layer**. This file is the
> **Context Map** that keeps those boundaries explicit.

**Last refreshed:** 2026-06-21 — shallow `git clone --depth 1` of every upstream `main`.

---

## 1. Strategic design — subdomain classification

DDD separates *what is differentiating* (Core) from *what is necessary but generic* (Supporting /
Generic). The vendored contexts fall out as follows:

| Bounded Context | Subdomain type | Responsibility (its Ubiquitous Language) | Upstream layer (cloned) |
|-----------------|----------------|------------------------------------------|-------------------------|
| **torsh**        | **Generic** (compute substrate) | Tensors, autograd, `nn`, optim — a PyTorch-compatible DL framework. *Tensor, Module, Graph, Shard.* | `d6657ca` · 2026-04-27 · 70 MB |
| **trustformers** | **Core**       | Pure-Rust transformer stack — 49+ architectures, tokenizers, training, serving. *Model, Tokenizer, KVCache, Attention.* | `b815f2d` · 2026-06-21 · 94 MB |
| **tensorlogic**  | **Core**       | Neural-symbolic *Logic-as-Tensor* planning layer + IR/compiler/infer. *Rule, Predicate, Plan, IR, Backend.* | `0800acf` · 2026-06-09 · 23 MB |
| **oxirag**       | **Supporting** | Four-layer RAG with SMT verification + knowledge graphs. *Query, Draft, Judge, Retriever, Graph.* | `4a6b504` · 2026-02-07 · 3 MB |
| **oxigaf**       | **Supporting** | Gaussian Avatar reconstruction from monocular video via multi-view diffusion. *Avatar, Gaussian, FLAME, Diffusion.* | `2f885d3` · 2026-06-19 · 19 MB |
| _ipfrs_         | _Generic_      | Content-addressed P2P filesystem (host's own substrate; mirror kept empty — see `ipfrs_source/`). | reference |
| _go-ethereum_   | _Generic_      | Ledger / blockchain client. | reference |

**Shared Kernel — SciRS2.** `torsh`, `tensorlogic` and `trustformers` each ship a
`SCIRS2_INTEGRATION_POLICY.md`: they share the SciRS2 scientific-computing substrate as a
deliberately governed **Shared Kernel**. Changes to that kernel are a cross-context concern and
must not be made unilaterally inside `Vendor/`.

---

## 2. Context map

Arrows point **downstream** (D = downstream / consumer, U = upstream / supplier). Integration
patterns are named per the DDD catalogue.

```
                          ┌──────────────────────────────────────────┐
                          │            SHARED KERNEL  ·  SciRS2        │
                          │   (scientific computing: linalg, rng, …)   │
                          └───────▲───────────────▲──────────────▲─────┘
                                  │ Shared Kernel │              │
                  ┌───────────────┴───┐   ┌───────┴────────┐  ┌──┴───────────────┐
   Generic ▶      │      torsh         │   │  trustformers   │  │   tensorlogic     │  ◀ Core
  (substrate)     │  tensor · autograd │   │ transformers ·  │  │ logic-as-tensor · │
                  │  nn · optim · dist │   │ tokenizers·serve│  │ IR · compiler·infer│
                  └─────────▲──────────┘   └───────▲─────────┘  └──┬─────────┬──────┘
                            │ U                  U │  Customer/      │ ACL     │ ACL
                            │                      │  Supplier       │ bridge  │ bridge
                            │            ┌─────────┘   (tensorlogic- │         │
                            │            │             trustformers) ▼         ▼
                  ┌─────────┴──────┐  ┌──┴───────────┐         (oxirs-bridge)  (adapters,
   Supporting ▶   │     oxigaf      │  │    oxirag     │                         scirs/oxicuda
  (generative)    │ diffusion·flame │  │ echo·spec·    │  ◀ Supporting           backends)
                  │ render·trainer  │  │ judge·graph   │   (retrieval)
                  └─────────────────┘  └──────▲────────┘
                       │ Conformist            │ Conformist
                       │ (tensor backend)      │ (transformer models / SLM drafts)
                       └───────────────────────┘
```

### Relationships, named

| Pattern | Contexts | Where it lives |
|---------|----------|----------------|
| **Shared Kernel** | SciRS2 ⇄ {torsh, trustformers, tensorlogic} | `SCIRS2_INTEGRATION_POLICY.md` in each |
| **Customer / Supplier** | tensorlogic (D) → trustformers (U) | `tensorlogic/crates/tensorlogic-trustformers` |
| **Anti-Corruption Layer** | tensorlogic → external symbolic/quantum/ML | `tensorlogic-{adapters, oxirs-bridge, quantrs-hooks, sklears-kernels}` |
| **Open Host / pluggable backends** | tensorlogic core → compute backends | `tensorlogic-{scirs,oxicuda}-backend` |
| **Conformist** | oxirag (D) → transformer model formats (U) | `oxirag/src/layer2_speculator`, `reranker` |
| **Conformist** | oxigaf (D) → tensor backend (U) | `oxigaf/crates/oxigaf-bridge` |
| **Published Language** | shared model/plan interchange | `tensorlogic/crates/tensorlogic-ir`, model manifests |

---

## 3. Internal structure of each context (aggregates)

Each upstream is itself a Cargo workspace whose member crates read as **aggregates / modules** of
one bounded context — the boundary is the workspace, not the individual crate.

- **torsh** — `torsh-{core,tensor,autograd,nn,optim,functional,linalg,sparse,special}` (numeric
  core) + `torsh-{distributed,cluster,jit,fx,graph,quantization}` (scale/compile) +
  `torsh-{vision,text,series,signal,data,models,hub}` (domain modules) + `torsh-{ffi,python,cli}`
  (ports). 31 crates.
- **trustformers** — `trustformers-core` (tensor/runtime) · `-tokenizers` · `-models` · `-optim` ·
  `-training` · plus delivery contexts `-serve` (REST/gRPC/GraphQL), `-mobile`, `-wasm`, `-py`,
  `-c`, `-js`. The packaging crates are **delivery adapters**, not new domains.
- **tensorlogic** — `tensorlogic-ir` (Published Language) → `-compiler` → `-infer` / `-train`;
  backends `-scirs-backend`, `-oxicuda-{backend,rng,solver,sparse}`; ACL bridges `-adapters`,
  `-oxirs-bridge`, `-trustformers`, `-quantrs-hooks`, `-sklears-kernels`; ports `-cli`, `-py`.
- **oxirag** — single crate; modules form a pipeline aggregate: `layer1_echo` → `layer2_speculator`
  → `layer3_judge` → `layer4_graph`, supported by `hybrid_search`, `reranker`, `prefix_cache`,
  `distillation`, `hidden_states`.
- **oxigaf** — `oxigaf-{diffusion, flame, render, trainer}` (domain) + `oxigaf-bridge` (ACL) +
  `oxigaf-cli` (port).

---

## 4. Integration philosophy (rules for the host)

1. **Depend on the boundary, not the guts.** Consume each context through its top-level façade
   crate / published IR — never reach into a sibling member crate of an upstream workspace.
2. **Translate at the edge.** Host-side code (e.g. `ipfrs_source/crates/ipfrs-tensorlogic`) is the
   **Anti-Corruption Layer** that maps upstream types into the host's Ubiquitous Language. Keep
   upstream types from leaking past it.
3. **Pin the layer, refresh deliberately.** These are `--depth 1` snapshots. Record the commit
   (table §1) and re-clone as a conscious act, not implicitly — an upstream language drift is a
   context-map change, not a silent bump.
4. **Respect the Shared Kernel.** Anything touching SciRS2 semantics is a multi-context decision.
5. **Generic stays replaceable.** `torsh` / `ipfrs` / `go-ethereum` are Generic subdomains —
   isolate them so they can be swapped without disturbing the Core (`trustformers`, `tensorlogic`).

---

## 5. Provenance

All five contexts cloned shallow (`git clone --depth 1`) from `github.com/cool-japan/<name>` on
2026-06-21. Per-context commit hash and upstream date are in the table in §1. `ipfrs` and
`go-ethereum` remain placeholder mirrors (the live IPFRS tree is the host repo under
`ipfrs_source/`).
