
use cid::Cid;
use ipld_blockstore::BlockStore;
use actor_interface::Map;
use actor_interface::ActorVersion;
use fil_types::HAMT_BIT_WIDTH;
use encoding::Cbor;
use ipld::Code;

use crate::ActorMigration;

// Migrates a HAMT from v2 to v3 without re-encoding keys or values.
fn migrate_hamt_raw<BS: BlockStore>(store: BS, root: Cid, new_bitwidth: i32) -> Result<Cid, Box<dyn std::error::Error>> {
	let in_root_node = Map::load(&root, &store, ActorVersion::V2)?;
	// newOpts := append(adt3.DefaultHamtOptions, hamt3.UseTreeBitWidth(newBitwidth))  // TODO?
    // TODO: lotus initializes the below HAMT with a specific versioned HAMT_BIT_WIDTH? should we as well?
	let out_root_node = Map::new(&store,  ActorVersion::V3); // FIXME BIT WIDTH needs to be different based on v2/v3

    in_root_node.for_each(|k, v: Vec<u8>| {
        let a = v.marshal_cbor();
        out_root_node.set(k.clone(), v.clone())
    });

    let a = out_root_node.flush();

    let root_cid = store.put(&out_root_node, Code::Blake2b256);

    root_cid
}

// Migrates a HAMT from v2 to v3 without re-encoding keys or values.
fn migrate_amt_raw<BS: BlockStore>(store: BS, root: Cid, new_bitwidth: i32) -> Result<Cid, Box<dyn std::error::Error>> {
	let in_root_node = Map::load(&root, &store, ActorVersion::V2)?;
	// newOpts := append(adt3.DefaultHamtOptions, hamt3.UseTreeBitWidth(newBitwidth))  // TODO?
    // TODO: lotus initializes the below HAMT with a specific versioned HAMT_BIT_WIDTH? should we as well?
	let out_root_node = Map::new(&store,  ActorVersion::V3); // FIXME BIT WIDTH needs to be different based on v2/v3

    in_root_node.for_each(|k, v: Vec<u8>| {
        let a = v.marshal_cbor();
        out_root_node.set(k.clone(), v.clone())
    });

    let a = out_root_node.flush();

    let root_cid = store.put(&out_root_node, Code::Blake2b256);

    root_cid
}