#!/usr/bin/env bash
# draw.sh -- build what the Board tab draws a .kicad_pcb with: UltraRoute Studio's core (the
# board's lines into a board-draw scene and its mesh) and ultraroute's web module (view(): a
# .kicad_pcb into those lines, without routing), both WASM, at the ultraroute commit below.
#
#     app/web/scripts/draw.sh          # on this machine, book cores first:
#     ~/.claude/bin/reserve -c 8 -l "boardroom draw wasm" -- app/web/scripts/draw.sh
#
# Writes src/draw/core/ and src/draw/router/ (not committed). Needs wasm-pack and the wasm32
# target. The checkout is a sparse clone in .cache/, so it never touches yours.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."

# The same commit's studio core must match package.json's board-draw (ultraroute pins both).
REV=ce50b64155115ef42b1a0b3397138cb8ab1b53ac
REPO=https://github.com/sequoia-hope/ultraroute.git
src=.cache/ultraroute

if [[ ! -d $src/.git ]]; then
  git clone --quiet --filter=blob:none --no-checkout "$REPO" "$src"
  git -C "$src" sparse-checkout set --no-cone /core/Cargo.toml /core/Cargo.lock /core/rust-toolchain.toml \
    /core/crates/ /studio/Cargo.toml /studio/Cargo.lock /studio/core/ /studio/server/
fi
# A fresh clone has checked nothing out, even when its HEAD is already the commit.
if [[ $(git -C "$src" rev-parse HEAD 2>/dev/null) != "$REV" || ! -f $src/studio/Cargo.toml ]]; then
  git -C "$src" cat-file -e "$REV^{commit}" 2>/dev/null || git -C "$src" fetch --quiet origin "$REV"
  git -C "$src" checkout --quiet "$REV"
fi

out=$PWD/src/draw
mkdir -p "$out"
(cd "$src/studio" && wasm-pack build core --release --target web --out-dir "$out/core" --out-name studio_core --no-pack -- --features wasm)
(cd "$src/core" && wasm-pack build crates/web --release --target web --out-dir "$out/router" --out-name router --no-pack)
echo "Built: src/draw/core (studio core) and src/draw/router (view) at ultraroute ${REV:0:7}"
