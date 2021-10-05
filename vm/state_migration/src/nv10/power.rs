use crate::{
    ActorMigration, ActorMigrationInput, MigrationError, MigrationOutput, MigrationResult,
};
use ipld_blockstore::BlockStore;
use std::sync::Arc;

use cid::Cid;

use actor_interface::actorv2::power::Claim as Power2Claim;
use actor_interface::actorv3::power::Claim as Power3Claim;
use actor_interface::ActorVersion;
use actor_interface::Map;

pub struct PowerMigrator(Cid);

impl<BS: BlockStore + Send + Sync> ActorMigration<BS> for PowerMigrator {
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
    let in_claims: Map<BS, Cid> = Map::load(&root, store, ActorVersion::V2).map_err(|_| {
        MigrationError::BlockStoreRead("Could not load Power map from root".to_string())
    })?;

    let mut out_claims: Map<BS, Cid> = Map::new(store, ActorVersion::V3);

    let _ = in_claims.for_each(|k, v| {
        let in_claim: Power2Claim = store.get(k).map_err(|e| {
            MigrationError::BlockStoreRead("Could not load claim from blockstore".to_string())
        })?;
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
