---
title: 07-TensorLogicContext
type: domain
summary: TensorLogic Context (ipfrs-tensorlogic) — нейро-символический Core Domain; полный каталог 20+ движков вывода, автоград, FedAvg, CID-адресуемые правила
tags: [ipfrs, ddd, tensorlogic, inference, neural-symbolic, autograd, federated-learning]
source: ipfrs_source/crates/ipfrs-tensorlogic/src/
related: ["[[03-SharedKernel]]", "[[06-SemanticContext]]", "[[11-RealityCheck]]"]
read_time: 18 мин
updated: 2026-06-19
---

# TensorLogic Context — `ipfrs-tensorlogic`

**Краткое резюме**: Самый большой контекст IPFRS (≈156K строк) и его главная
уникальная ценность. **Нейро-символический**: соединяет тензорные вычисления
(автоград, как мини-DL-фреймворк) с **20+ движками логического и вероятностного
вывода**. Шов между мирами — **CID**: логический `Term` может ссылаться на
тензор-блок по CID, а сами правила/факты/KB контент-адресуемы.

> ⚠️ **Это крупнейший пробел старых вики**: они документируют 2-3 движка (SLD,
> Tabling, Temporal). Реальный каталог — **20+ движков**. Ниже — полный список,
> выверенный по коду.

---

## 1. Двойная модель (два IR, сходящиеся на контент-адресации)

### Символический IR
```rust
// ir.rs:13
enum Term { Var(String), Const(Constant), Fun(name, args), Ref(TermRef) }
// TermRef { cid: Cid, hint } — мост символика → контент-адрес (ir.rs:39)
struct Predicate { name, args: Vec<Term> }          // ir.rs:165
struct Rule { head: Predicate, body: Vec<Predicate> } // хорн-клауза; факт = пустое тело (ir.rs:217)
struct KnowledgeBase { facts, rules }               // символический агрегат (ir.rs:277)
```
> `Constant::Float` хранится как `String` (`ir.rs:34`) — «для детерминированного
> хеширования». Это и делает контент-адресацию правил стабильной между машинами (I7).

### Численный IR
- `AutogradGraph` (`autograd.rs:52`) — скалярный reverse-mode AD (`f64`, 7 операций).
- `ComputationGraph` (`computation_graph.rs:511`) — DAG над `TensorOp` (~40 операций:
  MatMul, Einsum, Softmax, LayerNorm, fused-ops).

### Шов нейро-символической фузии (3 механизма)
1. **IPLD-term `Tensor`** (`ipld_codec.rs:42`): `TermIpld::Tensor { dtype, shape, cid }`
   — логический терм *является* дескриптором тензора, чьи байты лежат по отдельному CID.
   Буквальное место встраивания численного в символический IR.
2. **`NeuralSymbolicIntegrator`** (`neural_symbolic.rs:227`): гибридная смесь
   ```rust
   // neural_symbolic.rs:487 — Hybrid { neural_weight }
   nw * neural + (1.0 - nw) * symbolic
   ```
   возвращает `neural_contribution` и `symbolic_contribution` раздельно (объяснимость).
3. **CID-адресуемые правила**, скармливаемые распределённому чейнеру (§4).

---

## 2. Полный каталог движков вывода (20+)

Каждый подтверждён в коде. ✅ = реализован.

| # | Движок | Что вычисляет | Источник |
|---|--------|---------------|----------|
| 1 | **SLD backward chaining** | целенаправленная резолюция + унификация + детект циклов | `reasoning.rs:259,555` ✅ |
| 2 | Memoized SLD | SLD с мемоизацией подцелей | `reasoning.rs:632` ✅ |
| 3 | **Tabling / SLG** | табличная резолюция, детект петель в рекурсии | `recursive_reasoning.rs:101` ✅ |
| 4 | Stratified fixpoint (Datalog) | bottom-up итерация до сходимости | `recursive_reasoning.rs:312` ✅ |
| 5 | **Distributed backward chaining** | 3 стадии: локальные факты → локальные правила → делегирование пирам по CID | `distributed_backward_chainer.rs:66` ✅ |
| 6 | **Temporal (Allen)** | 13 интервальных отношений Аллена + проверка ограничений | `temporal_reasoning.rs:100` ✅ |
| 7 | **Fuzzy logic** | Mamdani/Sugeno, дефаззификация Centroid/MoM/LoM | `fuzzy_logic.rs:325` ✅ |
| 8 | Fuzzy (полный Mamdani) | деревья антецедентов, 7 MF, 5 методов дефаззификации | `fuzzy_logic_engine/` ✅ |
| 9 | **Epistemic S5 (Kripke)** | model-checking над конечной структурой Крипке; общее знание через fixpoint | `epistemic_logic.rs:210` ✅ |
| 10 | **PLN** | неопределённые truth values `(strength, confidence)`, 8 правил | `probabilistic_logic_network.rs:483` ✅ |
| 11 | **Bayesian network** | variable elimination / belief propagation / sampling | `bayesian_network_inference.rs:575` ✅ |
| 12 | Bayesian updater | сопряжённые приоры (Beta-Bernoulli, Gauss-Gauss, Dirichlet, Gamma-Poisson) | `bayesian_updater.rs` ✅ |
| 13 | **Abductive** | branch-and-bound по подмножествам гипотез, минимизация стоимости | `abductive_reasoning_engine.rs:392` ✅ |
| 14 | **Causal (do-calculus)** | do-исчисление Пёрла над гауссовой SCM; интервенции + контрфактика | `causal_inference.rs:244` ✅ |
| 15 | **Constraint solver (CSP)** | AC-3 propagation + backtracking + MRV | `constraint_solver.rs:322` ✅ |
| 16 | **Belief revision (AGM)** | expansion/contraction/Levi-revision с entrenchment | `belief_revision_engine.rs:336` ✅ |
| 17 | Constraint propagation | сужение доменов (отдельно от CSP-solver) | `constraint_propagation_engine.rs` ✅ |
| 18 | **MDP** | value/policy iteration, Беллман `V(s)=max_a Σ P[R+γV(s')]` | `markov_decision_process/` ✅ |
| 19 | **RL agent** | SARSA/Q-Learning/Expected-SARSA/Double-Q/N-step; ε-greedy/Boltzmann/UCB | `reinforcement_learning_agent.rs:52` ✅ |
| 20 | RL (alt tabular) | вторая, простая табличная реализация | `reinforcement_learner.rs` ✅ |
| 21 | Hypothesis testing | частотные тесты (chi², t-test) | `hypothesis_test_engine.rs` ✅ |
| 22 | Probabilistic program | байесовский sampling апостериори | `probabilistic_program_engine.rs` ✅ |
| 23 | **Neural-symbolic** | гибридная смесь (§1) | `neural_symbolic.rs:474` ✅ |
| 24 | Decision-tree learner | ID3/C4.5 с прунингом | `decision_tree_learner.rs` ✅ |
| 25 | Ensemble learner | Bagging/AdaBoost/GradBoost/RandomForest/Stacking | `ensemble_learner.rs` ✅ |

> №24-25 — индуктивные *обучатели*, а не дедуктивные *движки*, но живут в этом
> контексте. ⚠️ Нет единого `trait InferenceEngine`, который реализуют все движки;
> `reasoning::InferenceEngine` — конкретная структура, не полиморфная абстракция.

---

## 3. Корневые агрегаты

| Агрегат | Идентичность | Граница инвариантов | Источник |
|---------|--------------|---------------------|----------|
| **KnowledgeBase** | контент-адресуемый (`kb_to_block`) | владеет `facts` + `rules`; мутации через `add_fact`/`add_rule` | `ir.rs:277`, `ipld_codec.rs:157` |
| **ComputationGraph** | `cid: Option<Cid>` | граф ацикличен (валидируется) | `computation_graph.rs:511,761` |
| **AutogradGraph** | монотонный `next_id` | градиенты валидны только после `backward()` | `autograd.rs:52` |
| **Model / Checkpoint** | `Cid` / `CheckpointId` | автономный снимок параметров + состояния оптимизатора | `checkpoint_manager.rs` |
| **ProofTree** | корневой `ProofNode` | каждый узел ссылается на правило/факт | `proof_tree.rs` |

> KB намеренно **не** проверяет согласованность при построении (`add_fact` просто
> push, `ir.rs:292`). Согласованность — забота сервисов (belief revision, rule
> conflict resolver). Это сознательный «open-world» выбор: KB может хранить `p` и `¬p`.

---

## 4. Доменные сервисы

| Сервис | Точка входа | Источник |
|--------|-------------|----------|
| Диспетчер вывода | `InferenceEngine::query` | `reasoning.rs:259` |
| Распределённая сборка доказательства | `DistributedBackwardChainer::prove_with_tree` | `distributed_backward_chainer.rs:66` |
| Обратный проход (автоград) | `AutogradGraph::backward(out)` — seed grad=1.0, обратный топосорт, цепное правило | `autograd.rs:152` |
| Оптимизация графа | CSE, constant folding, fusion, DCE | `computation_graph.rs:1066` |
| Шаг оптимизатора | Adam/AdamW/AdaGrad/RMSProp | `adaptive_optimizer.rs:279` |
| **FedAvg** | `federated_average(&[Vec<f32>])` — невзвешенное среднее | `gradient/backward_pass.rs:19` |
| Распределённая агрегация градиентов | `DistributedGradientAccumulator::aggregate` | `gradient/federated.rs:1125` |
| Раундовый консенсус | кворумное голосование commit/abort | `consensus.rs` |

**Корректность автограда (I8)**: `accumulate_grad` *складывает* (не присваивает)
градиент (`autograd.rs:217`), поэтому общее подвыражение получает сумму восходящих
градиентов — фундаментальный инвариант reverse-mode AD. Обратный топопорядок
гарантирует полную аккумуляцию до чтения.

---

## 5. Интеграция

- **Shared Kernel**: `Cid` протянут через 20+ файлов — lingua franca с Storage/Network.
  `rule_to_block`/`kb_to_block` кодируют символику в DAG-CBOR `Block` для шеринга по IPFS.
- **Федеративное обучение по сети**: `gradient/federated.rs` собирает CID-обновления
  пиров через broadcast-канал с дедлайном; `consensus.rs` добавляет кворум раунда.
- **Semantic**: мост через `solver.rs` ([[06-SemanticContext]] §7.1).

---

## 6. Что реально работает, а что заглушка

| Подсистема | Статус |
|------------|--------|
| 20+ движков вывода (см. §2) | ✅ реализованы |
| Скалярный автоград + обратный проход | ✅ работает |
| FedAvg / агрегация градиентов | ✅ работает |
| CID-адресуемые правила (детерминизм) | ✅ работает |
| **Распределённое исполнение графа** | ⚠️ заглушка: `execute_distributed` → `Err("requires ipfrs-network integration")` (`computation_graph.rs:1667`) |
| Тензорный автоград (backward над `TensorOp`) | ⚠️ отсутствует: `ComputationGraph` только forward; градиенты — рукописные per-layer |
| FedAvg при таймауте кворума | ⚠️ `collect_updates` прерывается по таймауту и молча усредняет по меньшему кругу (`federated.rs:1014`) |

> ⚠️ **Опровержение гипотезы старых заметок**: файла `tensorlogic_ops.rs` и «бага
> FedAvg на строке 1131» **не существует**. Реальный FL-код — `gradient/federated.rs`
> и `gradient/backward_pass.rs`. Подробно — [[11-RealityCheck]].

---

## Что дальше?

- **Семантический мост** → [[06-SemanticContext]]
- **Сценарии INFER и FedAvg целиком** → [[10-DataFlows]]

**Связанные**: [[03-SharedKernel]] | [[06-SemanticContext]] | [[10-DataFlows]] | [[11-RealityCheck]]
**Источник кода**: `ipfrs-tensorlogic/src/{ir,reasoning,recursive_reasoning,neural_symbolic,ipld_codec,autograd,computation_graph,distributed_backward_chainer,epistemic_logic,probabilistic_logic_network,bayesian_network_inference,gradient/}.rs`
