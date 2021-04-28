use std::sync::Arc;

use address::Address;
use cid::Cid;
use forest_db::MemoryDB;
use futures::TryFutureExt;
use num_bigint::BigInt;
use paych::PaychStore;
use paych::{PaychProvider, TestPaychProvider};
use paych::Manager as PaychManager;
use vm::ActorState;
use wallet::generate_key;
use crypto::SignatureType;

fn new_manager() -> PaychManager<TestPaychProvider<MemoryDB>, MemoryDB> {
    let pch_store = PaychStore::default();
    let bs = MemoryDB::default();
    let api = TestPaychProvider::new(bs);
    PaychManager::new(pch_store, Arc::new(api))
}

fn gen_key_pair() -> (Vec<u8>, Vec<u8>) { // priv, pub
	let k = generate_key(SignatureType::Secp256k1).unwrap();
    let pubk = k.public_key.clone();
    let privk = k.key_info.private_key().clone();
    (privk, pubk)
}

#[async_std::test]
async fn t1() {
    let pmgr = new_manager();
    let mut mock = pmgr.api.clone();
    let (from_key_private, from_key_public) = gen_key_pair();
    let (to_key_private, to_key_public) = gen_key_pair();
    let (rand_key_private, _) = gen_key_pair();

    let ch = Address::new_id(100);
    let from = Address::new_secp256k1(&from_key_public).unwrap();
    let to = Address::new_secp256k1(&to_key_public).unwrap();
    let from_acc = Address::new_actor(b"fromAct");
    let to_acc = Address::new_actor(b"toAct");

    mock.set_account_addr(from_acc, from).await;
    mock.set_account_addr(to_acc, to).await;


    	// name:          "passes when voucher amount < balance",
		// key:           fromKeyPrivate,
		// actorBalance:  big.NewInt(10),
		// voucherAmount: big.NewInt(5),
    let act = ActorState {
        code: *actor::actorv2::ACCOUNT_ACTOR_CODE_ID,
        state: cid::new_from_cbor(&[], cid::Code::Identity),
        sequence: 0,
        balance: 10.into(),
    };
    let empty_arr_cid = actor::actorv2::ipld_amt::Amt::<Cid, _>::new(mock.bs()).flush().unwrap();
    let pch_st = actor::actorv2::paych::State::new(from_acc, to_acc, empty_arr_cid);

    mock.set_paych_state(ch, (act, actor::paych::State::V2(pch_st))).await;

    let store = PaychStore::default();
    let mut mgr = PaychManager::new(store, mock.clone());
    mgr.start().await.unwrap();
    println!("tppp: {}", to);
    mock.add_wallet_addr(to).await;
    let sv = create_test_voucher(ch, 0, 0, 5.into(), &from_key_private);

    mgr.check_voucher_valid(ch, actor::paych::SignedVoucher::V2(sv)).await.unwrap()
}

#[async_std::test]
async fn t2() {
    let pmgr = new_manager();
    let mut mock = pmgr.api.clone();
    let (from_key_private, from_key_public) = gen_key_pair();
    let (to_key_private, to_key_public) = gen_key_pair();
    let (rand_key_private, _) = gen_key_pair();

    let ch = Address::new_id(100);
    let from = Address::new_secp256k1(&from_key_public).unwrap();
    let to = Address::new_secp256k1(&to_key_public).unwrap();
    let from_acc = Address::new_actor(b"fromAct");
    let to_acc = Address::new_actor(b"toAct");

    mock.set_account_addr(from_acc, from).await;
    mock.set_account_addr(to_acc, to).await;


    	//		name:          "fails when funds too low",
		// expectError:   true,
		// key:           fromKeyPrivate,
		// actorBalance:  big.NewInt(5),
		// voucherAmount: big.NewInt(10),
    let act = ActorState {
        code: *actor::actorv2::ACCOUNT_ACTOR_CODE_ID,
        state: cid::new_from_cbor(&[], cid::Code::Identity),
        sequence: 0,
        balance: 5.into(),
    };
    let empty_arr_cid = actor::actorv2::ipld_amt::Amt::<Cid, _>::new(mock.bs()).flush().unwrap();
    let pch_st = actor::actorv2::paych::State::new(from_acc, to_acc, empty_arr_cid);

    mock.set_paych_state(ch, (act, actor::paych::State::V2(pch_st))).await;

    let store = PaychStore::default();
    let mut mgr = PaychManager::new(store, mock.clone());
    mgr.start().await.unwrap();
    println!("tppp: {}", to);
    mock.add_wallet_addr(to).await;
    let sv = create_test_voucher(ch, 0, 0, 10.into(), &from_key_private);

    assert!(mgr.check_voucher_valid(ch, actor::paych::SignedVoucher::V2(sv)).await.is_err())
}

#[async_std::test]
async fn t3() {
    let pmgr = new_manager();
    let mut mock = pmgr.api.clone();
    let (from_key_private, from_key_public) = gen_key_pair();
    let (to_key_private, to_key_public) = gen_key_pair();
    let (rand_key_private, _) = gen_key_pair();

    let ch = Address::new_id(100);
    let from = Address::new_secp256k1(&from_key_public).unwrap();
    let to = Address::new_secp256k1(&to_key_public).unwrap();
    let from_acc = Address::new_actor(b"fromAct");
    let to_acc = Address::new_actor(b"toAct");

    mock.set_account_addr(from_acc, from).await;
    mock.set_account_addr(to_acc, to).await;


    	//		name:          "passes when nonce higher",
		// key:           fromKeyPrivate,
		// actorBalance:  big.NewInt(10),
		// voucherAmount: big.NewInt(5),
		// voucherLane:   1,
		// voucherNonce:  3,
		// laneStates: map[uint64]paych.LaneState{
		// 	1: paychmock.NewMockLaneState(big.NewInt(2), 2),// redemed, nonce

    let act = ActorState {
        code: *actor::actorv2::ACCOUNT_ACTOR_CODE_ID,
        state: cid::new_from_cbor(&[], cid::Code::Identity),
        sequence: 0,
        balance: 10.into(),
    };
    let mut empty_arr = actor::actorv2::ipld_amt::Amt::<actor::paych::LaneState, _>::new(mock.bs());
    empty_arr.set(1, actor::paych::LaneState::new(2.into(), 2)).unwrap();
    let empty_arr_cid = empty_arr.flush().unwrap();
    let pch_st = actor::actorv2::paych::State::new(from_acc, to_acc, empty_arr_cid);

    mock.set_paych_state(ch, (act, actor::paych::State::V2(pch_st))).await;

    let store = PaychStore::default();
    let mut mgr = PaychManager::new(store, mock.clone());
    mgr.start().await.unwrap();
    println!("tppp: {}", to);
    mock.add_wallet_addr(to).await;
    let sv = create_test_voucher(ch, 1, 3, 5.into(), &from_key_private);

    mgr.check_voucher_valid(ch, actor::paych::SignedVoucher::V2(sv)).await.unwrap();
}

#[async_std::test]
async fn t4() {
    let pmgr = new_manager();
    let mut mock = pmgr.api.clone();
    let (from_key_private, from_key_public) = gen_key_pair();
    let (to_key_private, to_key_public) = gen_key_pair();
    let (rand_key_private, _) = gen_key_pair();

    let ch = Address::new_id(100);
    let from = Address::new_secp256k1(&from_key_public).unwrap();
    let to = Address::new_secp256k1(&to_key_public).unwrap();
    let from_acc = Address::new_actor(b"fromAct");
    let to_acc = Address::new_actor(b"toAct");

    mock.set_account_addr(from_acc, from).await;
    mock.set_account_addr(to_acc, to).await;



    // {
		// required balance = total redeemed
		//                  = 6 (voucher lane 1) + 5 (lane 2)
		//                  = 11
		// So required balance: 11 > actor balance: 10
		// name:          "fails when voucher total redeemed > balance",
		// expectError:   true,
		// key:           fromKeyPrivate,
		// actorBalance:  big.NewInt(10),
		// voucherAmount: big.NewInt(6),
		// voucherLane:   1,
		// voucherNonce:  1,
		// laneStates: map[uint64]paych.LaneState{
		// 	// Lane 2 (different from voucher lane 1)
		// 	2: paychmock.NewMockLaneState(big.NewInt(5), 1),
		// },

        let act = ActorState {
            code: *actor::actorv2::ACCOUNT_ACTOR_CODE_ID,
            state: cid::new_from_cbor(&[], cid::Code::Identity),
            sequence: 0,
            balance: 10.into(),
        };
        let mut empty_arr = actor::actorv2::ipld_amt::Amt::<actor::paych::LaneState, _>::new(mock.bs());
        empty_arr.set(2, actor::paych::LaneState::new(5.into(), 1)).unwrap();
        let empty_arr_cid = empty_arr.flush().unwrap();
        let pch_st = actor::actorv2::paych::State::new(from_acc, to_acc, empty_arr_cid);
    
        mock.set_paych_state(ch, (act, actor::paych::State::V2(pch_st))).await;
    
        let store = PaychStore::default();
        let mut mgr = PaychManager::new(store, mock.clone());
        mgr.start().await.unwrap();
        mock.add_wallet_addr(to).await;
        let sv = create_test_voucher(ch, 1, 1, 6.into(), &from_key_private);
    
        assert!(mgr.check_voucher_valid(ch, actor::paych::SignedVoucher::V2(sv)).await.is_err());
}

#[async_std::test]
async fn t5() {
    let pmgr = new_manager();
    let mut mock = pmgr.api.clone();
    let (from_key_private, from_key_public) = gen_key_pair();
    let (to_key_private, to_key_public) = gen_key_pair();
    let (rand_key_private, _) = gen_key_pair();

    let ch = Address::new_id(100);
    let from = Address::new_secp256k1(&from_key_public).unwrap();
    let to = Address::new_secp256k1(&to_key_public).unwrap();
    let from_acc = Address::new_actor(b"fromAct");
    let to_acc = Address::new_actor(b"toAct");

    mock.set_account_addr(from_acc, from).await;
    mock.set_account_addr(to_acc, to).await;



		// voucher supersedes lane 1 redeemed so
		// lane 1 effective redeemed = voucher amount
		//
		// required balance = total redeemed
		//                  = 5 (new voucher lane 1) + 5 (lane 2)
		//                  = 10
		// So required balance: 10 <= actor balance: 10
		// name:          "passes when voucher total redeemed <= balance",
		// expectError:   false,
		// key:           fromKeyPrivate,
		// actorBalance:  big.NewInt(10),
		// voucherAmount: big.NewInt(5),
		// voucherLane:   1,
		// voucherNonce:  2,
		// laneStates: map[uint64]paych.LaneState{
		// 	// Lane 1 (superseded by new voucher in voucher lane 1)
		// 	1: paychmock.NewMockLaneState(big.NewInt(4), 1),
		// 	// Lane 2 (different from voucher lane 1)
		// 	2: paychmock.NewMockLaneState(big.NewInt(5), 1),
		// },

        let act = ActorState {
            code: *actor::actorv2::ACCOUNT_ACTOR_CODE_ID,
            state: cid::new_from_cbor(&[], cid::Code::Identity),
            sequence: 0,
            balance: 10.into(),
        };
        let mut empty_arr = actor::actorv2::ipld_amt::Amt::<actor::paych::LaneState, _>::new(mock.bs());
        empty_arr.set(1, actor::paych::LaneState::new(4.into(), 1)).unwrap();
        empty_arr.set(2, actor::paych::LaneState::new(5.into(), 1)).unwrap();
        let empty_arr_cid = empty_arr.flush().unwrap();
        let pch_st = actor::actorv2::paych::State::new(from_acc, to_acc, empty_arr_cid);
    
        mock.set_paych_state(ch, (act, actor::paych::State::V2(pch_st))).await;
    
        let store = PaychStore::default();
        let mut mgr = PaychManager::new(store, mock.clone());
        mgr.start().await.unwrap();
        mock.add_wallet_addr(to).await;
        let sv = create_test_voucher(ch, 1, 2, 5.into(), &from_key_private);
    
        mgr.check_voucher_valid(ch, actor::paych::SignedVoucher::V2(sv)).await.unwrap();
}




fn create_test_voucher(ch: Address, voucher_lane: u64, nonce: u64, amount: BigInt, key: &[u8]) -> actor::actorv2::paych::SignedVoucher {
    let mut sv = actor::actorv2::paych::SignedVoucher {
        channel_addr: ch,
        time_lock_min: Default::default(),
        time_lock_max: Default::default(),
        secret_pre_image: Default::default(),
        extra: Default::default(),
        lane: voucher_lane,
        nonce: nonce,
        amount: amount,
        min_settle_height: Default::default(),
        merges: Default::default(),
        signature: Default::default(),
    };
    let signing_bytes = sv.signing_bytes().unwrap();
    let sig = wallet::sign(SignatureType::Secp256k1, key, &signing_bytes).unwrap();
    sv.signature = Some(sig);
    sv
}

// func createTestVoucher(t *testing.T, ch address.Address, voucherLane uint64, nonce uint64, voucherAmount big.Int, key []byte) *paych2.SignedVoucher {
// 	sv := &paych2.SignedVoucher{
// 		ChannelAddr: ch,
// 		Lane:        voucherLane,
// 		Nonce:       nonce,
// 		Amount:      voucherAmount,
// 	}

// 	signingBytes, err := sv.SigningBytes()
// 	require.NoError(t, err)
// 	sig, err := sigs.Sign(crypto.SigTypeSecp256k1, key, signingBytes)
// 	require.NoError(t, err)
// 	sv.Signature = sig
// 	return sv
// }


// fn setup
// func testSetupMgrWithChannel(t *testing.T) *testScaffold {
// 	fromKeyPrivate, fromKeyPublic := testGenerateKeyPair(t)

// 	ch := tutils.NewIDAddr(t, 100)
// 	from := tutils.NewSECP256K1Addr(t, string(fromKeyPublic))
// 	to := tutils.NewSECP256K1Addr(t, "secpTo")
// 	fromAcct := tutils.NewActorAddr(t, "fromAct")
// 	toAcct := tutils.NewActorAddr(t, "toAct")

// 	mock := newMockManagerAPI()
// 	mock.setAccountAddress(fromAcct, from)
// 	mock.setAccountAddress(toAcct, to)

// 	// Create channel in state
// 	balance := big.NewInt(20)
// 	act := &types.Actor{
// 		Code:    builtin.AccountActorCodeID,
// 		Head:    cid.Cid{},
// 		Nonce:   0,
// 		Balance: balance,
// 	}
// 	mock.setPaychState(ch, act, paychmock.NewMockPayChState(fromAcct, toAcct, abi.ChainEpoch(0), make(map[uint64]paych.LaneState)))

// 	store := NewStore(ds_sync.MutexWrap(ds.NewMapDatastore()))
// 	mgr, err := newManager(store, mock)
// 	require.NoError(t, err)

// 	// Create the channel in the manager's store
// 	ci := &ChannelInfo{
// 		Channel:   &ch,
// 		Control:   fromAcct,
// 		Target:    toAcct,
// 		Direction: DirOutbound,
// 	}
// 	err = mgr.store.putChannelInfo(ci)
// 	require.NoError(t, err)

// 	// Add the from signing key to the wallet
// 	mock.addSigningKey(fromKeyPrivate)

// 	return &testScaffold{
// 		mgr:            mgr,
// 		mock:           mock,
// 		ch:             ch,
// 		amt:            balance,
// 		fromAcct:       fromAcct,
// 		fromKeyPrivate: fromKeyPrivate,
// 	}
// }