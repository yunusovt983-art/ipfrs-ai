//! Model announcement/discovery operations for `Node` (RoadMap Phase 1.2 + 2).
//!
//! Ties gossipsub to model distribution: announcing a model both **provides** its
//! CID in the DHT (so `find_providers`/`geo_fetch` can locate it) and **gossips**
//! the CID on [`MODELS_TOPIC`] so subscribed peers learn about it immediately.

use std::sync::Arc;

use ipfrs_core::{Cid, Result};
use ipfrs_network::models::{decode_announcement, encode_announcement, MODELS_TOPIC};
use ipfrs_network::NetworkEvent;

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

    /// Start the background consumer that drains network events and records model
    /// CIDs announced on [`MODELS_TOPIC`] into the node's known-models registry.
    ///
    /// Takes the network event receiver (single-consumer); a second call after a
    /// successful take is a no-op. Call once after `start()`, typically alongside
    /// [`Node::subscribe_models`]. The task ends when the network shuts down.
    pub fn start_model_consumer(&mut self) -> Result<()> {
        let mut rx = match self.network_mut()?.take_event_receiver() {
            Some(rx) => rx,
            None => return Ok(()), // already taken / consumer already running
        };
        let registry = Arc::clone(&self.known_models);
        tokio::spawn(async move {
            while let Some(ev) = rx.recv().await {
                if let NetworkEvent::GossipMessage { topic, data, .. } = ev {
                    if topic == MODELS_TOPIC {
                        if let Some(cid) = decode_announcement(&data) {
                            *registry.write().entry(cid).or_insert(0) += 1;
                        }
                    }
                }
            }
        });
        Ok(())
    }

    /// All model CIDs learned from gossip announcements so far.
    pub fn known_models(&self) -> Vec<Cid> {
        self.known_models.read().keys().copied().collect()
    }

    /// How many times a given model CID has been announced (0 if unseen).
    pub fn model_announcement_count(&self, cid: &Cid) -> u32 {
        self.known_models.read().get(cid).copied().unwrap_or(0)
    }
}
