//! Distributed semantic search protocol (RoadMap Phase 1.3).
//!
//! A thin libp2p `request-response` protocol (`/ipfrs/semsearch/1.0.0`) that lets
//! a node ask a peer to run a vector similarity query against the peer's local
//! semantic index and return the top hits. This closes the semantic-DHT
//! transport stub (previously `query_peer` returned `None`).
//!
//! Wire types are dependency-free (no coupling to `ipfrs-semantic`): hits carry
//! the CID as a string and a score; the application maps to/from its own
//! `SearchResult` at the boundary.

use serde::{Deserialize, Serialize};

/// libp2p protocol name for distributed semantic search.
pub const PROTOCOL: &str = "/ipfrs/semsearch/1.0.0";

/// Upper bound on requested/returned results (guards against abuse).
pub const MAX_RESULTS: u32 = 1000;

/// Ask a peer for the `k` nearest neighbours of `embedding`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SemSearchRequest {
    /// Query vector.
    pub embedding: Vec<f32>,
    /// Number of results requested.
    pub k: u32,
}

/// A single search hit: content CID (string) + similarity score.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SemHit {
    pub cid: String,
    pub score: f32,
}

/// Response: the peer's top hits (already truncated to `k`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SemSearchResponse {
    pub hits: Vec<SemHit>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_is_versioned() {
        assert_eq!(PROTOCOL, "/ipfrs/semsearch/1.0.0");
    }

    #[test]
    fn types_round_trip_equality() {
        let req = SemSearchRequest {
            embedding: vec![0.1, 0.2, 0.3],
            k: 5,
        };
        assert_eq!(req.clone(), req);
        let resp = SemSearchResponse {
            hits: vec![SemHit {
                cid: "bafyX".into(),
                score: 0.9,
            }],
        };
        assert_eq!(resp.clone(), resp);
    }
}
