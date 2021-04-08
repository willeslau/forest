// Copyright 2020 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use address::Address;
use async_std::sync::{Arc, RwLock};
use blocks::{Tipset, TipsetKeys};
use blockstore::BlockStore;
use chain::ChainStore;
use cid::Cid;
use crypto::Signature;
use fil_types::{NetworkVersion, verifier::ProofVerifier};
use message::{MessageReceipt, SignedMessage, UnsignedMessage};
use message_pool::{MessagePool, MpoolRpcProvider};
use state_manager::{StateCallResult, StateManager};
use std::error::Error;
use vm::ActorState;
use wallet::Wallet;

use crate::StateAccessor;
trait PaychProvider {
    /// message_pool
    // StateAccountKey(context.Context, address.Address, types.TipSetKey) (address.Address, error)
    // StateWaitMsg(ctx context.Context, msg cid.Cid, confidence uint64) (*api.MsgLookup, error)
    // MpoolPushMessage(ctx context.Context, msg *types.Message, maxFee *api.MessageSendSpec) (*types.SignedMessage, error)
    // WalletHas(ctx context.Context, addr address.Address) (bool, error)
    // WalletSign(ctx context.Context, k address.Address, msg []byte) (*crypto.Signature, error)
    // StateNetworkVersion(context.Context, types.TipSetKey) (network.Version, error)

    // API
    fn state_account_key(
        &self,
        addr: Address,
        ts_key: TipsetKeys,
    ) -> Result<Address, Box<dyn Error>>;
    fn state_wait_msg(&self, msg: Cid, confidence: u64) -> Result<(Option<Arc<Tipset>>, Option<MessageReceipt>), Box<dyn Error>>;
    fn mpool_push_message(
        &self,
        msg: UnsignedMessage,
        // max_fee: Option<MessageSendSpec>,
    ) -> Result<SignedMessage, Box<dyn Error>>;
    fn wallet_has(&self, addr: Address) -> Result<bool, Box<dyn Error>>;
    fn wallet_sign(&self, k: Address, msg: &[u8]) -> Result<Signature, Box<dyn Error>>;
    fn state_network_version(&self, ts_key: TipsetKeys) -> Result<NetworkVersion, Box<dyn Error>>;

    
    fn resolve_to_key_address(&self, addr: Address, ts: Tipset) -> Result<Address, Box<dyn Error>>;
    fn get_paych_state(
        &self,
        addr: Address,
        ts: Tipset,
    ) -> Result<(ActorState, actor::paych::State), Box<dyn Error>>;
    fn call<V: ProofVerifier>(
        &self,
        message: &mut UnsignedMessage,
        tipset: Option<Arc<Tipset>>,
    ) -> StateCallResult;
}
pub struct DefaultPaychProvider<DB, KS> {
    pub sm: Arc<StateManager<DB>>,
    pub cs: Arc<ChainStore<DB>>,
    pub keystore: Arc<RwLock<KS>>,
    pub mpool: Arc<MessagePool<MpoolRpcProvider<DB>>>,
    pub sa: StateAccessor<DB>,
    pub wallet: Arc<RwLock<Wallet<KS>>>,
}
