// Copyright 2020 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

#![cfg(feature = "submodule_tests")]

pub mod rand_replay;

use address::{Address, Protocol};
use blockstore::BlockStore;
use cid::Cid;
use clock::ChainEpoch;
use crypto::Signature;
use db::MemoryDB;
use forest_message::{ChainMessage, Message, MessageReceipt, SignedMessage, UnsignedMessage};
use interpreter::{CircSupplyCalc, LookbackStateGetter};
use rand_replay::*;
use serde::{Deserialize, Deserializer, Serialize};
use vm::{ExitCode, Serialized, TokenAmount};

use std::error::Error;

mod base64_bytes {
    use super::*;
    use serde::de;
    use std::borrow::Cow;

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: Cow<'de, str> = Deserialize::deserialize(deserializer)?;
        base64::decode(s.as_ref()).map_err(de::Error::custom)
    }

    pub mod vec {
        use super::*;

        pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<Vec<u8>>, D::Error>
        where
            D: Deserializer<'de>,
        {
            let v: Vec<Cow<'de, str>> = Deserialize::deserialize(deserializer)?;
            v.into_iter()
                .map(|s| base64::decode(s.as_ref()))
                .collect::<Result<Vec<_>, _>>()
                .map_err(de::Error::custom)
        }
    }
}

mod message_receipt_vec {
    use super::*;

    #[derive(Deserialize)]
    pub struct MessageReceiptVector {
        exit_code: ExitCode,
        #[serde(rename = "return", with = "base64_bytes")]
        return_value: Vec<u8>,
        gas_used: i64,
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<MessageReceipt>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: Vec<MessageReceiptVector> = Deserialize::deserialize(deserializer)?;
        Ok(s.into_iter()
            .map(|v| MessageReceipt {
                exit_code: v.exit_code,
                return_data: Serialized::new(v.return_value),
                gas_used: v.gas_used,
            })
            .collect())
    }
}

pub struct MockCircSupply(pub TokenAmount);
impl CircSupplyCalc for MockCircSupply {
    fn get_supply<DB: BlockStore>(
        &self,
        _: ChainEpoch,
        _: &state_tree::StateTree<DB>,
    ) -> Result<TokenAmount, Box<dyn Error>> {
        Ok(self.0.clone())
    }
}

pub struct MockStateLB<'db, MemoryDB>(pub &'db MemoryDB);
impl<'db> LookbackStateGetter<'db, MemoryDB> for MockStateLB<'db, MemoryDB> {
    fn state_lookback(
        &self,
        _: ChainEpoch,
    ) -> Result<state_tree::StateTree<'db, MemoryDB>, Box<dyn Error>> {
        Err("Lotus runner doesn't seem to initialize this?".into())
    }
}

pub struct ExecuteMessageParams<'a> {
    pub pre_root: &'a Cid,
    pub epoch: ChainEpoch,
    pub msg: &'a ChainMessage,
    pub circ_supply: TokenAmount,
    pub basefee: TokenAmount,
    pub randomness: ReplayingRand<'a>,
}

/// Encoded VM randomness used to be replayed
pub type Randomness = Vec<RandomnessMatch>;

#[derive(Debug, Deserialize)]
pub struct TestVector {
    pub class: String,
    #[serde(rename = "_meta")]
    pub meta: Metadata,
    #[serde(with = "base64_bytes")]
    pub car: Vec<u8>,
    pub preconditions: PreConditions,
    pub apply_messages: Vec<ApplyMessage>,
    pub postconditions: PostConditions,
    #[serde(default)]
    pub randomness: Randomness,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ApplyMessage {
    #[serde(with = "base64_bytes")]
    pub bytes: Vec<u8>,
}

#[derive(Debug, Deserialize)]
pub struct Selector {
    #[serde(default)]
    pub puppet_actor: Option<String>,
    #[serde(default)]
    pub chaos_actor: Option<String>,
    #[serde(default)]
    pub min_protocol_version: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Metadata {
    pub id: String,
    pub gen: Vec<GenData>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GenData {
    pub source: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PreConditions {
    pub variants: Vec<Variant>,
    pub state_tree: StateTree,
    pub base_fee: Option<f64>,
    pub circ_supply: Option<f64>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PostConditions {
    pub state_tree: StateTree,
    #[serde(with = "message_receipt_vec")]
    pub receipts: Vec<MessageReceipt>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct StateTree {
    #[serde(with = "cid::json")]
    pub root_cid: Cid,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Variant {
    pub id: String,
    pub epoch: ChainEpoch,
    pub nv: u32,
}

pub fn to_chain_msg(msg: UnsignedMessage) -> ChainMessage {
    if msg.from().protocol() == Protocol::Secp256k1 {
        ChainMessage::Signed(SignedMessage {
            message: msg,
            signature: Signature::new_secp256k1(vec![0, 65]),
        })
    } else {
        ChainMessage::Unsigned(msg)
    }
}