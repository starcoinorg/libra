// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

use super::*;
use libra_crypto::{
    hash::{CryptoHash, SPARSE_MERKLE_PLACEHOLDER_HASH},
    HashValue,
};
use libra_types::proof::{SparseMerkleInternalNode, SparseMerkleLeafNode};
use proptest::prelude::*;
use std::{panic, rc::Rc};

fn hash_internal(left: HashValue, right: HashValue) -> HashValue {
    SparseMerkleInternalNode::new(left, right).hash()
}

fn hash_leaf(key: HashValue, value_hash: HashValue) -> HashValue {
    SparseMerkleLeafNode::new(key, value_hash).hash()
}

// Generate a random node key with 63 nibbles.
fn random_63nibbles_node_key() -> NodeKey {
    let tmp_hash = HashValue::random();
    let mut bytes = tmp_hash.to_vec();
    *bytes.last_mut().unwrap() &= 0xf0;
    NodeKey::new(tmp_hash, NibblePath::new_odd(bytes))
}

// Generate a pair of leaf node key and account key with a passed-in 63-nibble node key and the last
// nibble to be appended.
fn gen_leaf_keys(nibble_path: &NibblePath, nibble: Nibble) -> (NodeKey, HashValue) {
    assert_eq!(nibble_path.num_nibbles(), 63);
    let mut np = nibble_path.clone();
    np.push(nibble);
    let account_key = HashValue::from_slice(np.bytes()).unwrap();
    (NodeKey::new(account_key, np), account_key)
}

#[test]
fn test_encode_decode() {
    let internal_node_key = random_63nibbles_node_key();

    let leaf1_keys = gen_leaf_keys(internal_node_key.nibble_path(), Nibble::from(1));
    let leaf1_node = Node::new_leaf(leaf1_keys.1, AccountStateBlob::from(vec![0x00]));
    let leaf2_keys = gen_leaf_keys(internal_node_key.nibble_path(), Nibble::from(2));
    let leaf2_node = Node::new_leaf(leaf2_keys.1, AccountStateBlob::from(vec![0x01]));

    let mut children = Children::default();
    children.insert(Nibble::from(1), Child::new(leaf1_node.hash(), true));
    children.insert(Nibble::from(2), Child::new(leaf2_node.hash(), true));

    let account_key = HashValue::random();
    let nodes = vec![
        Node::new_internal(children),
        Node::new_leaf(account_key, AccountStateBlob::from(vec![0x02])),
    ];
    for n in &nodes {
        let v = n.encode().unwrap();
        assert_eq!(*n, Node::decode(&v).unwrap());
    }
    // Error cases
    if let Err(e) = Node::decode(&[]) {
        assert_eq!(
            e.downcast::<NodeDecodeError>().unwrap(),
            NodeDecodeError::EmptyInput
        );
    }
    if let Err(e) = Node::decode(&[100]) {
        assert_eq!(
            e.downcast::<NodeDecodeError>().unwrap(),
            NodeDecodeError::UnknownTag { unknown_tag: 100 }
        );
    }
}

proptest! {
    #[test]
    fn test_u64_varint_roundtrip(input in any::<u64>()) {
        let mut vec = vec![];
        serialize_u64_varint(input, &mut vec);
        assert_eq!(deserialize_u64_varint(&mut Cursor::new(vec)).unwrap(), input);
    }

    #[test]
    fn test_internal_node_roundtrip(input in any::<InternalNode>()) {
        let mut vec = vec![];
        input.serialize(&mut vec).unwrap();
        assert_eq!(InternalNode::deserialize(&vec).unwrap(), input);
    }
}

#[test]
fn test_internal_validity() {
    let result = panic::catch_unwind(|| {
        let children = Children::default();
        InternalNode::new(children)
    });
    assert!(result.is_err());

    let result = panic::catch_unwind(|| {
        let mut children = Children::default();
        children.insert(
            Nibble::from(1),
            Child::new(HashValue::random(), true /* is_leaf */),
        );
        InternalNode::new(children);
    });
    assert!(result.is_err());
}

#[test]
fn test_leaf_hash() {
    {
        let address = HashValue::random();
        let blob = AccountStateBlob::from(vec![0x02]);
        let value_hash = blob.hash();
        let hash = hash_leaf(address, value_hash);
        let leaf_node = Node::new_leaf(address, blob);
        assert_eq!(leaf_node.hash(), hash);
    }
}

enum BinaryTreeNode {
    Internal(BinaryTreeInternalNode),
    Child(BinaryTreeChildNode),
    Null,
}

impl BinaryTreeNode {
    fn new_child(index: u8, child: &Child) -> Self {
        Self::Child(BinaryTreeChildNode {
            index,
            hash: child.hash,
            is_leaf: child.is_leaf,
        })
    }

    fn new_internal(
        first_child_index: u8,
        num_children: u8,
        left: BinaryTreeNode,
        right: BinaryTreeNode,
    ) -> Self {
        let hash = SparseMerkleInternalNode::new(left.hash(), right.hash()).hash();

        Self::Internal(BinaryTreeInternalNode {
            begin: first_child_index,
            width: num_children,
            left: Rc::new(left),
            right: Rc::new(right),
            hash,
        })
    }

    fn hash(&self) -> HashValue {
        match self {
            BinaryTreeNode::Internal(node) => node.hash,
            BinaryTreeNode::Child(node) => node.hash,
            BinaryTreeNode::Null => *SPARSE_MERKLE_PLACEHOLDER_HASH,
        }
    }
}

/// An internal node in a binary tree corresponding to a `InternalNode` being tested.
///
/// To describe its position in the binary tree, we use a range of level 0 (children level)
/// positions expressed by (`begin`, `width`)
///
/// For example, in the below graph, node A has (begin:0, width:4), while node B has
/// (begin:2, width: 2):
///            ...
///         /
///       [A]    ...
///     /    \
///    * [B]   ...
///   / \    / \
///  0   1  2   3    ... 15
struct BinaryTreeInternalNode {
    begin: u8,
    width: u8,
    left: Rc<BinaryTreeNode>,
    right: Rc<BinaryTreeNode>,
    hash: HashValue,
}

impl BinaryTreeInternalNode {
    fn in_left_subtree(&self, n: u8) -> bool {
        assert!(n >= self.begin);
        assert!(n < self.begin + self.width);

        n < self.begin + self.width / 2
    }
}

/// A child node, corresponding to one that is in the corresponding `InternalNode` being tested.
///
/// `index` is its key in `InternalNode::children`.
/// N.B. when `is_leaf` is true, in the binary tree represented by a `NaiveInternalNode`, the child
/// node will be brought up to the root of the highest subtree that has only that leaf.
#[derive(Clone, Copy)]
struct BinaryTreeChildNode {
    index: u8,
    hash: HashValue,
    is_leaf: bool,
}

struct NaiveInternalNode {
    root: Rc<BinaryTreeNode>,
}

impl NaiveInternalNode {
    fn from_clever_node(node: &InternalNode) -> Self {
        Self {
            root: Rc::new(Self::node_for_subtree(0, 16, &node.children)),
        }
    }

    fn node_for_subtree(begin: u8, width: u8, children: &Children) -> BinaryTreeNode {
        if width == 1 {
            return children
                .get(&begin.into())
                .map_or(BinaryTreeNode::Null, |child| {
                    BinaryTreeNode::new_child(begin, &child)
                });
        }

        let half_width = width / 2;
        let left = Self::node_for_subtree(begin, half_width, children);
        let right = Self::node_for_subtree(begin + half_width, half_width, children);

        match (&left, &right) {
            (BinaryTreeNode::Null, BinaryTreeNode::Null) => {
                return BinaryTreeNode::Null;
            }
            (BinaryTreeNode::Null, BinaryTreeNode::Child(node))
            | (BinaryTreeNode::Child(node), BinaryTreeNode::Null) => {
                if node.is_leaf {
                    return BinaryTreeNode::Child(*node);
                }
            }
            _ => (),
        };

        BinaryTreeNode::new_internal(begin, width, left, right)
    }

    fn get_child_with_siblings(
        &self,
        node_key: &NodeKey,
        n: u8,
    ) -> (Option<NodeKey>, Vec<HashValue>) {
        let mut current_node = Rc::clone(&self.root);
        let mut siblings = Vec::new();

        loop {
            match current_node.as_ref() {
                BinaryTreeNode::Internal(node) => {
                    if node.in_left_subtree(n) {
                        siblings.push(node.right.hash());
                        current_node = Rc::clone(&node.left);
                    } else {
                        siblings.push(node.left.hash());
                        current_node = Rc::clone(&node.right);
                    }
                }
                BinaryTreeNode::Child(node) => {
                    return (
                        Some(node_key.gen_child_node_key(node.hash, node.index.into())),
                        siblings,
                    )
                }
                BinaryTreeNode::Null => return (None, siblings),
            }
        }
    }
}

proptest! {
    #[test]
    #[allow(clippy::unnecessary_operation)]
    fn test_get_child_with_siblings(
        node_key in any::<NodeKey>().prop_filter(
            "Filter out keys for leaves.",
            |k| k.nibble_path().num_nibbles() < 64
        ).no_shrink(),
        node in any::<InternalNode>(),
    ) {
        for n in 0..16u8 {
            prop_assert_eq!(
                node.get_child_with_siblings(&node_key, n.into()),
                NaiveInternalNode::from_clever_node(&node).get_child_with_siblings(&node_key, n)
            )
        }
    }
}
