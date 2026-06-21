//! Model announcement over gossipsub (RoadMap Phase 1.2 + 2).
//!
//! When a node publishes a model it announces the model's `Cid` on the
//! [`MODELS_TOPIC`] gossipsub topic (and provides it in the DHT). Subscribers
//! receive the announcement as a [`crate::NetworkEvent::GossipMessage`] and can
//! decode it back into a `Cid` with [`decode_announcement`], then resolve/fetch
//! the manifest via the geo-fetch path.

use ipfrs_core::Cid;

/// Gossipsub topic on which model CIDs are announced.
pub const MODELS_TOPIC: &str = "/ipfrs/models";

/// Encode a model CID into a gossip announcement payload (raw CID bytes).
pub fn encode_announcement(model_cid: &Cid) -> Vec<u8> {
    model_cid.to_bytes()
}

/// Decode a gossip announcement payload back into a model CID, or `None` if the
/// bytes are not a valid CID.
pub fn decode_announcement(data: &[u8]) -> Option<Cid> {
    Cid::try_from(data).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ipfrs_core::CidBuilder;

    fn sample_cid() -> Cid {
        CidBuilder::new().build(b"model-weights-v1").unwrap()
    }

    #[test]
    fn round_trip() {
        let cid = sample_cid();
        let payload = encode_announcement(&cid);
        assert_eq!(decode_announcement(&payload), Some(cid));
    }

    #[test]
    fn garbage_is_none() {
        assert_eq!(decode_announcement(&[0xff, 0x00, 0x01]), None);
    }

    #[test]
    fn topic_is_stable() {
        assert_eq!(MODELS_TOPIC, "/ipfrs/models");
    }
}
