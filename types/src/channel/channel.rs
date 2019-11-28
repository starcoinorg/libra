use crate::{
    account_address::AccountAddress,
    account_config::core_code_address,
    identifier::{IdentStr, Identifier},
    language_storage::StructTag,
};
use failure::prelude::*;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;

lazy_static! {
    // Channel
    static ref CHANNEL_MODULE_NAME: Identifier = Identifier::new("LibraAccount").unwrap();
    static ref CHANNEL_STRUCT_NAME: Identifier = Identifier::new("Channel").unwrap();

    static ref CHANNEL_MIRROR_MODULE_NAME: Identifier = Identifier::new("LibraAccount").unwrap();
    static ref CHANNEL_MIRROR_STRUCT_NAME: Identifier = Identifier::new("ChannelMirror").unwrap();

    static ref CHANNEL_PARTICIPANT_MODULE_NAME: Identifier = Identifier::new("LibraAccount").unwrap();
    static ref CHANNEL_PARTICIPANT_STRUCT_NAME: Identifier = Identifier::new("ChannelParticipantAccount").unwrap();

    static ref USER_CHANNELS_MODULE_NAME: Identifier = Identifier::new("LibraAccount").unwrap();
    static ref USER_CHANNELS_STRUCT_NAME: Identifier = Identifier::new("UserChannels").unwrap();
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

pub fn channel_mirror_module_name() -> &'static IdentStr {
    &*CHANNEL_MIRROR_MODULE_NAME
}

pub fn channel_mirror_struct_name() -> &'static IdentStr {
    &*CHANNEL_MIRROR_STRUCT_NAME
}

pub fn channel_mirror_struct_tag() -> StructTag {
    StructTag {
        address: core_code_address(),
        module: channel_mirror_module_name().to_owned(),
        name: channel_mirror_struct_name().to_owned(),
        type_params: vec![],
    }
}

pub fn channel_participant_module_name() -> &'static IdentStr {
    &*CHANNEL_PARTICIPANT_MODULE_NAME
}

pub fn channel_participant_struct_name() -> &'static IdentStr {
    &*CHANNEL_PARTICIPANT_STRUCT_NAME
}

pub fn channel_participant_struct_tag() -> StructTag {
    StructTag {
        address: core_code_address(),
        module: channel_participant_module_name().to_owned(),
        name: channel_participant_struct_name().to_owned(),
        type_params: vec![],
    }
}

pub fn user_channels_module_name() -> &'static IdentStr {
    &*USER_CHANNELS_MODULE_NAME
}

pub fn user_channels_struct_name() -> &'static IdentStr {
    &*USER_CHANNELS_STRUCT_NAME
}

pub fn user_channels_struct_tag() -> StructTag {
    StructTag {
        address: core_code_address(),
        module: user_channels_module_name().to_owned(),
        name: user_channels_struct_name().to_owned(),
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

/// A Rust representation of an ChannelMirror resource.
/// ChannelMirror resource save on channel's shared resource space.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChannelMirrorResource {
    channel_sequence_number: u64,
}

impl ChannelMirrorResource {
    pub fn new(channel_sequence_number: u64) -> Self {
        Self {
            channel_sequence_number,
        }
    }

    pub fn channel_sequence_number(&self) -> u64 {
        self.channel_sequence_number
    }

    pub fn make_from(bytes: Vec<u8>) -> Result<Self> {
        lcs::from_bytes(bytes.as_slice()).map_err(|e| Into::into(e))
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        lcs::to_bytes(self).unwrap()
    }
}

/// A Rust representation of an ChannelParticipantAccount resource.
/// ChannelParticipantAccount resource save on channel's participant resource space.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChannelParticipantAccountResource {
    balance: u64,
}

impl ChannelParticipantAccountResource {
    pub fn new(balance: u64) -> Self {
        Self { balance }
    }

    pub fn balance(&self) -> u64 {
        self.balance
    }

    pub fn make_from(bytes: Vec<u8>) -> Result<Self> {
        lcs::from_bytes(bytes.as_slice()).map_err(|e| Into::into(e))
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        lcs::to_bytes(self).unwrap()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserChannelsResource {
    channels: Vec<AccountAddress>,
}

impl UserChannelsResource {
    pub fn new(channels: Vec<AccountAddress>) -> Self {
        Self { channels }
    }

    pub fn channels(&self) -> &[AccountAddress] {
        &self.channels
    }
}

impl TryFrom<&[u8]> for UserChannelsResource {
    type Error = Error;

    fn try_from(value: &[u8]) -> Result<Self> {
        lcs::from_bytes(value).map_err(|e| Into::into(e))
    }
}

impl Into<Vec<AccountAddress>> for UserChannelsResource {
    fn into(self) -> Vec<AccountAddress> {
        self.channels
    }
}
