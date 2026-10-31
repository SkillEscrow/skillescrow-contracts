# Follow-ups

Ideas and gaps that are out of scope for v1. Nothing here is built until it is promoted into the spec.

| Date | Area | Note |
|---|---|---|
| 2026-10-31 | deps | `cargo audit` ignores RUSTSEC-2026-0387 (crypto-bigint 0.5.5, timing issue). Test-host only, not in the wasm; blocked on upstream soroban-env-host / elliptic-curve. See `.cargo/audit.toml`. Remove the ignore when soroban-sdk upgrades. |
| 2026-10-31 | contract | No test yet for a successful `upgrade` (needs an uploaded wasm, e.g. via `contractimport!` of the built artifact). Cover it with the testnet smoke run or a build-then-test CI step. |
| 2026-10-31 | contract | `release` / `resolve` accept any winner address except the maintainer, including the contract's own address (funds would be stuck) and the treasury. Consider rejecting the contract address. Not in the spec, so not built. |
| 2026-10-31 | contract | The arbiter can open a dispute on any Open bounty and resolve it alone (see trust model). Consider requiring the maintainer or a contributor to request a dispute, or a delay, if the arbiter becomes more than the SkillEscrow operators. |
