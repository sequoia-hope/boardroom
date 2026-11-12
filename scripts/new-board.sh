#!/usr/bin/env bash
# new-board.sh -- create a fresh board project repo from template/.
#
#     scripts/new-board.sh <dir> [--name <title>] [--github <owner>/<repo> [--private]]
set -euo pipefail

usage() {
  echo "usage: scripts/new-board.sh <dir> [--name <title>] [--github <owner>/<repo> [--private]]"
}

dest=""
title=""
has_name=0
github_repo=""
private_flag=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    -h|--help)
      usage
      exit 0
      ;;
    --name)
      if [[ $# -lt 2 ]]; then
        echo "new-board.sh: --name requires an argument" >&2
        exit 2
      fi
      title="$2"
      has_name=1
      shift 2
      ;;
    --name=*)
      title="${1#--name=}"
      has_name=1
      shift
      ;;
    --github)
      if [[ $# -lt 2 ]]; then
        echo "new-board.sh: --github requires <owner>/<repo>" >&2
        exit 2
      fi
      github_repo="$2"
      shift 2
      ;;
    --github=*)
      github_repo="${1#--github=}"
      shift
      ;;
    --private)
      private_flag=1
      shift
      ;;
    --)
      shift
      while [[ $# -gt 0 ]]; do
        if [[ -z "$dest" ]]; then
          dest="$1"
        else
          echo "new-board.sh: unexpected argument: $1" >&2
          usage >&2
          exit 2
        fi
        shift
      done
      ;;
    -*)
      echo "new-board.sh: unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
    *)
      if [[ -z "$dest" ]]; then
        dest="$1"
      else
        echo "new-board.sh: unexpected argument: $1" >&2
        usage >&2
        exit 2
      fi
      shift
      ;;
  esac
done

if [[ -z "$dest" ]]; then
  usage >&2
  exit 2
fi

if [[ "$private_flag" -eq 1 && -z "$github_repo" ]]; then
  echo "new-board.sh: --private requires --github <owner>/<repo>" >&2
  exit 2
fi

if [[ -n "$github_repo" ]]; then
  if [[ ! "$github_repo" =~ ^[^/]+/[^/]+$ ]]; then
    echo "new-board.sh: --github expects <owner>/<repo>, got: $github_repo" >&2
    exit 2
  fi
  if ! command -v gh >/dev/null; then
    echo "new-board.sh: gh CLI not found; install GitHub CLI to use --github" >&2
    exit 1
  fi
fi

if [[ -e "$dest" && ! -d "$dest" ]]; then
  echo "new-board.sh: $dest exists and is not a directory" >&2
  exit 1
fi

if [[ -d "$dest" && -n "$(ls -A "$dest")" ]]; then
  echo "new-board.sh: refusing to create board in non-empty directory: $dest" >&2
  exit 1
fi

boardroom_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

pinned_sha() {
  local sub="$1" sha=""
  # Boardroom no longer vendors these (docs/one-system-spec.md): board repos stay on the last
  # pins of the old repositories until `boardroom new` replaces this script (step 7).
  case "$sub" in
    pcbview) sha=26ba32bb8edbc5761d84e3238a312a29cbcc1aca ;;
    ultraroute) sha=b38a039495387813147dca046ff48d2b13954a5f ;;
  esac
  if [[ -z "$sha" ]]; then
    echo "new-board.sh: cannot determine pinned commit for vendor/$sub" >&2
    exit 1
  fi
  printf '%s\n' "$sha"
}

pcbview_sha="$(pinned_sha pcbview)"
ultraroute_sha="$(pinned_sha ultraroute)"
boardroom_sha="$(git -C "$boardroom_root" rev-parse --short HEAD)"
skill_sha="$(git -C "$boardroom_root" log -1 --format=%h -- .claude/skills/pcb-design)"

mkdir -p "$dest"
dest="$(cd "$dest" && pwd -P)"

cp -a "$boardroom_root/template/." "$dest/"

mkdir -p "$dest/.claude/skills/pcb-design"
tar -C "$boardroom_root/.claude/skills/pcb-design" \
  --exclude='__pycache__' \
  --exclude='.pytest_cache' \
  --exclude='.mypy_cache' \
  --exclude='.ruff_cache' \
  --exclude='*.pyc' \
  --exclude='*.pyo' \
  --exclude='*.kicad_prl' \
  --exclude='fp-info-cache' \
  -cf - . | tar -C "$dest/.claude/skills/pcb-design" -xf -

mkdir -p "$dest/scripts"
cp -p \
  "$boardroom_root/scripts/doctor.sh" \
  "$boardroom_root/scripts/view.sh" \
  "$boardroom_root/scripts/route.sh" \
  "$dest/scripts/"

if [[ "$has_name" -eq 1 ]]; then
  python3 - "$dest/pcbview.toml" "$title" <<'PY'
import json
import re
import sys
import tomllib
from pathlib import Path

path = Path(sys.argv[1])
title = sys.argv[2]
text = path.read_text(encoding="utf-8")
updated, count = re.subn(
    r'^title\s*=\s*"[^"]*"',
    lambda _: f"title = {json.dumps(title)}",
    text,
    count=1,
    flags=re.M,
)
if count != 1:
    sys.exit(f"new-board.sh: expected one title field in {path}")
tomllib.loads(updated)
path.write_text(updated, encoding="utf-8")
PY
fi

git -C "$dest" init -b main

add_pinned_submodule() {
  local sub="$1" url="$2" sha="$3"
  git -C "$dest" submodule add "$url" "vendor/$sub"
  if ! git -C "$dest/vendor/$sub" cat-file -e "${sha}^{commit}" 2>/dev/null; then
    git -C "$dest/vendor/$sub" fetch origin "$sha" 2>/dev/null \
      || { echo "new-board.sh: cannot fetch $sub at $sha from $url" >&2; exit 1; }
  fi
  git -C "$dest/vendor/$sub" checkout --quiet "$sha"
}

add_pinned_submodule pcbview "https://github.com/sequoia-hope/pcbview" "$pcbview_sha"
add_pinned_submodule ultraroute "https://github.com/sequoia-hope/ultraroute" "$ultraroute_sha"

git -C "$dest" add -A

commit_cfg=()
if ! git -C "$dest" config user.name >/dev/null 2>&1; then
  fallback_name="$(git -C "$boardroom_root" log -1 --format=%an 2>/dev/null || true)"
  commit_cfg+=(-c "user.name=${fallback_name:-Boardroom}")
fi
if ! git -C "$dest" config user.email >/dev/null 2>&1; then
  fallback_email="$(git -C "$boardroom_root" log -1 --format=%ae 2>/dev/null || true)"
  commit_cfg+=(-c "user.email=${fallback_email:-boardroom@localhost}")
fi

git -C "$dest" "${commit_cfg[@]}" commit -m \
  "Initial board project (boardroom $boardroom_sha, pcb-design skill $skill_sha)"

if [[ -n "$github_repo" ]]; then
  visibility="--public"
  if [[ "$private_flag" -eq 1 ]]; then
    visibility="--private"
  fi
  gh repo create "$github_repo" "$visibility" --source "$dest" --push
fi

projects_toml="${XDG_CONFIG_HOME:-$HOME/.config}/boardroom/projects.toml"
if [[ -f "$projects_toml" ]]; then
  python3 - "$projects_toml" "$dest" <<'PY'
import json
import os
import sys
import tomllib
from pathlib import Path

projects_toml = Path(sys.argv[1])
dest = Path(sys.argv[2]).resolve()

text = projects_toml.read_text(encoding="utf-8")
data = tomllib.loads(text)
entries = data.get("projects", [])

home = Path(os.environ.get("HOME", "/"))
for entry in entries:
    raw = entry.get("path", "")
    if not raw:
        continue
    p = home / raw[2:] if raw.startswith("~/") else Path(raw)
    try:
        resolved = p.resolve()
    except OSError:
        resolved = p
    if resolved == dest or Path(raw) == dest:
        sys.exit(0)

prefix = "" if not text or text.endswith("\n\n") else ("\n" if text.endswith("\n") else "\n\n")
snippet = f"{prefix}[[projects]]\npath = {json.dumps(str(dest))}\npinned = false\n"
with projects_toml.open("a", encoding="utf-8") as fh:
    fh.write(snippet)
PY
fi

echo "Created board project at $dest"
