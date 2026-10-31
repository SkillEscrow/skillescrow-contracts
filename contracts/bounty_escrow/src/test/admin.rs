use soroban_sdk::{
    symbol_short,
    testutils::{
        Address as _, AuthorizedFunction, AuthorizedInvocation, Events as _, MockAuth,
        MockAuthInvoke,
    },
    Address, BytesN, Env, Event as _, IntoVal,
};

use super::setup::*;
use crate::errors::Error;
use crate::events::{
    AddressSet, DurationsSet, FeeSet, GraceSet, Paused, TokenRemoved, TokenSet, Unpaused,
};
use crate::types::{Config, Status};

#[test]
fn init_stores_config() {
    let env = Env::default();
    let s = Setup::new(&env);
    assert_eq!(
        s.client.get_config(),
        Config {
            admin: s.admin.clone(),
            relayer: s.relayer.clone(),
            arbiter: s.arbiter.clone(),
            treasury: s.treasury.clone(),
            fee_bps: FEE_BPS,
            grace_secs: GRACE_SECS,
            min_duration_secs: MIN_DURATION,
            max_duration_secs: MAX_DURATION,
            paused: false,
        }
    );
}

#[test]
fn init_once_only() {
    let env = Env::default();
    let s = Setup::new(&env);
    let again = s.client.try_init(
        &s.admin,
        &s.relayer,
        &s.arbiter,
        &s.treasury,
        &FEE_BPS,
        &GRACE_SECS,
        &MIN_DURATION,
        &MAX_DURATION,
    );
    assert_eq!(again, Err(Ok(Error::AlreadyInitialized)));
}

#[test]
fn init_fee_cap() {
    let env = Env::default();
    let s = Setup::uninitialized(&env);
    let init = |fee_bps: u32| {
        s.client.try_init(
            &s.admin,
            &s.relayer,
            &s.arbiter,
            &s.treasury,
            &fee_bps,
            &GRACE_SECS,
            &MIN_DURATION,
            &MAX_DURATION,
        )
    };
    assert_eq!(init(1_001), Err(Ok(Error::InvalidFee)));
    assert_eq!(init(u32::MAX), Err(Ok(Error::InvalidFee)));
    // A failed init leaves the contract uninitialized, so exactly 10% still works.
    assert_eq!(init(1_000), Ok(Ok(())));
    assert_eq!(s.client.get_config().fee_bps, 1_000);
}

#[test]
fn init_rejects_min_duration_above_max() {
    let env = Env::default();
    let s = Setup::uninitialized(&env);
    let r = s.client.try_init(
        &s.admin,
        &s.relayer,
        &s.arbiter,
        &s.treasury,
        &FEE_BPS,
        &GRACE_SECS,
        &(MAX_DURATION + 1),
        &MAX_DURATION,
    );
    assert_eq!(r, Err(Ok(Error::InvalidDeadline)));
}

#[test]
fn init_requires_admin_auth() {
    let env = Env::default();
    let s = Setup::uninitialized(&env);
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
    let auths = env.auths();
    assert_eq!(auths.len(), 1);
    assert_eq!(auths[0].0, s.admin);
    assert!(matches!(
        &auths[0].1.function,
        AuthorizedFunction::Contract((id, name, _)) if *id == s.contract_id && *name == symbol_short!("init")
    ));
}

#[test]
fn uninitialized_contract_reports_not_initialized() {
    let env = Env::default();
    let s = Setup::uninitialized(&env);
    assert_eq!(s.client.try_get_config(), Err(Ok(Error::NotInitialized)));
    assert_eq!(
        s.client
            .try_set_token(&s.admin, &s.token, &MIN_AMOUNT, &MAX_AMOUNT),
        Err(Ok(Error::NotInitialized))
    );
}

#[test]
fn set_token_stores_limits_and_emits_event() {
    let env = Env::default();
    let s = Setup::new(&env);
    let other = Setup::new_token(&env);
    s.client.set_token(&s.admin, &other, &5, &50);

    assert_eq!(
        env.events().all().filter_by_contract(&s.contract_id),
        [TokenSet {
            field: symbol_short!("token"),
            token: other.clone(),
            min_amount: 5,
            max_amount: 50,
        }
        .to_xdr(&env, &s.contract_id)]
    );
}

#[test]
fn set_token_validates_limits() {
    let env = Env::default();
    let s = Setup::new(&env);
    let t = Setup::new_token(&env);
    assert_eq!(
        s.client.try_set_token(&s.admin, &t, &0, &10),
        Err(Ok(Error::AmountTooSmall))
    );
    assert_eq!(
        s.client.try_set_token(&s.admin, &t, &-1, &10),
        Err(Ok(Error::AmountTooSmall))
    );
    assert_eq!(
        s.client.try_set_token(&s.admin, &t, &10, &9),
        Err(Ok(Error::AmountTooLarge))
    );
    // min == max is allowed.
    s.client.set_token(&s.admin, &t, &10, &10);
}

#[test]
fn admin_functions_reject_non_admin() {
    let env = Env::default();
    let s = Setup::new(&env);
    let rando = Address::generate(&env);
    assert_eq!(
        s.client
            .try_set_token(&rando, &s.token, &MIN_AMOUNT, &MAX_AMOUNT),
        Err(Ok(Error::Unauthorized))
    );
    assert_eq!(s.client.try_pause(&rando), Err(Ok(Error::Unauthorized)));
    assert_eq!(s.client.try_unpause(&rando), Err(Ok(Error::Unauthorized)));
}

#[test]
fn admin_functions_require_admin_signature() {
    let env = Env::default();
    let s = Setup::new(&env);
    // Fixtures used mock_all_auths; turn mocking off so real auth is enforced.
    env.set_auths(&[]);
    assert!(s.client.try_pause(&s.admin).is_err());
    assert!(!s.client.get_config().paused);
}

#[test]
fn pause_toggles_and_emits() {
    let env = Env::default();
    let s = Setup::new(&env);

    s.client.pause(&s.admin);
    // Events are those of the last invocation, so check before the next call.
    assert_eq!(
        env.events().all().filter_by_contract(&s.contract_id),
        [Paused {
            admin: s.admin.clone()
        }
        .to_xdr(&env, &s.contract_id)]
    );
    assert!(s.client.get_config().paused);

    s.client.unpause(&s.admin);
    assert_eq!(
        env.events().all().filter_by_contract(&s.contract_id),
        [Unpaused {
            admin: s.admin.clone()
        }
        .to_xdr(&env, &s.contract_id)]
    );
    assert!(!s.client.get_config().paused);
}
#[test]
fn pause_requires_admin_auth_assertion() {
    let env = Env::default();
    let s = Setup::new(&env);
    s.client.pause(&s.admin);
    assert_eq!(
        env.auths(),
        [(
            s.admin.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.contract_id.clone(),
                    symbol_short!("pause"),
                    (&s.admin,).into_val(&env),
                )),
                sub_invocations: [].into(),
            }
        )]
    );
}

// ---- remaining admin entry points ----

#[test]
fn set_fee_enforces_cap_and_emits() {
    let env = Env::default();
    let s = Setup::new(&env);
    assert_eq!(
        s.client.try_set_fee(&s.admin, &1_001),
        Err(Ok(Error::InvalidFee))
    );
    assert_eq!(
        s.client.try_set_fee(&s.admin, &u32::MAX),
        Err(Ok(Error::InvalidFee))
    );
    assert_eq!(s.client.get_config().fee_bps, FEE_BPS);

    s.client.set_fee(&s.admin, &1_000);
    assert_eq!(
        env.events().all().filter_by_contract(&s.contract_id),
        [FeeSet {
            field: symbol_short!("fee_bps"),
            old_bps: FEE_BPS,
            new_bps: 1_000,
        }
        .to_xdr(&env, &s.contract_id)]
    );
    assert_eq!(s.client.get_config().fee_bps, 1_000);
    s.client.set_fee(&s.admin, &0);
    assert_eq!(s.client.get_config().fee_bps, 0);
}

#[test]
fn fee_change_does_not_affect_existing_bounties() {
    let env = Env::default();
    let s = Setup::new(&env);
    let winner = Address::generate(&env);
    let old = s.fund(1, 1, 100 * UNIT);

    s.client.set_fee(&s.admin, &1_000);
    let new = s.fund(1, 2, 100 * UNIT);

    assert_eq!(s.client.get_bounty(&old).fee_bps, FEE_BPS);
    assert_eq!(s.client.get_bounty(&new).fee_bps, 1_000);

    // The old bounty still pays the 3% it was funded with; the new one pays 10%.
    s.client.release(&s.maintainer, &old, &winner);
    assert_eq!(s.balance(&s.token, &s.treasury), 3 * UNIT);
    s.client.release(&s.maintainer, &new, &winner);
    assert_eq!(s.balance(&s.token, &s.treasury), 13 * UNIT);
    assert_eq!(s.balance(&s.token, &winner), 97 * UNIT + 90 * UNIT);
    s.assert_invariant(&s.token);
}

#[test]
fn zero_fee_pays_everything_to_winner() {
    let env = Env::default();
    let s = Setup::new(&env);
    s.client.set_fee(&s.admin, &0);
    let winner = Address::generate(&env);
    let id = s.fund(1, 1, 100 * UNIT);

    s.client.release(&s.maintainer, &id, &winner);

    assert_eq!(s.balance(&s.token, &winner), 100 * UNIT);
    assert_eq!(s.balance(&s.token, &s.treasury), 0);
    s.assert_invariant(&s.token);
}

#[test]
fn removing_token_blocks_new_funding_but_not_release_refund_or_resolve() {
    let env = Env::default();
    let s = Setup::new(&env);
    let winner = Address::generate(&env);
    let to_release = s.fund(1, 1, 10 * UNIT);
    let to_refund = s.fund(1, 2, 10 * UNIT);
    let to_resolve = s.fund(1, 3, 10 * UNIT);
    s.client.open_dispute(&s.maintainer, &to_resolve);

    s.client.remove_token(&s.admin, &s.token);
    assert_eq!(
        env.events().all().filter_by_contract(&s.contract_id),
        [TokenRemoved {
            field: symbol_short!("token_rm"),
            token: s.token.clone(),
        }
        .to_xdr(&env, &s.contract_id)]
    );

    // No new bounties in the removed token.
    let r = s.client.try_fund_bounty(
        &s.maintainer,
        &1,
        &4,
        &s.token,
        &(10 * UNIT),
        &s.deadline(),
        &true,
    );
    assert_eq!(r, Err(Ok(Error::TokenNotAllowed)));

    // Money already in escrow can still leave by every route.
    s.client.release(&s.maintainer, &to_release, &winner);
    let deadline = s.client.get_bounty(&to_refund).deadline;
    s.set_time(deadline + GRACE_SECS);
    s.client.refund(&s.maintainer, &to_refund);
    s.client
        .resolve(&s.arbiter, &to_resolve, &Some(winner.clone()), &10_000);

    assert_eq!(s.client.get_bounty(&to_release).status, Status::Released);
    assert_eq!(s.client.get_bounty(&to_refund).status, Status::Refunded);
    assert_eq!(s.client.get_bounty(&to_resolve).status, Status::Resolved);
    assert_eq!(s.balance(&s.token, &s.contract_id), 0);
    s.assert_invariant(&s.token);
}

#[test]
fn remove_token_rejects_unknown_token_and_non_admin() {
    let env = Env::default();
    let s = Setup::new(&env);
    let other = Setup::new_token(&env);
    assert_eq!(
        s.client.try_remove_token(&s.admin, &other),
        Err(Ok(Error::TokenNotAllowed))
    );
    let rando = Address::generate(&env);
    assert_eq!(
        s.client.try_remove_token(&rando, &s.token),
        Err(Ok(Error::Unauthorized))
    );
}

#[test]
fn set_admin_needs_both_auths() {
    let env = Env::default();
    let s = Setup::new(&env);
    let new_admin = Address::generate(&env);
    let args: soroban_sdk::Vec<soroban_sdk::Val> = (&s.admin, &new_admin).into_val(&env);

    // Only the current admin signs: rejected.
    env.mock_auths(&[MockAuth {
        address: &s.admin,
        invoke: &MockAuthInvoke {
            contract: &s.contract_id,
            fn_name: "set_admin",
            args: args.clone(),
            sub_invokes: &[],
        },
    }]);
    assert!(matches!(
        s.client.try_set_admin(&s.admin, &new_admin),
        Err(Err(_))
    ));
    assert_eq!(s.client.get_config().admin, s.admin);

    // Only the new admin signs: rejected.
    env.mock_auths(&[MockAuth {
        address: &new_admin,
        invoke: &MockAuthInvoke {
            contract: &s.contract_id,
            fn_name: "set_admin",
            args: args.clone(),
            sub_invokes: &[],
        },
    }]);
    assert!(matches!(
        s.client.try_set_admin(&s.admin, &new_admin),
        Err(Err(_))
    ));
    assert_eq!(s.client.get_config().admin, s.admin);

    // Both sign: accepted, and both authorizations are recorded.
    env.mock_auths(&[
        MockAuth {
            address: &s.admin,
            invoke: &MockAuthInvoke {
                contract: &s.contract_id,
                fn_name: "set_admin",
                args: args.clone(),
                sub_invokes: &[],
            },
        },
        MockAuth {
            address: &new_admin,
            invoke: &MockAuthInvoke {
                contract: &s.contract_id,
                fn_name: "set_admin",
                args: args.clone(),
                sub_invokes: &[],
            },
        },
    ]);
    s.client.set_admin(&s.admin, &new_admin);
    let auths = env.auths();
    assert_eq!(auths.len(), 2);
    assert!(auths.iter().any(|(a, _)| *a == s.admin));
    assert!(auths.iter().any(|(a, _)| *a == new_admin));
    assert_eq!(
        env.events().all().filter_by_contract(&s.contract_id),
        [AddressSet {
            field: symbol_short!("admin"),
            old: s.admin.clone(),
            new: new_admin.clone(),
        }
        .to_xdr(&env, &s.contract_id)]
    );
    assert_eq!(s.client.get_config().admin, new_admin);
}

#[test]
fn set_admin_moves_the_admin_role() {
    let env = Env::default();
    let s = Setup::new(&env);
    let new_admin = Address::generate(&env);
    s.client.set_admin(&s.admin, &new_admin);

    // The old admin is out, the new admin is in.
    assert_eq!(s.client.try_pause(&s.admin), Err(Ok(Error::Unauthorized)));
    s.client.pause(&new_admin);
    assert!(s.client.get_config().paused);
}

#[test]
fn all_setters_require_admin_and_emit_events() {
    let env = Env::default();
    let s = Setup::new(&env);
    let rando = Address::generate(&env);
    let new_addr = Address::generate(&env);
    let unauthorized = Err(Ok(Error::Unauthorized));

    // Every setter rejects a non-admin and leaves config untouched.
    let before = s.client.get_config();
    assert_eq!(s.client.try_set_fee(&rando, &100), unauthorized);
    assert_eq!(s.client.try_set_relayer(&rando, &new_addr), unauthorized);
    assert_eq!(s.client.try_set_arbiter(&rando, &new_addr), unauthorized);
    assert_eq!(s.client.try_set_treasury(&rando, &new_addr), unauthorized);
    assert_eq!(s.client.try_set_grace(&rando, &1), unauthorized);
    assert_eq!(s.client.try_set_durations(&rando, &1, &2), unauthorized);
    assert_eq!(s.client.try_set_admin(&rando, &new_addr), unauthorized);
    assert_eq!(
        s.client
            .try_upgrade(&rando, &BytesN::from_array(&env, &[0; 32])),
        unauthorized
    );
    assert_eq!(s.client.get_config(), before);

    // And the admin can use each one, with the event to show for it.
    s.client.set_relayer(&s.admin, &new_addr);
    assert_eq!(
        env.events().all().filter_by_contract(&s.contract_id),
        [AddressSet {
            field: symbol_short!("relayer"),
            old: s.relayer.clone(),
            new: new_addr.clone(),
        }
        .to_xdr(&env, &s.contract_id)]
    );
    s.client.set_arbiter(&s.admin, &new_addr);
    assert_eq!(
        env.events().all().filter_by_contract(&s.contract_id),
        [AddressSet {
            field: symbol_short!("arbiter"),
            old: s.arbiter.clone(),
            new: new_addr.clone(),
        }
        .to_xdr(&env, &s.contract_id)]
    );
    s.client.set_treasury(&s.admin, &new_addr);
    assert_eq!(
        env.events().all().filter_by_contract(&s.contract_id),
        [AddressSet {
            field: symbol_short!("treasury"),
            old: s.treasury.clone(),
            new: new_addr.clone(),
        }
        .to_xdr(&env, &s.contract_id)]
    );
    s.client.set_grace(&s.admin, &1_234);
    assert_eq!(
        env.events().all().filter_by_contract(&s.contract_id),
        [GraceSet {
            field: symbol_short!("grace"),
            old_secs: GRACE_SECS,
            new_secs: 1_234,
        }
        .to_xdr(&env, &s.contract_id)]
    );
    s.client.set_durations(&s.admin, &10, &20);
    assert_eq!(
        env.events().all().filter_by_contract(&s.contract_id),
        [DurationsSet {
            field: symbol_short!("durations"),
            min_secs: 10,
            max_secs: 20,
        }
        .to_xdr(&env, &s.contract_id)]
    );

    let after = s.client.get_config();
    assert_eq!(after.relayer, new_addr);
    assert_eq!(after.arbiter, new_addr);
    assert_eq!(after.treasury, new_addr);
    assert_eq!(after.grace_secs, 1_234);
    assert_eq!((after.min_duration_secs, after.max_duration_secs), (10, 20));
}

#[test]
fn setters_require_the_admin_signature() {
    let env = Env::default();
    let s = Setup::new(&env);
    let new_addr = Address::generate(&env);
    env.set_auths(&[]);
    assert!(matches!(s.client.try_set_fee(&s.admin, &100), Err(Err(_))));
    assert!(matches!(
        s.client.try_set_relayer(&s.admin, &new_addr),
        Err(Err(_))
    ));
    assert!(matches!(
        s.client.try_remove_token(&s.admin, &s.token),
        Err(Err(_))
    ));
    assert_eq!(s.client.get_config().fee_bps, FEE_BPS);
}

#[test]
fn set_durations_validates_and_changes_the_funding_window() {
    let env = Env::default();
    let s = Setup::new(&env);
    assert_eq!(
        s.client.try_set_durations(&s.admin, &20, &10),
        Err(Ok(Error::InvalidDeadline))
    );

    s.client.set_durations(&s.admin, &(2 * DAY), &(3 * DAY));
    let fund = |deadline: u64, issue: u64| {
        s.client.try_fund_bounty(
            &s.maintainer,
            &1,
            &issue,
            &s.token,
            &(10 * UNIT),
            &deadline,
            &true,
        )
    };
    // The default 14 day deadline is now too far; 1 day is too soon; 2 and 3 days are fine.
    assert_eq!(fund(s.now() + 14 * DAY, 1), Err(Ok(Error::InvalidDeadline)));
    assert_eq!(fund(s.now() + DAY, 1), Err(Ok(Error::InvalidDeadline)));
    assert!(fund(s.now() + 2 * DAY, 1).is_ok());
    assert!(fund(s.now() + 3 * DAY, 2).is_ok());
}

#[test]
fn new_relayer_arbiter_and_treasury_take_effect() {
    let env = Env::default();
    let s = Setup::new(&env);
    let new_relayer = Address::generate(&env);
    let new_arbiter = Address::generate(&env);
    let new_treasury = Address::generate(&env);
    let winner = Address::generate(&env);
    let released = s.fund(1, 1, 100 * UNIT);
    let disputed = s.fund(1, 2, 100 * UNIT);
    s.client.open_dispute(&s.maintainer, &disputed);

    s.client.set_relayer(&s.admin, &new_relayer);
    s.client.set_arbiter(&s.admin, &new_arbiter);
    s.client.set_treasury(&s.admin, &new_treasury);

    // The old relayer and arbiter are out; the new ones are in.
    assert_eq!(
        s.client.try_release(&s.relayer, &released, &winner),
        Err(Ok(Error::Unauthorized))
    );
    assert_eq!(
        s.client
            .try_resolve(&s.arbiter, &disputed, &Some(winner.clone()), &10_000),
        Err(Ok(Error::Unauthorized))
    );
    s.client.release(&new_relayer, &released, &winner);
    s.client
        .resolve(&new_arbiter, &disputed, &Some(winner.clone()), &10_000);

    // Fees go to the new treasury only.
    assert_eq!(s.balance(&s.token, &new_treasury), 6 * UNIT);
    assert_eq!(s.balance(&s.token, &s.treasury), 0);
    s.assert_invariant(&s.token);
}

#[test]
fn upgrade_with_an_unknown_wasm_is_rejected() {
    let env = Env::default();
    let s = Setup::new(&env);
    let r = s
        .client
        .try_upgrade(&s.admin, &BytesN::from_array(&env, &[7; 32]));
    assert!(r.is_err());
    // The contract keeps working on its current code.
    assert_eq!(s.client.get_config().admin, s.admin);
}
