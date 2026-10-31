# SkillEscrow Bounties: skillescrow-contracts

> Paid GitHub issues with AI PR triage, settled in USDC on Stellar/Soroban.

The Soroban escrow contract (`bounty_escrow`) for SkillEscrow Bounties. A maintainer locks USDC on a GitHub issue. When the maintainer merges a contributor's PR, the contributor is paid from escrow. If nothing is merged by the deadline, the maintainer reclaims the funds.

The API, GitHub App, indexer, relayer, AI reviewer and web app live in [skillescrow-app](https://github.com/SkillEscrow/skillescrow-app).

**Status:** Phase 1. The contract is feature-complete and deployed to Stellar testnet (see [deployments/testnet.json](deployments/testnet.json)); mainnet has not been deployed. The full plan is in the build spec.

## Prerequisites

| Tool | Version | Notes |
|---|---|---|
| Rust | 1.99.1 | Installed automatically from `rust-toolchain.toml` by [rustup](https://rustup.rs). Includes rustfmt, clippy and the `wasm32v1-none` target. |
| Stellar CLI | 28.1.0 | [Install instructions](https://developers.stellar.org/docs/tools/cli/install-cli). Required to build wasm: soroban-sdk 29 refuses a plain `cargo build` for the wasm target. |

## Layout

```
Cargo.toml                     workspace, release profile, pinned soroban-sdk
rust-toolchain.toml            pinned toolchain
contracts/bounty_escrow/       the contract crate and its tests
scripts/                       build, deploy, smoke test and bindings (see below)
deployments/                   public record of each deployment
docs/CONTRACT.md              entry points, errors, events, trust model, TTL numbers
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

## Scripts

Run them with bash (Git Bash on Windows). They need the Stellar CLI; `bindings.sh` also needs Node.

| Script | What it does |
|---|---|
| `scripts/build.sh` | Optimized wasm build; prints path, size and SHA-256. |
| `scripts/deploy-testnet.sh` | Deploys to testnet, runs `init`, allowlists Circle's testnet USDC and native XLM, writes `deployments/testnet.json`. Settings come from the environment, with defaults in the script. |
| `scripts/smoke-testnet.sh` | Runs fund to release, fund to refund and fund to dispute to resolve against the deployed testnet contract, asserting balances and the expected rejections. |
| `scripts/bindings.sh` | Generates the TypeScript client into `../skillescrow-app/packages/contract-client`. The output is generated code: never edit it by hand. |
| `scripts/deploy-mainnet.sh` | Mainnet deploy. Refuses without `--i-have-human-approval`, takes every address from env vars, enforces the 500 USDC cap and asks for two typed confirmations. Not run as part of the build; it needs explicit human approval. |

**Keys never live in this repo.** The scripts refer to Stellar CLI identities by name (default `se-testnet-admin`, `-relayer`, `-arbiter`, `-treasury`, `-maintainer`, `-contributor`). Create them with `stellar keys generate se-testnet-admin --fund` and so on; the CLI keeps the secrets in its own config directory.

Beyond the unit tests, `cargo test invariants` runs a randomized property test of the accounting invariant (16 cases by default; `PROPTEST_CASES=300 cargo test invariants` for a deeper run).
## CI

GitHub Actions runs format, clippy, tests, the wasm build (with size report) and `cargo audit` on every pull request and push to `main`. Third-party actions are pinned by commit SHA.

## Contributing

`main` is the default branch. Open pull requests against it, use [Conventional Commits](https://www.conventionalcommits.org), and make sure the checks above pass. The Sororail code that used to live here is preserved on the `legacy/sororail` branch.

## License

Apache-2.0. See [LICENSE](LICENSE).
