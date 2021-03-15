use ipld_blockstore::BlockStore;
use serde::Serialize;
use std::error::Error;
use vm::ActorState;

/// Paych actor method.
pub type Method = actorv3::paych::Method;

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
