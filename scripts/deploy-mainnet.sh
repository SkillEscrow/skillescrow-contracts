#!/usr/bin/env bash
# Deploys bounty_escrow to Stellar MAINNET. This moves real money: it is deliberately hard to
# run by accident and has NEVER been run as part of the build. It needs explicit human approval
# (BUILD_SPEC.md section 19.4).
#
#   scripts/deploy-mainnet.sh --i-have-human-approval
#
# There are no defaults for addresses or tokens. Set every variable below yourself.
#   MAINNET_DEPLOYER_IDENTITY   Stellar CLI identity that pays for and signs the deploy
#   MAINNET_ADMIN_ADDRESS       the admin account. It must be a multisig before launch.
#                               Admin calls (init, set_token) are signed by the deployer, so the
#                               deployer identity must be able to sign for this account; if it is
#                               a multisig, run the init and set_token calls by hand instead.
#   MAINNET_RELAYER_ADDRESS, MAINNET_ARBITER_ADDRESS, MAINNET_TREASURY_ADDRESS
#   MAINNET_USDC_ISSUER         verify against Circle's / Stellar's official documentation
# Optional: FEE_BPS (300), GRACE_SECS (259200), MIN_DURATION_SECS (86400), MAX_DURATION_SECS (7776000)
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

[ "${1:-}" = "--i-have-human-approval" ] || die "refusing to run without --i-have-human-approval"

need stellar
cd "$ROOT"

RPC_URL="${STELLAR_RPC_URL:?set STELLAR_RPC_URL to a mainnet Soroban RPC endpoint}"
PASSPHRASE="Public Global Stellar Network ; September 2015"

DEPLOYER_ID="${MAINNET_DEPLOYER_IDENTITY:?set MAINNET_DEPLOYER_IDENTITY}"
ADMIN="${MAINNET_ADMIN_ADDRESS:?set MAINNET_ADMIN_ADDRESS}"
RELAYER="${MAINNET_RELAYER_ADDRESS:?set MAINNET_RELAYER_ADDRESS}"
ARBITER="${MAINNET_ARBITER_ADDRESS:?set MAINNET_ARBITER_ADDRESS}"
TREASURY="${MAINNET_TREASURY_ADDRESS:?set MAINNET_TREASURY_ADDRESS}"
USDC_ISSUER="${MAINNET_USDC_ISSUER:?set MAINNET_USDC_ISSUER (verify it from official docs)}"

FEE_BPS="${FEE_BPS:-300}"
GRACE_SECS="${GRACE_SECS:-259200}"
MIN_DURATION_SECS="${MIN_DURATION_SECS:-86400}"
MAX_DURATION_SECS="${MAX_DURATION_SECS:-7776000}"
MIN_AMOUNT="${MIN_AMOUNT:-10000000}"
# Hard cap: 500 USDC per bounty. Raising it requires the human (BUILD_SPEC.md section 17).
MAX_AMOUNT="${MAX_AMOUNT:-5000000000}"
[ "$MAX_AMOUNT" -le 5000000000 ] || die "MAX_AMOUNT above the 500 USDC launch cap"

OUT="deployments/mainnet.json"
[ ! -e "$OUT" ] || die "$OUT already exists; refusing to deploy a second instance"

net=(--rpc-url "$RPC_URL" --network-passphrase "$PASSPHRASE")
USDC_SAC="$(stellar contract id asset --asset "USDC:$USDC_ISSUER" "${net[@]}")"

echo "MAINNET deployment. This uses real funds."
echo "  deployer : $DEPLOYER_ID ($(addr "$DEPLOYER_ID"))"
echo "  admin    : $ADMIN"
echo "  relayer  : $RELAYER"
echo "  arbiter  : $ARBITER"
echo "  treasury : $TREASURY"
echo "  USDC     : $USDC_ISSUER  ->  $USDC_SAC"
echo "  fee      : $FEE_BPS bps, grace ${GRACE_SECS}s, durations ${MIN_DURATION_SECS}s to ${MAX_DURATION_SECS}s"
echo "  USDC cap : $MIN_AMOUNT to $MAX_AMOUNT base units"
echo
read -r -p "Type DEPLOY TO MAINNET to continue: " confirm
[ "$confirm" = "DEPLOY TO MAINNET" ] || die "not confirmed"

"$ROOT/scripts/build.sh" >/dev/null
WASM_HASH="$(sha256sum "$WASM" | cut -d' ' -f1)"
echo "wasm sha256: $WASM_HASH"
read -r -p "Deploy this wasm? Type the first 8 characters of the hash: " confirm_hash
[ "$confirm_hash" = "${WASM_HASH:0:8}" ] || die "hash did not match"

CONTRACT_ID="$(stellar contract deploy --wasm "$WASM" --source-account "$DEPLOYER_ID" "${net[@]}" | tail -n 1)"
[ -n "$CONTRACT_ID" ] || die "deploy returned no contract id"
echo "contract: $CONTRACT_ID"

invoke "$DEPLOYER_ID" init \
  --admin "$ADMIN" --relayer "$RELAYER" --arbiter "$ARBITER" --treasury "$TREASURY" \
  --fee_bps "$FEE_BPS" --grace_secs "$GRACE_SECS" \
  --min_duration_secs "$MIN_DURATION_SECS" --max_duration_secs "$MAX_DURATION_SECS"
invoke "$DEPLOYER_ID" set_token --admin "$ADMIN" --token "$USDC_SAC" \
  --min_amount "$MIN_AMOUNT" --max_amount "$MAX_AMOUNT"

cat >"$OUT" <<EOF
{
  "network": "mainnet",
  "rpcUrl": "$RPC_URL",
  "networkPassphrase": "$PASSPHRASE",
  "contractId": "$CONTRACT_ID",
  "wasmSha256": "$WASM_HASH",
  "sourceCommit": "$(git rev-parse HEAD)",
  "deployedAt": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "admin": "$ADMIN",
  "relayer": "$RELAYER",
  "arbiter": "$ARBITER",
  "treasury": "$TREASURY",
  "tokens": {
    "USDC": {
      "issuer": "$USDC_ISSUER",
      "contractId": "$USDC_SAC",
      "minAmount": "$MIN_AMOUNT",
      "maxAmount": "$MAX_AMOUNT"
    }
  }
}
EOF
echo "wrote $OUT"