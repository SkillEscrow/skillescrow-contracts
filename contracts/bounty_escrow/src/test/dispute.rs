use soroban_sdk::{
    testutils::{Address as _, AuthorizedFunction, AuthorizedInvocation, Events as _},
    Address, Env, Event as _, IntoVal, Symbol,
};

use super::setup::*;
use crate::errors::Error;
use crate::events::{Disputed, Resolved};
use crate::fees;
use crate::types::{Status, Totals};

const REPO: u64 = 7;
const ISSUE: u64 = 9;

/// Funds a 100 unit bounty (relayer allowed) and returns its id.
fn funded(s: &Setup) -> u64 {
    s.fund(REPO, ISSUE, 100 * UNIT)
}

/// Funds and disputes a 100 unit bounty.
fn disputed(s: &Setup) -> u64 {
    let id = funded(s);
    s.client.open_dispute(&s.maintainer, &id);
    id
}

#[test]
fn maintainer_opens() {
    let env = Env::default();
    let s = Setup::new(&env);
    let id = funded(&s);

    s.client.open_dispute(&s.maintainer, &id);

    assert_eq!(s.client.get_bounty(&id).status, Status::Disputed);
    // Funds stay in escrow and the issue stays locked.
    assert_eq!(s.balance(&s.token, &s.contract_id), 100 * UNIT);
    assert_eq!(s.client.active_bounty(&REPO, &ISSUE), Some(id));
    s.assert_invariant(&s.token);
}

#[test]
fn arbiter_opens() {
    let env = Env::default();
    let s = Setup::new(&env);
    let id = funded(&s);

    s.client.open_dispute(&s.arbiter, &id);
    assert_eq!(s.client.get_bounty(&id).status, Status::Disputed);
}

#[test]
fn random_rejected() {
    let env = Env::default();
    let s = Setup::new(&env);
    let id = funded(&s);
    let rando = Address::generate(&env);

    for caller in [&rando, &s.relayer, &s.admin, &s.treasury] {
        let r = s.client.try_open_dispute(caller, &id);
        assert_eq!(r, Err(Ok(Error::Unauthorized)));
    }
    assert_eq!(s.client.get_bounty(&id).status, Status::Open);
}

#[test]
fn open_dispute_requires_an_open_bounty() {
    let env = Env::default();
    let s = Setup::new(&env);
    let id = funded(&s);
    s.client.open_dispute(&s.maintainer, &id);
    // Already disputed.
    assert_eq!(
        s.client.try_open_dispute(&s.maintainer, &id),
        Err(Ok(Error::InvalidStatus))
    );

    // Released.
    let id2 = s.fund(REPO, ISSUE + 1, 10 * UNIT);
    s.client
        .release(&s.maintainer, &id2, &Address::generate(&env));
    assert_eq!(
        s.client.try_open_dispute(&s.maintainer, &id2),
        Err(Ok(Error::InvalidStatus))
    );

    // Unknown.
    assert_eq!(
        s.client.try_open_dispute(&s.maintainer, &99),
        Err(Ok(Error::BountyNotFound))
    );
}

#[test]
fn open_dispute_works_while_paused() {
    let env = Env::default();
    let s = Setup::new(&env);
    let id = funded(&s);
    s.client.pause(&s.admin);
    s.client.open_dispute(&s.maintainer, &id);
    assert_eq!(s.client.get_bounty(&id).status, Status::Disputed);
}

#[test]
fn blocks_release_and_refund() {
    let env = Env::default();
    let s = Setup::new(&env);
    let id = disputed(&s);
    let winner = Address::generate(&env);

    assert_eq!(
        s.client.try_release(&s.maintainer, &id, &winner),
        Err(Ok(Error::InvalidStatus))
    );
    assert_eq!(
        s.client.try_release(&s.relayer, &id, &winner),
        Err(Ok(Error::InvalidStatus))
    );
    let deadline = s.client.get_bounty(&id).deadline;
    s.set_time(deadline + GRACE_SECS + DAY);
    assert_eq!(
        s.client.try_refund(&s.maintainer, &id),
        Err(Ok(Error::InvalidStatus))
    );
    // Nothing moved.
    assert_eq!(s.balance(&s.token, &s.contract_id), 100 * UNIT);
    assert_eq!(s.balance(&s.token, &winner), 0);
    s.assert_invariant(&s.token);
}

#[test]
fn dispute_keeps_issue_locked_until_resolved() {
    let env = Env::default();
    let s = Setup::new(&env);
    let id = disputed(&s);

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

    s.client.resolve(&s.arbiter, &id, &None, &0);
    assert_eq!(s.client.active_bounty(&REPO, &ISSUE), None);
    assert_eq!(s.fund(REPO, ISSUE, 10 * UNIT), id + 1);
}

#[test]
fn resolve_0_bps() {
    let env = Env::default();
    let s = Setup::new(&env);
    let id = disputed(&s);
    let before = s.balance(&s.token, &s.maintainer);

    s.client.resolve(&s.arbiter, &id, &None, &0);

    let bounty = s.client.get_bounty(&id);
    assert_eq!(bounty.status, Status::Resolved);
    assert_eq!(bounty.winner, None);
    // Everything goes back to the maintainer, with no fee.
    assert_eq!(s.balance(&s.token, &s.maintainer), before + 100 * UNIT);
    assert_eq!(s.balance(&s.token, &s.treasury), 0);
    assert_eq!(s.balance(&s.token, &s.contract_id), 0);
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
fn resolve_5000_bps() {
    let env = Env::default();
    let s = Setup::new(&env);
    let id = disputed(&s);
    let winner = Address::generate(&env);
    let before = s.balance(&s.token, &s.maintainer);

    s.client
        .resolve(&s.arbiter, &id, &Some(winner.clone()), &5_000);

    // Portion 50 units; fee is 3% of that portion = 1.5 units.
    let portion = 50 * UNIT;
    let fee = 15_000_000;
    assert_eq!(fee, fees::fee(portion, FEE_BPS).unwrap());
    assert_eq!(s.balance(&s.token, &winner), portion - fee);
    assert_eq!(s.balance(&s.token, &s.treasury), fee);
    assert_eq!(s.balance(&s.token, &s.maintainer), before + 50 * UNIT);
    assert_eq!(s.balance(&s.token, &s.contract_id), 0);
    let bounty = s.client.get_bounty(&id);
    assert_eq!(bounty.status, Status::Resolved);
    assert_eq!(bounty.winner, Some(winner));
    assert_eq!(
        s.assert_invariant(&s.token),
        Totals {
            funded: 100 * UNIT,
            released_net: portion - fee,
            fees: fee,
            refunded: 50 * UNIT,
            held: 0,
        }
    );
}

#[test]
fn resolve_10000_bps() {
    let env = Env::default();
    let s = Setup::new(&env);
    let id = disputed(&s);
    let winner = Address::generate(&env);
    let before = s.balance(&s.token, &s.maintainer);

    s.client
        .resolve(&s.arbiter, &id, &Some(winner.clone()), &10_000);

    // Same as a release: 97 to the winner, 3 fee, nothing back to the maintainer.
    assert_eq!(s.balance(&s.token, &winner), 97 * UNIT);
    assert_eq!(s.balance(&s.token, &s.treasury), 3 * UNIT);
    assert_eq!(s.balance(&s.token, &s.maintainer), before);
    assert_eq!(s.balance(&s.token, &s.contract_id), 0);
    s.assert_invariant(&s.token);
}

#[test]
fn resolve_rounding_dust_goes_to_the_right_side() {
    let env = Env::default();
    let s = Setup::new(&env);
    let amount = 10_000_001;
    let id = s.fund(REPO, ISSUE, amount);
    s.client.open_dispute(&s.maintainer, &id);
    let winner = Address::generate(&env);
    let before = s.balance(&s.token, &s.maintainer);

    s.client
        .resolve(&s.arbiter, &id, &Some(winner.clone()), &3_333);

    // portion = floor(10_000_001 * 3333 / 10_000) = 3_333_000; the maintainer side gets
    // the remainder, so it receives the dust.
    let portion = 3_333_000;
    let fee = fees::fee(portion, FEE_BPS).unwrap();
    assert_eq!(fee, 99_990);
    assert_eq!(s.balance(&s.token, &winner), portion - fee);
    assert_eq!(s.balance(&s.token, &s.treasury), fee);
    assert_eq!(
        s.balance(&s.token, &s.maintainer),
        before + amount - portion
    );
    s.assert_invariant(&s.token);
}

#[test]
fn resolve_with_winner_maintainer_rejected() {
    let env = Env::default();
    let s = Setup::new(&env);
    let id = disputed(&s);

    for bps in [1, 5_000, 10_000] {
        let r = s
            .client
            .try_resolve(&s.arbiter, &id, &Some(s.maintainer.clone()), &bps);
        assert_eq!(r, Err(Ok(Error::SelfPayout)), "bps = {bps}");
    }
    assert_eq!(s.client.get_bounty(&id).status, Status::Disputed);
    assert_eq!(s.balance(&s.token, &s.contract_id), 100 * UNIT);
}

#[test]
fn resolve_zero_bps_ignores_the_winner_argument() {
    let env = Env::default();
    let s = Setup::new(&env);
    let id = disputed(&s);

    // Nothing is owed to a winner at 0 bps, so even the maintainer here is harmless.
    s.client
        .resolve(&s.arbiter, &id, &Some(s.maintainer.clone()), &0);
    let bounty = s.client.get_bounty(&id);
    assert_eq!(bounty.status, Status::Resolved);
    assert_eq!(bounty.winner, None);
}

#[test]
fn resolve_requires_a_winner_when_owed_something() {
    let env = Env::default();
    let s = Setup::new(&env);
    let id = disputed(&s);
    assert_eq!(
        s.client.try_resolve(&s.arbiter, &id, &None, &1),
        Err(Ok(Error::InvalidSplit))
    );
    assert_eq!(s.client.get_bounty(&id).status, Status::Disputed);
}

#[test]
fn resolve_rejects_more_than_10000_bps() {
    let env = Env::default();
    let s = Setup::new(&env);
    let id = disputed(&s);
    let winner = Address::generate(&env);
    for bps in [10_001, u32::MAX] {
        let r = s
            .client
            .try_resolve(&s.arbiter, &id, &Some(winner.clone()), &bps);
        assert_eq!(r, Err(Ok(Error::InvalidSplit)), "bps = {bps}");
    }
    assert_eq!(s.client.get_bounty(&id).status, Status::Disputed);
}

#[test]
fn resolve_by_non_arbiter_rejected() {
    let env = Env::default();
    let s = Setup::new(&env);
    let id = disputed(&s);
    let winner = Address::generate(&env);
    let rando = Address::generate(&env);

    // Not even the maintainer, the relayer or the admin may resolve.
    for caller in [&rando, &s.maintainer, &s.relayer, &s.admin, &winner] {
        let r = s
            .client
            .try_resolve(caller, &id, &Some(winner.clone()), &10_000);
        assert_eq!(r, Err(Ok(Error::Unauthorized)));
    }
    assert_eq!(s.client.get_bounty(&id).status, Status::Disputed);
    assert_eq!(s.balance(&s.token, &s.contract_id), 100 * UNIT);
}

#[test]
fn resolve_works_while_paused() {
    let env = Env::default();
    let s = Setup::new(&env);
    let id = disputed(&s);
    let winner = Address::generate(&env);

    s.client.pause(&s.admin);
    s.client
        .resolve(&s.arbiter, &id, &Some(winner.clone()), &10_000);
    assert_eq!(s.client.get_bounty(&id).status, Status::Resolved);
    assert_eq!(s.balance(&s.token, &winner), 97 * UNIT);
}

#[test]
fn resolve_requires_a_disputed_bounty() {
    let env = Env::default();
    let s = Setup::new(&env);
    let id = funded(&s);
    let winner = Address::generate(&env);

    // Open, not disputed.
    assert_eq!(
        s.client
            .try_resolve(&s.arbiter, &id, &Some(winner.clone()), &10_000),
        Err(Ok(Error::InvalidStatus))
    );
    // Already resolved.
    s.client.open_dispute(&s.maintainer, &id);
    s.client.resolve(&s.arbiter, &id, &None, &0);
    assert_eq!(
        s.client.try_resolve(&s.arbiter, &id, &None, &0),
        Err(Ok(Error::InvalidStatus))
    );
    // Unknown.
    assert_eq!(
        s.client.try_resolve(&s.arbiter, &99, &None, &0),
        Err(Ok(Error::BountyNotFound))
    );
}

#[test]
fn emits_disputed_and_resolved_events() {
    let env = Env::default();
    let s = Setup::new(&env);
    let id = funded(&s);
    let winner = Address::generate(&env);

    s.client.open_dispute(&s.arbiter, &id);
    assert_eq!(
        env.events().all().filter_by_contract(&s.contract_id),
        [Disputed {
            id,
            caller: s.arbiter.clone(),
        }
        .to_xdr(&env, &s.contract_id)]
    );

    s.client
        .resolve(&s.arbiter, &id, &Some(winner.clone()), &5_000);
    assert_eq!(
        env.events().all().filter_by_contract(&s.contract_id),
        [Resolved {
            id,
            winner: Some(winner.clone()),
            winner_net: 485_000_000,
            fee: 15_000_000,
            maintainer_amount: 500_000_000,
        }
        .to_xdr(&env, &s.contract_id)]
    );
}

#[test]
fn requires_caller_auth() {
    let env = Env::default();
    let s = Setup::new(&env);
    let id = funded(&s);

    s.client.open_dispute(&s.maintainer, &id);
    assert_eq!(
        env.auths(),
        [(
            s.maintainer.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    s.contract_id.clone(),
                    Symbol::new(&env, "open_dispute"),
                    (&s.maintainer, id).into_val(&env),
                )),
                sub_invocations: [].into(),
            }
        )]
    );

    s.client.resolve(&s.arbiter, &id, &None, &0);
    let auths = env.auths();
    assert_eq!(auths.len(), 1);
    assert_eq!(auths[0].0, s.arbiter);
}

#[test]
fn unsigned_dispute_and_resolve_are_rejected() {
    let env = Env::default();
    let s = Setup::new(&env);
    let id = funded(&s);
    let winner = Address::generate(&env);

    env.set_auths(&[]);
    let r = s.client.try_open_dispute(&s.maintainer, &id);
    assert!(matches!(r, Err(Err(_))), "{r:?}");
    assert_eq!(s.client.get_bounty(&id).status, Status::Open);

    // Disputed through the fixtures path, then try to resolve without a signature.
    env.mock_all_auths();
    s.client.open_dispute(&s.maintainer, &id);
    env.set_auths(&[]);
    let r = s
        .client
        .try_resolve(&s.arbiter, &id, &Some(winner.clone()), &10_000);
    assert!(matches!(r, Err(Err(_))), "{r:?}");
    assert_eq!(s.client.get_bounty(&id).status, Status::Disputed);
}
