// Copyright 2020 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT


trait PaychProvider {
    /// message_pool
    // StateAccountKey(context.Context, address.Address, types.TipSetKey) (address.Address, error)
	// StateWaitMsg(ctx context.Context, msg cid.Cid, confidence uint64) (*api.MsgLookup, error)
	// MpoolPushMessage(ctx context.Context, msg *types.Message, maxFee *api.MessageSendSpec) (*types.SignedMessage, error)
	// WalletHas(ctx context.Context, addr address.Address) (bool, error)
	// WalletSign(ctx context.Context, k address.Address, msg []byte) (*crypto.Signature, error)
	// StateNetworkVersion(context.Context, types.TipSetKey) (network.Version, error)

    // API
    fn state_account_key (&self, addr: Address, ts_key: TipsetKey) -> Result<Address, Box<dyn Error>>;
    fn state_wait_msg (&self, msg: Cid, confidence: u64) -> Result<MessageLookup, Box<dyn Error>>;
    fn mpool_push_message(&self, msg: UnsignedMessage, max_fee: Option<MessageSendSpec>) -> Result<SignedMessage, Box<dyn Error>>;
    fn wallet_has(&self, addr: Address) -> Result<bool, Box<dyn Error>>;
    fn wallet_sign(&self, k: Address, msg: &[u8]) -> Result<Signature, Box<dyn Error>>;
    fn state_network_version(&self, ts_key: TipsetKey) -> Result<NetworkVersion, Box<dyn Error>>;

    // ResolveToKeyAddress(ctx context.Context, addr address.Address, ts *types.TipSet) (address.Address, error)
	// GetPaychState(ctx context.Context, addr address.Address, ts *types.TipSet) (*types.Actor, paych.State, error)
	// Call(ctx context.Context, msg *types.Message, ts *types.TipSet) (*api.InvocResult, error)
    fn resolve_to_key_address(&self, addr: Address, ts: Tipset) -> Result<Address, Box<dyn Error>>;
    fn get_paych_state (&self, addr: Address, ts: Tipset) -> Result<(ActorState, actor::paych::State), Box<dyn Error>>;
    fn call<V: ProofVerifier>(
        self: &self,
        message: &mut UnsignedMessage,
        tipset: Option<Arc<Tipset>>,
    ) -> StateCallResult;
}