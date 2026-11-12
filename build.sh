#!/usr/bin/env bash
# build.sh -- build Boardroom: the web app, then the `boardroom` binary (release).
#
#     ./build.sh              # on this machine, book cores first:
#     ~/.claude/bin/reserve -c 4 -l "boardroom build" -- ./build.sh
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

(cd web && { [[ -d node_modules ]] || npm ci --no-audit --no-fund; } && npm run build)
cargo build --release
echo "Built: target/release/boardroom (serves web/dist)"

# The desktop window needs GTK 4 and WebKitGTK 6.0's headers
# (libgtk-4-dev, libwebkitgtk-6.0-dev); without them, skip it.
if pkg-config --exists gtk4 webkitgtk-6.0; then
  cargo build --release -p boardroom-gtk
  echo "Built: target/release/boardroom-gtk (the desktop window)"
else
  echo "Skipped the desktop window: no GTK 4 / WebKitGTK 6.0 headers"
fi
