//! Tensor sharding: split a 2-D [`NumTensor`] across peers and reassemble it.
//!
//! ACL port of `torsh`'s tensor-parallel vocabulary (`ShardInfo`,
//! row/column-parallel strategies). `torsh`'s implementation is entangled with its
//! `Module`/`Tensor<T>` stack; the slice/gather here are our own and operate
//! directly on the dense row-major `f32` [`NumTensor`]. Limited to 2-D tensors
//! (matching the numeric engine's matmul focus); higher rank is a follow-up.

use crate::computation_graph::GraphError;
use crate::numeric_exec::NumTensor;

/// How a tensor is split across peers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShardStrategy {
    /// Split along rows (dim 0) — e.g. the output features of a weight matrix.
    RowParallel,
    /// Split along columns (dim 1) — e.g. the input features of a weight matrix.
    ColumnParallel,
}

impl ShardStrategy {
    /// The tensor dimension this strategy splits.
    pub fn shard_dim(self) -> usize {
        match self {
            ShardStrategy::RowParallel => 0,
            ShardStrategy::ColumnParallel => 1,
        }
    }
}

/// Describes the contiguous slice of a tensor that one peer owns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShardSpec {
    /// Dimension that is split (0 = rows, 1 = columns).
    pub shard_dim: usize,
    /// Start index along `shard_dim` (inclusive).
    pub start: usize,
    /// Length of this shard along `shard_dim`.
    pub len: usize,
    /// Full extent of `shard_dim` before splitting.
    pub dim_total: usize,
    /// Strategy that produced this shard.
    pub strategy: ShardStrategy,
}

/// Plan an even split of `dim_total` into `num_shards` contiguous shards.
///
/// Sizes differ by at most one element: the first `dim_total % num_shards` shards
/// are one larger than the rest, so the union covers `0..dim_total` exactly.
pub fn plan_shards(
    dim_total: usize,
    num_shards: usize,
    strategy: ShardStrategy,
) -> Result<Vec<ShardSpec>, GraphError> {
    if num_shards == 0 {
        return Err(GraphError::ExecutionError(
            "plan_shards: num_shards must be > 0".to_string(),
        ));
    }
    let base = dim_total / num_shards;
    let rem = dim_total % num_shards;
    let mut specs = Vec::with_capacity(num_shards);
    let mut start = 0usize;
    for i in 0..num_shards {
        let len = base + if i < rem { 1 } else { 0 };
        specs.push(ShardSpec {
            shard_dim: strategy.shard_dim(),
            start,
            len,
            dim_total,
            strategy,
        });
        start += len;
    }
    Ok(specs)
}

/// Extract the slice described by `spec` from a 2-D tensor.
pub fn shard_tensor(t: &NumTensor, spec: &ShardSpec) -> Result<NumTensor, GraphError> {
    if t.shape.len() != 2 {
        return Err(GraphError::ShapeMismatch(format!(
            "shard_tensor: only 2-D tensors supported, got {:?}",
            t.shape
        )));
    }
    let (rows, cols) = (t.shape[0], t.shape[1]);
    let axis_total = t.shape.get(spec.shard_dim).copied().ok_or_else(|| {
        GraphError::ExecutionError(format!("shard_tensor: bad shard_dim {}", spec.shard_dim))
    })?;
    if spec.dim_total != axis_total {
        return Err(GraphError::ShapeMismatch(format!(
            "shard_tensor: spec.dim_total {} != tensor dim {}",
            spec.dim_total, axis_total
        )));
    }
    if spec.start + spec.len > axis_total {
        return Err(GraphError::ShapeMismatch(format!(
            "shard_tensor: slice {}..{} out of bounds for dim {}",
            spec.start,
            spec.start + spec.len,
            axis_total
        )));
    }

    match spec.shard_dim {
        0 => {
            // Contiguous rows: a single row-major block.
            let mut data = Vec::with_capacity(spec.len * cols);
            for r in spec.start..spec.start + spec.len {
                data.extend_from_slice(&t.data[r * cols..(r + 1) * cols]);
            }
            NumTensor::new(data, vec![spec.len, cols])
        }
        1 => {
            // Strided columns: pick a window of each row.
            let mut data = Vec::with_capacity(rows * spec.len);
            for r in 0..rows {
                let row = &t.data[r * cols..(r + 1) * cols];
                data.extend_from_slice(&row[spec.start..spec.start + spec.len]);
            }
            NumTensor::new(data, vec![rows, spec.len])
        }
        d => Err(GraphError::ExecutionError(format!(
            "shard_tensor: unsupported shard_dim {} for 2-D tensor",
            d
        ))),
    }
}

/// Reassemble shards (in peer order) back into the full 2-D tensor.
///
/// The inverse of [`shard_tensor`] applied to every shard of one tensor: row
/// shards are stacked, column shards are concatenated within each row.
pub fn gather_shards(
    shards: &[NumTensor],
    strategy: ShardStrategy,
) -> Result<NumTensor, GraphError> {
    if shards.is_empty() {
        return Err(GraphError::ExecutionError(
            "gather_shards: no shards to gather".to_string(),
        ));
    }
    for s in shards {
        if s.shape.len() != 2 {
            return Err(GraphError::ShapeMismatch(format!(
                "gather_shards: only 2-D shards supported, got {:?}",
                s.shape
            )));
        }
    }

    match strategy {
        ShardStrategy::RowParallel => {
            // All shards share column count; stack rows in order.
            let cols = shards[0].shape[1];
            let mut data = Vec::new();
            let mut total_rows = 0usize;
            for s in shards {
                if s.shape[1] != cols {
                    return Err(GraphError::ShapeMismatch(format!(
                        "gather_shards(row): column mismatch {} vs {}",
                        s.shape[1], cols
                    )));
                }
                total_rows += s.shape[0];
                data.extend_from_slice(&s.data);
            }
            NumTensor::new(data, vec![total_rows, cols])
        }
        ShardStrategy::ColumnParallel => {
            // All shards share row count; concatenate columns row by row.
            let rows = shards[0].shape[0];
            let total_cols: usize = shards.iter().map(|s| s.shape[1]).sum();
            for s in shards {
                if s.shape[0] != rows {
                    return Err(GraphError::ShapeMismatch(format!(
                        "gather_shards(col): row mismatch {} vs {}",
                        s.shape[0], rows
                    )));
                }
            }
            let mut data = vec![0.0f32; rows * total_cols];
            let mut col_off = 0usize;
            for s in shards {
                let sc = s.shape[1];
                for r in 0..rows {
                    let dst = r * total_cols + col_off;
                    let src = r * sc;
                    data[dst..dst + sc].copy_from_slice(&s.data[src..src + sc]);
                }
                col_off += sc;
            }
            NumTensor::new(data, vec![rows, total_cols])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(data: Vec<f32>, shape: Vec<usize>) -> NumTensor {
        NumTensor::new(data, shape).unwrap()
    }

    #[test]
    fn plan_even_split() {
        let specs = plan_shards(6, 3, ShardStrategy::RowParallel).unwrap();
        assert_eq!(specs.len(), 3);
        assert_eq!(specs.iter().map(|s| s.len).collect::<Vec<_>>(), vec![2, 2, 2]);
        assert_eq!(specs.iter().map(|s| s.start).collect::<Vec<_>>(), vec![0, 2, 4]);
    }

    #[test]
    fn plan_uneven_split_remainder_first() {
        // 7 into 3 → 3,2,2 covering 0..7 exactly.
        let specs = plan_shards(7, 3, ShardStrategy::ColumnParallel).unwrap();
        assert_eq!(specs.iter().map(|s| s.len).collect::<Vec<_>>(), vec![3, 2, 2]);
        let last = specs.last().unwrap();
        assert_eq!(last.start + last.len, 7);
        assert!(specs.iter().all(|s| s.shard_dim == 1));
    }

    #[test]
    fn plan_zero_shards_errs() {
        assert!(plan_shards(4, 0, ShardStrategy::RowParallel).is_err());
    }

    #[test]
    fn row_shard_then_gather_roundtrips() {
        // 4x2 matrix.
        let m = t(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0], vec![4, 2]);
        let specs = plan_shards(4, 2, ShardStrategy::RowParallel).unwrap();
        let shards: Vec<NumTensor> = specs.iter().map(|s| shard_tensor(&m, s).unwrap()).collect();
        assert_eq!(shards[0].shape, vec![2, 2]);
        assert_eq!(shards[0].data, vec![1.0, 2.0, 3.0, 4.0]);
        assert_eq!(shards[1].data, vec![5.0, 6.0, 7.0, 8.0]);
        let back = gather_shards(&shards, ShardStrategy::RowParallel).unwrap();
        assert_eq!(back, m);
    }

    #[test]
    fn col_shard_then_gather_roundtrips() {
        // 2x4 matrix.
        let m = t(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0], vec![2, 4]);
        let specs = plan_shards(4, 2, ShardStrategy::ColumnParallel).unwrap();
        let shards: Vec<NumTensor> = specs.iter().map(|s| shard_tensor(&m, s).unwrap()).collect();
        // shard 0 = cols 0..2 of each row.
        assert_eq!(shards[0].shape, vec![2, 2]);
        assert_eq!(shards[0].data, vec![1.0, 2.0, 5.0, 6.0]);
        assert_eq!(shards[1].data, vec![3.0, 4.0, 7.0, 8.0]);
        let back = gather_shards(&shards, ShardStrategy::ColumnParallel).unwrap();
        assert_eq!(back, m);
    }

    #[test]
    fn shard_out_of_bounds_errs() {
        let m = t(vec![1.0, 2.0, 3.0, 4.0], vec![2, 2]);
        let bad = ShardSpec {
            shard_dim: 0,
            start: 1,
            len: 5,
            dim_total: 2,
            strategy: ShardStrategy::RowParallel,
        };
        assert!(shard_tensor(&m, &bad).is_err());
    }

    #[test]
    fn shard_dim_total_mismatch_errs() {
        let m = t(vec![1.0, 2.0, 3.0, 4.0], vec![2, 2]);
        let bad = ShardSpec {
            shard_dim: 0,
            start: 0,
            len: 1,
            dim_total: 99,
            strategy: ShardStrategy::RowParallel,
        };
        assert!(shard_tensor(&m, &bad).is_err());
    }
}
