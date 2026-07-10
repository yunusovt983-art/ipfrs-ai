//! Knowledge-graph gateway handlers.
//!
//! Exposes the `ipfrs-knowledge` crate over `/api/v0/knowledge/*`, backed by the
//! gateway's sled block store through a `TieredStore` (hot MemStore + cold sled).
//! Sync graph mutations run under a tokio mutex; `commit` flushes hot → cold.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::{extract::State, Json};
use ipfrs_knowledge::{project, EntityId, EntitySpec, KnowledgeGraph, KnowledgeNode, TieredStore};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use super::{AppError, GatewayState};

type Graph = Arc<Mutex<KnowledgeGraph<TieredStore>>>;

fn graph(state: &GatewayState) -> Result<&Graph, AppError> {
    state
        .knowledge
        .as_ref()
        .ok_or_else(|| AppError::FeatureDisabled("Knowledge graph not enabled".to_string()))
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
    let mut kg = graph(&state)?.lock().await;
    let head = kg.commit().map_err(|e| AppError::Knowledge(e.to_string()))?;
    kg.store_mut().flush().await.map_err(|e| AppError::Knowledge(e.to_string()))?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use ipfrs_storage::BlockStoreConfig;

    fn state_in(dir: &tempfile::TempDir) -> GatewayState {
        GatewayState::new(BlockStoreConfig::testing().with_path(dir.path().join("db")))
            .unwrap()
            .with_knowledge()
            .unwrap()
    }

    #[tokio::test]
    async fn add_search_commit_flow() {
        let dir = tempfile::tempdir().unwrap();
        let st = state_in(&dir);

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

    #[tokio::test]
    async fn disabled_without_feature() {
        let dir = tempfile::tempdir().unwrap();
        let st = GatewayState::new(BlockStoreConfig::testing().with_path(dir.path().join("db"))).unwrap();
        let r = api_knowledge_stats(State(st)).await;
        assert!(r.is_err());
    }
}
