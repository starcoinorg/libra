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
    let cache = TreeCache::new(&db, HashValue::zero(),0);

    let (node, node_key) = random_leaf_with_key(HashValue::zero());
    db.put_node(node_key.clone(), node.clone()).unwrap();

    assert_eq!(cache.get_node(&node_key).unwrap(), node);
}

#[test]
fn test_root_node() {
    let db = MockTreeStore::default();
    let mut cache = TreeCache::new(&db, HashValue::zero(),0);
    assert_eq!(
        *cache.get_root_node_key(),
        NodeKey::new_empty_path(HashValue::zero())
    );

    let (node, node_key) = random_leaf_with_key(HashValue::zero());
    db.put_node(node_key.clone(), node.clone()).unwrap();
    cache.set_root_node_key(node_key.clone());

    assert_eq!(*cache.get_root_node_key(), node_key);
}

#[test]
fn test_freeze_with_delete() {
    let db = MockTreeStore::default();
    let mut cache = TreeCache::new(&db, HashValue::zero(),0);

    assert_eq!(
        *cache.get_root_node_key(),
        NodeKey::new_empty_path(HashValue::zero())
    );

    let (node1, node1_key) = random_leaf_with_key(HashValue::zero());
    cache.put_node(node1_key.clone(), node1.clone()).unwrap();
    let (node2, node2_key) = random_leaf_with_key(HashValue::zero());
    cache.put_node(node2_key.clone(), node2.clone()).unwrap();
    assert_eq!(cache.get_node(&node1_key).unwrap(), node1);
    assert_eq!(cache.get_node(&node2_key).unwrap(), node2);
    cache.freeze();
    assert_eq!(cache.get_node(&node1_key).unwrap(), node1);
    assert_eq!(cache.get_node(&node2_key).unwrap(), node2);

    cache.delete_node(&node1_key, true /* is_leaf */);
    cache.freeze();
    let (_, update_batch) = cache.into();
    assert_eq!(update_batch.node_batch.len(), 3);
    assert_eq!(update_batch.stale_node_index_batch.len(), 1);
}
