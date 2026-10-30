#!/usr/bin/env bash
# serve.sh -- run Boardroom's server.
#
#     app/serve.sh --port 8054      or      PORT=8054 app/serve.sh
#
# The port comes from --port, then $PORT, then `proj port`; with none the
# server fails rather than guessing. Build first with app/build.sh.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

bin=server/target/release/boardroom
if [[ ! -x "$bin" || ! -f web/dist/index.html ]]; then
  echo "serve.sh: Boardroom isn't built. Run app/build.sh first." >&2
  exit 1
fi
exec "$bin" serve "$@"
