# Follow-ups

Ideas and gaps that are out of scope for v1. Nothing here is built until it is promoted into the spec.

| Date | Area | Note |
|---|---|---|
| 2026-10-31 | deps | `cargo audit` ignores RUSTSEC-2026-0387 (crypto-bigint 0.5.5, timing issue). Test-host only, not in the wasm; blocked on upstream soroban-env-host / elliptic-curve. See `.cargo/audit.toml`. Remove the ignore when soroban-sdk upgrades. |
| 2026-10-31 | contract | No test yet for a successful `upgrade` (needs an uploaded wasm, e.g. via `contractimport!` of the built artifact). Cover it with the testnet smoke run or a build-then-test CI step. |
| 2026-10-31 | contract | `release` / `resolve` accept any winner address except the maintainer, including the contract's own address (funds would be stuck) and the treasury. Consider rejecting the contract address. Not in the spec, so not built. |
| 2026-10-31 | contract | The arbiter can open a dispute on any Open bounty and resolve it alone (see trust model). Consider requiring the maintainer or a contributor to request a dispute, or a delay, if the arbiter becomes more than the SkillEscrow operators. |
| 2026-10-31 | contract | The config events share the topics `(bounty, config)` and differ only by a `field` topic, so generic decoders (the Stellar CLI showed a token change as `FeeSet`) pick the wrong shape. The indexer must dispatch on `field`. If that proves error-prone, give each config event its own topic before mainnet; the spec text (`config {field, ...}`) would need a small change. |
| 2026-10-31 | tooling | `stellar contract bindings typescript` is marked deprecated by the CLI in favour of the js-stellar-sdk CLI. It works today (`bindings.sh`); plan a move to the js-stellar-sdk generator. |
| 2026-10-31 | tooling | The public testnet RPC occasionally drops a request mid-transaction (`client error (SendRequest)`); the transaction can still land. `smoke-testnet.sh` retries reads but aborts on a failed send rather than risk a double submit. |
