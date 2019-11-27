// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{mock_tree_store::MockTreeStore, MerklePatriciaTree};
use libra_crypto::HashValue;
use libra_types::{account_state_blob::AccountStateBlob, transaction::Version};
use std::collections::HashMap;

/// Computes the key immediately after `key`.
pub fn plus_one(key: HashValue) -> HashValue {
    assert_ne!(key, HashValue::new([0xff; HashValue::LENGTH]));

    let mut buf = key.to_vec();
    for i in (0..HashValue::LENGTH).rev() {
        if buf[i] == 255 {
            buf[i] = 0;
        } else {
            buf[i] += 1;
            break;
        }
    }
    HashValue::from_slice(&buf).unwrap()
}

/// Initializes a DB with a set of key-value pairs by inserting one key at each version.
pub fn init_mock_db(kvs: &HashMap<HashValue, AccountStateBlob>) -> (MockTreeStore, Version) {
    assert!(!kvs.is_empty());

    let db = MockTreeStore::default();
    let tree = MerklePatriciaTree::new(&db);

    for (_i, (key, value)) in kvs.iter().enumerate() {
        let (_root_hash, write_batch) = tree
            .put_blob_set(HashValue::zero(), vec![(*key, value.clone())])
            .unwrap();
        db.write_tree_update_batch(write_batch).unwrap();
    }

    (db, (kvs.len() - 1) as Version)
}
