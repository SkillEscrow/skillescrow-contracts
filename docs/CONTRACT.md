# bounty_escrow contract

Reference for the `bounty_escrow` Soroban contract. **Work in progress:** this documents what is implemented so far (`init`, `set_token`, `pause`, `unpause`, `fund_bounty` and the read functions). `release`, `refund`, the dispute functions and the remaining admin setters arrive in later sessions, and the trust model and fee-math sections will be completed then.

The contract is the source of truth for money. The AI reviewer never moves funds, and the database is a cache rebuilt from events.

## Entry points

| Function | Auth | Notes |
|---|---|---|
| `init(admin, relayer, arbiter, treasury, fee_bps, grace_secs, min_duration_secs, max_duration_secs)` | `admin` | Once only. `fee_bps <= 1000`. |
| `set_token(admin, token, min_amount, max_amount)` | `admin` | Allowlists a token or updates its limits. `0 < min <= max`. |
| `pause(admin)` / `unpause(admin)` | `admin` | Blocks `fund_bounty` only. |
| `fund_bounty(maintainer, repo_id, issue, token, amount, deadline, relayer_allowed) -> u64` | `maintainer` | See below. |
| `get_bounty(id) -> Bounty` | none | |
| `active_bounty(repo_id, issue) -> Option<u64>` | none | Set while a bounty is `Open` (or `Disputed`). |
| `get_config() -> Config` | none | |

### `fund_bounty`

Order: auth, validate, write state, transfer tokens, emit event.

Rejected with, in this order:

1. `NotInitialized`
2. `Paused`
3. `TokenNotAllowed`
4. `AmountTooSmall` / `AmountTooLarge` (against the token's limits)
5. `InvalidDeadline`: `deadline` must be in `[now + min_duration_secs, now + max_duration_secs]`, both ends inclusive
6. `IssueAlreadyFunded`: one active bounty per `(repo_id, issue)`

On success it stores the bounty (status `Open`, `fee_bps` snapshotted from config), sets the issue lock, updates the token totals, transfers `amount` from the maintainer to the contract and emits `funded`. Bounty ids start at 1.

The maintainer's authorization covers the `fund_bounty` call and the token `transfer` it makes as a sub-invocation, which is what Freighter will be asked to sign.

## Errors

Stable numbering, never renumber after deploy.

| # | Name | # | Name |
|---|---|---|---|
| 1 | AlreadyInitialized | 10 | BountyNotFound |
| 2 | NotInitialized | 11 | InvalidStatus |
| 3 | Unauthorized | 12 | DeadlineNotReached |
| 4 | Paused | 13 | SelfPayout |
| 5 | TokenNotAllowed | 14 | InvalidSplit |
| 6 | AmountTooSmall | 15 | InvalidFee |
| 7 | AmountTooLarge | 16 | Overflow |
| 8 | InvalidDeadline | 17 | RelayerNotAllowed |
| 9 | IssueAlreadyFunded | | |

## Events

Topics are `("bounty", <name>)`, plus `bounty_id` where applicable. Data is a map with every relevant field.

| Event | Topics | Data |
|---|---|---|
| `funded` | `bounty`, `funded`, `id` | `maintainer, repo_id, issue, token, amount, fee_bps, deadline, relayer_allowed` |
| `config` (token) | `bounty`, `config`, `field = "token"` | `token, min_amount, max_amount` |
| `paused` / `unpaused` | `bounty`, `paused` / `unpaused` | `admin` |

## Accounting invariant

Per token the contract keeps `Totals { funded, released_net, fees, refunded, held }`. After every state change:

```
funded == released_net + fees + refunded + held
held   == token.balance(contract)
```

The test helper `Setup::assert_invariant` checks both after every funding test.

## Fee math

`fee = floor(amount * bps / 10_000)`, with checked multiplication (`Overflow` on failure). `net = amount - fee`, so rounding dust goes to the contributor. `portion` (used by dispute resolution) rounds down, so dust goes to the maintainer. Amounts must be non-negative. See `src/fees.rs` and its boundary tests.

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

## Decisions beyond the spec text

Recorded for review; none changes a spec rule.

- `init` rejects `min_duration_secs > max_duration_secs` with `InvalidDeadline`.
- `set_token` rejects `min_amount <= 0` with `AmountTooSmall` and `max_amount < min_amount` with `AmountTooLarge`.
- The `Totals` type and storage are in place; only `fund_bounty` writes them so far.
- Events use the `#[contractevent]` macro because `Env::events().publish` is deprecated in soroban-sdk 29.