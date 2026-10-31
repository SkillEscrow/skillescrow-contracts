#!/usr/bin/env bash
# Optimized wasm build. Prints the wasm path, size and SHA-256.
# soroban-sdk 29 refuses a plain `cargo build` for wasm, so this goes through the Stellar CLI.
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

need stellar
cd "$ROOT"

stellar contract build

[ -f "$WASM" ] || die "expected wasm at $WASM"
echo
echo "wasm:   ${WASM#"$ROOT"/}"
echo "size:   $(wc -c <"$WASM" | tr -d ' ') bytes"
echo "sha256: $(sha256sum "$WASM" | cut -d' ' -f1)"