use actorv0::TokenAmount;
use address::Address;
use encoding::to_vec;
use forest_message::UnsignedMessage;
use ipld_blockstore::BlockStore;
use serde::Serialize;
use std::error::Error;
use vm::ActorState;

/// Paych actor method.
pub type Method = actorv3::paych::Method;

// /// Re-exports from the actors crate.
// pub type SignedVoucher = actorv0::paych::SignedVoucher;
// pub type ModVerifyParams = actorv0::paych::ModVerifyParams;

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
}
#[derive(Serialize)]
#[serde(untagged)]
pub enum SignedVoucher {
    V0(actorv0::paych::SignedVoucher),
    V2(actorv2::paych::SignedVoucher),
    V3(actorv3::paych::SignedVoucher),
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
