use soroban_sdk::{contractevent, symbol_short, Address, Env, Symbol};

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
