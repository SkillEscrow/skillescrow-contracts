use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    token::StellarAssetClient,
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
}
