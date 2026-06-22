//! Distributed-execution primitives for the numeric engine (RoadMap Phase 5).
//!
//! These are the missing vertebrae between the existing greedy
//! [`graph_partitioner`](crate::graph_partitioner) ("which node runs where") and a
//! future wire-level distributed executor ("stream activations between peers"):
//!
//! * [`sharding`] — split / reassemble a [`NumTensor`](crate::numeric_exec::NumTensor)
//!   across peers (`ShardSpec` / `ShardStrategy`).
//! * [`collectives`] — real, pure-`f32` reduction semantics over tensors
//!   (`ReduceOp`, `all_reduce`, `all_gather`, `reduce_scatter`).
//! * [`schedule`] — turn a set of [`Partition`](crate::graph_partitioner::Partition)s
//!   into a staged communication plan that respects cross-partition dependencies.
//! * [`wire`] — serde request/response for executing one partition's subgraph on a
//!   peer (`StageRequest`/`StageResponse`) plus the local `execute_stage` it runs.
//! * [`transport`] — the dependency-inversion seam: an `ActivationTransport` trait,
//!   a pipeline orchestrator (`execute_pipeline`) and an in-process `LocalTransport`.
//!   The real libp2p transport lives in the node layer, which *depends on* this crate
//!   — so the `network ↔ tensorlogic` cycle stays structurally impossible.
//!
//! ## Provenance (DDD: ACL port, not a dependency)
//!
//! The *vocabulary* (shard strategies, `ReduceOp`, communication schedule / stage /
//! transfer) is an Anti-Corruption-Layer port of `torsh`'s `torsh-distributed` and
//! `torsh-fx` modules, translated into our own Ubiquitous Language and types. The
//! *algorithms here are our own and actually compute* — `torsh`'s collective bodies
//! are mock stubs ("skip the averaging to avoid type issues"), so only the type
//! shapes were worth borrowing. Everything is local, pure and synchronous; no
//! `ipfrs-network` dependency, so there is no `network ↔ tensorlogic` cycle risk.
//! The wire protocol (`/ipfrs/activation/*`) is a deliberate follow-up that will
//! call these tested primitives.

pub mod collectives;
pub mod schedule;
pub mod sharding;
pub mod transport;
pub mod wire;

pub use collectives::{all_gather, all_reduce, reduce_scatter, ReduceOp};
pub use schedule::{build_communication_schedule, CommunicationSchedule, CommunicationStage, DataTransfer};
pub use sharding::{gather_shards, plan_shards, shard_tensor, ShardSpec, ShardStrategy};
pub use transport::{execute_pipeline, ActivationTransport, LocalTransport, PipelineStage};
pub use wire::{execute_stage, StageRequest, StageResponse, WireTensor};
