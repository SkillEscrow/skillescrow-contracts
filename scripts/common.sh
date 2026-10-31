# Shared helpers, sourced by the other scripts. Not meant to be run directly.

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WASM="$ROOT/target/wasm32v1-none/release/bounty_escrow.wasm"

die() {
  echo "error: $*" >&2
  exit 1
}

need() {
  command -v "$1" >/dev/null 2>&1 || die "'$1' is required but was not found on PATH"
}

# Public address of a Stellar CLI identity. Never prints the secret.
addr() {
  stellar keys address "$1"
}

# Reads a string field from a flat JSON file without needing jq.
json_get() {
  sed -n "s/.*\"$2\": *\"\([^\"]*\)\".*/\1/p" "$1" | head -n 1
}

# Invokes the contract. Args: identity, then the function and its arguments after `--`.
invoke() {
  local source="$1"
  shift
  stellar contract invoke \
    --id "$CONTRACT_ID" \
    --source-account "$source" \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$PASSPHRASE" \
    -- "$@"
}