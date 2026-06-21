//! Region-aware federated averaging (RoadMap Phase 6 — FedAvg by regions).
//!
//! Builds on the plain [`super::backward_pass::federated_average`] to support
//! geo-distributed training: aggregate gradients per region, restrict to allowed
//! regions (data-residency), or do a two-level (hierarchical) average that gives
//! each region equal weight regardless of how many peers it contributed.

use std::collections::BTreeMap;

use super::backward_pass::federated_average;
use super::GradientError;

/// Average gradients grouped by region tag → `{ region: mean gradient }`.
///
/// Each region is averaged independently (FedAvg within the region). Regions are
/// returned in sorted order for determinism.
pub fn federated_average_by_region(
    tagged: &[(String, Vec<f32>)],
) -> Result<BTreeMap<String, Vec<f32>>, GradientError> {
    if tagged.is_empty() {
        return Err(GradientError::EmptyGradients);
    }
    let mut groups: BTreeMap<String, Vec<Vec<f32>>> = BTreeMap::new();
    for (region, grad) in tagged {
        groups.entry(region.clone()).or_default().push(grad.clone());
    }
    let mut out = BTreeMap::new();
    for (region, grads) in groups {
        out.insert(region, federated_average(&grads)?);
    }
    Ok(out)
}

/// Average only gradients whose region is in `allowed` (one global mean) —
/// data-residency-constrained FedAvg. Errors `EmptyGradients` if none match.
pub fn federated_average_in_regions(
    tagged: &[(String, Vec<f32>)],
    allowed: &[String],
) -> Result<Vec<f32>, GradientError> {
    let filtered: Vec<Vec<f32>> = tagged
        .iter()
        .filter(|(r, _)| allowed.iter().any(|a| a == r))
        .map(|(_, g)| g.clone())
        .collect();
    federated_average(&filtered)
}

/// Two-level (hierarchical) FedAvg: average within each region, then average the
/// per-region means into one global gradient. Each region gets **equal weight**,
/// mitigating imbalance when some regions have far more peers than others.
pub fn hierarchical_federated_average(
    tagged: &[(String, Vec<f32>)],
) -> Result<Vec<f32>, GradientError> {
    let per_region = federated_average_by_region(tagged)?;
    let means: Vec<Vec<f32>> = per_region.into_values().collect();
    federated_average(&means)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tagged() -> Vec<(String, Vec<f32>)> {
        vec![
            ("eu".into(), vec![0.0, 0.0]),
            ("eu".into(), vec![2.0, 2.0]), // eu mean = [1,1]
            ("us".into(), vec![4.0, 4.0]), // us mean = [4,4]
        ]
    }

    #[test]
    fn by_region_groups_and_averages() {
        let m = federated_average_by_region(&tagged()).unwrap();
        assert_eq!(m.get("eu").unwrap(), &vec![1.0, 1.0]);
        assert_eq!(m.get("us").unwrap(), &vec![4.0, 4.0]);
    }

    #[test]
    fn in_regions_filters() {
        let g = federated_average_in_regions(&tagged(), &["eu".into()]).unwrap();
        assert_eq!(g, vec![1.0, 1.0]); // only the two eu peers
    }

    #[test]
    fn in_regions_no_match_errs() {
        let r = federated_average_in_regions(&tagged(), &["ap".into()]);
        assert!(matches!(r, Err(GradientError::EmptyGradients)));
    }

    #[test]
    fn hierarchical_equal_weights_regions() {
        // Flat mean over 3 peers = [2,2]; hierarchical = mean([1,1],[4,4]) = [2.5,2.5].
        let g = hierarchical_federated_average(&tagged()).unwrap();
        assert_eq!(g, vec![2.5, 2.5]);
    }

    #[test]
    fn empty_errs() {
        assert!(matches!(
            federated_average_by_region(&[]),
            Err(GradientError::EmptyGradients)
        ));
    }
}
