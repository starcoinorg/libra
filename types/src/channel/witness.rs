use crate::write_set::WriteSet;
use libra_crypto::hash::CryptoHash;
use libra_crypto::{ed25519::Ed25519Signature, hash::HashValue};
use libra_crypto_derive::CryptoHasher;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Hash, Eq, PartialEq, Serialize, Deserialize, CryptoHasher)]
pub struct WitnessData {
    channel_sequence_number: u64,
    write_set: WriteSet,
}
impl WitnessData {
    pub fn new(channel_sequence_number: u64, write_set: WriteSet) -> Self {
        Self {
            channel_sequence_number,
            write_set,
        }
    }

    pub fn channel_sequence_number(&self) -> u64 {
        self.channel_sequence_number
    }

    pub fn write_set(&self) -> &WriteSet {
        &self.write_set
    }
}

impl CryptoHash for WitnessData {
    type Hasher = WitnessDataHasher;

    fn hash(&self) -> HashValue {
        let mut state = Self::Hasher::default();
        libra_crypto::hash::CryptoHasher::write(
            &mut state,
            &lcs::to_bytes(self).expect("Serialization should work."),
        );
        libra_crypto::hash::CryptoHasher::finish(state)
    }
}

#[derive(Clone, Debug, Default, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct Witness {
    data: WitnessData,
    /// Channel participant's signatures.
    signatures: Vec<Ed25519Signature>,
}

impl Witness {
    pub fn new(
        channel_sequence_number: u64,
        write_set: WriteSet,
        signatures: Vec<Ed25519Signature>,
    ) -> Self {
        Self {
            data: WitnessData {
                channel_sequence_number,
                write_set,
            },
            signatures,
        }
    }

    pub fn channel_sequence_number(&self) -> u64 {
        self.data.channel_sequence_number
    }

    pub fn write_set(&self) -> &WriteSet {
        &self.data.write_set
    }

    pub fn signatures(&self) -> &[Ed25519Signature] {
        self.signatures.as_slice()
    }
}
