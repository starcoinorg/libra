use std::collections::{HashMap, HashSet, hash_map};
use libra_crypto::HashValue;
use executor::ProcessedVMOutput;
use failure::prelude::*;
use libra_crypto::hash::{GENESIS_BLOCK_ID, PRE_GENESIS_BLOCK_ID};

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
    heads: HashSet<HashValue>,
    id_to_block: HashMap<HashValue, BlockInfo>,
    tail: HashValue,
}

impl BlockTree {

    pub fn new() -> Self {
        let genesis_block_info = BlockInfo::genesis_block_info();
        let id = genesis_block_info.id().clone();
        let height = genesis_block_info.height();
        let mut heads = HashSet::new();
        heads.insert(id.clone());
        let mut id_to_block = HashMap::new();
        id_to_block.insert(id.clone(), genesis_block_info);
        BlockTree {
            height,
            root: id.clone(),
            heads,
            id_to_block,
            tail: id
        }
    }

    pub fn add_block_info(&mut self, id: &HashValue, parent_id: &HashValue, vm_output: ProcessedVMOutput) -> Result<()> {
        //1. new_block_info not exist
        let id_exist = self.id_to_block.contains_key(id);
        ensure!(id_exist, "block already exist in block tree.");

        //2. parent exist
        let parent_height = self.id_to_block.get(parent_id).expect("parent block not exist in block tree.").height();

        //3. is new root
        let (height, new_root) = if parent_height == self.height {// new root
            (self.height + 1, true)
        } else {
            (parent_height + 1, false)
        };

        let new_block_info = BlockInfo::new(id, parent_id, height, vm_output);

        //4. add new block info
        self.id_to_block.insert(id.clone(), new_block_info);

        //5. update height and root
        if new_root {
            self.height = height;
            self.root = id.clone();
        }

        //6. update heads
        self.heads.remove(parent_id);
        self.heads.insert(id.clone());

        //7. update parent
        self.id_to_block.get_mut(parent_id).expect("parent block not exist in block tree.").append_children(id);

        Ok(())
    }

    fn prune(&self) {
        //1. prune from begin
        unimplemented!()
    }

    fn height(&self) -> u64 {
        self.height
    }

    fn root(&self) -> HashValue {
        self.root
    }

    pub fn find_output_by_id(&self, id: HashValue) -> Option<BlockInfo> {
        unimplemented!()
    }
}

/// Can find parent block or children block by BlockInfo
pub struct BlockInfo {
    id: HashValue,
    parent_id : HashValue,
    height: u64,
    children: HashSet<HashValue>,
    vm_output: Option<ProcessedVMOutput>,
}

impl BlockInfo {

    fn new(id: &HashValue, parent_id : &HashValue, height: u64, vm_output: ProcessedVMOutput) -> Self {
        BlockInfo {
            id:id.clone(),
            parent_id:parent_id.clone(),
            height,
            children: HashSet::new(),
            vm_output: Some(vm_output)
        }
    }

    fn genesis_block_info() -> Self {
        BlockInfo {
            id: *GENESIS_BLOCK_ID,
            parent_id: *PRE_GENESIS_BLOCK_ID,
            height: 0,
            children: HashSet::new(),
            vm_output: None
        }
    }

    fn append_children(&mut self, children_id: &HashValue) {
        self.children.insert(children_id.clone());
    }

    fn id(&self) -> &HashValue {
        &self.id
    }

    fn parent_id(&self) -> &HashValue {
        &self.parent_id
    }

    fn height(&self) -> u64 {
        self.height
    }
}

