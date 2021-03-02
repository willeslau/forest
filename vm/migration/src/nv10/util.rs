use actor::actorv2::ipld_amt::Amt as AmtV2;
use actor::actorv2::ipld_hamt::Hamt as HamtV2;
use actor::actorv3::ipld_amt::Amt as AmtV3;
use actor::actorv3::ipld_hamt::Hamt as HamtV3;

use cid::Cid;
use forest_hash_utils::{BytesKey, Hash};
use ipld_blockstore::BlockStore;
use serde::{de::DeserializeOwned, Serialize};
use std::borrow::Borrow;
use std::error::Error as StdError;

pub(crate) fn migrate_hamt_raw<BS, V>(
    store: &BS,
    root: &Cid,
    new_bitwidth: u32,
) -> Result<Cid, Box<dyn StdError>>
where
    V: Serialize + DeserializeOwned + PartialEq + Clone,
    BS: BlockStore,
{
    let in_root_node: HamtV2<'_, BS, V, BytesKey> = HamtV2::load(&root, store)?;
    let mut out_root_node: HamtV3<'_, BS, V, BytesKey> =
        HamtV3::new_with_bit_width(store, new_bitwidth);

    in_root_node.for_each(|k, v| {
        out_root_node
            .set(k.clone(), v.clone())?
            .ok_or_else(|| "Failed to set Hamt node in v3".to_owned())?;
        Ok(())
    })?;
    Ok(out_root_node.flush()?)
}

pub(crate) fn migrate_amt_raw<BS, V>(
    store: &BS,
    root: &Cid,
    new_bitwidth: usize,
) -> Result<Cid, Box<dyn StdError>>
where
    V: Serialize + DeserializeOwned + PartialEq + Clone,
    BS: BlockStore,
{
    let in_root_node: AmtV2<'_, V, BS> = AmtV2::load(&root, store)?;
    let mut out_root_node: AmtV3<'_, V, BS> = AmtV3::new_with_bit_width(store, new_bitwidth);

    in_root_node.for_each(|k, v| {
        out_root_node.set(k as usize, v.clone())?;
        Ok(())
    })?;
    Ok(out_root_node.flush()?)
}

pub(crate) fn migrate_hamt_hamt<BS, V>(
    store: &BS,
    root: Cid,
    new_outer_bitwidth: u32,
    new_inner_bitwidth: u32,
) -> Result<Cid, Box<dyn StdError>>
where
    V: Serialize + DeserializeOwned + PartialEq + Clone,
    BS: BlockStore,
{
    todo!()
}
