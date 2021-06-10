
use actor_interface::actorv2;

use crate::ActorMigration;
use ipld_blockstore::BlockStore;
use async_std::sync::Arc;

use crate::{ActorMigrationInput, MigrationResult, MigrationOutput};

struct VerifregMigrator {

}

// each actor's state migration is read from blockstore, changes state tree, and writes back to the blocstore.
impl<BS: BlockStore + Send + Sync> ActorMigration<BS> for VerifregMigrator {
    fn migrate_state(
        &self,
        store: Arc<BS>,
        input: ActorMigrationInput,
    ) -> MigrationResult<MigrationOutput> {
        
        todo!();
    }
}