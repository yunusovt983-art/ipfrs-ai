//! Typed knowledge nodes, encoded as authentic DAG-CBOR IPLD blocks.
//!
//! Every node is a content-addressed [`ipfrs_core::Ipld`] map with a `@type` tag;
//! inter-node references are real IPLD links (CBOR tag 42), so a generic IPLD tool
//! traverses the graph. `EntityId` hashes only the *invariant identity* (kind +
//! canonical name), not the whole node — so an entity can gain attributes (new CID)
//! while its identity, and therefore the edges pointing at it, stay stable.

use std::collections::BTreeMap;

use ipfrs_core::{Cid, CidBuilder, Ipld};
use sha2::{Digest, Sha256};

use crate::error::KError;

/// Stable identity of an entity: `sha2-256(kind \0 canonical_name)`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntityId(pub [u8; 32]);

impl EntityId {
    /// Derive the identity from an entity's kind and canonical name.
    pub fn of(kind: &str, name: &str) -> Self {
        let mut h = Sha256::new();
        h.update(kind.as_bytes());
        h.update([0u8]);
        h.update(name.trim().to_lowercase().as_bytes());
        EntityId(h.finalize().into())
    }
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }
    pub fn from_hex(s: &str) -> Result<Self, KError> {
        let v = hex::decode(s).map_err(|e| KError::Decode(format!("entity id hex: {e}")))?;
        let arr: [u8; 32] = v
            .try_into()
            .map_err(|_| KError::Decode("entity id must be 32 bytes".into()))?;
        Ok(EntityId(arr))
    }
}

impl std::fmt::Debug for EntityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EntityId({}…)", &self.to_hex()[..12])
    }
}

/// Epistemic status of a hypothesis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HypothesisStatus {
    Open,
    Supported,
    Refuted,
}
impl HypothesisStatus {
    fn as_str(self) -> &'static str {
        match self {
            HypothesisStatus::Open => "open",
            HypothesisStatus::Supported => "supported",
            HypothesisStatus::Refuted => "refuted",
        }
    }
    fn parse(s: &str) -> Self {
        match s {
            "supported" => HypothesisStatus::Supported,
            "refuted" => HypothesisStatus::Refuted,
            _ => HypothesisStatus::Open,
        }
    }
}

/// A typed node in the knowledge graph.
#[derive(Clone, Debug, PartialEq)]
pub enum KnowledgeNode {
    Entity {
        id: EntityId,
        kind: String,
        name: String,
        aliases: Vec<String>,
        attrs: BTreeMap<String, String>,
        observations: Vec<Cid>,
    },
    Concept {
        name: String,
        definition: String,
        broader: Vec<Cid>,
        related: Vec<Cid>,
    },
    Relation {
        subject: Cid,
        predicate: String,
        object: Cid,
        evidence: Vec<Cid>,
        weight: f32,
    },
    Evidence {
        source: Cid,
        span: Option<(u64, u64)>,
        extracted_by: String,
        /// CID of a proof-carrying block (e.g. `ipfrs-tensorlogic` Proof).
        proof: Option<Cid>,
    },
    Observation {
        about: Cid,
        statement: String,
        ts: u64,
        agent: String,
    },
    Hypothesis {
        claim: String,
        supports: Vec<Cid>,
        refutes: Vec<Cid>,
        status: HypothesisStatus,
        proof: Option<Cid>,
    },
}

impl KnowledgeNode {
    /// The `@type` discriminator used in the encoded map.
    pub fn type_name(&self) -> &'static str {
        match self {
            KnowledgeNode::Entity { .. } => "Entity",
            KnowledgeNode::Concept { .. } => "Concept",
            KnowledgeNode::Relation { .. } => "Relation",
            KnowledgeNode::Evidence { .. } => "Evidence",
            KnowledgeNode::Observation { .. } => "Observation",
            KnowledgeNode::Hypothesis { .. } => "Hypothesis",
        }
    }

    /// Encode to a DAG-CBOR block: returns its CID and bytes.
    pub fn encode(&self) -> Result<(Cid, Vec<u8>), KError> {
        let bytes = self.to_ipld().to_dag_cbor().map_err(KError::Core)?;
        let cid = CidBuilder::new().build_dag_cbor(&bytes).map_err(KError::Core)?;
        Ok((cid, bytes))
    }

    /// Decode a node from a DAG-CBOR block.
    pub fn decode(bytes: &[u8]) -> Result<Self, KError> {
        let ipld = Ipld::from_dag_cbor(bytes).map_err(KError::Core)?;
        Self::from_ipld(&ipld)
    }

    fn links(cids: &[Cid]) -> Ipld {
        Ipld::List(cids.iter().map(|c| Ipld::link(*c)).collect())
    }

    pub fn to_ipld(&self) -> Ipld {
        let mut m: BTreeMap<String, Ipld> = BTreeMap::new();
        m.insert("@type".into(), Ipld::String(self.type_name().into()));
        match self {
            KnowledgeNode::Entity { id, kind, name, aliases, attrs, observations } => {
                m.insert("id".into(), Ipld::Bytes(id.0.to_vec()));
                m.insert("kind".into(), Ipld::String(kind.clone()));
                m.insert("name".into(), Ipld::String(name.clone()));
                m.insert("aliases".into(), Ipld::List(aliases.iter().cloned().map(Ipld::String).collect()));
                m.insert(
                    "attrs".into(),
                    Ipld::Map(attrs.iter().map(|(k, v)| (k.clone(), Ipld::String(v.clone()))).collect()),
                );
                m.insert("observations".into(), Self::links(observations));
            }
            KnowledgeNode::Concept { name, definition, broader, related } => {
                m.insert("name".into(), Ipld::String(name.clone()));
                m.insert("definition".into(), Ipld::String(definition.clone()));
                m.insert("broader".into(), Self::links(broader));
                m.insert("related".into(), Self::links(related));
            }
            KnowledgeNode::Relation { subject, predicate, object, evidence, weight } => {
                m.insert("subject".into(), Ipld::link(*subject));
                m.insert("predicate".into(), Ipld::String(predicate.clone()));
                m.insert("object".into(), Ipld::link(*object));
                m.insert("evidence".into(), Self::links(evidence));
                m.insert("weight".into(), Ipld::Float(*weight as f64));
            }
            KnowledgeNode::Evidence { source, span, extracted_by, proof } => {
                m.insert("source".into(), Ipld::link(*source));
                if let Some((a, b)) = span {
                    m.insert("span".into(), Ipld::List(vec![Ipld::Integer(*a as i128), Ipld::Integer(*b as i128)]));
                }
                m.insert("extracted_by".into(), Ipld::String(extracted_by.clone()));
                if let Some(p) = proof {
                    m.insert("proof".into(), Ipld::link(*p));
                }
            }
            KnowledgeNode::Observation { about, statement, ts, agent } => {
                m.insert("about".into(), Ipld::link(*about));
                m.insert("statement".into(), Ipld::String(statement.clone()));
                m.insert("ts".into(), Ipld::Integer(*ts as i128));
                m.insert("agent".into(), Ipld::String(agent.clone()));
            }
            KnowledgeNode::Hypothesis { claim, supports, refutes, status, proof } => {
                m.insert("claim".into(), Ipld::String(claim.clone()));
                m.insert("supports".into(), Self::links(supports));
                m.insert("refutes".into(), Self::links(refutes));
                m.insert("status".into(), Ipld::String(status.as_str().into()));
                if let Some(p) = proof {
                    m.insert("proof".into(), Ipld::link(*p));
                }
            }
        }
        Ipld::Map(m)
    }

    pub fn from_ipld(ipld: &Ipld) -> Result<Self, KError> {
        let m = as_map(ipld)?;
        let ty = gstr(m, "@type")?;
        Ok(match ty.as_str() {
            "Entity" => {
                let id_bytes = gbytes(m, "id")?;
                let id: [u8; 32] = id_bytes
                    .try_into()
                    .map_err(|_| KError::Decode("Entity.id must be 32 bytes".into()))?;
                KnowledgeNode::Entity {
                    id: EntityId(id),
                    kind: gstr(m, "kind")?,
                    name: gstr(m, "name")?,
                    aliases: gstrlist(m, "aliases")?,
                    attrs: gstrmap(m, "attrs")?,
                    observations: glinks(m, "observations")?,
                }
            }
            "Concept" => KnowledgeNode::Concept {
                name: gstr(m, "name")?,
                definition: gstr(m, "definition")?,
                broader: glinks(m, "broader")?,
                related: glinks(m, "related")?,
            },
            "Relation" => KnowledgeNode::Relation {
                subject: glink(m, "subject")?,
                predicate: gstr(m, "predicate")?,
                object: glink(m, "object")?,
                evidence: glinks(m, "evidence")?,
                weight: gfloat(m, "weight")? as f32,
            },
            "Evidence" => KnowledgeNode::Evidence {
                source: glink(m, "source")?,
                span: gspan(m)?,
                extracted_by: gstr(m, "extracted_by")?,
                proof: glink_opt(m, "proof"),
            },
            "Observation" => KnowledgeNode::Observation {
                about: glink(m, "about")?,
                statement: gstr(m, "statement")?,
                ts: gint(m, "ts")? as u64,
                agent: gstr(m, "agent")?,
            },
            "Hypothesis" => KnowledgeNode::Hypothesis {
                claim: gstr(m, "claim")?,
                supports: glinks(m, "supports")?,
                refutes: glinks(m, "refutes")?,
                status: HypothesisStatus::parse(&gstr(m, "status")?),
                proof: glink_opt(m, "proof"),
            },
            other => return Err(KError::Decode(format!("unknown node @type '{other}'"))),
        })
    }
}

// ---- Ipld field accessors -------------------------------------------------

fn as_map(i: &Ipld) -> Result<&BTreeMap<String, Ipld>, KError> {
    match i {
        Ipld::Map(m) => Ok(m),
        _ => Err(KError::Decode("expected IPLD map".into())),
    }
}
fn field<'a>(m: &'a BTreeMap<String, Ipld>, k: &str) -> Result<&'a Ipld, KError> {
    m.get(k).ok_or_else(|| KError::Decode(format!("missing field '{k}'")))
}
fn gstr(m: &BTreeMap<String, Ipld>, k: &str) -> Result<String, KError> {
    match field(m, k)? {
        Ipld::String(s) => Ok(s.clone()),
        _ => Err(KError::Decode(format!("field '{k}' not a string"))),
    }
}
fn gint(m: &BTreeMap<String, Ipld>, k: &str) -> Result<i128, KError> {
    match field(m, k)? {
        Ipld::Integer(n) => Ok(*n),
        _ => Err(KError::Decode(format!("field '{k}' not an integer"))),
    }
}
fn gfloat(m: &BTreeMap<String, Ipld>, k: &str) -> Result<f64, KError> {
    match field(m, k)? {
        Ipld::Float(x) => Ok(*x),
        Ipld::Integer(n) => Ok(*n as f64),
        _ => Err(KError::Decode(format!("field '{k}' not a float"))),
    }
}
fn gbytes(m: &BTreeMap<String, Ipld>, k: &str) -> Result<Vec<u8>, KError> {
    match field(m, k)? {
        Ipld::Bytes(b) => Ok(b.clone()),
        _ => Err(KError::Decode(format!("field '{k}' not bytes"))),
    }
}
fn glink(m: &BTreeMap<String, Ipld>, k: &str) -> Result<Cid, KError> {
    field(m, k)?
        .as_link()
        .copied()
        .ok_or_else(|| KError::Decode(format!("field '{k}' not a link")))
}
fn glink_opt(m: &BTreeMap<String, Ipld>, k: &str) -> Option<Cid> {
    m.get(k).and_then(|v| v.as_link().copied())
}
fn glinks(m: &BTreeMap<String, Ipld>, k: &str) -> Result<Vec<Cid>, KError> {
    match field(m, k)? {
        Ipld::List(l) => l
            .iter()
            .map(|v| v.as_link().copied().ok_or_else(|| KError::Decode(format!("'{k}' has non-link"))))
            .collect(),
        _ => Err(KError::Decode(format!("field '{k}' not a list"))),
    }
}
fn gstrlist(m: &BTreeMap<String, Ipld>, k: &str) -> Result<Vec<String>, KError> {
    match field(m, k)? {
        Ipld::List(l) => l
            .iter()
            .map(|v| match v {
                Ipld::String(s) => Ok(s.clone()),
                _ => Err(KError::Decode(format!("'{k}' has non-string"))),
            })
            .collect(),
        _ => Err(KError::Decode(format!("field '{k}' not a list"))),
    }
}
fn gstrmap(m: &BTreeMap<String, Ipld>, k: &str) -> Result<BTreeMap<String, String>, KError> {
    match field(m, k)? {
        Ipld::Map(mm) => mm
            .iter()
            .map(|(kk, v)| match v {
                Ipld::String(s) => Ok((kk.clone(), s.clone())),
                _ => Err(KError::Decode(format!("attr '{kk}' not a string"))),
            })
            .collect(),
        _ => Err(KError::Decode(format!("field '{k}' not a map"))),
    }
}
fn gspan(m: &BTreeMap<String, Ipld>) -> Result<Option<(u64, u64)>, KError> {
    match m.get("span") {
        None => Ok(None),
        Some(Ipld::List(l)) if l.len() == 2 => {
            let a = match &l[0] { Ipld::Integer(n) => *n as u64, _ => return Err(KError::Decode("span[0]".into())) };
            let b = match &l[1] { Ipld::Integer(n) => *n as u64, _ => return Err(KError::Decode("span[1]".into())) };
            Ok(Some((a, b)))
        }
        _ => Err(KError::Decode("span must be a 2-element list".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_id_is_identity_stable() {
        let a = EntityId::of("person", "Ada Lovelace");
        let b = EntityId::of("person", "  ada lovelace ");
        assert_eq!(a, b, "identity ignores case/whitespace");
        assert_ne!(a, EntityId::of("org", "Ada Lovelace"));
    }

    #[test]
    fn entity_round_trips_through_dag_cbor() {
        let n = KnowledgeNode::Entity {
            id: EntityId::of("person", "Ada"),
            kind: "person".into(),
            name: "Ada".into(),
            aliases: vec!["Countess Lovelace".into()],
            attrs: [("born".to_string(), "1815".to_string())].into_iter().collect(),
            observations: vec![],
        };
        let (cid, bytes) = n.encode().unwrap();
        let back = KnowledgeNode::decode(&bytes).unwrap();
        assert_eq!(n, back);
        // Deterministic: same node → same CID.
        assert_eq!(cid, n.encode().unwrap().0);
    }

    #[test]
    fn relation_and_evidence_links_round_trip() {
        let (subj, _) = KnowledgeNode::Entity {
            id: EntityId::of("person", "Ada"), kind: "person".into(), name: "Ada".into(),
            aliases: vec![], attrs: Default::default(), observations: vec![],
        }.encode().unwrap();
        let (obj, _) = KnowledgeNode::Concept {
            name: "Analytical Engine".into(), definition: "…".into(), broader: vec![], related: vec![],
        }.encode().unwrap();
        let (proof, _) = KnowledgeNode::Observation {
            about: subj, statement: "wrote notes".into(), ts: 1, agent: "recognizer".into(),
        }.encode().unwrap();
        let ev = KnowledgeNode::Evidence { source: obj, span: Some((10, 42)), extracted_by: "ner".into(), proof: Some(proof) };
        let (evcid, evbytes) = ev.encode().unwrap();
        assert_eq!(KnowledgeNode::decode(&evbytes).unwrap(), ev);

        let rel = KnowledgeNode::Relation {
            subject: subj, predicate: "designed".into(), object: obj, evidence: vec![evcid], weight: 0.9,
        };
        let (_, rbytes) = rel.encode().unwrap();
        assert_eq!(KnowledgeNode::decode(&rbytes).unwrap(), rel);
    }
}
