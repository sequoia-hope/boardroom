#!/usr/bin/env bash
# build.sh -- build Boardroom: the web page, then the server (release).
#
#     app/build.sh            # on this machine, book cores first:
#     ~/.claude/bin/reserve -c 4 -l "boardroom build" -- app/build.sh
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

(cd web && { [[ -d node_modules ]] || npm ci --no-audit --no-fund; } && npm run build)
(cd server && cargo build --release)
echo "Built: app/server/target/release/boardroom (serves app/web/dist)"

# The desktop window needs GTK 4 and WebKitGTK 6.0's headers
# (libgtk-4-dev, libwebkitgtk-6.0-dev); without them, skip it.
if pkg-config --exists gtk4 webkitgtk-6.0; then
  (cd gtk && cargo build --release)
  echo "Built: app/gtk/target/release/boardroom-gtk (the desktop window)"
else
  echo "Skipped the desktop window: no GTK 4 / WebKitGTK 6.0 headers"
fi
