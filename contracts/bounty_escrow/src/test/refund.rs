use soroban_sdk::{
    testutils::{Address as _, AuthorizedFunction, AuthorizedInvocation, Events as _},
    Address, Env, Event as _, IntoVal, Symbol,
};

use super::setup::*;
use crate::errors::Error;
use crate::events::Refunded;
use crate::types::{Status, Totals};

const REPO: u64 = 7;
const ISSUE: u64 = 9;

/// Funds a 100 unit bounty and returns `(id, unlock_time)`, where `unlock_time` is
/// `deadline + grace`.
fn funded(s: &Setup) -> (u64, u64) {
    let id = s.fund(REPO, ISSUE, 100 * UNIT);
    let deadline = s.client.get_bounty(&id).deadline;
    (id, deadline + GRACE_SECS)
}

#[test]
fn rejected_before_deadline_plus_grace() {
    let env = Env::default();
    let s = Setup::new(&env);
    let (id, unlock) = funded(&s);
    let deadline = unlock - GRACE_SECS;

    // Right after funding, at the deadline, and one second before the window opens.
    for t in [s.now(), deadline, unlock - 1] {
        s.set_time(t);
        let r = s.client.try_refund(&s.maintainer, &id);
        assert_eq!(r, Err(Ok(Error::DeadlineNotReached)), "t = {t}");
    }
    assert_eq!(s.client.get_bounty(&id).status, Status::Open);
    assert_eq!(s.balance(&s.token, &s.contract_id), 100 * UNIT);
    s.assert_invariant(&s.token);
}

#[test]
fn works_at_exactly_deadline_plus_grace() {
    let env = Env::default();
    let s = Setup::new(&env);
    let (id, unlock) = funded(&s);
    let before = s.balance(&s.token, &s.maintainer);

    s.set_time(unlock);
    s.client.refund(&s.maintainer, &id);

    assert_eq!(s.client.get_bounty(&id).status, Status::Refunded);
    assert_eq!(s.balance(&s.token, &s.maintainer), before + 100 * UNIT);
    assert_eq!(s.client.active_bounty(&REPO, &ISSUE), None);
    assert_eq!(
        s.assert_invariant(&s.token),
        Totals {
            funded: 100 * UNIT,
            released_net: 0,
            fees: 0,
            refunded: 100 * UNIT,
            held: 0,
        }
    );
}

#[test]
fn works_while_paused() {
    let env = Env::default();
    let s = Setup::new(&env);
    let (id, unlock) = funded(&s);

    s.client.pause(&s.admin);
    s.set_time(unlock + 1);
    s.client.refund(&s.maintainer, &id);
    assert_eq!(s.client.get_bounty(&id).status, Status::Refunded);
}

#[test]
fn no_fee_taken() {
    let env = Env::default();
    let s = Setup::new(&env);
    let (id, unlock) = funded(&s);
    let before = s.balance(&s.token, &s.maintainer);

    s.set_time(unlock);
    s.client.refund(&s.maintainer, &id);

    // The maintainer gets back everything, and the treasury gets nothing.
    assert_eq!(s.balance(&s.token, &s.maintainer), before + 100 * UNIT);
    assert_eq!(s.balance(&s.token, &s.treasury), 0);
    assert_eq!(s.assert_invariant(&s.token).fees, 0);
}

#[test]
fn non_maintainer_rejected() {
    let env = Env::default();
    let s = Setup::new(&env);
    let (id, unlock) = funded(&s);
    s.set_time(unlock);

    let rando = Address::generate(&env);
    for caller in [&rando, &s.relayer, &s.arbiter, &s.admin, &s.treasury] {
        let r = s.client.try_refund(caller, &id);
        assert_eq!(r, Err(Ok(Error::Unauthorized)));
    }
    assert_eq!(s.client.get_bounty(&id).status, Status::Open);
    assert_eq!(s.balance(&s.token, &s.contract_id), 100 * UNIT);
}

#[test]
fn refund_after_release_rejected() {
    let env = Env::default();
    let s = Setup::new(&env);
    let (id, unlock) = funded(&s);
    let winner = Address::generate(&env);
    s.client.release(&s.maintainer, &id, &winner);

    s.set_time(unlock + DAY);
    let r = s.client.try_refund(&s.maintainer, &id);
    assert_eq!(r, Err(Ok(Error::InvalidStatus)));
    // The winner keeps what they were paid; nothing came back to the maintainer.
    assert_eq!(s.balance(&s.token, &winner), 97 * UNIT);
    s.assert_invariant(&s.token);
}

#[test]
fn double_refund_rejected() {
    let env = Env::default();
    let s = Setup::new(&env);
    let (id, unlock) = funded(&s);
    s.set_time(unlock);
    s.client.refund(&s.maintainer, &id);

    let r = s.client.try_refund(&s.maintainer, &id);
    assert_eq!(r, Err(Ok(Error::InvalidStatus)));
    assert_eq!(s.balance(&s.token, &s.maintainer), 1_000 * UNIT);
    s.assert_invariant(&s.token);
}

#[test]
fn release_after_refund_rejected() {
    let env = Env::default();
    let s = Setup::new(&env);
    let (id, unlock) = funded(&s);
    s.set_time(unlock);
    s.client.refund(&s.maintainer, &id);

    let winner = Address::generate(&env);
    let r = s.client.try_release(&s.maintainer, &id, &winner);
    assert_eq!(r, Err(Ok(Error::InvalidStatus)));
    assert_eq!(s.balance(&s.token, &winner), 0);
}

#[test]
fn unknown_bounty_rejected() {
    let env = Env::default();
    let s = Setup::new(&env);
    assert_eq!(
        s.client.try_refund(&s.maintainer, &5),
        Err(Ok(Error::BountyNotFound))
    );
}

#[test]
fn emits_refunded_event() {
    let env = Env::default();
    let s = Setup::new(&env);
    let (id, unlock) = funded(&s);
    s.set_time(unlock);

    s.client.refund(&s.maintainer, &id);

    assert_eq!(
        env.events().all().filter_by_contract(&s.contract_id),
        [Refunded {
            id,
            amount: 100 * UNIT,
        }
        .to_xdr(&env, &s.contract_id)]
    );
}

#[test]
fn requires_maintainer_auth() {
    let env = Env::default();
    let s = Setup::new(&env);
    let (id, unlock) = funded(&s);
    s.set_time(unlock);

    s.client.refund(&s.maintainer, &id);
    assert_eq!(
        env.auths(),
        [(
            s.maintainer.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.contract_id.clone(),
                    Symbol::new(&env, "refund"),
                    (&s.maintainer, id).into_val(&env),
                )),
                sub_invocations: [].into(),
            }
        )]
    );
}

#[test]
fn unsigned_refund_is_rejected() {
    let env = Env::default();
    let s = Setup::new(&env);
    let (id, unlock) = funded(&s);
    s.set_time(unlock);

    env.set_auths(&[]);
    let r = s.client.try_refund(&s.maintainer, &id);
    assert!(matches!(r, Err(Err(_))), "{r:?}");
    assert_eq!(s.client.get_bounty(&id).status, Status::Open);
}
