use soroban_sdk::{contractevent, symbol_short, Address, BytesN, Env, Symbol};

use crate::types::Bounty;

// Topics are ("bounty", <name>) plus the bounty id where applicable. Data carries every
// relevant field so the indexer never needs a follow-up read.

#[contractevent(topics = ["bounty", "funded"], data_format = "map")]
pub struct Funded {
    #[topic]
    pub id: u64,
    pub maintainer: Address,
    pub repo_id: u64,
    pub issue: u64,
    pub token: Address,
    pub amount: i128,
    pub fee_bps: u32,
    pub deadline: u64,
    pub relayer_allowed: bool,
}

/// An admin changed the token allowlist entry for `token`.
#[contractevent(topics = ["bounty", "config"], data_format = "map")]
pub struct TokenSet {
    #[topic]
    pub field: Symbol,
    pub token: Address,
    pub min_amount: i128,
    pub max_amount: i128,
}

#[contractevent(topics = ["bounty", "paused"], data_format = "map")]
pub struct Paused {
    pub admin: Address,
}

#[contractevent(topics = ["bounty", "unpaused"], data_format = "map")]
pub struct Unpaused {
    pub admin: Address,
}

pub fn funded(env: &Env, bounty: &Bounty) {
    Funded {
        id: bounty.id,
        maintainer: bounty.maintainer.clone(),
        repo_id: bounty.repo_id,
        issue: bounty.issue,
        token: bounty.token.clone(),
        amount: bounty.amount,
        fee_bps: bounty.fee_bps,
        deadline: bounty.deadline,
        relayer_allowed: bounty.relayer_allowed,
    }
    .publish(env);
}

pub fn token_set(env: &Env, token: &Address, min_amount: i128, max_amount: i128) {
    TokenSet {
        field: symbol_short!("token"),
        token: token.clone(),
        min_amount,
        max_amount,
    }
    .publish(env);
}

pub fn paused(env: &Env, admin: &Address) {
    Paused {
        admin: admin.clone(),
    }
    .publish(env);
}

pub fn unpaused(env: &Env, admin: &Address) {
    Unpaused {
        admin: admin.clone(),
    }
    .publish(env);
}

#[contractevent(topics = ["bounty", "released"], data_format = "map")]
pub struct Released {
    #[topic]
    pub id: u64,
    pub winner: Address,
    pub net: i128,
    pub fee: i128,
    pub caller: Address,
}

#[contractevent(topics = ["bounty", "refunded"], data_format = "map")]
pub struct Refunded {
    #[topic]
    pub id: u64,
    pub amount: i128,
}

#[contractevent(topics = ["bounty", "disputed"], data_format = "map")]
pub struct Disputed {
    #[topic]
    pub id: u64,
    pub caller: Address,
}

#[contractevent(topics = ["bounty", "resolved"], data_format = "map")]
pub struct Resolved {
    #[topic]
    pub id: u64,
    pub winner: Option<Address>,
    pub winner_net: i128,
    pub fee: i128,
    pub maintainer_amount: i128,
}

/// An address-valued config field changed: `admin`, `relayer`, `arbiter` or `treasury`.
#[contractevent(topics = ["bounty", "config"], data_format = "map")]
pub struct AddressSet {
    #[topic]
    pub field: Symbol,
    pub old: Address,
    pub new: Address,
}

#[contractevent(topics = ["bounty", "config"], data_format = "map")]
pub struct FeeSet {
    #[topic]
    pub field: Symbol,
    pub old_bps: u32,
    pub new_bps: u32,
}

#[contractevent(topics = ["bounty", "config"], data_format = "map")]
pub struct GraceSet {
    #[topic]
    pub field: Symbol,
    pub old_secs: u64,
    pub new_secs: u64,
}

#[contractevent(topics = ["bounty", "config"], data_format = "map")]
pub struct DurationsSet {
    #[topic]
    pub field: Symbol,
    pub min_secs: u64,
    pub max_secs: u64,
}

#[contractevent(topics = ["bounty", "config"], data_format = "map")]
pub struct TokenRemoved {
    #[topic]
    pub field: Symbol,
    pub token: Address,
}

#[contractevent(topics = ["bounty", "upgraded"], data_format = "map")]
pub struct Upgraded {
    pub wasm_hash: BytesN<32>,
}

pub fn released(env: &Env, id: u64, winner: &Address, net: i128, fee: i128, caller: &Address) {
    Released {
        id,
        winner: winner.clone(),
        net,
        fee,
        caller: caller.clone(),
    }
    .publish(env);
}

pub fn refunded(env: &Env, id: u64, amount: i128) {
    Refunded { id, amount }.publish(env);
}

pub fn disputed(env: &Env, id: u64, caller: &Address) {
    Disputed {
        id,
        caller: caller.clone(),
    }
    .publish(env);
}

pub fn resolved(
    env: &Env,
    id: u64,
    winner: Option<Address>,
    winner_net: i128,
    fee: i128,
    maintainer_amount: i128,
) {
    Resolved {
        id,
        winner,
        winner_net,
        fee,
        maintainer_amount,
    }
    .publish(env);
}

pub fn address_set(env: &Env, field: Symbol, old: &Address, new: &Address) {
    AddressSet {
        field,
        old: old.clone(),
        new: new.clone(),
    }
    .publish(env);
}

pub fn fee_set(env: &Env, old_bps: u32, new_bps: u32) {
    FeeSet {
        field: symbol_short!("fee_bps"),
        old_bps,
        new_bps,
    }
    .publish(env);
}

pub fn grace_set(env: &Env, old_secs: u64, new_secs: u64) {
    GraceSet {
        field: symbol_short!("grace"),
        old_secs,
        new_secs,
    }
    .publish(env);
}

pub fn durations_set(env: &Env, min_secs: u64, max_secs: u64) {
    DurationsSet {
        field: symbol_short!("durations"),
        min_secs,
        max_secs,
    }
    .publish(env);
}

pub fn token_removed(env: &Env, token: &Address) {
    TokenRemoved {
        field: symbol_short!("token_rm"),
        token: token.clone(),
    }
    .publish(env);
}

pub fn upgraded(env: &Env, wasm_hash: &BytesN<32>) {
    Upgraded {
        wasm_hash: wasm_hash.clone(),
    }
    .publish(env);
}
