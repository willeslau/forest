use std::sync::Arc;

use ipld_blockstore::BlockStore;

use crate::{ActorMigration, ActorMigrationInput, MigrationOutput, MigrationResult};

use super::market::MarketMigrator;

pub struct PowerMigrator(Cid);

impl<BS: BlockStore + Send + Sync> ActorMigration<BS> for MarketMigrator {
    fn migrate_state(
        &self,
        store: Arc<BS>,
        input: ActorMigrationInput,
    ) -> MigrationResult<MigrationOutput> {
        todo!()
    }
}

fn migrate_claims<BS: BlockStore + Send + Sync>(
    store: &BS,
    root: Cid,
) -> Result<Cid, MigrationError> {
    todo!()
}
