// Copyright 2020 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use crate::utils::{get_gas_perf, get_gas_reward};
use message::{Message, SignedMessage};
use num_bigint::BigInt;
use std::cmp::Ordering;
use std::f64::EPSILON;
use rand::seq::SliceRandom;
use rand::thread_rng;

/// Represents a node in the MsgChain.
/// The next and previous pointers are indexes on the `chain` vector in MsgChain implementation.
#[derive(Clone, Debug)]
pub(crate) struct MsgChainNodeFixed {
    // A list of messages to be included in a block
    pub msgs: Vec<SignedMessage>,
    // The cumulative gas reward of the first message
    pub gas_reward: BigInt,
    pub gas_limit: i64,
    pub gas_perf: f64,
    pub eff_perf: f64,
    pub bp: f64,
    pub parent_offset: f64,
    pub valid: bool,
    pub merged: bool,
    pub next: Option<usize>,
    pub prev: Option<usize>
}

impl MsgChainNodeFixed {
    pub(crate) fn new() -> Self {
        Self {
            msgs: vec![],
            gas_reward: Default::default(),
            gas_limit: 0,
            gas_perf: 0.0,
            eff_perf: 0.0,
            bp: 0.0,
            parent_offset: 0.0,
            valid: false,
            merged: false,
            next: None,
            prev: None
        }
    }

    #[allow(dead_code)]
    pub fn set_null_effective_perf(&mut self) {
        if self.gas_perf < 0.0 {
            self.eff_perf = self.gas_perf;
        } else {
            self.eff_perf = 0.0;
        }
    }

    pub(crate) fn cmp_effective(&self, other: &Self) -> Ordering {
        self.merged.cmp(&other.merged)
        .then_with(|| approx_cmp(self.gas_perf, 0.0).cmp(&approx_cmp(other.gas_perf, 0.0)))
        .then_with(|| approx_cmp(self.eff_perf, other.eff_perf))
        .then_with(|| approx_cmp(self.eff_perf, other.eff_perf).cmp( &approx_cmp(self.gas_perf, other.gas_perf)))
        .then_with(|| approx_cmp(self.eff_perf, other.eff_perf).cmp(&approx_cmp(self.gas_perf, other.gas_perf).cmp(&self.gas_reward.cmp(&other.gas_reward))))
    }

    // helper for the main set_eff_perf method on MsgChain
    // this accepts the eff_perf and gas_limit of the previous node.
    pub(crate) fn set_eff_perf(&mut self, prev: Option<(f64, i64)>) {
        let mut eff_perf = self.gas_perf * self.bp;
        if let Some(prev) = prev {
            if eff_perf > 0.0 {
                let prev_eff_perf = prev.0;
                let prev_gas_limit = prev.1;
                let eff_perf_with_parent = (eff_perf * self.gas_limit as f64
                    + prev_eff_perf * prev_gas_limit as f64)
                    / (self.gas_limit + prev_gas_limit) as f64;
                self.parent_offset = eff_perf - eff_perf_with_parent;
                eff_perf = eff_perf_with_parent;
            }
        }
        self.eff_perf = eff_perf;
    }

    // pub(crate) fn test_set_eff_perf(&mut self, prev: Option<&MsgChainNodeFixed>) {
    //     let mut eff_perf = self.gas_perf * self.bp;
    //     if let Some(prev) = prev {
    //         if eff_perf > 0.0 {
    //             let prev_eff_perf = prev.eff_perf;
    //             let prev_gas_limit = prev.gas_limit;
    //             let eff_perf_with_parent = (eff_perf * self.gas_limit as f64
    //                 + prev_eff_perf * prev_gas_limit as f64)
    //                 / (self.gas_limit + prev_gas_limit) as f64;
    //             self.parent_offset = eff_perf - eff_perf_with_parent;
    //             eff_perf = eff_perf_with_parent;
    //         }
    //     }
    //     self.eff_perf = eff_perf;
    // }

    // prev: (gas_limit, eff_perf)
    pub(crate) fn set_effperf_with_block_prob(&mut self, block_prob: f64, prev: Option<(f64, i64)>) {
        self.bp = block_prob;
        let mut eff_perf = self.gas_perf * self.bp;
        if eff_perf > 0.0 && prev.is_some() {
            let prev = prev.unwrap();
            let prev_gas_limit = prev.0;
            let prev_eff_perf = prev.1 as f64;
            let eff_perf_with_parent = (eff_perf * self.gas_limit as f64 + prev_eff_perf * prev_gas_limit) / self.gas_limit as f64 + prev_gas_limit as f64;
            self.parent_offset = eff_perf - eff_perf_with_parent;
            eff_perf = eff_perf_with_parent;
        }
        self.eff_perf = eff_perf;
    }

    pub(crate) fn compare(&self, other: &Self) -> Ordering {
        approx_cmp(self.gas_perf, other.gas_perf)
            .then_with(|| self.gas_reward.cmp(&other.gas_reward))
    }

    // pub fn set_null_effective_perf(&mut self) {
    //     if self.gas_perf < 0.0 {
    //         self.eff_perf = self.gas_perf
    //     } else {
    //         self.eff_perf = 0.0
    //     }
    // }

    // // TODO this should be on MsgChainFixed as we want to access next and prev pointers.
    // pub(crate) fn trim(&mut self, gas_limit: i64, base_fee: &BigInt, chain_ref: &MsgChainFixed) {
    //     let mut i = self.msgs.len() - 1;
    //     while i >= 0 && (self.gas_limit > gas_limit || self.gas_perf < 0.0) {
    //         let gas_reward = get_gas_reward(&self.msgs[i as usize], base_fee);
    //         self.gas_reward -= gas_reward;
    //         self.gas_limit -= self.msgs[i].message.gas_limit;
    //         if self.gas_limit > 0 {
    //             self.gas_perf = get_gas_perf(&self.gas_reward, self.gas_limit);
    //             if self.bp != 0.0 {
    //                 if let Some(prev_idx) = self.prev {
    //                     let prev = chain_ref.chain.get(prev_idx);
    //                     self.test_set_eff_perf(prev);
    //                 }
    //             }
    //         } else {
    //             self.gas_perf = 0.0;
    //             self.eff_perf = 0.0;
    //         }
    //         i -= 1;
    //     }

    //     if i < 0 {
    //         self.msgs = vec![];
    //         self.valid = false;
    //     } else {
    //         self.msgs = self.msgs.drain(0..i+1).collect();
    //     }
    // }
}

/// Mimics the doubly linked circular-referenced message chain from Lotus by keeping a current index
/// The MsgChain is an abstraction of a list of MsgChainNode where each one has a next and previous pointer.
/// Each msg chain node is segmented according to the gas perf calculated during the create_message_chain call
#[derive(Clone, Debug)]
pub(crate) struct MsgChainFixed {
    pub(crate) chain: Vec<MsgChainNodeFixed>,
}

impl Default for MsgChainFixed {
    fn default() -> Self {
        Self {
            chain: Vec::new(),
        }
    }
}

impl MsgChainFixed {
    /// Creates a new message chain
    pub(crate) fn new(nodes: Vec<MsgChainNodeFixed>) -> Self {
        Self {
            chain: nodes,
        }
    }

    // link each chain node prev and next pointers
    // this is required for optimal msg selection algorithm
    pub(crate) fn link(&mut self) {
        // link next pointers
        for i in 0..self.chain.len()-1 {
            self.chain[i].next = Some(i + 1);    
        }
        // link prev pointers
        for i in (1..self.chain.len()).rev() {
            self.chain[i].prev = Some(i - 1);
        }
    }

    // This method assumes that node already exists in its chain
    // This panics if `node` doesn't exist in chain
    // gets the next node with the given next index
    pub(crate) fn next_mut_idx(&mut self, next_idx: usize) -> Option<&mut MsgChainNodeFixed> {
        self.chain.get_mut(next_idx)
    }

    // This method assumes that node already exists in its chain
    // This panics if `node` doesn't exist in chain
    pub(crate) fn next_mut(&mut self, node: &MsgChainNodeFixed) -> Option<&mut MsgChainNodeFixed> {
        node.next.map(move |n| self.chain.get_mut(n).expect("Expected MsgChainNode to be in chain"))
    }

    // This method assumes that node already exists in its chain
    // This panics if `node` doesn't exist in chain
    pub(crate) fn next(&mut self, node: &MsgChainNodeFixed) -> Option<&MsgChainNodeFixed> {
        node.next.map(move |n| self.chain.get(n).expect("Expected MsgChainNode to be in chain"))
    }

    // This panics if `node` doesn't exist in chain
    pub(crate) fn prev_mut(&mut self, node: &MsgChainNodeFixed) -> Option<&mut MsgChainNodeFixed> {
        node.prev.map(move |n| self.chain.get_mut(n).expect("Expected MsgChainNode to be in chain"))
    }

    // This panics if `node` doesn't exist in chain
    pub(crate) fn prev(&mut self, node: &MsgChainNodeFixed) -> Option<&MsgChainNodeFixed> {
        node.prev.map(move |n| self.chain.get(n).expect("Expected MsgChainNode to be in chain"))
    }
}

impl MsgChainFixed {
    // pub(crate) fn compare(&self, other: &Self) -> Ordering {
    //     let self_curr = self.curr();
    //     let other_curr = other.curr();
    //     approx_cmp(self_curr.gas_perf, other_curr.gas_perf)
    //         .then_with(|| self_curr.gas_reward.cmp(&other_curr.gas_reward))
    // }

    pub(crate) fn trim(&mut self, gas_limit: i64, base_fee: &BigInt, node_idx: usize) {
        let mut i = self.chain[node_idx].msgs.len() as i64 - 1;
        let prev = self.chain[node_idx].prev.map(|n| (self.chain[n].eff_perf, self.chain[n].gas_limit));
        {
            // unwrap is fine as the caller ensures that node_idx is within bounds
            let mc = self.chain.get_mut(node_idx).unwrap();
            while i >= 0 && (mc.gas_limit > gas_limit || (mc.gas_perf < 0.0)) {
                let gas_reward = get_gas_reward(&mc.msgs[i as usize], base_fee);
                mc.gas_reward -= gas_reward;
                mc.gas_limit -= mc.msgs[i as usize].gas_limit();
                if mc.gas_limit > 0 {
                    mc.gas_perf = get_gas_perf(&mc.gas_reward, mc.gas_limit);
                    if mc.bp != 0.0 {
                        // set eff perf
                        mc.set_eff_perf(prev);
                    }
                } else {
                    mc.gas_perf = 0.0;
                    mc.eff_perf = 0.0;
                }
                i -= 1;
            }

            if i < 0 {
                mc.msgs.clear();
                mc.valid = false;
            } else {
                mc.msgs.drain(0..i as usize);
            }
        }

        self.invalidate_next_nodes(node_idx + 1);
    }

    pub(crate) fn invalidate_next_nodes(&mut self, node_idx: usize) {
        let mut node_idx = node_idx;
        while let Some(n) = self.chain.get_mut(node_idx) {
            n.valid = false;
            n.msgs.clear();
            n.next = None;
            node_idx = if n.next.is_some() {n.next.unwrap()} else {break};
        }
    }

    #[allow(dead_code)]
    pub(crate) fn set_effective_perf(&mut self, node: &mut MsgChainNodeFixed, bp: f64) {
        node.bp = bp;
        self.set_eff_perf(node);
    }

    #[allow(dead_code)]
    pub(crate) fn set_eff_perf(&mut self, node: &mut MsgChainNodeFixed) {
        let prev = self.prev(node).map(|n| (n.eff_perf, n.gas_limit));

        // let mc = self.curr_mut();
        let mut eff_perf = node.gas_perf * node.bp;
        if let Some(prev) = prev {
            if eff_perf > 0.0 {
                let prev_eff_perf = prev.0;
                let prev_gas_limit = prev.1;
                let eff_perf_with_parent = (eff_perf * node.gas_limit as f64
                    + prev_eff_perf * prev_gas_limit as f64)
                    / (node.gas_limit + prev_gas_limit) as f64;
                node.parent_offset = eff_perf - eff_perf_with_parent;
                eff_perf = eff_perf_with_parent;
            }
        }
        node.eff_perf = eff_perf;
    }

    // #[allow(dead_code)]
    // pub(crate) fn set_eff_perf_at(&mut self, idx: usize) {
    //     let prev = match self.prev() {
    //         Some(prev) => Some((prev.eff_perf, prev.gas_limit)),
    //         None => None,
    //     };

    //     let mc = &mut self.chain[idx];
    //     let mut eff_perf = mc.gas_perf * mc.bp;
    //     if let Some(prev) = prev {
    //         if eff_perf > 0.0 {
    //             let prev_eff_perf = prev.0;
    //             let prev_gas_limit = prev.1;
    //             let eff_perf_with_parent = (eff_perf * mc.gas_limit as f64
    //                 + prev_eff_perf * prev_gas_limit as f64)
    //                 / (mc.gas_limit + prev_gas_limit) as f64;
    //             mc.parent_offset = eff_perf - eff_perf_with_parent;
    //             eff_perf = eff_perf_with_parent;
    //         }
    //     }
    //     mc.eff_perf = eff_perf;
    // }

    #[allow(dead_code)]
    pub fn set_null_effective_perf(&self, node: &mut MsgChainNodeFixed) {
        if node.gas_perf < 0.0 {
            node.eff_perf = node.gas_perf;
        } else {
            node.eff_perf = 0.0;
        }
    }

    // pub(crate) fn cmp_effective(&self, other: &Self) -> Ordering {
    //     let mc = self.curr();
    //     let other = other.curr();

    //     mc.merged.cmp(&other.merged)
    //     .then_with(|| approx_cmp(mc.gas_perf, 0.0).cmp(&approx_cmp(other.gas_perf, 0.0)))
    //     .then_with(|| approx_cmp(mc.eff_perf, other.eff_perf))
    //     .then_with(|| approx_cmp(mc.eff_perf, other.eff_perf).cmp( &approx_cmp(mc.gas_perf, other.gas_perf)))
    //     .then_with(|| approx_cmp(mc.eff_perf, other.eff_perf).cmp(&approx_cmp(mc.gas_perf, other.gas_perf).cmp(&mc.gas_reward.cmp(&other.gas_reward))))
    // }

    // pub(crate) fn cmp_effective(&self, other: &Self) -> Ordering {
    //     let mc = self.curr();
    //     let other = other.curr();

    //     // let merged = mc.merged && !other.merged;

    //     let last = ((mc.eff_perf == other.eff_perf) && (mc.gas_perf == other.gas_perf)).cmp(&true);
    //     // let last = mc.gas_reward.cmp(other.gas_reward);


    //     mc.merged.cmp(&other.merged)
    //     .then_with(|| mc.gas_perf.partial_cmp(&0.0).unwrap()).cmp(&other.gas_perf.partial_cmp(&0.0).unwrap())
    //     .then_with(|| mc.eff_perf.partial_cmp(&other.eff_perf).unwrap())
    //     .then_with(|| mc.eff_perf.partial_cmp(&other.eff_perf).unwrap().cmp(&mc.gas_perf.partial_cmp(&other.gas_perf).unwrap()))
    //     .then_with(|| mc.eff_perf.partial_cmp(&other.eff_perf.unwrap().cmp(&approx_cmp(mc.gas_perf, other.gas_perf).cmp(&mc.gas_reward.cmp(&other.gas_reward))))
    // }

    // #[allow(dead_code)]
    // pub(crate) fn cmp_effective(&self, other: &Self) -> Ordering {
    //     let mc = self.curr();
    //     let other = other.curr();
    //     mc.merged
    //         .cmp(&other.merged)
    //         .then_with(|| (mc.gas_perf >= 0.0).cmp(&(other.gas_perf >= 0.0)))
    //         .then_with(|| approx_cmp(mc.eff_perf, other.eff_perf))
    //         .then_with(|| approx_cmp(mc.gas_perf, other.gas_perf))
    //         .then_with(|| mc.gas_reward.cmp(&other.gas_reward))
    // }
}

fn approx_cmp(a: f64, b: f64) -> Ordering {
    if (a - b).abs() < EPSILON {
        Ordering::Equal
    } else {
        a.partial_cmp(&b).unwrap()
    }
}

pub(crate) fn shuffle_chains(chains: &mut Vec<MsgChainNodeFixed>) {
    chains.shuffle(&mut thread_rng());
}
