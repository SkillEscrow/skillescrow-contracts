use soroban_sdk::{
    testutils::{Address as _, AuthorizedFunction, AuthorizedInvocation, Events as _},
    Address, Env, Event as _, IntoVal, Symbol,
};

use super::setup::*;
use crate::errors::Error;
use crate::events::Released;
use crate::fees;
use crate::types::{Status, Totals};

const REPO: u64 = 7;
const ISSUE: u64 = 9;

/// Funds a bounty from the default maintainer with the given relayer opt-in.
fn fund_with(s: &Setup, relayer_allowed: bool, amount: i128) -> u64 {
    s.client.fund_bounty(
        &s.maintainer,
        &REPO,
        &ISSUE,
        &s.token,
        &amount,
        &s.deadline(),
        &relayer_allowed,
    )
}

#[test]
fn maintainer_releases() {
    let env = Env::default();
    let s = Setup::new(&env);
    let winner = Address::generate(&env);
    let amount = 100 * UNIT;
    let id = fund_with(&s, false, amount);

    s.client.release(&s.maintainer, &id, &winner);

    let fee = fees::fee(amount, FEE_BPS).unwrap();
    let net = amount - fee;
    let bounty = s.client.get_bounty(&id);
    assert_eq!(bounty.status, Status::Released);
    assert_eq!(bounty.winner, Some(winner.clone()));
    assert_eq!(s.balance(&s.token, &winner), net);
    assert_eq!(s.balance(&s.token, &s.treasury), fee);
    assert_eq!(s.balance(&s.token, &s.contract_id), 0);
    assert_eq!(s.client.active_bounty(&REPO, &ISSUE), None);
    assert_eq!(
        s.assert_invariant(&s.token),
        Totals {
            funded: amount,
            released_net: net,
            fees: fee,
            refunded: 0,
            held: 0,
        }
    );
}

#[test]
fn relayer_releases_when_allowed() {
    let env = Env::default();
    let s = Setup::new(&env);
    let winner = Address::generate(&env);
    let id = fund_with(&s, true, 100 * UNIT);

    s.client.release(&s.relayer, &id, &winner);

    assert_eq!(s.client.get_bounty(&id).status, Status::Released);
    assert_eq!(s.balance(&s.token, &winner), 97 * UNIT);
    assert_eq!(s.balance(&s.token, &s.treasury), 3 * UNIT);
    s.assert_invariant(&s.token);
}

#[test]
fn relayer_rejected_when_not_allowed() {
    let env = Env::default();
    let s = Setup::new(&env);
    let winner = Address::generate(&env);
    let id = fund_with(&s, false, 100 * UNIT);

    let r = s.client.try_release(&s.relayer, &id, &winner);
    assert_eq!(r, Err(Ok(Error::RelayerNotAllowed)));
    assert_eq!(s.client.get_bounty(&id).status, Status::Open);
    assert_eq!(s.balance(&s.token, &s.contract_id), 100 * UNIT);
    assert_eq!(s.balance(&s.token, &winner), 0);
}

#[test]
fn random_address_rejected() {
    let env = Env::default();
    let s = Setup::new(&env);
    let winner = Address::generate(&env);
    let rando = Address::generate(&env);
    let id = fund_with(&s, true, 100 * UNIT);

    for caller in [&rando, &winner, &s.arbiter, &s.admin, &s.treasury] {
        let r = s.client.try_release(caller, &id, &winner);
        assert_eq!(r, Err(Ok(Error::Unauthorized)));
    }
    assert_eq!(s.client.get_bounty(&id).status, Status::Open);
    assert_eq!(s.balance(&s.token, &s.contract_id), 100 * UNIT);
}

#[test]
fn self_payout_rejected() {
    let env = Env::default();
    let s = Setup::new(&env);
    let id = fund_with(&s, true, 100 * UNIT);

    // Neither the maintainer nor the relayer can pay the maintainer.
    assert_eq!(
        s.client.try_release(&s.maintainer, &id, &s.maintainer),
        Err(Ok(Error::SelfPayout))
    );
    assert_eq!(
        s.client.try_release(&s.relayer, &id, &s.maintainer),
        Err(Ok(Error::SelfPayout))
    );
    assert_eq!(s.client.get_bounty(&id).status, Status::Open);
    assert_eq!(s.balance(&s.token, &s.contract_id), 100 * UNIT);
}

#[test]
fn release_after_deadline_but_before_refund_works() {
    let env = Env::default();
    let s = Setup::new(&env);
    let winner = Address::generate(&env);
    let id = fund_with(&s, true, 100 * UNIT);
    let deadline = s.client.get_bounty(&id).deadline;

    // Past the deadline but inside the grace window.
    s.set_time(deadline + 1);
    s.client.release(&s.maintainer, &id, &winner);
    assert_eq!(s.client.get_bounty(&id).status, Status::Released);
    assert_eq!(s.balance(&s.token, &winner), 97 * UNIT);

    // Even long after the refund window opened, release is fine while nobody has refunded.
    let id2 = s.client.fund_bounty(
        &s.maintainer,
        &REPO,
        &(ISSUE + 1),
        &s.token,
        &(100 * UNIT),
        &(s.now() + 2 * DAY),
        &true,
    );
    let deadline2 = s.client.get_bounty(&id2).deadline;
    s.set_time(deadline2 + GRACE_SECS + 30 * DAY);
    s.client.release(&s.relayer, &id2, &winner);
    assert_eq!(s.client.get_bounty(&id2).status, Status::Released);
    s.assert_invariant(&s.token);
}

#[test]
fn fee_correct_treasury_receives_fee() {
    let env = Env::default();
    let s = Setup::new(&env);
    let winner = Address::generate(&env);
    // An amount that does not divide evenly: 300 bps of 10_000_001 is 300_000.03.
    let amount = 10_000_001;
    let id = fund_with(&s, true, amount);

    s.client.release(&s.maintainer, &id, &winner);

    assert_eq!(s.balance(&s.token, &s.treasury), 300_000);
    // Rounding dust stays with the contributor.
    assert_eq!(s.balance(&s.token, &winner), 9_700_001);
    assert_eq!(
        s.balance(&s.token, &winner) + s.balance(&s.token, &s.treasury),
        amount
    );
    assert_eq!(s.assert_invariant(&s.token).fees, 300_000);
}

#[test]
fn double_release_rejected() {
    let env = Env::default();
    let s = Setup::new(&env);
    let winner = Address::generate(&env);
    let id = fund_with(&s, true, 100 * UNIT);

    s.client.release(&s.maintainer, &id, &winner);
    let again = s.client.try_release(&s.maintainer, &id, &winner);
    assert_eq!(again, Err(Ok(Error::InvalidStatus)));
    let again_relayer = s.client.try_release(&s.relayer, &id, &winner);
    assert_eq!(again_relayer, Err(Ok(Error::InvalidStatus)));

    // Paid exactly once.
    assert_eq!(s.balance(&s.token, &winner), 97 * UNIT);
    assert_eq!(s.balance(&s.token, &s.treasury), 3 * UNIT);
    s.assert_invariant(&s.token);
}

#[test]
fn unknown_bounty_rejected() {
    let env = Env::default();
    let s = Setup::new(&env);
    let winner = Address::generate(&env);
    assert_eq!(
        s.client.try_release(&s.maintainer, &99, &winner),
        Err(Ok(Error::BountyNotFound))
    );
}

#[test]
fn works_while_paused() {
    let env = Env::default();
    let s = Setup::new(&env);
    let winner = Address::generate(&env);
    let id = fund_with(&s, true, 100 * UNIT);

    s.client.pause(&s.admin);
    s.client.release(&s.maintainer, &id, &winner);
    assert_eq!(s.client.get_bounty(&id).status, Status::Released);
}

#[test]
fn emits_released_event() {
    let env = Env::default();
    let s = Setup::new(&env);
    let winner = Address::generate(&env);
    let id = fund_with(&s, true, 100 * UNIT);

    s.client.release(&s.relayer, &id, &winner);

    assert_eq!(
        env.events().all().filter_by_contract(&s.contract_id),
        [Released {
            id,
            winner: winner.clone(),
            net: 97 * UNIT,
            fee: 3 * UNIT,
            caller: s.relayer.clone(),
        }
        .to_xdr(&env, &s.contract_id)]
    );
}

#[test]
fn requires_caller_auth() {
    let env = Env::default();
    let s = Setup::new(&env);
    let winner = Address::generate(&env);
    let id = fund_with(&s, true, 100 * UNIT);

    s.client.release(&s.relayer, &id, &winner);
    assert_eq!(
        env.auths(),
        [(
            s.relayer.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.contract_id.clone(),
                    Symbol::new(&env, "release"),
                    (&s.relayer, id, &winner).into_val(&env),
                )),
                sub_invocations: [].into(),
            }
        )]
    );
}

#[test]
fn unsigned_release_is_rejected() {
    let env = Env::default();
    let s = Setup::new(&env);
    let winner = Address::generate(&env);
    let id = fund_with(&s, true, 100 * UNIT);

    // Fixtures mocked all auths; turn mocking off so a missing signature is an error.
    env.set_auths(&[]);
    let r = s.client.try_release(&s.maintainer, &id, &winner);
    assert!(matches!(r, Err(Err(_))), "{r:?}");
    assert_eq!(s.client.get_bounty(&id).status, Status::Open);
    assert_eq!(s.balance(&s.token, &winner), 0);
}

#[test]
fn release_on_disputed_rejected() {
    let env = Env::default();
    let s = Setup::new(&env);
    let winner = Address::generate(&env);
    let id = fund_with(&s, true, 100 * UNIT);
    s.client.open_dispute(&s.maintainer, &id);

    let r = s.client.try_release(&s.maintainer, &id, &winner);
    assert_eq!(r, Err(Ok(Error::InvalidStatus)));
    assert_eq!(s.client.get_bounty(&id).status, Status::Disputed);
    assert_eq!(s.balance(&s.token, &winner), 0);
}
