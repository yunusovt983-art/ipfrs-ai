//! Geo-distributed inference operations for `Node` (RoadMap Phase 4 MVP).
//!
//! This is the integration seam between the pure routing planner (`crate::geo`)
//! and the live network: resolve providers of a model/block CID, rank them with
//! [`crate::geo::plan_routing`], then fetch from the chosen peer(s) using the
//! real block-fetch protocol (RoadMap Phase 1.1, `/ipfrs/blockfetch/1.0.0`).
//!
//! NOTE (MVP scope): RTT/region/load are not yet populated — that comes from
//! Phase 3 (wiring `QualityPredictor`/`GeoRouter`). Until then candidates carry
//! neutral metrics and routing degenerates to deterministic peer ordering. Actual
//! inference *execution* on the fetched model is Phase 5. gRPC/GraphQL surfacing
//! is a follow-up.

use std::collections::HashMap;

use ipfrs_core::{Block, Cid, Error, Result};

use super::Node;
use crate::geo::{plan_routing, PeerCandidate, RoutingError, RoutingPolicy};

impl Node {
    /// Geo-aware fetch of a content-addressed block (e.g. a model manifest or
    /// layer) from the best available provider.
    ///
    /// Pipeline: `find_providers(cid)` → build candidates → `plan_routing` →
    /// hedged fetch over the chosen peers (sequential in this MVP; parallel
    /// hedging per ADR-004 is a follow-up). The returned block is integrity-
    /// verified by the transport layer before delivery.
    pub async fn geo_fetch_block(&mut self, cid: &Cid, policy: &RoutingPolicy) -> Result<Block> {
        // 1. Resolve providers via the DHT.
        let providers = self.find_providers(cid).await?;
        if providers.is_empty() {
            return Err(Error::NotFound(format!("no providers found for {}", cid)));
        }

        // 2. Build candidates. Metrics are neutral until Phase 3 (QualityPredictor).
        //    Keep a string→PeerId lookup so we can act on the planner's decision.
        let mut by_id: HashMap<String, ipfrs_network::libp2p::PeerId> = HashMap::new();
        let candidates: Vec<PeerCandidate> = providers
            .iter()
            .map(|p| {
                let id = p.to_string();
                by_id.insert(id.clone(), *p);
                PeerCandidate {
                    peer_id: id,
                    region: String::new(),
                    rtt_ms: 0.0,
                    load: 0.0,
                    has_model: true,
                }
            })
            .collect();

        // 3. Plan routing (primary + hedge peers).
        let decision = plan_routing(&candidates, policy).map_err(|e| match e {
            RoutingError::NoCandidates => Error::NotFound(format!("no candidates for {}", cid)),
            RoutingError::NoModelHolder => {
                Error::NotFound(format!("no provider holds {}", cid))
            }
        })?;

        // 4. Hedged fetch: try peers in ranked order, return first verified block.
        let mut last_err =
            Error::NotFound(format!("no peer served {} within policy", cid));
        for peer_str in decision.all() {
            let Some(peer) = by_id.get(&peer_str).copied() else {
                continue;
            };
            match self.network_mut()?.fetch_block_from_peer(&peer, cid).await {
                Ok(block) => return Ok(block),
                Err(e) => last_err = e,
            }
        }
        Err(last_err)
    }

    /// Convenience wrapper using the default [`RoutingPolicy`].
    pub async fn geo_fetch_block_default(&mut self, cid: &Cid) -> Result<Block> {
        let policy = RoutingPolicy::default();
        self.geo_fetch_block(cid, &policy).await
    }
}
