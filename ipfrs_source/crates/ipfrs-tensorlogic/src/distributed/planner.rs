//! Build an ordered pipeline of self-contained stages from a node→peer placement
//! (RoadMap Phase 5.1, Spike 2c — "auto-`PipelineStage` from partitions").
//!
//! [`super::transport::execute_pipeline`] consumes a `Vec<PipelineStage>` that, so
//! far, the caller assembled by hand. This module derives that list automatically
//! from a *placement* (which peer owns each graph node):
//!
//! * each peer becomes one stage whose subgraph contains its own nodes plus an
//!   `Input` placeholder for every activation produced elsewhere — so the subgraph
//!   is **self-contained and independently executable** by `execute_stage`;
//! * stages are emitted in dependency order (a stage runs only after the stages
//!   producing its inputs), via a topological sort of the peer-level graph;
//! * graph inputs are sourced from the pipeline's `initial` map, so they never
//!   create a false stage dependency.
//!
//! Unlike the legacy round-robin [`DistributedExecutor`](crate::computation_graph),
//! the subgraphs here carry the boundary `Input` placeholders the numeric engine
//! needs, and a contiguous placement yields clean pipeline parallelism.

use std::collections::{HashMap, HashSet, VecDeque};

use crate::computation_graph::{ComputationGraph, GraphError, GraphNode, TensorOp};

use super::transport::PipelineStage;

fn push_unique(
    vecs: &mut HashMap<String, Vec<String>>,
    seen: &mut HashMap<String, HashSet<String>>,
    key: &str,
    val: &str,
) {
    if seen.entry(key.to_string()).or_default().insert(val.to_string()) {
        vecs.entry(key.to_string()).or_default().push(val.to_string());
    }
}

/// Build an ordered, self-contained pipeline from `placement` (node id → peer).
///
/// Every node of `graph` must have a placement. Returns the stages in execution
/// order, ready for [`execute_pipeline`](super::transport::execute_pipeline).
///
/// # Errors
/// - [`GraphError::InvalidGraph`] if a node lacks a placement, or if the placement
///   induces a cycle at the peer level (no valid stage ordering exists).
pub fn plan_pipeline(
    graph: &ComputationGraph,
    placement: &HashMap<String, String>,
) -> Result<Vec<PipelineStage>, GraphError> {
    let topo = graph.topological_sort()?;
    for id in &topo {
        if !placement.contains_key(id) {
            return Err(GraphError::InvalidGraph(format!(
                "plan_pipeline: node '{id}' has no placement"
            )));
        }
    }

    let graph_inputs: HashSet<&String> = graph.inputs.iter().collect();
    let graph_outputs: HashSet<&String> = graph.outputs.iter().collect();

    // Distinct peers in first-appearance (topological) order, for determinism.
    let mut stage_order: Vec<String> = Vec::new();
    let mut peers_seen: HashSet<String> = HashSet::new();
    for id in &topo {
        let p = &placement[id];
        if peers_seen.insert(p.clone()) {
            stage_order.push(p.clone());
        }
    }

    // Peer-level dependency DAG + per-stage boundary inputs / outputs.
    let mut indeg: HashMap<String, usize> = stage_order.iter().map(|s| (s.clone(), 0)).collect();
    let mut adj: HashMap<String, Vec<String>> = HashMap::new();
    let mut edges: HashSet<(String, String)> = HashSet::new();
    let mut boundary: HashMap<String, Vec<String>> = HashMap::new();
    let mut boundary_seen: HashMap<String, HashSet<String>> = HashMap::new();
    let mut outputs: HashMap<String, Vec<String>> = HashMap::new();
    let mut outputs_seen: HashMap<String, HashSet<String>> = HashMap::new();

    for id in &topo {
        let node = &graph.nodes[id];
        let stage = placement[id].clone();

        if graph_outputs.contains(id) {
            push_unique(&mut outputs, &mut outputs_seen, &stage, id);
        }
        // A graph input that lives on this stage is fed from `initial`.
        if graph_inputs.contains(id) {
            push_unique(&mut boundary, &mut boundary_seen, &stage, id);
        }

        for op in &node.inputs {
            if graph_inputs.contains(op) {
                // Boundary input sourced from `initial`; no stage dependency.
                push_unique(&mut boundary, &mut boundary_seen, &stage, op);
            } else {
                let prod = placement[op].clone();
                if prod != stage {
                    // Cross-stage activation: boundary in for us, output for prod.
                    push_unique(&mut boundary, &mut boundary_seen, &stage, op);
                    push_unique(&mut outputs, &mut outputs_seen, &prod, op);
                    if edges.insert((prod.clone(), stage.clone())) {
                        adj.entry(prod.clone()).or_default().push(stage.clone());
                        *indeg.entry(stage.clone()).or_insert(0) += 1;
                    }
                }
            }
        }
    }

    // Kahn topological order over the peer DAG (stable: seeds in first-seen order).
    let mut working = indeg.clone();
    let mut queue: VecDeque<String> = stage_order
        .iter()
        .filter(|s| working[*s] == 0)
        .cloned()
        .collect();
    let mut order = Vec::with_capacity(stage_order.len());
    while let Some(s) = queue.pop_front() {
        order.push(s.clone());
        if let Some(neighbours) = adj.get(&s) {
            for n in neighbours {
                let d = working.get_mut(n).expect("indegree entry exists");
                *d -= 1;
                if *d == 0 {
                    queue.push_back(n.clone());
                }
            }
        }
    }
    if order.len() != stage_order.len() {
        return Err(GraphError::InvalidGraph(
            "plan_pipeline: placement induces a cyclic stage graph".to_string(),
        ));
    }

    // Assemble one PipelineStage per peer, in dependency order.
    let mut stages = Vec::with_capacity(order.len());
    for stage in &order {
        let own_ids: Vec<&String> = topo.iter().filter(|id| placement[*id] == *stage).collect();
        let own_set: HashSet<&String> = own_ids.iter().copied().collect();
        let bvec = boundary.get(stage).cloned().unwrap_or_default();
        let ovec = outputs.get(stage).cloned().unwrap_or_default();

        let mut sub = ComputationGraph::new();
        // Boundary placeholders for activations produced elsewhere (or graph
        // inputs not represented as an own node) come first, so own nodes that
        // consume them pass `add_node`'s operand-existence check.
        for bid in &bvec {
            if !own_set.contains(bid) {
                sub.add_node(GraphNode::new(
                    bid.clone(),
                    TensorOp::Input { name: bid.clone() },
                ))?;
                sub.mark_input(bid.clone());
            }
        }
        // Own nodes in global topological order (operands precede dependents).
        for id in &own_ids {
            sub.add_node(graph.nodes[*id].clone())?;
        }
        // Own graph-input nodes are also fed from `initial`.
        for bid in &bvec {
            if own_set.contains(bid) {
                sub.mark_input(bid.clone());
            }
        }
        for oid in &ovec {
            sub.mark_output(oid.clone());
        }

        stages.push(PipelineStage::new(stage.clone(), sub, bvec, ovec));
    }

    Ok(stages)
}

/// Convenience: split `graph` into `peers.len()` contiguous topological chunks
/// (one stage per peer) and plan the pipeline. This is the natural placement for
/// pipeline parallelism — consecutive layers stay together.
///
/// # Errors
/// - [`GraphError::InvalidGraph`] if `peers` is empty.
pub fn plan_pipeline_contiguous(
    graph: &ComputationGraph,
    peers: &[String],
) -> Result<Vec<PipelineStage>, GraphError> {
    if peers.is_empty() {
        return Err(GraphError::InvalidGraph(
            "plan_pipeline_contiguous: no peers".to_string(),
        ));
    }
    let topo = graph.topological_sort()?;
    let n = topo.len();
    let k = peers.len();
    let base = n / k;
    let rem = n % k;

    let mut placement = HashMap::with_capacity(n);
    let mut idx = 0usize;
    for (pi, peer) in peers.iter().enumerate() {
        let len = base + if pi < rem { 1 } else { 0 };
        for _ in 0..len {
            if idx < n {
                placement.insert(topo[idx].clone(), peer.clone());
                idx += 1;
            }
        }
    }
    while idx < n {
        placement.insert(topo[idx].clone(), peers[k - 1].clone());
        idx += 1;
    }

    plan_pipeline(graph, &placement)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::computation_graph::TensorOp;
    use crate::distributed::transport::{execute_pipeline, LocalTransport};
    use crate::distributed::wire::WireTensor;
    use crate::numeric_exec::{self, NumTensor};

    fn input(id: &str) -> GraphNode {
        GraphNode::new(id.to_string(), TensorOp::Input { name: id.to_string() })
    }

    /// y = silu(relu(x + b) * w) — a short chain that splits cleanly into stages.
    fn chain_graph() -> ComputationGraph {
        let mut g = ComputationGraph::new();
        for id in ["x", "b", "w"] {
            g.add_node(input(id)).unwrap();
            g.mark_input(id.to_string());
        }
        g.add_node(
            GraphNode::new("s".into(), TensorOp::Add)
                .add_input("x".into())
                .add_input("b".into()),
        )
        .unwrap();
        g.add_node(GraphNode::new("h".into(), TensorOp::ReLU).add_input("s".into()))
            .unwrap();
        g.add_node(
            GraphNode::new("m".into(), TensorOp::Mul)
                .add_input("h".into())
                .add_input("w".into()),
        )
        .unwrap();
        g.add_node(GraphNode::new("y".into(), TensorOp::SiLU).add_input("m".into()))
            .unwrap();
        g.mark_output("y".into());
        g
    }

    fn initial() -> HashMap<String, WireTensor> {
        let mut m = HashMap::new();
        m.insert("x".into(), WireTensor { data: vec![1.0, -3.0], shape: vec![1, 2] });
        m.insert("b".into(), WireTensor { data: vec![2.0, 1.0], shape: vec![1, 2] });
        m.insert("w".into(), WireTensor { data: vec![2.0, 5.0], shape: vec![1, 2] });
        m
    }

    fn oracle() -> NumTensor {
        let inputs: HashMap<String, NumTensor> = initial()
            .into_iter()
            .map(|(k, v)| (k, v.into_num().unwrap()))
            .collect();
        numeric_exec::execute_output(&chain_graph(), &inputs, "y").unwrap()
    }

    #[tokio::test]
    async fn contiguous_pipeline_matches_monolithic() {
        let g = chain_graph();
        let peers = vec!["p0".to_string(), "p1".to_string(), "p2".to_string()];
        let stages = plan_pipeline_contiguous(&g, &peers).unwrap();
        assert!(stages.len() >= 2, "expected multiple stages");

        let env = execute_pipeline(&stages, &LocalTransport, initial())
            .await
            .unwrap();
        assert_eq!(env["y"].data, oracle().data);
    }

    #[test]
    fn explicit_placement_builds_self_contained_subgraphs() {
        let g = chain_graph();
        // Stage A: inputs + add + relu; Stage B: mul + silu.
        let mut placement = HashMap::new();
        for id in ["x", "b", "s", "h"] {
            placement.insert(id.to_string(), "A".to_string());
        }
        for id in ["w", "m", "y"] {
            placement.insert(id.to_string(), "B".to_string());
        }
        let stages = plan_pipeline(&g, &placement).unwrap();
        assert_eq!(stages.len(), 2);
        assert_eq!(stages[0].peer, "A");
        assert_eq!(stages[1].peer, "B");
        // A outputs h (consumed by B); B needs h as a boundary input.
        assert!(stages[0].output_ids.contains(&"h".to_string()));
        assert!(stages[1].input_ids.contains(&"h".to_string()));
        // B's subgraph carries an Input placeholder for h.
        assert!(matches!(
            stages[1].graph.nodes["h"].op,
            TensorOp::Input { .. }
        ));
    }

    #[test]
    fn missing_placement_errors() {
        let g = chain_graph();
        let mut placement = HashMap::new();
        placement.insert("x".to_string(), "A".to_string()); // incomplete
        assert!(matches!(
            plan_pipeline(&g, &placement),
            Err(GraphError::InvalidGraph(_))
        ));
    }

    #[test]
    fn cyclic_placement_errors() {
        // Two nodes that depend on each other across stages would cycle; build a
        // placement where A needs B's output and B needs A's output.
        let mut g = ComputationGraph::new();
        g.add_node(input("x")).unwrap();
        g.mark_input("x".into());
        g.add_node(GraphNode::new("a".into(), TensorOp::ReLU).add_input("x".into()))
            .unwrap();
        g.add_node(GraphNode::new("b".into(), TensorOp::ReLU).add_input("a".into()))
            .unwrap();
        g.add_node(GraphNode::new("c".into(), TensorOp::Add).add_input("a".into()).add_input("b".into()))
            .unwrap();
        g.mark_output("c".into());
        // Force a cross-stage cycle: a on S1, b on S2, c on S1 (S1 needs b from S2,
        // S2 needs a from S1) — acyclic actually. Construct a real cycle instead:
        // put a,c on S1 and b on S2; edges S1->S2 (a->b) and S2->S1 (b->c) => cycle.
        let mut placement = HashMap::new();
        placement.insert("x".into(), "S1".to_string());
        placement.insert("a".into(), "S1".to_string());
        placement.insert("b".into(), "S2".to_string());
        placement.insert("c".into(), "S1".to_string());
        assert!(matches!(
            plan_pipeline(&g, &placement),
            Err(GraphError::InvalidGraph(_))
        ));
    }
}
