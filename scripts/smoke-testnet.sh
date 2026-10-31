#!/usr/bin/env bash
# Testnet smoke run against the contract in deployments/testnet.json, using native XLM:
#   A. fund -> release (signed by the relayer)
#   B. fund -> refund   (waits for deadline + grace; the testnet config has short durations)
#   C. fund -> dispute -> resolve 70/30
# It asserts balances and that the expected rejections happen on chain. Every transaction hash
# is printed as a "TX <label> <hash>" line so it can be recorded.
#
# Needs the se-testnet-* identities from deploy-testnet.sh. Safe to re-run: issue numbers are
# derived from the clock, so runs never collide.
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

need stellar
cd "$ROOT"
FILE="deployments/testnet.json"
[ -f "$FILE" ] || die "$FILE not found; run scripts/deploy-testnet.sh first"

CONTRACT_ID="$(json_get "$FILE" contractId)"
RPC_URL="$(json_get "$FILE" rpcUrl)"
PASSPHRASE="$(json_get "$FILE" networkPassphrase)"
GRACE="$(sed -n 's/.*"graceSecs": *\([0-9]*\).*/\1/p' "$FILE")"
XLM="$(sed -n '/"XLM": {/,/}/p' "$FILE" | sed -n 's/.*"contractId": *"\([^"]*\)".*/\1/p')"
[ -n "$CONTRACT_ID" ] && [ -n "$XLM" ] || die "could not read contract ids from $FILE"

MAINT_ID=se-testnet-maintainer
WINNER_ID=se-testnet-contributor
RELAYER_ID=se-testnet-relayer
ARBITER_ID=se-testnet-arbiter
MAINT="$(addr "$MAINT_ID")"
WINNER="$(addr "$WINNER_ID")"
RELAYER="$(addr "$RELAYER_ID")"
ARBITER="$(addr "$ARBITER_ID")"
TREASURY="$(json_get "$FILE" treasury)"

REPO=1
RUN="$(date +%s)"
AMOUNT=100000000 # 10 XLM
FEE_BPS="$(sed -n 's/.*"feeBps": *\([0-9]*\).*/\1/p' "$FILE")"

# tx <label> <source-identity> <contract> <function> [args...]: sends a transaction, prints the
# return value on stdout and "TX <label> <hash>" on stderr. Aborts if it fails.
tx() {
  local label="$1" source="$2" id="$3"
  shift 3
  local err out hash
  err="$(mktemp)"
  out="$(stellar contract invoke --id "$id" --source-account "$source" \
    --rpc-url "$RPC_URL" --network-passphrase "$PASSPHRASE" -- "$@" 2>"$err")" || {
    cat "$err" >&2
    rm -f "$err"
    die "$label failed"
  }
  hash="$(grep -o 'tx/[0-9a-f]\{64\}' "$err" | head -n 1 | cut -d/ -f2 || true)"
  echo "TX  $label  ${hash:-none}" >&2
  rm -f "$err"
  printf '%s' "$out" | tr -d '"'
}

# expect_fail <label> <error-code> <source> <contract> <function> [args...]: the call must be
# rejected with Error(Contract, #<code>).
expect_fail() {
  local label="$1" code="$2" source="$3" id="$4"
  shift 4
  local err
  err="$(mktemp)"
  if stellar contract invoke --id "$id" --source-account "$source" \
    --rpc-url "$RPC_URL" --network-passphrase "$PASSPHRASE" -- "$@" >/dev/null 2>"$err"; then
    rm -f "$err"
    die "$label was accepted but should have been rejected (#$code)"
  fi
  grep -q "#$code" "$err" || {
    cat "$err" >&2
    rm -f "$err"
    die "$label was rejected, but not with #$code"
  }
  rm -f "$err"
  echo "OK  $label rejected with #$code" >&2
}

# read_call <invoke args...>: a read-only invoke that retries, because the public RPC
# occasionally returns nothing. Prints the result without JSON quotes.
read_call() {
  local i out
  for i in 1 2 3 4 5; do
    if out="$(stellar contract invoke "$@" 2>/dev/null)" && [ -n "$out" ]; then
      printf '%s' "$out" | tr -d '"'
      return 0
    fi
    sleep 3
  done
  die "read call kept failing: $*"
}

bal() {
  read_call --id "$XLM" --source-account "$MAINT_ID" \
    --rpc-url "$RPC_URL" --network-passphrase "$PASSPHRASE" -- balance --id "$1"
}

bounty() {
  read_call --id "$CONTRACT_ID" --source-account "$MAINT_ID" \
    --rpc-url "$RPC_URL" --network-passphrase "$PASSPHRASE" -- get_bounty --id "$1"
}

active() {
  read_call --id "$CONTRACT_ID" --source-account "$MAINT_ID" \
    --rpc-url "$RPC_URL" --network-passphrase "$PASSPHRASE" -- active_bounty --repo_id "$REPO" --issue "$1"
}
check() { # check <label> <expected> <actual>
  [ "$2" = "$3" ] || die "$1: expected $2, got $3"
  echo "OK  $1 = $3" >&2
}

fund() { # fund <issue> <deadline> <relayer_allowed>
  tx "fund issue $1" "$MAINT_ID" "$CONTRACT_ID" fund_bounty \
    --maintainer "$MAINT" --repo_id "$REPO" --issue "$1" --token "$XLM" \
    --amount "$AMOUNT" --deadline "$2" --relayer_allowed "$3"
}

fee=$((AMOUNT * FEE_BPS / 10000))
net=$((AMOUNT - fee))

echo "== A. fund -> release by relayer"
issue=$((RUN * 10 + 1))
w0=$(bal "$WINNER"); t0=$(bal "$TREASURY"); c0=$(bal "$CONTRACT_ID")
id=$(fund "$issue" $(($(date +%s) + 3600)) true)
check "A contract holds the bounty" $((c0 + AMOUNT)) "$(bal "$CONTRACT_ID")"
check "A issue is locked" "$id" "$(active "$issue")"
tx "A release (relayer)" "$RELAYER_ID" "$CONTRACT_ID" release \
  --caller "$RELAYER" --bounty_id "$id" --winner "$WINNER" >/dev/null
check "A winner received net" $((w0 + net)) "$(bal "$WINNER")"
check "A treasury received fee" $((t0 + fee)) "$(bal "$TREASURY")"
check "A contract balance restored" "$c0" "$(bal "$CONTRACT_ID")"
bounty "$id" | grep -q Released || die "A bounty is not Released"
check "A issue unlocked" "null" "$(active "$issue")"
expect_fail "A second release" 11 "$MAINT_ID" "$CONTRACT_ID" release \
  --caller "$MAINT" --bounty_id "$id" --winner "$WINNER"

echo "== B. fund -> refund"
issue=$((RUN * 10 + 2))
c0=$(bal "$CONTRACT_ID"); t0=$(bal "$TREASURY")
deadline=$(($(date +%s) + 120)) # computed now: it must be at least min_duration ahead of the ledger clock
id=$(fund "$issue" "$deadline" true)
expect_fail "B early refund" 12 "$MAINT_ID" "$CONTRACT_ID" refund --maintainer "$MAINT" --bounty_id "$id"
unlock=$((deadline + GRACE + 20))
echo "   waiting until deadline + grace has passed (local clock $(date +%s) -> $unlock)" >&2
while [ "$(date +%s)" -lt "$unlock" ]; do sleep 5; done
tx "B refund" "$MAINT_ID" "$CONTRACT_ID" refund --maintainer "$MAINT" --bounty_id "$id" >/dev/null
check "B contract balance restored" "$c0" "$(bal "$CONTRACT_ID")"
bounty "$id" | grep -q Refunded || die "B bounty is not Refunded"
check "B treasury took no fee" "$t0" "$(bal "$TREASURY")"

echo "== C. fund -> dispute -> resolve 70/30"
issue=$((RUN * 10 + 3))
w0=$(bal "$WINNER"); t0=$(bal "$TREASURY"); c0=$(bal "$CONTRACT_ID")
id=$(fund "$issue" $(($(date +%s) + 3600)) false)
expect_fail "C relayer release without opt-in" 17 "$RELAYER_ID" "$CONTRACT_ID" release \
  --caller "$RELAYER" --bounty_id "$id" --winner "$WINNER"
tx "C open_dispute (arbiter)" "$ARBITER_ID" "$CONTRACT_ID" open_dispute \
  --caller "$ARBITER" --bounty_id "$id" >/dev/null
expect_fail "C release while disputed" 11 "$MAINT_ID" "$CONTRACT_ID" release \
  --caller "$MAINT" --bounty_id "$id" --winner "$WINNER"
tx "C resolve 7000 bps" "$ARBITER_ID" "$CONTRACT_ID" resolve \
  --arbiter "$ARBITER" --bounty_id "$id" --winner "\"$WINNER\"" --winner_bps 7000 >/dev/null
portion=$((AMOUNT * 7000 / 10000))
cfee=$((portion * FEE_BPS / 10000))
check "C winner received 70% minus fee" $((w0 + portion - cfee)) "$(bal "$WINNER")"
check "C treasury received fee on the portion" $((t0 + cfee)) "$(bal "$TREASURY")"
check "C contract balance restored" "$c0" "$(bal "$CONTRACT_ID")"
bounty "$id" | grep -q Resolved || die "C bounty is not Resolved"
check "C issue unlocked" "null" "$(active "$issue")"

echo "SMOKE RUN PASSED" >&2