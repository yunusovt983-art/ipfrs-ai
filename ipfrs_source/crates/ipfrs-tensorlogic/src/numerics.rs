//! Pure-`f32` numeric kernels for the inference engine (RoadMap Phase 5, Spike 3).
//!
//! Numerically-stable implementations of the activation / normalization primitives
//! the symbolic op set needs but the tree-walking [`numeric_exec`](crate::numeric_exec)
//! interpreter did not yet have: stable softmax (log-sum-exp), layer-norm,
//! RMS-norm, GELU (tanh approximation) and SiLU/Swish.
//!
//! ## Provenance (DDD: ACL port, not a dependency)
//!
//! These are an Anti-Corruption-Layer port of `oxigaf`'s
//! `oxigaf-diffusion/src/numerics.rs` selective-FP32 kernels. The algorithms are
//! the same well-known formulas, re-expressed in our Ubiquitous Language: failures
//! surface as [`GraphError`] (not a separate `NumericsError`), and the kernels
//! operate on flat `&[f32]` slices so the engine can apply them per-row / per-axis.
//! Nothing here depends on `oxigaf`; only the kernel shapes were worth borrowing.

use crate::computation_graph::GraphError;

/// GELU activation (tanh approximation, as used by BERT/GPT-2).
///
/// `gelu(x) = 0.5 · x · (1 + tanh(√(2/π) · (x + 0.044715·x³)))`
#[inline]
pub fn gelu(x: f32) -> f32 {
    const SQRT_2_OVER_PI: f32 = 0.797_884_6;
    const COEF: f32 = 0.044_715;
    0.5 * x * (1.0 + (SQRT_2_OVER_PI * (x + COEF * x * x * x)).tanh())
}

/// SiLU / Swish activation: `silu(x) = x · sigmoid(x) = x / (1 + e⁻ˣ)`.
#[inline]
pub fn silu(x: f32) -> f32 {
    x / (1.0 + (-x).exp())
}

/// Numerically-stable softmax over a flat slice, in place.
///
/// Subtracts the max before `exp()` (log-sum-exp trick) so large logits do not
/// overflow. Empty input is a no-op; an all-`-inf` input is left unnormalized
/// rather than producing `NaN`. Output sums to 1.0 up to rounding.
pub fn softmax_inplace(logits: &mut [f32]) {
    if logits.is_empty() {
        return;
    }
    let max = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let mut sum = 0.0_f32;
    for v in logits.iter_mut() {
        *v = (*v - max).exp();
        sum += *v;
    }
    if sum > 0.0 {
        for v in logits.iter_mut() {
            *v /= sum;
        }
    }
}

/// Owning variant of [`softmax_inplace`].
pub fn softmax(logits: &[f32]) -> Vec<f32> {
    let mut out = logits.to_vec();
    softmax_inplace(&mut out);
    out
}

/// Numerically-stable log-softmax: `x_i − log Σ exp(x_j)`, via log-sum-exp.
///
/// `exp(log_softmax(x)) ≈ softmax(x)`. Empty input returns empty; a single
/// element returns `[0.0]`.
pub fn log_softmax(logits: &[f32]) -> Vec<f32> {
    if logits.is_empty() {
        return Vec::new();
    }
    let max = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let log_sum_exp: f32 = logits.iter().map(|&x| (x - max).exp()).sum::<f32>().ln();
    logits.iter().map(|&x| (x - max) - log_sum_exp).collect()
}

/// Layer normalization over a flat slice with optional affine parameters:
///
/// `out_i = (x_i − mean) / √(var + eps) · gamma_i + beta_i`
///
/// Mean and variance are accumulated in `f64` for precision. `gamma` / `beta`,
/// when present, must match `input.len()`.
///
/// # Errors
/// - [`GraphError::InvalidGraph`] if `input` is empty.
/// - [`GraphError::ShapeMismatch`] if `gamma` / `beta` length ≠ `input.len()`.
pub fn layer_norm(
    input: &[f32],
    gamma: Option<&[f32]>,
    beta: Option<&[f32]>,
    eps: f32,
) -> Result<Vec<f32>, GraphError> {
    if input.is_empty() {
        return Err(GraphError::InvalidGraph("layer_norm: empty input".to_string()));
    }
    check_affine_len("layer_norm", "gamma", gamma, input.len())?;
    check_affine_len("layer_norm", "beta", beta, input.len())?;

    let n = input.len() as f64;
    let mean = (input.iter().map(|&x| x as f64).sum::<f64>() / n) as f32;
    let var =
        (input.iter().map(|&x| (x as f64 - mean as f64).powi(2)).sum::<f64>() / n) as f32;
    let inv_std = 1.0 / (var + eps).sqrt();

    Ok(input
        .iter()
        .enumerate()
        .map(|(i, &x)| {
            let normalized = (x - mean) * inv_std;
            let scaled = gamma.map_or(normalized, |g| normalized * g[i]);
            beta.map_or(scaled, |b| scaled + b[i])
        })
        .collect())
}

/// Root-mean-square normalization (RMSNorm, as in LLaMA/PaLM):
///
/// `rms = √(mean(x²) + eps)`,  `out_i = x_i / rms · gamma_i`
///
/// Cheaper than [`layer_norm`] — no mean-centering. `gamma`, when present, must
/// match `input.len()`.
///
/// # Errors
/// - [`GraphError::InvalidGraph`] if `input` is empty.
/// - [`GraphError::ShapeMismatch`] if `gamma` length ≠ `input.len()`.
pub fn rms_norm(input: &[f32], gamma: Option<&[f32]>, eps: f32) -> Result<Vec<f32>, GraphError> {
    if input.is_empty() {
        return Err(GraphError::InvalidGraph("rms_norm: empty input".to_string()));
    }
    check_affine_len("rms_norm", "gamma", gamma, input.len())?;

    let n = input.len() as f64;
    let mean_sq = (input.iter().map(|&x| (x as f64).powi(2)).sum::<f64>() / n) as f32;
    let inv_rms = 1.0 / (mean_sq + eps).sqrt();

    Ok(input
        .iter()
        .enumerate()
        .map(|(i, &x)| {
            let normalized = x * inv_rms;
            gamma.map_or(normalized, |g| normalized * g[i])
        })
        .collect())
}

fn check_affine_len(
    op: &str,
    which: &str,
    param: Option<&[f32]>,
    expected: usize,
) -> Result<(), GraphError> {
    match param {
        Some(p) if p.len() != expected => Err(GraphError::ShapeMismatch(format!(
            "{op}: {which} length {} != input length {expected}",
            p.len()
        ))),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: &[f32], b: &[f32], tol: f32) {
        assert_eq!(a.len(), b.len(), "length mismatch");
        for (i, (x, y)) in a.iter().zip(b).enumerate() {
            assert!((x - y).abs() <= tol, "index {i}: {x} vs {y} (tol {tol})");
        }
    }

    #[test]
    fn gelu_known_values() {
        assert!((gelu(0.0)).abs() < 1e-6);
        assert!((gelu(1.0) - 0.8413).abs() < 2e-3);
        assert!(gelu(-1.0) < 0.0 && gelu(-1.0) > -0.2);
    }

    #[test]
    fn silu_known_values() {
        assert!(silu(0.0).abs() < 1e-6);
        assert!((silu(1.0) - 0.7311).abs() < 1e-3);
    }

    #[test]
    fn softmax_sums_to_one_and_stable() {
        let s = softmax(&[1.0, 2.0, 3.0]);
        assert!((s.iter().sum::<f32>() - 1.0).abs() < 1e-6);
        // No overflow for large logits.
        let big = softmax(&[1000.0, 1001.0]);
        assert!(big.iter().all(|v| v.is_finite()));
        assert!((big.iter().sum::<f32>() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn softmax_empty_and_single() {
        assert!(softmax(&[]).is_empty());
        assert_eq!(softmax(&[42.0]), vec![1.0]);
    }

    #[test]
    fn log_softmax_recovers_softmax() {
        let logits = [1.0, 2.0, 3.0, 0.5];
        let lsm = log_softmax(&logits);
        let sm = softmax(&logits);
        approx(&lsm.iter().map(|v| v.exp()).collect::<Vec<_>>(), &sm, 1e-5);
        assert!(lsm.iter().all(|&v| v <= 1e-6));
    }

    #[test]
    fn layer_norm_zero_mean_unit_var() {
        let out = layer_norm(&[1.0, 2.0, 3.0, 4.0, 5.0], None, None, 1e-5).unwrap();
        let mean = out.iter().sum::<f32>() / out.len() as f32;
        let var = out.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / out.len() as f32;
        assert!(mean.abs() < 1e-4, "mean {mean}");
        assert!((var - 1.0).abs() < 1e-3, "var {var}");
    }

    #[test]
    fn layer_norm_affine_shifts_and_scales() {
        let out = layer_norm(&[1.0, 2.0, 3.0], Some(&[2.0; 3]), Some(&[1.0; 3]), 1e-5).unwrap();
        let mean = out.iter().sum::<f32>() / 3.0;
        let var = out.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / 3.0;
        assert!((mean - 1.0).abs() < 1e-3);
        assert!((var - 4.0).abs() < 1e-2);
    }

    #[test]
    fn layer_norm_errors() {
        assert!(matches!(
            layer_norm(&[], None, None, 1e-5),
            Err(GraphError::InvalidGraph(_))
        ));
        assert!(matches!(
            layer_norm(&[1.0, 2.0, 3.0], Some(&[1.0; 5]), None, 1e-5),
            Err(GraphError::ShapeMismatch(_))
        ));
    }

    #[test]
    fn rms_norm_unit_rms_and_gamma() {
        let out = rms_norm(&[1.0, 2.0, 3.0, 4.0], None, 1e-6).unwrap();
        let rms = (out.iter().map(|x| x * x).sum::<f32>() / 4.0).sqrt();
        assert!((rms - 1.0).abs() < 1e-4, "rms {rms}");
        let g = rms_norm(&[1.0, 2.0, 3.0], Some(&[2.0; 3]), 1e-6).unwrap();
        let base = rms_norm(&[1.0, 2.0, 3.0], None, 1e-6).unwrap();
        approx(&g, &base.iter().map(|x| x * 2.0).collect::<Vec<_>>(), 1e-5);
    }

    #[test]
    fn rms_norm_errors() {
        assert!(matches!(
            rms_norm(&[], None, 1e-6),
            Err(GraphError::InvalidGraph(_))
        ));
        assert!(matches!(
            rms_norm(&[1.0; 4], Some(&[1.0; 3]), 1e-6),
            Err(GraphError::ShapeMismatch(_))
        ));
    }
}
