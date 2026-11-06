#!/usr/bin/env bash
# serve.sh -- serve the project page (index.html, viewer/, design/) over HTTP.
#
#     ./serve.sh --port 8000      or      PORT=8000 ./serve.sh
#
# The port comes from --port, then $PORT, then `proj port` if this machine
# runs the proj launcher. There is deliberately no default: a project that
# cannot find its port says so rather than quietly binding whatever looked free.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

port="${PORT:-}"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --port) port="$2"; shift 2 ;;
    --port=*) port="${1#--port=}"; shift ;;
    *) echo "usage: $0 [--port N]" >&2; exit 2 ;;
  esac
done

if [[ -z "$port" ]]; then
  PROJ="$(command -v proj || echo "$HOME/scripts/proj")"
  [[ -x "$PROJ" ]] && port="$("$PROJ" port 2>/dev/null || true)"
fi
if [[ -z "$port" ]]; then
  echo "serve.sh: no port. Pass --port N or set PORT." >&2
  exit 1
fi

echo "Project page: http://localhost:$port/"
exec python3 -m http.server "$port" --bind 0.0.0.0
