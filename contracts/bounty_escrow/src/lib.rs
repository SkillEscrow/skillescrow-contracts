#![no_std]

pub mod errors;
pub mod events;
pub mod fees;
pub mod storage;
pub mod types;

use soroban_sdk::{
    contract, contractimpl, symbol_short, token::TokenClient, Address, BytesN, ContractExecutable,
    Env,
};

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

/// Sends `amount` from the contract to `to`. Skips zero amounts.
fn pay(env: &Env, token: &Address, to: &Address, amount: i128) {
    if amount > 0 {
        TokenClient::new(env, token).transfer(&env.current_contract_address(), to, &amount);
    }
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

    /// Stops `token` from being used for new bounties. Existing bounties in that token can
    /// still be released, refunded and resolved.
    pub fn remove_token(env: Env, admin: Address, token: Address) -> Result<(), Error> {
        require_admin(&env, &admin)?;
        if storage::get_token_limits(&env, &token).is_none() {
            return Err(Error::TokenNotAllowed);
        }
        storage::remove_token_limits(&env, &token);
        storage::bump_instance(&env);
        events::token_removed(&env, &token);
        Ok(())
    }

    /// Affects bounties funded afterwards only: each bounty snapshots the fee at funding.
    pub fn set_fee(env: Env, admin: Address, fee_bps: u32) -> Result<(), Error> {
        let mut config = require_admin(&env, &admin)?;
        if fee_bps > MAX_FEE_BPS {
            return Err(Error::InvalidFee);
        }
        let old = config.fee_bps;
        config.fee_bps = fee_bps;
        storage::set_config(&env, &config);
        storage::bump_instance(&env);
        events::fee_set(&env, old, fee_bps);
        Ok(())
    }

    pub fn set_relayer(env: Env, admin: Address, relayer: Address) -> Result<(), Error> {
        let mut config = require_admin(&env, &admin)?;
        let old = core::mem::replace(&mut config.relayer, relayer.clone());
        storage::set_config(&env, &config);
        storage::bump_instance(&env);
        events::address_set(&env, symbol_short!("relayer"), &old, &relayer);
        Ok(())
    }

    pub fn set_arbiter(env: Env, admin: Address, arbiter: Address) -> Result<(), Error> {
        let mut config = require_admin(&env, &admin)?;
        let old = core::mem::replace(&mut config.arbiter, arbiter.clone());
        storage::set_config(&env, &config);
        storage::bump_instance(&env);
        events::address_set(&env, symbol_short!("arbiter"), &old, &arbiter);
        Ok(())
    }

    pub fn set_treasury(env: Env, admin: Address, treasury: Address) -> Result<(), Error> {
        let mut config = require_admin(&env, &admin)?;
        let old = core::mem::replace(&mut config.treasury, treasury.clone());
        storage::set_config(&env, &config);
        storage::bump_instance(&env);
        events::address_set(&env, symbol_short!("treasury"), &old, &treasury);
        Ok(())
    }

    /// Changes how long after the deadline a refund must wait. Applies to existing bounties
    /// too, because it is read from config at refund time.
    pub fn set_grace(env: Env, admin: Address, grace_secs: u64) -> Result<(), Error> {
        let mut config = require_admin(&env, &admin)?;
        let old = config.grace_secs;
        config.grace_secs = grace_secs;
        storage::set_config(&env, &config);
        storage::bump_instance(&env);
        events::grace_set(&env, old, grace_secs);
        Ok(())
    }

    pub fn set_durations(
        env: Env,
        admin: Address,
        min_duration_secs: u64,
        max_duration_secs: u64,
    ) -> Result<(), Error> {
        let mut config = require_admin(&env, &admin)?;
        if min_duration_secs > max_duration_secs {
            return Err(Error::InvalidDeadline);
        }
        config.min_duration_secs = min_duration_secs;
        config.max_duration_secs = max_duration_secs;
        storage::set_config(&env, &config);
        storage::bump_instance(&env);
        events::durations_set(&env, min_duration_secs, max_duration_secs);
        Ok(())
    }

    /// Hands the admin role to `new_admin`. Both must authorize, so a typo in the new
    /// address cannot lock the contract.
    pub fn set_admin(env: Env, admin: Address, new_admin: Address) -> Result<(), Error> {
        let mut config = require_admin(&env, &admin)?;
        new_admin.require_auth();
        config.admin = new_admin.clone();
        storage::set_config(&env, &config);
        storage::bump_instance(&env);
        events::address_set(&env, symbol_short!("admin"), &admin, &new_admin);
        Ok(())
    }

    /// Replaces the contract code. The admin can change what this contract does, including
    /// for funds already held, so the admin key must be a multisig before mainnet.
    /// See the trust model in docs/CONTRACT.md.
    pub fn upgrade(env: Env, admin: Address, wasm_hash: BytesN<32>) -> Result<(), Error> {
        require_admin(&env, &admin)?;
        storage::bump_instance(&env);
        events::upgraded(&env, &wasm_hash);
        env.deployer()
            .update_current_contract(ContractExecutable::Wasm(wasm_hash));
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
    /// Pays `winner` the bounty minus the fee, and the fee to the treasury.
    ///
    /// Callable by the maintainer, or by the relayer if the maintainer opted in at funding
    /// time. Still allowed after the deadline, as long as no refund has happened.
    pub fn release(
        env: Env,
        caller: Address,
        bounty_id: u64,
        winner: Address,
    ) -> Result<(), Error> {
        caller.require_auth();
        storage::bump_instance(&env);

        let config = storage::get_config(&env)?;
        let mut bounty = storage::get_bounty(&env, bounty_id)?;
        if bounty.status != Status::Open {
            return Err(Error::InvalidStatus);
        }
        if caller != bounty.maintainer {
            if caller != config.relayer {
                return Err(Error::Unauthorized);
            }
            if !bounty.relayer_allowed {
                return Err(Error::RelayerNotAllowed);
            }
        }
        if winner == bounty.maintainer {
            return Err(Error::SelfPayout);
        }
        let fee = fees::fee(bounty.amount, bounty.fee_bps)?;
        let net = fees::net(bounty.amount, bounty.fee_bps)?;

        // Effects.
        bounty.status = Status::Released;
        bounty.winner = Some(winner.clone());
        storage::set_bounty(&env, &bounty);
        storage::clear_active(&env, bounty.repo_id, bounty.issue);
        let mut totals = storage::get_totals(&env, &bounty.token);
        totals.held = totals
            .held
            .checked_sub(bounty.amount)
            .ok_or(Error::Overflow)?;
        totals.released_net = totals
            .released_net
            .checked_add(net)
            .ok_or(Error::Overflow)?;
        totals.fees = totals.fees.checked_add(fee).ok_or(Error::Overflow)?;
        storage::set_totals(&env, &bounty.token, &totals);

        // Interactions.
        pay(&env, &bounty.token, &winner, net);
        pay(&env, &bounty.token, &config.treasury, fee);
        events::released(&env, bounty.id, &winner, net, fee, &caller);
        Ok(())
    }
    /// Returns the full amount to the maintainer once `deadline + grace_secs` has passed.
    /// No fee is taken. Works while paused.
    pub fn refund(env: Env, maintainer: Address, bounty_id: u64) -> Result<(), Error> {
        maintainer.require_auth();
        storage::bump_instance(&env);

        let config = storage::get_config(&env)?;
        let mut bounty = storage::get_bounty(&env, bounty_id)?;
        if bounty.status != Status::Open {
            return Err(Error::InvalidStatus);
        }
        if maintainer != bounty.maintainer {
            return Err(Error::Unauthorized);
        }
        let unlocks_at = bounty
            .deadline
            .checked_add(config.grace_secs)
            .ok_or(Error::Overflow)?;
        if env.ledger().timestamp() < unlocks_at {
            return Err(Error::DeadlineNotReached);
        }

        // Effects.
        bounty.status = Status::Refunded;
        storage::set_bounty(&env, &bounty);
        storage::clear_active(&env, bounty.repo_id, bounty.issue);
        let mut totals = storage::get_totals(&env, &bounty.token);
        totals.held = totals
            .held
            .checked_sub(bounty.amount)
            .ok_or(Error::Overflow)?;
        totals.refunded = totals
            .refunded
            .checked_add(bounty.amount)
            .ok_or(Error::Overflow)?;
        storage::set_totals(&env, &bounty.token, &totals);

        // Interactions.
        pay(&env, &bounty.token, &bounty.maintainer, bounty.amount);
        events::refunded(&env, bounty.id, bounty.amount);
        Ok(())
    }
    /// Freezes an open bounty: release and refund are blocked until the arbiter resolves it.
    /// Callable by the maintainer or the arbiter.
    pub fn open_dispute(env: Env, caller: Address, bounty_id: u64) -> Result<(), Error> {
        caller.require_auth();
        storage::bump_instance(&env);

        let config = storage::get_config(&env)?;
        let mut bounty = storage::get_bounty(&env, bounty_id)?;
        if bounty.status != Status::Open {
            return Err(Error::InvalidStatus);
        }
        if caller != bounty.maintainer && caller != config.arbiter {
            return Err(Error::Unauthorized);
        }

        bounty.status = Status::Disputed;
        storage::set_bounty(&env, &bounty);
        events::disputed(&env, bounty.id, &caller);
        Ok(())
    }

    /// Splits a disputed bounty. The winner gets `winner_bps` of the amount minus the fee on
    /// that portion, the treasury gets that fee, and the maintainer gets the rest.
    /// Works while paused.
    pub fn resolve(
        env: Env,
        arbiter: Address,
        bounty_id: u64,
        winner: Option<Address>,
        winner_bps: u32,
    ) -> Result<(), Error> {
        arbiter.require_auth();
        storage::bump_instance(&env);

        let config = storage::get_config(&env)?;
        if arbiter != config.arbiter {
            return Err(Error::Unauthorized);
        }
        let mut bounty = storage::get_bounty(&env, bounty_id)?;
        if bounty.status != Status::Disputed {
            return Err(Error::InvalidStatus);
        }
        if winner_bps > fees::BPS_DENOMINATOR as u32 {
            return Err(Error::InvalidSplit);
        }
        // A winner is only meaningful, and only required, when they are owed something.
        let winner = if winner_bps > 0 {
            let w = winner.ok_or(Error::InvalidSplit)?;
            if w == bounty.maintainer {
                return Err(Error::SelfPayout);
            }
            Some(w)
        } else {
            None
        };

        let portion = fees::portion(bounty.amount, winner_bps)?;
        let fee = fees::fee(portion, bounty.fee_bps)?;
        let winner_net = portion - fee;
        let maintainer_amount = bounty.amount - portion;

        // Effects.
        bounty.status = Status::Resolved;
        bounty.winner = winner.clone();
        storage::set_bounty(&env, &bounty);
        storage::clear_active(&env, bounty.repo_id, bounty.issue);
        let mut totals = storage::get_totals(&env, &bounty.token);
        totals.held = totals
            .held
            .checked_sub(bounty.amount)
            .ok_or(Error::Overflow)?;
        totals.released_net = totals
            .released_net
            .checked_add(winner_net)
            .ok_or(Error::Overflow)?;
        totals.fees = totals.fees.checked_add(fee).ok_or(Error::Overflow)?;
        totals.refunded = totals
            .refunded
            .checked_add(maintainer_amount)
            .ok_or(Error::Overflow)?;
        storage::set_totals(&env, &bounty.token, &totals);

        // Interactions.
        if let Some(w) = &winner {
            pay(&env, &bounty.token, w, winner_net);
        }
        pay(&env, &bounty.token, &config.treasury, fee);
        pay(&env, &bounty.token, &bounty.maintainer, maintainer_amount);
        events::resolved(&env, bounty.id, winner, winner_net, fee, maintainer_amount);
        Ok(())
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
