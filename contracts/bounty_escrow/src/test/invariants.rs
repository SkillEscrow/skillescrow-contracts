//! Property test for the accounting invariant (spec 6.7).
//!
//! Random sequences of fund / release / refund / dispute / resolve / time jumps / fee
//! changes run over two tokens and two maintainers. Most operations are chosen blindly, so
//! many fail; that is the point. After every step, whether it succeeded or not:
//!
//! 1. `funded == released_net + fees + refunded + held` and `held == token.balance(contract)`
//! 2. no token is created or destroyed: the balances of every known account sum to the same
//!    total as at the start
//! 3. the one-active-bounty-per-issue lock agrees with bounty statuses

use proptest::prelude::*;
use soroban_sdk::{testutils::Address as _, Address, Env};
use std::vec::Vec;

use super::setup::*;
use crate::types::Status;

#[derive(Clone, Debug)]
enum Op {
    Fund {
        maintainer: usize,
        token: usize,
        repo: u64,
        issue: u64,
        units: i128,
        dust: i128,
        days: u64,
        relayer_allowed: bool,
    },
    Release {
        pick: usize,
        by_relayer: bool,
        winner: usize,
    },
    Refund {
        pick: usize,
    },
    Dispute {
        pick: usize,
        by_arbiter: bool,
    },
    Resolve {
        pick: usize,
        bps: u32,
        winner: Option<usize>,
    },
    Advance {
        secs: u64,
    },
    SetFee {
        bps: u32,
    },
}

fn op_strategy() -> impl Strategy<Value = Op> {
    prop_oneof![
        4 => (
            0..2usize,
            0..2usize,
            1..3u64,
            1..4u64,
            1..=500i128,
            0..UNIT,
            prop_oneof![9 => 1..=5u64, 1 => 0..=95u64],
            any::<bool>(),
        )
            .prop_map(|(maintainer, token, repo, issue, units, dust, days, relayer_allowed)| {
                Op::Fund { maintainer, token, repo, issue, units, dust, days, relayer_allowed }
            }),
        3 => (any::<usize>(), any::<bool>(), 0..5usize)
            .prop_map(|(pick, by_relayer, winner)| Op::Release { pick, by_relayer, winner }),
        2 => any::<usize>().prop_map(|pick| Op::Refund { pick }),
        2 => (any::<usize>(), any::<bool>())
            .prop_map(|(pick, by_arbiter)| Op::Dispute { pick, by_arbiter }),
        3 => (any::<usize>(), 0..=10_100u32, proptest::option::of(0..5usize))
            .prop_map(|(pick, bps, winner)| Op::Resolve { pick, bps, winner }),
        3 => (0..=8 * DAY).prop_map(|secs| Op::Advance { secs }),
        1 => (0..=1_100u32).prop_map(|bps| Op::SetFee { bps }),
    ]
}

struct World<'a> {
    s: Setup<'a>,
    tokens: [Address; 2],
    maintainers: [Address; 2],
    /// Candidate winners: three fresh addresses, then the two maintainers (to hit SelfPayout).
    winners: [Address; 5],
    /// Every account whose balance we track for conservation.
    known: Vec<Address>,
    baseline: [i128; 2],
    bounties: Vec<u64>,
}

impl<'a> World<'a> {
    fn new(env: &'a Env) -> Self {
        let s = Setup::new(env);
        let second_token = Setup::new_token(env);
        s.client
            .set_token(&s.admin, &second_token, &MIN_AMOUNT, &MAX_AMOUNT);
        let tokens = [s.token.clone(), second_token];
        let maintainers = [s.maintainer.clone(), Address::generate(env)];
        for m in &maintainers {
            for t in &tokens {
                s.mint(t, m, 10_000 * UNIT);
            }
        }
        let fresh = [
            Address::generate(env),
            Address::generate(env),
            Address::generate(env),
        ];
        let winners = [
            fresh[0].clone(),
            fresh[1].clone(),
            fresh[2].clone(),
            maintainers[0].clone(),
            maintainers[1].clone(),
        ];
        let mut known = std::vec![s.contract_id.clone(), s.treasury.clone()];
        known.extend(maintainers.iter().cloned());
        known.extend(fresh.iter().cloned());
        let mut w = World {
            s,
            tokens,
            maintainers,
            winners,
            known,
            baseline: [0, 0],
            bounties: Vec::new(),
        };
        w.baseline = [w.supply(0), w.supply(1)];
        w
    }

    fn supply(&self, token: usize) -> i128 {
        self.known
            .iter()
            .map(|a| self.s.balance(&self.tokens[token], a))
            .sum()
    }

    /// Picks a bounty, usually one in a status the operation can act on (`wanted`), so that
    /// releases, disputes and resolutions actually happen; one time in eight any bounty at
    /// all, so that operations on the wrong status are exercised too.
    fn pick(&self, pick: usize, wanted: &[Status]) -> Option<u64> {
        if self.bounties.is_empty() {
            return None;
        }
        let matching: Vec<u64> = self
            .bounties
            .iter()
            .copied()
            .filter(|id| wanted.contains(&self.s.client.get_bounty(id).status))
            .collect();
        if matching.is_empty() || pick.is_multiple_of(8) {
            Some(self.bounties[pick % self.bounties.len()])
        } else {
            Some(matching[pick % matching.len()])
        }
    }
    fn apply(&mut self, op: &Op) {
        let s = &self.s;
        match op {
            Op::Fund {
                maintainer,
                token,
                repo,
                issue,
                units,
                dust,
                days,
                relayer_allowed,
            } => {
                let r = s.client.try_fund_bounty(
                    &self.maintainers[*maintainer],
                    repo,
                    issue,
                    &self.tokens[*token],
                    &(units * UNIT + dust),
                    &(s.now() + days * DAY),
                    relayer_allowed,
                );
                if let Ok(Ok(id)) = r {
                    self.bounties.push(id);
                }
            }
            Op::Release {
                pick,
                by_relayer,
                winner,
            } => {
                if let Some(id) = self.pick(*pick, &[Status::Open]) {
                    let caller = if *by_relayer {
                        s.relayer.clone()
                    } else {
                        s.client.get_bounty(&id).maintainer
                    };
                    let _ = s.client.try_release(&caller, &id, &self.winners[*winner]);
                }
            }
            Op::Refund { pick } => {
                if let Some(id) = self.pick(*pick, &[Status::Open]) {
                    let m = s.client.get_bounty(&id).maintainer;
                    let _ = s.client.try_refund(&m, &id);
                }
            }
            Op::Dispute { pick, by_arbiter } => {
                if let Some(id) = self.pick(*pick, &[Status::Open]) {
                    let caller = if *by_arbiter {
                        s.arbiter.clone()
                    } else {
                        s.client.get_bounty(&id).maintainer
                    };
                    let _ = s.client.try_open_dispute(&caller, &id);
                }
            }
            Op::Resolve { pick, bps, winner } => {
                if let Some(id) = self.pick(*pick, &[Status::Disputed]) {
                    let w = winner.map(|i| self.winners[i].clone());
                    let _ = s.client.try_resolve(&s.arbiter, &id, &w, bps);
                }
            }
            Op::Advance { secs } => s.set_time(s.now() + secs),
            Op::SetFee { bps } => {
                let _ = s.client.try_set_fee(&s.admin, bps);
            }
        }
    }

    fn check(&self, after: &Op) {
        for (i, token) in self.tokens.iter().enumerate() {
            // 1. accounting invariant (also asserts held == contract balance)
            self.s.assert_invariant(token);
            // 2. conservation of the token across every known account
            assert_eq!(
                self.supply(i),
                self.baseline[i],
                "token {i} was created or destroyed after {after:?}"
            );
        }
        // 3. issue lock vs statuses
        let mut open_per_issue: Vec<(u64, u64)> = Vec::new();
        for id in &self.bounties {
            let b = self.s.client.get_bounty(id);
            let active = self.s.client.active_bounty(&b.repo_id, &b.issue);
            match b.status {
                Status::Open | Status::Disputed => {
                    assert_eq!(
                        active,
                        Some(*id),
                        "live bounty {id} not locked after {after:?}"
                    );
                    let key = (b.repo_id, b.issue);
                    assert!(
                        !open_per_issue.contains(&key),
                        "two live bounties on {key:?} after {after:?}"
                    );
                    open_per_issue.push(key);
                }
                Status::Released | Status::Refunded | Status::Resolved => {
                    assert_ne!(
                        active,
                        Some(*id),
                        "terminal bounty {id} still locks its issue"
                    );
                    assert!(b.amount > 0);
                }
            }
        }
    }
}

proptest! {
    // 16 cases keep CI quick. For a deeper run: PROPTEST_CASES=300 cargo test invariants
    #![proptest_config(ProptestConfig::with_cases(16))]

    #[test]
    fn funds_are_conserved_over_random_sequences(ops in proptest::collection::vec(op_strategy(), 1..40)) {
        let env = Env::default();
        let mut world = World::new(&env);
        for op in &ops {
            world.apply(op);
            world.check(op);
        }
    }
}
