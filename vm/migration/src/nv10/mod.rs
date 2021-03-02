mod init_actor;
mod util;
use async_std::sync::Arc;
use std::{collections::HashMap, error::Error as StdError};

use address::Address;
use cid::Cid;
use clock::ChainEpoch;
use ipld_blockstore::BlockStore;

use state_tree::StateTree;
use vm::ActorState;

use crate::{ActorMigrationInput, Migrator};
use futures::stream::{self, StreamExt};

struct MigrationJob {
    address: Address,
    act_state: ActorState,
    // actor_migration: Arc<Box<dyn Migrator<BS>>>,
    // cache MigrationCache
}
struct MigrationJobResult {
    address: Address,
    act_state: ActorState,
}

pub async fn migrate_state_tree<BS: BlockStore>(
    store: &BS,
    state_root_in: Cid,
    prior_epoch: ChainEpoch,
) -> Result<Cid, Box<dyn StdError>> {
    let mut migrations: HashMap<Cid, Arc<Box<dyn Migrator<BS>>>> = HashMap::new();
    let actors_in = migrations.insert(
        *actor::actorv2::INIT_ACTOR_CODE_ID,
        Arc::new(Box::new(init_actor::InitMigrator {})),
    );

    // Load input and output StateTrees
    let state_in = StateTree::new_from_root(store, &state_root_in)?;
    let mut state_out = StateTree::new(store, fil_types::StateTreeVersion::V2)?;

    // TODO: Need to do this concurrently. Will require modifications of the Hamt...
    state_in.for_each(|addr, act| {
        let job = MigrationJob {
            address: addr,
            act_state: act.clone(),
        };
        let migrate_fn = migrations.get(&act.code).unwrap();
        let out = migrate_fn.migrate_state(
            store,
            ActorMigrationInput {
                address: addr,
                balance: act.balance.clone(),
                head: act.state,
                prior_epoch: prior_epoch,
            },
        )?;
        state_out.set_actor(
            &addr,
            ActorState {
                code: out.new_code_cid,
                state: out.new_head,
                sequence: act.sequence,
                balance: act.balance.clone(),
            },
        )?;
        Ok(())
    })?;

    Ok(state_out.flush()?)
}
