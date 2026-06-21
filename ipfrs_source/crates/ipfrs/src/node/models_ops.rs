//! Model announcement/discovery operations for `Node` (RoadMap Phase 1.2 + 2).
//!
//! Ties gossipsub to model distribution: announcing a model both **provides** its
//! CID in the DHT (so `find_providers`/`geo_fetch` can locate it) and **gossips**
//! the CID on [`MODELS_TOPIC`] so subscribed peers learn about it immediately.

use ipfrs_core::{Cid, Result};
use ipfrs_network::models::{decode_announcement, encode_announcement, MODELS_TOPIC};

use super::Node;

impl Node {
    /// Announce a model by CID: register as a DHT provider and gossip the CID on
    /// the models topic. Call after publishing the model's manifest/layers.
    pub async fn announce_model(&mut self, model_cid: &Cid) -> Result<()> {
        // Populate DHT providers so others can resolve+fetch the manifest.
        self.provide(model_cid).await?;
        // Broadcast the announcement to subscribed peers (best-effort).
        self.network()?
            .publish_topic(MODELS_TOPIC, encode_announcement(model_cid))?;
        Ok(())
    }

    /// Subscribe to the models topic so this node receives model announcements
    /// as `NetworkEvent::GossipMessage` (decode with [`Node::parse_model_announcement`]).
    pub fn subscribe_models(&self) -> Result<()> {
        self.network()?.subscribe_topic(MODELS_TOPIC)
    }

    /// Stop receiving model announcements.
    pub fn unsubscribe_models(&self) -> Result<()> {
        self.network()?.unsubscribe_topic(MODELS_TOPIC)
    }

    /// Decode a gossip announcement payload into a model `Cid` (helper for
    /// consumers of `NetworkEvent::GossipMessage` on [`MODELS_TOPIC`]).
    pub fn parse_model_announcement(data: &[u8]) -> Option<Cid> {
        decode_announcement(data)
    }
}
