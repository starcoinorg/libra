//! transaction accumulator store with hash

use schemadb::{
    define_schema,
    schema::{KeyCodec, ValueCodec},
};
use libra_crypto::HashValue;
use failure::prelude::*;

//(pre block id, txn id)
type TxnAccumulatorHashKey = (HashValue, HashValue);

define_schema!(
    TxnAccumulatorSchema,
    TxnAccumulatorHashKey,
    HashValue,
    TXN_ACCUMULATOR_HASH_CF_NAME
);

impl KeyCodec<TxnAccumulatorSchema> for TxnAccumulatorHashKey {
    fn encode_key(&self) -> Result<Vec<u8>> {
        unimplemented!()
    }

    fn decode_key(data: &[u8]) -> Result<Self> {
        unimplemented!()
    }
}

impl ValueCodec<TxnAccumulatorSchema> for HashValue {
    fn encode_value(&self) -> Result<Vec<u8>> {
        Ok(self.to_vec())
    }

    fn decode_value(data: &[u8]) -> Result<Self> {
        Self::from_slice(data)
    }
}

