//! Knowledge-graph gateway handlers.
//!
//! Exposes the `ipfrs-knowledge` crate over `/api/v0/knowledge/*`, backed by the
//! gateway's sled block store through a `TieredStore` (hot MemStore + cold sled).
//! Sync graph mutations run under a tokio mutex; `commit` flushes hot → cold.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::{
    body::{Body, Bytes},
    extract::State,
    http::header,
    response::{IntoResponse, Response},
    Json,
};
use ipfrs_core::{Block, Cid};
use ipfrs_knowledge::{gc, project, EntityId, EntitySpec, KnowledgeGraph, KnowledgeNode, TieredStore};
use ipfrs_storage::{BlockStoreTrait, CarHeader};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

// ---- minimal CARv1 framing (unsigned LEB128 varint) ----------------------

fn encode_varint(mut v: u64) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let mut b = (v & 0x7f) as u8;
        v >>= 7;
        if v != 0 {
            b |= 0x80;
        }
        out.push(b);
        if v == 0 {
            break;
        }
    }
    out
}

fn decode_varint(data: &[u8]) -> Result<(u64, usize), AppError> {
    let mut result = 0u64;
    let mut shift = 0u32;
    for (i, &b) in data.iter().take(10).enumerate() {
        result |= ((b & 0x7f) as u64) << shift;
        if b & 0x80 == 0 {
            return Ok((result, i + 1));
        }
        shift += 7;
    }
    Err(AppError::Knowledge("truncated CAR varint".to_string()))
}

use super::{AppError, GatewayState};

type Graph = Arc<Mutex<KnowledgeGraph<TieredStore>>>;

/// How many most-recent commit heads are auto-pinned (retained through GC).
pub(crate) const RETAIN_HEADS: usize = 10;

/// The gateway's knowledge feature: the graph, a durable head pointer, a durable
/// manual pin set, and a ring of the most-recent commit heads (auto-pinned).
#[derive(Clone)]
pub(crate) struct KnowledgeState {
    pub(crate) graph: Graph,
    pub(crate) head_path: PathBuf,
    pub(crate) pins: Arc<Mutex<HashSet<Cid>>>,
    pub(crate) pins_path: PathBuf,
    /// Recent commit heads, oldest first, capped at [`RETAIN_HEADS`].
    pub(crate) recent: Arc<Mutex<Vec<Cid>>>,
    pub(crate) recent_path: PathBuf,
}

/// Read a persisted head CID, if one was written by a previous run.
pub(crate) fn read_head(path: &Path) -> Option<Cid> {
    let s = std::fs::read_to_string(path).ok()?;
    s.trim().parse::<Cid>().ok()
}

/// Durably record the current head CID (write-then-rename to avoid torn writes).
fn write_head(path: &Path, head: &Cid) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, head.to_string())?;
    std::fs::rename(&tmp, path)
}

/// Read the persisted pin set (one CID per line); empty if absent.
pub(crate) fn read_pins(path: &Path) -> HashSet<Cid> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.lines().filter_map(|l| l.trim().parse::<Cid>().ok()).collect())
        .unwrap_or_default()
}

fn write_pins(path: &Path, pins: &HashSet<Cid>) -> std::io::Result<()> {
    let body = pins.iter().map(|c| c.to_string()).collect::<Vec<_>>().join("\n");
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, body)?;
    std::fs::rename(&tmp, path)
}

/// Read the recent-heads ring (oldest first); empty if absent.
pub(crate) fn read_recent(path: &Path) -> Vec<Cid> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.lines().filter_map(|l| l.trim().parse::<Cid>().ok()).collect())
        .unwrap_or_default()
}

fn write_recent(path: &Path, recent: &[Cid]) -> std::io::Result<()> {
    let body = recent.iter().map(|c| c.to_string()).collect::<Vec<_>>().join("\n");
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, body)?;
    std::fs::rename(&tmp, path)
}

fn kstate(state: &GatewayState) -> Result<&KnowledgeState, AppError> {
    state
        .knowledge
        .as_ref()
        .ok_or_else(|| AppError::FeatureDisabled("Knowledge graph not enabled".to_string()))
}

fn graph(state: &GatewayState) -> Result<&Graph, AppError> {
    Ok(&kstate(state)?.graph)
}

// ---- request / response shapes ------------------------------------------

#[derive(Deserialize)]
pub(super) struct EntityReq {
    kind: String,
    name: String,
    #[serde(default)]
    aliases: Vec<String>,
    #[serde(default)]
    attrs: BTreeMap<String, String>,
}

#[derive(Serialize)]
pub(super) struct EntityResp {
    id: String,
}

#[derive(Deserialize)]
pub(super) struct RelationReq {
    subject_kind: String,
    subject_name: String,
    predicate: String,
    object_kind: String,
    object_name: String,
    #[serde(default = "default_weight")]
    weight: f32,
}

fn default_weight() -> f32 {
    1.0
}

#[derive(Serialize)]
pub(super) struct CidResp {
    cid: String,
}

#[derive(Serialize)]
pub(super) struct CommitResp {
    head: String,
}

#[derive(Deserialize)]
pub(super) struct SearchReq {
    query: String,
    #[serde(default)]
    k: Option<usize>,
}

#[derive(Serialize)]
pub(super) struct HitJson {
    cid: String,
    score: f32,
    kind: String,
    title: String,
}

#[derive(Serialize)]
pub(super) struct SearchResp {
    results: Vec<HitJson>,
}

#[derive(Serialize)]
pub(super) struct StatsResp {
    entities: usize,
    index: usize,
}

#[derive(Serialize)]
pub(super) struct ProjectionResp {
    pages: BTreeMap<String, String>,
}

#[derive(Deserialize)]
pub(super) struct CidReq {
    cid: String,
}

#[derive(Serialize)]
pub(super) struct PinsResp {
    pins: Vec<String>,
}

#[derive(Serialize)]
pub(super) struct HeadsResp {
    live: Option<String>,
    recent: Vec<String>,
    retain: usize,
}

#[derive(Deserialize)]
pub(super) struct GcReq {
    #[serde(default)]
    keep_history: Option<bool>,
}

#[derive(Serialize)]
pub(super) struct GcResp {
    kept: usize,
    deleted: usize,
    roots: usize,
}

// ---- handlers ------------------------------------------------------------

/// POST /api/v0/knowledge/entity — add or replace an entity.
pub(super) async fn api_knowledge_add_entity(
    State(state): State<GatewayState>,
    Json(req): Json<EntityReq>,
) -> Result<Json<EntityResp>, AppError> {
    let mut kg = graph(&state)?.lock().await;
    let id = kg
        .add_entity(EntitySpec { kind: req.kind, name: req.name, aliases: req.aliases, attrs: req.attrs })
        .map_err(|e| AppError::Knowledge(e.to_string()))?;
    Ok(Json(EntityResp { id: id.to_hex() }))
}

/// POST /api/v0/knowledge/relation — add a relation between two existing entities.
pub(super) async fn api_knowledge_add_relation(
    State(state): State<GatewayState>,
    Json(req): Json<RelationReq>,
) -> Result<Json<CidResp>, AppError> {
    let subject = EntityId::of(&req.subject_kind, &req.subject_name);
    let object = EntityId::of(&req.object_kind, &req.object_name);
    let mut kg = graph(&state)?.lock().await;
    let cid = kg
        .add_relation(subject, &req.predicate, object, req.weight, vec![])
        .map_err(|e| AppError::Knowledge(e.to_string()))?;
    Ok(Json(CidResp { cid: cid.to_string() }))
}

/// POST /api/v0/knowledge/commit — persist a new head (blocks flushed to sled).
pub(super) async fn api_knowledge_commit(
    State(state): State<GatewayState>,
) -> Result<Json<CommitResp>, AppError> {
    let ks = kstate(&state)?;
    let mut kg = ks.graph.lock().await;
    let head = kg.commit().map_err(|e| AppError::Knowledge(e.to_string()))?;
    kg.store_mut().flush().await.map_err(|e| AppError::Knowledge(e.to_string()))?;
    // Durably record the head so a restart reopens this exact graph.
    write_head(&ks.head_path, &head).map_err(|e| AppError::Knowledge(format!("head write: {e}")))?;

    // Auto-pin: keep the last RETAIN_HEADS commit heads (bounded ring, deduped).
    {
        let mut recent = ks.recent.lock().await;
        recent.retain(|c| *c != head);
        recent.push(head);
        let overflow = recent.len().saturating_sub(RETAIN_HEADS);
        if overflow > 0 {
            recent.drain(0..overflow);
        }
        write_recent(&ks.recent_path, &recent)
            .map_err(|e| AppError::Knowledge(format!("recent write: {e}")))?;
    }
    Ok(Json(CommitResp { head: head.to_string() }))
}

/// POST /api/v0/knowledge/search — cosine top-k over the maintained vector index.
pub(super) async fn api_knowledge_search(
    State(state): State<GatewayState>,
    Json(req): Json<SearchReq>,
) -> Result<Json<SearchResp>, AppError> {
    let kg = graph(&state)?.lock().await;
    let k = req.k.unwrap_or(8);
    let results = kg
        .search(&req.query, k)
        .into_iter()
        .map(|(cid, score)| {
            let (kind, title) = match kg.get_node_public(&cid) {
                Ok(KnowledgeNode::Entity { name, .. }) => ("entity".to_string(), name),
                Ok(KnowledgeNode::Evidence { .. }) => ("evidence".to_string(), "Evidence".to_string()),
                Ok(node) => (node.type_name().to_string(), String::new()),
                Err(_) => ("unknown".to_string(), String::new()),
            };
            HitJson { cid: cid.to_string(), score, kind, title }
        })
        .collect();
    Ok(Json(SearchResp { results }))
}

/// GET /api/v0/knowledge/stats — entity count and index size.
pub(super) async fn api_knowledge_stats(
    State(state): State<GatewayState>,
) -> Result<Json<StatsResp>, AppError> {
    let kg = graph(&state)?.lock().await;
    let entities = kg.entity_ids().map_err(|e| AppError::Knowledge(e.to_string()))?.len();
    Ok(Json(StatsResp { entities, index: kg.vindex().len() }))
}

/// GET /api/v0/knowledge/projection — deterministic Markdown pages for all entities.
pub(super) async fn api_knowledge_projection(
    State(state): State<GatewayState>,
) -> Result<Json<ProjectionResp>, AppError> {
    let kg = graph(&state)?.lock().await;
    let pages = project::render(&kg).map_err(|e| AppError::Knowledge(e.to_string()))?;
    Ok(Json(ProjectionResp { pages }))
}

// ---- pins & garbage collection ------------------------------------------

fn parse_cid(s: &str) -> Result<Cid, AppError> {
    s.trim().parse::<Cid>().map_err(|_| AppError::InvalidCid(s.to_string()))
}

/// POST /api/v0/knowledge/pin — retain a head CID through GC.
pub(super) async fn api_knowledge_pin(
    State(state): State<GatewayState>,
    Json(req): Json<CidReq>,
) -> Result<Json<PinsResp>, AppError> {
    let ks = kstate(&state)?;
    let cid = parse_cid(&req.cid)?;
    let mut pins = ks.pins.lock().await;
    pins.insert(cid);
    write_pins(&ks.pins_path, &pins).map_err(|e| AppError::Knowledge(format!("pins write: {e}")))?;
    Ok(Json(PinsResp { pins: pins.iter().map(|c| c.to_string()).collect() }))
}

/// POST /api/v0/knowledge/unpin — release a pinned head CID.
pub(super) async fn api_knowledge_unpin(
    State(state): State<GatewayState>,
    Json(req): Json<CidReq>,
) -> Result<Json<PinsResp>, AppError> {
    let ks = kstate(&state)?;
    let cid = parse_cid(&req.cid)?;
    let mut pins = ks.pins.lock().await;
    pins.remove(&cid);
    write_pins(&ks.pins_path, &pins).map_err(|e| AppError::Knowledge(format!("pins write: {e}")))?;
    Ok(Json(PinsResp { pins: pins.iter().map(|c| c.to_string()).collect() }))
}

/// GET /api/v0/knowledge/pins — the current pin set.
pub(super) async fn api_knowledge_pins(
    State(state): State<GatewayState>,
) -> Result<Json<PinsResp>, AppError> {
    let ks = kstate(&state)?;
    let pins = ks.pins.lock().await;
    Ok(Json(PinsResp { pins: pins.iter().map(|c| c.to_string()).collect() }))
}

/// GET /api/v0/knowledge/heads — the live head and the recent auto-pinned ring
/// (newest first).
pub(super) async fn api_knowledge_heads(
    State(state): State<GatewayState>,
) -> Result<Json<HeadsResp>, AppError> {
    let ks = kstate(&state)?;
    let live = read_head(&ks.head_path).map(|c| c.to_string());
    let mut recent: Vec<String> =
        ks.recent.lock().await.iter().rev().map(|c| c.to_string()).collect();
    recent.dedup();
    Ok(Json(HeadsResp { live, recent, retain: RETAIN_HEADS }))
}

/// POST /api/v0/knowledge/gc — mark-and-sweep the cold tier, retaining the live
/// head plus every pinned head. Holds the graph lock so no commit races the sweep.
pub(super) async fn api_knowledge_gc(
    State(state): State<GatewayState>,
    Json(req): Json<GcReq>,
) -> Result<Json<GcResp>, AppError> {
    let ks = kstate(&state)?;
    let _guard = ks.graph.lock().await; // serialize GC with mutations

    // Roots = manual pins ∪ recent auto-pinned heads ∪ the live head.
    let mut set: HashSet<Cid> = ks.pins.lock().await.iter().copied().collect();
    set.extend(ks.recent.lock().await.iter().copied());
    if let Some(head) = read_head(&ks.head_path) {
        set.insert(head);
    }
    let roots: Vec<Cid> = set.into_iter().collect();
    let cold: Arc<dyn BlockStoreTrait> = state.store.clone();
    let report = gc::collect(&cold, &roots, req.keep_history.unwrap_or(true))
        .await
        .map_err(|e| AppError::Knowledge(e.to_string()))?;
    Ok(Json(GcResp { kept: report.kept, deleted: report.deleted.len(), roots: roots.len() }))
}

// ---- CAR export / import -------------------------------------------------

/// GET /api/v0/knowledge/export — the whole graph (everything reachable from the
/// head, history included) as one CARv1, root = head. Streamed block-by-block
/// straight from the cold tier — no temp file, constant memory per block.
pub(super) async fn api_knowledge_export(
    State(state): State<GatewayState>,
) -> Result<Response, AppError> {
    let ks = kstate(&state)?;
    let head = read_head(&ks.head_path)
        .ok_or_else(|| AppError::NotFound("no committed knowledge head to export".to_string()))?;
    let cold: Arc<dyn BlockStoreTrait> = state.store.clone();

    let live = gc::reachable(&cold, &[head], true).await.map_err(|e| AppError::Knowledge(e.to_string()))?;
    let header_cbor = CarHeader::new(vec![head])
        .to_cbor()
        .map_err(|e| AppError::Knowledge(format!("car header: {e}")))?;

    let stream = async_stream::stream! {
        // Header frame: varint(len) | header CBOR
        let mut h = encode_varint(header_cbor.len() as u64);
        h.extend_from_slice(&header_cbor);
        yield Ok::<Bytes, std::io::Error>(Bytes::from(h));

        // One frame per block: varint(cid_len + data_len) | cid | data
        for cid in live {
            if let Ok(Some(block)) = cold.get(&cid).await {
                let cid_bytes = cid.to_bytes();
                let data = block.data();
                let mut frame = encode_varint((cid_bytes.len() + data.len()) as u64);
                frame.extend_from_slice(&cid_bytes);
                frame.extend_from_slice(data);
                yield Ok(Bytes::from(frame));
            }
        }
    };

    Ok((
        [
            (header::CONTENT_TYPE, "application/vnd.ipld.car"),
            (header::CONTENT_DISPOSITION, "attachment; filename=\"knowledge.car\""),
        ],
        Body::from_stream(stream),
    )
        .into_response())
}

/// POST /api/v0/knowledge/import (body = raw CARv1 bytes) — parse in memory, load
/// every block into the cold tier, and adopt the CAR's first root as the new head.
pub(super) async fn api_knowledge_import(
    State(state): State<GatewayState>,
    body: Bytes,
) -> Result<Json<CommitResp>, AppError> {
    let ks = kstate(&state)?;
    let mut guard = ks.graph.lock().await; // serialize with mutations
    let cold: Arc<dyn BlockStoreTrait> = state.store.clone();

    // Header: varint(len) | header CBOR
    let mut pos = 0usize;
    let (hlen, n) = decode_varint(&body[pos..])?;
    pos += n;
    let hend = pos + hlen as usize;
    let car_header = CarHeader::from_cbor(body.get(pos..hend).ok_or_else(|| AppError::Knowledge("truncated CAR header".to_string()))?)
        .map_err(|e| AppError::Knowledge(format!("car header: {e}")))?;
    pos = hend;
    let head = *car_header
        .roots
        .first()
        .ok_or_else(|| AppError::Knowledge("CAR has no root".to_string()))?;

    // Blocks: varint(cid_len + data_len) | cid | data
    while pos < body.len() {
        let (blen, n) = decode_varint(&body[pos..])?;
        pos += n;
        let end = pos + blen as usize;
        let frame = body.get(pos..end).ok_or_else(|| AppError::Knowledge("truncated CAR block".to_string()))?;
        pos = end;
        let cid = Cid::try_from(frame.to_vec()).map_err(|e| AppError::Knowledge(format!("bad CID in CAR: {e}")))?;
        let cid_len = cid.to_bytes().len();
        let data = Bytes::copy_from_slice(&frame[cid_len..]);
        cold.put(&Block::from_parts(cid, data)).await.map_err(AppError::Storage)?;
    }

    // Adopt the imported head: hydrate + reopen the running graph, persist the head.
    let mut ts = TieredStore::new(cold);
    ts.hydrate(&head).await.map_err(|e| AppError::Knowledge(e.to_string()))?;
    *guard = KnowledgeGraph::open(ts, &head).map_err(|e| AppError::Knowledge(e.to_string()))?;
    write_head(&ks.head_path, &head).map_err(|e| AppError::Knowledge(format!("head write: {e}")))?;

    Ok(Json(CommitResp { head: head.to_string() }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ipfrs_storage::BlockStoreConfig;

    async fn state_at(path: std::path::PathBuf) -> GatewayState {
        GatewayState::new(BlockStoreConfig::testing().with_path(path))
            .unwrap()
            .with_knowledge()
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn add_search_commit_flow() {
        let dir = tempfile::tempdir().unwrap();
        let st = state_at(dir.path().join("db")).await;

        for (kind, name) in [("person", "Ada Lovelace"), ("machine", "Analytical Engine")] {
            let _ = api_knowledge_add_entity(
                State(st.clone()),
                Json(EntityReq { kind: kind.into(), name: name.into(), aliases: vec![], attrs: Default::default() }),
            )
            .await
            .unwrap();
        }
        let _ = api_knowledge_add_relation(
            State(st.clone()),
            Json(RelationReq {
                subject_kind: "person".into(),
                subject_name: "Ada Lovelace".into(),
                predicate: "wrote-notes-on".into(),
                object_kind: "machine".into(),
                object_name: "Analytical Engine".into(),
                weight: 0.95,
            }),
        )
        .await
        .unwrap();

        // search finds Ada for "lovelace"
        let resp = api_knowledge_search(
            State(st.clone()),
            Json(SearchReq { query: "lovelace".into(), k: Some(3) }),
        )
        .await
        .unwrap();
        assert_eq!(resp.0.results[0].title, "Ada Lovelace");
        assert_eq!(resp.0.results[0].kind, "entity");

        // commit yields a head and stats reflect the graph
        let head = api_knowledge_commit(State(st.clone())).await.unwrap();
        assert!(!head.0.head.is_empty());
        let stats = api_knowledge_stats(State(st.clone())).await.unwrap();
        assert_eq!(stats.0.entities, 2);

        // projection contains the wikilink
        let proj = api_knowledge_projection(State(st.clone())).await.unwrap();
        assert!(proj.0.pages.get("ada-lovelace.md").unwrap().contains("[[analytical-engine]]"));
    }

    /// The head pointer + sled blocks let a fresh gateway state reopen the exact
    /// graph after a "restart" (drop + reconstruct over the same path).
    #[tokio::test]
    async fn head_survives_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db");

        // session 1: seed + commit (persists head file + flushes blocks), then close
        {
            let st = state_at(path.clone()).await;
            for (kind, name) in [("person", "Ada Lovelace"), ("machine", "Analytical Engine")] {
                let _ = api_knowledge_add_entity(
                    State(st.clone()),
                    Json(EntityReq { kind: kind.into(), name: name.into(), aliases: vec![], attrs: Default::default() }),
                )
                .await
                .unwrap();
            }
            let head = api_knowledge_commit(State(st.clone())).await.unwrap();
            assert!(!head.0.head.is_empty());
        } // st dropped → sled DB closed

        // session 2: fresh gateway state over the same path → graph reopened
        let st2 = state_at(path).await;
        let stats = api_knowledge_stats(State(st2.clone())).await.unwrap();
        assert_eq!(stats.0.entities, 2, "graph reopened from persisted head");
        let hits = api_knowledge_search(
            State(st2),
            Json(SearchReq { query: "lovelace".into(), k: Some(1) }),
        )
        .await
        .unwrap();
        assert_eq!(hits.0.results[0].title, "Ada Lovelace", "index survived restart");
    }

    /// A pinned head survives GC even when history is dropped; the live head is
    /// always retained; the graph stays queryable afterwards.
    #[tokio::test]
    async fn gc_respects_pins_and_live_head() {
        use ipfrs_storage::BlockStoreTrait;

        let dir = tempfile::tempdir().unwrap();
        let st = state_at(dir.path().join("db")).await;

        let _ = api_knowledge_add_entity(
            State(st.clone()),
            Json(EntityReq { kind: "person".into(), name: "Ada".into(), aliases: vec![], attrs: Default::default() }),
        )
        .await
        .unwrap();
        let head1 = api_knowledge_commit(State(st.clone())).await.unwrap().0.head;

        // pin the v1 head, then supersede it with v2
        let pins = api_knowledge_pin(State(st.clone()), Json(CidReq { cid: head1.clone() })).await.unwrap();
        assert!(pins.0.pins.contains(&head1));
        let _ = api_knowledge_add_entity(
            State(st.clone()),
            Json(EntityReq { kind: "person".into(), name: "Grace".into(), aliases: vec![], attrs: Default::default() }),
        )
        .await
        .unwrap();
        let _head2 = api_knowledge_commit(State(st.clone())).await.unwrap();

        // GC dropping history: pinned head1 + live head2 are both retained.
        let report = api_knowledge_gc(State(st.clone()), Json(GcReq { keep_history: Some(false) }))
            .await
            .unwrap();
        assert!(report.0.roots >= 2, "pinned + live heads are roots");

        // head1's block is still on the cold tier because it was pinned.
        let h1: Cid = head1.parse().unwrap();
        assert!(st.store.has(&h1).await.unwrap(), "pinned head survived GC");

        // graph still fully queryable
        let stats = api_knowledge_stats(State(st.clone())).await.unwrap();
        assert_eq!(stats.0.entities, 2);
        let hits = api_knowledge_search(State(st), Json(SearchReq { query: "grace".into(), k: Some(1) })).await.unwrap();
        assert_eq!(hits.0.results[0].title, "Grace");
    }

    /// The whole graph survives export → import into a fresh, independent gateway.
    #[tokio::test]
    async fn car_export_import_roundtrip() {
        let dir1 = tempfile::tempdir().unwrap();
        let src = state_at(dir1.path().join("db")).await;
        for (kind, name) in [("person", "Ada Lovelace"), ("machine", "Analytical Engine")] {
            let _ = api_knowledge_add_entity(
                State(src.clone()),
                Json(EntityReq { kind: kind.into(), name: name.into(), aliases: vec![], attrs: Default::default() }),
            )
            .await
            .unwrap();
        }
        let _ = api_knowledge_add_relation(
            State(src.clone()),
            Json(RelationReq {
                subject_kind: "person".into(),
                subject_name: "Ada Lovelace".into(),
                predicate: "wrote-notes-on".into(),
                object_kind: "machine".into(),
                object_name: "Analytical Engine".into(),
                weight: 0.95,
            }),
        )
        .await
        .unwrap();
        let _ = api_knowledge_commit(State(src.clone())).await.unwrap();

        // export → CAR bytes
        let resp = api_knowledge_export(State(src)).await.unwrap();
        let car = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        assert!(car.len() > 32, "non-trivial CAR");

        // import into a brand-new gateway over a different store
        let dir2 = tempfile::tempdir().unwrap();
        let dst = state_at(dir2.path().join("db")).await;
        assert_eq!(api_knowledge_stats(State(dst.clone())).await.unwrap().0.entities, 0);
        let head = api_knowledge_import(State(dst.clone()), car).await.unwrap();
        assert!(!head.0.head.is_empty());

        let stats = api_knowledge_stats(State(dst.clone())).await.unwrap();
        assert_eq!(stats.0.entities, 2, "graph rebuilt from CAR");
        let hits = api_knowledge_search(State(dst), Json(SearchReq { query: "lovelace".into(), k: Some(1) }))
            .await
            .unwrap();
        assert_eq!(hits.0.results[0].title, "Ada Lovelace", "index rebuilt from CAR");
    }

    /// Each commit auto-pins into a bounded ring; GC retains the last RETAIN_HEADS
    /// commits while an older, evicted head becomes collectable.
    #[tokio::test]
    async fn auto_pin_retains_recent_heads() {
        use ipfrs_storage::BlockStoreTrait;

        let dir = tempfile::tempdir().unwrap();
        let st = state_at(dir.path().join("db")).await;

        let mut heads = Vec::new();
        for i in 0..(RETAIN_HEADS + 2) {
            let _ = api_knowledge_add_entity(
                State(st.clone()),
                Json(EntityReq { kind: "n".into(), name: format!("e{i}"), aliases: vec![], attrs: Default::default() }),
            )
            .await
            .unwrap();
            heads.push(api_knowledge_commit(State(st.clone())).await.unwrap().0.head);
        }

        // ring holds exactly the last RETAIN_HEADS, newest first
        let hs = api_knowledge_heads(State(st.clone())).await.unwrap().0;
        assert_eq!(hs.recent.len(), RETAIN_HEADS);
        assert_eq!(hs.recent[0], *heads.last().unwrap());
        assert!(!hs.recent.contains(&heads[0]), "oldest head evicted from ring");

        // GC dropping history keeps the ring; the evicted oldest head is collected.
        let _ = api_knowledge_gc(State(st.clone()), Json(GcReq { keep_history: Some(false) }))
            .await
            .unwrap();
        let oldest: Cid = heads[0].parse().unwrap();
        let newest: Cid = heads.last().unwrap().parse().unwrap();
        assert!(!st.store.has(&oldest).await.unwrap(), "evicted head collected");
        assert!(st.store.has(&newest).await.unwrap(), "recent head retained");

        // graph still fully intact
        assert_eq!(api_knowledge_stats(State(st)).await.unwrap().0.entities, RETAIN_HEADS + 2);
    }

    #[tokio::test]
    async fn disabled_without_feature() {
        let dir = tempfile::tempdir().unwrap();
        let st = GatewayState::new(BlockStoreConfig::testing().with_path(dir.path().join("db"))).unwrap();
        let r = api_knowledge_stats(State(st)).await;
        assert!(r.is_err());
    }
}
