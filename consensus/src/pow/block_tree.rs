use std::collections::{HashMap, HashSet};
use libra_crypto::HashValue;
use executor::ProcessedVMOutput;

///
/// ```text
///   Committed(B4)-> B5  -> B6  -> B7
///                |
///                └--> B5' -> B6' -> B7'
///                            |
///                            └----> B7"
/// ```
/// height: B7 B7' B7"
/// root: B7
/// begin: B4
struct BlockTree {
    height: u64,
    root: HashValue,
    id_to_block: HashMap<HashValue, BlockInfo>,
    begin: HashValue,
}

/// Can find parent block or children block by BlockInfo
struct BlockInfo {
    id: HashValue,
    parent_id : HashValue,
    children: HashSet<HashValue>,
    vm_output: ProcessedVMOutput,
}