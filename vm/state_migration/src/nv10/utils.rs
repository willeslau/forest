use cid::Cid;
use ipld_amt::Amt;
use ipld_blockstore::BlockStore;

use crate::MigrationError;

pub fn migrate_amt_raw<BS: BlockStore + Send + Sync>(
    store: &BS,
    root: &Cid,
    new_bit_width: usize,
) -> Result<Cid, MigrationError> {
    let in_root_node = Amt::load(root, store).map_err(|e| {
        MigrationError::BlockStoreRead("Could not load Amt from root node".to_string())
    })?;

    let out_root_node = Amt::new_with_bit_width(store, new_bit_width);

    in_root_node.for_each(|key, data| out_root_node.set(key, data).map_err(|e| e.into()));

    out_root_node
        .flush()
        .map_err(|e| MigrationError::FlushFailed(e.to_string()))
}
