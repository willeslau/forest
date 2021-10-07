use cid::{Cid, Code::Blake2b256};
use ipld_amt::Amt;
use ipld_blockstore::BlockStore;
use ipld_hamt::Hamt;

use crate::MigrationError;

pub fn migrate_amt_raw<BS: BlockStore + Send + Sync>(
    store: &BS,
    root: &Cid,
    new_bit_width: usize,
) -> Result<Cid, MigrationError> {
    let in_root_node =
        Amt::load(root, store).map_err(|e| MigrationError::BlockStoreRead(e.to_string()))?;

    let mut out_root_node: Amt<Cid, BS> = Amt::new_with_bit_width(store, new_bit_width);

    let _ = in_root_node.for_each(|key, data| out_root_node.set(key, *data).map_err(|e| e.into()));

    out_root_node
        .flush()
        .map_err(|e| MigrationError::FlushFailed(e.to_string()))
}

pub fn migrate_hamt_raw<BS: BlockStore + Send + Sync>(
    store: &BS,
    root: &Cid,
    new_bit_width: u32,
) -> Result<Cid, MigrationError> {
    let in_root_node: Hamt<BS, Cid> =
        Hamt::load(root, store).map_err(|e| MigrationError::BlockStoreRead(e.to_string()))?;

    let mut out_root_node: Hamt<BS, Cid> = Hamt::new_with_bit_width(store, new_bit_width);

    let _ = in_root_node.for_each(|key, data| {
        let _ = out_root_node.set(key.to_owned(), *data);
        Ok(())
    });

    out_root_node
        .flush()
        .map_err(|e| MigrationError::FlushFailed(e.to_string()))
}

pub fn migrate_hamt_hamt_raw<BS: BlockStore + Send + Sync>(
    store: &BS,
    root: &Cid,
    new_outer_bitwidth: u32,
    new_inner_bitwidth: u32,
) -> Result<Cid, MigrationError> {
    let in_root_node_outer: Hamt<BS, Cid> =
        Hamt::load(root, store).map_err(|e| MigrationError::BlockStoreRead(e.to_string()))?;

    let mut out_root_node_outer: Hamt<BS, Cid> =
        Hamt::new_with_bit_width(store, new_outer_bitwidth);

    let _ = in_root_node_outer.for_each(|k, v| {
        let out_inner = migrate_hamt_raw(store, v, new_inner_bitwidth)?;
        let _ = out_root_node_outer
            .set(k.to_owned(), out_inner)
            .map_err(|e| MigrationError::BlockStoreWrite(e.to_string()))?;
        Ok(())
    });

    out_root_node_outer
        .flush()
        .map_err(|e| MigrationError::FlushFailed(e.to_string()))?;

    store
        .put(&out_root_node_outer, Blake2b256)
        .map_err(|e| MigrationError::BlockStoreWrite(e.to_string()))
}

pub fn migrate_hamt_amt_raw<BS: BlockStore + Send + Sync>(
    store: &BS,
    root: &Cid,
    new_outer_bitwidth: u32,
    new_inner_bitwidth: usize,
) -> Result<Cid, MigrationError> {
    let in_root_node_outer: Hamt<BS, Cid> =
        Hamt::load(root, store).map_err(|e| MigrationError::BlockStoreRead(e.to_string()))?;

    let out_root_node_outer: Hamt<BS, Cid> = Hamt::new_with_bit_width(store, new_outer_bitwidth);

    let _ = in_root_node_outer.for_each(|k, v| {
        let out_inner = migrate_amt_raw(store, v, new_inner_bitwidth)?;
        let _ = out_root_node_outer.set(k.to_owned(), out_inner);
        Ok(())
    });

    out_root_node_outer
        .flush()
        .map_err(|e| MigrationError::BlockStoreWrite(e.to_string()))?;

    store
        .put(&out_root_node_outer, Blake2b256)
        .map_err(|e| MigrationError::BlockStoreWrite(e.to_string()))
}
