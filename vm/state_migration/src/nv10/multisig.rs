use crate::{
    ActorMigration, ActorMigrationInput, MigrationError, MigrationOutput, MigrationResult,
};

use actor_interface::actorv2::multisig::State as V2State;
use actor_interface::actorv3::multisig::State as V3State;
use actor_interface::actorv3::multisig::TxnID as TxnIdV3;
use actor_interface::actorv3::MULTISIG_ACTOR_CODE_ID;

use cid::{Cid, Code::Blake2b256};
use ipld_blockstore::BlockStore;
use std::sync::Arc;

use super::migrate_hamt_raw;

pub struct MultisigMigrator(Cid);

impl<BS: BlockStore + Send + Sync> ActorMigration<BS> for MultisigMigrator {
    fn migrate_state(
        &self,
        store: Arc<BS>,
        input: ActorMigrationInput,
    ) -> MigrationResult<MigrationOutput> {
        let in_state: V2State = store
            .get(&input.head)
            .map_err(|e| MigrationError::BlockStoreRead(e.to_string()))?
            .ok_or_else(|| {
                MigrationError::BlockStoreRead(
                    "Multisig actor: could not read v2 state".to_string(),
                )
            })?;

        let pending_txs = migrate_hamt_raw(store.as_ref(), &in_state.pending_txs, 5)?;

        let out_state = V3State {
            signers: in_state.signers,
            num_approvals_threshold: in_state.num_approvals_threshold,
            next_tx_id: TxnIdV3(in_state.next_tx_id.0),
            initial_balance: in_state.initial_balance,
            start_epoch: in_state.start_epoch,
            unlock_duration: in_state.unlock_duration,
            pending_txs,
        };

        let new_head = store
            .put(&out_state, Blake2b256)
            .map_err(|e| MigrationError::BlockStoreWrite(e.to_string()))?;

        Ok(MigrationOutput {
            new_code_cid: *MULTISIG_ACTOR_CODE_ID,
            new_head,
        })
    }
}
