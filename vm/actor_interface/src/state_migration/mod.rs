use std::error::Error as StdError;

use address::Address;
use cid::Cid;
use clock::ChainEpoch;
use ipld_blockstore::BlockStore;
use vm::TokenAmount;

pub mod nv10;

pub struct ActorMigrationInput {
    pub address: Address,
    pub balance: TokenAmount,
    pub head: Cid,
    pub prior_epoch: ChainEpoch,
}

pub struct ActorMigrationOutput {
    pub new_code_cid: Cid,
    pub new_head: Cid,
}

pub trait Migrator {
    fn migrate_state<BS: BlockStore>(
        &self,
        store: &BS,
        input: ActorMigrationInput,
    ) -> Result<ActorMigrationOutput, Box<dyn StdError>>;
}
