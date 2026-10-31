use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    token::{StellarAssetClient, TokenClient},
    Address, Env,
};

use crate::{BountyEscrow, BountyEscrowClient};

pub const DAY: u64 = 86_400;
pub const FEE_BPS: u32 = 300;
pub const GRACE_SECS: u64 = 72 * 3_600;
pub const MIN_DURATION: u64 = DAY;
pub const MAX_DURATION: u64 = 90 * DAY;
/// Stellar assets have 7 decimals: 1 unit = 10_000_000 base units.
pub const UNIT: i128 = 10_000_000;
pub const MIN_AMOUNT: i128 = UNIT;
pub const MAX_AMOUNT: i128 = 500 * UNIT;
pub const START_TIME: u64 = 1_700_000_000;

pub struct Setup<'a> {
    pub env: &'a Env,
    pub client: BountyEscrowClient<'a>,
    pub contract_id: Address,
    pub admin: Address,
    pub relayer: Address,
    pub arbiter: Address,
    pub treasury: Address,
    pub maintainer: Address,
    /// An allowlisted token; the maintainer holds 1_000 units of it.
    pub token: Address,
}

impl<'a> Setup<'a> {
    /// Registers the contract, leaving it uninitialized. All auths are mocked.
    pub fn uninitialized(env: &'a Env) -> Self {
        env.mock_all_auths();
        env.ledger().set_timestamp(START_TIME);

        let contract_id = env.register(BountyEscrow, ());
        let client = BountyEscrowClient::new(env, &contract_id);
        let token = Self::new_token(env);

        Setup {
            env,
            client,
            contract_id,
            admin: Address::generate(env),
            relayer: Address::generate(env),
            arbiter: Address::generate(env),
            treasury: Address::generate(env),
            maintainer: Address::generate(env),
            token,
        }
    }

    /// Initialized contract with `token` allowlisted at [MIN_AMOUNT, MAX_AMOUNT].
    pub fn new(env: &'a Env) -> Self {
        let s = Self::uninitialized(env);
        s.client.init(
            &s.admin,
            &s.relayer,
            &s.arbiter,
            &s.treasury,
            &FEE_BPS,
            &GRACE_SECS,
            &MIN_DURATION,
            &MAX_DURATION,
        );
        s.client
            .set_token(&s.admin, &s.token, &MIN_AMOUNT, &MAX_AMOUNT);
        s.mint(&s.token, &s.maintainer, 1_000 * UNIT);
        s
    }

    /// A new Stellar asset contract that is not allowlisted.
    pub fn new_token(env: &Env) -> Address {
        env.register_stellar_asset_contract_v2(Address::generate(env))
            .address()
    }

    pub fn mint(&self, token: &Address, to: &Address, amount: i128) {
        StellarAssetClient::new(self.env, token).mint(to, &amount);
    }

    pub fn balance(&self, token: &Address, who: &Address) -> i128 {
        TokenClient::new(self.env, token).balance(who)
    }

    pub fn now(&self) -> u64 {
        self.env.ledger().timestamp()
    }

    pub fn set_time(&self, timestamp: u64) {
        self.env.ledger().set_timestamp(timestamp);
    }

    /// A valid deadline: 14 days from now.
    pub fn deadline(&self) -> u64 {
        self.now() + 14 * DAY
    }

    /// Funds a bounty from the default maintainer with the default token, relayer allowed.
    pub fn fund(&self, repo_id: u64, issue: u64, amount: i128) -> u64 {
        self.client.fund_bounty(
            &self.maintainer,
            &repo_id,
            &issue,
            &self.token,
            &amount,
            &self.deadline(),
            &true,
        )
    }

    /// Checks the 6.7 invariant for `token` and returns its totals.
    pub fn assert_invariant(&self, token: &Address) -> crate::types::Totals {
        let totals = self.env.as_contract(&self.contract_id, || {
            crate::storage::get_totals(self.env, token)
        });
        assert_eq!(
            totals.funded,
            totals.released_net + totals.fees + totals.refunded + totals.held,
            "funded == released_net + fees + refunded + held"
        );
        assert_eq!(
            totals.held,
            self.balance(token, &self.contract_id),
            "held == token.balance(contract)"
        );
        totals
    }
}
