use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, AuthorizedFunction, AuthorizedInvocation, Events as _},
    Address, Env, Event as _, IntoVal, Symbol,
};

use super::setup::*;
use crate::errors::Error;
use crate::events::Funded;
use crate::types::{Bounty, Status, Totals};

const REPO: u64 = 1_001;
const ISSUE: u64 = 42;

#[test]
fn happy_path_stores_bounty_and_moves_tokens() {
    let env = Env::default();
    let s = Setup::new(&env);
    let amount = 50 * UNIT;
    let deadline = s.deadline();
    let before = s.balance(&s.token, &s.maintainer);

    let id = s.client.fund_bounty(
        &s.maintainer,
        &REPO,
        &ISSUE,
        &s.token,
        &amount,
        &deadline,
        &true,
    );

    assert_eq!(id, 1);
    assert_eq!(
        s.client.get_bounty(&id),
        Bounty {
            id: 1,
            maintainer: s.maintainer.clone(),
            repo_id: REPO,
            issue: ISSUE,
            token: s.token.clone(),
            amount,
            fee_bps: FEE_BPS,
            deadline,
            relayer_allowed: true,
            status: Status::Open,
            winner: None,
            created_at: START_TIME,
        }
    );
    assert_eq!(s.client.active_bounty(&REPO, &ISSUE), Some(id));
    assert_eq!(s.balance(&s.token, &s.maintainer), before - amount);
    assert_eq!(s.balance(&s.token, &s.contract_id), amount);
    assert_eq!(
        s.assert_invariant(&s.token),
        Totals {
            funded: amount,
            released_net: 0,
            fees: 0,
            refunded: 0,
            held: amount,
        }
    );
}

#[test]
fn emits_funded_event() {
    let env = Env::default();
    let s = Setup::new(&env);
    let amount = 50 * UNIT;
    let deadline = s.deadline();
    s.client.fund_bounty(
        &s.maintainer,
        &REPO,
        &ISSUE,
        &s.token,
        &amount,
        &deadline,
        &false,
    );
    assert_eq!(
        env.events().all().filter_by_contract(&s.contract_id),
        [Funded {
            id: 1,
            maintainer: s.maintainer.clone(),
            repo_id: REPO,
            issue: ISSUE,
            token: s.token.clone(),
            amount,
            fee_bps: FEE_BPS,
            deadline,
            relayer_allowed: false,
        }
        .to_xdr(&env, &s.contract_id)]
    );
}

#[test]
fn ids_increment_and_totals_accumulate() {
    let env = Env::default();
    let s = Setup::new(&env);
    assert_eq!(s.fund(REPO, 1, 10 * UNIT), 1);
    assert_eq!(s.fund(REPO, 2, 20 * UNIT), 2);
    assert_eq!(s.fund(REPO + 1, 1, 30 * UNIT), 3);
    assert_eq!(s.assert_invariant(&s.token).funded, 60 * UNIT);
    assert_eq!(s.client.get_bounty(&2).amount, 20 * UNIT);
}

#[test]
fn snapshots_fee_at_funding_time() {
    let env = Env::default();
    let s = Setup::new(&env);
    let id = s.fund(REPO, ISSUE, 10 * UNIT);
    assert_eq!(s.client.get_bounty(&id).fee_bps, FEE_BPS);
}

#[test]
fn rejects_when_not_initialized() {
    let env = Env::default();
    let s = Setup::uninitialized(&env);
    let r = s.client.try_fund_bounty(
        &s.maintainer,
        &REPO,
        &ISSUE,
        &s.token,
        &(10 * UNIT),
        &s.deadline(),
        &true,
    );
    assert_eq!(r, Err(Ok(Error::NotInitialized)));
}

#[test]
fn rejects_when_paused() {
    let env = Env::default();
    let s = Setup::new(&env);
    s.client.pause(&s.admin);
    let before = s.balance(&s.token, &s.maintainer);
    let r = s.client.try_fund_bounty(
        &s.maintainer,
        &REPO,
        &ISSUE,
        &s.token,
        &(10 * UNIT),
        &s.deadline(),
        &true,
    );
    assert_eq!(r, Err(Ok(Error::Paused)));
    assert_eq!(s.balance(&s.token, &s.maintainer), before);

    // Funding works again after unpause.
    s.client.unpause(&s.admin);
    assert_eq!(s.fund(REPO, ISSUE, 10 * UNIT), 1);
}

#[test]
fn rejects_non_allowlisted_token() {
    let env = Env::default();
    let s = Setup::new(&env);
    let other = Setup::new_token(&env);
    s.mint(&other, &s.maintainer, 100 * UNIT);
    let r = s.client.try_fund_bounty(
        &s.maintainer,
        &REPO,
        &ISSUE,
        &other,
        &(10 * UNIT),
        &s.deadline(),
        &true,
    );
    assert_eq!(r, Err(Ok(Error::TokenNotAllowed)));
    assert_eq!(s.balance(&other, &s.maintainer), 100 * UNIT);
}

#[test]
fn rejects_below_min() {
    let env = Env::default();
    let s = Setup::new(&env);
    let r = s.client.try_fund_bounty(
        &s.maintainer,
        &REPO,
        &ISSUE,
        &s.token,
        &(MIN_AMOUNT - 1),
        &s.deadline(),
        &true,
    );
    assert_eq!(r, Err(Ok(Error::AmountTooSmall)));
    let zero = s.client.try_fund_bounty(
        &s.maintainer,
        &REPO,
        &ISSUE,
        &s.token,
        &0,
        &s.deadline(),
        &true,
    );
    assert_eq!(zero, Err(Ok(Error::AmountTooSmall)));
    let negative = s.client.try_fund_bounty(
        &s.maintainer,
        &REPO,
        &ISSUE,
        &s.token,
        &-5,
        &s.deadline(),
        &true,
    );
    assert_eq!(negative, Err(Ok(Error::AmountTooSmall)));
    // Exactly the minimum is accepted.
    assert_eq!(s.fund(REPO, ISSUE, MIN_AMOUNT), 1);
}

#[test]
fn rejects_above_max() {
    let env = Env::default();
    let s = Setup::new(&env);
    s.mint(&s.token, &s.maintainer, 10_000 * UNIT);
    let r = s.client.try_fund_bounty(
        &s.maintainer,
        &REPO,
        &ISSUE,
        &s.token,
        &(MAX_AMOUNT + 1),
        &s.deadline(),
        &true,
    );
    assert_eq!(r, Err(Ok(Error::AmountTooLarge)));
    // Exactly the maximum is accepted.
    assert_eq!(s.fund(REPO, ISSUE, MAX_AMOUNT), 1);
}

#[test]
fn rejects_deadline_too_soon() {
    let env = Env::default();
    let s = Setup::new(&env);
    let r = s.client.try_fund_bounty(
        &s.maintainer,
        &REPO,
        &ISSUE,
        &s.token,
        &(10 * UNIT),
        &(s.now() + MIN_DURATION - 1),
        &true,
    );
    assert_eq!(r, Err(Ok(Error::InvalidDeadline)));
    let past = s.client.try_fund_bounty(
        &s.maintainer,
        &REPO,
        &ISSUE,
        &s.token,
        &(10 * UNIT),
        &0,
        &true,
    );
    assert_eq!(past, Err(Ok(Error::InvalidDeadline)));
    // Exactly now + min duration is accepted.
    let id = s.client.fund_bounty(
        &s.maintainer,
        &REPO,
        &ISSUE,
        &s.token,
        &(10 * UNIT),
        &(s.now() + MIN_DURATION),
        &true,
    );
    assert_eq!(id, 1);
}

#[test]
fn rejects_deadline_too_far() {
    let env = Env::default();
    let s = Setup::new(&env);
    let r = s.client.try_fund_bounty(
        &s.maintainer,
        &REPO,
        &ISSUE,
        &s.token,
        &(10 * UNIT),
        &(s.now() + MAX_DURATION + 1),
        &true,
    );
    assert_eq!(r, Err(Ok(Error::InvalidDeadline)));
    let huge = s.client.try_fund_bounty(
        &s.maintainer,
        &REPO,
        &ISSUE,
        &s.token,
        &(10 * UNIT),
        &u64::MAX,
        &true,
    );
    assert_eq!(huge, Err(Ok(Error::InvalidDeadline)));
    // Exactly now + max duration is accepted.
    let id = s.client.fund_bounty(
        &s.maintainer,
        &REPO,
        &ISSUE,
        &s.token,
        &(10 * UNIT),
        &(s.now() + MAX_DURATION),
        &true,
    );
    assert_eq!(id, 1);
}

#[test]
fn rejects_second_bounty_on_same_issue() {
    let env = Env::default();
    let s = Setup::new(&env);
    s.fund(REPO, ISSUE, 10 * UNIT);
    let balance = s.balance(&s.token, &s.maintainer);

    let r = s.client.try_fund_bounty(
        &s.maintainer,
        &REPO,
        &ISSUE,
        &s.token,
        &(10 * UNIT),
        &s.deadline(),
        &true,
    );
    assert_eq!(r, Err(Ok(Error::IssueAlreadyFunded)));

    // A different maintainer cannot fund the same issue either.
    let other = Address::generate(&env);
    s.mint(&s.token, &other, 100 * UNIT);
    let r2 = s.client.try_fund_bounty(
        &other,
        &REPO,
        &ISSUE,
        &s.token,
        &(10 * UNIT),
        &s.deadline(),
        &true,
    );
    assert_eq!(r2, Err(Ok(Error::IssueAlreadyFunded)));

    // Nothing moved and the original bounty is untouched.
    assert_eq!(s.balance(&s.token, &s.maintainer), balance);
    assert_eq!(s.client.active_bounty(&REPO, &ISSUE), Some(1));
    s.assert_invariant(&s.token);

    // The same issue number in another repo, and another issue, are independent.
    assert_eq!(s.fund(REPO + 1, ISSUE, 10 * UNIT), 2);
    assert_eq!(s.fund(REPO, ISSUE + 1, 10 * UNIT), 3);
}

#[test]
fn requires_maintainer_auth() {
    let env = Env::default();
    let s = Setup::new(&env);
    let amount = 10 * UNIT;
    let deadline = s.deadline();

    // Fixtures mocked all auths; turn mocking off so a missing signature is an error.
    env.set_auths(&[]);
    let before = s.balance(&s.token, &s.maintainer);
    let r = s.client.try_fund_bounty(
        &s.maintainer,
        &REPO,
        &ISSUE,
        &s.token,
        &amount,
        &deadline,
        &true,
    );
    // A host-level auth failure, not one of the contract's own errors.
    assert!(matches!(r, Err(Err(_))), "{r:?}");
    assert_eq!(s.balance(&s.token, &s.maintainer), before);
    assert_eq!(s.client.active_bounty(&REPO, &ISSUE), None);
}

#[test]
fn authorizes_exactly_the_funding_call_and_token_transfer() {
    let env = Env::default();
    let s = Setup::new(&env);
    let amount = 10 * UNIT;
    let deadline = s.deadline();
    s.client.fund_bounty(
        &s.maintainer,
        &REPO,
        &ISSUE,
        &s.token,
        &amount,
        &deadline,
        &true,
    );
    assert_eq!(
        env.auths(),
        [(
            s.maintainer.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.contract_id.clone(),
                    Symbol::new(&env, "fund_bounty"),
                    (&s.maintainer, REPO, ISSUE, &s.token, amount, deadline, true).into_val(&env),
                )),
                sub_invocations: [AuthorizedInvocation {
                    function: AuthorizedFunction::Contract((
                        s.token.clone(),
                        symbol_short!("transfer"),
                        (&s.maintainer, &s.contract_id, amount).into_val(&env),
                    )),
                    sub_invocations: [].into(),
                }]
                .into(),
            }
        )]
    );
}

#[test]
fn insufficient_balance_fails_without_changing_state() {
    let env = Env::default();
    let s = Setup::new(&env);
    let poor = Address::generate(&env);
    s.mint(&s.token, &poor, MIN_AMOUNT);
    let r = s.client.try_fund_bounty(
        &poor,
        &REPO,
        &ISSUE,
        &s.token,
        &(10 * UNIT),
        &s.deadline(),
        &true,
    );
    assert!(r.is_err());
    // The failed transaction is rolled back: no bounty, no issue lock, no totals.
    assert_eq!(s.client.active_bounty(&REPO, &ISSUE), None);
    assert_eq!(s.client.try_get_bounty(&1), Err(Ok(Error::BountyNotFound)));
    assert_eq!(s.balance(&s.token, &poor), MIN_AMOUNT);
    assert_eq!(s.assert_invariant(&s.token), Totals::default());
    // The next successful bounty still gets id 1.
    assert_eq!(s.fund(REPO, ISSUE, 10 * UNIT), 1);
}

#[test]
fn allows_new_bounty_on_issue_after_previous_one_is_terminal() {
    let env = Env::default();
    let s = Setup::new(&env);
    let winner = Address::generate(&env);

    // Released.
    let first = s.fund(REPO, ISSUE, 10 * UNIT);
    assert_eq!(s.client.active_bounty(&REPO, &ISSUE), Some(first));
    s.client.release(&s.maintainer, &first, &winner);
    assert_eq!(s.client.active_bounty(&REPO, &ISSUE), None);

    let second = s.fund(REPO, ISSUE, 20 * UNIT);
    assert_eq!(second, first + 1);
    assert_eq!(s.client.active_bounty(&REPO, &ISSUE), Some(second));
    // The old bounty keeps its own record.
    assert_eq!(s.client.get_bounty(&first).amount, 10 * UNIT);
    assert_eq!(s.client.get_bounty(&first).status, Status::Released);

    // Refunded.
    let deadline = s.client.get_bounty(&second).deadline;
    s.set_time(deadline + GRACE_SECS);
    s.client.refund(&s.maintainer, &second);
    assert_eq!(s.client.active_bounty(&REPO, &ISSUE), None);
    let third = s.fund(REPO, ISSUE, 30 * UNIT);
    assert_eq!(third, second + 1);
    assert_eq!(s.client.get_bounty(&second).status, Status::Refunded);
    s.assert_invariant(&s.token);
}
