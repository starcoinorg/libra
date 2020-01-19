// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

use libra_crypto::hash::HashValue;
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::fmt;

/// RPC to get a chain of block of the given length starting from the given block id.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PowSyncBlock {
    height: u64,
    block_hash: HashValue,
}

impl PowSyncBlock {
    pub fn new(height: u64, block_hash: HashValue) -> Self {
        Self {
            height,
            block_hash,
        }
    }
    pub fn height(&self) -> u64 {
        self.height
    }

    pub fn block_hash(&self) -> HashValue {
        self.block_hash.clone()
    }
}

impl fmt::Display for PowSyncBlock {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "[PowSyncBlock height {} hash {:?}]",
            self.height, self.block_hash
        )
    }
}

impl TryFrom<network::proto::PowSyncBlock> for PowSyncBlock {
    type Error = anyhow::Error;

    fn try_from(proto: network::proto::PowSyncBlock) -> anyhow::Result<Self> {
        Ok(lcs::from_bytes(&proto.bytes)?)
    }
}

impl TryFrom<PowSyncBlock> for network::proto::PowSyncBlock {
    type Error = anyhow::Error;

    fn try_from(req: PowSyncBlock) -> anyhow::Result<Self> {
        Ok(Self {
            bytes: lcs::to_bytes(&req)?,
        })
    }
}

/// RPC to get a chain of block of the given length starting from the given block id.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PowSyncInfoReq {
    latest_blocks: Vec<PowSyncBlock>,
}

impl PowSyncInfoReq {
    pub fn new(latest_blocks: Vec<PowSyncBlock>) -> Self {
        Self {
            latest_blocks,
        }
    }

    pub fn new_req(latest_blocks: Vec<(u64, HashValue)>) -> Self {
        Self {
            latest_blocks: latest_blocks.iter().map(|(h, id)| {
                PowSyncBlock::new(h.clone(), id.clone())
            }).collect(),
        }
    }

    pub fn latest_blocks(&self) -> Vec<PowSyncBlock> {
        self.latest_blocks.clone()
    }
}

impl fmt::Display for PowSyncInfoReq {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "[PowSyncInfoReq with {:?} blocks]",
            self.latest_blocks
        )
    }
}

impl TryFrom<network::proto::PowSyncInfoReq> for PowSyncInfoReq {
    type Error = anyhow::Error;

    fn try_from(proto: network::proto::PowSyncInfoReq) -> anyhow::Result<Self> {
        Ok(lcs::from_bytes(&proto.bytes)?)
    }
}

impl TryFrom<PowSyncInfoReq> for network::proto::PowSyncInfoReq {
    type Error = anyhow::Error;

    fn try_from(req: PowSyncInfoReq) -> anyhow::Result<Self> {
        Ok(Self {
            bytes: lcs::to_bytes(&req)?,
        })
    }
}

/// RPC to get a chain of block of the given length starting from the given block id.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PowSyncInfoResp {
    latest_height: u64,
    common_ancestor: Option<PowSyncBlock>,
}

impl PowSyncInfoResp {
    pub fn new(latest_height: u64, common_ancestor: Option<PowSyncBlock>) -> Self {
        Self {
            latest_height,
            common_ancestor,
        }
    }
    pub fn latest_height(&self) -> u64 {
        self.latest_height
    }

    pub fn common_ancestor(&self) -> Option<PowSyncBlock> {
        self.common_ancestor.clone()
    }
}

impl fmt::Display for PowSyncInfoResp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "[PowSyncInfoResp height {} common ancestor {:?} blocks]",
            self.latest_height, self.common_ancestor
        )
    }
}

impl TryFrom<network::proto::PowSyncInfoResp> for PowSyncInfoResp {
    type Error = anyhow::Error;

    fn try_from(proto: network::proto::PowSyncInfoResp) -> anyhow::Result<Self> {
        Ok(lcs::from_bytes(&proto.bytes)?)
    }
}

impl TryFrom<PowSyncInfoResp> for network::proto::PowSyncInfoResp {
    type Error = anyhow::Error;

    fn try_from(req: PowSyncInfoResp) -> anyhow::Result<Self> {
        Ok(Self {
            bytes: lcs::to_bytes(&req)?,
        })
    }
}
