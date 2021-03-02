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

pub trait Migrator<BS: BlockStore> {
    fn migrate_state(
        &self,
        store: &BS,
        input: ActorMigrationInput,
    ) -> Result<ActorMigrationOutput, Box<dyn StdError>>;
}
