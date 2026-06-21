//! Geo-distributed inference operations for `Node` (RoadMap Phase 4 MVP).
//!
//! Thin delegation to [`ipfrs_network::NetworkNode::geo_fetch_block`], which does
//! the real work: resolve providers of a CID, rank them with the geo routing
//! planner (`ipfrs_network::geo`), then fetch from the chosen peer(s) over the
//! block-fetch protocol (RoadMap Phase 1.1).
//!
//! Inference *execution* on the fetched model is Phase 5; candidate RTT/region
//! metrics are neutral until Phase 3.

use ipfrs_core::{Block, Cid, Result};

use super::Node;
use crate::geo::RoutingPolicy;

impl Node {
    /// Geo-aware fetch of a content-addressed block (model manifest or layer)
    /// from the best available provider, using the given routing policy.
    pub async fn geo_fetch_block(&mut self, cid: &Cid, policy: &RoutingPolicy) -> Result<Block> {
        self.network_mut()?.geo_fetch_block(cid, policy).await
    }

    /// Convenience wrapper using the default [`RoutingPolicy`].
    pub async fn geo_fetch_block_default(&mut self, cid: &Cid) -> Result<Block> {
        self.network_mut()?
            .geo_fetch_block(cid, &RoutingPolicy::default())
            .await
    }

    /// Subscribe to peer load advertisements so geo routing can weight peers by
    /// load (RoadMap Phase 3). Call once after `start()`.
    pub fn subscribe_peer_load(&self) -> Result<()> {
        self.network()?.subscribe_load()
    }

    /// Advertise this node's current load in `[0.0, 1.0]` to peers (RoadMap
    /// Phase 3) so they avoid routing geo-fetches to us when we are busy.
    pub fn advertise_load(&self, load: f32) -> Result<()> {
        self.network()?.publish_load(load)
    }
}
