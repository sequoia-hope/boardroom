#!/usr/bin/env bash
# wasm.sh -- build Boardroom's WASM module (crates/wasm) for the web app, into src/wasm/
# (not committed). Needs wasm-pack and the wasm32 target (rust-toolchain.toml names it).
#
#     web/scripts/wasm.sh       # on this machine, book cores first:
#     ~/.claude/bin/reserve -c 4 -l "boardroom wasm" -- web/scripts/wasm.sh
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
out=$PWD/src/wasm
wasm-pack build ../crates/wasm --release --target web --out-dir "$out" --out-name boardroom --no-pack
rm -f "$out/.gitignore"
echo "Built: web/src/wasm (boardroom.js, boardroom_bg.wasm)"
