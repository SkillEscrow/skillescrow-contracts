#!/usr/bin/env bash
# Deploys bounty_escrow to Stellar testnet, initializes it, allowlists USDC and native XLM,
# and records the result in deployments/testnet.json.
#
# Keys live in the Stellar CLI keystore (outside the repo) and are referred to by identity
# name. Create them with:  stellar keys generate se-testnet-admin --fund
#
# Everything is overridable through the environment; defaults are below.
#   --redeploy   deploy a new instance even if deployments/testnet.json already exists
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

need stellar
cd "$ROOT"

RPC_URL="${STELLAR_RPC_URL:-https://soroban-testnet.stellar.org}"
PASSPHRASE="${STELLAR_NETWORK_PASSPHRASE:-Test SDF Network ; September 2015}"

# Identities (names in the Stellar CLI keystore).
ADMIN_ID="${ADMIN_IDENTITY:-se-testnet-admin}"
RELAYER_ID="${RELAYER_IDENTITY:-se-testnet-relayer}"
ARBITER_ID="${ARBITER_IDENTITY:-se-testnet-arbiter}"
TREASURY_ID="${TREASURY_IDENTITY:-se-testnet-treasury}"

# Contract config. Testnet starts with SHORT durations so refunds can be smoke-tested in
# minutes. Production values are a 3 day grace and a 1 to 90 day window: restore them on
# testnet afterwards with set_grace (259200) and set_durations (86400 7776000).
FEE_BPS="${FEE_BPS:-300}"
GRACE_SECS="${GRACE_SECS:-60}"
MIN_DURATION_SECS="${MIN_DURATION_SECS:-60}"
MAX_DURATION_SECS="${MAX_DURATION_SECS:-7776000}"

# Token limits in base units (7 decimals): 1 to 500 units.
MIN_AMOUNT="${MIN_AMOUNT:-10000000}"
MAX_AMOUNT="${MAX_AMOUNT:-5000000000}"

# Circle's testnet USDC issuer (https://developers.circle.com/stablecoins/usdc-contract-addresses).
USDC_ISSUER="${TOKEN_USDC_ISSUER:-GBBD47IF6LWK7P7MDEVSCWR7DPUWV3NY3DTQEVFL4NAT4AQH3ZLLFLA5}"

OUT="deployments/testnet.json"
if [ -e "$OUT" ] && [ "${1:-}" != "--redeploy" ]; then
  die "$OUT already exists. Pass --redeploy to deploy a new instance."
fi

ADMIN="$(addr "$ADMIN_ID")"
RELAYER="$(addr "$RELAYER_ID")"
ARBITER="$(addr "$ARBITER_ID")"
TREASURY="$(addr "$TREASURY_ID")"

net=(--rpc-url "$RPC_URL" --network-passphrase "$PASSPHRASE")
USDC_SAC="$(stellar contract id asset --asset "USDC:$USDC_ISSUER" "${net[@]}")"
XLM_SAC="$(stellar contract id asset --asset native "${net[@]}")"

echo "== build"
"$ROOT/scripts/build.sh" >/dev/null
WASM_HASH="$(sha256sum "$WASM" | cut -d' ' -f1)"
echo "wasm sha256: $WASM_HASH"

echo "== deploy"
CONTRACT_ID="$(stellar contract deploy --wasm "$WASM" --source-account "$ADMIN_ID" "${net[@]}" | tail -n 1)"
[ -n "$CONTRACT_ID" ] || die "deploy returned no contract id"
echo "contract: $CONTRACT_ID"

echo "== init"
invoke "$ADMIN_ID" init \
  --admin "$ADMIN" --relayer "$RELAYER" --arbiter "$ARBITER" --treasury "$TREASURY" \
  --fee_bps "$FEE_BPS" --grace_secs "$GRACE_SECS" \
  --min_duration_secs "$MIN_DURATION_SECS" --max_duration_secs "$MAX_DURATION_SECS"

echo "== allowlist USDC ($USDC_SAC) and native XLM ($XLM_SAC)"
invoke "$ADMIN_ID" set_token --admin "$ADMIN" --token "$USDC_SAC" \
  --min_amount "$MIN_AMOUNT" --max_amount "$MAX_AMOUNT"
invoke "$ADMIN_ID" set_token --admin "$ADMIN" --token "$XLM_SAC" \
  --min_amount "$MIN_AMOUNT" --max_amount "$MAX_AMOUNT"

echo "== record deployment"
cat >"$OUT" <<EOF
{
  "network": "testnet",
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
  "config": {
    "feeBps": $FEE_BPS,
    "graceSecs": $GRACE_SECS,
    "minDurationSecs": $MIN_DURATION_SECS,
    "maxDurationSecs": $MAX_DURATION_SECS
  },
  "tokens": {
    "USDC": {
      "issuer": "$USDC_ISSUER",
      "contractId": "$USDC_SAC",
      "minAmount": "$MIN_AMOUNT",
      "maxAmount": "$MAX_AMOUNT"
    },
    "XLM": {
      "contractId": "$XLM_SAC",
      "minAmount": "$MIN_AMOUNT",
      "maxAmount": "$MAX_AMOUNT"
    }
  }
}
EOF
echo "wrote $OUT"