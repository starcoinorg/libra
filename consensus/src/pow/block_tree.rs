use std::collections::{HashMap, LinkedList};
use libra_crypto::HashValue;
use executor::ProcessedVMOutput;
use failure::prelude::*;
use libra_crypto::hash::{PRE_GENESIS_BLOCK_ID};
use crate::chained_bft::consensusdb::BlockIndex;
use atomic_refcell::AtomicRefCell;
use crate::pow::payload_ext::{BlockPayloadExt, genesis_id};

pub type BlockHeight = u64;

///
/// ```text
///   Committed(B4) --> B5  -> B6  -> B7
///                |
///             B4'└--> B5' -> B6' -> B7'
///                            |
///                            └----> B7"
/// ```
/// height: B7 B7' B7"
/// tail_height: B4 B4'
pub struct BlockTree {
    height: BlockHeight,
    id_to_block: HashMap<HashValue, BlockInfo>,
    indexes: HashMap<BlockHeight, LinkedList<HashValue>>,
    main_chain: AtomicRefCell<HashMap<BlockHeight, BlockIndex>>,
    tail_height: BlockHeight,
}

impl BlockTree {
    pub fn new() -> Self {
        // genesis block info
        let genesis_block_info = BlockInfo::genesis_block_info();
        let genesis_id = genesis_block_info.id();
        let genesis_height = genesis_block_info.height();

        // indexes
        let mut genesis_indexes = LinkedList::new();
        genesis_indexes.push_front(genesis_id.clone());
        let mut indexes = HashMap::new();
        indexes.insert(genesis_height, genesis_indexes);

        // main chain
        let main_chain = AtomicRefCell::new(HashMap::new());
        main_chain
            .borrow_mut()
            .insert(genesis_height, genesis_block_info.block_index().clone());

        // id to block
        let mut id_to_block = HashMap::new();
        id_to_block.insert(genesis_id.clone(), genesis_block_info);

        BlockTree {
            height: genesis_height,
            id_to_block,
            indexes,
            main_chain,
            tail_height: genesis_height,
        }
    }

    fn prune(&self) {
        let times = self.height - self.tail_height - 1000;
        if times > 0 {
            for i in 0..times {
                //TODO
            }
        }
    }

    fn add_block_info_inner(&mut self, new_block_info: BlockInfo, new_root: bool) -> Result<()> {
        //4. update height\indexes\main chain
        if new_root {
            self.height = new_block_info.height();
            self.main_chain.borrow_mut().insert(new_block_info.height(), new_block_info.block_index().clone());
            let mut hash_list = LinkedList::new();
            hash_list.push_front(new_block_info.id().clone());
            self.indexes.insert(new_block_info.height(), hash_list);
        } else {
            self.indexes.get_mut(&new_block_info.height()).unwrap().push_back(new_block_info.id().clone());
        }

        //5. add new block info
        self.id_to_block.insert(new_block_info.id().clone(), new_block_info);

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

    pub fn height(&self) -> BlockHeight {
        self.height
    }

    pub fn find_output_by_id(&self, id: HashValue) -> Option<BlockInfo> {
        unimplemented!()
    }
}

/// Can find parent block or children block by BlockInfo
pub struct BlockInfo {
    block_index: BlockIndex,
    height: BlockHeight,
    vm_output: Option<ProcessedVMOutput>,
}

impl BlockInfo {
    pub fn new(id: &HashValue, parent_id: &HashValue, height: BlockHeight, vm_output: ProcessedVMOutput) -> Self {
        Self::new_inner(id, parent_id, height, Some(vm_output))
    }

    fn new_inner(id: &HashValue, parent_id: &HashValue, height: BlockHeight, vm_output: Option<ProcessedVMOutput>) -> Self {
        let block_index = BlockIndex::new(id, parent_id);
        BlockInfo {
            block_index,
            height,
            vm_output,
        }
    }

    fn genesis_block_info() -> Self {
        BlockInfo::new_inner(&genesis_id(),
                             &PRE_GENESIS_BLOCK_ID,
                             0,
                             None)
    }

    fn block_index(&self) -> BlockIndex {
        self.block_index
    }

    fn id(&self) -> HashValue {
        self.block_index.id()
    }

    fn height(&self) -> BlockHeight {
        self.height
    }

    fn parent_id(&self) -> HashValue {
        self.block_index.parent_id()
    }
}

#[cfg(any(test, feature = "fuzzing"))]
impl BlockTree {
    pub fn add_block_info_for_test(&mut self, id: &HashValue, parent_id: &HashValue) -> Result<()> {
        //1. new_block_info not exist
        let id_exist = self.id_to_block.contains_key(id);
        ensure!( ! id_exist, "block already exist in block tree.");

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
    fn new_for_test(id: &HashValue, parent_id: &HashValue, height: BlockHeight) -> Self {
        Self::new_inner(
            id,
            parent_id, height, None)
    }
}
