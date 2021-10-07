use crate::{
    ActorMigration, ActorMigrationInput, MigrationError, MigrationOutput, MigrationResult,
};
use cid::Code::Blake2b256;
use ipld_blockstore::BlockStore;
use std::sync::Arc;

use cid::Cid;

use actor_interface::actorv2::power::Claim as Power2Claim;
use actor_interface::actorv2::power::State as Power2State;
use actor_interface::actorv3::power::Claim as Power3Claim;
use actor_interface::actorv3::power::State as Power3State;
use actor_interface::actorv3::POWER_ACTOR_CODE_ID;
use actor_interface::ActorVersion;
use actor_interface::Map;

pub struct PowerMigrator(Cid);

impl<BS: BlockStore + Send + Sync> ActorMigration<BS> for PowerMigrator {
    fn migrate_state(
        &self,
        store: Arc<BS>,
        input: ActorMigrationInput,
    ) -> MigrationResult<MigrationOutput> {
        let in_state: Power2State = store
            .get(&input.head)
            .map_err(|e| MigrationError::BlockStoreRead(e.to_string()))?
            .ok_or_else(|| {
                MigrationError::BlockStoreRead("Power actor: could not read v2 state".to_string())
            })?;

        let mut proof_validation_batch = None;
        if in_state.proof_validation_batch.is_none() {
            let proof_validation_batch_out =
                migrate_hamt_amt_raw(store.as_ref(), in_state.proof_validation_batch, 5, 4)?;
            proof_validation_batch = Some(proof_validation_batch_out);
        }

        let claims = migrate_claims(store.as_ref(), in_state.claims)?;

        let cron_event_queue =
            migrate_hamt_amt_raw(store.as_ref(), in_state.cron_event_queue, 6, 6)?;

        let out_state = Power3State {
            total_raw_byte_power: in_state.total_raw_byte_power,
            total_bytes_committed: in_state.total_bytes_committed,
            total_quality_adj_power: in_state.total_quality_adj_power,
            total_qa_bytes_committed: in_state.total_qa_bytes_committed,
            total_pledge_collateral: in_state.total_pledge_collateral,
            this_epoch_raw_byte_power: in_state.this_epoch_raw_byte_power,
            this_epoch_quality_adj_power: in_state.this_epoch_quality_adj_power,
            this_epoch_pledge_collateral: in_state.this_epoch_pledge_collateral,
            this_epoch_qa_power_smoothed: todo!(), // smoothing3
            miner_count: in_state.miner_count,
            miner_above_min_power_count: in_state.miner_above_min_power_count,
            cron_event_queue,
            first_cron_epoch: in_state.first_cron_epoch,
            claims,
            proof_validation_batch,
        };

        let new_head = store
            .put(&out_state, Blake2b256)
            .map_err(|e| MigrationError::BlockStoreWrite(e.to_string()))?;

        Ok(MigrationOutput {
            new_code_cid: *POWER_ACTOR_CODE_ID,
            new_head,
        });
    }
}

fn migrate_claims<BS: BlockStore + Send + Sync>(
    store: &BS,
    root: Cid,
) -> Result<Cid, MigrationError> {
    let in_claims: Map<BS, Cid> = Map::load(&root, store, ActorVersion::V2).map_err(|_| {
        MigrationError::BlockStoreRead("Could not load Power map from root".to_string())
    })?;

    let mut out_claims: Map<BS, Power3Claim> = Map::new(store, ActorVersion::V3);

    let _ = in_claims.for_each(|k, v| {
        let in_claim: Option<Power2Claim> = store.get(v).map_err(|e| {
            MigrationError::BlockStoreRead("Could not load claim from blockstore".to_string())
        })?;

        let in_claim = match in_claim {
            Some(claim) => claim,
            None => return Ok(()),
        };

        let post_proof = in_claim
            .seal_proof_type
            .registered_window_post_proof()
            .map_err(|e| MigrationError::Other)?;

        let out_claim = Power3Claim {
            window_post_proof_type: post_proof,
            raw_byte_power: in_claim.raw_byte_power,
            quality_adj_power: in_claim.quality_adj_power,
        };
        out_claims.set(k.to_owned(), out_claim)
    });

    out_claims
        .flush()
        .map_err(|e| MigrationError::BlockStoreWrite(e.to_string()))
}
