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

use std::collections::{HashMap, HashSet};

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
        // General 2-operand contraction (single-char labels). Covers matmul,
        // batched matmul, matrix-vector, dot, outer — any spec whose contracted
        // labels are those present in the inputs but absent from the output.
        let normalized: String = spec.chars().filter(|c| !c.is_whitespace()).collect();
        let (lhs, out) = normalized
            .split_once("->")
            .ok_or_else(|| GraphError::ExecutionError(format!("einsum: missing '->' in '{spec}'")))?;
        let operands: Vec<&str> = lhs.split(',').collect();
        if operands.len() != 2 || inputs.len() != 2 {
            return Err(GraphError::ExecutionError(format!(
                "einsum: only 2-operand contraction supported, got spec '{spec}'"
            )));
        }
        einsum_2op(operands[0], operands[1], out, &inputs[0], &inputs[1])
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
        reduce_nd(op, x, axes)
    }
}

/// Row-major strides for `shape`.
fn strides(shape: &[usize]) -> Vec<usize> {
    let mut s = vec![1usize; shape.len()];
    for i in (0..shape.len().saturating_sub(1)).rev() {
        s[i] = s[i + 1] * shape[i + 1];
    }
    s
}

/// Advance a mixed-radix odometer `idx` over `dims` (last axis fastest). Returns
/// `false` once it wraps back to all-zero (i.e. the full space was covered).
fn odometer_next(idx: &mut [usize], dims: &[usize]) -> bool {
    for k in (0..idx.len()).rev() {
        idx[k] += 1;
        if idx[k] < dims[k] {
            return true;
        }
        idx[k] = 0;
    }
    false
}

/// General 2-operand einsum (single-char labels, no repeats within an operand).
/// Contracted labels = those present in the inputs but absent from the output;
/// they are summed over. Covers matmul, batched matmul, matrix-vector, dot, outer.
fn einsum_2op(
    a_sub: &str,
    b_sub: &str,
    out_sub: &str,
    a: &NumTensor,
    b: &NumTensor,
) -> Result<NumTensor, GraphError> {
    let a_lbl: Vec<char> = a_sub.chars().collect();
    let b_lbl: Vec<char> = b_sub.chars().collect();
    let o_lbl: Vec<char> = out_sub.chars().collect();
    if a_lbl.len() != a.shape.len() || b_lbl.len() != b.shape.len() {
        return Err(GraphError::ShapeMismatch(format!(
            "einsum: subscripts '{a_sub}','{b_sub}' do not match ranks {:?},{:?}",
            a.shape, b.shape
        )));
    }

    // label -> size, checked for consistency across both operands.
    let mut size: HashMap<char, usize> = HashMap::new();
    for (lbls, shape) in [(&a_lbl, &a.shape), (&b_lbl, &b.shape)] {
        for (i, &l) in lbls.iter().enumerate() {
            match size.get(&l) {
                Some(&prev) if prev != shape[i] => {
                    return Err(GraphError::ShapeMismatch(format!(
                        "einsum: label '{l}' bound to {prev} and {}",
                        shape[i]
                    )))
                }
                _ => {
                    size.insert(l, shape[i]);
                }
            }
        }
    }
    for &l in &o_lbl {
        if !size.contains_key(&l) {
            return Err(GraphError::ExecutionError(format!(
                "einsum: output label '{l}' not present in inputs"
            )));
        }
    }

    // All distinct labels: output (free) labels first, then the contracted ones.
    let mut all: Vec<char> = o_lbl.clone();
    for &l in a_lbl.iter().chain(b_lbl.iter()) {
        if !all.contains(&l) {
            all.push(l);
        }
    }
    let dims: Vec<usize> = all.iter().map(|l| size[l]).collect();
    let pos: HashMap<char, usize> = all.iter().enumerate().map(|(i, &l)| (l, i)).collect();

    let a_str = strides(&a.shape);
    let b_str = strides(&b.shape);
    let o_shape: Vec<usize> = o_lbl.iter().map(|l| size[l]).collect();
    let o_str = strides(&o_shape);
    let o_len: usize = o_shape.iter().product::<usize>().max(1);

    let total: usize = dims.iter().product::<usize>().max(1);
    let mut out = vec![0.0f32; o_len];
    let mut idx = vec![0usize; all.len()];
    for _ in 0..total {
        let a_off: usize = a_lbl.iter().enumerate().map(|(i, l)| idx[pos[l]] * a_str[i]).sum();
        let b_off: usize = b_lbl.iter().enumerate().map(|(i, l)| idx[pos[l]] * b_str[i]).sum();
        let o_off: usize = o_lbl.iter().enumerate().map(|(i, l)| idx[pos[l]] * o_str[i]).sum();
        out[o_off] += a.data[a_off] * b.data[b_off];
        if !odometer_next(&mut idx, &dims) {
            break;
        }
    }
    NumTensor::new(out, o_shape)
}

/// General N-D reduction over `axes` (the reduced axes are dropped). Empty `axes`
/// — or a set covering every axis — reduces the whole tensor to a `[1]` scalar.
fn reduce_nd(op: ReduceOp, x: &NumTensor, axes: &[usize]) -> Result<NumTensor, GraphError> {
    let rank = x.shape.len();
    for &ax in axes {
        if ax >= rank {
            return Err(GraphError::ExecutionError(format!(
                "reduce: axis {ax} out of range for shape {:?}",
                x.shape
            )));
        }
    }
    let axis_set: HashSet<usize> = axes.iter().copied().collect();

    if axes.is_empty() || axis_set.len() == rank {
        let acc = x.data.iter().fold(reduce_init(op), |a, &v| reduce_step(op, a, v));
        let val = if op == ReduceOp::Mean {
            acc / x.data.len().max(1) as f32
        } else {
            acc
        };
        return NumTensor::new(vec![val], vec![1]);
    }

    let keep: Vec<usize> = (0..rank).filter(|d| !axis_set.contains(d)).collect();
    let out_shape: Vec<usize> = keep.iter().map(|&d| x.shape[d]).collect();
    let out_len: usize = out_shape.iter().product::<usize>().max(1);
    let count = (x.data.len() / out_len.max(1)).max(1);
    let out_str = strides(&out_shape);

    let mut out = vec![reduce_init(op); out_len];
    let mut idx = vec![0usize; rank];
    for &v in &x.data {
        let o_off: usize = keep.iter().enumerate().map(|(j, &d)| idx[d] * out_str[j]).sum();
        out[o_off] = reduce_step(op, out[o_off], v);
        odometer_next(&mut idx, &x.shape);
    }
    if op == ReduceOp::Mean {
        let c = count as f32;
        out.iter_mut().for_each(|v| *v /= c);
    }
    NumTensor::new(out, out_shape)
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
    fn reduce_2d_axes_drop() {
        let mut e = NumExecutor::new();
        let x = t(vec![1.0, 2.0, 3.0, 4.0], vec![2, 2]);
        // axis 0 → per column, dim dropped: [1+3, 2+4] = [4, 6], shape [2]
        let c = e.reduce(ReduceOp::Sum, &x, &[0]).unwrap();
        assert_eq!(c.shape, vec![2]);
        assert_eq!(c.data, vec![4.0, 6.0]);
        // axis 1 → per row: [1+2, 3+4] = [3, 7], shape [2]
        let r = e.reduce(ReduceOp::Sum, &x, &[1]).unwrap();
        assert_eq!(r.shape, vec![2]);
        assert_eq!(r.data, vec![3.0, 7.0]);
    }

    #[test]
    fn reduce_3d_multi_axis() {
        let mut e = NumExecutor::new();
        // shape [2,2,2], values 0..8.
        let x = t((0..8).map(|v| v as f32).collect(), vec![2, 2, 2]);
        // Reduce axes {0,2} → keep axis 1 (size 2).
        // group by middle index: m=0 → {0,1,4,5}=10; m=1 → {2,3,6,7}=18.
        let r = e.reduce(ReduceOp::Sum, &x, &[0, 2]).unwrap();
        assert_eq!(r.shape, vec![2]);
        assert_eq!(r.data, vec![10.0, 18.0]);
        // Mean over all → 3.5
        assert_eq!(e.reduce(ReduceOp::Mean, &x, &[]).unwrap().data, vec![3.5]);
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
    fn einsum_matvec_dot_outer_batched() {
        let mut e = NumExecutor::new();
        // matrix-vector: ij,j->i  ([[1,2],[3,4]]·[1,1]) = [3,7]
        let m = t(vec![1.0, 2.0, 3.0, 4.0], vec![2, 2]);
        let v = t(vec![1.0, 1.0], vec![2]);
        assert_eq!(e.einsum("ij,j->i", &[m.clone(), v.clone()]).unwrap().data, vec![3.0, 7.0]);
        // dot: i,i->  (scalar)
        let d = e.einsum("i,i->", &[v.clone(), t(vec![2.0, 3.0], vec![2])]).unwrap();
        assert_eq!(d.shape, vec![] as Vec<usize>);
        assert_eq!(d.data, vec![5.0]);
        // outer: i,j->ij
        let o = e.einsum("i,j->ij", &[t(vec![1.0, 2.0], vec![2]), t(vec![3.0, 4.0], vec![2])]).unwrap();
        assert_eq!(o.shape, vec![2, 2]);
        assert_eq!(o.data, vec![3.0, 4.0, 6.0, 8.0]);
        // batched matmul: bij,bjk->bik over batch of 1 → same as matmul
        let ba = t(vec![1.0, 2.0, 3.0, 4.0], vec![1, 2, 2]);
        let bb = t(vec![5.0, 6.0, 7.0, 8.0], vec![1, 2, 2]);
        assert_eq!(
            e.einsum("bij,bjk->bik", &[ba, bb]).unwrap().data,
            vec![19.0, 22.0, 43.0, 50.0]
        );
    }

    #[test]
    fn einsum_invalid_errs() {
        let mut e = NumExecutor::new();
        let a = t(vec![1.0], vec![1, 1]);
        // wrong operand count
        assert!(e.einsum("ij,jk,kl->il", &[a.clone(), a.clone(), a.clone()]).is_err());
        // subscript rank mismatch
        assert!(e.einsum("ijk,kl->il", &[a.clone(), a.clone()]).is_err());
        // output label not in inputs
        assert!(e.einsum("ij,jk->iz", &[a.clone(), a]).is_err());
    }
}
