//! Model announcement/discovery operations for `Node` (RoadMap Phase 1.2 + 2).
//!
//! Ties gossipsub to model distribution: announcing a model both **provides** its
//! CID in the DHT (so `find_providers`/`geo_fetch` can locate it) and **gossips**
//! the CID on [`MODELS_TOPIC`] so subscribed peers learn about it immediately.

use std::collections::HashMap;
use std::sync::Arc;

use ipfrs_core::{Cid, Result};
use ipfrs_network::models::{decode_announcement, encode_announcement, MODELS_TOPIC};
use ipfrs_network::{NetworkEvent, INFERENCE_REQUEST_TOPIC, INFERENCE_RESULT_TOPIC};
use ipfrs_tensorlogic::{InferenceRequest, InferenceResponse};

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
        // Capture handles for inference serving/correlation before taking the rx.
        let tensorlogic = self.tensorlogic().ok().map(Arc::clone);
        let publisher = self.network()?.topic_publisher();
        let inference_waiters = Arc::clone(&self.network()?.inference_waiters);
        let local_peer = self.network()?.peer_id().to_string();

        let mut rx = match self.network_mut()?.take_event_receiver() {
            Some(rx) => rx,
            None => return Ok(()), // already taken / consumer already running
        };
        let registry = Arc::clone(&self.known_models);

        tokio::spawn(async move {
            while let Some(ev) = rx.recv().await {
                let NetworkEvent::GossipMessage { topic, data, .. } = ev else {
                    continue;
                };
                if topic == MODELS_TOPIC {
                    // Model announcement → record in the known-models registry.
                    if let Some(cid) = decode_announcement(&data) {
                        *registry.write().entry(cid).or_insert(0) += 1;
                    }
                } else if topic == INFERENCE_REQUEST_TOPIC {
                    // Serve a remote inference request from the local KB (Phase 1.2).
                    let Ok(req) = serde_json::from_slice::<InferenceRequest>(&data) else {
                        continue;
                    };
                    if req.requester_peer_id == local_peer {
                        continue; // don't answer our own request
                    }
                    if let (Some(tl), Some(pubh)) = (&tensorlogic, &publisher) {
                        let resp = serve_inference(&req, tl, &local_peer);
                        if let Ok(json) = serde_json::to_vec(&resp) {
                            pubh.publish(INFERENCE_RESULT_TOPIC, json);
                        }
                    }
                } else if topic == INFERENCE_RESULT_TOPIC {
                    // Deliver a remote response to any waiter registered by
                    // distributed_infer (correlated by request_id).
                    if let Ok(resp) = serde_json::from_slice::<InferenceResponse>(&data) {
                        let mut waiters = inference_waiters.lock().await;
                        if let Some(senders) = waiters.remove(&resp.request_id) {
                            for tx in senders {
                                let _ = tx.send(resp.clone());
                            }
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

/// Run a remote inference request against the local KB and build the wire
/// response (RoadMap Phase 1.2). Pure helper used by the gossip consumer.
fn serve_inference(
    req: &InferenceRequest,
    tl: &ipfrs_tensorlogic::TensorLogicStore<super::NodeStore>,
    local_peer: &str,
) -> InferenceResponse {
    let pred = ipfrs_tensorlogic::parse_query(&req.goal).ok();

    let bindings: Vec<HashMap<String, String>> = pred
        .as_ref()
        .and_then(|p| tl.infer(p).ok())
        .map(|subs| {
            subs.into_iter()
                .map(|s| {
                    s.into_iter()
                        .map(|(k, v)| (k, v.to_string()))
                        .collect::<HashMap<String, String>>()
                })
                .collect()
        })
        .unwrap_or_default();

    // Proof-carrying inference (RoadMap Phase 6): attach a serialized proof tree
    // for explainability when the engine can produce one.
    let proof_json = pred
        .as_ref()
        .and_then(|p| tl.prove(p).ok().flatten())
        .and_then(|proof| serde_json::to_string(&proof).ok());

    InferenceResponse {
        request_id: req.request_id.clone(),
        proof_found: !bindings.is_empty(),
        bindings,
        error: None,
        responder_peer_id: local_peer.to_string(),
        proof_json,
    }
}
