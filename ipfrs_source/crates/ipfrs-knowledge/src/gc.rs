//! Reachability-based garbage collection over the cold (persistent) tier.
//!
//! Every mutation path-copies the HAMT spine and writes a new `KnowledgeRoot`, so
//! superseded blocks accumulate. GC is mark-and-sweep: mark the transitive closure
//! of a set of pinned head CIDs, then delete every stored block outside it.
//!
//! `keep_history` controls whether the `prev` chain of a `KnowledgeRoot` is a live
//! link. With `keep_history = false`, superseded versions become collectable while
//! structural sharing keeps blocks still referenced by the current head alive — so
//! collecting old history is cheap and never touches shared data.

use std::collections::HashSet;
use std::sync::Arc;

use ipfrs_core::{Cid, Ipld};
use ipfrs_storage::BlockStoreTrait;

use crate::error::{KError, KResult};
use crate::tiered::collect_links_filtered;

/// Outcome of a sweep.
#[derive(Debug, Clone)]
pub struct GcReport {
    /// Number of blocks retained (the live set size).
    pub kept: usize,
    /// CIDs that were deleted.
    pub deleted: Vec<Cid>,
}

/// The live set: transitive closure of `pins` over dag-cbor links. When
/// `keep_history` is false, `KnowledgeRoot.prev` links are not followed.
pub async fn reachable(
    cold: &Arc<dyn BlockStoreTrait>,
    pins: &[Cid],
    keep_history: bool,
) -> KResult<HashSet<Cid>> {
    let mut seen = HashSet::new();
    let mut frontier: Vec<Cid> = pins.to_vec();
    while let Some(cid) = frontier.pop() {
        if !seen.insert(cid) {
            continue;
        }
        let Some(block) = cold.get(&cid).await.map_err(KError::Core)? else {
            continue; // dangling (e.g. history already collected) — tolerate
        };
        if let Ok(ipld) = Ipld::from_dag_cbor(block.data()) {
            collect_links_filtered(&ipld, &mut frontier, !keep_history);
        }
    }
    Ok(seen)
}

/// Mark from `pins`, then sweep every stored block not in the live set.
pub async fn collect(
    cold: &Arc<dyn BlockStoreTrait>,
    pins: &[Cid],
    keep_history: bool,
) -> KResult<GcReport> {
    let live = reachable(cold, pins, keep_history).await?;
    let all = cold.list_cids().map_err(KError::Core)?;
    let mut deleted = Vec::new();
    for cid in all {
        if !live.contains(&cid) {
            cold.delete(&cid).await.map_err(KError::Core)?;
            deleted.push(cid);
        }
    }
    Ok(GcReport { kept: live.len(), deleted })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{EntitySpec, KnowledgeGraph};
    use crate::tiered::TieredStore;
    use crate::project;
    use ipfrs_storage::MemoryBlockStore;

    fn spec(kind: &str, name: &str) -> EntitySpec {
        EntitySpec { kind: kind.into(), name: name.into(), aliases: vec![], attrs: Default::default() }
    }

    /// After a second commit, dropping history collects the superseded root/index
    /// blocks but keeps the shared entity block, and the current head still
    /// reconstructs fully.
    #[tokio::test]
    async fn gc_drops_history_keeps_shared_blocks() {
        let cold: Arc<dyn BlockStoreTrait> = Arc::new(MemoryBlockStore::new());
        let mut kg = KnowledgeGraph::new(TieredStore::new(cold.clone())).unwrap();

        let ada = kg.add_entity(spec("person", "Ada")).unwrap();
        let head1 = kg.commit().unwrap();
        let ada_cid = kg.entity_cid(&ada).unwrap().unwrap();

        kg.add_entity(spec("person", "Grace")).unwrap();
        let head2 = kg.commit().unwrap();
        kg.store_mut().flush().await.unwrap();

        let before = cold.len();
        let report = collect(&cold, &[head2], /* keep_history */ false).await.unwrap();

        assert!(!report.deleted.is_empty(), "superseded blocks collected");
        assert!(cold.len() < before, "store shrank");
        assert!(cold.get(&head1).await.unwrap().is_none(), "old head collected");
        assert!(cold.has(&ada_cid).await.unwrap(), "shared entity block survives");
        assert!(report.deleted.contains(&head1));

        // Current head still fully reconstructs (prev now dangles — tolerated).
        let mut ts = TieredStore::new(cold.clone());
        ts.hydrate(&head2).await.unwrap();
        let kg2 = KnowledgeGraph::open(ts, &head2).unwrap();
        let pages = project::render(&kg2).unwrap();
        assert!(pages.contains_key("ada.md") && pages.contains_key("grace.md"));
    }

    /// With history kept, `prev` keeps the whole chain live — nothing collected.
    #[tokio::test]
    async fn gc_keep_history_retains_chain() {
        let cold: Arc<dyn BlockStoreTrait> = Arc::new(MemoryBlockStore::new());
        let mut kg = KnowledgeGraph::new(TieredStore::new(cold.clone())).unwrap();
        kg.add_entity(spec("person", "Ada")).unwrap();
        let head1 = kg.commit().unwrap();
        kg.add_entity(spec("person", "Grace")).unwrap();
        let head2 = kg.commit().unwrap();
        kg.store_mut().flush().await.unwrap();

        let report = collect(&cold, &[head2], /* keep_history */ true).await.unwrap();
        assert!(report.deleted.is_empty(), "history reachable via prev, nothing collected");
        assert!(cold.has(&head1).await.unwrap(), "old head retained");
    }

    /// Blocks from an unpinned independent graph are swept.
    #[tokio::test]
    async fn gc_sweeps_unpinned_graph() {
        let cold: Arc<dyn BlockStoreTrait> = Arc::new(MemoryBlockStore::new());

        let head_a = {
            let mut a = KnowledgeGraph::new(TieredStore::new(cold.clone())).unwrap();
            a.add_entity(spec("person", "Ada")).unwrap();
            let h = a.commit().unwrap();
            a.store_mut().flush().await.unwrap();
            h
        };
        let _head_b = {
            let mut b = KnowledgeGraph::new(TieredStore::new(cold.clone())).unwrap();
            b.add_entity(spec("machine", "Zeta")).unwrap();
            let h = b.commit().unwrap();
            b.store_mut().flush().await.unwrap();
            h
        };

        // Pin only A; B is unreachable and must be swept.
        let report = collect(&cold, &[head_a], true).await.unwrap();
        assert!(!report.deleted.is_empty());
        let mut ts = TieredStore::new(cold.clone());
        ts.hydrate(&head_a).await.unwrap();
        assert!(KnowledgeGraph::open(ts, &head_a).is_ok(), "pinned graph intact");
    }
}
