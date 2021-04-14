// Copyright 2020 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use crate::{ChannelInfo, DIR_INBOUND, DIR_OUTBOUND};
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
use std::{convert::TryFrom, error::Error};
use vm::ActorState;
use wallet::{Key, KeyStore, Wallet};

#[async_trait]
pub trait PaychProvider<BS: BlockStore + Send + Sync + 'static> {
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
    async fn wallet_sign<V: ProofVerifier + Send + Sync + 'static>(&self, k: Address, msg: &[u8]) -> Result<Signature, Box<dyn Error>>;
    async fn state_network_version(&self, ts_key: Option<TipsetKeys>) -> Result<NetworkVersion, Box<dyn Error>>;

    fn resolve_to_key_address(&self, addr: Address, ts: Option<Tipset>) -> Result<Address, Box<dyn Error>>;
    fn get_paych_state(
        &self,
        addr: Address,
        ts: Option<Tipset>,
    ) -> Result<(ActorState, actor::paych::State), Box<dyn Error>>;
    fn call<V: ProofVerifier>(
        &self,
        message: &mut UnsignedMessage,
        tipset: Option<Arc<Tipset>>,
    ) -> StateCallResult;

    // BlockStore
    fn bs(&self) -> &BS;
    

    // State accessor
    async fn load_state_channel_info(
        &self,
        ch: Address,
        dir: u8,
    ) -> Result<ChannelInfo, Box<dyn Error>>;
    fn next_lane_from_state(&self, st:  actor::paych::State) -> Result<u64, Box<dyn Error>>;


}
pub struct DefaultPaychProvider<DB, KS> {
    pub sm: Arc<StateManager<DB>>,
    pub cs: Arc<ChainStore<DB>>,
    pub keystore: Arc<RwLock<KS>>,
    pub mpool: Arc<MessagePool<MpoolRpcProvider<DB>>>,
    pub wallet: Arc<RwLock<Wallet<KS>>>,
}

#[async_trait]
impl<DB, KS> PaychProvider<DB> for DefaultPaychProvider<DB, KS>
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

    async fn wallet_sign<V>(&self, addr: Address, msg: &[u8]) -> Result<Signature, Box<dyn Error>> 
    where
    V: ProofVerifier + Send + Sync + 'static,{
        let heaviest_tipset = self
            .sm
            .chain_store()
            .heaviest_tipset()
            .await
            .ok_or_else(|| "Could not get heaviest tipset".to_string())?;
        let key_addr = self.sm
            .resolve_to_key_addr::<V>(&addr, &heaviest_tipset)
            .await?;
        let keystore = &mut *self.keystore.write().await;
        let key = match wallet::find_key(&key_addr, keystore) {
            Ok(key) => key,
            Err(_) => {
                let key_info = wallet::try_find(&key_addr, keystore)?;
                Key::try_from(key_info)?
            }
        };
    
        let sig = wallet::sign(
            *key.key_info.key_type(),
            key.key_info.private_key(),
            msg,
        )?;
        Ok(sig)
    }

    async fn state_network_version(&self, ts_key: Option<TipsetKeys>) -> Result<NetworkVersion, Box<dyn Error>> {
        let ts_key = if let Some(ts_key) = ts_key {
            ts_key
        } else {
            self.cs.heaviest_tipset().await.unwrap().key().clone()
        };
        let ts = self.cs.tipset_from_keys(&ts_key).await?;
        Ok(self.sm.get_network_version(ts.epoch()))
    }

    fn resolve_to_key_address(&self, addr: Address, ts: Option<Tipset>) -> Result<Address, Box<dyn Error>> {
        todo!()
    }

    fn get_paych_state(
        &self,
        addr: Address,
        ts: Option<Tipset>,
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



    // State accessor 

    /// Returns channel info of provided address
    async fn load_state_channel_info(
        &self,
        ch: Address,
        dir: u8,
    ) -> Result<ChannelInfo, Box<dyn Error>> {
        let (_, st) = self.get_paych_state(ch, None)?;

        let from = self.resolve_to_key_address(st.from(), None)?;
        let to = self.resolve_to_key_address(st.to(), None)?;

        let next_lane = self.next_lane_from_state(st)?;
        if dir == DIR_INBOUND {
            let ci = ChannelInfo::builder()
                .next_lane(next_lane)
                .direction(dir)
                .control(to)
                .target(from)
                .build()?;
            Ok(ci)
        } else if dir == DIR_OUTBOUND {
            let ci = ChannelInfo::builder()
                .next_lane(next_lane)
                .direction(dir)
                .control(from)
                .target(to)
                .build()?;
            Ok(ci)
        } else {
            Err("invalid Direction".to_string().into())
        }
    }
    fn next_lane_from_state(&self, st: actor::paych::State) -> Result<u64, Box<dyn Error>> {
        let lane_count = st
            .lane_count(self.sm.chain_store().db.as_ref())?;
        if lane_count == 0 {
            return Ok(0);
        }
        let mut max_id = 0;
        st.for_each_lane_state(self.sm.chain_store().db.as_ref(), |idx: u64, _| {
            if idx > max_id {
                max_id = idx;
            }
            Ok(())
        })?;
        Ok(max_id + 1)
    }
fn bs(&self) -> &DB { 
    self.sm.chain_store().blockstore()
 }
}
