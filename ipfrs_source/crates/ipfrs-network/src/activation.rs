//! Distributed activation-streaming protocol (RoadMap Phase 5.1).
//!
//! A libp2p `request-response` protocol (`/ipfrs/activation/1.0.0`) that carries
//! one partition's work between peers: the requester ships a self-describing
//! [`StageRequest`] (the subgraph to run plus the boundary activations it needs)
//! and the executing peer returns a [`StageResponse`] with the requested output
//! tensors.
//!
//! ## Why the wire types live elsewhere (DDD / no cycle)
//!
//! [`StageRequest`] / [`StageResponse`] are defined in `ipfrs-tensorlogic`
//! (`distributed::wire`) — the pure, network-free layer this crate *depends on*.
//! Keeping them there preserves the `network → tensorlogic` dependency direction
//! (so there is no `network ↔ tensorlogic` cycle) and lets the orchestration logic
//! be unit-tested without a swarm via `distributed::transport::LocalTransport`.
//! This module only names the protocol and re-exports those types for the
//! request-response behaviour wired up in [`crate::node`].

pub use ipfrs_tensorlogic::distributed::wire::{StageRequest, StageResponse, WireTensor};

/// libp2p protocol name for distributed activation streaming.
pub const PROTOCOL: &str = "/ipfrs/activation/1.0.0";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_is_versioned() {
        assert_eq!(PROTOCOL, "/ipfrs/activation/1.0.0");
    }

    #[test]
    fn wire_tensor_serde_roundtrip() {
        let w = WireTensor {
            data: vec![1.0, -2.0, 0.5],
            shape: vec![1, 3],
        };
        let bytes = serde_json::to_vec(&w).unwrap();
        let back: WireTensor = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(back, w);
    }
}
