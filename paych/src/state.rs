// Copyright 2020 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use super::Error;
use crate::{ChannelInfo, DIR_INBOUND, DIR_OUTBOUND, PaychProvider};
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
pub struct StateAccessor<BS, P> {
    pub bs: BS,
    pub api: Arc<P>,
}

impl<BS, P> StateAccessor<BS, P>
where
    BS: BlockStore+ Send + Sync + 'static,
    P: PaychProvider + Send + Sync + 'static,
{
    /// Returns ActorState of provided address
    // TODO ask about CID default?
    pub async fn load_paych_state(&self, ch: &Address) -> Result<(ActorState, PaychState), Error> {
        self.api.get_paych_state(*ch, None).map_err(|e| Error::Other(e.to_string()))
    }
    /// Returns channel info of provided address
    pub async fn load_state_channel_info(
        &self,
        ch: Address,
        dir: u8,
    ) -> Result<ChannelInfo, Error> {
        let (_, st) = self.load_paych_state(&ch).await?;

        let from = self.api.resolve_to_key_address(st.from(), None).map_err(|e| Error::Other(e.to_string()))?;
        let to = self.api.resolve_to_key_address(st.to(), None).map_err(|e| Error::Other(e.to_string()))?;

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
        let lane_count = st
            .lane_count(&self.bs)
            .map_err(|e| Error::Other(e.to_string()))?;
        if lane_count == 0 {
            return Ok(0);
        }
        let mut max_id = 0;
        st.for_each_lane_state(&self.bs, |idx: u64, _| {
            if idx > max_id {
                max_id = idx;
            }
            Ok(())
        })
        .map_err(|e| Error::Encoding(format!("failed to iterate over values in AMT: {}", e)))?;
        Ok(max_id + 1)
    }
}
