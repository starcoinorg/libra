use crate::{
    account_address::AccountAddress,
    account_config::core_code_address,
    identifier::{IdentStr, Identifier},
    language_storage::StructTag,
};
use failure::prelude::*;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

lazy_static! {
    // Channel
    static ref CHANNEL_MODULE_NAME: Identifier = Identifier::new("LibraAccount").unwrap();
    static ref CHANNEL_STRUCT_NAME: Identifier = Identifier::new("Channel").unwrap();
}

pub fn channel_module_name() -> &'static IdentStr {
    &*CHANNEL_MODULE_NAME
}

pub fn channel_struct_name() -> &'static IdentStr {
    &*CHANNEL_STRUCT_NAME
}

pub fn channel_struct_tag() -> StructTag {
    StructTag {
        address: core_code_address(),
        module: channel_module_name().to_owned(),
        name: channel_struct_name().to_owned(),
        type_params: vec![],
    }
}

/// A Rust representation of an Channel resource.
/// This is not how the Channel is represented in the VM but it's a convenient
/// representation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChannelResource {
    channel_sequence_number: u64,
    closed: bool,
    locked: bool,
    participants: Vec<AccountAddress>,
}

impl ChannelResource {
    pub fn new(
        channel_sequence_number: u64,
        closed: bool,
        locked: bool,
        participants: Vec<AccountAddress>,
    ) -> Self {
        Self {
            channel_sequence_number,
            closed,
            locked,
            participants,
        }
    }

    pub fn channel_sequence_number(&self) -> u64 {
        self.channel_sequence_number
    }

    pub fn closed(&self) -> bool {
        self.closed
    }

    pub fn locked(&self) -> bool {
        self.locked
    }

    pub fn participants(&self) -> &[AccountAddress] {
        self.participants.as_slice()
    }

    pub fn make_from(bytes: Vec<u8>) -> Result<Self> {
        lcs::from_bytes(bytes.as_slice()).map_err(|e| Into::into(e))
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        lcs::to_bytes(self).unwrap()
    }
}
