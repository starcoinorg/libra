// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::{mock_tree_store::MockTreeStore, nibble_path::NibblePath, node_type::Node, NodeKey};
use libra_crypto::HashValue;
use libra_types::account_state_blob::AccountStateBlob;

fn random_leaf_with_key(next_hash: HashValue) -> (Node, NodeKey) {
    let address = HashValue::random();
    let node = Node::new_leaf(
        address,
        AccountStateBlob::from(HashValue::random().to_vec()),
    );
    let node_key = NodeKey::new(next_hash, NibblePath::new(address.to_vec()));
    (node, node_key)
}

#[test]
fn test_get_node() {
    let db = MockTreeStore::default();
    let cache = TreeCache::new(&db, HashValue::zero(), 0);

    let (node, node_key) = random_leaf_with_key(HashValue::zero());
    db.put_node(node_key.clone(), node.clone()).unwrap();

    assert_eq!(cache.get_node(&node_key).unwrap(), node);
}
