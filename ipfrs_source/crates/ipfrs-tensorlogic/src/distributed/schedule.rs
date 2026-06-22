//! Per-stage communication schedule for a partitioned computation graph
//! (RoadMap Phase 5.1 — "partition graph + stream activations").
//!
//! The existing [`TensorGraphPartitioner`](crate::graph_partitioner) decides *which
//! node runs where* and counts cut edges, but produces no plan for *when* each
//! cross-partition activation must move. This module is an ACL port of `torsh`'s
//! `CommunicationSchedule` / `CommunicationStage` / `DataTransfer` types, with a
//! real scheduling algorithm built on our own [`Partition`] / [`GraphEdge`] data:
//! it derives a partition-level DAG from the cut edges and assigns each transfer
//! to the stage at which its source partition becomes ready (longest-path layering).
//!
//! The schedule is pure data — a future libp2p activation-streaming layer executes
//! stage `k` only once every transfer in stages `< k` has completed.

use std::collections::{HashMap, HashSet, VecDeque};

use crate::graph_partitioner::{GraphEdge, Partition};

/// One activation that must move from a producing partition to a consuming one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataTransfer {
    /// Partition that produced (and currently holds) the activation.
    pub from_partition: usize,
    /// Partition that needs the activation as input.
    pub to_partition: usize,
    /// Producing node whose output tensor is transferred.
    pub node_id: u64,
    /// Size of the transferred activation in bytes.
    pub data_bytes: u64,
}

/// A set of transfers that may all proceed once earlier stages have completed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommunicationStage {
    /// Zero-based stage index; lower stages run first.
    pub stage_id: usize,
    /// Transfers belonging to this stage (independent of one another).
    pub transfers: Vec<DataTransfer>,
}

/// An ordered, staged plan for moving every cross-partition activation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommunicationSchedule {
    /// Stages in execution order.
    pub stages: Vec<CommunicationStage>,
    /// Total bytes moved across all transfers.
    pub total_bytes: u64,
}

impl CommunicationSchedule {
    /// Number of stages in the schedule.
    pub fn num_stages(&self) -> usize {
        self.stages.len()
    }

    /// Total number of transfers across all stages.
    pub fn num_transfers(&self) -> usize {
        self.stages.iter().map(|s| s.transfers.len()).sum()
    }
}

/// Build a staged communication schedule for `partitions` given the graph `edges`.
///
/// Each distinct producing-node → consuming-partition activation becomes one
/// [`DataTransfer`] (an output is sent to a partition once, however many nodes
/// there consume it). Transfers are layered by the longest-path level of their
/// source partition in the partition-level DAG, so a partition's outputs are only
/// scheduled after the inputs it depends on. Should the partition graph contain a
/// cycle, the partitions on the cycle are placed in a final catch-all stage.
pub fn build_communication_schedule(
    partitions: &[Partition],
    edges: &[GraphEdge],
) -> CommunicationSchedule {
    // node_id → owning partition_id
    let mut node_to_partition: HashMap<u64, usize> = HashMap::new();
    for p in partitions {
        for &nid in &p.node_ids {
            node_to_partition.insert(nid, p.partition_id);
        }
    }

    // Deduplicate transfers: one activation per (src_part, dst_part, src_node).
    let mut transfer_bytes: HashMap<(usize, usize, u64), u64> = HashMap::new();
    // Partition-level adjacency (deduped) for the dependency DAG.
    let mut part_edges: HashSet<(usize, usize)> = HashSet::new();
    for e in edges {
        let (from_p, to_p) = match (
            node_to_partition.get(&e.from),
            node_to_partition.get(&e.to),
        ) {
            (Some(&f), Some(&t)) if f != t => (f, t),
            _ => continue, // intra-partition or dangling edge: no transfer
        };
        let key = (from_p, to_p, e.from);
        let entry = transfer_bytes.entry(key).or_insert(0);
        *entry = (*entry).max(e.data_bytes);
        part_edges.insert((from_p, to_p));
    }

    // Longest-path layering over the partition DAG (Kahn + relaxation).
    let mut indeg: HashMap<usize, usize> = HashMap::new();
    let mut adj: HashMap<usize, Vec<usize>> = HashMap::new();
    for p in partitions {
        indeg.entry(p.partition_id).or_insert(0);
        adj.entry(p.partition_id).or_default();
    }
    for &(f, t) in &part_edges {
        indeg.entry(t).and_modify(|d| *d += 1).or_insert(1);
        indeg.entry(f).or_insert(0);
        adj.entry(f).or_default().push(t);
        adj.entry(t).or_default();
    }

    let mut level: HashMap<usize, usize> = indeg.keys().map(|&p| (p, 0usize)).collect();
    let mut queue: VecDeque<usize> = indeg
        .iter()
        .filter(|(_, &d)| d == 0)
        .map(|(&p, _)| p)
        .collect();
    let mut working_indeg = indeg.clone();
    let mut processed = 0usize;
    while let Some(u) = queue.pop_front() {
        processed += 1;
        let lu = level[&u];
        if let Some(neighbours) = adj.get(&u) {
            for &v in neighbours {
                let nl = lu + 1;
                let lv = level.entry(v).or_insert(0);
                if nl > *lv {
                    *lv = nl;
                }
                if let Some(d) = working_indeg.get_mut(&v) {
                    *d -= 1;
                    if *d == 0 {
                        queue.push_back(v);
                    }
                }
            }
        }
    }

    // Cycle fallback: any partition not drained by Kahn goes to a final stage.
    if processed < indeg.len() {
        let max_level = level.values().copied().max().unwrap_or(0);
        for (&p, &d) in &working_indeg {
            if d > 0 {
                level.insert(p, max_level + 1);
            }
        }
    }

    // Group transfers into stages by their source partition's level.
    let mut by_stage: HashMap<usize, Vec<DataTransfer>> = HashMap::new();
    let mut total_bytes = 0u64;
    for ((from_p, to_p, node_id), bytes) in transfer_bytes {
        let stage = *level.get(&from_p).unwrap_or(&0);
        total_bytes = total_bytes.saturating_add(bytes);
        by_stage.entry(stage).or_default().push(DataTransfer {
            from_partition: from_p,
            to_partition: to_p,
            node_id,
            data_bytes: bytes,
        });
    }

    let mut stage_ids: Vec<usize> = by_stage.keys().copied().collect();
    stage_ids.sort_unstable();
    let mut stages = Vec::with_capacity(stage_ids.len());
    for (new_id, old_id) in stage_ids.into_iter().enumerate() {
        let mut transfers = by_stage.remove(&old_id).unwrap_or_default();
        // Deterministic order within a stage.
        transfers.sort_unstable_by_key(|t| (t.from_partition, t.to_partition, t.node_id));
        stages.push(CommunicationStage {
            stage_id: new_id,
            transfers,
        });
    }

    CommunicationSchedule {
        stages,
        total_bytes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part(id: usize, nodes: Vec<u64>) -> Partition {
        Partition {
            partition_id: id,
            node_ids: nodes,
            total_compute: 0,
            total_memory: 0,
        }
    }

    fn edge(from: u64, to: u64, bytes: u64) -> GraphEdge {
        GraphEdge {
            from,
            to,
            data_bytes: bytes,
        }
    }

    #[test]
    fn no_cross_edges_yields_empty_schedule() {
        let parts = vec![part(0, vec![1, 2]), part(1, vec![3, 4])];
        let edges = vec![edge(1, 2, 100), edge(3, 4, 200)]; // both intra-partition
        let s = build_communication_schedule(&parts, &edges);
        assert_eq!(s.num_stages(), 0);
        assert_eq!(s.total_bytes, 0);
    }

    #[test]
    fn single_cross_edge_one_stage() {
        let parts = vec![part(0, vec![1]), part(1, vec![2])];
        let edges = vec![edge(1, 2, 512)];
        let s = build_communication_schedule(&parts, &edges);
        assert_eq!(s.num_stages(), 1);
        assert_eq!(s.total_bytes, 512);
        let tr = &s.stages[0].transfers[0];
        assert_eq!(tr.from_partition, 0);
        assert_eq!(tr.to_partition, 1);
        assert_eq!(tr.node_id, 1);
    }

    #[test]
    fn pipeline_three_partitions_three_stages() {
        // 0 → 1 → 2 chain: each cut edge schedules at its source's level.
        let parts = vec![part(0, vec![1]), part(1, vec![2]), part(2, vec![3])];
        let edges = vec![edge(1, 2, 10), edge(2, 3, 20)];
        let s = build_communication_schedule(&parts, &edges);
        assert_eq!(s.num_stages(), 2);
        // stage 0: transfer out of partition 0 (level 0)
        assert_eq!(s.stages[0].transfers[0].from_partition, 0);
        // stage 1: transfer out of partition 1 (level 1)
        assert_eq!(s.stages[1].transfers[0].from_partition, 1);
        assert_eq!(s.total_bytes, 30);
    }

    #[test]
    fn fan_out_same_node_deduped() {
        // node 1 in part 0 feeds nodes 2 and 3, both in part 1 → one transfer.
        let parts = vec![part(0, vec![1]), part(1, vec![2, 3])];
        let edges = vec![edge(1, 2, 64), edge(1, 3, 64)];
        let s = build_communication_schedule(&parts, &edges);
        assert_eq!(s.num_transfers(), 1);
        assert_eq!(s.total_bytes, 64);
    }

    #[test]
    fn parallel_transfers_share_a_stage() {
        // Two independent producers (parts 0 and 1) each feed part 2.
        let parts = vec![part(0, vec![1]), part(1, vec![2]), part(2, vec![3])];
        let edges = vec![edge(1, 3, 100), edge(2, 3, 200)];
        let s = build_communication_schedule(&parts, &edges);
        assert_eq!(s.num_stages(), 1);
        assert_eq!(s.stages[0].transfers.len(), 2);
        assert_eq!(s.total_bytes, 300);
    }

    #[test]
    fn cycle_does_not_panic() {
        // Pathological 0 ↔ 1 cycle: must still produce a finite schedule.
        let parts = vec![part(0, vec![1]), part(1, vec![2])];
        let edges = vec![edge(1, 2, 10), edge(2, 1, 10)];
        let s = build_communication_schedule(&parts, &edges);
        assert_eq!(s.num_transfers(), 2);
        assert_eq!(s.total_bytes, 20);
    }
}
