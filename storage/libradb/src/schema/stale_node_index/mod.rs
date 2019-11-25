// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

//! This module defines the physical storage schema for information related to outdated state
//! Jellyfish Merkle tree nodes, which are ready to be pruned after being old enough.
//!
//! An index entry in this data set has 2 pieces of information:
//!     1. The version since which a node (in another data set) becomes stale, meaning,
//! replaced by an updated node.
//!     2. The node_key to identify the stale node.
//!
//! ```text
//! |<--------------key-------------->|
//! | stale_since_vesrion | node_key |
//! ```
//!
//! `stale_since_version` is serialized in big endian so that records in RocksDB will be in order of
//! its numeric value.

use crate::schema::{ensure_slice_len_eq, STALE_NODE_INDEX_CF_NAME};
use failure::prelude::*;
use libra_crypto::HashValue;
use libra_types::transaction::Version;
use merkle_patricia::{node_type::NodeKey, StaleNodeIndex};
use schemadb::{
    define_schema,
    schema::{KeyCodec, SeekKeyCodec, ValueCodec},
};
use std::io::Write;

define_schema!(
    StaleNodeIndexSchema,
    StaleNodeIndex,
    (),
    STALE_NODE_INDEX_CF_NAME
);

impl KeyCodec<StaleNodeIndexSchema> for StaleNodeIndex {
    fn encode_key(&self) -> Result<Vec<u8>> {
        let mut encoded = vec![];
        encoded.write_all(self.stale_since_hash.to_vec().as_slice())?;
        encoded.write_all(&self.node_key.encode()?)?;

        Ok(encoded)
    }

    fn decode_key(data: &[u8]) -> Result<Self> {
        let since_hash = HashValue::from_slice(&data[0..HashValue::LENGTH])?;
        let node_key = NodeKey::decode(&data[HashValue::LENGTH..])?;

        Ok(Self {
            stale_since_hash: since_hash,
            node_key,
        })
    }
}

impl ValueCodec<StaleNodeIndexSchema> for () {
    fn encode_value(&self) -> Result<Vec<u8>> {
        Ok(Vec::new())
    }

    fn decode_value(data: &[u8]) -> Result<Self> {
        ensure_slice_len_eq(data, 0)?;
        Ok(())
    }
}

impl SeekKeyCodec<StaleNodeIndexSchema> for Version {
    fn encode_seek_key(&self) -> Result<Vec<u8>> {
        Ok(self.to_be_bytes().to_vec())
    }
}

#[cfg(test)]
mod test;
