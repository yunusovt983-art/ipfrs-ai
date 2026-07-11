//! A block-backed Hash-Array-Mapped-Trie mapping `EntityId → Cid`.
//!
//! Each trie node is itself a content-addressed IPLD block (16-way, keyed by the
//! nibbles of the 32-byte `EntityId` — which is already a hash, so no extra
//! hashing). Insert path-copies only the nodes on the affected path, so mutating
//! one entry in a 100M-entry index touches O(log n) blocks instead of re-hashing
//! the whole DAG — the structural-sharing property the flat-file Wiki lacks.

use ipfrs_core::{Cid, CidBuilder, Ipld};

use crate::error::{KError, KResult};
use crate::node::EntityId;
use crate::store::BlockStore;

const WIDTH: usize = 16;

#[derive(Clone, Copy)]
enum Slot {
    Empty,
    Leaf(EntityId, Cid),
    Branch(Cid),
}

/// A 32-byte key has exactly 64 nibbles; no legitimate trie descends beyond that.
const MAX_DEPTH: usize = 64;

fn nibble(key: &EntityId, depth: usize) -> usize {
    let byte = key.0[depth / 2];
    (if depth.is_multiple_of(2) { byte >> 4 } else { byte & 0x0f }) as usize
}

fn slot_to_ipld(s: &Slot) -> Ipld {
    match s {
        Slot::Empty => Ipld::Null,
        Slot::Leaf(k, v) => Ipld::Map(
            [("k".to_string(), Ipld::Bytes(k.0.to_vec())), ("v".to_string(), Ipld::link(*v))]
                .into_iter()
                .collect(),
        ),
        Slot::Branch(c) => Ipld::link(*c),
    }
}

fn slot_from_ipld(i: &Ipld) -> KResult<Slot> {
    match i {
        Ipld::Null => Ok(Slot::Empty),
        Ipld::Link(c) => Ok(Slot::Branch(c.0)),
        Ipld::Map(m) => {
            let k = match m.get("k") {
                Some(Ipld::Bytes(b)) => {
                    let arr: [u8; 32] = b.clone().try_into().map_err(|_| KError::Decode("hamt leaf key size".into()))?;
                    EntityId(arr)
                }
                _ => return Err(KError::Decode("hamt leaf missing key".into())),
            };
            let v = m.get("v").and_then(|x| x.as_link().copied())
                .ok_or_else(|| KError::Decode("hamt leaf missing value".into()))?;
            Ok(Slot::Leaf(k, v))
        }
        _ => Err(KError::Decode("bad hamt slot".into())),
    }
}

fn node_to_cid<S: BlockStore>(store: &mut S, slots: &[Slot; WIDTH]) -> KResult<Cid> {
    let ipld = Ipld::Map(
        [
            ("@type".to_string(), Ipld::String("hamt".into())),
            ("slots".to_string(), Ipld::List(slots.iter().map(slot_to_ipld).collect())),
        ]
        .into_iter()
        .collect(),
    );
    let bytes = ipld.to_dag_cbor().map_err(KError::Core)?;
    let cid = CidBuilder::new().build_dag_cbor(&bytes).map_err(KError::Core)?;
    store.put(cid, bytes);
    Ok(cid)
}

fn load<S: BlockStore>(store: &S, cid: &Cid) -> KResult<[Slot; WIDTH]> {
    let bytes = store.get(cid).ok_or_else(|| KError::NotFound(format!("hamt node {cid}")))?;
    let ipld = Ipld::from_dag_cbor(bytes).map_err(KError::Core)?;
    let slots_ipld = match &ipld {
        Ipld::Map(m) => match m.get("slots") {
            Some(Ipld::List(l)) if l.len() == WIDTH => l,
            _ => return Err(KError::Decode("hamt node bad slots".into())),
        },
        _ => return Err(KError::Decode("hamt node not a map".into())),
    };
    let mut slots = [Slot::Empty; WIDTH];
    for (i, s) in slots_ipld.iter().enumerate() {
        slots[i] = slot_from_ipld(s)?;
    }
    Ok(slots)
}

/// Create an empty HAMT and return its root CID.
pub fn empty<S: BlockStore>(store: &mut S) -> KResult<Cid> {
    node_to_cid(store, &[Slot::Empty; WIDTH])
}

/// Insert (or overwrite) `key → val`, returning the new root CID (path-copied).
pub fn insert<S: BlockStore>(store: &mut S, root: &Cid, key: EntityId, val: Cid) -> KResult<Cid> {
    let slots = load(store, root)?;
    insert_slots(store, slots, key, val, 0)
}

fn insert_slots<S: BlockStore>(
    store: &mut S,
    mut slots: [Slot; WIDTH],
    key: EntityId,
    val: Cid,
    depth: usize,
) -> KResult<Cid> {
    if depth >= MAX_DEPTH {
        return Err(KError::Graph("hamt depth exceeded".into()));
    }
    let nib = nibble(&key, depth);
    let new = match slots[nib] {
        Slot::Empty => Slot::Leaf(key, val),
        Slot::Leaf(k, _) if k == key => Slot::Leaf(key, val),
        Slot::Leaf(k, v) => {
            // Collision: push both the existing leaf and the new one one level down.
            let c1 = insert_slots(store, [Slot::Empty; WIDTH], k, v, depth + 1)?;
            let child = load(store, &c1)?;
            let c2 = insert_slots(store, child, key, val, depth + 1)?;
            Slot::Branch(c2)
        }
        Slot::Branch(c) => {
            let child = load(store, &c)?;
            Slot::Branch(insert_slots(store, child, key, val, depth + 1)?)
        }
    };
    slots[nib] = new;
    node_to_cid(store, &slots)
}

/// Look up a key.
pub fn get<S: BlockStore>(store: &S, root: &Cid, key: &EntityId) -> KResult<Option<Cid>> {
    let mut cur = *root;
    let mut depth = 0;
    loop {
        // A crafted (imported) trie could nest Branches past the 64-nibble key; stop
        // rather than index out of bounds in `nibble`.
        if depth >= MAX_DEPTH {
            return Ok(None);
        }
        let slots = load(store, &cur)?;
        match slots[nibble(key, depth)] {
            Slot::Empty => return Ok(None),
            Slot::Leaf(k, v) => return Ok(if &k == key { Some(v) } else { None }),
            Slot::Branch(c) => {
                cur = c;
                depth += 1;
            }
        }
    }
}

/// Collect all `(EntityId, Cid)` leaves (order unspecified).
pub fn entries<S: BlockStore>(store: &S, root: &Cid) -> KResult<Vec<(EntityId, Cid)>> {
    let mut out = Vec::new();
    let mut stack = vec![*root];
    while let Some(cid) = stack.pop() {
        for s in load(store, &cid)? {
            match s {
                Slot::Empty => {}
                Slot::Leaf(k, v) => out.push((k, v)),
                Slot::Branch(c) => stack.push(c),
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::MemStore;

    fn dummy_cid(store: &mut MemStore, tag: u8) -> Cid {
        // any content-addressed block
        let bytes = ipfrs_core::Ipld::Bytes(vec![tag; 4]).to_dag_cbor().unwrap();
        let cid = CidBuilder::new().build_dag_cbor(&bytes).unwrap();
        store.put(cid, bytes);
        cid
    }

    #[test]
    fn insert_get_roundtrip_and_overwrite() {
        let mut s = MemStore::new();
        let mut root = empty(&mut s).unwrap();
        let k1 = EntityId::of("person", "Ada");
        let k2 = EntityId::of("person", "Grace");
        let v1 = dummy_cid(&mut s, 1);
        let v2 = dummy_cid(&mut s, 2);
        root = insert(&mut s, &root, k1, v1).unwrap();
        root = insert(&mut s, &root, k2, v2).unwrap();
        assert_eq!(get(&s, &root, &k1).unwrap(), Some(v1));
        assert_eq!(get(&s, &root, &k2).unwrap(), Some(v2));
        assert_eq!(get(&s, &root, &EntityId::of("person", "Nobody")).unwrap(), None);
        // overwrite
        let v3 = dummy_cid(&mut s, 3);
        root = insert(&mut s, &root, k1, v3).unwrap();
        assert_eq!(get(&s, &root, &k1).unwrap(), Some(v3));
    }

    #[test]
    fn many_keys_all_present() {
        let mut s = MemStore::new();
        let mut root = empty(&mut s).unwrap();
        let mut expect = Vec::new();
        for i in 0..500u32 {
            let k = EntityId::of("n", &format!("e{i}"));
            let v = dummy_cid(&mut s, (i % 251) as u8);
            root = insert(&mut s, &root, k, v).unwrap();
            expect.push((k, v));
        }
        for (k, v) in &expect {
            assert_eq!(get(&s, &root, k).unwrap(), Some(*v));
        }
        assert_eq!(entries(&s, &root).unwrap().len(), 500);
    }

    #[test]
    fn structural_sharing_root_is_deterministic() {
        // Same inserts in the same order → same root CID (content addressed).
        let build = || {
            let mut s = MemStore::new();
            let mut r = empty(&mut s).unwrap();
            for i in 0..20u32 {
                let k = EntityId::of("n", &format!("e{i}"));
                let v = dummy_cid(&mut s, i as u8);
                r = insert(&mut s, &r, k, v).unwrap();
            }
            r
        };
        assert_eq!(build(), build());
    }
}
