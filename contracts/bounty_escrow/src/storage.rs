use soroban_sdk::{Address, Env};

use crate::errors::Error;
use crate::types::{Bounty, Config, DataKey, TokenLimits, Totals};

// TTL numbers. Ledgers close every 5 s, so a day is 17_280 ledgers.
//
// Network limits (testnet, read with `stellar network settings` on 2026-10-31):
//   max_entry_ttl     = 3_110_400 ledgers (180 days)
//   min_persistent_ttl =   120_960 ledgers (7 days)
// Re-check these on mainnet before launch; see docs/CONTRACT.md.
//
// Worst case a bounty must stay readable: the longest duration (90 days by default)
// plus the grace period (72 hours by default) = 93 days. Every read and write
// refreshes an entry to TTL_EXTEND_TO (120 days) once it falls below TTL_THRESHOLD
// (30 days), leaving at least 27 days of margin over the worst case, and staying
// well under the network maximum.
pub const DAY_IN_LEDGERS: u32 = 17_280;
pub const TTL_THRESHOLD: u32 = 30 * DAY_IN_LEDGERS;
pub const TTL_EXTEND_TO: u32 = 120 * DAY_IN_LEDGERS;

// Compile-time checks on the TTL numbers above.
const LEDGER_SECS: u64 = 5;
const TESTNET_MAX_ENTRY_TTL: u32 = 3_110_400;
// Spec defaults: 90 day max duration plus 72 hour grace.
const WORST_CASE_LEDGERS: u64 = (90 * 86_400 + 72 * 3_600) / LEDGER_SECS;
const _: () = {
    assert!(DAY_IN_LEDGERS as u64 * LEDGER_SECS == 86_400);
    assert!(TTL_THRESHOLD < TTL_EXTEND_TO);
    assert!(TTL_EXTEND_TO < TESTNET_MAX_ENTRY_TTL);
    // At least 25 days of margin beyond the worst-case bounty lifetime.
    assert!(TTL_EXTEND_TO as u64 >= WORST_CASE_LEDGERS + 25 * DAY_IN_LEDGERS as u64);
};
/// Extend the instance TTL (Config, NextId, Token(*)). Call on every entry point.
pub fn bump_instance(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
}

fn bump_persistent(env: &Env, key: &DataKey) {
    env.storage()
        .persistent()
        .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
}

// Config (instance)

pub fn has_config(env: &Env) -> bool {
    env.storage().instance().has(&DataKey::Config)
}

pub fn get_config(env: &Env) -> Result<Config, Error> {
    env.storage()
        .instance()
        .get(&DataKey::Config)
        .ok_or(Error::NotInitialized)
}

pub fn set_config(env: &Env, config: &Config) {
    env.storage().instance().set(&DataKey::Config, config);
}

// Bounty ids (instance). Ids start at 1.

pub fn take_next_id(env: &Env) -> Result<u64, Error> {
    let id: u64 = env.storage().instance().get(&DataKey::NextId).unwrap_or(1);
    let next = id.checked_add(1).ok_or(Error::Overflow)?;
    env.storage().instance().set(&DataKey::NextId, &next);
    Ok(id)
}

// Token allowlist (instance)

pub fn get_token_limits(env: &Env, token: &Address) -> Option<TokenLimits> {
    env.storage().instance().get(&DataKey::Token(token.clone()))
}

pub fn set_token_limits(env: &Env, token: &Address, limits: &TokenLimits) {
    env.storage()
        .instance()
        .set(&DataKey::Token(token.clone()), limits);
}

// Bounties (persistent)

pub fn get_bounty(env: &Env, id: u64) -> Result<Bounty, Error> {
    let key = DataKey::Bounty(id);
    let bounty = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(Error::BountyNotFound)?;
    bump_persistent(env, &key);
    Ok(bounty)
}

pub fn set_bounty(env: &Env, bounty: &Bounty) {
    let key = DataKey::Bounty(bounty.id);
    env.storage().persistent().set(&key, bounty);
    bump_persistent(env, &key);
}

// Active bounty per issue (persistent)

pub fn get_active(env: &Env, repo_id: u64, issue: u64) -> Option<u64> {
    let key = DataKey::ActiveByIssue(repo_id, issue);
    let id = env.storage().persistent().get(&key);
    if id.is_some() {
        bump_persistent(env, &key);
    }
    id
}

pub fn set_active(env: &Env, repo_id: u64, issue: u64, id: u64) {
    let key = DataKey::ActiveByIssue(repo_id, issue);
    env.storage().persistent().set(&key, &id);
    bump_persistent(env, &key);
}

// Per-token accounting (persistent)

pub fn get_totals(env: &Env, token: &Address) -> Totals {
    let key = DataKey::Totals(token.clone());
    let totals = env.storage().persistent().get(&key);
    if totals.is_some() {
        bump_persistent(env, &key);
    }
    totals.unwrap_or_default()
}

pub fn set_totals(env: &Env, token: &Address, totals: &Totals) {
    let key = DataKey::Totals(token.clone());
    env.storage().persistent().set(&key, totals);
    bump_persistent(env, &key);
}
