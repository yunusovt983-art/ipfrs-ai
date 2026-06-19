# Logic Context — IR, Inference, Neural-Symbolic, Tensor Execution

> **Focus**: Symbolic reasoning, neural-symbolic fusion, tensor computation, distributed inference  
> **Source**: `ipfrs_source/crates/ipfrs-tensorlogic/src/` (194 files, ~129,000 LOC)  
> **Analysis**: Deep dive using Opus 4.8 model

---

## 1. Context Overview

Logic Context — самый концептуально сложный bounded context в IPFRS. Объединяет **symbolic reasoning**, **tensor computation**, и **neural-symbolic integration** через content-addressed архитектуру.

```
┌─────────────────────────────────────────────────────────────────────┐
│                    LOGIC CONTEXT (ipfrs-tensorlogic)                │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    IR (INTERMEDIATE REPRESENTATION)          │   │
│  │  ─────────────────────────────────────────────────────────── │   │
│  │  Term(Var|Const|Fun|Ref)  →  Predicate  →  Rule  →  KB       │   │
│  │        (value objects)         (VO)       (VO)    (AR)       │   │
│  │                                                              │   │
│  │  KEY FILES:                                                  │   │
│  │  • ir.rs           — Term, Predicate, Rule, Fact             │   │
│  │  • term_index.rs   — HashCons<Term>                          │   │
│  │  • rule_index.rs   — HashCons<Rule>                          │   │
│  │  • rule_dependency.rs — DAG dependency analysis              │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    TENSOR MEMORY MANAGEMENT                  │   │
│  │  ─────────────────────────────────────────────────────────── │   │
│  │  TensorArena      — Bump allocator (O(1) allocation)         │   │
│  │  TensorPool       — Slab-based buffer pool (8 buckets)       │   │
│  │  TensorGC         — Mark-and-sweep garbage collector         │   │
│  │  TensorQuantizer  — INT8/INT4/FP16/BF16 compression          │   │
│  │  TensorDiffEngine — Federated learning change detection      │   │
│  │  TensorChecksumEngine — Corruption detection (4 algos)       │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    TENSOR EXECUTION                          │   │
│  │  ─────────────────────────────────────────────────────────── │   │
│  │  ComputationGraph — DAG with 30+ TensorOp types              │   │
│  │  AutogradGraph    — Reverse-mode automatic differentiation   │   │
│  │  OpFusion         — Greedy pattern matching                  │   │
│  │  OpDispatcher     — Backend routing (CPU/GPU/Remote)         │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    INFERENCE ENGINES (25 engines)            │   │
│  │  ─────────────────────────────────────────────────────────── │   │
│  │  InferenceEngine         — SLD Resolution                    │   │
│  │  TabledInferenceEngine   — SLG Tabling (recursion-safe)      │   │
│  │  TemporalReasoningEngine — Allen's 13 interval relations     │   │
│  │  FuzzyLogicEngine        — Mamdani/Sugeno inference          │   │
│  │  EpistemicLogicReasoner  — S5 Kripke semantics               │   │
│  │  ProbabilisticLogicNetwork — PLN uncertain reasoning         │   │
│  │  BayesianNetworkInference — VE/BP/Gibbs sampling             │   │
│  │  NeuralSymbolicIntegrator — Hybrid neural-symbolic           │   │
│  │  DistributedBackwardChainer — Cross-peer reasoning           │   │
│  │  + 16 more: Memoized SLD, Fixpoint/Datalog, full Mamdani,    │   │
│  │    Bayesian Updater, Abductive, Causal (do-calculus), CSP,   │   │
│  │    Constraint Propagation, Belief Revision (AGM), MDP, 2× RL,│   │
│  │    Hypothesis Testing, Prob. Program, Decision Tree, Ensemble│   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    GRADIENT SYSTEM                           │   │
│  │  ─────────────────────────────────────────────────────────── │   │
│  │  SparseGradient    — CSR format for sparse updates           │   │
│  │  QuantizedGradient — Compressed gradient transmission        │   │
│  │  GradientDelta     — Delta encoding for bandwidth reduction  │   │
│  │  DifferentialPrivacy — Gaussian/Laplace noise mechanisms     │   │
│  │  SecureAggregation  — Cryptographic gradient aggregation     │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    PROOF SYSTEM                              │   │
│  │  ─────────────────────────────────────────────────────────── │   │
│  │  ProofNode        — Single proof step                        │   │
│  │  ProofTree        — Sound, acyclic proof structure           │   │
│  │  ProofCache       — LRU cache for proven goals               │   │
│  │  Provenance       — Source attribution for facts             │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 2. Tensor Memory Management

### 2.1 TensorArena — Bump Allocator

**Файл**: `tensor_arena.rs` (~300 LOC)

Bump allocator для inference pipelines. Выделяет память за O(1) из pre-allocated slabs.

```rust
pub struct TensorArena {
    pub regions: Vec<ArenaRegion>,    // Fixed-size slabs
    pub region_size: usize,           // Default: 1 MiB
    pub stats: ArenaStats,
}

pub struct ArenaRegion {
    slab: Vec<u8>,       // Pre-allocated memory
    offset: usize,       // Bump pointer
    capacity: usize,
}

pub struct ArenaSlice {
    pub region_index: usize,
    pub start: usize,
    pub end: usize,
}
```

**Операции**:
- `allocate(size)` → O(1) — bump pointer increment
- `reset_all()` → O(regions) — вернуть всю память
- `write_f32(slice, values)` → записать f32 массив
- `read_f32(slice)` → прочитать f32 массив

**Alignment**: 8-byte для всех allocations (ARENA_ALIGN = 8)

**Статистика**:
```rust
pub struct ArenaStats {
    pub total_allocations: u64,
    pub total_bytes_allocated: u64,
    pub total_resets: u64,
    pub regions_created: u64,
}
```

**Инварианты**:
- Все allocations выровнены по 8 байт
- `ArenaSlice` становится invalid после `reset_all()`
- Нет individual free — только bulk reset

---

### 2.2 TensorPool — Slab-based Buffer Pool

**Файл**: `tensor_pool.rs` (~450 LOC)

Thread-safe buffer pool с 8 buckets для power-of-two size classes.

```rust
pub struct TensorPool {
    free_lists: [Mutex<Vec<Vec<u8>>>; NUM_BUCKETS],  // 8 buckets
    stats: TensorPoolStats,
    config: TensorPoolConfig,
}

pub const NUM_BUCKETS: usize = 8;
const BUCKET_MIN_SIZE: usize = 256;  // Bucket 0 capacity
```

**Size Classes**:

| Bucket | Min Size | Max Size | Capacity |
|--------|----------|----------|----------|
| 0 | 0 B | 255 B | 256 B |
| 1 | 256 B | 511 B | 512 B |
| 2 | 512 B | 1023 B | 1 KiB |
| 3 | 1 KiB | 2047 B | 2 KiB |
| 4 | 2 KiB | 4095 B | 4 KiB |
| 5 | 4 KiB | 8191 B | 8 KiB |
| 6 | 8 KiB | 16383 B | 16 KiB |
| 7 | 16 KiB | ∞ | exact size (cap 32 MiB) |

**API**:
```rust
pub fn acquire(&self, min_bytes: usize) -> PooledBuffer;
pub fn release(&self, buf: PooledBuffer);
pub fn pool_depth(&self, bucket: usize) -> usize;
pub fn prune(&self, max_per_bucket: usize);
pub fn stats(&self) -> TensorPoolSnapshot;
```

**Thread Safety**: Каждый bucket под отдельным `Mutex` — минимальный contention.

**Статистика**:
```rust
pub struct TensorPoolSnapshot {
    pub total_acquired: u64,
    pub total_released: u64,
    pub total_allocs: u64,    // Fresh allocations
    pub total_reuses: u64,    // Pool hits
    pub total_bytes_pooled: u64,
}
```

---

### 2.3 TensorGC — Mark-and-Sweep Garbage Collector

**Файл**: `tensor_gc.rs` (~400 LOC)

Garbage collector для unreachable tensor allocations с dependency-aware reachability.

```rust
pub struct TensorGarbageCollector {
    pub tensors: HashMap<u64, TensorRef>,
    pub roots: Vec<u64>,
}

pub struct TensorRef {
    pub tensor_id: u64,
    pub name: Option<String>,
    pub size_bytes: u64,
    pub ref_count: u32,          // External references
    pub dependencies: Vec<u64>,  // Outgoing edges
    pub pinned: bool,            // Never collect
}

pub enum GcPhase {
    MarkRoots,  // Seed from roots + pinned
    Trace,      // BFS through dependency edges
    Sweep,      // Remove unreachable with ref_count=0
}
```

**Algorithm**:
1. **MarkRoots**: Seed reachable set from `roots` + `pinned` tensors
2. **Trace**: BFS expansion through `dependencies` edges
3. **Sweep**: Remove tensors with `!visited && ref_count == 0`

**Инварианты**:
- `pinned == true` → никогда не собирается
- `ref_count > 0` → никогда не собирается
- BFS max depth: не ограничено (граф должен быть acyclic)

**API**:
```rust
pub fn register(&mut self, tensor: TensorRef);
pub fn add_root(&mut self, tensor_id: u64);
pub fn remove_root(&mut self, tensor_id: u64);
pub fn pin(&mut self, tensor_id: u64);
pub fn add_ref(&mut self, tensor_id: u64);
pub fn remove_ref(&mut self, tensor_id: u64);
pub fn reachable_set(&self) -> Vec<u64>;
pub fn collect(&mut self) -> GcStats;
```

**Статистика**:
```rust
pub struct GcStats {
    pub total_tensors: usize,
    pub reachable: usize,
    pub collected: usize,
    pub freed_bytes: u64,
    pub pinned_tensors: usize,
}
```

---

### 2.4 TensorQuantizer — Multi-Precision Compression

**Файл**: `tensor_quantizer.rs` (~1250 LOC)

Production-grade quantization для INT8, INT4, FP16, BF16 с per-channel support.

```rust
pub enum QuantizationMode {
    Int8Symmetric,   // scale = percentile(|x|) / 127
    Int8Asymmetric,  // scale + zero_point, range [0, 255]
    Int4,            // scale = percentile(|x|) / 7
    Fp16,            // 5-bit exp, 10-bit mantissa
    Bf16,            // Top 16 bits of f32
}

pub struct QuantizerConfig {
    pub mode: QuantizationMode,
    pub per_channel: bool,           // Per-channel scale/zp
    pub channel_dim: usize,          // Axis for channels
    pub calibration_percentile: f64,  // e.g. 99.9 (outlier suppression)
}

pub struct QuantizedTensor {
    pub mode: QuantizationMode,
    pub data: Vec<i32>,             // Quantized data
    pub scale: f64,
    pub zero_point: i32,
    pub original_dims: Vec<usize>,
    pub original_min: f64,
    pub original_max: f64,
    pub channel_scales: Vec<f64>,           // Per-channel
    pub channel_zero_points: Vec<i32>,      // Per-channel
}
```

**Compression Ratios**:
- INT8: 64/8 = **8×**
- INT4: 64/4 = **16×**
- FP16/BF16: 64/16 = **4×**

**API**:
```rust
pub fn quantize(&mut self, values: &[f64], dims: &[usize]) -> Result<QuantizedTensor>;
pub fn dequantize(&self, qt: &QuantizedTensor) -> Result<DequantizedTensor>;
pub fn quantization_error(&self, original: &[f64], qt: &QuantizedTensor) -> Result<f64>;
pub fn compression_ratio(original_len: usize, mode: &QuantizationMode) -> f64;
pub fn clamp_to_range(x: f64, mode: &QuantizationMode) -> f64;
```

**Calibration**: Percentile-based outlier suppression (default: 99.9%)

---

### 2.5 TensorDiffEngine — Federated Learning Diff

**Файл**: `tensor_diff.rs` (~350 LOC)

Change detection для federated learning checkpoints.

```rust
pub enum DiffKind {
    Added,
    Removed,
    ShapeChanged { old_shape: Vec<usize>, new_shape: Vec<usize> },
    ValueChanged { max_abs_diff: f32, mean_abs_diff: f32, changed_elements: usize },
    Unchanged,
}

pub struct TensorSnapshot {
    pub name: String,
    pub shape: Vec<usize>,
    pub data: Vec<f32>,
}

pub struct TensorDiff {
    pub name: String,
    pub kind: DiffKind,
}

pub struct TensorDiffEngine {
    pub threshold: f32,  // Value difference threshold
}
```

**API**:
```rust
pub fn diff_tensors(&self, old: &TensorSnapshot, new: &TensorSnapshot) -> TensorDiff;
pub fn diff_snapshots(&self, old_set: &[TensorSnapshot], new_set: &[TensorSnapshot]) -> Vec<TensorDiff>;
pub fn summarize(&self, diffs: &[TensorDiff]) -> DiffSummary;
pub fn significant_diffs<'a>(&self, diffs: &'a [TensorDiff]) -> Vec<&'a TensorDiff>;
```

**DiffSummary**:
```rust
pub struct DiffSummary {
    pub added: usize,
    pub removed: usize,
    pub shape_changed: usize,
    pub value_changed: usize,
    pub unchanged: usize,
    pub total_changed_elements: usize,
}
```

---

### 2.6 TensorChecksumEngine — Corruption Detection

**Файл**: `tensor_checksum.rs` (~400 LOC)

Checksum computation и verification для tensor data integrity.

```rust
pub enum ChecksumAlgorithm {
    Fnv1a64,     // Fast non-cryptographic (64-bit)
    Adler32,     // zlib-compatible (32-bit)
    Fletcher16,  // Lightweight (16-bit)
    XorFold,     // Ultra-fast for large tensors (64-bit)
}

pub struct TensorChecksum {
    pub algorithm: ChecksumAlgorithm,
    pub value: u64,
    pub data_len: usize,
    pub computed_at_secs: u64,
}

pub struct ChecksumRecord {
    pub tensor_id: u64,
    pub checksum: TensorChecksum,
    pub layer_name: String,
}

pub struct TensorChecksumEngine {
    pub records: HashMap<u64, ChecksumRecord>,
    pub stats: ChecksumEngineStats,
}
```

**Pure-Rust Implementations**:
```rust
pub fn fnv1a64(data: &[u8]) -> u64;     // FNV-1a 64-bit
pub fn adler32(data: &[u8]) -> u64;     // Adler-32
pub fn fletcher16(data: &[u8]) -> u64;  // Fletcher-16
pub fn xor_fold(data: &[u8]) -> u64;    // XOR-fold
```

**API**:
```rust
pub fn compute(&mut self, tensor_id: u64, layer_name: String, data: &[u8], algorithm: ChecksumAlgorithm, now_secs: u64) -> &ChecksumRecord;
pub fn verify(&mut self, tensor_id: u64, data: &[u8]) -> Option<bool>;
pub fn remove(&mut self, tensor_id: u64) -> bool;
```

**Статистика**:
```rust
pub struct ChecksumEngineStats {
    pub total_computed: u64,
    pub total_verified: u64,
    pub total_failures: u64,
}
```

---

## 3. Tensor Execution

### 3.1 ComputationGraph — DAG Execution

**Файл**: `computation_graph.rs` (~1723 LOC)

Directed Acyclic Graph для tensor operations с optimization passes.

```rust
pub struct ComputationGraph {
    nodes: HashMap<NodeId, ComputationNode>,
    outputs: Vec<NodeId>,
}

pub struct ComputationNode {
    pub id: NodeId,
    pub op: TensorOp,
    pub inputs: Vec<NodeId>,
    pub shape: Option<Vec<usize>>,
}

pub enum TensorOp {
    // Input/Constant
    Input { name: String },
    Constant { data: Vec<f32> },
    
    // Binary ops
    Add { a: NodeId, b: NodeId },
    Sub { a: NodeId, b: NodeId },
    Mul { a: NodeId, b: NodeId },
    Div { a: NodeId, b: NodeId },
    
    // Matrix ops
    MatMul { a: NodeId, b: NodeId },
    Einsum { equation: String, inputs: Vec<NodeId> },
    
    // Normalization
    Softmax { input: NodeId, axis: usize },
    LayerNorm { input: NodeId, eps: f32 },
    BatchNorm { input: NodeId, mean: NodeId, var: NodeId, gamma: NodeId, beta: NodeId, eps: f32 },
    
    // Activation
    Relu { input: NodeId },
    Gelu { input: NodeId },
    Sigmoid { input: NodeId },
    Tanh { input: NodeId },
    
    // Reduction
    Sum { input: NodeId, axes: Vec<usize> },
    Mean { input: NodeId, axes: Vec<usize> },
    Max { input: NodeId, axes: Vec<usize> },
    
    // Shape ops
    Reshape { input: NodeId, shape: Vec<usize> },
    Transpose { input: NodeId, perm: Vec<usize> },
    Concat { inputs: Vec<NodeId>, axis: usize },
    Split { input: NodeId, splits: Vec<usize>, axis: usize },
    
    // Fused ops (optimization)
    FusedLinear { input: NodeId, weight: NodeId, bias: Option<NodeId> },
    FusedAddReLU { a: NodeId, b: NodeId },
    FusedScaleBias { input: NodeId, scale: NodeId, bias: NodeId },
    
    // ... 40+ operations total
}
```

**Optimizations**:
- Topological sort (Kahn's algorithm)
- CSE (Common Subexpression Elimination)
- Constant folding
- Operation fusion (FusedLinear, FusedAddReLU, FusedScaleBias)
- Dead code elimination
- Shape inference

**API**:
```rust
pub fn add_node(&mut self, op: TensorOp, inputs: Vec<NodeId>) -> NodeId;
pub fn set_outputs(&mut self, outputs: Vec<NodeId>);
pub fn validate_dag(&self) -> Result<()>;
pub fn topological_sort(&self) -> Vec<NodeId>;
pub fn infer_shapes(&mut self) -> Result<()>;
pub fn optimize(&mut self) -> Result<()>;
pub fn execute(&self, inputs: HashMap<NodeId, Vec<f32>>) -> Result<HashMap<NodeId, Vec<f32>>>;
```

---

### 3.2 AutogradGraph — Reverse-Mode AD

**Файл**: `autograd.rs` (~400 LOC)

Reverse-mode automatic differentiation для gradient computation.

```rust
pub struct AutogradGraph {
    nodes: HashMap<NodeId, AutogradNode>,
}

pub enum AutogradOp {
    Input { requires_grad: bool },
    Add { a: NodeId, b: NodeId },
    Mul { a: NodeId, b: NodeId },
    MatMul { a: NodeId, b: NodeId },
    // ... mirror of TensorOp with grad functions
}

pub struct AutogradNode {
    pub id: NodeId,
    pub op: AutogradOp,
    pub inputs: Vec<NodeId>,
    pub grad_fn: Option<GradFn>,
}
```

**Алгоритм**: Iterative post-order DFS для backward pass.

**API**:
```rust
pub fn forward(&self, inputs: HashMap<NodeId, Vec<f32>>) -> Result<HashMap<NodeId, Vec<f32>>>;
pub fn backward(&self, loss: NodeId) -> Result<HashMap<NodeId, Vec<f32>>>;
```

---

### 3.3 OpFusion — Pattern Matching

**Файл**: `op_fusion.rs` (~300 LOC)

Greedy pattern matching для operation fusion.

**Patterns**:
- `ScaleBias`: `Mul(input, scale) + Add(bias)` → `FusedScaleBias`
- `ScaleReluBias`: `Mul(input, scale) + Add(bias) + Relu` → `FusedScaleBiasRelu`
- `ClampNormalize`: `Clamp(input, min, max) + Div(input, max)` → `FusedClampNorm`
- `Linear`: `MatMul(input, weight) + Add(bias)` → `FusedLinear`

```rust
pub struct OpFusion;

impl OpFusion {
    pub fn fuse(graph: &mut ComputationGraph) -> Result<usize>;  // Returns count of fused ops
}
```

---

### 3.4 OpDispatcher — Backend Routing

**Файл**: `op_dispatcher.rs` (~250 LOC)

Backend routing для tensor operations.

```rust
pub enum Backend {
    Cpu,
    Gpu,
    Remote { endpoint: String },
    Simulated { latency_ms: u64 },
}

pub struct OpDispatcher {
    backends: HashMap<BackendType, Box<dyn BackendExecutor>>,
    fallback: BackendType,
}

pub trait BackendExecutor {
    fn execute(&self, op: &TensorOp, inputs: &[Vec<f32>]) -> Result<Vec<f32>>;
    fn is_available(&self) -> bool;
}
```

**Routing Logic**:
1. Check if primary backend is available
2. If not, fall back to `fallback` backend
3. If both fail, return error

---

## 4. Gradient System

### 4.1 Gradient Types

**Файл**: `gradient/mod.rs` (~500 LOC)

```rust
pub struct SparseGradient {
    pub indices: Vec<usize>,
    pub values: Vec<f32>,
    pub shape: Vec<usize>,
}

pub struct QuantizedGradient {
    pub data: Vec<i8>,
    pub scale: f32,
    pub zero_point: i32,
    pub shape: Vec<usize>,
}

pub struct GradientDelta {
    pub base_version: u64,
    pub deltas: Vec<GradientDeltaEntry>,
}

pub struct GradientDeltaEntry {
    pub param_name: String,
    pub indices: Vec<usize>,
    pub values: Vec<f32>,
}
```

### 4.2 Differential Privacy

```rust
pub struct DifferentialPrivacy {
    pub mechanism: DpMechanism,
    pub epsilon: f64,
    pub delta: f64,
}

pub enum DpMechanism {
    Gaussian { sigma: f64 },
    Laplace { scale: f64 },
}
```

### 4.3 Secure Aggregation

```rust
pub struct SecureAggregation {
    pub secret_shares: Vec<Vec<u8>>,
    pub public_keys: HashMap<PeerId, Vec<u8>>,
}
```

---

## 5. Inference Engines

> **Полный каталог (обновлено 2026-06-19, выверено по коду).** Контекст содержит
> **25 движков вывода и обучения** — это крупнейшая уникальная ценность IPFRS. Ранее
> в этой секции были описаны лишь 3 (SLD, SLG, Temporal); ниже — все 25 с реальными
> сигнатурами и привязкой `file:line`. Имена/алиасы сверены с `lib.rs` (`pub use`,
> строки 558–706); многие типы реэкспортируются с префиксами (`Fle`, `Csp`, `Cpe`,
> `Mdp`, `Pln`, `Abr`, `El`, `Dtl`, `Hte`, `Ppe`, `Rla`) во избежание коллизий имён.

### Сводная таблица 25 движков

| # | Движок | Тип | Алгоритм | Источник |
|---|--------|-----|----------|----------|
| 1 | `InferenceEngine` | дедуктивный | SLD-резолюция + унификация | `reasoning.rs:221,259` |
| 2 | `MemoizedInferenceEngine` | дедуктивный | SLD + кэш по QueryKey | `reasoning.rs:632,657` |
| 3 | `TabledInferenceEngine` | дедуктивный | SLG-табуляция (recursion-safe) | `recursive_reasoning.rs:101,130` |
| 4 | `FixpointEngine` / `StratificationAnalyzer` | дедуктивный | стратифицированный Datalog-fixpoint | `recursive_reasoning.rs:312,482` |
| 5 | `DistributedBackwardChainer` | распределённый | обратный вывод + делегирование пирам по CID | `distributed_backward_chainer.rs:33,66` |
| 6 | `TemporalReasoningEngine` | темпоральный | 13 интервальных отношений Аллена | `temporal_reasoning.rs:293,100` |
| 7 | `FuzzyLogicEngine` | нечёткий | Mamdani/Sugeno + дефаззификация | `fuzzy_logic.rs:325,456` |
| 8 | `FleFuzzyLogicEngine` | нечёткий | полный Mamdani, 7 MF, 5 дефаззификаций | `fuzzy_logic_engine/types.rs:380,496` |
| 9 | `EpistemicLogicReasoner` | модальный | S5 Kripke model-checking, K/M/E/C | `epistemic_logic.rs:210,332` |
| 10 | `ProbabilisticLogicNetwork` | вероятностный | PLN, truth values (strength, confidence) | `probabilistic_logic_network.rs:483,577` |
| 11 | `BayesianNetworkInference` | вероятностный | VE / Belief Propagation / Sampling | `bayesian_network_inference.rs:575,665` |
| 12 | `BayesianUpdateEngine` | вероятностный | сопряжённые приоры (Beta/Gauss/Dirichlet/Gamma) | `bayesian_updater.rs:335,357` |
| 13 | `AbductiveReasoningEngine` | абдуктивный | branch-and-bound по гипотезам | `abductive_reasoning_engine.rs:392,476` |
| 14 | `CausalInferenceEngine` | каузальный | do-исчисление Пёрла + контрфактика | `causal_inference.rs:244,573` |
| 15 | `ConstraintSolver` | CSP | AC-3 + backtracking + MRV/LCV | `constraint_solver.rs:322,764` |
| 16 | `BeliefRevisionEngine` | ревизия | AGM expansion/contraction/Levi-revision | `belief_revision_engine.rs:336,487` |
| 17 | `ConstraintPropagationEngine` | CSP | AC3/AC4/AC6 + bounds propagation | `constraint_propagation_engine/types_3.rs:16,93` |
| 18 | `MarkovDecisionProcess` | планирование | value/policy iteration (Беллман) | `markov_decision_process/types.rs:150,259` |
| 19 | `ReinforcementLearningAgent` | RL | SARSA/Q/Expected-SARSA/Double-Q/N-step | `reinforcement_learning_agent.rs:53,201` |
| 20 | `ReinforcementLearner` | RL | Q/SARSA/Double-Q (упрощённый) | `reinforcement_learner.rs:217,386` |
| 21 | `HypothesisTestEngine` | статистика | z/t-тесты, χ², тесты долей | `hypothesis_test_engine/types.rs:73,107` |
| 22 | `ProbabilisticProgramEngine` | вероятностный | MCMC: MH/Gibbs/Importance/Rejection | `probabilistic_program_engine/ppe_types.rs:131` |
| 23 | `NeuralSymbolicIntegrator` | гибридный | смесь нейро + символика | `neural_symbolic.rs:227,474` |
| 24 | `DecisionTreeLearner` | индуктивный | ID3/C4.5 (Entropy/Gini), прунинг | `decision_tree_learner.rs:334,396` |
| 25 | `EnsembleLearner` | индуктивный | Bagging/AdaBoost/GradBoost/RF/Stacking | `ensemble_learner/types.rs:217,266` |

---

### 5.1 SLD Resolution

```rust
pub struct InferenceEngine {                 // reasoning.rs:221
    max_depth: usize,                         // default 100
    max_solutions: usize,                     // default 100
    cycle_detection: bool,                    // default true
}
// reasoning.rs:259
pub fn query(&self, goal: &Predicate, kb: &KnowledgeBase) -> Result<Vec<Substitution>>;
pub fn prove(&self, goal: &Predicate, kb: &KnowledgeBase) -> Result<Option<Proof>>;
```
Обратный вывод (SLD): унифицирует цель с фактами и головами правил, рекурсивно решает
конъюнкцию тела, собирает подстановки до лимитов; циклы детектируются по стеку целей.
`unify_predicates` экспортируется отдельно.

### 5.2 Memoized SLD

```rust
pub struct MemoizedInferenceEngine {          // reasoning.rs:632
    engine: InferenceEngine,
    cache: Arc<CacheManager>,
}
```
Обёртка над `InferenceEngine` с кэшированием результатов по `QueryKey`: попадание → готовые
подстановки, промах → запрос + запись в кэш.

### 5.3 SLG Tabling

```rust
pub struct TabledInferenceEngine {            // recursive_reasoning.rs:101
    table: HashMap<String, TableEntry>,
    max_depth: usize,                         // default 100
    max_solutions: usize,                     // default 1000
}
```
SLG-резолюция: мемоизирует подцели в таблице; первая запись помечается неполной (детект
циклов), затем полной. Обращение к ещё вычисляемой подцели возвращает пусто, разрывая цикл.

### 5.4 Stratified Fixpoint (Datalog)

```rust
pub struct FixpointEngine { max_iterations: usize }   // recursive_reasoning.rs:312, default 100
pub fn compute_fixpoint(&self, kb: &KnowledgeBase) -> Result<KnowledgeBase>;
pub enum StratificationResult { Stratifiable(Vec<Vec<String>>), NonStratifiable } // :482
```
Итеративно применяет все правила, выводя наземные факты до неподвижной точки.
`StratificationAnalyzer` строит граф зависимостей предикатов, ищет циклы DFS и разбивает на слои.

### 5.5 Distributed Backward Chaining

```rust
pub struct DistributedBackwardChainer {       // distributed_backward_chainer.rs:33
    pub max_depth: usize,        // default 10
    pub max_remote_peers: usize, // default 3
    pub timeout_ms: u64,         // default 5000
}
// distributed_backward_chainer.rs:66
pub async fn prove_with_tree<FP, FQ>(&self, goal: &Term, local_kb: &KnowledgeBase,
                                     find_providers: FP, remote_query: FQ) -> Result<ProofTree>;
```
Асинхронный вывод: при неудаче локально вычисляет CID правил, ищет провайдеров в DHT и
рассылает цель ≤`max_remote_peers` пирам; первый успех встраивается в дерево с аннотацией пира.

### 5.6 Temporal Reasoning

```rust
pub struct TemporalReasoningEngine {          // temporal_reasoning.rs:293
    events: HashMap<String, TemporalEvent>,
    constraints: Vec<TemporalConstraint>,
    max_events: usize,
}
pub enum AllenRelation {                       // temporal_reasoning.rs:100 — 13 отношений
    Precedes, Meets, Overlaps, FinishedBy, Contains, Starts,
    Equals, StartedBy, During, Finishes, OverlappedBy, MetBy, PrecededBy,
}
pub fn allen_relation(&self, a: &str, b: &str) -> Option<AllenRelation>;
```
Алгебра Аллена: классифицирует пару интервалов сравнением границ (13 отношений). Проверяет
ограничения и находит цепи событий BFS по графу перекрытий.

### 5.7 Fuzzy Logic (simple) & 5.8 Full Mamdani

```rust
pub struct FuzzyLogicEngine {                  // fuzzy_logic.rs:325
    variables: HashMap<String, FuzzyVariable>, rules: Vec<FuzzyRule>,
    inference: InferenceMethod, defuzz: DefuzzMethod,
}
pub enum InferenceMethod { Mamdani, Sugeno }
pub enum DefuzzMethod { Centroid, MeanOfMax, LargestOfMax }
pub fn infer(&self, inputs: &HashMap<String, f64>, output_var: &str) -> Result<f64, FuzzyError>; // :456
```
Mamdani активирует/агрегирует множества поточечным max + дефаззификация; Sugeno — взвешенная
сумма центроидов. **Полный движок** `FleFuzzyLogicEngine` (`fuzzy_logic_engine/types.rs:380`)
добавляет деревья антецедентов (And/Or/Not/Very/Somewhat), 7 MF и 5 методов дефаззификации,
дискретизация `resolution=100`.

### 5.9 Epistemic Logic (S5)

```rust
pub struct EpistemicLogicReasoner {            // epistemic_logic.rs:210
    pub model: KripkeModel, pub agents: Vec<AgentId>, pub max_depth: usize,
}
pub enum EpistemicFormula {                     // Atom/Not/And/Or/Knows/Possible/EveryoneKnows/CommonKnowledge
    Knows { agent: AgentId, phi: Box<EpistemicFormula> },
    CommonKnowledge(Box<EpistemicFormula>), /* ... */
}
pub fn evaluate_actual(&self, formula: &EpistemicFormula) -> Result<bool, EpistemicError>;
```
Model-checking модальных формул K_i/M_i в модели Крипке S5 (проверка во всех доступных мирах).
E(φ) — по всем агентам; C(φ) — fixpoint на объединённом отношении доступности (BFS до `max_depth`).

### 5.10 PLN (Probabilistic Logic Network)

```rust
pub struct TruthValue { pub strength: f64, pub confidence: f64 }   // probabilistic_logic_network.rs
pub enum PlnInferenceRule { Deduction, Induction, Abduction, Revision,
                            Conjunction, Disjunction, Negation, ModusPonens }
pub fn infer(&mut self, rule: PlnInferenceRule, premise_ids: Vec<String>)
    -> Result<PlnInferenceResult, PlnError>;                        // :577
```
Неопределённый вывод над гиперграфом атомов 8 правилами: Conjunction = s₁·s₂,
Disjunction = 1−(1−s₁)(1−s₂), Revision = взвешенное объединение. BFS ищет цепи вывода.
Defaults: `max_atoms=100_000`, `inference_depth=6`, `min_confidence_threshold=0.01`.

### 5.11–5.12 Bayesian: Network & Updater

```rust
pub enum InferenceAlgorithm { VariableElimination, BeliefPropagation, Sampling { n_samples, seed } }
pub fn query(&mut self, q: &InferenceQuery) -> Result<Vec<QueryResult>, BniError>; // bayesian_network_inference.rs:665
pub enum Prior { Beta{..}, Gaussian{..}, Dirichlet{..}, Gamma{..} }                // bayesian_updater.rs
pub fn update(&mut self, prior: Prior, observation: &Observation) -> Result<Posterior, BayesError>; // :357
```
**Network**: точный/приближённый вывод P(query|evidence) над DAG (VE/BP/weighted sampling, xorshift64).
**Updater**: сопряжённое обновление (Beta-Bernoulli, Gauss-Gauss, Dirichlet-Categorical, Gamma-Poisson) +
кредальные интервалы и KL-дивергенция.

### 5.13 Abductive Reasoning

```rust
pub enum AbrCostFunction { SumCost, MaxCost, CountCost, WeightedCost(HashMap<HypothesisId,f64>) }
pub fn abduce(&mut self) -> Vec<AbrExplanation>;   // abductive_reasoning_engine.rs:476
```
Branch-and-bound по подмножествам гипотез, упорядоченным по стоимости; отсекает наборы, не
покрывающие наблюдения. Defaults: `max_explanations=10`, `max_hypothesis_set_size=8`, `SumCost`.

### 5.14 Causal Inference (do-calculus)

```rust
pub struct Intervention { pub node: CausalNodeId, pub value: f64 }
pub fn do_calculus(&self, intervention: &Intervention, target: &CausalNodeId) -> InferenceResult; // causal_inference.rs:573
pub fn counterfactual(&self, query: &CounterfactualQuery) -> InferenceResult;                     // :612
```
P(target | do(intervention)) аккумуляцией линейных эффектов по направленным путям (Gaussian SCM);
`counterfactual` добавляет взвешенную коррекцию от условных переменных.

### 5.15 & 5.17 Constraint: Solver (CSP) & Propagation

```rust
pub enum Constraint { AllDifferent(..), Equal(..), NotEqual(..), LessThan(..),
                      LessEqual(..), Sum{vars, target}, InDomain{var, allowed} }
pub fn solve(&mut self) -> SolverResult;   // constraint_solver.rs:764
```
**CSP-solver**: AC-3 + backtracking + MRV/LCV/forward-checking. **Constraint Propagation**
(`constraint_propagation_engine/types_3.rs:93`) — уровни AC3/AC4/AC6 + bounds propagation + Fail-First.

### 5.16 Belief Revision (AGM)

```rust
pub enum RevisionOp { Expansion(Belief), Contraction(String), Revision(Belief), Consolidation }
pub enum RetentionFunction { EpistemicEntrenchment, RecencyBias, SourcePriority(..), MinimalChange }
pub fn revise(&mut self, belief: Belief) -> Result<(Vec<String>, Vec<String>), RevisionError>; // :revise
```
AGM: expansion (+ следствия), contraction (макс. непротиворечивые подмножества),
revision (тождество Леви: contract ¬φ → expand φ). Consolidation по `RetentionFunction`.

### 5.18 Markov Decision Process

```rust
pub enum SolverType { ValueIteration, PolicyIteration, ModifiedPolicyIteration(usize), Qlearning{alpha,epsilon} }
pub fn value_iteration(&self, config: &SolverConfig) -> (ValueFunction, SolverResult);          // markov_decision_process/types.rs:259
pub fn policy_iteration(&self, config: &SolverConfig) -> (MdpPolicy, ValueFunction, SolverResult); // :353
```
Беллмановское обновление `V(s)=max_a Σ p(t)·(r+γ·V(t'))` до сходимости; policy iteration
чередует оценку и жадное улучшение. Defaults: `gamma=0.99`, `epsilon=1e-6`, `max_iter=1000`.

### 5.19–5.20 Reinforcement Learning (×2)

```rust
pub enum AlgorithmType { Sarsa, QLearning, ExpectedSarsa, DoubleQLearning, NStepTD(u8) }   // rla_types.rs:139
pub enum AgentPolicy { EpsilonGreedy{..}, Boltzmann{temperature}, UCB{c}, Random }
pub fn update(&mut self, transition: &Transition) -> Result<f64, RlAgentError>;            // reinforcement_learning_agent.rs:201
```
Табличный RL: обновление по TD-ошибке δ = r + γ·Q(s',a') − Q(s,a), с experience replay и
eligibility traces. `ReinforcementLearner` (`reinforcement_learner.rs:217`) — упрощённый вариант
(Q/SARSA/Double-Q).

### 5.21–5.22 Hypothesis Testing & Probabilistic Program

```rust
pub enum TestType { OneSampleZTest{..}, OneSampleTTest{..}, TwoSampleTTest{..},
                    ChiSquareGoodnessOfFit{..}, ChiSquareIndependence{..}, OneSampleProportion{..}, TwoSampleProportion }
pub enum PpeSamplingMethod { MetropolisHastings, GibbsSampling, ImportanceSampling, RejectionSampling }
```
**Hypothesis Testing** (`hypothesis_test_engine/types.rs:107`): z/t/χ²/тесты долей → p-value,
доверительные интервалы, размер эффекта (Cohen's d, Cramér's V), мощность Монте-Карло.
**Probabilistic Program** (`probabilistic_program_engine/mod.rs:132`): апостериорный сэмплинг
(MH/Gibbs/Importance/Rejection) с burn-in и thinning.

### 5.23 Neural-Symbolic Integrator

```rust
pub enum InferenceMode { PureSymbolic, PureNeural, Hybrid { neural_weight: f64 } }  // neural_symbolic.rs:124
pub fn infer(&mut self, query: &NsQuery) -> Result<NsResult, NsError>;              // :474
// формула смешивания (neural_symbolic.rs:489):
let nw = neural_weight.clamp(0.0, 1.0);
nw * neural + (1.0 - nw) * symbolic
```
Гибрид: близость эмбеддингов (нейро) + forward-chaining по правилам (символика). `NsResult`
возвращает `neural_contribution` и `symbolic_contribution` раздельно (объяснимость).
Defaults: `embedding_dim=128`, `inference_depth=5`, `similarity_threshold=0.7`.

### 5.24–5.25 Inductive Learners: Decision Tree & Ensemble

```rust
pub enum DtlCriterion { Entropy, Gini, MisclassificationRate }   // decision_tree_learner.rs
pub enum ElMethod { Bagging, AdaBoost, GradientBoosting, RandomForest, Stacking }  // ensemble_learner/types.rs:182
pub fn fit(&mut self, samples: &[DtlSample]) -> Result<(), DtlError>;
```
**Decision Tree**: ID3/C4.5, бинарные разбиения по непрерывным признакам (Entropy/Gini/MisclassRate),
прунинг. **Ensemble**: 5 стратегий ансамблирования. Весь рандом — на `xorshift64` (без crate `rand`).

> ⚠️ **Архитектурная заметка**: единого `trait InferenceEngine`, который реализуют все 25
> движков, **нет** — `reasoning::InferenceEngine` это конкретная структура. Отсюда дублирование
> примитивов (несколько PRNG, 2 RL, 2 fuzzy) — технический долг (см. `[[../Wiki/11-RealityCheck]]`).

---

## 6. Proof System

### 6.1 ProofTree

**Файл**: `proof_tree.rs` (~400 LOC)

```rust
pub struct ProofTree {
    pub goal: Predicate,
    pub rule: Option<Rule>,
    pub subproofs: Vec<ProofTree>,
    pub attribution: Option<PeerAttribution>,
}

pub struct ProofNode {
    pub id: ProofNodeId,
    pub goal: Predicate,
    pub rule_cid: Option<Cid>,
    pub children: Vec<ProofNodeId>,
}

pub struct ProofCache {
    cache: LruCache<Predicate, ProofTree>,
    max_size: usize,
}
```

---

## 7. Key Files Summary

| Category | File | Lines | Purpose |
|----------|------|-------|---------|
| **IR** | ir.rs | 350+ | Term, Predicate, Rule, Fact |
| **IR** | term_index.rs | 200+ | HashCons for Terms |
| **IR** | rule_index.rs | 200+ | HashCons for Rules |
| **IR** | rule_dependency.rs | 300+ | DAG dependency analysis |
| **Memory** | tensor_arena.rs | 300+ | Bump allocator |
| **Memory** | tensor_pool.rs | 450+ | Slab-based buffer pool |
| **Memory** | tensor_gc.rs | 400+ | Mark-and-sweep GC |
| **Memory** | tensor_quantizer.rs | 1250+ | Multi-precision quantization |
| **Memory** | tensor_diff.rs | 350+ | Federated learning diff |
| **Memory** | tensor_checksum.rs | 400+ | Corruption detection |
| **Execution** | computation_graph.rs | 1723+ | DAG execution |
| **Execution** | autograd.rs | 400+ | Reverse-mode AD |
| **Execution** | op_fusion.rs | 300+ | Pattern matching fusion |
| **Execution** | op_dispatcher.rs | 250+ | Backend routing |
| **Inference** | reasoning.rs | 800+ | SLD Resolution |
| **Inference** | recursive_reasoning.rs | 600+ | SLG Tabling |
| **Inference** | temporal_reasoning.rs | 500+ | Allen's algebra |
| **Inference** | fuzzy_logic.rs | 400+ | Mamdani/Sugeno |
| **Inference** | epistemic_logic.rs | 450+ | S5 Kripke |
| **Inference** | probabilistic_logic_network.rs | 700+ | PLN |
| **Inference** | bayesian_network_inference.rs | 500+ | BN inference (VE/BP/sampling) |
| **Inference** | neural_symbolic.rs | 600+ | Hybrid integration |
| **Inference** | distributed_backward_chainer.rs | 300+ | Cross-peer backward chaining |
| **Inference** | bayesian_updater.rs | 400+ | Conjugate-prior updating |
| **Inference** | abductive_reasoning_engine.rs | 500+ | Abduction (branch-and-bound) |
| **Inference** | causal_inference.rs | 600+ | Do-calculus + counterfactuals |
| **Inference** | constraint_solver.rs | 800+ | CSP (AC-3 + backtracking) |
| **Inference** | constraint_propagation_engine/ | 400+ | AC3/AC4/AC6 propagation |
| **Inference** | belief_revision_engine.rs | 500+ | AGM belief revision |
| **Inference** | markov_decision_process/ | 400+ | MDP value/policy iteration |
| **Inference** | reinforcement_learning_agent.rs | 500+ | RL (SARSA/Q/Double-Q/N-step) |
| **Inference** | reinforcement_learner.rs | 400+ | RL (simplified tabular) |
| **Inference** | hypothesis_test_engine/ | 300+ | z/t/χ² statistical tests |
| **Inference** | probabilistic_program_engine/ | 400+ | MCMC posterior sampling |
| **Inference** | fuzzy_logic_engine/ | 600+ | Full Mamdani (7 MF, 5 defuzz) |
| **Learning** | decision_tree_learner.rs | 400+ | ID3/C4.5 with pruning |
| **Learning** | ensemble_learner/ | 500+ | Bagging/Boosting/RF/Stacking |
| **Gradient** | gradient/mod.rs | 500+ | Gradient types |
| **Gradient** | backward_pass.rs | 300+ | Backward pass |
| **Gradient** | federated.rs | 400+ | Federated aggregation |
| **Proof** | proof_tree.rs | 400+ | Proof structure |
| **Proof** | proof_cache.rs | 200+ | LRU cache |
| **Proof** | provenance.rs | 250+ | Source attribution |

---

## 8. Invariants

| Invariant | Enforcement |
|-----------|-------------|
| Facts are ground | Validation on add |
| Rule DAG is acyclic | `rule_dependency.rs` |
| Head vars bound by body | Validation |
| Identical rule ⟹ identical CID | Content-addressed |
| Proof is sound | Each node ↔ KB rule |
| Proof is acyclic | Tree structure |
| ComputationGraph is DAG | `validate_dag()` |
| Arena alignment is 8-byte | ARENA_ALIGN constant |
| GC never collects pinned | `tensor.pinned == true` check |
| GC never collects ref_counted | `tensor.ref_count > 0` check |

---

## 9. Performance

| Operation | P50 | P99 | Notes |
|-----------|-----|-----|-------|
| Arena allocate | 10 ns | 50 ns | O(1) bump |
| Pool acquire | 100 ns | 1 µs | Lock contention |
| Pool release | 100 ns | 1 µs | Lock contention |
| GC collect | 1 ms | 10 ms | Depends on graph size |
| Quantize (1M elems) | 5 ms | 20 ms | Depends on mode |
| Checksum compute | 100 µs | 1 ms | Depends on algorithm |
| Simple query | 1 ms | 5 ms | SLD Resolution |
| Recursive (tabling) | 5 ms | 50 ms | Depends on recursion depth |
| Distributed query | 100 ms | 1000 ms | Network latency dominant |

---

## 10. Context Integration

```
┌─────────────────────────────────────────────────────────────────────┐
│                    LOGIC INTEGRATION                                │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Consumes (Shared Kernel):                                          │
│    • Cid, Block, Ipld                                               │
│    • Error, Result                                                  │
│                                                                     │
│  Consumes (Customer/Supplier):                                      │
│    • Storage — BlockStore (rule storage)                            │
│    • Semantic — embeddings (neural-symbolic)                        │
│    • Network — DistributedBackwardChainer                           │
│    • Transport — proof streaming                                    │
│                                                                     │
│  Publishes:                                                         │
│    • ProofTree — cacheable, verifiable                              │
│    • QuantizedTensor — compressed gradients                         │
│    • TensorDiff — federated change detection                        │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

**Next**: [07-TransportContext.md](07-TransportContext.md) — Bitswap, sessions, want-list
