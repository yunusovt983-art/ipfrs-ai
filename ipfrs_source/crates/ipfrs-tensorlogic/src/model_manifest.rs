//! Content-addressed model manifest (RoadMap Phase 2 / ADR-002).
//!
//! A model is published as a DAG: a `ModelManifest` IPLD node that references its
//! weight layers by CID. This enables:
//! - **lazy fetch**: a peer pulls only the layers it needs (via TensorSwap),
//! - **dedup across versions**: an unchanged layer keeps the same CID,
//! - **verifiable distribution**: `model_cid` is deterministic over the manifest.
//!
//! Mirrors the encoding approach of [`crate::ipld_codec`] (serde_json bytes +
//! DAG-CBOR-stamped CID via SHA2-256) so the codec stays dependency-light.

use bytes::Bytes;
use ipfrs_core::{Block, Cid, Error};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// DAG-CBOR codec code (0x71).
const DAG_CBOR_CODEC: u64 = 0x71;

/// Reference to one model layer/shard stored as its own content-addressed block.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LayerRef {
    /// CID string of the layer's `Block`.
    pub cid: String,
    /// Human-readable layer name (e.g. "model.layers.0.attn.q_proj").
    pub name: String,
    /// Tensor shape of the layer.
    pub shape: Vec<u64>,
    /// Optional per-layer dtype override (falls back to the manifest dtype).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dtype: Option<String>,
}

/// A content-addressed description of a model as a DAG of layer blocks.
///
/// `metadata` **is** part of the hash: two manifests differing only in metadata
/// get different `model_cid` (same rule as content-addressed rules).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelManifest {
    /// Architecture identifier (e.g. "llama-7b", "bert-base").
    pub arch: String,
    /// Default dtype for layers (e.g. "f16", "int8").
    pub dtype: String,
    /// Model version (semver recommended).
    pub version: String,
    /// Layer references, in model order.
    pub layers: Vec<LayerRef>,
    /// Arbitrary metadata (sorted for deterministic encoding).
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

impl ModelManifest {
    /// Create an empty manifest for the given architecture/dtype/version.
    pub fn new(
        arch: impl Into<String>,
        dtype: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        Self {
            arch: arch.into(),
            dtype: dtype.into(),
            version: version.into(),
            layers: Vec::new(),
            metadata: BTreeMap::new(),
        }
    }

    /// Append a layer reference (builder style).
    pub fn with_layer(mut self, layer: LayerRef) -> Self {
        self.layers.push(layer);
        self
    }

    /// Number of layers in the manifest.
    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    /// All layer CIDs (for provider announcement / prefetch planning).
    pub fn layer_cids(&self) -> impl Iterator<Item = &str> {
        self.layers.iter().map(|l| l.cid.as_str())
    }
}

/// Build a CIDv1 with DAG-CBOR codec (0x71) from raw bytes using SHA2-256.
fn build_dag_cbor_cid(data: &[u8]) -> Result<Cid, Error> {
    use ipfrs_core::CidBuilder;
    CidBuilder::new()
        .codec(DAG_CBOR_CODEC)
        .build(data)
        .map_err(|e| Error::Cid(format!("Failed to compute model manifest CID: {}", e)))
}

/// Serialize a manifest to a DAG-CBOR–stamped `Block`. Identical manifests yield
/// identical CIDs (deterministic content addressing).
pub fn manifest_to_block(manifest: &ModelManifest) -> Result<Block, Error> {
    let json_bytes = serde_json::to_vec(manifest)
        .map_err(|e| Error::Serialization(format!("model manifest serialization: {}", e)))?;
    let cid = build_dag_cbor_cid(&json_bytes)?;
    Ok(Block::from_parts(cid, Bytes::from(json_bytes)))
}

/// Decode a `Block` back into a `ModelManifest`.
pub fn block_to_manifest(block: &Block) -> Result<ModelManifest, Error> {
    serde_json::from_slice(block.data())
        .map_err(|e| Error::Deserialization(format!("model manifest deserialization: {}", e)))
}

/// Compute the content-addressed `model_cid` without storing the block.
pub fn model_cid(manifest: &ModelManifest) -> Result<Cid, Error> {
    let json_bytes = serde_json::to_vec(manifest)
        .map_err(|e| Error::Serialization(format!("model_cid serialization: {}", e)))?;
    build_dag_cbor_cid(&json_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ModelManifest {
        ModelManifest::new("bert-base", "f16", "1.0.0")
            .with_layer(LayerRef {
                cid: "bafyL0".into(),
                name: "embed".into(),
                shape: vec![30522, 768],
                dtype: None,
            })
            .with_layer(LayerRef {
                cid: "bafyL1".into(),
                name: "layer0.attn".into(),
                shape: vec![768, 768],
                dtype: Some("int8".into()),
            })
    }

    #[test]
    fn round_trip_block() {
        let m = sample();
        let block = manifest_to_block(&m).unwrap();
        let back = block_to_manifest(&block).unwrap();
        assert_eq!(m, back);
    }

    #[test]
    fn cid_is_deterministic() {
        let a = model_cid(&sample()).unwrap();
        let b = model_cid(&sample()).unwrap();
        assert_eq!(a, b);
        // and equals the block's CID
        assert_eq!(a, *manifest_to_block(&sample()).unwrap().cid());
    }

    #[test]
    fn metadata_changes_cid() {
        let mut m2 = sample();
        m2.metadata.insert("source".into(), "hub".into());
        assert_ne!(model_cid(&sample()).unwrap(), model_cid(&m2).unwrap());
    }

    #[test]
    fn layer_helpers() {
        let m = sample();
        assert_eq!(m.layer_count(), 2);
        assert_eq!(m.layer_cids().collect::<Vec<_>>(), vec!["bafyL0", "bafyL1"]);
    }
}
