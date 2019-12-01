// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{
    mock_tree_store::MockTreeStore, restore::MerklePatriciaRestore, test_helper::init_mock_db,
    MerklePatriciaTree, TreeReader,
};
use libra_crypto::HashValue;
use libra_types::{account_state_blob::AccountStateBlob, transaction::Version};
use proptest::{collection::btree_map, prelude::*};
use std::collections::BTreeMap;

fn assert_success(
    db: &MockTreeStore,
    expected_root_hash: HashValue,
    btree: &BTreeMap<HashValue, AccountStateBlob>,
    _version: Version,
) {
    let tree = MerklePatriciaTree::new(db);
    for (key, value) in btree {
        assert_eq!(tree.get(*key).unwrap(), Some(value.clone()));
    }

    let actual_root_hash = tree.get_root_hash().unwrap();
    assert_eq!(actual_root_hash, expected_root_hash);
}
