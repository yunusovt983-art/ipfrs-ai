# Logic Context — IR, Inference, Neural-Symbolic

> **Focus**: Symbolic reasoning, neural-symbolic fusion, distributed inference  
> **Source**: `ipfrs_source/crates/ipfrs-tensorlogic/src/` (194 files)

---

## 1. Context Overview

Logic Context — самый концептуально сложный bounded context. Отвечает за **content-addressed symbolic reasoning fused with tensor computation**.

```
┌─────────────────────────────────────────────────────────────────────┐
│                    LOGIC CONTEXT                                    │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    IR (INTERMEDIATE REPRESENTATION)          │   │
│  │  ─────────────────────────────────────────────────────────── │   │
│  │  Term(Var|Const|Fun|Ref)  →  Predicate  →  Rule  →  KB       │   │
│  │        (value objects)         (VO)       (VO)    (AR)       │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    INFERENCE ENGINES                         │   │
│  │  ─────────────────────────────────────────────────────────── │   │
│  │  SLD Backward     — Standard resolution                      │   │
│  │  Tabling (SLG)    — Recursive handling                       │   │
│  │  Temporal         — Allen's interval algebra                 │   │
│  │  Fuzzy            — Mamdani/Sugeno                           │   │
│  │  Epistemic        — S5 Kripke semantics                      │   │
│  │  PLN              — OpenCog probabilistic logic              │   │
│  │  Bayesian         — Variable elimination, BP                 │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    NEURAL-SYMBOLIC FUSION                    │   │
│  │  ─────────────────────────────────────────────────────────── │   │
│  │  Symbol(embedding, confidence)                               │   │
│  │  LogicalRule(weight, RuleType)                               │   │
│  │  InferenceMode: PureSymbolic | PureNeural | Hybrid           │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    TENSOR EXECUTION                          │   │
│  │  ─────────────────────────────────────────────────────────── │   │
│  │  ComputationGraph (DAG)                                      │   │
│  │  AutogradGraph (reverse-mode)                                │   │
│  │  TensorOp: MatMul | Einsum | Softmax | Fused                 │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                    DISTRIBUTION                              │   │
│  │  ─────────────────────────────────────────────────────────── │   │
│  │  DistributedBackwardChainer — Cross-peer inference           │   │
│  │  IPLD codec — Rule ↔ Block                                   │   │
│  │  ProofTree — Peer-attributed proofs                          │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## 2. IR — Intermediate Representation

### 2.1 Term

```rust
pub enum Term {
    Var(String),                    // ?X, ?Y — unification variable
    Const(Constant),                // Ground term
    Fun(String, Vec<Term>),         // f(X, g(Y)) — compound
    Ref(TermRef),                   // CID-addressed external term
}

pub enum Constant {
    String(String),
    Int(i64),
    Bool(bool),
    Float(String),                  // Float-as-string for deterministic hash
}

pub struct TermRef {
    pub cid: Cid,                   // External term
    pub hint: Option<String>,       // Type hint
}
```

### 2.2 Predicate

```rust
pub struct Predicate {
    pub name: String,               // Relation name
    pub args: Vec<Term>,            // Arguments
}
```

### 2.3 Rule (Horn Clause)

```rust
pub struct Rule {
    pub head: Predicate,            // Conclusion
    pub body: Vec<Predicate>,       // Premises (conjunction)
}

// Horn form: head ← body₁ ∧ body₂ ∧ ... ∧ bodyₙ
```

### 2.4 KnowledgeBase (Aggregate Root)

```rust
pub struct KnowledgeBase {
    pub facts: Vec<Predicate>,      // Ground facts
    pub rules: Vec<Rule>,           // Inference rules
}
```

### 2.5 Substitution (Key Value Object)

```rust
pub type Substitution = HashMap<String, Term>;

// Unification:
fn unify(p1: &Predicate, p2: &Predicate) -> Option<Substitution>;
```

---

## 3. Inference Engines

### 3.1 SLD Resolution (Backward Chaining)

```rust
pub struct InferenceEngine {
    max_depth: usize,
    max_solutions: usize,
    cycle_detection: bool,
}

impl InferenceEngine {
    pub fn query(&self, goal: &Predicate, kb: &KnowledgeBase) -> Result<Vec<Substitution>>;
    pub fn prove(&self, goal: &Predicate, kb: &KnowledgeBase) -> Result<Option<Proof>>;
    pub fn verify(&self, proof: &Proof, kb: &KnowledgeBase) -> Result<bool>;
}

pub struct Proof {
    pub goal: Predicate,
    pub rule: Option<Rule>,
    pub subproofs: Vec<Proof>,
}
```

**Algorithm**:
```
query(goal, kb):
  for rule in kb.rules where rule.head unifies with goal:
    subst = unify(rule.head, goal)
    subgoals = apply(rule.body, subst)
    subproofs = [query(g, kb) for g in subgoals]
    if all subproofs succeed:
      return compose(subst, subproofs)
  return None
```

---

### 3.2 Tabling (SLG)

```rust
pub struct TabledInferenceEngine {
    table: HashMap<Predicate, TableEntry>,
}

enum TableEntry {
    InProgress,                     // Being computed
    Complete(Vec<Substitution>),    // Cached
}
```

**Prevents**: Redundant computation, infinite loops (left-recursion).

---

### 3.3 Temporal Logic

**Allen's 13 Relations**:

| Relation | Meaning |
|----------|---------|
| `before` | X ends before Y starts |
| `meets` | X ends when Y starts |
| `overlaps` | X starts before Y, overlaps |
| `starts` | X and Y start together |
| `during` | X contained in Y |
| `finishes` | X and Y end together |
| `equals` | X and Y identical |

```rust
pub struct TemporalPredicate {
    pub event_a: Term,
    pub event_b: Term,
    pub relation: AllenRelation,
}
```

---

### 3.4 Fuzzy Logic

```rust
pub struct FuzzyLogicEngine {
    t_norm: TNorm,                  // min | product | lukasiewicz
    s_norm: SNorm,                  // max | probabilistic
    defuzzify: Defuzzification,     // centroid | bisector
}

pub struct FuzzyRule {
    pub antecedent: Vec<FuzzyPredicate>,
    pub consequent: FuzzySet,
    pub weight: f64,
}
```

---

### 3.5 Epistemic Logic (S5)

```rust
pub enum EpistemicOperator {
    Knows,                          // □ (necessity)
    Possible,                       // ◇ (possibility)
    CommonKnowledge,                // Everyone knows everyone knows...
}

pub struct EpistemicFormula {
    pub agent: String,
    pub operator: EpistemicOperator,
    pub formula: Box<Formula>,
}
```

---

### 3.6 Probabilistic Logic Network

```rust
pub struct TruthValue {
    pub strength: f64,              // Probability
    pub confidence: f64,            // Evidence count
}

fn deduction(tv1: TruthValue, tv2: TruthValue) -> TruthValue;
fn induction(tv1: TruthValue, tv2: TruthValue) -> TruthValue;
fn abduction(tv1: TruthValue, tv2: TruthValue) -> TruthValue;
```

---

### 3.7 Bayesian Network

```rust
pub struct BayesianNetwork {
    nodes: HashMap<String, BnNode>,
    edges: Vec<(String, String)>,
}

pub enum InferenceMethod {
    VariableElimination,
    BeliefPropagation,
    GibbsSampling { iterations: usize },
}
```

---

## 4. Neural-Symbolic Fusion

### 4.1 The Thesis

**Hybrid inference**: Blend symbolic rules with neural embeddings.

```rust
pub struct Symbol {
    pub id: SymbolId,
    pub name: String,
    pub embedding: Vec<f64>,
    pub confidence: f64,
}

pub struct LogicalRule {
    pub head: SymbolId,
    pub body: Vec<SymbolId>,
    pub weight: f64,
    pub rule_type: RuleType,
}

pub enum RuleType {
    Definite,                       // Hard rule
    Probabilistic,                  // Soft rule
    Soft { temperature: f64 },      // Differentiable
}

pub enum InferenceMode {
    PureSymbolic,
    PureNeural,
    Hybrid { neural_weight: f64 },
}
```

### 4.2 Hybrid Formula

```
confidence = neural_weight × cosine(embeddings)
           + (1 - neural_weight) × forward_chain_confidence
```

### 4.3 Differentiable Unification

```rust
fn soft_unify(t1: &Term, t2: &Term, embeddings: &EmbeddingStore) -> f64 {
    let e1 = embeddings.get(t1);
    let e2 = embeddings.get(t2);
    sigmoid(cosine(e1, e2) / temperature)
}
```

---

## 5. Tensor Execution

### 5.1 ComputationGraph

```rust
pub struct ComputationGraph {
    nodes: Vec<TensorNode>,
    edges: Vec<(NodeId, NodeId)>,
}

pub enum TensorOp {
    MatMul { a: NodeId, b: NodeId },
    Einsum { equation: String, inputs: Vec<NodeId> },
    Softmax { input: NodeId, axis: usize },
    LayerNorm { input: NodeId, eps: f64 },
    Fused { ops: Vec<TensorOp> },
}

impl ComputationGraph {
    fn validate_dag(&self) -> Result<()>;
    fn infer_shapes(&self) -> Result<Vec<Vec<usize>>>;
}
```

### 5.2 Autograd

```rust
pub struct AutogradGraph {
    forward: ComputationGraph,
    backward: ComputationGraph,
    gradients: HashMap<NodeId, Tensor>,
}

impl AutogradGraph {
    fn forward(&self, inputs: HashMap<NodeId, Tensor>) -> HashMap<NodeId, Tensor>;
    fn backward(&self, loss: NodeId) -> HashMap<NodeId, Tensor>;
}
```

---

## 6. Content-Addressed Logic

### 6.1 IPLD Codec

```rust
impl Rule {
    pub fn to_ipld(&self) -> Ipld;
    pub fn from_ipld(ipld: &Ipld) -> Result<Self>;
    
    pub fn cid(&self) -> Result<Cid> {
        let ipld = self.to_ipld();
        let bytes = DagCborCodec::encode(&ipld)?;
        CidBuilder::new().codec(CBOR).build(&bytes)
    }
    
    pub fn to_block(&self) -> Result<Block>;
}
```

**Benefits**:
- Rules are deduplicated by content
- Shareable over Bitswap
- Resolvable by IPLD path

---

### 6.2 DistributedBackwardChainer

```rust
pub struct DistributedBackwardChainer {
    local_kb: KnowledgeBase,
    network: Arc<NetworkNode>,
    store: Arc<dyn BlockStore>,
}

impl DistributedBackwardChainer {
    async fn query(&self, goal: &Predicate) -> Result<Option<ProofTree>> {
        // 1. Try local
        if let Some(proof) = self.local_infer(goal)? {
            return Ok(Some(proof));
        }
        
        // 2. Find peers via DHT
        let peers = self.network.find_providers(goal_cid).await?;
        
        // 3. Delegate to peers
        for peer in peers {
            if let Some(remote_proof) = self.query_peer(peer, goal).await? {
                return Ok(Some(remote_proof));
            }
        }
        
        Ok(None)
    }
}
```

---

### 6.3 ProofTree

```rust
pub struct ProofTree {
    pub goal: Predicate,
    pub rule: Option<Rule>,
    pub subproofs: Vec<ProofTree>,
    pub attribution: Option<PeerAttribution>,
}

pub struct PeerAttribution {
    pub peer_id: String,
    pub rule_cid: Cid,
    pub timestamp: u64,
    pub signature: Option<Vec<u8>>,
}
```

---

## 7. Rule Validation

```rust
pub fn validate_rule(rule: &Rule, existing: &[Rule]) -> Result<()> {
    // 1. Head vars bound by body
    for var in extract_variables(&rule.head) {
        if !bound_in_body(&var, &rule.body) {
            return Err(ValidationError::UnboundVariable(var));
        }
    }
    
    // 2. No circular dependencies
    if has_circular_dependency(rule, existing) {
        return Err(ValidationError::CircularDependency);
    }
    
    Ok(())
}
```

---

## 8. Invariants

| Invariant | Enforcement |
|-----------|-------------|
| Facts are ground | Validation on add |
| Rule DAG is acyclic | `rule_validator.rs` |
| Head vars bound by body | Validation |
| Identical rule ⟹ identical CID | Content-addressed |
| Proof is sound | Each node ↔ KB rule |
| Proof is acyclic | Tree structure |
| ComputationGraph is DAG | `validate_dag()` |

---

## 9. Performance

| Operation | P50 | P99 |
|-----------|-----|-----|
| Simple query | 1ms | 5ms |
| Recursive (tabling) | 5ms | 50ms |
| Distributed query | 100ms | 1000ms |
| Proof verify | 0.5ms | 5ms |
| Rule → Block | 0.1ms | 1ms |

---

## 10. Key Files

| File | Lines | Purpose |
|------|-------|---------|
| `ir.rs` | 350+ | IR definitions |
| `reasoning.rs` | 500+ | SLD resolution |
| `recursive_reasoning.rs` | 400+ | SLG tabling |
| `temporal.rs` | 300+ | Allen's algebra |
| `fuzzy.rs` | 250+ | Fuzzy logic |
| `epistemic_logic.rs` | 200+ | S5 semantics |
| `pln.rs` | 400+ | Probabilistic logic |
| `neural_symbolic.rs` | 500+ | Neural-symbolic |
| `computation_graph.rs` | 400+ | Tensor DAG |
| `distributed_backward_chainer.rs` | 400+ | Distributed |

---

## 11. Design Decisions

### 11.1 Why Neural-Symbolic Fusion?

**Decision**: Blend rules + embeddings.

**Rationale**:
- Rules: Interpretable, verifiable
- Embeddings: Approximate, fuzzy
- Hybrid: Best of both

---

### 11.2 Why Content-Addressed Rules?

**Decision**: Rules → Block → CID.

**Rationale**:
- Natural dedup
- Shareable over Bitswap
- IPLD path resolution

---

### 11.3 Why Multiple Inference Engines?

**Decision**: Strategy pattern for inference.

**Rationale**:
- Different domains need different logic
- Temporal, fuzzy, probabilistic, etc.
- Pluggable, extensible

---

## 12. Context Integration

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
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

**Next**: [07-TransportContext.md](07-TransportContext.md) — Bitswap, sessions, want-list
