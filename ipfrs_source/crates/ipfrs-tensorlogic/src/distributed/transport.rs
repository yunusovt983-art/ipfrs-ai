//! Transport-agnostic driver for multi-stage distributed execution
//! (RoadMap Phase 5.1 — orchestration half of "stream activations").
//!
//! This is the **dependency-inversion seam** that keeps `ipfrs-tensorlogic` free of
//! any networking dependency (and so free of the `network ↔ tensorlogic` cycle):
//! the orchestrator drives a pipeline of [`StageRequest`]s through an abstract
//! [`ActivationTransport`], and the real libp2p implementation lives in the network
//! / node layer that *depends on* this crate. A [`LocalTransport`] runs every stage
//! in-process — both a single-node fallback and the test double that lets the whole
//! orchestration be exercised without a swarm.

use std::collections::HashMap;

use async_trait::async_trait;

use crate::computation_graph::{ComputationGraph, GraphError};

use super::wire::{execute_stage, StageRequest, StageResponse, WireTensor};

/// Moves a [`StageRequest`] to the peer that owns a partition and returns its
/// [`StageResponse`]. Implemented over libp2p in the node layer; over direct calls
/// by [`LocalTransport`].
#[async_trait]
pub trait ActivationTransport: Send + Sync {
    /// Execute `req` on `peer` and return its response.
    async fn run_stage(&self, peer: &str, req: StageRequest) -> Result<StageResponse, GraphError>;
}

/// One stage of a distributed pipeline: a subgraph pinned to a peer, the activation
/// ids it consumes from upstream, and the ids it produces for downstream stages.
#[derive(Debug, Clone)]
pub struct PipelineStage {
    /// Peer that executes this stage.
    pub peer: String,
    /// Subgraph for this stage (its boundary `Input` nodes are fed from `input_ids`).
    pub graph: ComputationGraph,
    /// Activation ids this stage needs, looked up in the running environment.
    pub input_ids: Vec<String>,
    /// Node ids this stage produces and publishes back to the environment.
    pub output_ids: Vec<String>,
}

impl PipelineStage {
    /// Construct a pipeline stage.
    pub fn new(
        peer: impl Into<String>,
        graph: ComputationGraph,
        input_ids: Vec<String>,
        output_ids: Vec<String>,
    ) -> Self {
        Self {
            peer: peer.into(),
            graph,
            input_ids,
            output_ids,
        }
    }
}

/// Drive `stages` in order through `transport`, threading activations between them.
///
/// Execution starts from `initial` (the graph's external inputs). Each stage pulls
/// its `input_ids` from the running environment, runs remotely, and merges its
/// outputs back in so later stages can consume them. Returns the full environment
/// of every produced activation; the caller plucks the graph's final outputs.
///
/// Fails fast: a missing input, a transport error, or a remote stage error stops
/// the pipeline and is returned to the caller.
pub async fn execute_pipeline<T: ActivationTransport + ?Sized>(
    stages: &[PipelineStage],
    transport: &T,
    initial: HashMap<String, WireTensor>,
) -> Result<HashMap<String, WireTensor>, GraphError> {
    let mut env = initial;

    for (i, stage) in stages.iter().enumerate() {
        let mut inputs = HashMap::with_capacity(stage.input_ids.len());
        for id in &stage.input_ids {
            let val = env.get(id).cloned().ok_or_else(|| {
                GraphError::MissingInput(format!(
                    "stage {} (peer {}): activation '{}' not yet available",
                    i, stage.peer, id
                ))
            })?;
            inputs.insert(id.clone(), val);
        }

        let req = StageRequest::new(stage.graph.clone(), inputs, stage.output_ids.clone());
        let resp = transport.run_stage(&stage.peer, req).await?;
        if let Some(err) = resp.error {
            return Err(GraphError::ExecutionError(format!(
                "stage {} (peer {}) failed: {}",
                i, stage.peer, err
            )));
        }
        env.extend(resp.outputs);
    }

    Ok(env)
}

/// In-process transport: executes every stage locally via [`execute_stage`],
/// ignoring the peer id. Serves as the single-node fallback and the test double for
/// the orchestrator. The `StageRequest` is round-tripped through its serde form so
/// that wire-encoding bugs surface even without a network.
#[derive(Debug, Default, Clone)]
pub struct LocalTransport;

#[async_trait]
impl ActivationTransport for LocalTransport {
    async fn run_stage(&self, _peer: &str, req: StageRequest) -> Result<StageResponse, GraphError> {
        // Faithfully simulate the wire: encode and decode before executing.
        let bytes = serde_json::to_vec(&req)
            .map_err(|e| GraphError::ExecutionError(format!("encode stage request: {e}")))?;
        let decoded: StageRequest = serde_json::from_slice(&bytes)
            .map_err(|e| GraphError::ExecutionError(format!("decode stage request: {e}")))?;
        Ok(execute_stage(&decoded))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::computation_graph::{GraphNode, TensorOp};
    use crate::numeric_exec;
    use crate::numeric_exec::NumTensor;

    fn input(id: &str) -> GraphNode {
        GraphNode::new(id.to_string(), TensorOp::Input { name: id.to_string() })
    }

    fn wt(data: Vec<f32>, shape: Vec<usize>) -> WireTensor {
        WireTensor { data, shape }
    }

    /// Stage 1: h = relu(x + b1).  Boundary inputs: x, b1.  Output: h.
    fn stage1_graph() -> ComputationGraph {
        let mut g = ComputationGraph::new();
        g.add_node(input("x")).unwrap();
        g.add_node(input("b1")).unwrap();
        g.mark_input("x".to_string());
        g.mark_input("b1".to_string());
        g.add_node(
            GraphNode::new("s1".to_string(), TensorOp::Add)
                .add_input("x".to_string())
                .add_input("b1".to_string()),
        )
        .unwrap();
        g.add_node(GraphNode::new("h".to_string(), TensorOp::ReLU).add_input("s1".to_string()))
            .unwrap();
        g.mark_output("h".to_string());
        g
    }

    /// Stage 2: y = h * b2.  Boundary inputs: h (from stage 1), b2.  Output: y.
    fn stage2_graph() -> ComputationGraph {
        let mut g = ComputationGraph::new();
        g.add_node(input("h")).unwrap();
        g.add_node(input("b2")).unwrap();
        g.mark_input("h".to_string());
        g.mark_input("b2".to_string());
        g.add_node(
            GraphNode::new("y".to_string(), TensorOp::Mul)
                .add_input("h".to_string())
                .add_input("b2".to_string()),
        )
        .unwrap();
        g.mark_output("y".to_string());
        g
    }

    /// The same computation as one monolithic graph, for an equivalence oracle.
    fn whole_graph() -> ComputationGraph {
        let mut g = ComputationGraph::new();
        g.add_node(input("x")).unwrap();
        g.add_node(input("b1")).unwrap();
        g.add_node(input("b2")).unwrap();
        g.mark_input("x".to_string());
        g.mark_input("b1".to_string());
        g.mark_input("b2".to_string());
        g.add_node(
            GraphNode::new("s1".to_string(), TensorOp::Add)
                .add_input("x".to_string())
                .add_input("b1".to_string()),
        )
        .unwrap();
        g.add_node(GraphNode::new("h".to_string(), TensorOp::ReLU).add_input("s1".to_string()))
            .unwrap();
        g.add_node(
            GraphNode::new("y".to_string(), TensorOp::Mul)
                .add_input("h".to_string())
                .add_input("b2".to_string()),
        )
        .unwrap();
        g.mark_output("y".to_string());
        g
    }

    #[tokio::test]
    async fn two_stage_pipeline_matches_monolithic_execution() {
        let stages = vec![
            PipelineStage::new(
                "peer-A",
                stage1_graph(),
                vec!["x".to_string(), "b1".to_string()],
                vec!["h".to_string()],
            ),
            PipelineStage::new(
                "peer-B",
                stage2_graph(),
                vec!["h".to_string(), "b2".to_string()],
                vec!["y".to_string()],
            ),
        ];

        let mut initial = HashMap::new();
        initial.insert("x".to_string(), wt(vec![1.0, -3.0], vec![1, 2]));
        initial.insert("b1".to_string(), wt(vec![2.0, 1.0], vec![1, 2]));
        initial.insert("b2".to_string(), wt(vec![10.0, 10.0], vec![1, 2]));

        let env = execute_pipeline(&stages, &LocalTransport, initial.clone())
            .await
            .unwrap();

        // Oracle: run the whole graph in one shot.
        let oracle_inputs: HashMap<String, NumTensor> = initial
            .into_iter()
            .map(|(k, v)| (k, v.into_num().unwrap()))
            .collect();
        let oracle = numeric_exec::execute_output(&whole_graph(), &oracle_inputs, "y").unwrap();

        assert_eq!(env["y"].data, oracle.data);
        // relu([1,-3]+[2,1]) = relu([3,-2]) = [3,0]; *[10,10] = [30,0]
        assert_eq!(env["y"].data, vec![30.0, 0.0]);
    }

    #[tokio::test]
    async fn missing_initial_input_fails_fast() {
        let stages = vec![PipelineStage::new(
            "peer-A",
            stage1_graph(),
            vec!["x".to_string(), "b1".to_string()],
            vec!["h".to_string()],
        )];
        // omit b1
        let mut initial = HashMap::new();
        initial.insert("x".to_string(), wt(vec![1.0], vec![1, 1]));
        let err = execute_pipeline(&stages, &LocalTransport, initial).await;
        assert!(matches!(err, Err(GraphError::MissingInput(_))));
    }

    #[tokio::test]
    async fn remote_stage_error_propagates() {
        // Stage requests output "h" but feeds a shape-mismatched add → remote error.
        let stages = vec![PipelineStage::new(
            "peer-A",
            stage1_graph(),
            vec!["x".to_string(), "b1".to_string()],
            vec!["h".to_string()],
        )];
        let mut initial = HashMap::new();
        initial.insert("x".to_string(), wt(vec![1.0, 2.0], vec![1, 2]));
        initial.insert("b1".to_string(), wt(vec![1.0], vec![1, 1])); // wrong shape
        let err = execute_pipeline(&stages, &LocalTransport, initial).await;
        assert!(matches!(err, Err(GraphError::ExecutionError(_))));
    }
}
