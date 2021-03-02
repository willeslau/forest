use std::error::Error as StdError;

use cid::{Cid, Code::Blake2b256};
use fil_types::HAMT_BIT_WIDTH;
use ipld_blockstore::BlockStore;

use crate::{ActorMigrationInput, ActorMigrationOutput, Migrator};

use super::util::migrate_hamt_raw;
use actor::actorv2::init::State as StateV2;
use actor::actorv3::{init::State as StateV3, INIT_ACTOR_CODE_ID};
pub struct InitMigrator {}

impl Migrator for InitMigrator {
    fn migrate_state<BS: BlockStore>(
        &self,
        store: &BS,
        input: ActorMigrationInput,
    ) -> Result<ActorMigrationOutput, Box<dyn StdError>> {
        let in_state: StateV2 = store
            .get(&input.head)?
            .ok_or_else(|| "Failed to get Init state v2".to_owned())?;

        let address_map_out =
            migrate_hamt_raw::<_, u64>(store, &in_state.address_map, HAMT_BIT_WIDTH)?;
        let out_state = StateV3 {
            address_map: address_map_out,
            next_id: in_state.next_id,
            network_name: in_state.network_name,
        };
        let new_head = store.put(&out_state, Blake2b256)?;
        Ok(ActorMigrationOutput {
            new_code_cid: *INIT_ACTOR_CODE_ID,
            new_head: new_head,
        })
    }
}
