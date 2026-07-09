//! Deterministic Markdown projection of the graph — the Wiki is a *view* of the
//! DAG, not the source of truth. Same graph → byte-identical files (sorted keys),
//! so the projection is itself content-addressable and diffable.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::error::KResult;
use crate::graph::KnowledgeGraph;
use crate::node::KnowledgeNode;
use crate::store::BlockStore;

/// Kebab-case slug for a name, used as the wiki filename stem and link target.
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in name.trim().chars() {
        if ch.is_alphanumeric() {
            for l in ch.to_lowercase() {
                out.push(l);
            }
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        out.push_str("entity");
    }
    out
}

/// Render every entity to a Markdown page. Keys are `"<slug>.md"`; the map is
/// sorted, so the output is fully deterministic.
pub fn render<S: BlockStore>(kg: &KnowledgeGraph<S>) -> KResult<BTreeMap<String, String>> {
    // Resolve names first so wikilinks can point at slugs deterministically.
    let mut ids: Vec<_> = kg.entity_ids()?;
    ids.sort_by_key(|id| id.to_hex());

    let mut pages = BTreeMap::new();
    for id in &ids {
        let Some(KnowledgeNode::Entity { kind, name, aliases, attrs, .. }) = kg.get_entity(id)?
        else {
            continue;
        };

        let mut md = String::new();
        // YAML frontmatter — machine-readable, mirrors the IPLD node.
        let _ = writeln!(md, "---");
        let _ = writeln!(md, "id: {}", id.to_hex());
        let _ = writeln!(md, "kind: {kind}");
        if !aliases.is_empty() {
            let _ = writeln!(md, "aliases: [{}]", aliases.join(", "));
        }
        let _ = writeln!(md, "---");
        let _ = writeln!(md, "\n# {name}\n");

        if !attrs.is_empty() {
            let _ = writeln!(md, "## Attributes\n");
            for (k, v) in &attrs {
                let _ = writeln!(md, "- **{k}**: {v}");
            }
            let _ = writeln!(md);
        }

        let rels = kg.relations_of(id)?;
        if !rels.is_empty() {
            let _ = writeln!(md, "## Relations\n");
            for (_, rel) in &rels {
                if let KnowledgeNode::Relation { object, predicate, weight, .. } = rel {
                    let target = match kg.get_node_public(object)? {
                        KnowledgeNode::Entity { name, .. } => name,
                        _ => "unknown".to_string(),
                    };
                    let _ = writeln!(md, "- {predicate} → [[{}]] ({target}) `w={weight}`", slug(&target));
                }
            }
            let _ = writeln!(md);
        }

        pages.insert(format!("{}.md", slug(&name)), md);
    }
    Ok(pages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::EntitySpec;
    use crate::store::MemStore;

    fn spec(kind: &str, name: &str) -> EntitySpec {
        EntitySpec { kind: kind.into(), name: name.into(), aliases: vec![], attrs: Default::default() }
    }

    #[test]
    fn projection_is_deterministic_and_has_wikilinks() {
        let build = || {
            let mut kg = KnowledgeGraph::new(MemStore::new()).unwrap();
            let ada = kg.add_entity(spec("person", "Ada Lovelace")).unwrap();
            let engine = kg.add_entity(spec("machine", "Analytical Engine")).unwrap();
            kg.add_relation(ada, "designed", engine, 1.0, vec![]).unwrap();
            render(&kg).unwrap()
        };
        let a = build();
        let b = build();
        assert_eq!(a, b); // byte-for-byte deterministic
        let page = a.get("ada-lovelace.md").expect("ada page exists");
        assert!(page.contains("[[analytical-engine]]"), "wikilink present:\n{page}");
        assert!(page.contains("kind: person"));
    }

    #[test]
    fn slug_is_stable() {
        assert_eq!(slug("Ada Lovelace"), "ada-lovelace");
        assert_eq!(slug("  Foo/Bar  Baz "), "foo-bar-baz");
        assert_eq!(slug("!!!"), "entity");
    }
}
