// Copyright 2020 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

//! This module implements network version 12 or actorv4 state migration
//! Please read https://filecoin.io/blog/posts/filecoin-network-v12/
//! to learn more about network version 12 migration.
//! This is more or less a direct port of the state migration
//! implemented in lotus' specs-actors library.

pub mod miner;

use crate::nil_migrator_v4;
use crate::MigrationJobOutput;
use crate::{ActorMigration, MigrationError, MigrationJob, MigrationResult};
use actor_interface::{actorv3, actorv4};
use async_std::sync::Arc;
use async_std::task;
use cid::Cid;
use clock::ChainEpoch;
use crossbeam_utils::thread;
use fil_types::StateTreeVersion;
use futures::stream::FuturesOrdered;
use ipld_blockstore::BlockStore;
use miner::miner_migrator_v4;
use state_tree::StateTree;
use std::collections::{HashMap, HashSet};

type Migrator<BS> = Arc<dyn ActorMigration<BS> + Send + Sync>;

const ACTORS_COUNT: usize = 11;

// Try to pass an Arc<BS> here.
pub fn migrate_state_tree<BS: BlockStore + Send + Sync>(
    store: Arc<BS>,
    actors_root_in: Cid,
    prior_epoch: ChainEpoch,
) -> MigrationResult<Cid> {
    dbg!("migration began");
    // let mut jobs = FuturesOrdered::new();
    // TODO
    // pass job_tx to each job instance's run method.
    // iterate and collect on job_rx with block_on

    // Maps prior version code CIDs to migration functions.
    let mut migrations: HashMap<Cid, Migrator<BS>> = HashMap::with_capacity(ACTORS_COUNT);
    migrations.insert(
        *actorv3::ACCOUNT_ACTOR_CODE_ID,
        nil_migrator_v4(*actorv4::ACCOUNT_ACTOR_CODE_ID),
    );
    migrations.insert(
        *actorv3::CRON_ACTOR_CODE_ID,
        nil_migrator_v4(*actorv4::CRON_ACTOR_CODE_ID),
    );
    migrations.insert(
        *actorv3::INIT_ACTOR_CODE_ID,
        nil_migrator_v4(*actorv4::INIT_ACTOR_CODE_ID),
    );
    migrations.insert(
        *actorv3::MULTISIG_ACTOR_CODE_ID,
        nil_migrator_v4(*actorv4::MULTISIG_ACTOR_CODE_ID),
    );
    migrations.insert(
        *actorv3::PAYCH_ACTOR_CODE_ID,
        nil_migrator_v4(*actorv4::PAYCH_ACTOR_CODE_ID),
    );
    migrations.insert(
        *actorv3::REWARD_ACTOR_CODE_ID,
        nil_migrator_v4(*actorv4::REWARD_ACTOR_CODE_ID),
    );
    migrations.insert(
        *actorv3::MARKET_ACTOR_CODE_ID,
        nil_migrator_v4(*actorv4::MARKET_ACTOR_CODE_ID),
    );
    migrations.insert(
        *actorv3::MINER_ACTOR_CODE_ID,
        miner_migrator_v4(*actorv4::MINER_ACTOR_CODE_ID),
    );
    migrations.insert(
        *actorv3::POWER_ACTOR_CODE_ID,
        nil_migrator_v4(*actorv4::POWER_ACTOR_CODE_ID),
    );
    migrations.insert(
        *actorv3::SYSTEM_ACTOR_CODE_ID,
        nil_migrator_v4(*actorv4::SYSTEM_ACTOR_CODE_ID),
    );
    migrations.insert(
        *actorv3::VERIFREG_ACTOR_CODE_ID,
        nil_migrator_v4(*actorv4::VERIFREG_ACTOR_CODE_ID),
    );

    // Set of prior version code CIDs for actors to defer during iteration, for explicit migration afterwards.
    let deferred_code_ids = HashSet::<Cid>::new(); // None in this migration

    if migrations.len() + deferred_code_ids.len() != ACTORS_COUNT {
        return Err(MigrationError::IncompleteMigrationSpec(migrations.len()));
    }

    // input actors state tree -
    let actors_in = StateTree::new_from_root(&*store, &actors_root_in).unwrap();
    let mut actors_out = StateTree::new(&*store, StateTreeVersion::V3)
        .map_err(|e| MigrationError::StateTreeCreation(e.to_string()))?;

    let mut i = 0;

    let tp = rayon::ThreadPoolBuilder::new()
        .num_threads(5)
        .build()
        .unwrap();
    tp.scope(|s| {
        let (tx_in, rx_in) = crossbeam_channel::bounded(5);
        let (tx_out, rx_out) = crossbeam_channel::bounded(5);
        let store_clone = store.clone();
        s.spawn(move |s1| {
            actors_in.for_each(|addr, state| {
                tx_in.send((addr, state.clone())).unwrap();
                Ok(())
            }).unwrap();
        });
        s.spawn(move |s2| {
            let (addr, state) = rx_in.recv().unwrap();
            let migrator = migrations.get(&state.code).cloned().unwrap();
            s2.spawn(move |_s22| {
                let next_input = MigrationJob {
                    address: addr.clone(),
                    actor_state: state,
                    actor_migration: migrator,
                };

                let a = next_input.run(store_clone, prior_epoch).unwrap();

                tx_out.send(a).expect("failed sending job output");
                i += 1;
                // dbg!("sent");
            });
        });
        while let Ok(job_output) = rx_out.recv() {
            actors_out
                .set_actor(&job_output.address, job_output.actor_state)
                .unwrap();
        }
    });

    // log::info!("number of for_each calls: {}", i);

    // let mut b = 0;

    // for i in job_rx.try_iter() {
    //     actors_out
    //         .set_actor(&i.address, i.actor_state)
    //         .map_err(|e| MigrationError::SetActorState(e.to_string()))?;
    //     b += 1;
    // }

    // for i in a {
    //     b += 1;
    //     actors_out.set_actor(&i.address, i.actor_state).map_err(|e| MigrationError::SetActorState(e.to_string()))?;
    // }
    // for i in job_rx.try_iter() {
    //     // dbg!("setting job output");
    //     actors_out.set_actor(&i.address, i.actor_state).map_err(|e| MigrationError::SetActorState(e.to_string()))?;
    //     b += 1;
    //     dbg!(b);
    // }

    // log::info!("number of received: {}", b);

    // task::spawn(async {
    //     while let Some(job_result) = jobs.next().await {
    //         let result = job_result?;
    //         actors_out
    //             .set_actor(&result.address, result.actor_state)
    //             .map_err(|e| MigrationError::SetActorState(e.to_string()))?;
    //     }

    //     Ok(())
    // });

    log::info!("before flush");
    let root_cid = actors_out
        .flush()
        .map_err(|e| MigrationError::FlushFailed(e.to_string()));
    log::info!("after flush");

    root_cid
}
