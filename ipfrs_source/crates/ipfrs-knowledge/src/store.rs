//! Content-addressed block store abstraction. The in-memory impl backs the slice;
//! a real deployment wires this to `ipfrs-storage`.

use std::collections::HashMap;

use ipfrs_core::Cid;

pub trait BlockStore {
    fn put(&mut self, cid: Cid, bytes: Vec<u8>);
    fn get(&self, cid: &Cid) -> Option<&[u8]>;
    fn has(&self, cid: &Cid) -> bool {
        self.get(cid).is_some()
    }
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Default, Clone)]
pub struct MemStore {
    blocks: HashMap<Cid, Vec<u8>>,
}

impl MemStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl BlockStore for MemStore {
    fn put(&mut self, cid: Cid, bytes: Vec<u8>) {
        self.blocks.insert(cid, bytes);
    }
    fn get(&self, cid: &Cid) -> Option<&[u8]> {
        self.blocks.get(cid).map(|v| v.as_slice())
    }
    fn len(&self) -> usize {
        self.blocks.len()
    }
}
