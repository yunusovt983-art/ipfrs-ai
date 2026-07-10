//! `TieredStore` — the synchronous, tested [`BlockStore`] of Slice 1 running on top
//! of a real async IPFRS storage backend (`ipfrs-storage`), with [`MemStore`] kept
//! as the hot tier.
//!
//! Why the hot tier is mandatory, not an optimisation: our sync [`BlockStore::get`]
//! returns `Option<&[u8]>` — a *borrow*. A sled/network backend hands back *owned*
//! `Block`s across an `.await`; you cannot borrow out of that. So bytes must first
//! land in an owned in-memory map before a synchronous borrow is possible. The
//! `MemStore` **is** the read-face of the backend.
//!
//! The async boundary lives only at the edges:
//!   1. [`TieredStore::hydrate`] — async BFS over the DAG from a root CID, pulling
//!      reachable blocks cold → hot (content-addressed links make the reachable set
//!      knowable up front).
//!   2. run all the synchronous Slice-1 code (`get`/`insert`/`export`/`render`)
//!      against the warm hot tier — no `.await`, tests stay instant.
//!   3. [`TieredStore::flush`] — async, push dirty blocks hot → cold. Everything is
//!      immutable and content-addressed, so ordering is irrelevant and re-flushing
//!      is idempotent.

use std::collections::HashSet;
use std::sync::Arc;

use bytes::Bytes;
use ipfrs_core::{Block, Cid, Ipld};
use ipfrs_storage::BlockStoreTrait;

use crate::error::{KError, KResult};
use crate::store::{BlockStore, MemStore};

/// A two-tier block store: `MemStore` (hot, owns the borrow) in front of any async
/// `ipfrs-storage` backend (cold, persistent/network).
pub struct TieredStore {
    hot: MemStore,
    /// CIDs written locally that are not yet persisted to the cold tier.
    dirty: HashSet<Cid>,
    cold: Arc<dyn BlockStoreTrait>,
}

impl TieredStore {
    /// Wrap a cold backend (e.g. `SledBlockStore`, `MemoryBlockStore`, a gateway).
    pub fn new(cold: Arc<dyn BlockStoreTrait>) -> Self {
        Self { hot: MemStore::new(), dirty: HashSet::new(), cold }
    }

    /// Number of blocks awaiting persistence.
    pub fn dirty_len(&self) -> usize {
        self.dirty.len()
    }

    /// Persist every dirty block to the cold tier, then clear the dirty set.
    /// Idempotent: re-flushing already-persisted content is a no-op by CID.
    pub async fn flush(&mut self) -> KResult<()> {
        let cids: Vec<Cid> = self.dirty.iter().copied().collect();
        for cid in cids {
            // Copy the bytes out and drop the borrow before awaiting.
            let Some(bytes) = self.hot.get(&cid).map(Bytes::copy_from_slice) else {
                continue;
            };
            // from_parts preserves our dag-cbor CID (backends key by block.cid()).
            let block = Block::from_parts(cid, bytes);
            self.cold.put(&block).await.map_err(KError::Core)?;
            self.dirty.remove(&cid);
        }
        Ok(())
    }

    /// Pull the transitive closure of `root` from the cold tier into the hot tier,
    /// following dag-cbor tag-42 links. Returns the number of blocks fetched.
    pub async fn hydrate(&mut self, root: &Cid) -> KResult<usize> {
        let mut fetched = 0;
        let mut seen = HashSet::new();
        let mut frontier = vec![*root];
        while let Some(cid) = frontier.pop() {
            if !seen.insert(cid) || self.hot.has(&cid) {
                continue;
            }
            let Some(block) = self.cold.get(&cid).await.map_err(KError::Core)? else {
                continue; // dangling link (e.g. optional evidence not stored yet)
            };
            let bytes = block.data().to_vec();
            // Follow links before we hand the bytes to the hot tier.
            if let Ok(ipld) = Ipld::from_dag_cbor(&bytes) {
                collect_links(&ipld, &mut frontier);
            }
            self.hot.put(cid, bytes); // hydrated == already persisted → not dirty
            fetched += 1;
        }
        Ok(fetched)
    }
}

/// Recursively gather every CID link reachable inside an IPLD value.
fn collect_links(ipld: &Ipld, out: &mut Vec<Cid>) {
    collect_links_filtered(ipld, out, false)
}

/// Link walker with an option to skip the `prev` field of a `KnowledgeRoot` — used
/// by GC to treat version history as detachable (see [`crate::gc`]).
pub(crate) fn collect_links_filtered(ipld: &Ipld, out: &mut Vec<Cid>, skip_prev: bool) {
    match ipld {
        Ipld::Link(c) => out.push(c.0),
        Ipld::Map(m) => {
            for (k, v) in m {
                if skip_prev && k == "prev" {
                    continue;
                }
                collect_links_filtered(v, out, skip_prev);
            }
        }
        Ipld::List(l) => l.iter().for_each(|v| collect_links_filtered(v, out, skip_prev)),
        _ => {}
    }
}

impl BlockStore for TieredStore {
    fn put(&mut self, cid: Cid, bytes: Vec<u8>) {
        self.dirty.insert(cid);
        self.hot.put(cid, bytes);
    }
    fn get(&self, cid: &Cid) -> Option<&[u8]> {
        self.hot.get(cid)
    }
    fn has(&self, cid: &Cid) -> bool {
        self.hot.has(cid)
    }
    fn len(&self) -> usize {
        self.hot.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{EntitySpec, KnowledgeGraph};
    use crate::project;
    use ipfrs_storage::MemoryBlockStore;

    fn spec(kind: &str, name: &str) -> EntitySpec {
        EntitySpec { kind: kind.into(), name: name.into(), aliases: vec![], attrs: Default::default() }
    }

    /// The whole point of Slice 2: a graph built in one "session", flushed to a cold
    /// backend, then reopened by a *fresh* TieredStore that shares only the cold
    /// tier — reconstructs byte-for-byte. Proves the knowledge layer survives a
    /// process restart while the sync core never changed.
    #[tokio::test]
    async fn graph_survives_flush_and_rehydrate() {
        let cold: Arc<dyn BlockStoreTrait> = Arc::new(MemoryBlockStore::new());

        // --- session 1: build, commit, flush hot → cold -------------------
        let (head, pages_before) = {
            let mut kg = KnowledgeGraph::new(TieredStore::new(cold.clone())).unwrap();
            let ada = kg.add_entity(spec("person", "Ada Lovelace")).unwrap();
            let eng = kg.add_entity(spec("machine", "Analytical Engine")).unwrap();
            let algo = kg.add_entity(spec("concept", "Algorithm")).unwrap();
            kg.add_relation(ada, "designed", eng, 0.9, vec![]).unwrap();
            kg.add_relation(ada, "invented", algo, 1.0, vec![]).unwrap();
            let head = kg.commit().unwrap();
            assert!(kg.store().dirty_len() > 0);
            kg.store_mut().flush().await.unwrap();
            assert_eq!(kg.store().dirty_len(), 0, "everything persisted");
            (head, project::render(&kg).unwrap())
        };

        // The cold backend now physically holds the blocks.
        assert!(!cold.is_empty(), "cold tier populated");

        // --- session 2: fresh hot tier, shares only the cold backend ------
        let mut ts = TieredStore::new(cold.clone());
        assert_eq!(ts.len(), 0, "hot tier starts empty");
        let n = ts.hydrate(&head).await.unwrap();
        assert!(n >= 6, "hydrated head + 2 hamts + 3 entities + rel lists, got {n}");

        let kg2 = KnowledgeGraph::open(ts, &head).unwrap();
        let pages_after = project::render(&kg2).unwrap();

        assert_eq!(pages_before, pages_after, "projection identical after rehydrate");
        assert!(pages_after.get("ada-lovelace.md").unwrap().contains("[[analytical-engine]]"));
    }

    /// The same proof as above, but through a *real* on-disk sled database that is
    /// fully closed and reopened as a separate instance — genuine persistence, not
    /// a shared in-memory Arc.
    #[tokio::test]
    async fn graph_survives_real_disk_restart() {
        use ipfrs_storage::{BlockStoreConfig, SledBlockStore};

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("blocks");

        // --- session 1: flush to a real sled DB, then close it (drop) ------
        let (head, pages_before) = {
            let sled = SledBlockStore::new(BlockStoreConfig::testing().with_path(path.clone())).unwrap();
            let cold: Arc<dyn BlockStoreTrait> = Arc::new(sled);
            let mut kg = KnowledgeGraph::new(TieredStore::new(cold.clone())).unwrap();
            let ada = kg.add_entity(spec("person", "Ada Lovelace")).unwrap();
            let eng = kg.add_entity(spec("machine", "Analytical Engine")).unwrap();
            kg.add_relation(ada, "designed", eng, 0.9, vec![]).unwrap();
            let head = kg.commit().unwrap();
            kg.store_mut().flush().await.unwrap();
            cold.flush().await.unwrap(); // fsync sled to disk
            (head, project::render(&kg).unwrap())
        }; // kg + sled instance dropped here → DB closed

        // --- session 2: a brand-new sled instance over the SAME path ------
        let sled2 = SledBlockStore::new(BlockStoreConfig::testing().with_path(path.clone())).unwrap();
        let cold2: Arc<dyn BlockStoreTrait> = Arc::new(sled2);
        assert!(!cold2.is_empty(), "blocks recovered from disk");
        let mut ts = TieredStore::new(cold2);
        ts.hydrate(&head).await.unwrap();
        let kg2 = KnowledgeGraph::open(ts, &head).unwrap();
        assert_eq!(pages_before, project::render(&kg2).unwrap(), "survives real disk restart");
    }

    #[tokio::test]
    async fn flush_is_idempotent() {
        let cold: Arc<dyn BlockStoreTrait> = Arc::new(MemoryBlockStore::new());
        let mut kg = KnowledgeGraph::new(TieredStore::new(cold.clone())).unwrap();
        kg.add_entity(spec("person", "Grace")).unwrap();
        kg.commit().unwrap();
        kg.store_mut().flush().await.unwrap();
        let n1 = cold.len();
        // second flush writes nothing new
        kg.store_mut().flush().await.unwrap();
        assert_eq!(cold.len(), n1);
    }
}
