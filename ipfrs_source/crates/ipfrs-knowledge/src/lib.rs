//! `ipfrs-knowledge` — a typed, content-addressed knowledge graph over IPFRS IPLD.
//!
//! The source of truth is an immutable DAG of typed [`KnowledgeNode`]s (DAG-CBOR,
//! tag-42 CID links). A block-backed [`hamt`] indexes entities by stable identity,
//! a [`KnowledgeGraph`] adds relations and a chained head-log, and everything can
//! be projected — losslessly and deterministically — into a Petgraph for
//! algorithms or into Markdown for a human-readable Wiki view.
//!
//! ```
//! use ipfrs_knowledge::{KnowledgeGraph, EntitySpec, MemStore, project};
//! let mut kg = KnowledgeGraph::new(MemStore::new()).unwrap();
//! let ada = kg.add_entity(EntitySpec { kind: "person".into(), name: "Ada".into(),
//!     aliases: vec![], attrs: Default::default() }).unwrap();
//! let eng = kg.add_entity(EntitySpec { kind: "machine".into(), name: "Engine".into(),
//!     aliases: vec![], attrs: Default::default() }).unwrap();
//! kg.add_relation(ada, "designed", eng, 1.0, vec![]).unwrap();
//! let head = kg.commit().unwrap();               // content-addressed head CID
//! let pages = project::render(&kg).unwrap();      // deterministic Wiki projection
//! assert!(pages.contains_key("ada.md"));
//! let _ = head;
//! ```

pub mod error;
pub mod gc;
pub mod graph;
pub mod hamt;
pub mod node;
pub mod project;
pub mod store;
pub mod tiered;

pub use error::{KError, KResult};
pub use gc::{collect as gc_collect, GcReport};
pub use graph::{EntitySpec, KnowledgeGraph};
pub use node::{EntityId, HypothesisStatus, KnowledgeNode};
pub use store::{BlockStore, MemStore};
pub use tiered::TieredStore;
