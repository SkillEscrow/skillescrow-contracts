# bounty_escrow contract

Reference for the `bounty_escrow` Soroban contract: entry points, auth, errors, events, the trust model, fee math and TTL numbers.

The contract is the source of truth for money. The AI reviewer never moves funds, and the database is a cache rebuilt from events.

## Lifecycle

```
            fund_bounty
   [none] ─────────────▶ Open
                          │  release   (maintainer, or relayer if relayer_allowed)
                          ├──────────────────────────────▶ Released   (terminal)
                          │  refund    (maintainer, now >= deadline + grace)
                          ├──────────────────────────────▶ Refunded   (terminal)
                          │  open_dispute (maintainer or arbiter)
                          └──────────────────────────────▶ Disputed
                                                             │ resolve (arbiter)
                                                             └──────▶ Resolved (terminal)
```

One bounty can be active (`Open` or `Disputed`) per `(repo_id, issue)`. A terminal bounty frees the issue for a new one.

## Entry points

"Auth" is the address whose signature is required. Every state-changing function authenticates first, then validates, then writes state, then moves tokens, then emits its event.

### Bounties

| Function | Auth | Rules |
|---|---|---|
| `fund_bounty(maintainer, repo_id, issue, token, amount, deadline, relayer_allowed) -> u64` | `maintainer` | Not paused. Token allowlisted. `min <= amount <= max`. `deadline` in `[now + min_duration, now + max_duration]`, both inclusive. No active bounty for the issue. Snapshots `fee_bps`. Ids start at 1. Authorizes the token `transfer` as a sub-invocation. |
| `release(caller, bounty_id, winner)` | `caller` | `Open`. `caller` is the maintainer, or the relayer if `relayer_allowed`. `winner != maintainer`. Pays `amount - fee` to the winner and `fee` to the treasury. Allowed after the deadline and while paused, until a refund happens. |
| `refund(maintainer, bounty_id)` | `maintainer` | `Open`. `now >= deadline + grace_secs` (inclusive; `grace_secs` is read from config at refund time). Returns the full amount, no fee. Works while paused. |
| `open_dispute(caller, bounty_id)` | `caller` | `Open`. `caller` is the maintainer or the arbiter. Blocks release and refund; the issue stays locked. |
| `resolve(arbiter, bounty_id, winner, winner_bps)` | `arbiter` | `Disputed`. `winner_bps <= 10_000`. If `winner_bps > 0`, `winner` is required and must not be the maintainer; at 0 it is ignored. Winner gets `portion - fee(portion)`, treasury gets that fee, maintainer gets `amount - portion`. Works while paused. |
| `get_bounty(id) -> Bounty` | none | |
| `active_bounty(repo_id, issue) -> Option<u64>` | none | |
| `get_config() -> Config` | none | |

### Admin

All require the configured admin's signature.

| Function | Notes |
|---|---|
| `init(admin, relayer, arbiter, treasury, fee_bps, grace_secs, min_duration_secs, max_duration_secs)` | Once only. `fee_bps <= 1000`, `min_duration <= max_duration`. |
| `set_token(admin, token, min_amount, max_amount)` | Allowlists a token or updates its limits. `0 < min <= max`. |
| `remove_token(admin, token)` | Blocks new funding only; existing bounties can still be released, refunded and resolved. |
| `set_fee(admin, fee_bps)` | `<= 1000`. Affects new bounties only (each snapshots the fee). |
| `set_relayer` / `set_arbiter` / `set_treasury` | Take effect immediately, including for existing bounties. |
| `set_grace(admin, grace_secs)` | Applies to existing bounties too (read at refund time). |
| `set_durations(admin, min, max)` | `min <= max`. Affects new bounties. |
| `set_admin(admin, new_admin)` | **Both** addresses must sign, so a typo cannot lock the contract. |
| `pause(admin)` / `unpause(admin)` | Pause blocks `fund_bounty` only. |
| `upgrade(admin, wasm_hash)` | Replaces the contract code. See the trust model. |

## Errors

Stable numbering, never renumber after deploy.

| # | Name | Raised by |
|---|---|---|
| 1 | AlreadyInitialized | `init` |
| 2 | NotInitialized | any call before `init` |
| 3 | Unauthorized | wrong caller for the function |
| 4 | Paused | `fund_bounty` |
| 5 | TokenNotAllowed | `fund_bounty`, `remove_token` |
| 6 | AmountTooSmall | `fund_bounty`, `set_token` (`min <= 0`) |
| 7 | AmountTooLarge | `fund_bounty`, `set_token` (`max < min`) |
| 8 | InvalidDeadline | `fund_bounty`, `init` / `set_durations` (`min > max`) |
| 9 | IssueAlreadyFunded | `fund_bounty` |
| 10 | BountyNotFound | any call with an unknown id |
| 11 | InvalidStatus | `release`, `refund`, `open_dispute`, `resolve` on the wrong status |
| 12 | DeadlineNotReached | `refund` before `deadline + grace` |
| 13 | SelfPayout | `release`, `resolve` with the maintainer as winner |
| 14 | InvalidSplit | `resolve` with `winner_bps > 10_000`, or no winner when one is owed |
| 15 | InvalidFee | `init`, `set_fee` above 1000 bps |
| 16 | Overflow | checked arithmetic failure |
| 17 | RelayerNotAllowed | `release` by the relayer on a bounty that did not opt in |

## Events

Topics are `("bounty", <name>)`, plus the bounty id where applicable. Data is a map with every relevant field, so the indexer never needs a follow-up read.

| Event | Topics | Data |
|---|---|---|
| `funded` | `bounty`, `funded`, `id` | `maintainer, repo_id, issue, token, amount, fee_bps, deadline, relayer_allowed` |
| `released` | `bounty`, `released`, `id` | `winner, net, fee, caller` |
| `refunded` | `bounty`, `refunded`, `id` | `amount` |
| `disputed` | `bounty`, `disputed`, `id` | `caller` |
| `resolved` | `bounty`, `resolved`, `id` | `winner` (optional), `winner_net, fee, maintainer_amount` |
| `config` | `bounty`, `config`, `field` | by field: `token` (`token, min_amount, max_amount`), `token_rm` (`token`), `fee_bps` (`old_bps, new_bps`), `grace` (`old_secs, new_secs`), `durations` (`min_secs, max_secs`), and `admin` / `relayer` / `arbiter` / `treasury` (`old, new`) |
| `paused` / `unpaused` | `bounty`, `paused` / `unpaused` | `admin` |
| `upgraded` | `bounty`, `upgraded` | `wasm_hash` |

### Indexing notes

All `config` events share the topic prefix `("bounty", "config")` and are told apart by the third topic, the `field` symbol (`token`, `token_rm`, `fee_bps`, `grace`, `durations`, `admin`, `relayer`, `arbiter`, `treasury`). Their data maps differ per field. A decoder that matches only on the first two topics will pick the wrong event shape: on testnet the Stellar CLI labelled a `token` change as `FeeSet` for exactly this reason. **The indexer must dispatch on the `field` topic**, not on the event name from the spec. All other events have distinct topics.
## Trust model

Who can do what, stated plainly. None of this is hidden; it is the design.

- **Maintainer.** Funds a bounty and can always get it back after `deadline + grace`, unless it is disputed. Can release to any address except their own. A merged PR is the maintainer's approval; there is no other.
- **Relayer.** Can call `release` only on bounties whose maintainer opted in (`relayer_allowed`) and only while `Open`. It chooses the winner address, so a leaked relayer key can send opted-in bounties to an attacker's address. The damage is bounded by the per-token `max_amount` and by each maintainer's choice to opt in. The relayer cannot refund, dispute or resolve.
- **Arbiter.** Can open a dispute on **any** `Open` bounty and then split it any way it likes (0 to 100% to any non-maintainer address, with the fee on that portion). The arbiter is therefore trusted with every bounty that is open at the time. In v1 this is the SkillEscrow operators.
- **Admin.** Can change fees (for new bounties), the relayer, arbiter and treasury, the allowlist, limits and durations, pause funding, and **upgrade the contract code**. An upgrade can change what the contract does with funds it already holds, so the admin is fully trusted. The admin key must be a multisig account before mainnet (BUILD_SPEC.md section 19.4). The admin cannot move funds directly, only through an upgrade or by acting as the other roles after changing them.
- **Pausing.** Blocks `fund_bounty` only. Release, refund, dispute and resolution always work, so funds can always leave.
- **Treasury.** Receives fees. Set by the admin.

What the contract guarantees regardless of roles: the accounting invariant below, one active bounty per issue, no fee on refunds, and no payout to the maintainer's own address through `release` or `resolve`.

## Accounting invariant

Per token the contract keeps `Totals { funded, released_net, fees, refunded, held }`. After every state change:

```
funded == released_net + fees + refunded + held
held   == token.balance(contract)
```

`refunded` also carries the maintainer's share of a resolved dispute. The test helper `Setup::assert_invariant` checks both lines after every money-moving test, and `src/test/invariants.rs` checks them (plus conservation of each token across all known accounts and the one-active-bounty-per-issue lock) after every step of randomized sequences of fund / release / refund / dispute / resolve / time jumps / fee changes over two tokens.

## Fee math

```
fee(amount, bps)  = floor(amount * bps / 10_000)       checked, Overflow on failure
net(amount, bps)  = amount - fee(amount, bps)           dust goes to the contributor
portion(amount, winner_bps) = floor(amount * winner_bps / 10_000)   dust goes to the maintainer
```

Amounts must be non-negative; the contract enforces `min_amount > 0` at funding. Boundary tests cover 0, 1, `i128::MAX / 10_000` and bps 0 / 1 / 1000 / 10000 (`src/fees.rs`). In a resolution the fee is charged on the winner's portion only.

## Storage and TTL

| Data | Storage | Key |
|---|---|---|
| Config, next id, token limits | instance | `Config`, `NextId`, `Token(addr)` |
| Bounties, issue lock, totals | persistent | `Bounty(id)`, `ActiveByIssue(repo, issue)`, `Totals(addr)` |

Network limits, read from testnet with `stellar network settings` on 2026-10-31 (the CLI warned the network is on protocol 29 while it supports 28; the `state_archival` values were present). **Re-check on mainnet before launch.**

| Setting | Value |
|---|---|
| Ledger close time | 5 s (17,280 ledgers per day) |
| `max_entry_ttl` | 3,110,400 ledgers (180 days) |
| `min_persistent_ttl` | 120,960 ledgers (7 days) |

Contract constants (`src/storage.rs`): on every read and write an entry is extended to `TTL_EXTEND_TO` = 120 days (2,073,600 ledgers) whenever its remaining TTL is below `TTL_THRESHOLD` = 30 days (518,400 ledgers). The longest a bounty must stay readable is the default maximum duration (90 days) plus the default grace period (72 hours) = 93 days, so there are at least 27 days of margin, and 120 days stays under the 180 day network maximum. These relations are checked at compile time.

## Deployments

Public record in `deployments/<network>.json`; keys are never in the repo.

### Testnet

| | |
|---|---|
| Contract | `CASHNDPK5BILKEQUJKKWRP3IUFIX5R6UY5CKNQRTYMB2GQMTZF3J4RKN` |
| Deployed | 2026-10-31, from commit `ac03803`, wasm SHA-256 `2652019099895eb263b952b69e7b82be877a66c5085459c5f26fc7a3b9f99c2f` (24,204 bytes) |
| Admin / relayer / arbiter / treasury | `se-testnet-admin` / `-relayer` / `-arbiter` / `-treasury`, public addresses in `deployments/testnet.json` |
| USDC | Circle's testnet issuer `GBBD47IF6LWK7P7MDEVSCWR7DPUWV3NY3DTQEVFL4NAT4AQH3ZLLFLA5`, Soroban contract `CBIELTK6YBZJU5UP2WWQEUCYKLPU6AUNZ2BQ4WWFEIE3USCIHMXQDAMA`, 1 to 500 units |
| XLM | native asset contract `CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC`, 1 to 500 units |
| Config at deploy | fee 300 bps, **grace 60 s, min duration 60 s**, max duration 90 days |
| Config now | fee 300 bps, grace 72 h (259200 s), duration window 1 to 90 days (86400 to 7776000 s) |

The USDC issuer comes from Circle's published address list. It was cross-checked on testnet: the issuer account exists (home domain `centre.io`), the derived contract exists, and it reports symbol `USDC` and 7 decimals. Circle's page lists no Soroban contract id; this one is derived with `stellar contract id asset`.

The short grace and minimum duration existed so refunds could be smoke-tested in minutes. After the smoke runs, testnet was moved to the production values with `set_grace` (259200) and `set_durations` (86400, 7776000), which also exercised those setters on chain.

Mainnet has not been deployed. `scripts/deploy-mainnet.sh` exists and is guarded; it needs explicit human approval and the launch steps in BUILD_SPEC.md section 19.4.
## Decisions beyond the spec text

Recorded for review; none changes a spec rule.

- `init` and `set_durations` reject `min_duration > max_duration` with `InvalidDeadline`.
- `set_token` rejects `min_amount <= 0` with `AmountTooSmall` and `max_amount < min_amount` with `AmountTooLarge`.
- `resolve` requires a winner when `winner_bps > 0` (`InvalidSplit` otherwise) and ignores the winner argument at 0 bps.
- `remove_token` on a token that is not allowlisted returns `TokenNotAllowed`.
- Bounty ids start at 1. Zero-amount transfers (a zero fee, or a maintainer share of 0) are skipped.
- Events use the `#[contractevent]` macro because `Env::events().publish` is deprecated in soroban-sdk 29. The upgrade call uses `update_current_contract(ContractExecutable::Wasm(..))` for the same reason.

## Known gaps

- A successful `upgrade` is not covered by a test yet (it needs an uploaded wasm); only its auth and the rejection of an unknown hash are.
- `release` and `resolve` do not reject the contract's own address, or the treasury, as the winner. See `docs/FOLLOWUPS.md`.