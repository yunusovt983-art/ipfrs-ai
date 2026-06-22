//! Distributed graph-execution integration test (RoadMap Phase 5.1).
//!
//! Proves the Spike 2b Definition of Done: a multi-stage computation graph
//! executes across two live IPFRS nodes connected over libp2p, with activations
//! streamed between them via `/ipfrs/activation/1.0.0`.
//!
//! Topology: node A is the orchestrator and owns stage 2 (run in-process); node B
//! is a remote executor that runs stage 1 over the wire. The intermediate
//! activation `h` produced by B is threaded into A's stage 2 — so the test
//! exercises the real request/response path *and* distributes work across both
//! nodes.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use ipfrs::{Node, NodeConfig};
use ipfrs_tensorlogic::computation_graph::{ComputationGraph, GraphNode, TensorOp};
use ipfrs_tensorlogic::distributed::transport::PipelineStage;
use ipfrs_tensorlogic::distributed::wire::WireTensor;
use tokio::time::sleep;

fn unique_tmp_dir(label: &str) -> PathBuf {
    let pid = std::process::id();
    let nonce = {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos()
    };
    std::env::temp_dir().join(format!("ipfrs_distexec_{}_{}_{}", label, pid, nonce))
}

fn test_port(offset: u16) -> u16 {
    let pid = (std::process::id() % 4000) as u16;
    43000u16.saturating_add(pid).saturating_add(offset)
}

fn net_config(dir: PathBuf, tcp_port: u16) -> NodeConfig {
    let mut config = NodeConfig::default();
    config.storage.path = dir.join("blocks");
    config.network.data_dir = dir.join("network");
    config.enable_tensorlogic = false;
    config.enable_semantic = false;
    config.network.enable_mdns = false;
    config.network.enable_nat_traversal = false;
    config.network.listen_addrs = vec![format!("/ip4/127.0.0.1/tcp/{}", tcp_port)];
    config
}

fn input(id: &str) -> GraphNode {
    GraphNode::new(id.to_string(), TensorOp::Input { name: id.to_string() })
}

/// Stage 1 (runs on the remote peer): h = relu(x + b1).
fn stage1_graph() -> ComputationGraph {
    let mut g = ComputationGraph::new();
    g.add_node(input("x")).unwrap();
    g.add_node(input("b1")).unwrap();
    g.mark_input("x".to_string());
    g.mark_input("b1".to_string());
    g.add_node(
        GraphNode::new("s1".to_string(), TensorOp::Add)
            .add_input("x".to_string())
            .add_input("b1".to_string()),
    )
    .unwrap();
    g.add_node(GraphNode::new("h".to_string(), TensorOp::ReLU).add_input("s1".to_string()))
        .unwrap();
    g.mark_output("h".to_string());
    g
}

/// Stage 2 (runs on the orchestrator): y = h * b2.
fn stage2_graph() -> ComputationGraph {
    let mut g = ComputationGraph::new();
    g.add_node(input("h")).unwrap();
    g.add_node(input("b2")).unwrap();
    g.mark_input("h".to_string());
    g.mark_input("b2".to_string());
    g.add_node(
        GraphNode::new("y".to_string(), TensorOp::Mul)
            .add_input("h".to_string())
            .add_input("b2".to_string()),
    )
    .unwrap();
    g.mark_output("y".to_string());
    g
}

/// Wait until `node` reports `peer` among its connected peers, or time out.
async fn await_connected(node: &Node, peer: &str, timeout: Duration) -> bool {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if let Ok(peers) = node.peers().await {
            if peers.iter().any(|p| p == peer) {
                return true;
            }
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_stage_graph_executes_across_two_nodes() {
    let port_a = test_port(0);
    let port_b = test_port(1);
    let dir_a = unique_tmp_dir("a");
    let dir_b = unique_tmp_dir("b");

    // ── Node B: remote executor ───────────────────────────────────────────
    let mut node_b = Node::new(net_config(dir_b.clone(), port_b)).expect("create node B");
    node_b.start().await.expect("start node B");
    node_b
        .enable_distributed_execution()
        .expect("enable activation provider on B");
    let peer_b = node_b.peer_id().expect("peer id B");

    // ── Node A: orchestrator (also executes stage 2 locally) ──────────────
    let mut node_a = Node::new(net_config(dir_a.clone(), port_a)).expect("create node A");
    node_a.start().await.expect("start node A");
    node_a
        .enable_distributed_execution()
        .expect("enable activation provider on A");
    let peer_a = node_a.peer_id().expect("peer id A");

    // Dial B and wait for the connection to come up.
    let addr_b = format!("/ip4/127.0.0.1/tcp/{}/p2p/{}", port_b, peer_b);
    node_a.connect(&addr_b).await.expect("A dials B");
    assert!(
        await_connected(&node_a, &peer_b, Duration::from_secs(15)).await,
        "node A never connected to node B"
    );

    // ── Pipeline: stage 1 on B (remote), stage 2 on A (local) ─────────────
    let stages = vec![
        PipelineStage::new(
            peer_b.clone(),
            stage1_graph(),
            vec!["x".to_string(), "b1".to_string()],
            vec!["h".to_string()],
        ),
        PipelineStage::new(
            peer_a.clone(),
            stage2_graph(),
            vec!["h".to_string(), "b2".to_string()],
            vec!["y".to_string()],
        ),
    ];

    let mut initial = HashMap::new();
    initial.insert("x".to_string(), WireTensor { data: vec![1.0, -3.0], shape: vec![1, 2] });
    initial.insert("b1".to_string(), WireTensor { data: vec![2.0, 1.0], shape: vec![1, 2] });
    initial.insert("b2".to_string(), WireTensor { data: vec![10.0, 10.0], shape: vec![1, 2] });

    let env = tokio::time::timeout(
        Duration::from_secs(30),
        node_a.run_distributed_pipeline(stages, initial),
    )
    .await
    .expect("distributed pipeline timed out")
    .expect("distributed pipeline failed");

    // relu([1,-3] + [2,1]) = relu([3,-2]) = [3,0]; * [10,10] = [30,0]
    let y = env.get("y").expect("pipeline produced output y");
    assert_eq!(y.shape, vec![1, 2]);
    assert_eq!(y.data, vec![30.0, 0.0]);

    node_a.stop().await.expect("stop node A");
    node_b.stop().await.expect("stop node B");

    let _ = std::fs::remove_dir_all(&dir_a);
    let _ = std::fs::remove_dir_all(&dir_b);
}
