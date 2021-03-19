// Copyright 2020 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

//! Contains routines for message selection APIs.
//! Whenever a miner is ready to create a block for a tipset, it invokes the select_messages API
//! which selects an appropriate set of messages such that it optimizes miner reward and chain capacity.
//! See https://docs.filecoin.io/mine/lotus/message-pool/#message-selection for more details

use super::{create_message_chains,test_create_message_chains, msg_pool::MessagePool};
use super::{gas_guess, provider::Provider};
use crate::msg_chain::MsgChain;
use crate::msg_pool::MsgSet;
use crate::Error;
use crate::{add_to_selected_msgs, remove_from_selected_msgs};
use address::Address;
use async_std::sync::{Arc, RwLock};
use blocks::Tipset;
use log::warn;
use message::Message;
use message::SignedMessage;
use num_bigint::BigInt;
use std::borrow::BorrowMut;
use std::cmp::Ordering;
use std::collections::HashMap;
use crate::msg_chain::MsgChainNode;
use crate::msg_chain_fixed::{MsgChainFixed, MsgChainNodeFixed};

// A cap on maximum number of message to include in a block
const MAX_BLOCK_MSGS: usize = 16000;
const MAX_BLOCKS: usize = 15;

type Pending = HashMap<Address, HashMap<u64, SignedMessage>>;

impl<T> MessagePool<T>
where
    T: Provider + Send + Sync + 'static,
{
    /// Forest employs a sophisticated algorithm for selecting messages
    /// for inclusion from the pool, given the ticket quality of a miner.
    /// This method selects messages for including in a block.
    pub async fn select_messages(&self, ts: &Tipset, tq: f64) -> Result<Vec<SignedMessage>, Error> {
        let cur_ts = self.cur_tipset.read().await.clone();
        // if the ticket quality is high enough that the first block has higher probability
        // than any other block, then we don't bother with optimal selection because the
        // first block will always have higher effective performance
        let mut msgs = if tq > 0.84 {
            self.select_messages_greedy(&cur_ts, ts).await
        } else {
            self.select_messages_optimal(&cur_ts, ts, tq).await
        }?;

        if msgs.len() > MAX_BLOCK_MSGS {
            msgs.truncate(MAX_BLOCK_MSGS)
        }

        Ok(msgs)
    }

    async fn select_messages_greedy(
        &self,
        cur_ts: &Tipset,
        ts: &Tipset,
    ) -> Result<Vec<SignedMessage>, Error> {
        let base_fee = self.api.read().await.chain_compute_base_fee(&ts)?;

        // 0. Load messages from the target tipset; if it is the same as the current tipset in
        //    the mpool, then this is just the pending messages
        let mut pending = self.get_pending_messages(&cur_ts, &ts).await?;

        if pending.is_empty() {
            return Ok(Vec::new());
        }
        // 0b. Select all priority messages that fit in the block
        // TODO: Implement guess gas
        let min_gas = 1298450;
        let (result, gas_limit) = self
            .select_priority_messages(&mut pending, &base_fee, &ts)
            .await?;

        // check if block has been filled
        if gas_limit < min_gas {
            return Ok(result);
        }
        // 1. Create a list of dependent message chains with maximal gas reward per limit consumed
        let mut chains = Vec::new();
        for (actor, mset) in pending.into_iter() {
            chains.extend(create_message_chains(&self.api, &actor, &mset, &base_fee, &ts).await?);
        }

        let (msgs, _) = merge_and_trim(chains, result, &base_fee, gas_limit, min_gas);
        Ok(msgs)
    }

    async fn select_messages_optimal(
        &self,
        current_tipset: &Tipset,
        target_tipset: &Tipset,
        ticket_quality: f64,
    ) -> Result<Vec<SignedMessage>, Error> {
        // Base fee is the sum of the gas limits of the blocks in the tipset + parent tipset's first block base fee (relocate)
        let base_fee = self
            .api
            .read()
            .await
            .chain_compute_base_fee(&target_tipset)?;
        // 0. Load messages from the target tipset; if it is the same as the current tipset in
        //    the message pool then this is just the pending messages
        //    pending messages map values are a hashmap of nonce -> signed messages (which includes the same nonce)
        let mut pending = self
            .get_pending_messages(&current_tipset, &target_tipset)
            .await?;


        if pending.is_empty() {
            return Ok(Vec::new());
        }

        // 0b. Select all priority messages that fit in the block
        let (mut result, mut gas_limit) = self
            .select_priority_messages(&mut pending, &base_fee, &target_tipset)
            .await?;

        // return if block has been filled
        if gas_limit < gas_guess::MIN_GAS {
            return Ok(result);
        }
        // 1. Create a list of dependent message chains with maximal gas reward per limit consumed
        let mut chains = MsgChainFixed::new(vec![]);
        for (actor, mset) in pending.into_iter() {
            chains.chain.extend(
                test_create_message_chains(&self.api, &actor, &mset, &base_fee, &target_tipset).await?.chain,
            );
        }
        
        // NOTE: verified messaged from both impls to be of 958 post create_message_chains
        // 2. Sort the chains by gas perf and gas reward
        // TODO check the sort order
        chains.chain.sort_by(|a, b| b.compare(&a));
        
        if !chains.chain.is_empty() && chains.chain[0].gas_perf < 0.0 {
            warn!(
                "all messages in message pool have non-positive gas performance {}",
                chains.chain[0].gas_perf
            );
            return Ok(result);
        }

        // 3. Parition chains into blocks (without trimming)
        //    we use the full BLOCK_GAS_LIMIT (as opposed to the residual `gas_limit` from the
        //    priority message selection) as we have to account for what other miners are doing
        let mut next_chain = 0;
        let mut partitions: Vec<Vec<MsgChainNodeFixed>> = vec![vec![]; MAX_BLOCKS];
        let mut i = 0;
        let chain_len = chains.chain.len();
        while i < MAX_BLOCKS && next_chain < chain_len {
            let mut gas_limit = types::BLOCK_GAS_LIMIT;
            while next_chain < chain_len {
                let chain = chains.chain[next_chain].clone();
                let chain_gas_limit = chain.gas_limit;
                next_chain += 1;
                partitions[i].push(chain);
                gas_limit -= chain_gas_limit;
                if gas_limit < gas_guess::MIN_GAS {
                    break;
                }
            }
            i += 1;
        }

        // dbg!(partitions);

        // 4. Compute effective performance for each chain, based on the partition they fall into
        //    The effective performance is the gas_perf of the chain * block probability
        let block_prob = crate::block_probabilities(ticket_quality);
        let mut eff_chains = 0;
        for i in 0..MAX_BLOCKS {
            let len = partitions[i].len().clone();
            for idx in 0..len.clone() {
                let chain = &mut partitions[i][idx];
                let prev = chain.prev.map(|s| (chains.chain[s].eff_perf, chains.chain[s].gas_limit));
                chain.set_effperf_with_block_prob(block_prob[i], prev);
            }
            eff_chains += partitions[i].len();
        }

        // set back the chains from partitions in chains
        let mut chains = vec![];
        for i in partitions {
            for j in i {
                chains.push(j);
            }
        }

        // restore back in MsgChain form
        let mut chains = MsgChainFixed::new(chains);

        // dbg!(&chains);

        // nullify the effective performance of chains that don't fit in any partition
        for chain in chains.chain.iter_mut().skip(eff_chains) {
            // TODO verify
            chain.set_null_effective_perf();
        }

        // 5. Resort the chains based on effective performance
        chains.chain.sort_by(|a, b| a.cmp_effective(b));

        // 6. Merge the head chains to produce the list of messages selected for inclusion
        //    subject to the residual gas limit
        //    When a chain is merged in, all its previous dependent chains *must* also be
        //    merged in or we'll have a broken block
        let mut last = chains.chain.len();
        for i in 0..chains.chain.len() {
            if chains.chain[i].gas_perf < 0.0 {
                break;
            }

            // has it already been merged?
            if chains.chain[i].merged {
                continue;
            }

            let mut chain_gas_limit = chains.chain[i].gas_limit;
            // compute the dependencies that must be merged and the gas limit including dependencies
            // let mut chain_gas_limit = chain.gas_limit;
            let mut chain_deps = vec![];
            // try merging any previous chains
            let mut prev_idx = i;
            // FIXME: running into a loop
            // Case: for index 1 (not merged) -> prev pointer is 477 (not merged) -> whose prev pointer is again 1
            // So we keep looping.
            loop {
                // basically loop till we find a merged chain and keep pushing them to chain_deps
                if let Some(idx) = chains.chain[prev_idx].prev {
                    if !chains.chain[idx].merged {
                        prev_idx = idx;
                        chain_deps.push(chains.chain[prev_idx].clone());
                        chain_gas_limit += chains.chain[prev_idx].gas_limit;
                    }
                } else {
                    break;
                }
            }
            
            // does it all fit in the block?
            if chain_gas_limit <= gas_limit {
                // include it together with all dependencies
                chain_deps.iter_mut().rev().for_each(|chain_node| {
                    result.extend(chain_node.msgs.clone());
                    chain_node.merged = true;
                });

                chains.chain[i].merged = true;
                // adjust the effective performance for all subsequent chains
                // FIXME: need to switch to loop above
                if let Some(next) = chains.next_mut() {
                    if next.eff_perf > 0.0 {
                        next.eff_perf += next.parent_offset;
                    }
                }

                result.extend(chains.chain[i].msgs.clone());
                gas_limit -= chain_gas_limit;

                // re-sort to account for already merged chains and effective performance adjustments
                // the sort *must* be stable or we end up getting negative gasPerfs pushed up.
                chains.chain[i + 1..].sort_by(|a, b| a.cmp_effective(b));

                continue;
            }

            // we can't fit this chain and its dependencies because of block gasLimit -- we are
            // at the edge
            last = i;
            break;
        }

        dbg!(gas_limit);
        dbg!(last);
        dbg!(chains.chain.len());
        // 7. We have reached the edge of what can fit wholesale; if we still hae available
        //    gas_limit to pack some more chains, then trim the last chain and push it down.
        //    Trimming invalidaates subsequent dependent chains so that they can't be selected
        //    as their dependency cannot be (fully) included.
        //    We do this in a loop because the blocks might have been inordinately large and
        //    we might have to do it multiple times to satisfy tail packing
        'tail_loop: while gas_limit >= gas_guess::MIN_GAS && last < chains.chain.len() {
            // trim if necessary
            if chains.chain[last].gas_limit > gas_limit {
                chains.trim(gas_limit, &base_fee, last);
            }

            // push down if it hasn't been invalidated
            if chains.chain[last].valid {
                for i in last..chains.chain.len() - 1 {
                    // TODO do we check for equality?
                    if chains.chain[i].cmp_effective(&chains.chain[i + 1]) == Ordering::Equal {
                        break;
                    }

                    chains.chain.swap(i, i + 1);
                }
            }

            // select the next (valid and fitting) chain and its dependencies for inclusion
            for i in last..chains.chain.len() {
                // let chain = &mut chains.chain[i];
                // has the chain been invalidated?
                if !chains.chain[i].valid {
                    continue;
                }

                // has it already been merged?
                if chains.chain[i].merged {
                    continue;
                }

                // if gasPerf < 0 we have no more profitable chains
                if chains.chain[i].gas_perf < 0.0 {
                    break 'tail_loop;
                }

                // compute the dependencies that must be merged and the gas limit including deps
                let mut chain_gas_limit = chains.chain[i].gas_limit;
                let mut dep_gas_limit: i64 = 0;
                let mut chain_deps = vec![];
                // FIXME: this needs fix as well
                {
                    while let Some(chain_node) = chains.move_backward() {
                        if !chain_node.merged {
                            chain_deps.push(chain_node.clone());
                            chain_gas_limit += chain_node.gas_limit;
                            dep_gas_limit += chain_node.gas_limit;
                        }
                    }
                }

                // does it all fit in a block
                if chain_gas_limit <= gas_limit {
                    // include it together with all dependencies
                    for i in (0..=chain_deps.len() - 1).rev() {
                        if let Some(cur_chain) = chain_deps.get_mut(i) {
                            cur_chain.merged = true;
                            result.extend(cur_chain.msgs.clone());
                        }
                    }

                    chains.chain[i].merged = true;
                    result.extend(chains.chain[i].msgs.clone());
                    gas_limit -= chain_gas_limit;
                    continue;
                }

                // it doesn't all fit; now we have to take into account the dependent chains before
                // making a decision about trimming or invalidating.
                // if the dependencies exceed the gas limit, then we must invalidate the chain
                // as it can never be included.
                // Otherwise we can just trim and continue
                if dep_gas_limit > gas_limit {
                    chains.invalidate_next_nodes(i);
                    last += i + 1;
                    continue 'tail_loop;
                }

                // dependencies fit, just trim it
                chains.trim(gas_limit - dep_gas_limit, &base_fee, i);
                last += 1;
                continue 'tail_loop;
            }

            // the merge loop ended after processing all the chains and we probably
            // have still gas to spare; end the loop
            break;
        }

        // if we have gasLimit to spare, pick some random (non-negative) chains to fill the block
        // we pick randomly so that we minimize the probability of duplication among all miners
        if gas_limit >= gas_guess::MIN_GAS {
            let mut random_count = 0;
            // shuffle to get any random chain
            crate::msg_chain_fixed::shuffle_chains(&mut chains.chain);

            for i in 0..chains.chain.len() {
                // let chain = &mut chains[i];
                // have we filled the block
                if gas_limit < gas_guess::MIN_GAS {
                    break;
                }

                // has it been merged or invalidated?
                if chains.chain[i].merged || !chains.chain[i].valid {
                    continue;
                }

                // is it negative
                if chains.chain[i].gas_perf < 0.0 {
                    continue;
                }

                let mut chain_gas_limit = chains.chain[i].gas_limit;
                let mut dep_gas_limit: i64 = 0;
                let mut chain_deps = vec![];

                while let Some(chain_node) = chains.move_backward() {
                    if !chain_node.merged {
                        chain_deps.push(chain_node.clone());
                        chain_gas_limit += chain_node.gas_limit;
                        dep_gas_limit += chain_node.gas_limit;
                    }
                }

                // do the deps fit? if the deps won't fit, invalidate the chain
                if dep_gas_limit > gas_limit {
                    // chains.chain[i].invalidate();
                    continue;
                }

                // do they fit as is? if it doesn't, trim to make it fit if possible
                if chain_gas_limit > gas_limit {
                    chains.trim(gas_limit - dep_gas_limit, &base_fee, i);

                    if !chains.chain[i].valid {
                        continue;
                    }
                }

                // include it together with all dependencies
                for i in (0..chain_deps.len()).rev() {
                    let mut cur_chain = &mut chain_deps[i];
                    cur_chain.merged = true;
                    result.append(&mut cur_chain.msgs.clone());
                    random_count += cur_chain.msgs.len();
                }

                chains.chain[i].merged = true;
                result.append(&mut chains.chain[i].msgs);
                random_count += chains.chain[i].msgs.len();
                gas_limit -= chain_gas_limit;

                if random_count > 0 {
                    warn!("optimal selection failed to pack a block; picked {} messages with random selection",
                    random_count);
                }
            }
        }

        Ok(result)
    }

    async fn get_pending_messages(
        &self,
        cur_ts: &Tipset,
        target_tipset: &Tipset,
    ) -> Result<Pending, Error> {
        let mut result: Pending = HashMap::new();
        let mut in_sync = false;
        // are we in sync?
        if cur_ts.epoch() == target_tipset.epoch() && cur_ts == target_tipset {
            in_sync = true;
        }

        // first add our current pending messages
        for (a, mset) in self.pending.read().await.iter() {
            if in_sync {
                result.insert(*a, mset.msgs.clone());
            } else {
                // we need to copy the map to avoid clobbering it as we load more messages
                let mut mset_copy = HashMap::new();
                for (nonce, m) in mset.msgs.iter() {
                    mset_copy.insert(*nonce, m.clone());
                }
                result.insert(*a, mset_copy);
            }
        }

        // we are in sync, that's the happy path
        if in_sync {
            return Ok(result);
        }

        // Run head change to do reorg detection
        run_head_change(
            &self.api,
            &self.pending,
            cur_ts.clone(),
            target_tipset.clone(),
            &mut result,
        )
        .await?;

        Ok(result)
    }

    async fn select_priority_messages(
        &self,
        pending: &mut Pending,
        base_fee: &BigInt,
        ts: &Tipset,
    ) -> Result<(Vec<SignedMessage>, i64), Error> {
        let result = Vec::with_capacity(self.config.size_limit_low() as usize);
        let gas_limit = types::BLOCK_GAS_LIMIT;
        let min_gas = 1298450;

        // 1. Get priority actor chains
        let mut chains = Vec::new();
        let priority = self.config.priority_addrs();
        for actor in priority.iter() {
            // remove actor from pending set as we are already processed these messages
            if let Some(mset) = pending.remove(actor) {
                let next = create_message_chains(&self.api, actor, &mset, base_fee, ts).await?;
                chains.extend(next);
            }
        }
        if chains.is_empty() {
            return Ok((Vec::new(), gas_limit));
        }

        Ok(merge_and_trim(chains, result, base_fee, gas_limit, min_gas))
    }
}

/// Returns merged and trimmed messages with the gas limit
fn merge_and_trim(
    mut chains: Vec<MsgChain>,
    mut result: Vec<SignedMessage>,
    base_fee: &BigInt,
    mut gas_limit: i64,
    min_gas: i64,
) -> (Vec<SignedMessage>, i64) {
    // 2. Sort the chains
    chains.sort_by(|a, b| b.compare(&a));

    if !chains.is_empty() && chains[0].curr().gas_perf < 0.0 {
        return (Vec::new(), gas_limit);
    }

    // 3. Merge chains until the block limit, as long as they have non-negative gas performance
    let mut last = chains.len();
    for (i, chain) in chains.iter().enumerate() {
        if chain.curr().gas_perf < 0.0 {
            break;
        }
        if chain.curr().gas_limit <= gas_limit {
            gas_limit -= chains[i].curr().gas_limit;
            result.extend(chain.curr().msgs.clone());
            continue;
        }
        last = i;
        break;
    }
    'tail_loop: while gas_limit >= min_gas && last < chains.len() {
        // trim, discard negative performing messages
        chains[last].trim(gas_limit, base_fee);

        // push down if it hasn't been invalidated
        if chains[last].curr().valid {
            for i in last..chains.len() - 1 {
                if chains[i].compare(&chains[i + 1]) == Ordering::Greater {
                    break;
                }
                chains.swap(i, i + 1);
            }
        }

        // select the next (valid and fitting) chain for inclusion
        for (i, chain) in chains.iter_mut().skip(last).enumerate() {
            if !chain.curr().valid {
                continue;
            }

            // if gas_perf < 0 then we have no more profitable chains
            if chain.curr().gas_perf < 0.0 {
                break 'tail_loop;
            }

            // does it fit in the block?
            if chain.curr().gas_limit <= gas_limit {
                gas_limit -= chain.curr().gas_limit;
                result.append(&mut chain.curr_mut().msgs);
                continue;
            }
            last += i;
            continue 'tail_loop;
        }
        break;
    }
    (result, gas_limit)
}

/// Like head_change, except it doesnt change the state of the MessagePool.
/// It simulates a head change call.
pub(crate) async fn run_head_change<T>(
    api: &RwLock<T>,
    pending: &RwLock<HashMap<Address, MsgSet>>,
    from: Tipset,
    to: Tipset,
    rmsgs: &mut HashMap<Address, HashMap<u64, SignedMessage>>,
) -> Result<(), Error>
where
    T: Provider + 'static,
{
    // TODO: This logic should probably be implemented in the ChainStore. It handles reorgs.
    let mut left = Arc::new(from);
    let mut right = Arc::new(to);
    let mut left_chain = Vec::new();
    let mut right_chain = Vec::new();
    while left != right {
        if left.epoch() > right.epoch() {
            left_chain.push(left.as_ref().clone());
            let par = api.read().await.load_tipset(left.parents()).await?;
            left = par;
        } else {
            right_chain.push(right.as_ref().clone());
            let par = api.read().await.load_tipset(right.parents()).await?;
            right = par;
        }
    }
    for ts in left_chain {
        let mut msgs: Vec<SignedMessage> = Vec::new();
        for block in ts.blocks() {
            let (_, smsgs) = api.read().await.messages_for_block(&block)?;
            msgs.extend(smsgs);
        }
        for msg in msgs {
            add_to_selected_msgs(msg, rmsgs);
        }
    }

    for ts in right_chain {
        for b in ts.blocks() {
            let (msgs, smsgs) = api.read().await.messages_for_block(b)?;

            for msg in smsgs {
                remove_from_selected_msgs(msg.from(), pending, msg.sequence(), rmsgs.borrow_mut())
                    .await?;
            }
            for msg in msgs {
                remove_from_selected_msgs(msg.from(), pending, msg.sequence(), rmsgs.borrow_mut())
                    .await?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod test_selection {
    use super::*;

    use crate::head_change;
    use crate::msgpool::test_provider::{mock_block, TestApi};
    use crate::msgpool::tests::create_smsg;
    use async_std::channel::bounded;
    use async_std::task;
    use crypto::SignatureType;
    use db::MemoryDB;
    use key_management::{MemKeyStore, Wallet};
    use message::Message;
    use std::sync::Arc;
    use types::NetworkParams;

    const TEST_GAS_LIMIT: i64 = 6955002;

    fn make_test_mpool() -> MessagePool<TestApi> {
        let tma = TestApi::default();
        task::block_on(async move {
            let (tx, _rx) = bounded(50);
            MessagePool::new(tma, "mptest".to_string(), tx, Default::default()).await
        })
        .unwrap()
    }

    #[async_std::test]
    async fn test_optimal_msg_selection1() {
        // this test uses just a single actor sending messages with a low tq
	    // the chain depenent merging algorithm should pick messages from the actor
	    // from the start
        let mpool = make_test_mpool();

        // create two actors
        let mut w1 = Wallet::new(MemKeyStore::new());
        let a1 = w1.generate_addr(SignatureType::Secp256k1).unwrap();
        let mut w2 = Wallet::new(MemKeyStore::new());
        let a2 = w2.generate_addr(SignatureType::Secp256k1).unwrap();

        // create a block
        let b1 = mock_block(1, 1);
        // add block to tipset
        let ts = Tipset::new(vec![b1.clone()]).unwrap();

        let api = mpool.api.clone();
        let bls_sig_cache = mpool.bls_sig_cache.clone();
        let pending = mpool.pending.clone();
        let cur_tipset = mpool.cur_tipset.clone();
        let repub_trigger = Arc::new(mpool.repub_trigger.clone());
        let republished = mpool.republished.clone();

        head_change(
            api.as_ref(),
            bls_sig_cache.as_ref(),
            repub_trigger.clone(),
            republished.as_ref(),
            pending.as_ref(),
            cur_tipset.as_ref(),
            Vec::new(),
            vec![Tipset::new(vec![b1]).unwrap()],
        )
        .await
        .unwrap();

        api.write()
        .await
        .set_state_balance_raw(&a1, types::DefaultNetworkParams::from_fil(1));

        api.write()
        .await
        .set_state_balance_raw(&a2, types::DefaultNetworkParams::from_fil(1));
        
        let n_msgs = 10 * types::BLOCK_GAS_LIMIT / TEST_GAS_LIMIT;

        // we create 10 messages from each actor to another, with the first actor paying higher
        // gas prices than the second; we expect message selection to order his messages first
        for i in 0..(n_msgs as usize) {
            let bias = (n_msgs as usize - i) / 3;
            let m = create_smsg(&a2, &a1, &mut w1, i as u64, TEST_GAS_LIMIT, (1 + i % 3 + bias) as u64);
            mpool.add(m).await.unwrap();
        }

        let msgs = mpool.select_messages(&ts, 0.25).await.unwrap();

        let expected_msgs = types::BLOCK_GAS_LIMIT / TEST_GAS_LIMIT;

        assert_eq!(msgs.len(), expected_msgs as usize);

        let mut next_nonce = 0u64;
        for m in msgs {
            assert_eq!(m.message().from(), &a1, "Expected message from a1");
            assert_eq!(m.message().sequence, next_nonce, "expected nonce {} but got {}", next_nonce, m.message().sequence);
            next_nonce += 1;
        }
    }

    #[async_std::test]
    async fn test_optimal_msg_selection2() {
        // this test uses two actors sending messages to each other, with the first
        // actor paying (much) higher gas premium than the second.
        // We select with a low ticket quality; the chain depenent merging algorithm should pick
        // messages from the second actor from the start
        let mpool = make_test_mpool();

        // create two actors
        let mut w1 = Wallet::new(MemKeyStore::new());
        let a1 = w1.generate_addr(SignatureType::Secp256k1).unwrap();
        let mut w2 = Wallet::new(MemKeyStore::new());
        let a2 = w2.generate_addr(SignatureType::Secp256k1).unwrap();

        // create a block
        let b1 = mock_block(1, 1);
        // add block to tipset
        let ts = Tipset::new(vec![b1.clone()]).unwrap();

        let api = mpool.api.clone();
        let bls_sig_cache = mpool.bls_sig_cache.clone();
        let pending = mpool.pending.clone();
        let cur_tipset = mpool.cur_tipset.clone();
        let repub_trigger = Arc::new(mpool.repub_trigger.clone());
        let republished = mpool.republished.clone();

        head_change(
            api.as_ref(),
            bls_sig_cache.as_ref(),
            repub_trigger.clone(),
            republished.as_ref(),
            pending.as_ref(),
            cur_tipset.as_ref(),
            Vec::new(),
            vec![Tipset::new(vec![b1]).unwrap()],
        )
        .await
        .unwrap();

        api.write()
        .await
        .set_state_balance_raw(&a1, types::DefaultNetworkParams::from_fil(1));

        api.write()
        .await
        .set_state_balance_raw(&a2, types::DefaultNetworkParams::from_fil(1));

        // TODO: change 1 back to 5
        let n_msgs = 1 * types::BLOCK_GAS_LIMIT / TEST_GAS_LIMIT;
        for i in 0..n_msgs as usize {
            let bias = (n_msgs as usize - i) / 3;
            let m = create_smsg(&a2, &a1, &mut w1, i as u64, TEST_GAS_LIMIT, (200000 + i % 3 + bias) as u64);
            mpool.add(m).await.unwrap();
            let m = create_smsg(&a1, &a2, &mut w2, i as u64, TEST_GAS_LIMIT, (190000 + i % 3 + bias) as u64);
            mpool.add(m).await.unwrap();
        }

        let msgs = mpool.select_messages(&ts, 0.1).await.unwrap();

        let expected_msgs = types::BLOCK_GAS_LIMIT / TEST_GAS_LIMIT;
        assert_eq!(msgs.len(), expected_msgs as usize, "Expected {} messages, but got {}", expected_msgs, msgs.len());

        let mut n_from1 = 0;
        let mut n_from2 = 0;
        let mut next_nonce1 = 0;
        let mut next_nonce2 = 0;

        for m in msgs {
            if m.message.from() == &a1 {
                // if m.message.sequence != next_nonce1 {
                //     panic!("oops");
                // }
                next_nonce1 += 1;       
                n_from1 += 1;
            } else {
                // if m.message.sequence != next_nonce2 {
                //     panic!("oops");
                // }
                next_nonce2 += 1;
                n_from2 += 1;
            }
        }

        dbg!(n_from1, n_from2);

        if n_from1 > n_from2 {
            panic!("Expected more msgs from a2 than a1");
        }
    }

    // #[async_std::test]
    // async fn test_optimal_msg_selection3() {
    //     // this test uses 10 actors sending a block of messages to each other, with the the first
    //     // actors paying higher gas premium than the subsequent actors.
    //     // We select with a low ticket quality; the chain depenent merging algorithm should pick
    // 	// messages from the median actor from the start
    //     let mpool = make_test_mpool();

    //     let n_actors: usize = 10;

    //     let mut actors = vec![];
    //     let mut wallets = vec![];

    //     for i in 0..n_actors {
    //         let mut w = Wallet::new(MemKeyStore::new());
    //         let a = w.generate_addr(SignatureType::Secp256k1).unwrap();
    //         actors.push(a);
    //         wallets.push(w);
    //     }

    //     // create a block
    //     let b1 = mock_block(1, 1);
    //     // add block to tipset
    //     let ts = Tipset::new(vec![b1.clone()]).unwrap();

        
    //     let api = mpool.api.clone();
    //     let bls_sig_cache = mpool.bls_sig_cache.clone();
    //     let pending = mpool.pending.clone();
    //     let cur_tipset = mpool.cur_tipset.clone();
    //     let repub_trigger = Arc::new(mpool.repub_trigger.clone());
    //     let republished = mpool.republished.clone();

    //     head_change(
    //         api.as_ref(),
    //         bls_sig_cache.as_ref(),
    //         repub_trigger.clone(),
    //         republished.as_ref(),
    //         pending.as_ref(),
    //         cur_tipset.as_ref(),
    //         Vec::new(),
    //         vec![Tipset::new(vec![b1]).unwrap()],
    //     )
    //     .await
    //     .unwrap();

    //     for a in &actors {
    //         api.write()
    //         .await
    //         .set_state_balance_raw(&a, types::DefaultNetworkParams::from_fil(1));
    //     }

    //     let n_msgs: usize = types::BLOCK_GAS_LIMIT as usize / TEST_GAS_LIMIT as usize + 1;
    //     for  i in 0..n_msgs {
    //         for j in 0..n_actors {
    //             let premium = 500000 + 10000*(n_actors -j) + (30*n_actors)/(n_msgs+2*i) + i%3;
    //             let m = create_smsg(&actors[j as usize], &actors[j%n_actors], &mut wallets[j], i as u64, TEST_GAS_LIMIT, premium as u64);
    //             mpool.add(m).await.unwrap();
    //         }
    //     }

    //     let msgs = mpool.select_messages(&ts, 0.1).await.unwrap();
    //     let expected_msgs = types::BLOCK_GAS_LIMIT / TEST_GAS_LIMIT;
        
    //     // TODO fix test case
    //     assert_eq!(expected_msgs as usize, msgs.len(), "expected {} messages, but got {}", expected_msgs, msgs.len());
    // }

    #[async_std::test]
    async fn basic_message_selection() {
        let mpool = make_test_mpool();

        let mut w1 = Wallet::new(MemKeyStore::new());
        let a1 = w1.generate_addr(SignatureType::Secp256k1).unwrap();

        let mut w2 = Wallet::new(MemKeyStore::new());
        let a2 = w2.generate_addr(SignatureType::Secp256k1).unwrap();

        let b1 = mock_block(1, 1);
        let ts = Tipset::new(vec![b1.clone()]).unwrap();
        let api = mpool.api.clone();
        let bls_sig_cache = mpool.bls_sig_cache.clone();
        let pending = mpool.pending.clone();
        let cur_tipset = mpool.cur_tipset.clone();
        let repub_trigger = Arc::new(mpool.repub_trigger.clone());
        let republished = mpool.republished.clone();
        head_change(
            api.as_ref(),
            bls_sig_cache.as_ref(),
            repub_trigger.clone(),
            republished.as_ref(),
            pending.as_ref(),
            cur_tipset.as_ref(),
            Vec::new(),
            vec![Tipset::new(vec![b1]).unwrap()],
        )
        .await
        .unwrap();

        let gas_limit = 6955002;
        api.write()
            .await
            .set_state_balance_raw(&a1, types::DefaultNetworkParams::from_fil(1));
        api.write()
            .await
            .set_state_balance_raw(&a2, types::DefaultNetworkParams::from_fil(1));

        // we create 10 messages from each actor to another, with the first actor paying higher
        // gas prices than the second; we expect message selection to order his messages first
        for i in 0..10 {
            let m = create_smsg(&a2, &a1, &mut w1, i, gas_limit, 2 * i + 1);
            mpool.add(m).await.unwrap();
        }
        for i in 0..10 {
            let m = create_smsg(&a1, &a2, &mut w2, i, gas_limit, i + 1);
            mpool.add(m).await.unwrap();
        }

        let msgs = mpool.select_messages(&ts, 1.0).await.unwrap();
        assert_eq!(msgs.len(), 20);
        let mut next_nonce = 0;
        for i in 0..10 {
            assert_eq!(
                *msgs[i].from(),
                a1,
                "first 10 returned messages should be from actor a1"
            );
            assert_eq!(msgs[i].sequence(), next_nonce, "nonce should be in order");
            next_nonce += 1;
        }
        next_nonce = 0;
        for i in 10..20 {
            assert_eq!(
                *msgs[i].from(),
                a2,
                "next 10 returned messages should be from actor a2"
            );
            assert_eq!(msgs[i].sequence(), next_nonce, "nonce should be in order");
            next_nonce += 1;
        }

        // now we make a block with all the messages and advance the chain
        let b2 = mpool.api.write().await.next_block();
        mpool.api.write().await.set_block_messages(&b2, msgs);
        head_change(
            api.as_ref(),
            bls_sig_cache.as_ref(),
            repub_trigger.clone(),
            republished.as_ref(),
            pending.as_ref(),
            cur_tipset.as_ref(),
            Vec::new(),
            vec![Tipset::new(vec![b2]).unwrap()],
        )
        .await
        .unwrap();

        // we should now have no pending messages in the MessagePool
        assert!(
            mpool.pending.read().await.is_empty(),
            "there should be no more pending messages"
        );

        // create a block and advance the chain without applying to the mpool
        let mut msgs = Vec::with_capacity(20);
        for i in 10..20 {
            msgs.push(create_smsg(&a2, &a1, &mut w1, i, gas_limit, 2 * i + 1));
            msgs.push(create_smsg(&a1, &a2, &mut w2, i, gas_limit, i + 1));
        }
        let b3 = mpool.api.write().await.next_block();
        let ts3 = Tipset::new(vec![b3.clone()]).unwrap();
        mpool.api.write().await.set_block_messages(&b3, msgs);

        // now create another set of messages and add them to the mpool
        for i in 20..30 {
            mpool
                .add(create_smsg(&a2, &a1, &mut w1, i, gas_limit, 2 * i + 200))
                .await
                .unwrap();
            mpool
                .add(create_smsg(&a1, &a2, &mut w2, i, gas_limit, i + 1))
                .await
                .unwrap();
        }
        // select messages in the last tipset; this should include the missed messages as well as
        // the last messages we added, with the first actor's messages first
        // first we need to update the nonce on the api
        mpool.api.write().await.set_state_sequence(&a1, 10);
        mpool.api.write().await.set_state_sequence(&a2, 10);
        let msgs = mpool.select_messages(&ts3, 1.0).await.unwrap();

        assert_eq!(msgs.len(), 20);

        let mut next_nonce = 20;
        for i in 0..10 {
            assert_eq!(
                *msgs[i].from(),
                a1,
                "first 10 returned messages should be from actor a1"
            );
            assert_eq!(msgs[i].sequence(), next_nonce, "nonce should be in order");
            next_nonce += 1;
        }
        next_nonce = 20;
        for i in 10..20 {
            assert_eq!(
                *msgs[i].from(),
                a2,
                "next 10 returned messages should be from actor a2"
            );
            assert_eq!(msgs[i].sequence(), next_nonce, "nonce should be in order");
            next_nonce += 1;
        }
    }

    #[async_std::test]
    // #[ignore = "test is incredibly slow"]
    // TODO optimize logic tested in this function
    async fn message_selection_trimming() {
        let mpool = make_test_mpool();

        let mut w1 = Wallet::new(MemKeyStore::new());
        let a1 = w1.generate_addr(SignatureType::Secp256k1).unwrap();

        let mut w2 = Wallet::new(MemKeyStore::new());
        let a2 = w2.generate_addr(SignatureType::Secp256k1).unwrap();

        let b1 = mock_block(1, 1);
        let ts = Tipset::new(vec![b1.clone()]).unwrap();
        let api = mpool.api.clone();
        let bls_sig_cache = mpool.bls_sig_cache.clone();
        let pending = mpool.pending.clone();
        let cur_tipset = mpool.cur_tipset.clone();
        let repub_trigger = Arc::new(mpool.repub_trigger.clone());
        let republished = mpool.republished.clone();
        head_change(
            api.as_ref(),
            bls_sig_cache.as_ref(),
            repub_trigger.clone(),
            republished.as_ref(),
            pending.as_ref(),
            cur_tipset.as_ref(),
            Vec::new(),
            vec![Tipset::new(vec![b1]).unwrap()],
        )
        .await
        .unwrap();

        let gas_limit = 6955002;
        api.write()
            .await
            .set_state_balance_raw(&a1, types::DefaultNetworkParams::from_fil(1));
        api.write()
            .await
            .set_state_balance_raw(&a2, types::DefaultNetworkParams::from_fil(1));

        let nmsgs = (types::BLOCK_GAS_LIMIT / gas_limit) + 1;

        // make many small chains for the two actors
        for i in 0..nmsgs {
            let bias = (nmsgs - i) / 3;
            let m = create_smsg(
                &a2,
                &a1,
                &mut w1,
                i as u64,
                gas_limit,
                (1 + i % 3 + bias) as u64,
            );
            mpool.add(m).await.unwrap();
            let m = create_smsg(
                &a1,
                &a2,
                &mut w2,
                i as u64,
                gas_limit,
                (1 + i % 3 + bias) as u64,
            );
            mpool.add(m).await.unwrap();
        }

        let msgs = mpool.select_messages(&ts, 1.0).await.unwrap();

        let expected = types::BLOCK_GAS_LIMIT / gas_limit;
        assert_eq!(msgs.len(), expected as usize);
        let mut m_gas_lim = 0;
        for m in msgs.iter() {
            m_gas_lim += m.gas_limit();
        }
        assert!(m_gas_lim <= types::BLOCK_GAS_LIMIT);
    }

    #[async_std::test]
    async fn message_selection_priority() {
        let db = MemoryDB::default();

        let mut mpool = make_test_mpool();

        let mut w1 = Wallet::new(MemKeyStore::new());
        let a1 = w1.generate_addr(SignatureType::Secp256k1).unwrap();

        let mut w2 = Wallet::new(MemKeyStore::new());
        let a2 = w2.generate_addr(SignatureType::Secp256k1).unwrap();

        // set priority addrs to a1
        let mut mpool_cfg = mpool.get_config().clone();
        mpool_cfg.priority_addrs.push(a1);
        mpool.set_config(&db, mpool_cfg).unwrap();

        let b1 = mock_block(1, 1);
        let ts = Tipset::new(vec![b1.clone()]).unwrap();
        let api = mpool.api.clone();
        let bls_sig_cache = mpool.bls_sig_cache.clone();
        let pending = mpool.pending.clone();
        let cur_tipset = mpool.cur_tipset.clone();
        let repub_trigger = Arc::new(mpool.repub_trigger.clone());
        let republished = mpool.republished.clone();
        head_change(
            api.as_ref(),
            bls_sig_cache.as_ref(),
            repub_trigger.clone(),
            republished.as_ref(),
            pending.as_ref(),
            cur_tipset.as_ref(),
            Vec::new(),
            vec![Tipset::new(vec![b1]).unwrap()],
        )
        .await
        .unwrap();

        let gas_limit = 6955002;
        api.write()
            .await
            .set_state_balance_raw(&a1, types::DefaultNetworkParams::from_fil(1));
        api.write()
            .await
            .set_state_balance_raw(&a2, types::DefaultNetworkParams::from_fil(1));

        let nmsgs = 10;

        // make many small chains for the two actors
        for i in 0..nmsgs {
            let bias = (nmsgs - i) / 3;
            let m = create_smsg(
                &a2,
                &a1,
                &mut w1,
                i as u64,
                gas_limit,
                (1 + i % 3 + bias) as u64,
            );
            mpool.add(m).await.unwrap();
            let m = create_smsg(
                &a1,
                &a2,
                &mut w2,
                i as u64,
                gas_limit,
                (1 + i % 3 + bias) as u64,
            );
            mpool.add(m).await.unwrap();
        }

        let msgs = mpool.select_messages(&ts, 1.0).await.unwrap();

        assert_eq!(msgs.len(), 20);

        let mut next_nonce = 0;
        for i in 0..10 {
            assert_eq!(
                *msgs[i].from(),
                a1,
                "first 10 returned messages should be from actor a1"
            );
            assert_eq!(msgs[i].sequence(), next_nonce, "nonce should be in order");
            next_nonce += 1;
        }
        next_nonce = 0;
        for i in 10..20 {
            assert_eq!(
                *msgs[i].from(),
                a2,
                "next 10 returned messages should be from actor a2"
            );
            assert_eq!(msgs[i].sequence(), next_nonce, "nonce should be in order");
            next_nonce += 1;
        }
    }
}
