#!/usr/bin/env bash
# serve.sh -- run Boardroom's server.
#
#     ./serve.sh --port N      or      PORT=N ./serve.sh
#
# The port comes from --port, then $PORT, then `proj port`; with none the
# server fails rather than guessing. Build first with ./build.sh.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

bin=target/release/boardroom
if [[ ! -x "$bin" || ! -f web/dist/index.html ]]; then
  echo "serve.sh: Boardroom isn't built. Run ./build.sh first." >&2
  exit 1
fi
exec "$bin" serve "$@"
