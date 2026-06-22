//! Minimal numeric executor for [`ComputationGraph`] (RoadMap Phase 5 foundation).
//!
//! The `ComputationGraph` is symbolic (ops + shapes, no data). This module adds a
//! small tree-walking interpreter that actually computes f32 tensors for a useful
//! subset of ops — enough to run a feed-forward layer (MatMul + Add + activation).
//! It is local-only and additive; distributed execution and the full op set are
//! follow-ups. Unsupported ops return `GraphError::ExecutionError`.

use std::collections::HashMap;

use crate::computation_graph::{ComputationGraph, GraphError, GraphNode, TensorOp};
use crate::numerics;
use crate::tl_executor::{ElemOp, NumExecutor, ReduceOp, TlExecutor};

/// A dense row-major f32 tensor.
#[derive(Debug, Clone, PartialEq)]
pub struct NumTensor {
    pub data: Vec<f32>,
    pub shape: Vec<usize>,
}

impl NumTensor {
    /// Build a tensor, validating that `data.len()` matches the shape's element count.
    pub fn new(data: Vec<f32>, shape: Vec<usize>) -> Result<Self, GraphError> {
        let n: usize = shape.iter().product();
        if data.len() != n {
            return Err(GraphError::ShapeMismatch(format!(
                "data len {} != shape {:?} ({} elems)",
                data.len(),
                shape,
                n
            )));
        }
        Ok(Self { data, shape })
    }

    fn unchecked(data: Vec<f32>, shape: Vec<usize>) -> Self {
        Self { data, shape }
    }
}

/// Execute `graph` numerically with the default [`NumExecutor`] backend, given
/// values for its `Input`/`Constant` nodes (keyed by node id). Returns the value
/// of every node, indexed by node id.
pub fn execute(
    graph: &ComputationGraph,
    inputs: &HashMap<String, NumTensor>,
) -> Result<HashMap<String, NumTensor>, GraphError> {
    execute_with(graph, inputs, &mut NumExecutor::new())
}

/// Execute `graph` with an explicit [`TlExecutor`] backend.
///
/// Core kernels — element-wise arithmetic / activations (`elem_op*`), matmul
/// (`einsum`) and reductions (`reduce`) — are delegated to `exec`, so a different
/// backend (GPU, SciRS2, …) can drive the same graph. Structural and axis-aware
/// ops the trait does not cover (`Reshape`, `Transpose`, `Pow`, `Softmax`,
/// `LayerNorm`) are evaluated directly on the dense `f32` representation.
pub fn execute_with<E>(
    graph: &ComputationGraph,
    inputs: &HashMap<String, NumTensor>,
    exec: &mut E,
) -> Result<HashMap<String, NumTensor>, GraphError>
where
    E: TlExecutor<Tensor = NumTensor, Error = GraphError>,
{
    let order = graph.topological_sort()?;
    let mut env: HashMap<String, NumTensor> = HashMap::new();
    for id in &order {
        let node = graph
            .nodes
            .get(id)
            .ok_or_else(|| GraphError::NodeNotFound(id.clone()))?;
        let val = eval_node(node, &env, inputs, exec)?;
        env.insert(id.clone(), val);
    }
    Ok(env)
}

/// Convenience: execute and return the value of a single output node.
pub fn execute_output(
    graph: &ComputationGraph,
    inputs: &HashMap<String, NumTensor>,
    output_id: &str,
) -> Result<NumTensor, GraphError> {
    let env = execute(graph, inputs)?;
    env.get(output_id)
        .cloned()
        .ok_or_else(|| GraphError::NodeNotFound(output_id.to_string()))
}

fn operand<'a>(
    node: &GraphNode,
    env: &'a HashMap<String, NumTensor>,
    i: usize,
) -> Result<&'a NumTensor, GraphError> {
    let id = node.inputs.get(i).ok_or_else(|| {
        GraphError::ExecutionError(format!("{}: missing operand #{}", node.id, i))
    })?;
    env.get(id).ok_or_else(|| GraphError::NodeNotFound(id.clone()))
}

fn unary(a: &NumTensor, f: impl Fn(f32) -> f32) -> NumTensor {
    NumTensor::unchecked(a.data.iter().map(|x| f(*x)).collect(), a.shape.clone())
}

/// Restore reduced axes as size-1 dims when an op requested `keepdims`.
fn apply_keepdims(
    reduced: NumTensor,
    orig_rank: usize,
    axes: &[usize],
    keepdims: bool,
) -> Result<NumTensor, GraphError> {
    if !keepdims {
        return Ok(reduced);
    }
    let shape = if axes.is_empty() {
        vec![1; orig_rank]
    } else {
        let mut s = reduced.shape.clone();
        // `reduced` already dropped the axes; re-insert 1s at each reduced position.
        let mut sorted = axes.to_vec();
        sorted.sort_unstable();
        for &ax in &sorted {
            if ax <= s.len() {
                s.insert(ax, 1);
            }
        }
        s
    };
    NumTensor::new(reduced.data, shape)
}

/// Apply a per-vector slice kernel along the last axis of a 1-D or 2-D tensor.
/// (Softmax / LayerNorm normalize over the trailing dimension, i.e. per row.)
fn map_last_axis(
    a: &NumTensor,
    f: impl Fn(&[f32]) -> Result<Vec<f32>, GraphError>,
) -> Result<NumTensor, GraphError> {
    match a.shape.len() {
        1 => NumTensor::new(f(&a.data)?, a.shape.clone()),
        2 => {
            let (rows, cols) = (a.shape[0], a.shape[1]);
            let mut out = Vec::with_capacity(a.data.len());
            for r in 0..rows {
                out.extend(f(&a.data[r * cols..(r + 1) * cols])?);
            }
            NumTensor::new(out, a.shape.clone())
        }
        _ => Err(GraphError::ExecutionError(format!(
            "op supports 1-D/2-D tensors only, got shape {:?}",
            a.shape
        ))),
    }
}

/// Softmax over the requested axis (negative axes count from the end).
fn softmax_op(a: &NumTensor, axis: i64) -> Result<NumTensor, GraphError> {
    let rank = a.shape.len() as i64;
    let ax = if axis < 0 { axis + rank } else { axis };
    match a.shape.len() {
        1 => NumTensor::new(numerics::softmax(&a.data), a.shape.clone()),
        2 if ax == 1 => map_last_axis(a, |row| Ok(numerics::softmax(row))),
        2 if ax == 0 => {
            // Softmax down each column.
            let (rows, cols) = (a.shape[0], a.shape[1]);
            let mut out = vec![0.0f32; a.data.len()];
            for c in 0..cols {
                let col: Vec<f32> = (0..rows).map(|r| a.data[r * cols + c]).collect();
                let sm = numerics::softmax(&col);
                for r in 0..rows {
                    out[r * cols + c] = sm[r];
                }
            }
            NumTensor::new(out, a.shape.clone())
        }
        _ => Err(GraphError::ExecutionError(format!(
            "softmax: unsupported axis {axis} for shape {:?}",
            a.shape
        ))),
    }
}

/// Layer normalization over the trailing dimension (no affine params; the graph
/// op carries none). `normalized_shape`, when given, must match that dimension.
fn layer_norm_op(
    a: &NumTensor,
    normalized_shape: &[usize],
    eps: f64,
) -> Result<NumTensor, GraphError> {
    let last = *a.shape.last().ok_or_else(|| {
        GraphError::ShapeMismatch("layer_norm: scalar has no axis to normalize".to_string())
    })?;
    if !normalized_shape.is_empty() {
        let prod: usize = normalized_shape.iter().product();
        if prod != last {
            return Err(GraphError::ShapeMismatch(format!(
                "layer_norm: normalized_shape {normalized_shape:?} (={prod}) != last dim {last}"
            )));
        }
    }
    let eps = eps as f32;
    map_last_axis(a, |row| numerics::layer_norm(row, None, None, eps))
}

fn eval_node<E>(
    node: &GraphNode,
    env: &HashMap<String, NumTensor>,
    inputs: &HashMap<String, NumTensor>,
    exec: &mut E,
) -> Result<NumTensor, GraphError>
where
    E: TlExecutor<Tensor = NumTensor, Error = GraphError>,
{
    match &node.op {
        TensorOp::Input { .. } | TensorOp::Constant { .. } => inputs
            .get(&node.id)
            .cloned()
            .ok_or_else(|| GraphError::MissingInput(node.id.clone())),

        // Element-wise arithmetic, matmul and reductions go through the backend.
        TensorOp::Add => {
            exec.elem_op_binary(ElemOp::Add, operand(node, env, 0)?, operand(node, env, 1)?)
        }
        TensorOp::Sub => {
            exec.elem_op_binary(ElemOp::Subtract, operand(node, env, 0)?, operand(node, env, 1)?)
        }
        TensorOp::Mul => {
            exec.elem_op_binary(ElemOp::Multiply, operand(node, env, 0)?, operand(node, env, 1)?)
        }
        TensorOp::Div => {
            exec.elem_op_binary(ElemOp::Divide, operand(node, env, 0)?, operand(node, env, 1)?)
        }

        TensorOp::MatMul => {
            let (a, b) = (operand(node, env, 0)?.clone(), operand(node, env, 1)?.clone());
            exec.einsum("ij,jk->ik", &[a, b])
        }

        TensorOp::ReLU => exec.elem_op(ElemOp::Relu, operand(node, env, 0)?),
        TensorOp::Tanh => exec.elem_op(ElemOp::Tanh, operand(node, env, 0)?),
        TensorOp::Sigmoid => exec.elem_op(ElemOp::Sigmoid, operand(node, env, 0)?),
        TensorOp::GELU => exec.elem_op(ElemOp::Gelu, operand(node, env, 0)?),
        TensorOp::SiLU => exec.elem_op(ElemOp::Silu, operand(node, env, 0)?),
        TensorOp::Exp => exec.elem_op(ElemOp::Exp, operand(node, env, 0)?),
        TensorOp::Log => exec.elem_op(ElemOp::Log, operand(node, env, 0)?),
        TensorOp::Sqrt => exec.elem_op(ElemOp::Sqrt, operand(node, env, 0)?),
        TensorOp::ReduceSum { axes, keepdims } => {
            let r = exec.reduce(ReduceOp::Sum, operand(node, env, 0)?, axes)?;
            apply_keepdims(r, operand(node, env, 0)?.shape.len(), axes, *keepdims)
        }
        TensorOp::ReduceMean { axes, keepdims } => {
            let r = exec.reduce(ReduceOp::Mean, operand(node, env, 0)?, axes)?;
            apply_keepdims(r, operand(node, env, 0)?.shape.len(), axes, *keepdims)
        }

        // Axis-aware / structural ops are evaluated directly (not in the trait).
        TensorOp::Softmax { axis } => softmax_op(operand(node, env, 0)?, *axis),
        TensorOp::LayerNorm {
            normalized_shape,
            eps,
        } => layer_norm_op(operand(node, env, 0)?, normalized_shape, *eps),
        TensorOp::Pow { exponent } => {
            let e = *exponent as f32;
            Ok(unary(operand(node, env, 0)?, move |x| x.powf(e)))
        }

        TensorOp::Reshape { shape } => {
            let a = operand(node, env, 0)?;
            let dims: Result<Vec<usize>, _> = shape
                .iter()
                .map(|d| {
                    if *d < 0 {
                        Err(GraphError::ExecutionError(
                            "reshape with -1 not supported yet".to_string(),
                        ))
                    } else {
                        Ok(*d as usize)
                    }
                })
                .collect();
            NumTensor::new(a.data.clone(), dims?)
        }

        TensorOp::Transpose { axes } => {
            let a = operand(node, env, 0)?;
            if a.shape.len() != 2 || axes.as_slice() != [1, 0] {
                return Err(GraphError::ExecutionError(
                    "transpose: only 2-D with axes [1,0] supported yet".to_string(),
                ));
            }
            let (r, c) = (a.shape[0], a.shape[1]);
            let mut data = vec![0.0f32; r * c];
            for i in 0..r {
                for j in 0..c {
                    data[j * r + i] = a.data[i * c + j];
                }
            }
            Ok(NumTensor::unchecked(data, vec![c, r]))
        }

        other => Err(GraphError::ExecutionError(format!(
            "op not supported by numeric executor yet: {:?}",
            other
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(id: &str) -> GraphNode {
        GraphNode::new(id.to_string(), TensorOp::Input { name: id.to_string() })
    }

    #[test]
    fn elementwise_add_relu() {
        let mut g = ComputationGraph::new();
        g.add_node(input("a")).unwrap();
        g.add_node(input("b")).unwrap();
        g.add_node(
            GraphNode::new("s".into(), TensorOp::Add)
                .add_input("a".into())
                .add_input("b".into()),
        )
        .unwrap();
        g.add_node(GraphNode::new("r".into(), TensorOp::ReLU).add_input("s".into()))
            .unwrap();

        let mut inputs = HashMap::new();
        inputs.insert("a".into(), NumTensor::new(vec![1.0, -2.0], vec![2]).unwrap());
        inputs.insert("b".into(), NumTensor::new(vec![0.5, 0.5], vec![2]).unwrap());

        let out = execute_output(&g, &inputs, "r").unwrap();
        assert_eq!(out.data, vec![1.5, 0.0]); // (1+0.5)=1.5; relu(-2+0.5)=relu(-1.5)=0
    }

    #[test]
    fn linear_layer_matmul_add() {
        // y = x[1x2] · W[2x2] + b[1x2]
        let mut g = ComputationGraph::new();
        g.add_node(input("x")).unwrap();
        g.add_node(input("w")).unwrap();
        g.add_node(input("b")).unwrap();
        g.add_node(
            GraphNode::new("mm".into(), TensorOp::MatMul)
                .add_input("x".into())
                .add_input("w".into()),
        )
        .unwrap();
        g.add_node(
            GraphNode::new("y".into(), TensorOp::Add)
                .add_input("mm".into())
                .add_input("b".into()),
        )
        .unwrap();

        let mut inputs = HashMap::new();
        inputs.insert("x".into(), NumTensor::new(vec![1.0, 2.0], vec![1, 2]).unwrap());
        // W = [[1,0],[0,1]] (identity) → x·W = x
        inputs.insert(
            "w".into(),
            NumTensor::new(vec![1.0, 0.0, 0.0, 1.0], vec![2, 2]).unwrap(),
        );
        inputs.insert("b".into(), NumTensor::new(vec![10.0, 20.0], vec![1, 2]).unwrap());

        let out = execute_output(&g, &inputs, "y").unwrap();
        assert_eq!(out.data, vec![11.0, 22.0]);
        assert_eq!(out.shape, vec![1, 2]);
    }

    #[test]
    fn transpose_2d() {
        let mut g = ComputationGraph::new();
        g.add_node(input("a")).unwrap();
        g.add_node(
            GraphNode::new("t".into(), TensorOp::Transpose { axes: vec![1, 0] })
                .add_input("a".into()),
        )
        .unwrap();
        let mut inputs = HashMap::new();
        // [[1,2,3],[4,5,6]] -> [[1,4],[2,5],[3,6]]
        inputs.insert(
            "a".into(),
            NumTensor::new(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], vec![2, 3]).unwrap(),
        );
        let out = execute_output(&g, &inputs, "t").unwrap();
        assert_eq!(out.shape, vec![3, 2]);
        assert_eq!(out.data, vec![1.0, 4.0, 2.0, 5.0, 3.0, 6.0]);
    }

    #[test]
    fn missing_input_errs() {
        let mut g = ComputationGraph::new();
        g.add_node(input("a")).unwrap();
        let r = execute(&g, &HashMap::new());
        assert!(matches!(r, Err(GraphError::MissingInput(_))));
    }

    #[test]
    fn unsupported_op_errs() {
        let mut g = ComputationGraph::new();
        g.add_node(input("a")).unwrap();
        g.add_node(
            GraphNode::new("c".into(), TensorOp::Concat { axis: 0 }).add_input("a".into()),
        )
        .unwrap();
        let mut inputs = HashMap::new();
        inputs.insert("a".into(), NumTensor::new(vec![1.0], vec![1]).unwrap());
        assert!(matches!(
            execute(&g, &inputs),
            Err(GraphError::ExecutionError(_))
        ));
    }

    fn run_unary(op: TensorOp, data: Vec<f32>, shape: Vec<usize>) -> NumTensor {
        let mut g = ComputationGraph::new();
        g.add_node(input("a")).unwrap();
        g.mark_input("a".into());
        g.add_node(GraphNode::new("y".into(), op).add_input("a".into()))
            .unwrap();
        g.mark_output("y".into());
        let mut inputs = HashMap::new();
        inputs.insert("a".into(), NumTensor::new(data, shape).unwrap());
        execute_output(&g, &inputs, "y").unwrap()
    }

    #[test]
    fn silu_op_matches_kernel() {
        let y = run_unary(TensorOp::SiLU, vec![0.0, 1.0], vec![1, 2]);
        assert!((y.data[0]).abs() < 1e-6);
        assert!((y.data[1] - numerics::silu(1.0)).abs() < 1e-7);
    }

    #[test]
    fn softmax_op_per_row() {
        // 2x2, axis -1 → each row sums to 1.
        let y = run_unary(TensorOp::Softmax { axis: -1 }, vec![1.0, 1.0, 0.0, 2.0], vec![2, 2]);
        assert!((y.data[0] + y.data[1] - 1.0).abs() < 1e-6);
        assert!((y.data[2] + y.data[3] - 1.0).abs() < 1e-6);
        assert!((y.data[0] - 0.5).abs() < 1e-6); // equal logits → 0.5 each
    }

    #[test]
    fn softmax_op_axis0_per_column() {
        let y = run_unary(TensorOp::Softmax { axis: 0 }, vec![1.0, 2.0, 1.0, 2.0], vec![2, 2]);
        // Each column has equal entries → 0.5 everywhere.
        assert!(y.data.iter().all(|&v| (v - 0.5).abs() < 1e-6));
    }

    #[test]
    fn layer_norm_op_zero_mean_unit_var_per_row() {
        let y = run_unary(
            TensorOp::LayerNorm { normalized_shape: vec![3], eps: 1e-5 },
            vec![1.0, 2.0, 3.0, 10.0, 20.0, 30.0],
            vec![2, 3],
        );
        for row in [&y.data[0..3], &y.data[3..6]] {
            let mean = row.iter().sum::<f32>() / 3.0;
            let var = row.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / 3.0;
            assert!(mean.abs() < 1e-4, "mean {mean}");
            assert!((var - 1.0).abs() < 1e-3, "var {var}");
        }
    }

    #[test]
    fn reduce_sum_op_drops_and_keepdims() {
        // 2x2 sum over axis 0 → [4,6] shape [2]
        let y = run_unary(
            TensorOp::ReduceSum { axes: vec![0], keepdims: false },
            vec![1.0, 2.0, 3.0, 4.0],
            vec![2, 2],
        );
        assert_eq!(y.shape, vec![2]);
        assert_eq!(y.data, vec![4.0, 6.0]);
        // keepdims → shape [1,2]
        let yk = run_unary(
            TensorOp::ReduceSum { axes: vec![0], keepdims: true },
            vec![1.0, 2.0, 3.0, 4.0],
            vec![2, 2],
        );
        assert_eq!(yk.shape, vec![1, 2]);
        assert_eq!(yk.data, vec![4.0, 6.0]);
    }

    #[test]
    fn reduce_mean_op_whole_tensor() {
        let y = run_unary(
            TensorOp::ReduceMean { axes: vec![], keepdims: false },
            vec![1.0, 2.0, 3.0, 4.0],
            vec![2, 2],
        );
        assert_eq!(y.data, vec![2.5]);
    }

    #[test]
    fn execute_with_explicit_backend_matches_default() {
        use crate::tl_executor::NumExecutor;
        let mut g = ComputationGraph::new();
        g.add_node(input("a")).unwrap();
        g.add_node(input("b")).unwrap();
        g.mark_input("a".into());
        g.mark_input("b".into());
        g.add_node(
            GraphNode::new("m".into(), TensorOp::MatMul)
                .add_input("a".into())
                .add_input("b".into()),
        )
        .unwrap();
        g.mark_output("m".into());
        let mut inputs = HashMap::new();
        inputs.insert("a".into(), NumTensor::new(vec![1.0, 2.0, 3.0, 4.0], vec![2, 2]).unwrap());
        inputs.insert("b".into(), NumTensor::new(vec![5.0, 6.0, 7.0, 8.0], vec![2, 2]).unwrap());
        let mut exec = NumExecutor::new();
        let env = execute_with(&g, &inputs, &mut exec).unwrap();
        assert_eq!(env["m"].data, vec![19.0, 22.0, 43.0, 50.0]);
    }

    #[test]
    fn layer_norm_op_rejects_mismatched_normalized_shape() {
        let mut g = ComputationGraph::new();
        g.add_node(input("a")).unwrap();
        g.mark_input("a".into());
        g.add_node(
            GraphNode::new(
                "y".into(),
                TensorOp::LayerNorm { normalized_shape: vec![5], eps: 1e-5 },
            )
            .add_input("a".into()),
        )
        .unwrap();
        g.mark_output("y".into());
        let mut inputs = HashMap::new();
        inputs.insert("a".into(), NumTensor::new(vec![1.0, 2.0, 3.0], vec![1, 3]).unwrap());
        assert!(matches!(
            execute(&g, &inputs),
            Err(GraphError::ShapeMismatch(_))
        ));
    }
}
