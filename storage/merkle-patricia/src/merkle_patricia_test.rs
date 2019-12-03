// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

use super::*;
use libra_crypto::HashValue;
use libra_nibble::Nibble;
use mock_tree_store::MockTreeStore;
use rand::{rngs::StdRng, SeedableRng};
use std::collections::HashMap;

fn update_nibble(original_key: &HashValue, n: usize, nibble: u8) -> HashValue {
    assert!(nibble < 16);
    let mut key = original_key.to_vec();
    key[n / 2] = if n % 2 == 0 {
        key[n / 2] & 0x0f | nibble << 4
    } else {
        key[n / 2] & 0xf0 | nibble
    };
    HashValue::from_slice(&key).unwrap()
}

#[test]
fn test_insert_to_empty_tree() {
    let db = MockTreeStore::default();
    let tree = MerklePatriciaTree::new(&db);

    // Tree is initially empty. Root is a null node. We'll insert a key-value pair which creates a
    // leaf node.
    let key = HashValue::random();
    let value = AccountStateBlob::from(vec![1u8, 2u8, 3u8, 4u8]);

    let (new_root_hash, batch) = tree
        .put_blob_set(HashValue::zero(), vec![(key, value.clone())])
        .unwrap();
    assert!(batch.stale_node_index_batch.is_empty());
    db.write_tree_update_batch(batch).unwrap();
    assert_eq!(tree.get(new_root_hash).unwrap().unwrap(), value);
}

#[test]
fn test_insert_at_leaf_with_internal_created() {
    let db = MockTreeStore::default();
    let tree = MerklePatriciaTree::new(&db);

    let key1 = HashValue::new([0x00u8; HashValue::LENGTH]);
    let value1 = AccountStateBlob::from(vec![1u8, 2u8]);

    let (root0_hash, batch) = tree
        .put_blob_set(HashValue::zero(), vec![(key1, value1.clone())])
        .unwrap();

    assert!(batch.stale_node_index_batch.is_empty());
    db.write_tree_update_batch(batch).unwrap();
    assert_eq!(tree.get(root0_hash).unwrap().unwrap(), value1);

    // Insert at the previous leaf node. Should generate an internal node at the root.
    // Change the 1st nibble to 15.
    let key2 = update_nibble(&key1, 0, 15);
    let value2 = AccountStateBlob::from(vec![3u8, 4u8]);

    let (root1_hash, batch) = tree
        .put_blob_set(HashValue::zero(), vec![(key2, value2.clone())])
        .unwrap();
    //TODO
    //    assert_eq!(batch.stale_node_index_batch.len(), 1);
    db.write_tree_update_batch(batch).unwrap();

    assert_eq!(tree.get(root0_hash).unwrap().unwrap(), value1);
    assert_eq!(tree.get(root1_hash).unwrap().unwrap(), value2);

    // get # of nodes
    assert_eq!(db.num_nodes(), 2);

    let internal_node_key = NodeKey::new_empty_path(key2);

    let leaf1 = Node::new_leaf(key1, value1);
    let leaf2 = Node::new_leaf(key2, value2);
    let mut children = HashMap::new();
    children.insert(
        Nibble::from(0),
        Child::new(leaf1.hash(), true /* is_leaf */),
    );
    children.insert(
        Nibble::from(15),
        Child::new(leaf2.hash(), true /* is_leaf */),
    );
    let internal = Node::new_internal(children);
    assert_eq!(
        db.get_node(&NodeKey::new_empty_path(leaf1.hash())).unwrap(),
        leaf1
    );
    assert_eq!(
        db.get_node(&NodeKey::new_empty_path(leaf2.hash())).unwrap(),
        leaf2
    );
}

#[test]
fn test_insert_at_leaf_with_multiple_internals_created() {
    let db = MockTreeStore::default();
    let tree = MerklePatriciaTree::new(&db);

    // 1. Insert the first leaf into empty tree
    let key1 = HashValue::new([0x00u8; HashValue::LENGTH]);
    let value1 = AccountStateBlob::from(vec![1u8, 2u8]);
    let mut kvs_map: HashMap<HashValue, AccountStateBlob> = HashMap::new();
    kvs_map.insert(key1, value1.clone());

    let (root0_hash, batch) = tree
        .put_blob_set(HashValue::zero(), vec![(key1, value1.clone())])
        .unwrap();
    db.write_tree_update_batch(batch.clone()).unwrap();
    batch_proof_verify(&tree, &mut kvs_map, root0_hash, batch);

    // 2. Insert at the previous leaf node. Should generate a branch node at root.
    // Change the 2nd nibble to 1.
    let key2 = update_nibble(&key1, 1 /* nibble_index */, 1 /* nibble */);
    let value2 = AccountStateBlob::from(vec![3u8, 4u8]);
    kvs_map.insert(key2, value2.clone());

    let (root1_hash, batch) = tree
        .put_blob_set(HashValue::zero(), vec![(key2, value2.clone())])
        .unwrap();
    db.write_tree_update_batch(batch.clone()).unwrap();
    batch_proof_verify(&tree, &mut kvs_map, root1_hash, batch);
    assert_eq!(db.num_nodes(), 2);
}

fn many_keys_get_proof_and_verify_tree_root(seed: &[u8], num_keys: usize) {
    assert!(seed.len() < 32);
    let mut actual_seed = [0u8; 32];
    actual_seed[..seed.len()].copy_from_slice(&seed);
    let mut rng: StdRng = StdRng::from_seed(actual_seed);

    let db = MockTreeStore::default();
    let tree = MerklePatriciaTree::new(&db);

    let mut kvs = vec![];
    let mut kvs_map: HashMap<HashValue, AccountStateBlob> = HashMap::new();
    for _i in 0..num_keys {
        let key = HashValue::random_with_rng(&mut rng);
        let value = AccountStateBlob::from(HashValue::random_with_rng(&mut rng).to_vec());
        kvs.push((key, value.clone()));
        kvs_map.insert(key, value.clone());
    }

    let (root, batch) = tree.put_blob_set(HashValue::zero(), kvs.clone()).unwrap();
    db.write_tree_update_batch(batch.clone()).unwrap();
    batch_proof_verify(&tree, &mut kvs_map, root, batch)
}

fn batch_proof_verify(
    tree: &MerklePatriciaTree<MockTreeStore>,
    kvs_map: &mut HashMap<HashValue, AccountStateBlob>,
    root: HashValue,
    batch: TreeUpdateBatch,
) -> () {
    for index in batch.stale_node_index_batch.iter() {
        let np = index.node_key.nibble_path();
        let np_bytes = np.bytes();
        if np_bytes.len() != HashValue::LENGTH {
            continue;
        }
        let hash = index.node_key.hash();
        let account_key = HashValue::from_slice(np_bytes).unwrap();
        let (_value, proof) = tree.get_with_proof(hash).unwrap();
        let v = kvs_map.get(&account_key);
        assert!(proof.verify(root, account_key, v).is_ok());
    }
}

#[test]
fn test_1000_keys() {
    let seed: &[_] = &[1, 2, 3, 4];
    many_keys_get_proof_and_verify_tree_root(seed, 1000);
}

fn test_existent_keys_impl<'a>(
    tree: &MerklePatriciaTree<'a, MockTreeStore>,
    vec_hash: Vec<HashValue>,
) {
    let _root_hash = tree.get_root_hash().unwrap();
    for hash in vec_hash.iter() {
        let (account, _proof) = tree.get_with_proof(*hash).unwrap();
        assert!(account.is_some());
    }
}

fn test_nonexistent_keys_impl<'a>(
    tree: &MerklePatriciaTree<'a, MockTreeStore>,
    nonexistent_keys: &[HashValue],
) {
    let root_hash = tree.get_root_hash().unwrap();

    for key in nonexistent_keys {
        let (account, proof) = tree.get_with_proof(*key).unwrap();
        assert!(proof.verify(root_hash, *key, account.as_ref()).is_ok());
        assert!(account.is_none());
    }
}
