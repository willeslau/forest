mod init_actor;
mod util;
use std::{collections::HashMap, error::Error as StdError};

use cid::Cid;
use clock::ChainEpoch;
use ipld_blockstore::BlockStore;

use state_tree::StateTree;

use crate::Migrator;

pub struct ActorMigrations<BS:BlockStore> {
    migration: Box<dyn Migrator<BS>>
}

pub fn migrate_state_tree<BS: BlockStore>(
    store: &BS,
    state_root_in: Cid,
    prior_epoch: ChainEpoch,
) -> Result<Cid, Box<dyn StdError>>{
    let mut migrations: HashMap<Cid, Box<dyn Migrator<BS>>> = HashMap::new();

    
    migrations.insert(*actor::actorv2::INIT_ACTOR_CODE_ID, Box::new(init_actor::InitMigrator{}));
    

    // Load input and output StateTrees
    let state_in = StateTree::new_from_root(store, &state_root_in)?;
    let state_out = StateTree::new(store, fil_types::StateTreeVersion::V2)?;


    todo!()
}
