# SkillEscrow Bounties: skillescrow-contracts

> Paid GitHub issues with AI PR triage, settled in USDC on Stellar/Soroban.

The Soroban escrow contract (`bounty_escrow`) for SkillEscrow Bounties. A maintainer locks USDC on a GitHub issue. When the maintainer merges a contributor's PR, the contributor is paid from escrow. If nothing is merged by the deadline, the maintainer reclaims the funds.

The API, GitHub App, indexer, relayer, AI reviewer and web app live in [skillescrow-app](https://github.com/SkillEscrow/skillescrow-app).

**Status:** Phase 0 (setup). The contract crate is empty; the entry points arrive in Phase 1. The full plan is in the build spec.

## Prerequisites

| Tool | Version | Notes |
|---|---|---|
| Rust | 1.99.1 | Installed automatically from `rust-toolchain.toml` by [rustup](https://rustup.rs). Includes rustfmt, clippy and the `wasm32v1-none` target. |
| Stellar CLI | 28.1.0 | [Install instructions](https://developers.stellar.org/docs/tools/cli/install-cli). Required to build wasm: soroban-sdk 29 refuses a plain `cargo build` for the wasm target. |

## Layout

```
Cargo.toml                     workspace, release profile, pinned soroban-sdk
rust-toolchain.toml            pinned toolchain
contracts/bounty_escrow/       the contract crate
docs/FOLLOWUPS.md              ideas that are out of scope for v1
```

## Build and test

```sh
cargo build                    # native build
cargo test                     # unit tests
cargo fmt --all -- --check     # formatting
cargo clippy --all-targets -- -D warnings
stellar contract build         # optimized wasm; prints hash and size
```

The wasm lands in `target/wasm32v1-none/release/bounty_escrow.wasm`.

## CI

GitHub Actions runs format, clippy, tests, the wasm build (with size report) and `cargo audit` on every pull request and push to `main`. Third-party actions are pinned by commit SHA.

## Contributing

`main` is the default branch. Open pull requests against it, use [Conventional Commits](https://www.conventionalcommits.org), and make sure the checks above pass. The Sororail code that used to live here is preserved on the `legacy/sororail` branch.

## License

Apache-2.0. See [LICENSE](LICENSE).
