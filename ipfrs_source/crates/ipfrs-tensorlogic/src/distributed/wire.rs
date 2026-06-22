//! Wire format for streaming a single partition's work between peers
//! (RoadMap Phase 5.1 — "stream activations").
//!
//! A distributed graph runs one *stage* per peer. The owner of a stage ships the
//! consuming peer a self-describing [`StageRequest`] — the subgraph to run plus the
//! boundary activations it needs — and gets back a [`StageResponse`] with the
//! requested outputs. Everything here is pure, serde-serializable data and a single
//! local [`execute_stage`] entry point; no transport is involved (that is
//! [`super::transport`]), so a receiving peer's behaviour is fully unit-testable.
//!
//! Tensors cross the wire as [`WireTensor`] (a serde mirror of
//! [`NumTensor`](crate::numeric_exec::NumTensor), which is not itself serde).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::computation_graph::{ComputationGraph, GraphError};
use crate::numeric_exec::{self, NumTensor};

/// A dense row-major `f32` tensor in transit between peers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WireTensor {
    /// Row-major tensor elements.
    pub data: Vec<f32>,
    /// Tensor shape; `data.len()` must equal the product of `shape`.
    pub shape: Vec<usize>,
}

impl WireTensor {
    /// Borrow a [`NumTensor`] as wire data (clones the buffer).
    pub fn from_num(t: &NumTensor) -> Self {
        Self {
            data: t.data.clone(),
            shape: t.shape.clone(),
        }
    }

    /// Convert back to a validated [`NumTensor`].
    pub fn into_num(self) -> Result<NumTensor, GraphError> {
        NumTensor::new(self.data, self.shape)
    }
}

impl From<NumTensor> for WireTensor {
    fn from(t: NumTensor) -> Self {
        Self {
            data: t.data,
            shape: t.shape,
        }
    }
}

/// A request to execute one partition's subgraph on the receiving peer.
//
// No `PartialEq`: `ComputationGraph` does not implement it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageRequest {
    /// The subgraph this peer must execute. Its boundary `Input`/`Constant` nodes
    /// are supplied via [`inputs`](Self::inputs), keyed by node id.
    pub graph: ComputationGraph,
    /// Boundary activations, keyed by the id of the `Input`/`Constant` node they feed.
    pub inputs: HashMap<String, WireTensor>,
    /// Node ids whose computed values must be returned to the caller.
    pub outputs: Vec<String>,
}

impl StageRequest {
    /// Construct a stage request.
    pub fn new(
        graph: ComputationGraph,
        inputs: HashMap<String, WireTensor>,
        outputs: Vec<String>,
    ) -> Self {
        Self {
            graph,
            inputs,
            outputs,
        }
    }
}

/// The result of executing a [`StageRequest`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StageResponse {
    /// Requested node values, keyed by node id. Empty when `error` is set.
    pub outputs: HashMap<String, WireTensor>,
    /// Execution error, if the stage failed on the remote peer.
    pub error: Option<String>,
}

impl StageResponse {
    /// A successful response carrying `outputs`.
    pub fn ok(outputs: HashMap<String, WireTensor>) -> Self {
        Self {
            outputs,
            error: None,
        }
    }

    /// A failed response carrying an error message.
    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            outputs: HashMap::new(),
            error: Some(msg.into()),
        }
    }
}

/// Execute a stage locally — the work a peer performs on receipt of a
/// [`StageRequest`]. Numeric-execution errors are captured into
/// [`StageResponse::error`] rather than propagated, so a misbehaving subgraph
/// fails one transfer instead of the transport.
pub fn execute_stage(req: &StageRequest) -> StageResponse {
    match run_stage(req) {
        Ok(outputs) => StageResponse::ok(outputs),
        Err(e) => StageResponse::err(e.to_string()),
    }
}

fn run_stage(req: &StageRequest) -> Result<HashMap<String, WireTensor>, GraphError> {
    let inputs: HashMap<String, NumTensor> = req
        .inputs
        .iter()
        .map(|(k, v)| Ok((k.clone(), v.clone().into_num()?)))
        .collect::<Result<_, GraphError>>()?;

    let env = numeric_exec::execute(&req.graph, &inputs)?;

    let mut outputs = HashMap::with_capacity(req.outputs.len());
    for id in &req.outputs {
        let t = env
            .get(id)
            .ok_or_else(|| GraphError::NodeNotFound(id.clone()))?;
        outputs.insert(id.clone(), WireTensor::from_num(t));
    }
    Ok(outputs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::computation_graph::{GraphNode, TensorOp};

    fn input(id: &str) -> GraphNode {
        GraphNode::new(id.to_string(), TensorOp::Input { name: id.to_string() })
    }

    /// y = relu(a + b)
    fn add_relu_graph() -> ComputationGraph {
        let mut g = ComputationGraph::new();
        g.add_node(input("a")).unwrap();
        g.add_node(input("b")).unwrap();
        g.mark_input("a".to_string());
        g.mark_input("b".to_string());
        g.add_node(
            GraphNode::new("sum".to_string(), TensorOp::Add)
                .add_input("a".to_string())
                .add_input("b".to_string()),
        )
        .unwrap();
        g.add_node(GraphNode::new("y".to_string(), TensorOp::ReLU).add_input("sum".to_string()))
            .unwrap();
        g.mark_output("y".to_string());
        g
    }

    #[test]
    fn wire_tensor_roundtrips_through_numtensor() {
        let n = NumTensor::new(vec![1.0, 2.0, 3.0, 4.0], vec![2, 2]).unwrap();
        let w = WireTensor::from_num(&n);
        assert_eq!(w.into_num().unwrap(), n);
    }

    #[test]
    fn wire_tensor_serde_roundtrip() {
        let w = WireTensor {
            data: vec![1.0, -2.0, 0.5],
            shape: vec![3],
        };
        let bytes = serde_json::to_vec(&w).unwrap();
        let back: WireTensor = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(back, w);
    }

    #[test]
    fn execute_stage_runs_subgraph() {
        let mut inputs = HashMap::new();
        inputs.insert("a".to_string(), WireTensor { data: vec![1.0, -5.0], shape: vec![1, 2] });
        inputs.insert("b".to_string(), WireTensor { data: vec![2.0, 1.0], shape: vec![1, 2] });
        let req = StageRequest::new(add_relu_graph(), inputs, vec!["y".to_string()]);

        let resp = execute_stage(&req);
        assert!(resp.error.is_none());
        // relu(a+b) = relu([3, -4]) = [3, 0]
        assert_eq!(resp.outputs["y"].data, vec![3.0, 0.0]);
        assert_eq!(resp.outputs["y"].shape, vec![1, 2]);
    }

    #[test]
    fn stage_request_serde_roundtrip() {
        let mut inputs = HashMap::new();
        inputs.insert("a".to_string(), WireTensor { data: vec![1.0, 2.0], shape: vec![1, 2] });
        inputs.insert("b".to_string(), WireTensor { data: vec![3.0, 4.0], shape: vec![1, 2] });
        let req = StageRequest::new(add_relu_graph(), inputs, vec!["y".to_string()]);

        let bytes = serde_json::to_vec(&req).unwrap();
        let back: StageRequest = serde_json::from_slice(&bytes).unwrap();
        // Round-tripped request computes identically.
        assert_eq!(execute_stage(&back).outputs["y"].data, vec![4.0, 6.0]);
    }

    #[test]
    fn missing_input_reports_error_not_panic() {
        let req = StageRequest::new(add_relu_graph(), HashMap::new(), vec!["y".to_string()]);
        let resp = execute_stage(&req);
        assert!(resp.error.is_some());
        assert!(resp.outputs.is_empty());
    }

    #[test]
    fn unknown_output_id_reports_error() {
        let mut inputs = HashMap::new();
        inputs.insert("a".to_string(), WireTensor { data: vec![1.0], shape: vec![1, 1] });
        inputs.insert("b".to_string(), WireTensor { data: vec![1.0], shape: vec![1, 1] });
        let req = StageRequest::new(add_relu_graph(), inputs, vec!["does_not_exist".to_string()]);
        assert!(execute_stage(&req).error.is_some());
    }
}
