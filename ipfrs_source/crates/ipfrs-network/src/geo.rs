//! Geo-distributed inference: routing & hedging policy (MVP scaffold).
//!
//! This module implements the *pure decision logic* of geo-distributed inference
//! (RoadMap/06-GeoInference.md, Phase 4.2): given a set of peer candidates that
//! hold a model (addressed by CID), pick the best primary peer plus `k-1` hedge
//! peers according to {has-model, region affinity, RTT, load}.
//!
//! It is intentionally dependency-free (std only) so it can be unit-tested in
//! isolation. At network-integration time, `PeerId`/`Cid` aliases below are
//! replaced by `libp2p::PeerId` / `ipfrs_core::Cid`, and candidates are produced
//! from `find_providers(model_cid)` + `QualityPredictor`/`PeerSelector`
//! (see ADR-GeoInference ADR-003/ADR-004).

/// Peer identifier (placeholder; becomes `libp2p::PeerId` on integration).
pub type PeerId = String;
/// Content identifier of a model manifest (placeholder; becomes `ipfrs_core::Cid`).
pub type ModelCid = String;

/// A candidate peer for serving an inference request.
#[derive(Debug, Clone, PartialEq)]
pub struct PeerCandidate {
    /// Peer identity.
    pub peer_id: PeerId,
    /// Geographic/region tag (e.g. "eu", "us-east", "ap").
    pub region: String,
    /// Measured round-trip latency in milliseconds (lower is better).
    pub rtt_ms: f64,
    /// Current load in `[0.0, 1.0]` (lower is better).
    pub load: f32,
    /// Whether the peer already holds the requested model CID.
    pub has_model: bool,
}

/// Policy controlling routing and hedging.
#[derive(Debug, Clone)]
pub struct RoutingPolicy {
    /// How many peers to hedge across (1 = no hedging). See ADR-004.
    pub hedge_k: usize,
    /// Overall time budget for the request, milliseconds.
    pub deadline_ms: u64,
    /// Preferred region; candidates in this region get an affinity bonus.
    pub prefer_region: Option<String>,
    /// Data-residency constraint (RoadMap Phase 6): when set, only peers whose
    /// region is in this list are eligible. `None` = no restriction.
    pub allowed_regions: Option<Vec<String>>,
    /// Whether the caller will verify `result_cid` / block integrity (ADR-005).
    pub verify: bool,
}

impl Default for RoutingPolicy {
    fn default() -> Self {
        Self {
            hedge_k: 2,
            deadline_ms: 5_000,
            prefer_region: None,
            allowed_regions: None,
            verify: true,
        }
    }
}

/// The routing decision: a primary peer plus hedge peers (ADR-004).
#[derive(Debug, Clone, PartialEq)]
pub struct RoutingDecision {
    /// First peer to try (best score).
    pub primary: PeerId,
    /// Additional peers queried in parallel (hedging); excludes `primary`.
    pub hedged: Vec<PeerId>,
}

impl RoutingDecision {
    /// All peers to contact, primary first.
    pub fn all(&self) -> Vec<PeerId> {
        let mut v = Vec::with_capacity(1 + self.hedged.len());
        v.push(self.primary.clone());
        v.extend(self.hedged.iter().cloned());
        v
    }
}

/// Reasons routing can fail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoutingError {
    /// No candidate holds the requested model.
    NoModelHolder,
    /// Candidate list was empty.
    NoCandidates,
    /// No model-holding peer is within the policy's `allowed_regions`.
    NoRegionMatch,
}

/// Lower score = better. Combines region affinity, RTT and load.
///
/// - region match with `prefer_region` subtracts a fixed affinity bonus,
/// - RTT contributes linearly (ms),
/// - load contributes scaled to a comparable magnitude.
fn score(c: &PeerCandidate, prefer_region: Option<&str>) -> f64 {
    const REGION_BONUS_MS: f64 = 50.0; // a same-region peer is worth ~50ms of RTT
    const LOAD_WEIGHT_MS: f64 = 100.0; // full load (1.0) costs ~100ms-equivalent
    let region_bonus = match prefer_region {
        Some(r) if r == c.region => REGION_BONUS_MS,
        _ => 0.0,
    };
    c.rtt_ms + (c.load as f64) * LOAD_WEIGHT_MS - region_bonus
}

/// Plan routing for a model request (RoadMap Phase 4.2).
///
/// Filters to peers that hold the model, ranks them by [`score`], then returns a
/// [`RoutingDecision`] with the best as primary and up to `hedge_k - 1` hedges.
pub fn plan_routing(
    candidates: &[PeerCandidate],
    policy: &RoutingPolicy,
) -> Result<RoutingDecision, RoutingError> {
    if candidates.is_empty() {
        return Err(RoutingError::NoCandidates);
    }
    let mut holders: Vec<&PeerCandidate> = candidates.iter().filter(|c| c.has_model).collect();
    if holders.is_empty() {
        return Err(RoutingError::NoModelHolder);
    }
    // Data-residency: drop peers outside the allowed regions (RoadMap Phase 6).
    if let Some(allowed) = &policy.allowed_regions {
        holders.retain(|c| allowed.iter().any(|r| r == &c.region));
        if holders.is_empty() {
            return Err(RoutingError::NoRegionMatch);
        }
    }
    let prefer = policy.prefer_region.as_deref();
    // Sort ascending by score; tie-break by peer_id for determinism.
    holders.sort_by(|a, b| {
        score(a, prefer)
            .partial_cmp(&score(b, prefer))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.peer_id.cmp(&b.peer_id))
    });
    let k = policy.hedge_k.max(1).min(holders.len());
    let primary = holders[0].peer_id.clone();
    let hedged = holders[1..k].iter().map(|c| c.peer_id.clone()).collect();
    Ok(RoutingDecision { primary, hedged })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(id: &str, region: &str, rtt: f64, load: f32, has: bool) -> PeerCandidate {
        PeerCandidate {
            peer_id: id.into(),
            region: region.into(),
            rtt_ms: rtt,
            load,
            has_model: has,
        }
    }

    #[test]
    fn empty_candidates_errs() {
        let p = RoutingPolicy::default();
        assert_eq!(plan_routing(&[], &p), Err(RoutingError::NoCandidates));
    }

    #[test]
    fn no_holder_errs() {
        let p = RoutingPolicy::default();
        let cs = vec![cand("a", "eu", 10.0, 0.1, false)];
        assert_eq!(plan_routing(&cs, &p), Err(RoutingError::NoModelHolder));
    }

    #[test]
    fn picks_lowest_rtt_holder_as_primary() {
        let p = RoutingPolicy { hedge_k: 2, ..Default::default() };
        let cs = vec![
            cand("far", "us", 200.0, 0.0, true),
            cand("near", "eu", 20.0, 0.0, true),
            cand("nomodel", "eu", 1.0, 0.0, false),
        ];
        let d = plan_routing(&cs, &p).unwrap();
        assert_eq!(d.primary, "near");
        assert_eq!(d.hedged, vec!["far".to_string()]);
        assert_eq!(d.all(), vec!["near".to_string(), "far".to_string()]);
    }

    #[test]
    fn region_affinity_can_beat_rtt() {
        // same-region peer at 60ms beats other-region at 20ms (50ms bonus).
        let p = RoutingPolicy {
            hedge_k: 1,
            prefer_region: Some("eu".into()),
            ..Default::default()
        };
        let cs = vec![
            cand("other", "us", 20.0, 0.0, true),
            cand("local", "eu", 60.0, 0.0, true),
        ];
        let d = plan_routing(&cs, &p).unwrap();
        assert_eq!(d.primary, "local");
        assert!(d.hedged.is_empty()); // hedge_k = 1
    }

    #[test]
    fn load_penalizes() {
        let p = RoutingPolicy { hedge_k: 1, ..Default::default() };
        let cs = vec![
            cand("busy", "eu", 10.0, 0.9, true), // 10 + 90 = 100
            cand("idle", "eu", 50.0, 0.0, true), // 50 + 0  = 50
        ];
        let d = plan_routing(&cs, &p).unwrap();
        assert_eq!(d.primary, "idle");
    }

    #[test]
    fn hedge_k_capped_to_holders() {
        let p = RoutingPolicy { hedge_k: 5, ..Default::default() };
        let cs = vec![cand("a", "eu", 10.0, 0.0, true), cand("b", "eu", 20.0, 0.0, true)];
        let d = plan_routing(&cs, &p).unwrap();
        assert_eq!(d.all().len(), 2); // only 2 holders available
    }

    #[test]
    fn data_residency_filters_other_regions() {
        let p = RoutingPolicy {
            hedge_k: 5,
            allowed_regions: Some(vec!["eu".into()]),
            ..Default::default()
        };
        let cs = vec![
            cand("us-fast", "us", 5.0, 0.0, true), // excluded by residency
            cand("eu-slow", "eu", 80.0, 0.0, true),
        ];
        let d = plan_routing(&cs, &p).unwrap();
        assert_eq!(d.all(), vec!["eu-slow".to_string()]); // only EU peer eligible
    }

    #[test]
    fn data_residency_no_match_errs() {
        let p = RoutingPolicy {
            allowed_regions: Some(vec!["ap".into()]),
            ..Default::default()
        };
        let cs = vec![cand("a", "eu", 10.0, 0.0, true), cand("b", "us", 20.0, 0.0, true)];
        assert_eq!(plan_routing(&cs, &p), Err(RoutingError::NoRegionMatch));
    }
}
