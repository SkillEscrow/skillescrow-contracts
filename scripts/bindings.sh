#!/usr/bin/env bash
# Generates the TypeScript client for a deployed contract into the app repo.
#
#   scripts/bindings.sh [testnet|mainnet]      (default: testnet)
#
# Reads the contract id from deployments/<network>.json. The app repo is expected next to
# this one (../skillescrow-app); override with APP_REPO. The output is generated code: do not
# edit it by hand, regenerate it with this script.
#
# The generator's package.json is adapted for the monorepo (scoped private name, exact versions,
# no own TypeScript, built to dist/ on install). STELLAR_SDK_VERSION is the exact
# @stellar/stellar-sdk version the client is pinned to.
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

need stellar
need node
STELLAR_SDK_VERSION="${STELLAR_SDK_VERSION:-17.4.0}"
network="${1:-testnet}"
case "$network" in
  testnet | mainnet) ;;
  *) die "network must be testnet or mainnet" ;;
esac

file="$ROOT/deployments/$network.json"
[ -f "$file" ] || die "$file not found; deploy first"
CONTRACT_ID="$(json_get "$file" contractId)"
RPC_URL="$(json_get "$file" rpcUrl)"
PASSPHRASE="$(json_get "$file" networkPassphrase)"
[ -n "$CONTRACT_ID" ] || die "no contractId in $file"

APP_REPO="${APP_REPO:-$ROOT/../skillescrow-app}"
[ -d "$APP_REPO" ] || die "app repo not found at $APP_REPO (set APP_REPO)"
OUT_DIR="$APP_REPO/packages/contract-client"

stellar contract bindings typescript \
  --contract-id "$CONTRACT_ID" \
  --rpc-url "$RPC_URL" \
  --network-passphrase "$PASSPHRASE" \
  --output-dir "$OUT_DIR" \
  --overwrite

# Adapt the generated package.json for the monorepo.
node - "$OUT_DIR/package.json" "$STELLAR_SDK_VERSION" <<'JS'
const fs = require('fs');
const [file, sdk] = process.argv.slice(2);
const pkg = JSON.parse(fs.readFileSync(file, 'utf8'));
pkg.name = '@skillescrow/contract-client';
pkg.private = true;
pkg.dependencies['@stellar/stellar-sdk'] = sdk;
for (const [name, range] of Object.entries(pkg.dependencies)) {
  pkg.dependencies[name] = range.replace(/^[\^~]/, '');
}
delete pkg.devDependencies; // the workspace root provides TypeScript
// Consumers must import built output (dist/ with declarations), not this generated source:
// TypeScript would otherwise check it under the app's strict compiler options, which generated
// code does not satisfy. postinstall builds it, so a plain pnpm install leaves it ready.
// TypeScript 6 requires an explicit rootDir to emit; the generated tsconfig has none.
pkg.exports = './dist/index.js';
pkg.types = './dist/index.d.ts';
delete pkg.typings;
pkg.scripts = {
  build: 'tsc --rootDir src',
  typecheck: 'tsc --noEmit',
  postinstall: 'tsc --rootDir src',
};
fs.writeFileSync(file, JSON.stringify(pkg, null, 2) + '\n');
JS
echo "bindings for $network contract $CONTRACT_ID written to $OUT_DIR"