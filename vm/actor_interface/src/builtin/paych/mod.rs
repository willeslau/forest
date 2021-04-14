use actorv0::{Serialized, TokenAmount};
use address::Address;
use clock::ChainEpoch;
use encoding::to_vec;
use forest_message::UnsignedMessage;
use ipld_blockstore::BlockStore;
use num_bigint::BigInt;
use serde::{Deserialize, Serialize};
use std::error::Error;
use vm::ActorState;
use crypto::Signature;
use crate::ActorVersion;

/// Paych actor method.
pub type Method = actorv3::paych::Method;

// /// Re-exports from the actors crate.
// pub type SignedVoucher = actorv0::paych::SignedVoucher;
// pub type ModVerifyParams = actorv0::paych::ModVerifyParams;
pub type UpdateChannelStateParams = actorv3::paych::UpdateChannelStateParams;
/// Paych actor state.
#[derive(Serialize)]
#[serde(untagged)]
pub enum State {
    V0(actorv0::paych::State),
    V2(actorv2::paych::State),
    V3(actorv2::paych::State),
}

impl State {
    pub fn load<BS>(store: &BS, actor: &ActorState) -> Result<State, Box<dyn Error>>
    where
        BS: BlockStore,
    {
        if actor.code == *actorv0::PAYCH_ACTOR_CODE_ID {
            Ok(store
                .get(&actor.state)?
                .map(State::V0)
                .ok_or("Actor state doesn't exist in store")?)
        } else if actor.code == *actorv2::PAYCH_ACTOR_CODE_ID {
            Ok(store
                .get(&actor.state)?
                .map(State::V2)
                .ok_or("Actor state doesn't exist in store")?)
        } else if actor.code == *actorv3::PAYCH_ACTOR_CODE_ID {
            Ok(store
                .get(&actor.state)?
                .map(State::V3)
                .ok_or("Actor state doesn't exist in store")?)
        } else {
            Err(format!("Unknown actor code {}", actor.code).into())
        }
    }

    pub fn from(&self) -> Address {
        match self {
            State::V0(st) => st.from,
            State::V2(st) => st.from,
            State::V3(st) => st.from,
        }
    }
    pub fn to(&self) -> Address {
        match self {
            State::V0(st) => st.to,
            State::V2(st) => st.to,
            State::V3(st) => st.to,
        }
    }
    pub fn settling_at(&self) -> ChainEpoch {
        match self {
            State::V0(st) => st.settling_at,
            State::V2(st) => st.settling_at,
            State::V3(st) => st.settling_at,
        }
    }
    pub fn to_send(&self) -> &TokenAmount {
        match self {
            State::V0(st) => &st.to_send,
            State::V2(st) => &st.to_send,
            State::V3(st) => &st.to_send,
        }
    }
    fn get_or_load_ls_amt<'a, BS: BlockStore>(
        &self,
        bs: &'a BS,
    ) -> Result<crate::adt::Array<'a, BS, LaneState>, Box<dyn Error>> {
        Ok(match self {
            State::V0(st) => crate::adt::Array::load(&st.lane_states, bs, ActorVersion::V0)?,
            State::V2(st) => crate::adt::Array::load(&st.lane_states, bs, ActorVersion::V2)?,
            State::V3(st) => crate::adt::Array::load(&st.lane_states, bs, ActorVersion::V3)?,
        })
    }
    pub fn lane_count<'a, BS: BlockStore>(&self, bs: &'a BS) -> Result<u64, Box<dyn Error>> {
        let lsamt = self.get_or_load_ls_amt(bs)?;

        Ok(lsamt.count())
    }

    pub fn for_each_lane_state<BS: BlockStore>(
        &self,
        bs: &BS,
        mut f: impl FnMut(u64, &LaneState) -> Result<(), Box<dyn Error>>,
    ) -> Result<(), Box<dyn Error>> {
        let lsamt = self.get_or_load_ls_amt(bs)?;
        lsamt.for_each(|idx, ls| f(idx, ls))
    }
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(untagged)]
pub enum LaneState {
    V0(actorv0::paych::LaneState),
    V2(actorv2::paych::LaneState),
    V3(actorv3::paych::LaneState),
}

impl LaneState {
    pub fn new(redeemed: BigInt, nonce: u64) -> Self {
        // TODO: Not sure if/how we want to version this. Shouldnt matter because the fields havent changed.
        Self::V3(actorv3::paych::LaneState {
            redeemed,
            nonce,
        })
    }
    pub fn redeemed(&self) -> &BigInt {
        match self {
            LaneState::V0(ls) => &ls.redeemed,
            LaneState::V2(ls) => &ls.redeemed,
            LaneState::V3(ls) => &ls.redeemed,
        }
    }
    pub fn nonce(&self) -> u64 {
        match self {
            LaneState::V0(ls) => ls.nonce,
            LaneState::V2(ls) => ls.nonce,
            LaneState::V3(ls) => ls.nonce,
        }
    }
    pub fn set_nonce(&mut self, nonce: u64) {
        match self {
            LaneState::V0(ls) => ls.nonce = nonce,
            LaneState::V2(ls) => ls.nonce = nonce,
            LaneState::V3(ls) => ls.nonce = nonce,
        }
    }
    pub fn set_redeemed(&mut self, redeemed: BigInt) {
        match self {
            LaneState::V0(ls) => ls.redeemed = redeemed,
            LaneState::V2(ls) => ls.redeemed = redeemed,
            LaneState::V3(ls) => ls.redeemed = redeemed,
        }
    }
}
#[derive(Deserialize, Serialize, Clone)]
pub struct Merge {
    pub lane: u64,
    pub nonce: u64,
}
#[derive(Debug, Clone, PartialEq)]
pub struct ModVerifyParams {
    pub actor: Address,
    pub method: u64,
    pub data: Serialized,
}

#[derive(Deserialize, Serialize, Clone, PartialEq)]
#[serde(untagged)]
pub enum SignedVoucher {
    V0(actorv0::paych::SignedVoucher),
    V2(actorv2::paych::SignedVoucher),
    V3(actorv3::paych::SignedVoucher),
}
impl SignedVoucher {
    pub fn merges(&self) -> Vec<Merge> {
        let merges = match self {
            SignedVoucher::V0(sv) => {
                sv.merges.iter().map(|m| {
                    Merge {
                        lane: m.lane,
                        nonce:m.nonce
                    }
                }).collect()
            }
            SignedVoucher::V2(sv) => {         
                sv.merges.iter().map(|m| {
                    Merge {
                        lane: m.lane,
                        nonce:m.nonce
                    }
                }).collect()
            }
            SignedVoucher::V3(sv) => {                
                sv.merges.iter().map(|m| {
                Merge {
                    lane: m.lane as u64,
                    nonce:m.nonce
                }
            }).collect()}
        };
        merges
        
    }
    pub fn lane(&self) -> usize {
        match self {
            SignedVoucher::V0(sv) => sv.lane as usize,
            SignedVoucher::V2(sv) => sv.lane as usize,
            SignedVoucher::V3(sv) => sv.lane,
        }
    }
    pub fn nonce(&self) -> u64 {
        match self {
            SignedVoucher::V0(sv) => sv.nonce,
            SignedVoucher::V2(sv) => sv.nonce,
            SignedVoucher::V3(sv) => sv.nonce,
        }
    }
    pub fn amount(&self) -> &BigInt {
        match self {
            SignedVoucher::V0(sv) => &sv.amount,
            SignedVoucher::V2(sv) => &sv.amount,
            SignedVoucher::V3(sv) => &sv.amount,
        }
    }
    pub fn signature(&self) -> Option<Signature> {
        match self {
            SignedVoucher::V0(sv) => sv.signature.clone(),
            SignedVoucher::V2(sv) => sv.signature.clone(),
            SignedVoucher::V3(sv) => sv.signature.clone(),
        }
    }
    pub fn set_signature(&mut self, sig: Signature) {
        match self {
            SignedVoucher::V0(sv) => sv.signature = Some(sig),
            SignedVoucher::V2(sv) => sv.signature = Some(sig),
            SignedVoucher::V3(sv) => sv.signature = Some(sig),
        }
    }
    pub fn set_nonce(&mut self, nonce: u64) {
        match self {
            SignedVoucher::V0(sv) => sv.nonce = nonce,
            SignedVoucher::V2(sv) => sv.nonce = nonce,
            SignedVoucher::V3(sv) => sv.nonce = nonce,
        }
    }
    pub fn signing_bytes(&self) -> Result<Vec<u8>, Box<dyn Error>> {
        Ok(match self {
            SignedVoucher::V0(sv) => sv.signing_bytes()?,
            SignedVoucher::V2(sv) => sv.signing_bytes()?,
            SignedVoucher::V3(sv) => sv.signing_bytes()?,
        })
    }
    pub fn channel_addr(&self) -> Address {
        match self {
            SignedVoucher::V0(sv) => sv.channel_addr,
            SignedVoucher::V2(sv) => sv.channel_addr,
            SignedVoucher::V3(sv) => sv.channel_addr,
        }
    }
    pub fn set_channel_addr(&mut self, addr: Address) {
        match self {
            SignedVoucher::V0(sv) => sv.channel_addr = addr,
            SignedVoucher::V2(sv) => sv.channel_addr= addr,
            SignedVoucher::V3(sv) => sv.channel_addr= addr,
        }
    }
    pub fn extra(&self) -> Option<ModVerifyParams>{
        match self {
            SignedVoucher::V0(sv) => {sv.extra.as_ref().map(|mvp| ModVerifyParams {
                actor: mvp.actor,
                method: mvp.method as u64,
                data: mvp.data.clone(),
                
            })}
            SignedVoucher::V2(sv) => {sv.extra.as_ref().map(|mvp| ModVerifyParams {
                actor: mvp.actor,
                method: mvp.method as u64,
                data: mvp.data.clone(),
                
            })}
            SignedVoucher::V3(sv) => {sv.extra.as_ref().map(|mvp| ModVerifyParams {
                actor: mvp.actor,
                method: mvp.method as u64,
                data: mvp.data.clone(),
                
            })}
        }
    }
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum Message {
    V0(MessageS),
    V2(MessageS),
    V3(MessageS),
}

#[derive(Serialize)]
pub struct MessageS {
    from: Address,
}
impl Message {
    pub fn new(nv: ActorVersion, from: Address) -> Self {
        match nv {
            ActorVersion::V0 => {
                Self::V0(MessageS{from})
            }
            ActorVersion::V2 => {
                Self::V2(MessageS{from})
            }
            ActorVersion::V3 => {
                Self::V3(MessageS{from})

            }
        }
    }
    pub fn create(
        &self,
        to: Address,
        initial_amount: TokenAmount,
    ) -> Result<UnsignedMessage, Box<dyn Error>> {
        let (from, to, method_num) = match self {
            Message::V0(message) => (
                message.from,
                *actorv0::INIT_ACTOR_ADDR,
                actorv0::init::Method::Exec as u64,
            ),
            Message::V2(message) => (
                message.from,
                *actorv2::INIT_ACTOR_ADDR,
                actorv2::init::Method::Exec as u64,
            ),
            Message::V3(message) => (
                message.from,
                *actorv3::INIT_ACTOR_ADDR,
                actorv3::init::Method::Exec as u64,
            ),
        };
        let params = actorv0::paych::ConstructorParams { from: from, to: to };
        let params = vm::Serialized::serialize(params)?;
        let ret = UnsignedMessage::builder()
            .to(to)
            .from(from)
            .value(initial_amount)
            .method_num(method_num)
            .params(params)
            .build()?;
        Ok(ret)
    }

    pub fn update(
        &self,
        paych: Address,
        sv: SignedVoucher,
        secret: &[u8],
    ) -> Result<UnsignedMessage, Box<dyn Error>> {
        let (method_num, params, from) = match self {
            Message::V0(message) => {
                let params = if let SignedVoucher::V0(sv) = sv {
                    vm::Serialized::serialize(actorv0::paych::UpdateChannelStateParams {
                        sv,
                        secret: secret.to_vec(),
                        proof: vec![],
                    })
                } else {
                    return Err("Paych SignedVoucher wrong version. Expected V0".into());
                }?;
                (
                    actorv0::paych::Method::UpdateChannelState as u64,
                    params,
                    message.from,
                )
            }
            Message::V2(message) => {
                let params = if let SignedVoucher::V2(sv) = sv {
                    vm::Serialized::serialize(actorv2::paych::UpdateChannelStateParams {
                        sv,
                        secret: secret.to_vec(),
                    })
                } else {
                    return Err("Paych SignedVoucher wrong version. Expected V2".into());
                }?;
                (
                    actorv2::paych::Method::UpdateChannelState as u64,
                    params,
                    message.from,
                )
            }
            Message::V3(message) => {
                let params = if let SignedVoucher::V3(sv) = sv {
                    vm::Serialized::serialize(actorv3::paych::UpdateChannelStateParams {
                        sv,
                        secret: secret.to_vec(),
                    })
                } else {
                    return Err("Paych SignedVoucher wrong version. Expected V3".into());
                }?;
                (
                    actorv3::paych::Method::UpdateChannelState as u64,
                    params,
                    message.from,
                )
            }
        };

        let ret = UnsignedMessage::builder()
            .to(paych)
            .from(from)
            .value(0.into())
            .method_num(method_num)
            .params(params)
            .build()?;
        Ok(ret)
    }

    pub fn settle(&self, paych: Address) -> Result<UnsignedMessage, Box<dyn Error>> {
        let (from, method_num) = match self {
            Message::V0(message) => (message.from, actorv0::paych::Method::Settle as u64),
            Message::V2(message) => (message.from, actorv2::paych::Method::Settle as u64),
            Message::V3(message) => (message.from, actorv3::paych::Method::Settle as u64),
        };
        let ret = UnsignedMessage::builder()
            .to(paych)
            .from(from)
            .value(0.into())
            .method_num(method_num)
            .build()?;
        Ok(ret)
    }

    pub fn collect(&self, paych: Address) -> Result<UnsignedMessage, Box<dyn Error>> {
        let (from, method_num) = match self {
            Message::V0(message) => (message.from, actorv0::paych::Method::Collect as u64),
            Message::V2(message) => (message.from, actorv2::paych::Method::Collect as u64),
            Message::V3(message) => (message.from, actorv3::paych::Method::Collect as u64),
        };
        let ret = UnsignedMessage::builder()
            .to(paych)
            .from(from)
            .value(0.into())
            .method_num(method_num)
            .build()?;
        Ok(ret)
    }
}
