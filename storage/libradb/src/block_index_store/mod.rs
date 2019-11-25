// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

use failure::prelude::*;
use libra_logger::prelude::*;
use schemadb::{DB, SchemaBatch};
use std::sync::Arc;
use crate::schema::block_index::{BlockIndex, BlockIndexSchema};

pub(crate) struct BlockIndexStore {
    db: Arc<DB>,
}

impl BlockIndexStore {
    pub fn new(db: Arc<DB>) -> Self {
        Self { db }
    }

    /// Insert BlockIndex
    pub fn insert_block_index(&self, height: &u64, block_index: &BlockIndex) -> Result<()> {
        let mut batch = SchemaBatch::new();
        batch.put::<BlockIndexSchema>(&height, &block_index)?;
        self.db.write_schemas(batch)
    }

    /// Load BlockIndex
    pub fn _load_block_index(&self) -> Result<Vec<BlockIndex>> {
        unimplemented!()
    }
}
