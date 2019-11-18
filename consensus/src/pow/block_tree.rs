use std::collections::{HashMap, HashSet};
use libra_crypto::HashValue;
use executor::ProcessedVMOutput;
use failure::Result;

///
/// ```text
///   Committed(B4) --> B5  -> B6  -> B7
///                |
///                └--> B5' -> B6' -> B7'
///                            |
///                            └----> B7"
/// ```
/// height: B7 B7' B7"
/// root: B7
/// begin: B4
pub struct BlockTree {
    height: u64,
    root: HashValue,
    id_to_block: HashMap<HashValue, BlockInfo>,
    begin: HashValue,
}

impl BlockTree {
    pub fn add_block_info(&self) -> Result<()> {
        unimplemented!()
    }

    fn prune(&self) {
        unimplemented!()
    }

    fn height(&self) -> u64 {
        self.height
    }

    fn root(&self) -> HashValue {
        unimplemented!()
    }

    pub fn find_output_by_id(&self, id: HashValue) -> Result<BlockInfo> {
        unimplemented!()
    }
}

/// Can find parent block or children block by BlockInfo
pub struct BlockInfo {
    id: HashValue,
    parent_id : HashValue,
    children: HashSet<HashValue>,
    vm_output: ProcessedVMOutput,
}

impl BlockInfo {

    fn append_children(&mut self, children_id: HashValue) {
        self.children.insert(children_id);
    }
}

