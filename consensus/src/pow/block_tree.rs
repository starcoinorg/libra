use std::collections::{HashMap, HashSet};
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

    fn add_block_info_inner(&mut self, new_block_info: BlockInfo, new_root: bool) -> Result<()> {
        let id = new_block_info.id();
        let parent_id = new_block_info.parent_id();

        //4. update height and root
        if new_root {
            self.height = new_block_info.height();
            self.root = id.clone();
        }

        //5. update parent
        self.id_to_block.get_mut(parent_id).expect("parent block not exist in block tree.").append_children(id);

        //6. update heads
        self.heads.remove(parent_id);
        self.heads.insert(id.clone());

        //7. add new block info
        self.id_to_block.insert(id.clone(), new_block_info);

        Ok(())
    }

    pub fn add_block_info(&mut self, id: &HashValue, parent_id: &HashValue, vm_output: ProcessedVMOutput) -> Result<()> {
        //1. new_block_info not exist
        let id_exist = self.id_to_block.contains_key(id);
        ensure!(!id_exist, "block already exist in block tree.");

        //2. parent exist
        let parent_height = self.id_to_block.get(parent_id).expect("parent block not exist in block tree.").height();

        //3. is new root
        let (height, new_root) = if parent_height == self.height {// new root
            (self.height + 1, true)
        } else {
            (parent_height + 1, false)
        };

        let new_block_info = BlockInfo::new(id, parent_id, height, vm_output);
        self.add_block_info_inner(new_block_info, new_root)
    }

    fn remove_branch(&mut self, head: &HashValue) {
        let parent_id = &self.id_to_block.get(head).expect("head not exist.").parent_id().clone();
        let children_len_by_parent = self.id_to_block.get(parent_id).expect("parent not exist.").children.len();
        if children_len_by_parent == 1 {
            self.id_to_block.remove(head);
            self.remove_branch(parent_id);
        }
    }

    fn remove_tail(&mut self) {
        let tail_block_info = self.id_to_block.get(&self.tail).expect("tail block not exist.");
        if (self.height - tail_block_info.height) > 100 {
            if tail_block_info.children.len() == 1 {
                let mut new_tail = None;
                for children in &tail_block_info.children {
                    new_tail = Some(children.clone())
                }

                self.id_to_block.remove(&self.tail);
                self.tail = new_tail.expect("new_tail is none.");
                self.remove_tail()
            }
        }
    }

    fn prune(&mut self) {
        //1. prune begin with heads
        if self.heads.len() > 0 {
            for head in &self.heads.clone() {
                let head_block_info = self.id_to_block.get(head).expect("head is none.");
                if (self.height - head_block_info.height()) > 100 {
                    self.heads.remove(head);
                    self.remove_branch(head);
                }
            }
        }

        //2. prune begin with tail
        self.remove_tail();
    }

    pub fn height(&self) -> u64 {
        self.height
    }

    fn root(&self) -> &HashValue {
        &self.root
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

#[cfg(any(test, feature = "fuzzing"))]
impl BlockTree {

    pub fn add_block_info_for_test(&mut self, id: &HashValue, parent_id: &HashValue) -> Result<()> {
        //1. new_block_info not exist
        let id_exist = self.id_to_block.contains_key(id);
        ensure!(!id_exist, "block already exist in block tree.");

        //2. parent exist
        let parent_height = self.id_to_block.get(parent_id).expect("parent block not exist in block tree.").height();

        //3. is new root
        let (height, new_root) = if parent_height == self.height {// new root
            (self.height + 1, true)
        } else {
            (parent_height + 1, false)
        };

        let new_block_info = BlockInfo::new_for_test(id, parent_id, height);
        self.add_block_info_inner(new_block_info, new_root)
    }
}

#[cfg(any(test, feature = "fuzzing"))]
impl BlockInfo {
    fn new_for_test(id: &HashValue, parent_id : &HashValue, height: u64) -> Self {
        BlockInfo {
            id: id.clone(),
            parent_id: parent_id.clone(),
            height,
            children: HashSet::new(),
            vm_output: None
        }
    }
}

#[test]
fn test_block_tree_add_block_info() {
    let mut block_tree = BlockTree::new();
    for i in 0..2 {
        let height = block_tree.height() + 1;
        let parent_id = block_tree.root();
        block_tree.add_block_info_for_test(&HashValue::random(), parent_id);
        if (height % 3) == 0 {
            for j in 0..2 {
                block_tree.add_block_info_for_test(&HashValue::random(), parent_id);
            }
        }
    }

    assert_eq!(block_tree.height(), 3)
}

#[test]
fn test_block_tree_prune() {
    let mut block_tree = BlockTree::new();
    for i in 0..200 {
        let height = block_tree.height() + 1;
        let parent_id = block_tree.root();
        block_tree.add_block_info_for_test(&HashValue::random(), parent_id);
        if (height % 3) == 0 {
            for j in 0..2 {
                block_tree.add_block_info_for_test(&HashValue::random(), parent_id);
            }
        }
    }

    block_tree.prune();

    let tmp = block_tree.height() - block_tree.id_to_block.get(&block_tree.tail).expect("err.").height();
    assert_eq!(tmp, 100)
}
