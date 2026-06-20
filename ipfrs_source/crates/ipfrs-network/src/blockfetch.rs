//! Block-fetch protocol: real block exchange over the libp2p swarm.
//!
//! RoadMap Phase 1.1 / ADR-001: a thin libp2p `request-response` protocol
//! (`/ipfrs/blockfetch/1.0.0`) that lets a node pull a `Block` by CID from a
//! connected peer. This closes the `fetch_block_from_peer` stub and unblocks
//! P2P GET (see RoadMap/06a-BlockFetch-Design.md).
//!
//! The wire types are serde-encoded via libp2p's CBOR request-response codec
//! (`request_response::cbor::Behaviour`). CIDs travel as raw bytes to avoid any
//! string-encoding ambiguity; the receiver re-validates `cid == hash(data)`.

use serde::{Deserialize, Serialize};

/// libp2p protocol name for block fetch.
pub const PROTOCOL: &str = "/ipfrs/blockfetch/1.0.0";

/// Upper bound on a served block (mirrors `ipfrs_core` `MAX_BLOCK_SIZE` = 2 MiB).
pub const MAX_BLOCK_BYTES: u32 = 2 * 1024 * 1024;

/// Request a single block by its CID (raw multihash bytes).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockRequest {
    /// `cid.to_bytes()` of the wanted block.
    pub cid: Vec<u8>,
    /// Reject responses whose payload exceeds this size (bytes).
    pub max_size: u32,
}

impl BlockRequest {
    /// Build a request for the given CID with the default size cap.
    pub fn new(cid_bytes: Vec<u8>) -> Self {
        Self {
            cid: cid_bytes,
            max_size: MAX_BLOCK_BYTES,
        }
    }
}

/// Response to a [`BlockRequest`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BlockResponse {
    /// The block bytes; the receiver MUST verify `cid == hash(data)`.
    Block(Vec<u8>),
    /// The peer does not hold the requested block.
    NotFound,
    /// The block exists but exceeds the requester's `max_size`.
    TooLarge,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_defaults_to_max_block_size() {
        let r = BlockRequest::new(vec![1, 2, 3]);
        assert_eq!(r.max_size, MAX_BLOCK_BYTES);
        assert_eq!(r.cid, vec![1, 2, 3]);
    }

    #[test]
    fn protocol_name_is_versioned() {
        assert_eq!(PROTOCOL, "/ipfrs/blockfetch/1.0.0");
    }

    #[test]
    fn responses_are_distinct() {
        assert_ne!(BlockResponse::NotFound, BlockResponse::TooLarge);
        assert_eq!(BlockResponse::Block(vec![9]), BlockResponse::Block(vec![9]));
    }
}
