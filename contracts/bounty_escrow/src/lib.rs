#![no_std]

pub mod errors;
pub mod events;
pub mod fees;
pub mod storage;
pub mod types;

use soroban_sdk::{contract, contractimpl, token::TokenClient, Address, Env};

use errors::Error;
use types::{Bounty, Config, Status, TokenLimits};

/// Hard cap on the protocol fee: 10%.
pub const MAX_FEE_BPS: u32 = 1_000;

#[contract]
pub struct BountyEscrow;

/// Authenticates `admin` and checks it is the configured admin.
fn require_admin(env: &Env, admin: &Address) -> Result<Config, Error> {
    admin.require_auth();
    let config = storage::get_config(env)?;
    if *admin != config.admin {
        return Err(Error::Unauthorized);
    }
    Ok(config)
}

#[contractimpl]
impl BountyEscrow {
    /// One-time setup. `admin` must authorize.
    #[allow(clippy::too_many_arguments)]
    pub fn init(
        env: Env,
        admin: Address,
        relayer: Address,
        arbiter: Address,
        treasury: Address,
        fee_bps: u32,
        grace_secs: u64,
        min_duration_secs: u64,
        max_duration_secs: u64,
    ) -> Result<(), Error> {
        admin.require_auth();
        if storage::has_config(&env) {
            return Err(Error::AlreadyInitialized);
        }
        if fee_bps > MAX_FEE_BPS {
            return Err(Error::InvalidFee);
        }
        if min_duration_secs > max_duration_secs {
            return Err(Error::InvalidDeadline);
        }
        storage::set_config(
            &env,
            &Config {
                admin,
                relayer,
                arbiter,
                treasury,
                fee_bps,
                grace_secs,
                min_duration_secs,
                max_duration_secs,
                paused: false,
            },
        );
        storage::bump_instance(&env);
        Ok(())
    }

    /// Allowlist `token` (or update its limits). Amounts are in the token's smallest unit.
    pub fn set_token(
        env: Env,
        admin: Address,
        token: Address,
        min_amount: i128,
        max_amount: i128,
    ) -> Result<(), Error> {
        require_admin(&env, &admin)?;
        if min_amount <= 0 {
            return Err(Error::AmountTooSmall);
        }
        if max_amount < min_amount {
            return Err(Error::AmountTooLarge);
        }
        storage::set_token_limits(
            &env,
            &token,
            &TokenLimits {
                min_amount,
                max_amount,
            },
        );
        storage::bump_instance(&env);
        events::token_set(&env, &token, min_amount, max_amount);
        Ok(())
    }

    /// Blocks `fund_bounty` only. Release, refund and dispute resolution keep working.
    pub fn pause(env: Env, admin: Address) -> Result<(), Error> {
        let mut config = require_admin(&env, &admin)?;
        config.paused = true;
        storage::set_config(&env, &config);
        storage::bump_instance(&env);
        events::paused(&env, &admin);
        Ok(())
    }

    pub fn unpause(env: Env, admin: Address) -> Result<(), Error> {
        let mut config = require_admin(&env, &admin)?;
        config.paused = false;
        storage::set_config(&env, &config);
        storage::bump_instance(&env);
        events::unpaused(&env, &admin);
        Ok(())
    }

    /// Locks `amount` of `token` from `maintainer` on a GitHub issue and returns the bounty id.
    ///
    /// Order: auth, validate, write state, transfer tokens, emit event.
    #[allow(clippy::too_many_arguments)]
    pub fn fund_bounty(
        env: Env,
        maintainer: Address,
        repo_id: u64,
        issue: u64,
        token: Address,
        amount: i128,
        deadline: u64,
        relayer_allowed: bool,
    ) -> Result<u64, Error> {
        maintainer.require_auth();
        storage::bump_instance(&env);

        let config = storage::get_config(&env)?;
        if config.paused {
            return Err(Error::Paused);
        }
        let limits = storage::get_token_limits(&env, &token).ok_or(Error::TokenNotAllowed)?;
        if amount < limits.min_amount {
            return Err(Error::AmountTooSmall);
        }
        if amount > limits.max_amount {
            return Err(Error::AmountTooLarge);
        }
        let now = env.ledger().timestamp();
        let earliest = now
            .checked_add(config.min_duration_secs)
            .ok_or(Error::Overflow)?;
        let latest = now
            .checked_add(config.max_duration_secs)
            .ok_or(Error::Overflow)?;
        if deadline < earliest || deadline > latest {
            return Err(Error::InvalidDeadline);
        }
        if storage::get_active(&env, repo_id, issue).is_some() {
            return Err(Error::IssueAlreadyFunded);
        }

        // Effects.
        let id = storage::take_next_id(&env)?;
        let bounty = Bounty {
            id,
            maintainer: maintainer.clone(),
            repo_id,
            issue,
            token: token.clone(),
            amount,
            fee_bps: config.fee_bps,
            deadline,
            relayer_allowed,
            status: Status::Open,
            winner: None,
            created_at: now,
        };
        storage::set_bounty(&env, &bounty);
        storage::set_active(&env, repo_id, issue, id);
        let mut totals = storage::get_totals(&env, &token);
        totals.funded = totals.funded.checked_add(amount).ok_or(Error::Overflow)?;
        totals.held = totals.held.checked_add(amount).ok_or(Error::Overflow)?;
        storage::set_totals(&env, &token, &totals);

        // Interactions.
        TokenClient::new(&env, &token).transfer(
            &maintainer,
            env.current_contract_address(),
            &amount,
        );
        events::funded(&env, &bounty);
        Ok(id)
    }
    pub fn get_config(env: Env) -> Result<Config, Error> {
        storage::bump_instance(&env);
        storage::get_config(&env)
    }

    pub fn get_bounty(env: Env, id: u64) -> Result<Bounty, Error> {
        storage::bump_instance(&env);
        storage::get_bounty(&env, id)
    }

    pub fn active_bounty(env: Env, repo_id: u64, issue: u64) -> Option<u64> {
        storage::bump_instance(&env);
        storage::get_active(&env, repo_id, issue)
    }
}

#[cfg(test)]
mod test;
