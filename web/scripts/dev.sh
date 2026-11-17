#!/usr/bin/env bash
# dev.sh -- run Vite's dev server on Boardroom's registered port.
#
#     npm run dev -- --port N      or      PORT=N npm run dev
#
# The port comes from --port, then $PORT, then `proj port`; with none the
# script fails rather than guessing or falling back to a free port.
set -euo pipefail

web_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
repo_dir="$(cd "$web_dir/.." && pwd)"

port="${PORT:-}"
vite_args=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --port)
      if [[ $# -lt 2 || -z "$2" ]]; then
        echo "web dev: option '--port' requires a port number" >&2
        exit 1
      fi
      port="$2"
      shift 2
      ;;
    --port=*)
      port="${1#--port=}"
      if [[ -z "$port" ]]; then
        echo "web dev: option '--port' requires a port number" >&2
        exit 1
      fi
      shift
      ;;
    *)
      vite_args+=("$1")
      shift
      ;;
  esac
done

if [[ -z "$port" ]] && command -v proj >/dev/null 2>&1; then
  port="$(cd "$repo_dir" && proj port 2>/dev/null || true)"
fi

if [[ -z "$port" ]]; then
  echo "web dev: no port. Pass --port N or set PORT (or register the project with 'proj add')." >&2
  exit 1
fi

if [[ ! "$port" =~ ^[0-9]+$ ]] || (( port < 1 || port > 65535 )); then
  echo "web dev: port '$port' is not a valid port number (1..65535)." >&2
  exit 1
fi

cd "$web_dir"
exec vite --port "$port" --strictPort "${vite_args[@]}"
