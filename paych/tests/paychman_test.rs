use std::sync::Arc;

use actor::{actorv0::cron::Actor, actorv2::paych::ModVerifyParams};
use address::Address;
use cid::Cid;
use fil_types::verifier::MockVerifier;
use forest_db::MemoryDB;
use futures::TryFutureExt;
use message::{MessageReceipt, UnsignedMessage};
use num_bigint::BigInt;
use paych::{ChannelInfo, DIR_OUTBOUND, Error, PaychStore};
use paych::{PaychProvider, TestPaychProvider};
use paych::Manager as PaychManager;
use serde::Serialize;
use state_manager::InvocResult;
use vm::ActorState;
use wallet::generate_key;
use crypto::SignatureType;

type TestManager = PaychManager<TestPaychProvider<MemoryDB>, MemoryDB>;
fn new_manager() -> TestManager {
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
    let sv = create_test_voucher(ch, 0, 0, 5.into(), Some(&from_key_private));

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
    let sv = create_test_voucher(ch, 0, 0, 10.into(), Some(&from_key_private));

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
    let sv = create_test_voucher(ch, 1, 3, 5.into(), Some(&from_key_private));

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
        let sv = create_test_voucher(ch, 1, 1, 6.into(), Some(&from_key_private));
    
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
        let sv = create_test_voucher(ch, 1, 2, 5.into(), Some(&from_key_private));
    
        mgr.check_voucher_valid(ch, actor::paych::SignedVoucher::V2(sv)).await.unwrap();
}

/// test create voucher

#[async_std::test]
async fn test_create_voucher() {
    let mut s = create_manager_with_channel().await;

    // Create a voucher in lane 1
    let voucher_lane_1_amt = BigInt::from(5);
    let voucher = actor::paych::SignedVoucher::V2(
        create_test_voucher(Address::new_id(1111), 1, 0, voucher_lane_1_amt.clone(), None)
    );
    println!("Start create voucher");
    let res = s.mgr.create_voucher::<MockVerifier>(s.ch, voucher).await.unwrap();
    
    assert_eq!(s.ch, res.channel_addr());
    assert_eq!(&voucher_lane_1_amt, res.amount());
    
    let nonce = res.nonce();

    // Create a voucher in lane 1 again, with a higher amount
    let voucher_lane_1_amt = BigInt::from(8);
    let voucher = actor::paych::SignedVoucher::V2(
        create_test_voucher(Address::new_id(1111), 1, 0, voucher_lane_1_amt.clone(), None)
    );
    println!("Start create voucher");
    let res = s.mgr.create_voucher::<MockVerifier>(s.ch, voucher).await.unwrap();
    
    assert_eq!(s.ch, res.channel_addr());
    assert_eq!(&voucher_lane_1_amt, res.amount());
    assert_eq!(nonce+1, res.nonce());

    // Create a voucher in lane 2 that covers all the remaining funds
	// in the channel
    let voucher_lane_2_amt = s.amt - &voucher_lane_1_amt;
    let voucher = actor::paych::SignedVoucher::V2(
        create_test_voucher(Address::new_id(1111), 2, 0, voucher_lane_2_amt.clone(), None)
    );
    println!("Start create voucher");
    let res = s.mgr.create_voucher::<MockVerifier>(s.ch, voucher).await.unwrap();
    assert_eq!(s.ch, res.channel_addr());
    assert_eq!(&voucher_lane_2_amt, res.amount());

    // Create a voucher in lane 2 that exceeds the remaining funds in the
	// channel
    let voucher_lane_2_amt: BigInt = voucher_lane_2_amt + 1;
    let voucher = actor::paych::SignedVoucher::V2(
        create_test_voucher(Address::new_id(1111), 2, 0, voucher_lane_2_amt.clone(), None)
    );
    println!("Start create voucher");
    let res = s.mgr.create_voucher::<MockVerifier>(s.ch, voucher).await;

    assert_eq!(Error::InsuffientFunds(1.into()), res.unwrap_err());
}

#[async_std::test]
async fn test_add_voucher_delta() {
    let s = create_manager_with_channel().await;
    let voucher_lane = 1;
    
    // Expect error when adding a voucher whose amount is less than minDelta
    let min_delta = BigInt::from(2);
    let mut nonce = 1;
    let voucher_amount = BigInt::from(1);
    let sv = actor::paych::SignedVoucher::V2(
        create_test_voucher(s.ch, voucher_lane, nonce, voucher_amount.clone(), Some(&s.from_key_private))
    );
    
    assert!(s.mgr.add_voucher_outbound(s.ch, sv, Vec::new(), min_delta.clone()).await.is_err());


    // Expect success when adding a voucher whose amount is equal to minDelta
    nonce += 1;
    let voucher_amount = BigInt::from(2);
    let sv = actor::paych::SignedVoucher::V2(
        create_test_voucher(s.ch, voucher_lane, nonce, voucher_amount.clone(), Some(&s.from_key_private))
    );
    
    assert_eq!(s.mgr.add_voucher_outbound(s.ch, sv, Vec::new(), min_delta.clone()).await.unwrap(), BigInt::from(2));
	
    // Check that delta is correct when there's an existing voucher
    nonce += 1;
    let voucher_amount = BigInt::from(5);
    let sv = actor::paych::SignedVoucher::V2(
        create_test_voucher(s.ch, voucher_lane, nonce, voucher_amount.clone(), Some(&s.from_key_private))
    );
    assert_eq!(s.mgr.add_voucher_outbound(s.ch, sv, Vec::new(), min_delta.clone()).await.unwrap(), BigInt::from(3));
	
    // Check that delta is correct when voucher added to a different lane
    let nonce = 1;
    let voucher_amount = BigInt::from(6);
    let voucher_lane = 2;
    let sv = actor::paych::SignedVoucher::V2(
        create_test_voucher(s.ch, voucher_lane, nonce, voucher_amount.clone(), Some(&s.from_key_private))
    );
    assert_eq!(s.mgr.add_voucher_outbound(s.ch, sv, Vec::new(), min_delta.clone()).await.unwrap(), BigInt::from(6));
}

#[async_std::test]
async fn test_add_voucher_next_lane() {
    let s = create_manager_with_channel().await;   

    let min_delta = BigInt::from(0);
    let voucher_amount = BigInt::from(2);

    // Add a voucher in lane 2
    let nonce = 1;
    let voucher_lane = 2;
    let sv = actor::paych::SignedVoucher::V2(
        create_test_voucher(s.ch, voucher_lane, nonce, voucher_amount.clone(), Some(&s.from_key_private))
    );
    s.mgr.add_voucher_outbound(s.ch, sv, Vec::new(), min_delta.clone()).await.unwrap();

    let ci = s.mgr.get_channel_info(&s.ch).await.unwrap();
    assert_eq!(ci.next_lane, 3);

    // Allocate a lane (should be lane 3)
    let lane = s.mgr.allocate_lane(s.ch).await.unwrap();
    assert_eq!(lane, 3);
    let ci = s.mgr.get_channel_info(&s.ch).await.unwrap();
    assert_eq!(ci.next_lane, 4);

    // Add a voucher in lane 1
    let voucher_lane = 1;
    let sv = actor::paych::SignedVoucher::V2(
        create_test_voucher(s.ch, voucher_lane, nonce, voucher_amount.clone(), Some(&s.from_key_private))
    );
    s.mgr.add_voucher_outbound(s.ch, sv, Vec::new(), min_delta.clone()).await.unwrap();
    let ci = s.mgr.get_channel_info(&s.ch).await.unwrap();
    assert_eq!(ci.next_lane, 4);

    // Add a voucher in lane 7
    let voucher_lane = 7;
    let sv = actor::paych::SignedVoucher::V2(
        create_test_voucher(s.ch, voucher_lane, nonce, voucher_amount.clone(), Some(&s.from_key_private))
    );
    s.mgr.add_voucher_outbound(s.ch, sv, Vec::new(), min_delta.clone()).await.unwrap();
    let ci = s.mgr.get_channel_info(&s.ch).await.unwrap();
    assert_eq!(ci.next_lane, 8);
}

#[async_std::test]
async fn test_allocate_lane() {
    let s = create_manager_with_channel().await;

    // First lane should be 0
    let lane = s.mgr.allocate_lane(s.ch).await.unwrap();
    assert_eq!(lane, 0);

    //Next lane should be 1
    let lane = s.mgr.allocate_lane(s.ch).await.unwrap();
    assert_eq!(lane, 1);
}

#[async_std::test]
async fn test_allocate_lane_with_existing_lane_state() {
    let mut pmgr = new_manager();
    let mock = pmgr.api.clone();
    let (from_key_private, from_key_public) = gen_key_pair();
    let (_to_key_private, to_key_public) = gen_key_pair();

    let ch = Address::new_id(100);
    let from = Address::new_secp256k1(&from_key_public).unwrap();
    let to = Address::new_secp256k1(&to_key_public).unwrap();
    let from_acc = Address::new_actor(b"fromAct");
    let to_acc = Address::new_actor(b"toAct");

    mock.set_account_addr(from_acc, from).await;
    mock.set_account_addr(to_acc, to).await;
    mock.add_wallet_addr(to).await;

    let actor_balance = BigInt::from(10);
    let act = ActorState {
        code: *actor::actorv2::ACCOUNT_ACTOR_CODE_ID,
        state: cid::new_from_cbor(&[], cid::Code::Identity),
        sequence: 0,
        balance: actor_balance.clone(),
    };

    let empty_arr_cid = actor::actorv2::ipld_amt::Amt::<actor::paych::LaneState, _>::new(mock.bs()).flush().unwrap();
    let pch_st = actor::actorv2::paych::State::new(from_acc, to_acc, empty_arr_cid);
    mock.set_paych_state(ch, (act, actor::paych::State::V2(pch_st))).await;

    
	// Create a voucher on lane 2
	// (also reads the channel from state and puts it in the store)
    let voucher_lane = 2;
    let min_delta = BigInt::from(0);
    let nonce = 2;
    let voucher_amount = BigInt::from(5);
    let sv = actor::paych::SignedVoucher::V2(create_test_voucher(ch, voucher_lane, nonce, voucher_amount.clone(),Some(&from_key_private)));

    pmgr.add_voucher_inbound(ch, sv, Vec::new(), min_delta.clone()).await.unwrap();


    // Allocate lane should return the next lane (lane 3)
	let lane = pmgr.allocate_lane(ch).await.unwrap();
	assert_eq!(lane, 3);
}

#[async_std::test]
async fn test_add_voucher_inbound_wallet_key() {
    let mut pmgr = new_manager();
    let mock = pmgr.api.clone();
    let (from_key_private, from_key_public) = gen_key_pair();
    let (_to_key_private, to_key_public) = gen_key_pair();

    let ch = Address::new_id(100);
    let from = Address::new_secp256k1(&from_key_public).unwrap();
    let to = Address::new_secp256k1(&to_key_public).unwrap();
    let from_acc = Address::new_actor(b"fromAct");
    let to_acc = Address::new_actor(b"toAct");

    mock.set_account_addr(from_acc, from).await;
    mock.set_account_addr(to_acc, to).await;

    let act = ActorState {
        code: *actor::actorv2::ACCOUNT_ACTOR_CODE_ID,
        state: cid::new_from_cbor(&[], cid::Code::Identity),
        sequence: 0,
        balance: 20.into(),
    };

    let empty_arr_cid = actor::actorv2::ipld_amt::Amt::<actor::paych::LaneState, _>::new(mock.bs()).flush().unwrap();
    let pch_st = actor::actorv2::paych::State::new(from_acc, to_acc, empty_arr_cid);
    mock.set_paych_state(ch, (act, actor::paych::State::V2(pch_st))).await;


    // Add a voucher
    let nonce  = 1;
    let voucher_lane = 1;
    let min_delta = BigInt::from(0);
    let voucher_amount = BigInt::from(2);
    let sv = actor::paych::SignedVoucher::V2(create_test_voucher(ch, voucher_lane, nonce, voucher_amount.clone(),Some(&from_key_private)));

    // Should fail because there is no wallet key matching the channel To
	// address (ie, the channel is not "owned" by this node)
    assert!(pmgr.add_voucher_inbound(ch, sv, Vec::new(), min_delta.clone()).await.is_err());

    // Add wallet key for To address
    mock.add_wallet_addr(to).await;

    // Add voucher again
    let sv = actor::paych::SignedVoucher::V2(create_test_voucher(ch, voucher_lane, nonce, voucher_amount.clone(),Some(&from_key_private)));
    pmgr.add_voucher_inbound(ch, sv, Vec::new(), min_delta.clone()).await.unwrap();
}

// #[async_std::test]
// async fn test_best_spendable() {
//     let mut s = create_manager_with_channel().await;

//     // Add vouchers to lane 1 with amounts: [1, 2, 3]
//     let mut nonce  = 1;
//     let voucher_lane = 1;
//     let min_delta = BigInt::from(0);
//     let voucher_amount = BigInt::from(1);

//     let sv_l1_v1 = actor::paych::SignedVoucher::V2(create_test_voucher(s.ch, voucher_lane, nonce, voucher_amount.clone(),Some(&s.from_key_private)));
//     s.mgr.add_voucher_inbound(s.ch, sv_l1_v1, Vec::new(), min_delta.clone()).await.unwrap();

//     nonce+=1;
//     let voucher_amount = BigInt::from(2);
//     let sv_l1_v2 = actor::paych::SignedVoucher::V2(create_test_voucher(s.ch, voucher_lane, nonce, voucher_amount.clone(),Some(&s.from_key_private)));
//     s.mgr.add_voucher_inbound(s.ch, sv_l1_v2, Vec::new(), min_delta.clone()).await.unwrap();

//     nonce+=1;
//     let voucher_amount = BigInt::from(3);
//     let sv_l1_v3 = actor::paych::SignedVoucher::V2(create_test_voucher(s.ch, voucher_lane, nonce, voucher_amount.clone(),Some(&s.from_key_private)));
//     s.mgr.add_voucher_inbound(s.ch, sv_l1_v3, Vec::new(), min_delta.clone()).await.unwrap();


//     // Add voucher to lane 2 with amounts: [2]
//     let voucher_lane = 2;
//     let nonce = 1;
//     let voucher_amount = BigInt::from(2);
//     let sv_l2_v1 =  actor::paych::SignedVoucher::V2(create_test_voucher(s.ch, voucher_lane, nonce, voucher_amount.clone(),Some(&s.from_key_private)));
//     s.mgr.add_voucher_inbound(s.ch, sv_l2_v1, Vec::new(), min_delta.clone()).await.unwrap();


// }

#[async_std::test]
async fn test_check_spendable() {
    let mut s = create_manager_with_channel().await;

    // Create a voucher with Extra
    let voucher_lane = 1;
    let nonce = 1;
    let voucher_amount = BigInt::from(1);
    let voucher =  actor::paych::SignedVoucher::V2(create_test_voucher_with_extra(s.ch, voucher_lane, nonce, voucher_amount.clone(),Some(&s.from_key_private)));

    // Add Voucher
    let min_delta = BigInt::from(0);
    s.mgr.add_voucher_inbound(s.ch, voucher.clone(), Vec::new(), min_delta.clone()).await.unwrap();

    // None of the values here matter except for msg_rct's exit code which is 0
    let success_response = InvocResult{
        msg: UnsignedMessage::builder().from(Address::new_id(1111)).to(Address::new_id(1112)).build().unwrap(),
        msg_rct: Some(MessageReceipt{
            exit_code: vm::ExitCode::Ok,
            return_data: Default::default(),
            gas_used: 0,
            
        }),
        error: None,
        
    };
    s.mgr.api.set_call_response(success_response).await;

    // Check that spendable is true
    let secret = b"secret";
    let spendable = s.mgr.check_voucher_spendable(s.ch, voucher.clone(), secret.to_vec(), Vec::new()).await.unwrap();
    assert_eq!(spendable, true);

    // Check that the secret was passed through correctly
    let last_call = s.mgr.api.get_last_call().await.unwrap();
    let p: actor::actorv2::paych::UpdateChannelStateParams = last_call.params.deserialize().unwrap();
    assert_eq!(&p.secret, secret);

    // Check that if VM call returns non-success exit code, spendable is false
       // None of the values here matter except for msg_rct's exit code which is non 0
       let success_response = InvocResult{
        msg: UnsignedMessage::builder().from(Address::new_id(1111)).to(Address::new_id(1112)).build().unwrap(),
        msg_rct: Some(MessageReceipt{
            exit_code: vm::ExitCode::SysErrSenderInvalid,
            return_data: Default::default(),
            gas_used: 0,
            
        }),
        error: None,
        
    };
    s.mgr.api.set_call_response(success_response).await;
    let spendable = s.mgr.check_voucher_spendable(s.ch, voucher.clone(), secret.to_vec(), Vec::new()).await.unwrap();
    assert_eq!(spendable, false);

    // Check that voucher is no longer spendable once it has been submitted
    s.mgr.submit_voucher(s.ch, voucher.clone(), Default::default(), Default::default()).await.unwrap();

    let spendable = s.mgr.check_voucher_spendable(s.ch, voucher, secret.to_vec(), Vec::new()).await.unwrap();
    assert_eq!(spendable, false);
}

#[async_std::test]
async fn test_submit_voucher() {
    let mut s = create_manager_with_channel().await;

    let voucher_lane = 1;
    let mut nonce = 1;
    let voucher_amount = BigInt::from(1);
    let voucher =  actor::paych::SignedVoucher::V2(create_test_voucher_with_extra(s.ch, voucher_lane, nonce, voucher_amount.clone(),Some(&s.from_key_private)));

    // Add voucher
    let min_delta = BigInt::from(0);
    s.mgr.add_voucher_inbound(s.ch, voucher.clone(), Vec::new(), min_delta.clone()).await.unwrap();

    // Submit voucher
    let secret = b"secret";
    let submit_cid = s.mgr.submit_voucher(s.ch, voucher.clone(), secret, Default::default()).await.unwrap();

    // Check that the secret was passed through correctly
    let msg = s.mgr.api.pushed_messages(&submit_cid).await.unwrap();
    let p: actor::actorv2::paych::UpdateChannelStateParams = msg.message().params.deserialize().unwrap();
    assert_eq!(&p.secret, secret); 

    // Submit a voucher without first adding it
    nonce += 1;
    let voucher_amount = BigInt::from(3);
    let secret3 = b"secret2";
    let voucher =  actor::paych::SignedVoucher::V2(create_test_voucher_with_extra(s.ch, voucher_lane, nonce, voucher_amount.clone(),Some(&s.from_key_private)));
    let submit_cid = s.mgr.submit_voucher(s.ch, voucher.clone(), secret3, Default::default()).await.unwrap();

    let msg = s.mgr.api.pushed_messages(&submit_cid).await.unwrap();
    let p3: actor::actorv2::paych::UpdateChannelStateParams = msg.message().params.deserialize().unwrap();
    assert_eq!(&p3.secret, secret3);

    // Verify that vouchers are marked as submitted
    let vis = s.mgr.list_vouchers(s.ch).await.unwrap();
    assert_eq!(vis.len(), 2)

}

#[async_std::test]
async fn test_paych_settle() {
    let expch = Address::new_id(100);
    let expch2 = Address::new_id(101);
    let from = Address::new_id(101);
    let t0 = Address::new_id(102);

    let mut pmgr = new_manager();
    let mock = pmgr.api.clone();

    let amt = BigInt::new(10);
    let mcid = pmgr.get_paych(from, to, amt).await.unwrap().mcid.unwrap();

}


// Setup begins heres. 

fn create_test_channel_response(ch: Address) -> MessageReceipt {
    let create_channel_ret = actor::actorv2::init::ExecReturn {
        id_address: ch,
        robust_address: ch,
    };
    let ccr_ser = create_channel_ret.to_vec();
}

// func testChannelResponse(t *testing.T, ch address.Address) types.MessageReceipt {
// 	createChannelRet := init2.ExecReturn{
// 		IDAddress:     ch,
// 		RobustAddress: ch,
// 	}
// 	createChannelRetBytes, err := cborrpc.Dump(&createChannelRet)
// 	require.NoError(t, err)
// 	createChannelResponse := types.MessageReceipt{
// 		ExitCode: 0,
// 		Return:   createChannelRetBytes,
// 	}
// 	return createChannelResponse
// }


struct TestScaffold {
    mgr: TestManager,
    ch: Address,
    amt: BigInt,
    from_acct: Address,
    from_key_private: Vec<u8>,
}

async fn create_manager_with_channel() -> TestScaffold {
    let pmgr = new_manager();
    let mut mock = pmgr.api.clone();
    let (from_key_private, from_key_public) = gen_key_pair();
    let (to_key_private, to_key_public) = gen_key_pair();

    let ch = Address::new_id(100);
    let from = Address::new_secp256k1(&from_key_public).unwrap();
    let to = Address::new_secp256k1(&to_key_public).unwrap();
    let from_acc = Address::new_actor(b"fromAct");
    let to_acc = Address::new_actor(b"toAct");

    mock.set_account_addr(from_acc, from).await;
    mock.set_account_addr(to_acc, to).await;

    let balance = BigInt::from(20);
    let act = ActorState {
        code: *actor::actorv2::ACCOUNT_ACTOR_CODE_ID,
        state: cid::new_from_cbor(&[], cid::Code::Identity),
        sequence: 0,
        balance: balance.clone(),
    };
    let empty_arr_cid = actor::actorv2::ipld_amt::Amt::<actor::paych::LaneState, _>::new(mock.bs()).flush().unwrap();

    let pch_st = actor::actorv2::paych::State::new(from_acc, to_acc, empty_arr_cid);

    mock.set_paych_state(ch, (act, actor::paych::State::V2(pch_st))).await;

    let mut ci = ChannelInfo::builder().channel(Some(ch)).control(from_acc).target(to_acc).direction(DIR_OUTBOUND).build().unwrap();
    pmgr.store.write().await.put_channel_info(&mut ci).await.unwrap();
    mock.add_signing_key(from_key_private.clone()).await;
    return TestScaffold {
        mgr: pmgr,
        ch: ch,
        amt: balance,
        from_acct: from_acc,
        from_key_private: from_key_private,
    }
}

fn create_test_voucher(ch: Address, voucher_lane: u64, nonce: u64, amount: BigInt, key: Option<&[u8]>) -> actor::actorv2::paych::SignedVoucher {
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
    if let Some(key) = key {
        let sig = wallet::sign(SignatureType::Secp256k1, key, &signing_bytes).unwrap();
        sv.signature = Some(sig);
    }
    sv
}

fn create_test_voucher_with_extra(ch: Address, voucher_lane: u64, nonce: u64, amount: BigInt, key: Option<&[u8]>) -> actor::actorv2::paych::SignedVoucher {
    let mut sv = actor::actorv2::paych::SignedVoucher {
        channel_addr: ch,
        time_lock_min: Default::default(),
        time_lock_max: Default::default(),
        secret_pre_image: Default::default(),
        extra: Some(ModVerifyParams {
            actor: Address::new_actor(b"act"),
            method: 0,
            data: Default::default(),
            
        }),
        lane: voucher_lane,
        nonce: nonce,
        amount: amount,
        min_settle_height: Default::default(),
        merges: Default::default(),
        signature: Default::default(),
    };
    let signing_bytes = sv.signing_bytes().unwrap();
    if let Some(key) = key {
        let sig = wallet::sign(SignatureType::Secp256k1, key, &signing_bytes).unwrap();
        sv.signature = Some(sig);
    }
    sv
}




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