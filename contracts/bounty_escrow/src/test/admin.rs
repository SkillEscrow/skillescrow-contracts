use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, AuthorizedFunction, AuthorizedInvocation, Events as _},
    Address, Env, Event as _, IntoVal,
};

use super::setup::*;
use crate::errors::Error;
use crate::events::{Paused, TokenSet, Unpaused};
use crate::types::Config;

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
