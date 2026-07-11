//! A content-addressed vector index for semantic-ish search over the graph.
//!
//! [`embed`] is a deterministic, dependency-free **character n-gram hashing
//! embedding** (signed FNV hashing into a fixed-dim, L2-normalized vector) — good
//! for fuzzy/lexical similarity on-device, no model to ship. The index itself is a
//! real cosine-similarity substrate (exact brute-force top-k) and serializes to an
//! IPLD block, so it lives in the same DAG as the knowledge.
//!
//! Swapping in a learned embedding model is a one-function change (`embed`) that
//! leaves the index, storage, and query path untouched — the same "swap the
//! backing" shape as [`crate::TieredStore`]. Brute-force search is O(n) and exact;
//! a large deployment would swap it for an ANN structure (HNSW) the same way.

use std::collections::HashSet;

use ipfrs_core::{Cid, CidBuilder, Ipld};

use crate::error::{KError, KResult};
use crate::graph::KnowledgeGraph;
use crate::node::KnowledgeNode;
use crate::store::BlockStore;

/// Default embedding dimensionality.
pub const DEFAULT_DIM: usize = 256;

/// A dense embedding vector.
pub type Embedding = Vec<f32>;

/// Character-trigram hashing embedding: deterministic, L2-normalized, `dim`-wide.
pub fn embed(text: &str, dim: usize) -> Embedding {
    let mut v = vec![0f32; dim];
    let normalized = text.trim().to_lowercase();
    let chars: Vec<char> = format!(" {normalized} ").chars().collect();
    for w in chars.windows(3) {
        // signed FNV-1a over the trigram — sign halves collision bias.
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for &c in w {
            h ^= c as u64;
            h = h.wrapping_mul(0x1000_0000_01b3);
        }
        let idx = (h % dim as u64) as usize;
        let sign = if (h >> 63) & 1 == 1 { -1.0 } else { 1.0 };
        v[idx] += sign;
    }
    let mag: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if mag > 0.0 {
        for x in &mut v {
            *x /= mag;
        }
    }
    v
}

/// Cosine similarity of two equal-length vectors (== dot product when normalized).
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// A flat vector index: `(target CID → embedding)`, exact cosine top-k.
#[derive(Debug, Clone)]
pub struct VectorIndex {
    dim: usize,
    entries: Vec<(Cid, Embedding)>,
}

impl VectorIndex {
    pub fn new(dim: usize) -> Self {
        Self { dim, entries: Vec::new() }
    }

    pub fn dim(&self) -> usize {
        self.dim
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Add a target's embedding (length must equal `dim`).
    pub fn add(&mut self, target: Cid, emb: Embedding) {
        debug_assert_eq!(emb.len(), self.dim);
        self.entries.push((target, emb));
    }

    /// Drop any embedding stored under `target`.
    pub fn remove(&mut self, target: &Cid) {
        self.entries.retain(|(c, _)| c != target);
    }

    /// Insert or replace the embedding for `target` (keeps at most one per CID).
    pub fn upsert(&mut self, target: Cid, emb: Embedding) {
        self.remove(&target);
        self.add(target, emb);
    }

    /// Embed `text` and add it under `target`.
    pub fn add_text(&mut self, target: Cid, text: &str) {
        self.add(target, embed(text, self.dim));
    }

    /// Top-`k` targets by cosine similarity to `query`, highest first.
    pub fn search(&self, query: &[f32], k: usize) -> Vec<(Cid, f32)> {
        let mut scored: Vec<(Cid, f32)> =
            self.entries.iter().map(|(c, e)| (*c, cosine(query, e))).collect();
        scored.sort_by(|a, b| b.1.total_cmp(&a.1));
        scored.truncate(k);
        scored
    }

    /// Convenience: embed a query string then search.
    pub fn search_text(&self, query: &str, k: usize) -> Vec<(Cid, f32)> {
        self.search(&embed(query, self.dim), k)
    }

    // ---- content-addressed persistence -----------------------------------

    fn to_ipld(&self) -> Ipld {
        let entries = self
            .entries
            .iter()
            .map(|(cid, emb)| {
                let mut bytes = Vec::with_capacity(emb.len() * 4);
                for f in emb {
                    bytes.extend_from_slice(&f.to_le_bytes());
                }
                Ipld::Map(
                    [("c".to_string(), Ipld::link(*cid)), ("v".to_string(), Ipld::Bytes(bytes))]
                        .into_iter()
                        .collect(),
                )
            })
            .collect();
        Ipld::Map(
            [
                ("@type".to_string(), Ipld::String("vindex".into())),
                ("dim".to_string(), Ipld::Integer(self.dim as i128)),
                ("entries".to_string(), Ipld::List(entries)),
            ]
            .into_iter()
            .collect(),
        )
    }

    /// Serialize to a content-addressed block; returns `(cid, bytes)`.
    pub fn encode(&self) -> KResult<(Cid, Vec<u8>)> {
        let bytes = self.to_ipld().to_dag_cbor().map_err(KError::Core)?;
        let cid = CidBuilder::new().build_dag_cbor(&bytes).map_err(KError::Core)?;
        Ok((cid, bytes))
    }

    /// Persist into a block store, returning the index CID.
    pub fn store<S: BlockStore>(&self, store: &mut S) -> KResult<Cid> {
        let (cid, bytes) = self.encode()?;
        store.put(cid, bytes);
        Ok(cid)
    }

    pub fn decode(bytes: &[u8]) -> KResult<Self> {
        let m = match Ipld::from_dag_cbor(bytes).map_err(KError::Core)? {
            Ipld::Map(m) => m,
            _ => return Err(KError::Decode("vindex not a map".into())),
        };
        // Validate `dim` from untrusted input: it becomes a modulo divisor and an
        // allocation size in `embed`, so 0 (divide-by-zero) or a huge value (OOM)
        // must be rejected rather than trusted.
        let dim = match m.get("dim") {
            Some(Ipld::Integer(n)) if *n >= 1 && *n <= 65_536 => *n as usize,
            Some(Ipld::Integer(_)) => return Err(KError::Decode("vindex dim out of range".into())),
            _ => return Err(KError::Decode("vindex missing dim".into())),
        };
        let list = match m.get("entries") {
            Some(Ipld::List(l)) => l,
            _ => return Err(KError::Decode("vindex missing entries".into())),
        };
        let mut entries = Vec::with_capacity(list.len());
        for e in list {
            let em = match e {
                Ipld::Map(m) => m,
                _ => return Err(KError::Decode("vindex entry not a map".into())),
            };
            let cid = em
                .get("c")
                .and_then(|v| v.as_link().copied())
                .ok_or_else(|| KError::Decode("vindex entry missing cid".into()))?;
            let bytes = match em.get("v") {
                Some(Ipld::Bytes(b)) => b,
                _ => return Err(KError::Decode("vindex entry missing vec".into())),
            };
            let emb: Vec<f32> =
                bytes.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
            if emb.len() != dim {
                return Err(KError::Decode("vindex entry vector length != dim".into()));
            }
            entries.push((cid, emb));
        }
        Ok(Self { dim, entries })
    }
}

/// The searchable text of a node, if it carries any.
pub fn node_text(node: &KnowledgeNode) -> Option<String> {
    use KnowledgeNode::*;
    Some(match node {
        Entity { name, aliases, attrs, .. } => {
            let mut t = name.clone();
            for a in aliases {
                t.push(' ');
                t.push_str(a);
            }
            for v in attrs.values() {
                t.push(' ');
                t.push_str(v);
            }
            t
        }
        Concept { name, definition, .. } => format!("{name} {definition}"),
        Observation { statement, .. } => statement.clone(),
        Hypothesis { claim, .. } => claim.clone(),
        Relation { predicate, .. } => predicate.clone(),
        Evidence { .. } => return None, // text lives in the linked source block
    })
}

/// Extract searchable text from a raw block: a knowledge node's [`node_text`], or a
/// bare `Ipld::String` source document.
pub(crate) fn block_text(bytes: &[u8]) -> Option<String> {
    if let Ok(node) = KnowledgeNode::decode(bytes) {
        return node_text(&node);
    }
    match Ipld::from_dag_cbor(bytes) {
        Ok(Ipld::String(s)) => Some(s),
        _ => None,
    }
}

/// Build a vector index over a graph: every entity (name + aliases + attribute
/// values) and, for every `Evidence` reachable through relations, the text of its
/// linked source block — keyed by the Evidence CID. Targets are node CIDs; resolve
/// them back with [`KnowledgeGraph::get_node_public`].
pub fn index_graph<S: BlockStore>(kg: &KnowledgeGraph<S>, dim: usize) -> KResult<VectorIndex> {
    let mut ix = VectorIndex::new(dim);
    let mut seen_ev = HashSet::new();
    for id in kg.entity_ids()? {
        if let Some(node) = kg.get_entity(&id)? {
            if let Some(text) = node_text(&node) {
                let cid = kg.entity_cid(&id)?.expect("indexed entity has a cid");
                ix.add_text(cid, &text);
            }
        }
        for (_, rel) in kg.relations_of(&id)? {
            if let KnowledgeNode::Relation { evidence, .. } = rel {
                for ev in evidence {
                    if !seen_ev.insert(ev) {
                        continue;
                    }
                    if let Ok(KnowledgeNode::Evidence { source, .. }) = kg.get_node_public(&ev) {
                        if let Some(text) = kg.store().get(&source).and_then(block_text) {
                            ix.add_text(ev, &text);
                        }
                    }
                }
            }
        }
    }
    Ok(ix)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::EntitySpec;
    use crate::store::MemStore;

    fn spec(kind: &str, name: &str) -> EntitySpec {
        EntitySpec { kind: kind.into(), name: name.into(), aliases: vec![], attrs: Default::default() }
    }

    #[test]
    fn embed_is_deterministic_and_normalized() {
        let a = embed("Analytical Engine", DEFAULT_DIM);
        let b = embed("Analytical Engine", DEFAULT_DIM);
        assert_eq!(a, b);
        let mag: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((mag - 1.0).abs() < 1e-4, "unit length, got {mag}");
        // self-similarity is 1, distinct text is lower
        assert!((cosine(&a, &b) - 1.0).abs() < 1e-4);
        assert!(cosine(&a, &embed("Photosynthesis", DEFAULT_DIM)) < 0.9);
    }

    #[test]
    fn search_ranks_lexical_match_first() {
        let mut kg = KnowledgeGraph::new(MemStore::new()).unwrap();
        let ada = kg.add_entity(spec("person", "Ada Lovelace")).unwrap();
        let eng = kg.add_entity(spec("machine", "Analytical Engine")).unwrap();
        let _photo = kg.add_entity(spec("concept", "Photosynthesis")).unwrap();

        let ix = index_graph(&kg, DEFAULT_DIM).unwrap();
        assert_eq!(ix.len(), 3);

        let top = ix.search_text("lovelace", 1)[0].0;
        assert_eq!(top, kg.entity_cid(&ada).unwrap().unwrap(), "lovelace -> Ada");
        let top = ix.search_text("analytical", 1)[0].0;
        assert_eq!(top, kg.entity_cid(&eng).unwrap().unwrap(), "analytical -> Engine");
    }

    #[test]
    fn decode_rejects_out_of_range_dim() {
        use ipfrs_core::Ipld;
        // dim = 0 would divide-by-zero in embed; must be rejected at decode.
        let bad = Ipld::Map(
            [
                ("@type".to_string(), Ipld::String("vindex".into())),
                ("dim".to_string(), Ipld::Integer(0)),
                ("entries".to_string(), Ipld::List(vec![])),
            ]
            .into_iter()
            .collect(),
        )
        .to_dag_cbor()
        .unwrap();
        assert!(VectorIndex::decode(&bad).is_err());

        let huge = Ipld::Map(
            [
                ("@type".to_string(), Ipld::String("vindex".into())),
                ("dim".to_string(), Ipld::Integer(1_000_000)),
                ("entries".to_string(), Ipld::List(vec![])),
            ]
            .into_iter()
            .collect(),
        )
        .to_dag_cbor()
        .unwrap();
        assert!(VectorIndex::decode(&huge).is_err());
    }

    #[test]
    fn index_round_trips_through_dag_cbor() {
        let mut kg = KnowledgeGraph::new(MemStore::new()).unwrap();
        kg.add_entity(spec("person", "Ada Lovelace")).unwrap();
        kg.add_entity(spec("machine", "Analytical Engine")).unwrap();
        let ix = index_graph(&kg, DEFAULT_DIM).unwrap();

        let (_cid, bytes) = ix.encode().unwrap();
        let back = VectorIndex::decode(&bytes).unwrap();
        assert_eq!(back.dim(), ix.dim());
        assert_eq!(back.entries, ix.entries);
        // queries behave identically after a round-trip
        assert_eq!(ix.search_text("engine", 2), back.search_text("engine", 2));
    }

    #[test]
    fn evidence_source_is_indexed_and_searchable() {
        use crate::store::BlockStore;
        use ipfrs_core::{CidBuilder, Ipld};

        let mut kg = KnowledgeGraph::new(MemStore::new()).unwrap();
        let ada = kg.add_entity(spec("person", "Ada Lovelace")).unwrap();
        let eng = kg.add_entity(spec("machine", "Analytical Engine")).unwrap();

        // A source document (a quoted passage) as its own content-addressed block.
        let src_bytes = Ipld::String("Menabrea memoir sketch of the engine".into())
            .to_dag_cbor()
            .unwrap();
        let src_cid = CidBuilder::new().build_dag_cbor(&src_bytes).unwrap();
        kg.store_mut().put(src_cid, src_bytes);

        let ev = KnowledgeNode::Evidence {
            source: src_cid,
            span: None,
            extracted_by: "test".into(),
            proof: None,
        };
        let ev_cid = kg.put_node_public(&ev).unwrap();
        kg.add_relation(ada, "designed", eng, 1.0, vec![ev_cid]).unwrap();

        let ix = index_graph(&kg, DEFAULT_DIM).unwrap();
        assert_eq!(ix.len(), 3, "2 entities + 1 evidence (by its source text)");
        let top = ix.search_text("memoir", 1)[0].0;
        assert_eq!(top, ev_cid, "evidence retrievable by the content of its source");
    }
}
