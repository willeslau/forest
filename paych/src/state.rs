// Copyright 2020 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use super::Error;
use crate::{ChannelInfo, DIR_INBOUND, DIR_OUTBOUND};
use actor::account::State as AccountState;
use actor::paych::State as PaychState;
use address::Address;
use async_std::sync::{Arc, RwLock};
use blockstore::BlockStore;
use cid::Cid;
use ipld_amt::Amt;
use state_manager::StateManager;
use vm::ActorState;

/// Thread safe access to state manager
pub struct StateAccessor<DB> {
    pub sm: Arc<RwLock<Arc<StateManager<DB>>>>,
}

impl<DB> StateAccessor<DB>
where
    DB: BlockStore + Send + Sync + 'static,
{
    /// Returns ActorState of provided address
    // TODO ask about CID default?
    pub async fn load_paych_state(&self, ch: &Address) -> Result<(ActorState, PaychState), Error> {
        let sm = self.sm.read().await;

        let actor = sm
            // This should not be Cid::default(). It should be the heaviest tipset. Its nil in lotus because it checks
            // if its nil, and if it is, it uses the heaviest tipset.
            .get_actor(ch, &Cid::default())
            .map_err(|err| Error::Other(err.to_string()))?
            .ok_or_else(|| Error::Other("could not find actor".to_string()))?;
        let state: PaychState = PaychState::load(sm.blockstore(), &actor)
            .map_err(|err| Error::Other(err.to_string()))?;
        Ok((actor, state))
    }
    /// Returns channel info of provided address
    pub async fn load_state_channel_info(
        &self,
        ch: Address,
        dir: u8,
    ) -> Result<ChannelInfo, Error> {
        let (_, st) = self.load_paych_state(&ch).await?;
        let sm = self.sm.read().await;

        // Load channel 'from' account actor state
        let account_from: AccountState = sm
            .load_actor_state(&st.from(), &Cid::default())
            .map_err(|err| Error::Other(err.to_string()))?;
        let from = account_from.pubkey_address();

        // Load channel 'to' account actor state
        let account_to: AccountState = sm
            .load_actor_state(&st.to(), &Cid::default())
            .map_err(|err| Error::Other(err.to_string()))?;
        let to = account_to.pubkey_address();

        let next_lane = self.next_lane_from_state(st).await?;
        if dir == DIR_INBOUND {
            let ci = ChannelInfo::builder()
                .next_lane(next_lane)
                .direction(dir)
                .control(to)
                .target(from)
                .build()
                .map_err(Error::Other)?;
            Ok(ci)
        } else if dir == DIR_OUTBOUND {
            let ci = ChannelInfo::builder()
                .next_lane(next_lane)
                .direction(dir)
                .control(from)
                .target(to)
                .build()
                .map_err(Error::Other)?;
            Ok(ci)
        } else {
            Err(Error::Other("invalid Direction".to_string()))
        }
    }
    async fn next_lane_from_state(&self, st: PaychState) -> Result<u64, Error> {
        // let sm = self.sm.read().await;
        // let store = sm.blockstore();
        // let lane_states: Amt<u64, _> =
        //     Amt::load(&st.lane_states, store).map_err(|e| Error::Other(e.to_string()))?;
        // let mut max_id: u64 = 0;

        // lane_states
        //     .for_each(|i: u64, _| {
        //         if i > max_id {
        //             max_id = i
        //         }
        //         Ok(())
        //     })
        //     .map_err(|e| Error::Encoding(format!("failed to iterate over values in AMT: {}", e)))?;

        // Ok(max_id + 1)
        let lane_count = st
            .lane_count(self.sm.read().await.blockstore())
            .map_err(|e| Error::Other(e.to_string()))?;
        if lane_count == 0 {
            return Ok(0);
        }
        let mut max_id = 0;
        st.for_each_lane_state(self.sm.read().await.blockstore(), |idx: u64, _| {
            if idx > max_id {
                max_id = idx;
            }
            Ok(())
        })
        .map_err(|e| Error::Encoding(format!("failed to iterate over values in AMT: {}", e)))?;
        Ok(max_id + 1)
    }
}
