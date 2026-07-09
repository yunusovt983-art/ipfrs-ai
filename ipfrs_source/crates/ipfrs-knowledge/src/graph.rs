//! The knowledge graph: entities + relations over content-addressed HAMTs, with a
//! persisted `KnowledgeRoot` (the mutable "head", chained via `prev`), plus a
//! Petgraph bridge so the logical graph moves in and out without structural loss.

use std::collections::{BTreeMap, HashMap};

use ipfrs_core::{Cid, CidBuilder, Ipld};
use petgraph::graph::DiGraph;

use crate::error::{KError, KResult};
use crate::hamt;
use crate::node::{EntityId, KnowledgeNode};
use crate::store::BlockStore;

/// Input description of an entity for import.
#[derive(Clone, Debug)]
pub struct EntitySpec {
    pub kind: String,
    pub name: String,
    pub aliases: Vec<String>,
    pub attrs: BTreeMap<String, String>,
}

pub struct KnowledgeGraph<S: BlockStore> {
    store: S,
    index: Cid, // HAMT EntityId → Entity node CID
    edges: Cid, // HAMT EntityId(subject) → CID of a list-block of Relation CIDs
    prev: Option<Cid>,
}

impl<S: BlockStore> KnowledgeGraph<S> {
    /// Start a fresh graph on the given store.
    pub fn new(mut store: S) -> KResult<Self> {
        let index = hamt::empty(&mut store)?;
        let edges = hamt::empty(&mut store)?;
        Ok(Self { store, index, edges, prev: None })
    }

    /// Reopen an existing graph from a persisted head CID. The head block (and the
    /// index/edges roots it points at) must already be readable from `store` — for
    /// a [`crate::TieredStore`], call `hydrate(head)` first.
    pub fn open(store: S, head: &Cid) -> KResult<Self> {
        let bytes = store.get(head).ok_or_else(|| KError::NotFound(format!("head {head}")))?;
        let m = match Ipld::from_dag_cbor(bytes).map_err(KError::Core)? {
            Ipld::Map(m) => m,
            _ => return Err(KError::Decode("head not a map".into())),
        };
        let link = |k: &str| {
            m.get(k)
                .and_then(|v| v.as_link().copied())
                .ok_or_else(|| KError::Decode(format!("head missing {k}")))
        };
        let index = link("index")?;
        let edges = link("edges")?;
        Ok(Self { store, index, edges, prev: Some(*head) })
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    /// Mutable access to the backing store — used to drive a tiered store's async
    /// `flush`/`hydrate` between synchronous graph operations.
    pub fn store_mut(&mut self) -> &mut S {
        &mut self.store
    }

    fn put_node(&mut self, node: &KnowledgeNode) -> KResult<Cid> {
        let (cid, bytes) = node.encode()?;
        self.store.put(cid, bytes);
        Ok(cid)
    }

    fn get_node(&self, cid: &Cid) -> KResult<KnowledgeNode> {
        let bytes = self.store.get(cid).ok_or_else(|| KError::NotFound(format!("node {cid}")))?;
        KnowledgeNode::decode(bytes)
    }

    /// Decode any node by CID (entities, relations, evidence, …).
    pub fn get_node_public(&self, cid: &Cid) -> KResult<KnowledgeNode> {
        self.get_node(cid)
    }

    /// Add (or replace) an entity; returns its stable identity.
    pub fn add_entity(&mut self, spec: EntitySpec) -> KResult<EntityId> {
        let id = EntityId::of(&spec.kind, &spec.name);
        let node = KnowledgeNode::Entity {
            id,
            kind: spec.kind,
            name: spec.name,
            aliases: spec.aliases,
            attrs: spec.attrs,
            observations: vec![],
        };
        let cid = self.put_node(&node)?;
        self.index = hamt::insert(&mut self.store, &self.index, id, cid)?;
        Ok(id)
    }

    pub fn entity_cid(&self, id: &EntityId) -> KResult<Option<Cid>> {
        hamt::get(&self.store, &self.index, id)
    }

    pub fn get_entity(&self, id: &EntityId) -> KResult<Option<KnowledgeNode>> {
        match self.entity_cid(id)? {
            Some(cid) => Ok(Some(self.get_node(&cid)?)),
            None => Ok(None),
        }
    }

    /// Add a relation `subject --predicate--> object` (both must exist).
    pub fn add_relation(
        &mut self,
        subject: EntityId,
        predicate: &str,
        object: EntityId,
        weight: f32,
        evidence: Vec<Cid>,
    ) -> KResult<Cid> {
        let subj_cid = self
            .entity_cid(&subject)?
            .ok_or_else(|| KError::Graph("subject entity not found".into()))?;
        let obj_cid = self
            .entity_cid(&object)?
            .ok_or_else(|| KError::Graph("object entity not found".into()))?;
        let rel = KnowledgeNode::Relation {
            subject: subj_cid,
            predicate: predicate.to_string(),
            object: obj_cid,
            evidence,
            weight,
        };
        let rel_cid = self.put_node(&rel)?;

        // Append to the subject's relation list (a list-block, itself addressed).
        let mut list = self.relation_cids(&subject)?;
        list.push(rel_cid);
        let list_ipld = Ipld::List(list.iter().map(|c| Ipld::link(*c)).collect());
        let list_bytes = list_ipld.to_dag_cbor().map_err(KError::Core)?;
        let list_cid = CidBuilder::new().build_dag_cbor(&list_bytes).map_err(KError::Core)?;
        self.store.put(list_cid, list_bytes);
        self.edges = hamt::insert(&mut self.store, &self.edges, subject, list_cid)?;
        Ok(rel_cid)
    }

    /// CIDs of relations whose subject is `id`.
    pub fn relation_cids(&self, id: &EntityId) -> KResult<Vec<Cid>> {
        let Some(list_cid) = hamt::get(&self.store, &self.edges, id)? else {
            return Ok(vec![]);
        };
        let bytes = self.store.get(&list_cid).ok_or_else(|| KError::NotFound("rel list".into()))?;
        match Ipld::from_dag_cbor(bytes).map_err(KError::Core)? {
            Ipld::List(l) => Ok(l.iter().filter_map(|v| v.as_link().copied()).collect()),
            _ => Err(KError::Decode("rel list not a list".into())),
        }
    }

    pub fn relations_of(&self, id: &EntityId) -> KResult<Vec<(Cid, KnowledgeNode)>> {
        self.relation_cids(id)?
            .into_iter()
            .map(|c| Ok((c, self.get_node(&c)?)))
            .collect()
    }

    /// All entity identities in the index.
    pub fn entity_ids(&self) -> KResult<Vec<EntityId>> {
        Ok(hamt::entries(&self.store, &self.index)?.into_iter().map(|(k, _)| k).collect())
    }

    /// Persist the current head as a `KnowledgeRoot` block; returns its CID and
    /// chains `prev` to the previous head.
    pub fn commit(&mut self) -> KResult<Cid> {
        let mut m: BTreeMap<String, Ipld> = BTreeMap::new();
        m.insert("@type".into(), Ipld::String("KnowledgeRoot".into()));
        m.insert("version".into(), Ipld::Integer(1));
        m.insert("index".into(), Ipld::link(self.index));
        m.insert("edges".into(), Ipld::link(self.edges));
        if let Some(p) = self.prev {
            m.insert("prev".into(), Ipld::link(p));
        }
        let bytes = Ipld::Map(m).to_dag_cbor().map_err(KError::Core)?;
        let cid = CidBuilder::new().build_dag_cbor(&bytes).map_err(KError::Core)?;
        self.store.put(cid, bytes);
        self.prev = Some(cid);
        Ok(cid)
    }

    // ---- Petgraph bridge --------------------------------------------------

    /// Export the logical graph: nodes = entities (`EntityId`), edges labelled by
    /// predicate. IPLD DAG → Petgraph, without losing structure.
    pub fn export_petgraph(&self) -> KResult<DiGraph<EntityId, String>> {
        let ids = self.entity_ids()?;
        let mut g = DiGraph::<EntityId, String>::new();
        let mut idx = HashMap::new();
        for id in &ids {
            idx.insert(*id, g.add_node(*id));
        }
        for id in &ids {
            for (_, rel) in self.relations_of(id)? {
                if let KnowledgeNode::Relation { object, predicate, .. } = rel {
                    // object CID → its EntityId (Entity carries its id)
                    if let KnowledgeNode::Entity { id: oid, .. } = self.get_node(&object)? {
                        if let (Some(&a), Some(&b)) = (idx.get(id), idx.get(&oid)) {
                            g.add_edge(a, b, predicate);
                        }
                    }
                }
            }
        }
        Ok(g)
    }

    /// Import a Petgraph into a fresh knowledge graph. Petgraph → IPLD DAG.
    pub fn import_petgraph(store: S, g: &DiGraph<EntitySpec, String>) -> KResult<Self> {
        let mut kg = KnowledgeGraph::new(store)?;
        let mut ids = HashMap::new();
        for ni in g.node_indices() {
            let id = kg.add_entity(g[ni].clone())?;
            ids.insert(ni, id);
        }
        for e in g.edge_indices() {
            let (a, b) = g.edge_endpoints(e).unwrap();
            kg.add_relation(ids[&a], &g[e], ids[&b], 1.0, vec![])?;
        }
        Ok(kg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::MemStore;

    fn spec(kind: &str, name: &str) -> EntitySpec {
        EntitySpec { kind: kind.into(), name: name.into(), aliases: vec![], attrs: Default::default() }
    }

    #[test]
    fn entities_relations_and_commit() {
        let mut kg = KnowledgeGraph::new(MemStore::new()).unwrap();
        let ada = kg.add_entity(spec("person", "Ada")).unwrap();
        let engine = kg.add_entity(spec("machine", "Analytical Engine")).unwrap();
        kg.add_relation(ada, "designed", engine, 0.95, vec![]).unwrap();

        assert!(kg.get_entity(&ada).unwrap().is_some());
        let rels = kg.relations_of(&ada).unwrap();
        assert_eq!(rels.len(), 1);
        // commit produces a head; a second commit chains prev → different CID
        let h1 = kg.commit().unwrap();
        let h2 = kg.commit().unwrap();
        assert_ne!(h1, h2);
    }

    #[test]
    fn petgraph_round_trip_preserves_edges() {
        let mut g = DiGraph::<EntitySpec, String>::new();
        let a = g.add_node(spec("person", "Ada"));
        let b = g.add_node(spec("machine", "Engine"));
        let c = g.add_node(spec("concept", "Algorithm"));
        g.add_edge(a, b, "designed".into());
        g.add_edge(a, c, "invented".into());

        let kg = KnowledgeGraph::import_petgraph(MemStore::new(), &g).unwrap();
        let out = kg.export_petgraph().unwrap();
        assert_eq!(out.node_count(), 3);
        assert_eq!(out.edge_count(), 2);
        // Ada has two outgoing edges in both graphs.
        let ada = EntityId::of("person", "Ada");
        assert_eq!(kg.relations_of(&ada).unwrap().len(), 2);
    }
}
