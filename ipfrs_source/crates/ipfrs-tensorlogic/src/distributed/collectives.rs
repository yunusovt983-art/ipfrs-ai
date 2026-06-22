//! Collective operations over [`NumTensor`] for distributed execution.
//!
//! ACL port of `torsh`'s collective *vocabulary* (`ReduceOp`, all-reduce /
//! all-gather / reduce-scatter). Unlike `torsh`'s bodies — which are explicit mock
//! stubs that "skip the averaging to avoid type issues" — these compute for real,
//! locally and synchronously. They define the reduction semantics that a future
//! libp2p activation-streaming layer will execute across peers, and they are fully
//! unit-tested here so that wire-level code can be validated against them.

use crate::computation_graph::GraphError;
use crate::numeric_exec::NumTensor;

/// Element-wise reduction applied across the tensors contributed by each peer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReduceOp {
    /// Sum across peers.
    Sum,
    /// Arithmetic mean across peers.
    Mean,
    /// Element-wise maximum.
    Max,
    /// Element-wise minimum.
    Min,
    /// Element-wise product.
    Product,
}

fn reduce_pair(op: ReduceOp, a: f32, b: f32) -> f32 {
    match op {
        ReduceOp::Sum | ReduceOp::Mean => a + b,
        ReduceOp::Max => a.max(b),
        ReduceOp::Min => a.min(b),
        ReduceOp::Product => a * b,
    }
}

/// All-reduce: combine same-shaped tensors from every peer element-wise and
/// return the single tensor that every peer would hold afterwards.
pub fn all_reduce(tensors: &[NumTensor], op: ReduceOp) -> Result<NumTensor, GraphError> {
    let first = tensors.first().ok_or_else(|| {
        GraphError::ExecutionError("all_reduce: no tensors to reduce".to_string())
    })?;
    for t in &tensors[1..] {
        if t.shape != first.shape {
            return Err(GraphError::ShapeMismatch(format!(
                "all_reduce: shape {:?} != {:?}",
                t.shape, first.shape
            )));
        }
    }

    let mut acc = first.data.clone();
    for t in &tensors[1..] {
        for (a, b) in acc.iter_mut().zip(&t.data) {
            *a = reduce_pair(op, *a, *b);
        }
    }
    if op == ReduceOp::Mean {
        let n = tensors.len() as f32;
        for a in acc.iter_mut() {
            *a /= n;
        }
    }
    NumTensor::new(acc, first.shape.clone())
}

/// All-gather: concatenate each peer's shard along the leading axis (dim 0),
/// yielding the full tensor. All shards must share their trailing dimensions.
pub fn all_gather(shards: &[NumTensor]) -> Result<NumTensor, GraphError> {
    let first = shards.first().ok_or_else(|| {
        GraphError::ExecutionError("all_gather: no shards to gather".to_string())
    })?;
    if first.shape.is_empty() {
        return Err(GraphError::ShapeMismatch(
            "all_gather: scalar shards have no leading axis".to_string(),
        ));
    }
    let trailing = &first.shape[1..];
    let mut lead_total = 0usize;
    let mut data = Vec::new();
    for s in shards {
        if s.shape.is_empty() || &s.shape[1..] != trailing {
            return Err(GraphError::ShapeMismatch(format!(
                "all_gather: trailing dims {:?} incompatible with {:?}",
                s.shape, first.shape
            )));
        }
        lead_total += s.shape[0];
        data.extend_from_slice(&s.data);
    }
    let mut shape = vec![lead_total];
    shape.extend_from_slice(trailing);
    NumTensor::new(data, shape)
}

/// Reduce-scatter: element-wise reduce the per-peer tensors, then split the
/// result along the leading axis into `num_chunks` contiguous pieces (peer `i`
/// receives chunk `i`). Chunk sizes differ by at most one along dim 0.
pub fn reduce_scatter(
    tensors: &[NumTensor],
    op: ReduceOp,
    num_chunks: usize,
) -> Result<Vec<NumTensor>, GraphError> {
    if num_chunks == 0 {
        return Err(GraphError::ExecutionError(
            "reduce_scatter: num_chunks must be > 0".to_string(),
        ));
    }
    let reduced = all_reduce(tensors, op)?;
    if reduced.shape.is_empty() {
        return Err(GraphError::ShapeMismatch(
            "reduce_scatter: scalar has no leading axis to scatter".to_string(),
        ));
    }
    let lead = reduced.shape[0];
    let inner: usize = reduced.shape[1..].iter().product::<usize>().max(1);
    let base = lead / num_chunks;
    let rem = lead % num_chunks;

    let mut out = Vec::with_capacity(num_chunks);
    let mut start = 0usize;
    for i in 0..num_chunks {
        let len = base + if i < rem { 1 } else { 0 };
        let slice = reduced.data[start * inner..(start + len) * inner].to_vec();
        let mut shape = vec![len];
        shape.extend_from_slice(&reduced.shape[1..]);
        out.push(NumTensor::new(slice, shape)?);
        start += len;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(data: Vec<f32>, shape: Vec<usize>) -> NumTensor {
        NumTensor::new(data, shape).unwrap()
    }

    #[test]
    fn all_reduce_sum() {
        let a = t(vec![1.0, 2.0, 3.0], vec![3]);
        let b = t(vec![10.0, 20.0, 30.0], vec![3]);
        let r = all_reduce(&[a, b], ReduceOp::Sum).unwrap();
        assert_eq!(r.data, vec![11.0, 22.0, 33.0]);
    }

    #[test]
    fn all_reduce_mean() {
        let a = t(vec![2.0, 4.0], vec![2]);
        let b = t(vec![4.0, 8.0], vec![2]);
        let r = all_reduce(&[a, b], ReduceOp::Mean).unwrap();
        assert_eq!(r.data, vec![3.0, 6.0]);
    }

    #[test]
    fn all_reduce_max_min_product() {
        let a = t(vec![1.0, 5.0], vec![2]);
        let b = t(vec![4.0, 2.0], vec![2]);
        assert_eq!(all_reduce(&[a.clone(), b.clone()], ReduceOp::Max).unwrap().data, vec![4.0, 5.0]);
        assert_eq!(all_reduce(&[a.clone(), b.clone()], ReduceOp::Min).unwrap().data, vec![1.0, 2.0]);
        assert_eq!(all_reduce(&[a, b], ReduceOp::Product).unwrap().data, vec![4.0, 10.0]);
    }

    #[test]
    fn all_reduce_shape_mismatch_errs() {
        let a = t(vec![1.0, 2.0], vec![2]);
        let b = t(vec![1.0, 2.0, 3.0], vec![3]);
        assert!(all_reduce(&[a, b], ReduceOp::Sum).is_err());
    }

    #[test]
    fn all_reduce_empty_errs() {
        assert!(all_reduce(&[], ReduceOp::Sum).is_err());
    }

    #[test]
    fn all_gather_stacks_rows() {
        let s0 = t(vec![1.0, 2.0], vec![1, 2]);
        let s1 = t(vec![3.0, 4.0, 5.0, 6.0], vec![2, 2]);
        let g = all_gather(&[s0, s1]).unwrap();
        assert_eq!(g.shape, vec![3, 2]);
        assert_eq!(g.data, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
    }

    #[test]
    fn all_gather_trailing_mismatch_errs() {
        let s0 = t(vec![1.0, 2.0], vec![1, 2]);
        let s1 = t(vec![3.0, 4.0, 5.0], vec![1, 3]);
        assert!(all_gather(&[s0, s1]).is_err());
    }

    #[test]
    fn reduce_scatter_reduces_then_splits() {
        // Two peers contribute 4x1 tensors; sum then scatter to 2 chunks of 2 rows.
        let a = t(vec![1.0, 2.0, 3.0, 4.0], vec![4, 1]);
        let b = t(vec![10.0, 20.0, 30.0, 40.0], vec![4, 1]);
        let chunks = reduce_scatter(&[a, b], ReduceOp::Sum, 2).unwrap();
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].shape, vec![2, 1]);
        assert_eq!(chunks[0].data, vec![11.0, 22.0]);
        assert_eq!(chunks[1].data, vec![33.0, 44.0]);
    }

    #[test]
    fn reduce_scatter_uneven_chunks() {
        // 5 rows into 2 chunks → 3 then 2.
        let a = t(vec![1.0, 2.0, 3.0, 4.0, 5.0], vec![5, 1]);
        let chunks = reduce_scatter(&[a], ReduceOp::Sum, 2).unwrap();
        assert_eq!(chunks[0].shape, vec![3, 1]);
        assert_eq!(chunks[1].shape, vec![2, 1]);
    }

    #[test]
    fn reduce_scatter_zero_chunks_errs() {
        let a = t(vec![1.0], vec![1, 1]);
        assert!(reduce_scatter(&[a], ReduceOp::Sum, 0).is_err());
    }
}
