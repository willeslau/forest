use std::{collections::{HashMap, HashSet}};

use address::Address;
use async_std::sync::RwLock;
use async_trait::async_trait;
use blockstore::BlockStore;
use cid::Cid;
use crypto::{Signature, Signer};
use message::{SignedMessage, UnsignedMessage};
use num_traits::sign;
use state_manager::InvocResult;

use crate::provider::PaychProvider;
use actor::paych::State as PaychState;
use encoding::{Cbor, from_slice};
use encoding::to_vec;
use vm::{ActorState, Serialized};
use std::error::Error;
use forest_db::MemoryDB;
use wallet::sign;
struct DummySigner;
const DUMMY_SIG: [u8; 1] = [0u8];
impl Signer for DummySigner {
    fn sign_bytes(&self, _: &[u8], _: &Address) -> Result<Signature, Box<dyn Error>> {
        Ok(Signature::new_secp256k1(DUMMY_SIG.to_vec()))
    }
}

struct TestPaychProvider<BS> {
    //	lk           sync.Mutex
    // accountState map[address.Address]address.Address
    // paychState   map[address.Address]mockPchState
    // response     *api.InvocResult
    // lastCall     *types.Message
    account_state: RwLock<HashMap<Address, Address>>,
    paych_state: RwLock<HashMap<Address, (ActorState, PaychState)>>,
    response: RwLock<Option<InvocResult>>,
    last_call: RwLock<Option<UnsignedMessage>>,
    
    bs: BS,

    //  messages         map[cid.Cid]*types.SignedMessage
    // 	waitingCalls     map[cid.Cid]*waitingCall
    // 	waitingResponses map[cid.Cid]*waitingResponse
    // 	wallet           map[address.Address]struct{}
    // 	signingKey       []byte
    // }
    messages: RwLock<HashMap<Cid, SignedMessage>>,
    wallet: RwLock<HashSet<Address>>,
    signing_key: Vec<u8>,
    
}

impl<BS: BlockStore + Send +Sync + 'static> TestPaychProvider<BS> {
    fn new(bs: BS) -> Self{ TestPaychProvider {
        bs: bs,
        account_state: Default::default(),
        paych_state: Default::default(),
        response: Default::default(),
        last_call: Default::default(),
        messages: Default::default(),
        wallet: Default::default(),
        signing_key: Default::default(),
        
    } }
    async fn set_account_addr(&mut self, a: Address, lookup: Address) {
        self.account_state.write().await.insert(a, lookup);
    }
    async fn set_paych_state(&mut self, a: Address, state: (ActorState, PaychState)) {
        self.paych_state.write().await.insert(a, state);
    }
    async fn set_call_response(&mut self, resp: InvocResult) {
        *self.response.write().await = Some(resp);
    }
    async fn get_last_call() {
        unimplemented!()
    }
}

#[async_trait]
impl<BS: BlockStore + Send + Sync + 'static> PaychProvider<BS> for TestPaychProvider<BS> {

    fn state_account_key(
        &self,
        addr: address::Address,
        ts_key: blocks::TipsetKeys,
    ) -> Result<address::Address, Box<dyn std::error::Error>> {
        todo!()
    }

    async fn state_wait_msg(
        &self,
        msg: cid::Cid,
        confidence: i64,
    ) -> Result<
        (
            Option<std::sync::Arc<blocks::Tipset>>,
            Option<message::MessageReceipt>,
        ),
        Box<dyn std::error::Error>,
    > {
        todo!()
    }

    async fn mpool_push_message<V: fil_types::verifier::ProofVerifier + Send + Sync + 'static>(
        &self,
        msg: message::UnsignedMessage,
    ) -> Result<message::SignedMessage, Box<dyn std::error::Error>> {

        let smsg = SignedMessage::new(msg, &DummySigner)?;
        self.messages.write().await.insert(smsg.cid()?, smsg.clone());
        Ok(smsg)
    }

    async fn wallet_has(&self, addr: Address) -> Result<bool, Box<dyn std::error::Error>> {
        Ok(self.wallet.read().await.contains(&addr))
    }

    async fn wallet_sign<V: fil_types::verifier::ProofVerifier + Send + Sync + 'static>(
        &self,
        k: address::Address,
        msg: &[u8],
    ) -> Result<crypto::Signature, Box<dyn std::error::Error>> {
        Ok(sign(crypto::SignatureType::Secp256k1, &self.signing_key, msg)?)
    }

    async fn state_network_version(
        &self,
        ts_key: Option<blocks::TipsetKeys>,
    ) -> Result<fil_types::NetworkVersion, Box<dyn std::error::Error>> {
        todo!()
    }

    fn resolve_to_key_address(
        &self,
        addr: address::Address,
        ts: Option<blocks::Tipset>,
    ) -> Result<address::Address, Box<dyn std::error::Error>> {
        let addr_map = async_std::task::block_on(self.account_state.read());
        let addr = addr_map.get(&addr);
        match addr {
            Some(a) => Ok(*a),
            None => Err(format!("cant find address when resolving {:?}", addr).into()),
        }
    }

    fn get_paych_state(
        &self,
        addr: address::Address,
        _ts: Option<blocks::Tipset>,
    ) -> Result<(vm::ActorState, actor::paych::State), Box<dyn std::error::Error>> {
        let st_map = async_std::task::block_on(self.paych_state.read());
        let st = st_map.get(&addr);
        match st {
            Some(st) => {
                let act_state = st.0.clone();
                // TODO: I know this is janky, but to derive clone and the PaychState means to re-release all the actors crates again.
                let paych_state = to_vec(&st.1).map_err(|e| {
                    format!(
                        "get_paych_state serialize failed: {}",
                        e.to_string()
                    )
                })?;
                let paych_state: PaychState = from_slice(&paych_state).map_err(|e| {
                    format!("get_paych_state deserialize failed: {}", e.to_string())
                })?;
                Ok((act_state, paych_state))
            }
            None => Err(format!("cant find address when resolving {:?}", addr).into()),
        }
    }

    fn call<V: fil_types::verifier::ProofVerifier>(
        &self,
        message: &mut message::UnsignedMessage,
        _tipset: Option<std::sync::Arc<blocks::Tipset>>,
    ) -> state_manager::StateCallResult {
        let mut last_call = async_std::task::block_on(self.last_call.write());
        *last_call = Some(message.clone());
        let resp = (*async_std::task::block_on(self.response.read())).as_ref().unwrap().clone();
        Ok(resp)
    }

    fn bs(&self) -> &BS {
        &self.bs
    }

    async fn load_state_channel_info(
        &self,
        ch: address::Address,
        dir: u8,
    ) -> Result<crate::ChannelInfo, Box<dyn std::error::Error>> {
        todo!()
    }

    fn next_lane_from_state(
        &self,
        st: actor::paych::State,
    ) -> Result<u64, Box<dyn std::error::Error>> {
        todo!()
    }
}
