// Copyright 2020 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use crate::StateAccessor;
use address::{Address, Protocol};
use async_std::sync::{Arc, RwLock};
use async_trait::async_trait;
use blocks::{Tipset, TipsetKeys};
use blockstore::BlockStore;
use chain::ChainStore;
use cid::Cid;
use crypto::Signature;
use fil_types::{verifier::ProofVerifier, NetworkVersion};
use message::{MessageReceipt, SignedMessage, UnsignedMessage};
use message_pool::{MessagePool, MpoolRpcProvider};
use state_manager::{StateCallResult, StateManager};
use std::error::Error;
use vm::ActorState;
use wallet::{KeyStore, Wallet};

#[async_trait]
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
    async fn state_wait_msg(
        &self,
        msg: Cid,
        confidence: i64,
    ) -> Result<(Option<Arc<Tipset>>, Option<MessageReceipt>), Box<dyn Error>>;
    async fn mpool_push_message<V: ProofVerifier + Send + Sync + 'static>(
        &self,
        msg: UnsignedMessage,
        // max_fee: Option<MessageSendSpec>,
    ) -> Result<SignedMessage, Box<dyn Error>>;
    async fn wallet_has(&self, addr: Address) -> Result<bool, Box<dyn Error>>;
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

#[async_trait]
impl<DB, KS> PaychProvider for DefaultPaychProvider<DB, KS>
where
    DB: BlockStore + Sync + Send + 'static,
    KS: KeyStore + Sync + Send + 'static,
{
    fn state_account_key(
        &self,
        addr: Address,
        ts_key: TipsetKeys,
    ) -> Result<Address, Box<dyn Error>> {
        todo!()
    }

    async fn state_wait_msg(
        &self,
        msg: Cid,
        confidence: i64,
    ) -> Result<(Option<Arc<Tipset>>, Option<MessageReceipt>), Box<dyn Error>> {
        Ok(self.sm.wait_for_message(msg, confidence).await?)
    }

    async fn mpool_push_message<V>(
        &self,
        umsg: UnsignedMessage,
        // max_fee: Option<MessageSendSpec>,
    ) -> Result<SignedMessage, Box<dyn Error>>
    where
        V: ProofVerifier + Send + Sync + 'static,
    {
        let mut umsg = umsg;
        let from = umsg.from;

        let keystore = self.keystore.as_ref().write().await;
        let heaviest_tipset = self
            .sm
            .chain_store()
            .heaviest_tipset()
            .await
            .ok_or_else(|| "Could not get heaviest tipset".to_string())?;
        let key_addr = self
            .sm
            .resolve_to_key_addr::<V>(&from, &heaviest_tipset)
            .await?;

        if umsg.sequence != 0 {
            return Err(
                "Expected nonce for MpoolPushMessage is 0, and will be calculated for you.".into(),
            );
        }
        /// TODO: Figure out how to refactor this without pulling in the RPC crate;
        // let mut umsg =
        // estimate_message_gas::<DB, KS, B, V>(&data, umsg, spec, Default::default()).await?;
        if umsg.gas_premium > umsg.gas_fee_cap {
            return Err("After estimation, gas premium is greater than gas fee cap".into());
        }

        if from.protocol() == Protocol::ID {
            umsg.from = key_addr;
        }
        let nonce = self.mpool.get_sequence(&from).await?;
        umsg.sequence = nonce;
        let key = wallet::find_key(&key_addr, &*keystore)?;
        let sig = wallet::sign(
            *key.key_info.key_type(),
            key.key_info.private_key(),
            umsg.to_signing_bytes().as_slice(),
        )?;

        let smsg = SignedMessage::new_from_parts(umsg, sig)?;

        self.mpool.as_ref().push(smsg.clone()).await?;
        Ok(smsg)
    }

    async fn wallet_has(&self, addr: Address) -> Result<bool, Box<dyn Error>> {
        let keystore = self.keystore.read().await;

        let key = wallet::find_key(&addr, &*keystore).is_ok();
        Ok(key)
    }

    fn wallet_sign(&self, k: Address, msg: &[u8]) -> Result<Signature, Box<dyn Error>> {
        todo!()
    }

    fn state_network_version(&self, ts_key: TipsetKeys) -> Result<NetworkVersion, Box<dyn Error>> {
        todo!()
    }

    fn resolve_to_key_address(&self, addr: Address, ts: Tipset) -> Result<Address, Box<dyn Error>> {
        todo!()
    }

    fn get_paych_state(
        &self,
        addr: Address,
        ts: Tipset,
    ) -> Result<(ActorState, actor::paych::State), Box<dyn Error>> {
        todo!()
    }

    fn call<V: ProofVerifier>(
        &self,
        message: &mut UnsignedMessage,
        tipset: Option<Arc<Tipset>>,
    ) -> StateCallResult {
        todo!()
    }
}
