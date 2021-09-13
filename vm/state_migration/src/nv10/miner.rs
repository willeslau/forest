use std::sync::Arc;

use actor_interface::actorv2::miner::Deadlines as Miner2Deadlines;
use actor_interface::actorv2::miner::MinerInfo as MinerInfo2;
use actor_interface::actorv2::miner::State as V2State;
use actor_interface::actorv3::miner::Deadlines as Miner3Deadlines;
use actor_interface::actorv3::miner::MinerInfo as MinerInfo3;
use actor_interface::actorv3::miner::State as V3State;
use actor_interface::actorv3::miner::WorkerKeyChange;
use actor_interface::{ActorVersion, Array};
use actorv2::miner::Partition as PartitionV2;
use actorv2::miner::PARTITION_EARLY_TERMINATION_ARRAY_AMT_BITWIDTH;
use actorv2::miner::PARTITION_EXPIRATION_AMT_BITWIDTH;
use actorv3::miner::Deadline;
use actorv3::miner::Partition as PartitionV3;
use actorv3::miner::PowerPair as MinerV3PowerPair;
use cid::{Cid, Code::Blake2b256};
use ipld_blockstore::BlockStore;

use crate::ActorMigrationInput;
use crate::{ActorMigration, MigrationError, MigrationOutput, MigrationResult};

use super::migrate_amt_raw;

pub struct MinerMigrator(Cid);

pub fn miner_migrator_v3<BS: BlockStore + Send + Sync>(
    cid: Cid,
) -> Arc<dyn ActorMigration<BS> + Send + Sync> {
    Arc::new(MinerMigrator(cid))
}

impl<BS: BlockStore + Send + Sync> ActorMigration<BS> for MinerMigrator {
    fn migrate_state(
        &self,
        store: Arc<BS>,
        input: ActorMigrationInput,
    ) -> MigrationResult<MigrationOutput> {
        let in_state: V2State = store
            .get(&input.head)
            .map_err(|e| MigrationError::BlockStoreRead(e.to_string()))?
            .ok_or_else(|| {
                MigrationError::BlockStoreRead("Miner actor: could not read v3 state".to_string())
            })?;

        let store_ref = store.as_ref();

        let info = migrate_info(store_ref, in_state.info)?;

        // let pre_committed_sectors_out =
        //     migrate_hamt_raw(store, in_state.pre_committed_sectors_expiry);

        let deadlines = migrate_deadlines(store_ref, in_state.deadlines)?;

        let out_state = V3State {
            info,
            pre_commit_deposits: in_state.pre_commit_deposits,
            locked_funds: in_state.locked_funds,
            vesting_funds: in_state.vesting_funds,
            fee_debt: in_state.fee_debt,
            initial_pledge: in_state.initial_pledge,
            pre_committed_sectors: in_state.pre_committed_sectors,
            pre_committed_sectors_expiry: in_state.pre_committed_sectors_expiry,
            allocated_sectors: in_state.allocated_sectors,
            sectors: in_state.sectors,
            proving_period_start: in_state.proving_period_start,
            current_deadline: in_state.current_deadline as usize,
            deadlines,
            early_terminations: in_state.early_terminations,
        };

        let new_head = store
            .put(&out_state, Blake2b256)
            .map_err(|e| MigrationError::BlockStoreWrite(e.to_string()))?;

        Ok(MigrationOutput {
            new_code_cid: self.0,
            new_head,
        })
    }
}

fn migrate_info<BS: BlockStore + Send + Sync>(
    store: &BS,
    info: Cid,
) -> Result<Cid, MigrationError> {
    let old_info: Option<MinerInfo2> = store
        .get(&info)
        .map_err(|e| MigrationError::BlockStoreRead(e.to_string()))?;

    if old_info.is_none() {
        return Err(MigrationError::BlockStoreRead(format!(
            "can't find {} in blockstore",
            info
        )));
    }

    let old_info = old_info.unwrap();

    let pending_worker_key = if let Some(worker_key) = old_info.pending_worker_key {
        Some(WorkerKeyChange {
            new_worker: worker_key.new_worker,
            effective_at: worker_key.effective_at,
        })
    } else {
        None
    };

    let window_post_proof_type = old_info
        .seal_proof_type
        .registered_window_post_proof()
        .map_err(|_| MigrationError::BlockStoreRead("Can't get window PoST proof".to_string()))?;

    let new_info = MinerInfo3 {
        owner: old_info.owner,
        worker: old_info.worker,
        control_addresses: old_info.control_addresses,
        pending_worker_key,
        peer_id: old_info.peer_id,
        multi_address: old_info.multi_address,
        sector_size: old_info.sector_size,
        pending_owner_address: old_info.pending_owner_address,
        window_post_proof_type,
        consensus_fault_elapsed: old_info.consensus_fault_elapsed,
        window_post_partition_sectors: old_info.window_post_partition_sectors,
    };

    store
        .put(&new_info, Blake2b256)
        .map_err(|e| MigrationError::BlockStoreWrite(e.to_string()))
}

fn migrate_deadlines<BS: BlockStore + Send + Sync>(
    store: &BS,
    deadlines: Cid,
) -> Result<Cid, MigrationError> {
    let in_deadlines: Option<Miner2Deadlines> = store
        .get(&deadlines)
        .map_err(|e| MigrationError::BlockStoreRead(e.to_string()))?;

    if in_deadlines.is_none() {
        return Err(MigrationError::BlockStoreRead(
            "could not fetch deadlines from blockstore".to_string(),
        ));
    }

    let in_deadlines = in_deadlines.unwrap();

    let out_deadlines = Miner3Deadlines { due: vec![] };

    let deadline_template =
        Deadline::new(store).map_err(|e| MigrationError::BlockStoreRead(e.to_string()));

    for (i, c) in in_deadlines.due.iter().enumerate() {}

    store
        .put(&out_deadlines, Blake2b256)
        .map_err(|e| MigrationError::BlockStoreWrite(e.to_string()))
}

fn migrate_partitions<BS: BlockStore + Send + Sync, V>(
    store: BS,
    root: Cid,
) -> Result<Cid, MigrationError> {
    let in_array =
        Array::load(&root, &store, ActorVersion::V2).map_err(|_| MigrationError::Other)?;

    let out_array = Array::new(&store, ActorVersion::V3);

    in_array.for_each(|i, in_partition: &PartitionV2| {
        let expirations_epochs = migrate_amt_raw(
            &store,
            &in_partition.expirations_epochs,
            PARTITION_EXPIRATION_AMT_BITWIDTH,
        )?;

        let early_terminated = migrate_amt_raw(
            &store,
            &in_partition.early_terminated,
            PARTITION_EARLY_TERMINATION_ARRAY_AMT_BITWIDTH,
        )?;

        let out_partition = PartitionV3 {
            expirations_epochs,
            early_terminated,
            faults: in_partition.faults,
            sectors: in_partition.sectors,
            unproven: in_partition.unproven,
            faulty_power: MinerV3PowerPair {
                raw: in_partition.faulty_power.raw,
                qa: in_partition.faulty_power.qa,
            },
            unproven_power: MinerV3PowerPair {
                raw: in_partition.unproven_power.raw,
                qa: in_partition.unproven_power.qa,
            },
            recovering_power: MinerV3PowerPair {
                raw: in_partition.recovering_power.raw,
                qa: in_partition.recovering_power.qa,
            },
            live_power: MinerV3PowerPair {
                raw: in_partition.live_power.raw,
                qa: in_partition.live_power.qa,
            },
            recoveries: in_partition.recoveries,
            terminated: in_partition.terminated,
        };

        out_array.set(i, out_partition)
    });
    out_array
        .flush()
        .map_err(|_| MigrationError::BlockStoreWrite("couldn't flush array to store".to_string()))
}
