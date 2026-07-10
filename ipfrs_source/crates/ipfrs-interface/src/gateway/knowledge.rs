//! Knowledge-graph gateway handlers.
//!
//! Exposes the `ipfrs-knowledge` crate over `/api/v0/knowledge/*`, backed by the
//! gateway's sled block store through a `TieredStore` (hot MemStore + cold sled).
//! Sync graph mutations run under a tokio mutex; `commit` flushes hot → cold.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use axum::{
    body::Bytes,
    extract::State,
    http::header,
    response::{IntoResponse, Response},
    Json,
};
use ipfrs_core::Cid;
use ipfrs_knowledge::{gc, project, EntityId, EntitySpec, KnowledgeGraph, KnowledgeNode, TieredStore};
use ipfrs_storage::{BlockStoreTrait, CarReader, CarWriter};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

/// Unique temp-file sequence (avoids collisions between concurrent CAR requests).
static TMP_SEQ: AtomicU64 = AtomicU64::new(0);

fn temp_car_path(tag: &str) -> PathBuf {
    let n = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("ipfrs-knowledge-{tag}-{}-{n}.car", std::process::id()))
}

use super::{AppError, GatewayState};

type Graph = Arc<Mutex<KnowledgeGraph<TieredStore>>>;

/// The gateway's knowledge feature: the graph, a durable head pointer, and a
/// durable pin set of extra heads to retain during GC.
#[derive(Clone)]
pub(crate) struct KnowledgeState {
    pub(crate) graph: Graph,
    pub(crate) head_path: PathBuf,
    pub(crate) pins: Arc<Mutex<HashSet<Cid>>>,
    pub(crate) pins_path: PathBuf,
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

/// POST /api/v0/knowledge/gc — mark-and-sweep the cold tier, retaining the live
/// head plus every pinned head. Holds the graph lock so no commit races the sweep.
pub(super) async fn api_knowledge_gc(
    State(state): State<GatewayState>,
    Json(req): Json<GcReq>,
) -> Result<Json<GcResp>, AppError> {
    let ks = kstate(&state)?;
    let _guard = ks.graph.lock().await; // serialize GC with mutations

    // Roots = pinned heads ∪ the live head (never collect the current graph).
    let mut roots: Vec<Cid> = ks.pins.lock().await.iter().copied().collect();
    if let Some(head) = read_head(&ks.head_path) {
        if !roots.contains(&head) {
            roots.push(head);
        }
    }
    let cold: Arc<dyn BlockStoreTrait> = state.store.clone();
    let report = gc::collect(&cold, &roots, req.keep_history.unwrap_or(true))
        .await
        .map_err(|e| AppError::Knowledge(e.to_string()))?;
    Ok(Json(GcResp { kept: report.kept, deleted: report.deleted.len(), roots: roots.len() }))
}

// ---- CAR export / import -------------------------------------------------

/// GET /api/v0/knowledge/export.car — the whole graph (everything reachable from
/// the head, history included) as one CAR file, root = head.
pub(super) async fn api_knowledge_export(
    State(state): State<GatewayState>,
) -> Result<Response, AppError> {
    let ks = kstate(&state)?;
    let head = read_head(&ks.head_path)
        .ok_or_else(|| AppError::NotFound("no committed knowledge head to export".to_string()))?;
    let cold: Arc<dyn BlockStoreTrait> = state.store.clone();

    let live = gc::reachable(&cold, &[head], true).await.map_err(|e| AppError::Knowledge(e.to_string()))?;
    let path = temp_car_path("export");
    let mut writer = CarWriter::create(&path, vec![head])
        .await
        .map_err(|e| AppError::Knowledge(format!("car create: {e}")))?;
    for cid in &live {
        if let Some(block) = cold.get(cid).await.map_err(AppError::Storage)? {
            writer.write_block(&block).await.map_err(|e| AppError::Knowledge(format!("car write: {e}")))?;
        }
    }
    writer.finish().await.map_err(|e| AppError::Knowledge(format!("car finish: {e}")))?;
    let bytes = tokio::fs::read(&path).await.map_err(|e| AppError::Knowledge(format!("car read: {e}")))?;
    let _ = tokio::fs::remove_file(&path).await;

    Ok((
        [
            (header::CONTENT_TYPE, "application/vnd.ipld.car"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"knowledge.car\"",
            ),
        ],
        bytes,
    )
        .into_response())
}

/// POST /api/v0/knowledge/import (body = raw CAR bytes) — load every block into the
/// cold tier and adopt the CAR's first root as the new head.
pub(super) async fn api_knowledge_import(
    State(state): State<GatewayState>,
    body: Bytes,
) -> Result<Json<CommitResp>, AppError> {
    let ks = kstate(&state)?;
    let mut guard = ks.graph.lock().await; // serialize with mutations

    let path = temp_car_path("import");
    tokio::fs::write(&path, &body).await.map_err(|e| AppError::Knowledge(format!("car spill: {e}")))?;
    let mut reader = CarReader::open(&path)
        .await
        .map_err(|e| AppError::Knowledge(format!("car open: {e}")))?;
    let head = reader
        .roots()
        .first()
        .copied()
        .ok_or_else(|| AppError::Knowledge("CAR has no root".to_string()))?;

    let cold: Arc<dyn BlockStoreTrait> = state.store.clone();
    let mut imported = 0usize;
    while let Some(block) = reader.read_block().await.map_err(|e| AppError::Knowledge(format!("car block: {e}")))? {
        cold.put(&block).await.map_err(AppError::Storage)?;
        imported += 1;
    }
    let _ = tokio::fs::remove_file(&path).await;

    // Adopt the imported head: hydrate + reopen the running graph, persist the head.
    let mut ts = TieredStore::new(cold);
    ts.hydrate(&head).await.map_err(|e| AppError::Knowledge(e.to_string()))?;
    *guard = KnowledgeGraph::open(ts, &head).map_err(|e| AppError::Knowledge(e.to_string()))?;
    write_head(&ks.head_path, &head).map_err(|e| AppError::Knowledge(format!("head write: {e}")))?;

    let _ = imported;
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

    #[tokio::test]
    async fn disabled_without_feature() {
        let dir = tempfile::tempdir().unwrap();
        let st = GatewayState::new(BlockStoreConfig::testing().with_path(dir.path().join("db"))).unwrap();
        let r = api_knowledge_stats(State(st)).await;
        assert!(r.is_err());
    }
}
