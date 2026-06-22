//! Distributed graph execution over libp2p (RoadMap Phase 5.1).
//!
//! Node-layer glue that turns the pure, transport-agnostic pipeline in
//! `ipfrs_tensorlogic::distributed` into real cross-peer execution:
//!
//! * **Server side** — [`Node::enable_distributed_execution`] installs an
//!   activation provider so inbound `/ipfrs/activation/1.0.0` requests are computed
//!   on this node's numeric engine via `execute_stage`.
//! * **Client side** — [`Node::run_distributed_pipeline`] drives a multi-stage
//!   pipeline across peers using the `Send + Sync` `ActivationHandle` from
//!   `ipfrs-network` as the `ActivationTransport` implementation.
//!
//! The orchestration itself (stage ordering, activation threading, error handling)
//! lives in `ipfrs_tensorlogic` and is unit-tested there with an in-process
//! transport; the libp2p implementation of the seam lives in `ipfrs-network`. Both
//! keep `ipfrs-tensorlogic` free of any network dependency — the dependency points
//! network → tensorlogic, never back.

use std::collections::HashMap;

use ipfrs_core::Result;
use ipfrs_tensorlogic::distributed::transport::{execute_pipeline, PipelineStage};
use ipfrs_tensorlogic::distributed::wire::{execute_stage, StageRequest, StageResponse, WireTensor};

use super::Node;

impl Node {
    /// Wire inbound activation requests (`/ipfrs/activation/1.0.0`) to the local
    /// numeric engine, so peers can run distributed-inference stages on this node.
    ///
    /// The handler is stateless: each [`StageRequest`] is self-describing (it
    /// carries the subgraph plus its boundary activations), so it is executed
    /// directly with `execute_stage`. Numeric errors are returned to the caller
    /// inside the [`StageResponse`] rather than dropping the connection.
    pub fn enable_distributed_execution(&self) -> Result<()> {
        self.network()?.set_activation_provider(std::sync::Arc::new(
            move |req: StageRequest| {
                Box::pin(async move { execute_stage(&req) })
                    as std::pin::Pin<
                        Box<dyn std::future::Future<Output = StageResponse> + Send>,
                    >
            },
        ));
        Ok(())
    }

    /// Execute a multi-stage pipeline across peers (RoadMap Phase 5.1).
    ///
    /// Each [`PipelineStage`] pins a subgraph to a peer (by peer-id string);
    /// `initial` supplies the graph's external inputs. Stages run in order, with
    /// each stage's outputs threaded into the environment for downstream stages.
    /// Returns the full environment of produced activations — pluck the graph's
    /// final outputs by id.
    ///
    /// For a purely in-process run, use
    /// `ipfrs_tensorlogic::distributed::transport::LocalTransport` directly.
    pub async fn run_distributed_pipeline(
        &self,
        stages: Vec<PipelineStage>,
        initial: HashMap<String, WireTensor>,
    ) -> Result<HashMap<String, WireTensor>> {
        let handle = self.network()?.activation_handle()?;
        execute_pipeline(&stages, &handle, initial)
            .await
            .map_err(|e| ipfrs_core::error::Error::Internal(e.to_string()))
    }
}
