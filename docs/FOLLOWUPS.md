# Follow-ups

Ideas and gaps that are out of scope for v1. Nothing here is built until it is promoted into the spec.

| Date | Area | Note |
|---|---|---|
| 2026-10-31 | deps | `cargo audit` ignores RUSTSEC-2026-0387 (crypto-bigint 0.5.5, timing issue). Test-host only, not in the wasm; blocked on upstream soroban-env-host / elliptic-curve. See `.cargo/audit.toml`. Remove the ignore when soroban-sdk upgrades. |
