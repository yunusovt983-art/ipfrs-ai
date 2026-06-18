---
title: 07-LogicDomain
type: domain
summary: Логический вывод TensorLogic — IR Term/Rule/KB, backward chaining, нейро-символический синтез
tags: [ipfrs, logic, ddd, tensorlogic, inference]
source: crates/ipfrs-tensorlogic/src/
related: ["[[03-BoundedContexts]]", "[[06-SemanticDomain]]", "[[09-DataFlows]]"]
read_time: 45 мин
updated: 2026-06-18
---

# Logic Domain: Логический вывод и TensorLogic

**Краткое резюме**: Logic Domain отвечает на вопрос "Что мы можем вывести?" с помощью backward chaining. Distinctive feature IPFRS — **neural-symbolic fusion**: логический вывод + семантический fallback.

---

## Язык домена

| Термин | Значение |
|--------|----------|
| **Term** | Переменная, константа или составное выражение |
| **Predicate** | Отношение, e.g., parent(alice, bob) |
| **Rule** | Если-то: ancestor(X,Z) :- parent(X,Y), ancestor(Y,Z) |
| **Fact** | Конкретное утверждение (no variables) |
| **Substitution** | Привязка переменных, e.g., {X: alice} |
| **Proof Tree** | DAG решений (SLD resolution) |

---

## Domain Model: Logic IR

**Source**: `crates/ipfrs-tensorlogic/src/ir.rs`

```rust
// tensorlogic/ir.rs:13–22
pub enum Term {
    Var(String),                      // ?X
    Const(Constant),
    Fun(String, Vec<Term>),           // f(X,Y)
    Ref(TermRef),                     // CID-addressed external term
}

pub enum Constant { 
    String(String), Int(i64), Bool(bool), 
    Float(String)  // Float-as-string ⟹ deterministic hash
}

pub struct TermRef { 
    pub cid: Cid, 
    pub hint: Option<String> 
}                             // :38–63

pub struct Predicate { 
    pub name: String, 
    pub args: Vec<Term> 
}                              // :163

pub struct Rule { 
    pub head: Predicate, 
    pub body: Vec<Predicate> 
}                              // Horn clause :216

pub struct KnowledgeBase { 
    pub facts: Vec<Predicate>, 
    pub rules: Vec<Rule> 
}                              // aggregate root :277

pub type Substitution = HashMap<String, Term>  // Variable bindings - key VO
```

**Инварианты** (`rule_validator.rs`):
- KB facts ground (no free vars)
- rule-dependency graph acyclic
- head vars bound by body → `ValidationError::UnboundVariable/CircularDependency`
- identical rule ⟹ identical CID

---

## Inference Services

**Source**: `crates/ipfrs-tensorlogic/src/reasoning.rs`

```rust
// reasoning.rs — backward chaining (SLD resolution)
pub struct InferenceEngine { 
    max_depth, max_solutions: usize, 
    cycle_detection: bool 
}

fn query(goal, kb) -> Result<Vec<Substitution>>
fn prove(goal, kb) -> Result<Option<Proof>>      // Proof{goal, rule, subproofs}
fn verify(proof, kb) -> Result<bool>
// + unify_predicates, rename_rule_vars (capture avoidance), apply_subst_predicate
```

**Variants**:
- `TabledInferenceEngine`/`FixpointEngine` (recursive_reasoning.rs, SLG tabling)
- `FuzzyLogicEngine` (Mamdani/Sugeno + defuzzification)
- `epistemic_logic.rs` (S5 Kripke, `Knows/CommonKnowledge`)
- `ProbabilisticLogicNetwork` (`TruthValue{strength, confidence}`, OpenCog-style)
- `BayesianNetwork` (VarElim/BeliefProp/Sampling)

// Example:
// unify(parent(X, bob), parent(alice, bob))
// → binds X = alice, returns true
```

---

## Inference: Backward Chaining

```rust
pub fn infer(goal: &Predicate, 
             rules: &[Rule], 
             facts: &[Fact],
             depth: usize) -> Vec<Substitution> {
    
    let mut solutions = Vec::new();
    
    if depth > MAX_DEPTH { return solutions; }  // Prevent infinite recursion
    
    // 1. Try to match with facts directly
    for fact in facts {
        let mut subst = Substitution::new();
        if unify(&goal, &fact.predicate, &mut subst) {
            solutions.push(subst);
        }
    }
    
    // 2. Try to match with rule heads
    for rule in rules {
        let mut subst = Substitution::new();
        if unify(&goal, &rule.head, &mut subst) {
            // Recursive case: prove body goals
            let body_solutions = prove_body(&rule.body, rules, facts, depth + 1, &subst);
            solutions.extend(body_solutions);
        }
    }
    
    solutions
}

fn prove_body(goals: &[Predicate], 
              rules: &[Rule], 
              facts: &[Fact],
              depth: usize,
              parent_subst: &Substitution) -> Vec<Substitution> {
    
    if goals.is_empty() { return vec![parent_subst.clone()]; }
    
    let first_goal = &goals[0];
    let rest = &goals[1..];
    let mut solutions = Vec::new();
    
    // Prove first goal
    for goal_subst in infer(first_goal, rules, facts, depth) {
        // Merge with parent substitution
        let mut merged = parent_subst.clone();
        merged.extend(goal_subst);
        
        // Prove rest of goals with merged substitution
        let rest_solutions = prove_body(rest, rules, facts, depth, &merged);
        solutions.extend(rest_solutions);
    }
    
    solutions
}
```

### Пример: Prove ancestor(alice, X)?

```
Goal: ancestor(alice, X)
Knowledge Base:
  Facts: parent(alice, bob), parent(bob, charlie)
  Rule: ancestor(A, Z) :- parent(A, Y), ancestor(Y, Z)

Step 1: Check facts
  ancestor(alice, X) vs available facts
  → No fact matches (no ancestor facts)

Step 2: Try Rule
  ancestor(A, Z) :- parent(A, Y), ancestor(Y, Z)
  Unify: ancestor(alice, X) with ancestor(A, Z)
  → A = alice, Z = X
  Subgoals: parent(alice, Y), ancestor(Y, X)

Step 3: Prove parent(alice, Y)
  Match with fact: parent(alice, bob)
  → Y = bob

Step 4: Prove ancestor(bob, X)
  Try rule again: ancestor(B, W) :- parent(B, V), ancestor(V, W)
  Unify: ancestor(bob, X) with ancestor(B, W)
  → B = bob, W = X
  Subgoals: parent(bob, V), ancestor(V, X)
  
  Prove parent(bob, V): parent(bob, charlie) → V = charlie
  Prove ancestor(charlie, X):
    Try facts: no match
    Try rule: parent(charlie, ...) → no match
    → No solutions
  
  But we can also match parent(bob, charlie) with itself!
  This gives us one solution: X = charlie

Solutions: {X = bob}, {X = charlie}
```

---

## Neural-Symbolic Fusion

**Distinctive feature** of IPFRS (NOT in traditional Prolog):

```rust
pub enum InferenceMode {
    Symbolic,          // Pure logic only
    Hybrid(f32),       // Symbolic + semantic fallback
                       // threshold = 0.7
    Neural(Vec<f32>),  // Vector embedding space
}

pub struct ComputationGraph {
    nodes: Vec<Operation>,
    edges: Vec<(NodeId, NodeId)>,
    autograd: bool,
}

pub enum Operation {
    Unify { term1: Term, term2: Term },
    Embed { predicate: Predicate },         // → vector
    ConvexCombination { w1: f32, w2: f32 }, // Blend results
}
```

### Hybrid Mode Example

```
Query: "Prove father(bob, X)"

Mode: Hybrid(threshold = 0.7)

Step 1: Try symbolic inference
  No matching facts/rules
  → Symbolic returns: []

Step 2: No solutions from pure logic
  Activate semantic fallback

Step 3: Embed predicate
  Encode "father" → [0.2, -0.1, 0.5, ...]
  Semantic search in HNSW index
  → Results: [
       (parent(bob, charlie), similarity: 0.92),
       (related_to(bob, diana), similarity: 0.75),
       (friend(bob, eve), similarity: 0.61)  ← below threshold
     ]

Step 4: Return candidates with scores
  Solutions:
    - father(bob, charlie) [confidence: 0.92]
    - related_to(bob, diana) [confidence: 0.75]

User gets both symbolic facts AND semantic approximations!
```

---

## Knowledge Base Management

```rust
pub struct KnowledgeBase {
    facts: Arc<Vec<Fact>>,
    rules: Arc<Vec<Rule>>,
    storage: Arc<dyn BlockStore>,  // Persistent store
}

impl KnowledgeBase {
    pub async fn add_fact(&mut self, fact: Fact) -> Result<Cid> {
        // 1. Serialize to IPLD
        let ipld = serde_ipld::to_ipld(&fact)?;
        
        // 2. Store in Storage domain
        let block = Block::from_ipld(ipld)?;
        let cid = self.storage.put(&block).await?;
        
        // 3. Add to in-memory index
        self.facts.push(fact);
        
        Ok(cid)
    }
    
    pub async fn load_from_storage(&mut self, cid: Cid) -> Result<()> {
        // Reconstruct knowledge base from content-addressed blocks
        let block = self.storage.get(&cid).await?;
        let ipld = serde_ipld::from_bytes(&block.data)?;
        let kb = serde_ipld::from_ipld(&ipld)?;
        *self = kb;
        Ok(())
    }
}
```

---

## Metrics & Performance

| Operation | Latency | Notes |
|-----------|---------|-------|
| Simple query (1 fact) | <1ms | Direct fact lookup |
| Recursive query (5 depth) | 1-5ms | Depends on branching |
| Unification | <1µs | O(size of terms) |
| Backward chaining search | Variable | Exponential in worst case |
| Load KB from storage | ~100ms | Parse IPLD blocks |

**Complexity**:
```
Worst case: O(2^d) where d = rule depth
Typical case: O(d * b^d) where b = branching factor
With max_depth = 1000: prevents infinite loops
```

---

## Storage Integration

Facts and rules are **content-addressed**:

```
┌─ Logic ───────────────┐
│ fact: parent(a, b)    │
└────────┬──────────────┘
         │
┌────────▼──────────────────┐
│ Serialize to IPLD         │
│ {                         │
│   "type": "fact",         │
│   "predicate": {          │
│     "name": "parent",     │
│     "args": ["a", "b"]    │
│   }                       │
│ }                         │
└────────┬──────────────────┘
         │
┌────────▼──────────────────┐
│ Storage: put(block)       │
│ CID = hash(IPLD)          │
└────────┬──────────────────┘
         │
         ↓
    CID = bafybeig...
    
Now other nodes can:
  - Request this fact by CID
  - Verify: hash(retrieved) == CID
  - Use in their own inferences
```

---

## Взаимодействие с другими доменами

### Logic → Storage
```
Persist rule: ancestor(X,Z) :- parent(X,Y), ancestor(Y,Z)
→ Serialize, compute CID
→ Other nodes load by CID
```

### Logic ← Semantic
```
If symbolic inference returns ∅:
  Fallback to semantic similarity
  Embed predicates, search HNSW
  Return candidate facts
```

### Logic → Application
```
Application: query_logic(ancestor(alice, X))
Logic: infer(...) → {X=bob}, {X=charlie}, ...
Return to user
```

---

## Важные свойства

| Свойство | Значение |
|----------|----------|
| **Deterministic** | Same input → same solutions (modulo order) |
| **Decidable** | Max depth prevents infinite loops |
| **Compositional** | Rules can call other rules |
| **Content-Addressed** | Facts stored as blocks with CID |
| **Hybrid** | Pure logic + semantic fallback |

---

## Что дальше?

→ [03-Bounded Contexts](03-BoundedContexts.md) для обзора  
→ [06-SemanticDomain](06-SemanticDomain.md) для fallback механизма  
→ [09-Data Flows](09-DataFlows.md) для сценария "Logic query"  
→ `/Volumes/Kingston/cool-japan/Vendor/ipfrs/crates/ipfrs-tensorlogic/` для кода

---

**Связанные**: [02-Architecture Stack](02-ArchitectureStack.md) | [03-Bounded Contexts](03-BoundedContexts.md) | [06-SemanticDomain](06-SemanticDomain.md) | [09-Data Flows](09-DataFlows.md)
