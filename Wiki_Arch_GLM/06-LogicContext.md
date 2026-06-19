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
│  │                    INFERENCE ENGINES (9 engines)             │   │
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

### 5.1 SLD Resolution

**Файл**: `reasoning.rs` (~800 LOC)

```rust
pub struct InferenceEngine {
    max_depth: usize,
    max_solutions: usize,
    cycle_detection: bool,
}

impl InferenceEngine {
    pub fn query(&self, goal: &Predicate, kb: &KnowledgeBase) -> Result<Vec<Substitution>>;
    pub fn prove(&self, goal: &Predicate, kb: &KnowledgeBase) -> Result<Option<Proof>>;
}
```

### 5.2 SLG Tabling

**Файл**: `recursive_reasoning.rs` (~600 LOC)

```rust
pub struct TabledInferenceEngine {
    tables: HashMap<Predicate, TableEntry>,
    worklist: VecDeque<Goal>,
}

enum TableEntry {
    InProgress,
    Complete(Vec<Substitution>),
}
```

### 5.3 Temporal Reasoning

**Файл**: `temporal_reasoning.rs` (~500 LOC)

**Allen's 13 Relations**:

| Relation | Inverse | Meaning |
|----------|---------|---------|
| Before | After | X ends before Y starts |
| Meets | MetBy | X ends when Y starts |
| Overlaps | OverlappedBy | X starts before Y, overlaps |
| Starts | StartedBy | X and Y start together |
| During | Contains | X contained in Y |
| Finishes | FinishedBy | X and Y end together |
| Equals | Equals | X and Y identical |

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
| **Inference** | bayesian_network_inference.rs | 500+ | BN inference |
| **Inference** | neural_symbolic.rs | 600+ | Hybrid integration |
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
