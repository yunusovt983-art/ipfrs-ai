//! Pluggable tensor-execution backend interface (RoadMap Phase 5, Spike 3).
//!
//! `NumTensor` and [`numeric_exec`](crate::numeric_exec) are our own lightweight
//! engine, but the strategic decision (see `Vendor/REUSE-ANALYSIS.md` §0) is to
//! keep numerics *behind a trait* so a heavier backend (SciRS2, GPU, …) can be
//! swapped in later without touching callers. This module defines that seam.
//!
//! ## Provenance (DDD: Conformist on the interface)
//!
//! The [`TlExecutor`] trait and the [`ElemOp`] / [`ReduceOp`] enums mirror the
//! *shape* of `tensorlogic`'s `tensorlogic-infer` (`traits.rs` / `ops.rs`) — we
//! speak its Ubiquitous Language (associated `Tensor`/`Error`; `einsum` /
//! `elem_op` / `elem_op_binary` / `reduce`) so an upstream engine could implement
//! the same trait. We **conform to the interface, not the implementation**:
//! [`NumExecutor`] is our own kernel set over [`NumTensor`], reusing the
//! ACL-ported [`numerics`](crate::numerics) kernels. The enum variant set is a
//! pragmatic subset of the upstream catalogue — what our engine actually computes.

use crate::computation_graph::GraphError;
use crate::numeric_exec::NumTensor;
use crate::numerics;

/// Element-wise operation (unary or binary). A pragmatic subset of
/// `tensorlogic-infer`'s `ElemOp`, restricted to what [`NumExecutor`] computes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElemOp {
    // Unary activations / maths
    Relu,
    Sigmoid,
    Tanh,
    Gelu,
    Silu,
    Exp,
    Log,
    Sqrt,
    OneMinus,
    // Binary arithmetic
    Add,
    Subtract,
    Multiply,
    Divide,
    Min,
    Max,
}

impl ElemOp {
    /// Whether this op takes one operand (`true`) or two (`false`).
    pub fn is_unary(self) -> bool {
        matches!(
            self,
            ElemOp::Relu
                | ElemOp::Sigmoid
                | ElemOp::Tanh
                | ElemOp::Gelu
                | ElemOp::Silu
                | ElemOp::Exp
                | ElemOp::Log
                | ElemOp::Sqrt
                | ElemOp::OneMinus
        )
    }
}

/// Reduction operation across tensor axes. Mirrors `tensorlogic-infer`'s `ReduceOp`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReduceOp {
    Sum,
    Mean,
    Max,
    Min,
    Product,
}

/// Core tensor-execution interface — the swappable backend seam.
///
/// Conformist port of `tensorlogic-infer::TlExecutor`. `&mut self` matches the
/// upstream signature so a stateful backend (caches, device handles) fits without
/// an interface change, even though [`NumExecutor`] is stateless.
pub trait TlExecutor {
    /// The backend's tensor representation.
    type Tensor;
    /// The backend's error type.
    type Error;

    /// Execute an einsum contraction over `inputs` per the `spec` string.
    fn einsum(&mut self, spec: &str, inputs: &[Self::Tensor]) -> Result<Self::Tensor, Self::Error>;

    /// Apply a unary element-wise op.
    fn elem_op(&mut self, op: ElemOp, x: &Self::Tensor) -> Result<Self::Tensor, Self::Error>;

    /// Apply a binary element-wise op (operands must share a shape).
    fn elem_op_binary(
        &mut self,
        op: ElemOp,
        x: &Self::Tensor,
        y: &Self::Tensor,
    ) -> Result<Self::Tensor, Self::Error>;

    /// Reduce `x` along `axes` (empty `axes` reduces the whole tensor).
    fn reduce(
        &mut self,
        op: ReduceOp,
        x: &Self::Tensor,
        axes: &[usize],
    ) -> Result<Self::Tensor, Self::Error>;
}

/// Our default backend: pure-`f32` kernels over [`NumTensor`].
#[derive(Debug, Default, Clone, Copy)]
pub struct NumExecutor;

impl NumExecutor {
    /// Construct the backend (stateless).
    pub fn new() -> Self {
        Self
    }
}

fn reduce_init(op: ReduceOp) -> f32 {
    match op {
        ReduceOp::Sum | ReduceOp::Mean => 0.0,
        ReduceOp::Product => 1.0,
        ReduceOp::Max => f32::NEG_INFINITY,
        ReduceOp::Min => f32::INFINITY,
    }
}

fn reduce_step(op: ReduceOp, acc: f32, x: f32) -> f32 {
    match op {
        ReduceOp::Sum | ReduceOp::Mean => acc + x,
        ReduceOp::Product => acc * x,
        ReduceOp::Max => acc.max(x),
        ReduceOp::Min => acc.min(x),
    }
}

impl TlExecutor for NumExecutor {
    type Tensor = NumTensor;
    type Error = GraphError;

    fn einsum(&mut self, spec: &str, inputs: &[NumTensor]) -> Result<NumTensor, GraphError> {
        // Minimal but real: support 2-operand 2-D matrix multiplication, the
        // `ab,bc->ac` family (whitespace ignored). Anything else is a follow-up.
        let normalized: String = spec.chars().filter(|c| !c.is_whitespace()).collect();

        // Parse "<lhs>,<rhs>-><out>".
        let (lhs, out) = normalized
            .split_once("->")
            .ok_or_else(|| GraphError::ExecutionError(format!("einsum: missing '->' in '{spec}'")))?;
        let operands: Vec<&str> = lhs.split(',').collect();
        if operands.len() != 2 || inputs.len() != 2 {
            return Err(GraphError::ExecutionError(format!(
                "einsum: only 2-operand contraction supported, got spec '{spec}'"
            )));
        }
        let (a_sub, b_sub) = (operands[0], operands[1]);
        // Classic matmul: ij,jk->ik with the shared middle index contracted.
        if a_sub.len() == 2
            && b_sub.len() == 2
            && out.len() == 2
            && a_sub.as_bytes()[1] == b_sub.as_bytes()[0]
            && out.as_bytes()[0] == a_sub.as_bytes()[0]
            && out.as_bytes()[1] == b_sub.as_bytes()[1]
        {
            return matmul_2d(&inputs[0], &inputs[1]);
        }
        Err(GraphError::ExecutionError(format!(
            "einsum: unsupported contraction '{spec}' (only matmul ab,bc->ac)"
        )))
    }

    fn elem_op(&mut self, op: ElemOp, x: &NumTensor) -> Result<NumTensor, GraphError> {
        let f: fn(f32) -> f32 = match op {
            ElemOp::Relu => |v| v.max(0.0),
            ElemOp::Sigmoid => |v| 1.0 / (1.0 + (-v).exp()),
            ElemOp::Tanh => |v| v.tanh(),
            ElemOp::Gelu => numerics::gelu,
            ElemOp::Silu => numerics::silu,
            ElemOp::Exp => |v| v.exp(),
            ElemOp::Log => |v| v.ln(),
            ElemOp::Sqrt => |v| v.sqrt(),
            ElemOp::OneMinus => |v| 1.0 - v,
            _ => {
                return Err(GraphError::ExecutionError(format!(
                    "elem_op: {op:?} is not a unary op"
                )))
            }
        };
        NumTensor::new(x.data.iter().map(|&v| f(v)).collect(), x.shape.clone())
    }

    fn elem_op_binary(
        &mut self,
        op: ElemOp,
        x: &NumTensor,
        y: &NumTensor,
    ) -> Result<NumTensor, GraphError> {
        let f: fn(f32, f32) -> f32 = match op {
            ElemOp::Add => |a, b| a + b,
            ElemOp::Subtract => |a, b| a - b,
            ElemOp::Multiply => |a, b| a * b,
            ElemOp::Divide => |a, b| a / b,
            ElemOp::Min => |a, b| a.min(b),
            ElemOp::Max => |a, b| a.max(b),
            _ => {
                return Err(GraphError::ExecutionError(format!(
                    "elem_op_binary: {op:?} is not a binary op"
                )))
            }
        };
        if x.shape != y.shape {
            return Err(GraphError::ShapeMismatch(format!(
                "elem_op_binary {op:?}: {:?} vs {:?}",
                x.shape, y.shape
            )));
        }
        let data = x.data.iter().zip(&y.data).map(|(&a, &b)| f(a, b)).collect();
        NumTensor::new(data, x.shape.clone())
    }

    fn reduce(
        &mut self,
        op: ReduceOp,
        x: &NumTensor,
        axes: &[usize],
    ) -> Result<NumTensor, GraphError> {
        let all_axes = axes.is_empty() || axes.len() == x.shape.len();

        // Whole-tensor reduction → scalar [1].
        if all_axes {
            let acc = x.data.iter().fold(reduce_init(op), |a, &v| reduce_step(op, a, v));
            let val = if op == ReduceOp::Mean {
                acc / x.data.len().max(1) as f32
            } else {
                acc
            };
            return NumTensor::new(vec![val], vec![1]);
        }

        // 2-D single-axis reduction.
        if x.shape.len() == 2 && axes.len() == 1 {
            let (rows, cols) = (x.shape[0], x.shape[1]);
            return match axes[0] {
                0 => {
                    // Reduce rows → one value per column.
                    let mut out = vec![reduce_init(op); cols];
                    for (c, slot) in out.iter_mut().enumerate() {
                        let mut acc = reduce_init(op);
                        for r in 0..rows {
                            acc = reduce_step(op, acc, x.data[r * cols + c]);
                        }
                        *slot = if op == ReduceOp::Mean {
                            acc / rows.max(1) as f32
                        } else {
                            acc
                        };
                    }
                    NumTensor::new(out, vec![1, cols])
                }
                1 => {
                    // Reduce columns → one value per row.
                    let mut out = vec![reduce_init(op); rows];
                    for (r, slot) in out.iter_mut().enumerate() {
                        let row = &x.data[r * cols..(r + 1) * cols];
                        let acc = row.iter().fold(reduce_init(op), |a, &v| reduce_step(op, a, v));
                        *slot = if op == ReduceOp::Mean {
                            acc / cols.max(1) as f32
                        } else {
                            acc
                        };
                    }
                    NumTensor::new(out, vec![rows, 1])
                }
                d => Err(GraphError::ExecutionError(format!(
                    "reduce: axis {d} out of range for 2-D tensor"
                ))),
            };
        }

        Err(GraphError::ExecutionError(format!(
            "reduce: unsupported axes {axes:?} for shape {:?}",
            x.shape
        )))
    }
}

/// 2-D matrix multiply, shared by `einsum`'s matmul path.
fn matmul_2d(a: &NumTensor, b: &NumTensor) -> Result<NumTensor, GraphError> {
    if a.shape.len() != 2 || b.shape.len() != 2 {
        return Err(GraphError::ShapeMismatch(
            "einsum matmul requires 2-D operands".to_string(),
        ));
    }
    let (m, k) = (a.shape[0], a.shape[1]);
    let (k2, n) = (b.shape[0], b.shape[1]);
    if k != k2 {
        return Err(GraphError::ShapeMismatch(format!(
            "einsum matmul inner dims: {m}x{k} · {k2}x{n}"
        )));
    }
    let mut data = vec![0.0f32; m * n];
    for i in 0..m {
        for j in 0..n {
            let mut s = 0.0f32;
            for p in 0..k {
                s += a.data[i * k + p] * b.data[p * n + j];
            }
            data[i * n + j] = s;
        }
    }
    NumTensor::new(data, vec![m, n])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(data: Vec<f32>, shape: Vec<usize>) -> NumTensor {
        NumTensor::new(data, shape).unwrap()
    }

    #[test]
    fn elem_op_unary_activations() {
        let mut e = NumExecutor::new();
        let x = t(vec![-1.0, 0.0, 1.0], vec![3]);
        assert_eq!(e.elem_op(ElemOp::Relu, &x).unwrap().data, vec![0.0, 0.0, 1.0]);
        let silu = e.elem_op(ElemOp::Silu, &x).unwrap();
        assert!((silu.data[2] - 0.7311).abs() < 1e-3);
    }

    #[test]
    fn elem_op_rejects_binary_in_unary() {
        let mut e = NumExecutor::new();
        let x = t(vec![1.0], vec![1]);
        assert!(e.elem_op(ElemOp::Add, &x).is_err());
    }

    #[test]
    fn elem_op_binary_arith_and_shape_check() {
        let mut e = NumExecutor::new();
        let a = t(vec![1.0, 2.0], vec![2]);
        let b = t(vec![3.0, 5.0], vec![2]);
        assert_eq!(e.elem_op_binary(ElemOp::Add, &a, &b).unwrap().data, vec![4.0, 7.0]);
        assert_eq!(e.elem_op_binary(ElemOp::Max, &a, &b).unwrap().data, vec![3.0, 5.0]);
        let bad = t(vec![1.0], vec![1]);
        assert!(e.elem_op_binary(ElemOp::Add, &a, &bad).is_err());
    }

    #[test]
    fn reduce_whole_tensor() {
        let mut e = NumExecutor::new();
        let x = t(vec![1.0, 2.0, 3.0, 4.0], vec![2, 2]);
        assert_eq!(e.reduce(ReduceOp::Sum, &x, &[]).unwrap().data, vec![10.0]);
        assert_eq!(e.reduce(ReduceOp::Mean, &x, &[]).unwrap().data, vec![2.5]);
        assert_eq!(e.reduce(ReduceOp::Max, &x, &[]).unwrap().data, vec![4.0]);
    }

    #[test]
    fn reduce_2d_axes() {
        let mut e = NumExecutor::new();
        let x = t(vec![1.0, 2.0, 3.0, 4.0], vec![2, 2]);
        // axis 0 → per column: [1+3, 2+4] = [4, 6]
        let c = e.reduce(ReduceOp::Sum, &x, &[0]).unwrap();
        assert_eq!(c.shape, vec![1, 2]);
        assert_eq!(c.data, vec![4.0, 6.0]);
        // axis 1 → per row: [1+2, 3+4] = [3, 7]
        let r = e.reduce(ReduceOp::Sum, &x, &[1]).unwrap();
        assert_eq!(r.shape, vec![2, 1]);
        assert_eq!(r.data, vec![3.0, 7.0]);
    }

    #[test]
    fn einsum_matmul() {
        let mut e = NumExecutor::new();
        let a = t(vec![1.0, 2.0, 3.0, 4.0], vec![2, 2]);
        let b = t(vec![5.0, 6.0, 7.0, 8.0], vec![2, 2]);
        let c = e.einsum("ij,jk->ik", &[a, b]).unwrap();
        // [[1,2],[3,4]]·[[5,6],[7,8]] = [[19,22],[43,50]]
        assert_eq!(c.data, vec![19.0, 22.0, 43.0, 50.0]);
    }

    #[test]
    fn einsum_unsupported_errs() {
        let mut e = NumExecutor::new();
        let a = t(vec![1.0], vec![1, 1]);
        assert!(e.einsum("i->i", std::slice::from_ref(&a)).is_err());
        assert!(e.einsum("ij,jk,kl->il", &[a.clone(), a.clone(), a]).is_err());
    }
}
