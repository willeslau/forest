use std::collections::HashMap;

use address::Address;
use async_std::sync::RwLock;
use async_trait::async_trait;
use blockstore::BlockStore;

use crate::provider::PaychProvider;
use actor::paych::State as PaychState;
use encoding::from_slice;
use encoding::to_vec;
use vm::{ActorState, Serialized};
struct TestPaychProvider {
    //	lk           sync.Mutex
    // accountState map[address.Address]address.Address
    // paychState   map[address.Address]mockPchState
    // response     *api.InvocResult
    // lastCall     *types.Message
    account_state: RwLock<HashMap<Address, Address>>,
    paych_state: RwLock<HashMap<Address, (ActorState, PaychState)>>,
}

impl TestPaychProvider {
    async fn set_account_addr(&mut self, a: Address, lookup: Address) {
        self.account_state.write().await.insert(a, lookup);
    }
    async fn set_paych_state(&mut self, a: Address, state: (ActorState, PaychState)) {
        self.paych_state.write().await.insert(a, state);
    }
}

#[async_trait]
impl<BS: BlockStore + Send + Sync + 'static> PaychProvider<BS> for TestPaychProvider {
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
        // max_fee: Option<MessageSendSpec>,
    ) -> Result<message::SignedMessage, Box<dyn std::error::Error>> {
        todo!()
    }

    async fn wallet_has(&self, addr: address::Address) -> Result<bool, Box<dyn std::error::Error>> {
        todo!()
    }

    async fn wallet_sign<V: fil_types::verifier::ProofVerifier + Send + Sync + 'static>(
        &self,
        k: address::Address,
        msg: &[u8],
    ) -> Result<crypto::Signature, Box<dyn std::error::Error>> {
        todo!()
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
        ts: Option<blocks::Tipset>,
    ) -> Result<(vm::ActorState, actor::paych::State), Box<dyn std::error::Error>> {
        let st_map = async_std::task::block_on(self.paych_state.read());
        let st = st_map.get(&addr);
        match st {
            Some(st) => {
                let act_state = st.0.clone();
                // TODO: I know this is janky, but to derive clone and the PaychState means to re-release all the actors crates again.
                let paych_state = to_vec(&st.1).map_err(|e| {
                    Box::new(format!(
                        "get_paych_state serialize failed: {}",
                        e.to_string()
                    ))
                })?;
                let paych_state: PaychState = from_slice(&paych_state).map_err(|e| {
                    format!("get_paych_state deserialize failed: {}", e.to_string()).into()
                })?;
                Ok((act_state, paych_state))
            }
            None => Err(format!("cant find address when resolving {:?}", addr).into()),
        }
    }

    fn call<V: fil_types::verifier::ProofVerifier>(
        &self,
        message: &mut message::UnsignedMessage,
        tipset: Option<std::sync::Arc<blocks::Tipset>>,
    ) -> state_manager::StateCallResult {
        todo!()
    }

    fn bs(&self) -> &BS {
        todo!()
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
