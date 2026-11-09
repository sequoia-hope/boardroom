#!/usr/bin/env bash
# import.sh -- Stage 1 monorepo history importer (docs/monorepo-spec.md).
#
# Usage:
#   scripts/monorepo/import.sh [--with-board-draw] [--source-root <dir>] <scratch-dir>
#
# Clones the local source checkouts with `git clone --no-local` (never modifying
# any source checkout or worktree), rewrites each imported project's `main`
# history into its monorepo subdirectory with
# `git filter-repo --preserve-commit-hashes --to-subdirectory-filter <dir>`,
# merges `main` (and any tags on `main`) into a fresh Boardroom clone on branch
# `monorepo/import`, verifies every Stage 1 acceptance check, and writes
# `scripts/monorepo/branches.txt` and `scripts/monorepo/import-report.md`.
#
# Re-runnable: cleans only its own subdirectories inside <scratch-dir>
# (`boardroom/`, `clones/`, `analysis/`, `mirrors/`), leaving any other files in
# <scratch-dir> untouched.

set -euo pipefail

usage() {
  cat <<'EOF'
usage: scripts/monorepo/import.sh [options] <scratch-dir>

Options:
  --with-board-draw        Also import and merge board-draw into board-draw/ (Stage 3)
  --source-root <dir>      Directory holding source checkouts (default: /home/sequoia/Software)
  --branches-out <path>    Output path for unmerged branches list (default: scripts/monorepo/branches.txt)
  --report-out <path>      Output path for import report (default: scripts/monorepo/import-report.md)
  -h, --help               Show this help text
EOF
}

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
worktree_root="$(cd "$script_dir/../.." && pwd -P)"

source_root="/home/sequoia/Software"
with_board_draw=0
scratch_dir=""
branches_out="$script_dir/branches.txt"
report_out="$script_dir/import-report.md"

while [[ $# -gt 0 ]]; do
  case "$1" in
    -h|--help)
      usage
      exit 0
      ;;
    --with-board-draw|--board-draw|--include-board-draw)
      with_board_draw=1
      shift
      ;;
    --source-root)
      if [[ $# -lt 2 ]]; then
        echo "import.sh: --source-root requires an argument" >&2
        exit 2
      fi
      source_root="$2"
      shift 2
      ;;
    --source-root=*)
      source_root="${1#--source-root=}"
      shift
      ;;
    --branches-out)
      if [[ $# -lt 2 ]]; then
        echo "import.sh: --branches-out requires an argument" >&2
        exit 2
      fi
      branches_out="$2"
      shift 2
      ;;
    --branches-out=*)
      branches_out="${1#--branches-out=}"
      shift
      ;;
    --report-out)
      if [[ $# -lt 2 ]]; then
        echo "import.sh: --report-out requires an argument" >&2
        exit 2
      fi
      report_out="$2"
      shift 2
      ;;
    --report-out=*)
      report_out="${1#--report-out=}"
      shift
      ;;
    --)
      shift
      while [[ $# -gt 0 ]]; do
        if [[ -z "$scratch_dir" ]]; then
          scratch_dir="$1"
        else
          echo "import.sh: unexpected argument: $1" >&2
          usage >&2
          exit 2
        fi
        shift
      done
      ;;
    -*)
      echo "import.sh: unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
    *)
      if [[ -z "$scratch_dir" ]]; then
        scratch_dir="$1"
      else
        echo "import.sh: unexpected argument: $1" >&2
        usage >&2
        exit 2
      fi
      shift
      ;;
  esac
done

if [[ -z "$scratch_dir" ]]; then
  echo "import.sh: missing required <scratch-dir> argument" >&2
  usage >&2
  exit 2
fi

if ! command -v git >/dev/null 2>&1; then
  echo "import.sh: git not found" >&2
  exit 1
fi
if ! git filter-repo --version >/dev/null 2>&1; then
  echo "import.sh: git-filter-repo not found" >&2
  exit 1
fi

if [[ ! -d "$source_root" ]]; then
  echo "import.sh: source root does not exist: $source_root" >&2
  exit 1
fi
source_root="$(cd "$source_root" && pwd -P)"

all_source_repos=(boardroom circuit-composer pcbview ultraroute board-draw)
for repo in "${all_source_repos[@]}"; do
  repo_path="$source_root/$repo"
  if [[ ! -d "$repo_path" ]]; then
    echo "import.sh: missing source checkout: $repo_path" >&2
    exit 1
  fi
  if ! GIT_OPTIONAL_LOCKS=0 git --no-optional-locks -C "$repo_path" rev-parse --verify refs/heads/main >/dev/null 2>&1; then
    echo "import.sh: source checkout $repo_path has no local 'main' branch" >&2
    exit 1
  fi
done

if [[ -e "$scratch_dir" && ! -d "$scratch_dir" ]]; then
  echo "import.sh: scratch path exists and is not a directory: $scratch_dir" >&2
  exit 1
fi
mkdir -p "$scratch_dir"
scratch_dir="$(cd "$scratch_dir" && pwd -P)"

home_real="$(cd "${HOME:-/}" && pwd -P)"
case "$scratch_dir" in
  /|/tmp|/var|/var/tmp|"$home_real"|"$source_root"|"$worktree_root")
    echo "import.sh: refusing unsafe scratch directory: $scratch_dir" >&2
    exit 1
    ;;
esac
for repo in "${all_source_repos[@]}"; do
  repo_real="$(cd "$source_root/$repo" && pwd -P)"
  if [[ "$scratch_dir" == "$repo_real" || "$scratch_dir" == "$repo_real/"* ]]; then
    echo "import.sh: refusing scratch directory inside source repo $repo_real: $scratch_dir" >&2
    exit 1
  fi
done

# Active projects to import into subdirectories of Boardroom.
import_repos=(circuit-composer pcbview ultraroute)
import_dirs=(composer pcbview ultraroute)
if [[ "$with_board_draw" -eq 1 ]]; then
  import_repos+=(board-draw)
  import_dirs+=(board-draw)
fi

echo "==> Preparing scratch workspace in $scratch_dir"
rm -rf \
  "$scratch_dir/boardroom" \
  "$scratch_dir/clones" \
  "$scratch_dir/analysis" \
  "$scratch_dir/mirrors" \
  "$scratch_dir/meta"
mkdir -p \
  "$scratch_dir/clones" \
  "$scratch_dir/analysis" \
  "$scratch_dir/mirrors" \
  "$scratch_dir/meta"

# Step 1a & Step 5: Audit source heads (ahead/behind origin/main) and write branches.txt.
echo "==> Auditing source checkouts (read-only) and writing $branches_out"
mkdir -p "$(dirname "$branches_out")"
python3 - "$source_root" "$branches_out" "$scratch_dir/meta/sources.json" "${all_source_repos[@]}" <<'PY'
import json
import os
import subprocess
import sys
from pathlib import Path

source_root = Path(sys.argv[1])
branches_out = Path(sys.argv[2])
sources_json = Path(sys.argv[3])
repos = sys.argv[4:]


def git_ro(repo_path: Path, *args: str) -> str:
    env = os.environ.copy()
    env["GIT_OPTIONAL_LOCKS"] = "0"
    res = subprocess.run(
        ["git", "--no-optional-locks", "-C", str(repo_path), *args],
        env=env,
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    return res.stdout


def git_ro_ok(repo_path: Path, *args: str) -> bool:
    env = os.environ.copy()
    env["GIT_OPTIONAL_LOCKS"] = "0"
    res = subprocess.run(
        ["git", "--no-optional-locks", "-C", str(repo_path), *args],
        env=env,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    return res.returncode == 0


summary = {}
lines = [
    "# Unmerged branches across source repositories",
    "# Every local and remote branch NOT merged into local `main`.",
    "# Generated by scripts/monorepo/import.sh (read-only inspection; none of these are imported).",
    "",
]

for repo in repos:
    rp = source_root / repo
    main_sha = git_ro(rp, "rev-parse", "refs/heads/main").strip()
    main_commits = int(git_ro(rp, "rev-list", "--count", "refs/heads/main").strip())
    tags = [t for t in git_ro(rp, "tag", "-l").splitlines() if t.strip()]
    main_tags = [
        t for t in git_ro(rp, "tag", "--merged", "refs/heads/main").splitlines() if t.strip()
    ]

    has_origin_main = git_ro_ok(rp, "rev-parse", "--verify", "refs/remotes/origin/main")
    origin_main_sha = (
        git_ro(rp, "rev-parse", "refs/remotes/origin/main").strip()
        if has_origin_main
        else None
    )
    ahead = 0
    behind = 0
    ahead_commits = []
    if origin_main_sha:
        lr = git_ro(
            rp, "rev-list", "--left-right", "--count", "refs/remotes/origin/main...refs/heads/main"
        ).strip()
        behind_s, ahead_s = lr.split()
        behind = int(behind_s)
        ahead = int(ahead_s)
        if ahead > 0:
            raw_ahead = git_ro(
                rp,
                "log",
                "--oneline",
                "refs/remotes/origin/main..refs/heads/main",
            ).strip()
            ahead_commits = raw_ahead.splitlines() if raw_ahead else []
            print(
                f"NOTE: {repo}: local main ({main_sha[:7]}) is {ahead} commit(s) AHEAD of origin/main ({origin_main_sha[:7]}):"
            )
            for c in ahead_commits:
                print(f"      {c}")
        if behind > 0:
            print(
                f"WARNING: {repo}: local main ({main_sha[:7]}) is {behind} commit(s) BEHIND origin/main ({origin_main_sha[:7]})"
            )

    # Map branch full ref -> list of worktree paths where checked out.
    wt_raw = git_ro(rp, "worktree", "list", "--porcelain")
    branch_worktrees: dict[str, list[str]] = {}
    detached_worktrees: list[dict[str, str]] = []
    cur_wt = None
    cur_head = None
    cur_branch = None
    cur_detached = False

    def flush_wt():
        if cur_wt is None:
            return
        if cur_branch:
            branch_worktrees.setdefault(cur_branch, []).append(cur_wt)
        elif cur_detached and cur_head:
            if not git_ro_ok(rp, "merge-base", "--is-ancestor", cur_head, "refs/heads/main"):
                date_s = git_ro(rp, "log", "-1", "--format=%ci", cur_head).strip()
                detached_worktrees.append(
                    {"worktree": cur_wt, "sha": cur_head[:7], "date": date_s}
                )

    for wt_line in wt_raw.splitlines():
        if not wt_line.strip():
            flush_wt()
            cur_wt = cur_head = cur_branch = None
            cur_detached = False
            continue
        if wt_line.startswith("worktree "):
            cur_wt = wt_line[len("worktree ") :]
        elif wt_line.startswith("HEAD "):
            cur_head = wt_line[len("HEAD ") :]
        elif wt_line.startswith("branch "):
            cur_branch = wt_line[len("branch ") :]
        elif wt_line == "detached":
            cur_detached = True
    flush_wt()

    # Find all local and remote branches not merged into refs/heads/main.
    refs_raw = git_ro(
        rp,
        "for-each-ref",
        "--no-merged=refs/heads/main",
        "--format=%(refname)\t%(symref)\t%(objectname:short)\t%(committerdate:iso8601)",
        "refs/heads",
        "refs/remotes",
    )
    unmerged = []
    for line in refs_raw.splitlines():
        if not line.strip():
            continue
        refname, symref, sha_short, cdate = line.split("\t")
        if symref:
            # Skip symbolic refs such as refs/remotes/origin/HEAD.
            continue
        if refname.startswith("refs/heads/"):
            kind = "local"
            short_name = refname[len("refs/heads/") :]
        elif refname.startswith("refs/remotes/"):
            kind = "remote"
            short_name = refname[len("refs/remotes/") :]
        else:
            kind = "other"
            short_name = refname
        wts = branch_worktrees.get(refname, [])
        unmerged.append(
            {
                "ref": refname,
                "kind": kind,
                "name": short_name,
                "sha": sha_short,
                "date": cdate,
                "worktrees": wts,
            }
        )

    lines.append(
        f"== {repo} ({rp}) | main={main_sha[:7]}"
        + (
            f" (ahead of origin/main {origin_main_sha[:7]} by {ahead})"
            if ahead > 0 and origin_main_sha
            else ""
        )
        + " =="
    )
    if not unmerged and not detached_worktrees:
        lines.append("  (none)")
    else:
        for item in unmerged:
            wt_note = (
                f"  worktree={','.join(item['worktrees'])}"
                if item["worktrees"]
                else "  worktree=-"
            )
            label = f"{item['kind']}:{item['name']}"
            lines.append(
                f"  {label:<42} {item['sha']}  {item['date']}{wt_note}"
            )
        for dwt in detached_worktrees:
            lines.append(
                f"  {'detached:(HEAD)':<42} {dwt['sha']}  {dwt['date']}  worktree={dwt['worktree']}"
            )
    lines.append("")

    summary[repo] = {
        "path": str(rp),
        "main_sha": main_sha,
        "origin_main_sha": origin_main_sha,
        "main_commits": main_commits,
        "ahead": ahead,
        "behind": behind,
        "ahead_commits": ahead_commits,
        "all_tag_count": len(tags),
        "main_tag_count": len(main_tags),
        "main_tags": main_tags,
        "unmerged": unmerged,
        "detached_unmerged": detached_worktrees,
    }

branches_out.write_text("\n".join(lines).rstrip() + "\n", encoding="utf-8")
sources_json.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
PY

# Step 1b: Fresh `git clone --no-local` of Boardroom (main only) into $scratch_dir/boardroom.
dest_repo="$scratch_dir/boardroom"
echo "==> Cloning boardroom (main only, --no-local) -> $dest_repo"
git clone --quiet --no-local --single-branch --branch main \
  "$source_root/boardroom" "$dest_repo"

boardroom_head="$(git -C "$dest_repo" rev-parse refs/heads/main)"

# Run `git filter-repo --analyze` on Boardroom's clean main clone before merging.
echo "==> Analyzing tracked blob sizes in boardroom"
git -C "$dest_repo" filter-repo --analyze --report-dir "$scratch_dir/analysis/boardroom"

# Prepare branch `monorepo/import` as the sole branch in $dest_repo and remove `origin`
# so no remote-tracking branches remain and accidental pushes to the local checkout are impossible.
git -C "$dest_repo" branch -m main monorepo/import
git -C "$dest_repo" remote remove origin

commit_cfg=()
if ! git -C "$dest_repo" config user.name >/dev/null 2>&1; then
  fallback_name="$(git -C "$dest_repo" log -1 --format=%an 2>/dev/null || true)"
  commit_cfg+=(-c "user.name=${fallback_name:-Boardroom}")
fi
if ! git -C "$dest_repo" config user.email >/dev/null 2>&1; then
  fallback_email="$(git -C "$dest_repo" log -1 --format=%ae 2>/dev/null || true)"
  commit_cfg+=(-c "user.email=${fallback_email:-boardroom@localhost}")
fi

# Step 2 & Step 3: Clone, analyze, filter-repo, and merge each imported project.
imported_records_tsv="$scratch_dir/meta/imported.tsv"
: > "$imported_records_tsv"

for idx in "${!import_repos[@]}"; do
  repo="${import_repos[$idx]}"
  dir="${import_dirs[$idx]}"
  src_path="$source_root/$repo"
  clone_dir="$scratch_dir/clones/$repo"

  # Guard: never run filter-repo outside $scratch_dir/clones/.
  if [[ "$clone_dir" != "$scratch_dir/clones/"* ]]; then
    echo "import.sh: internal error: unsafe clone_dir $clone_dir" >&2
    exit 1
  fi

  echo "==> Cloning $repo (main only, --no-local) -> $clone_dir"
  git clone --quiet --no-local --single-branch --branch main "$src_path" "$clone_dir"

  echo "==> Analyzing tracked blob sizes in $repo"
  git -C "$clone_dir" filter-repo --analyze --report-dir "$scratch_dir/analysis/$repo"

  echo "==> Rewriting $repo into subdirectory $dir/ (--preserve-commit-hashes)"
  git -C "$clone_dir" filter-repo --force --quiet \
    --preserve-commit-hashes \
    --to-subdirectory-filter "$dir" \
    --tag-rename ":$dir-"

  filtered_sha="$(git -C "$clone_dir" rev-parse refs/heads/main)"
  filtered_tag_count="$(git -C "$clone_dir" tag -l "$dir-*" | wc -l | tr -d ' ')"

  echo "==> Merging $repo ($dir/main -> $filtered_sha) into monorepo/import"
  git -C "$dest_repo" fetch --quiet --tags "$clone_dir" "main:refs/remotes/$dir/main"
  git -C "$dest_repo" "${commit_cfg[@]}" merge --quiet \
    --allow-unrelated-histories --no-ff "$dir/main" \
    -m "Import $repo history into $dir/"
  git -C "$dest_repo" update-ref -d "refs/remotes/$dir/main"

  merge_sha="$(git -C "$dest_repo" rev-parse HEAD)"
  printf '%s\t%s\t%s\t%s\t%s\n' \
    "$repo" "$dir" "$filtered_sha" "$merge_sha" "$filtered_tag_count" \
    >> "$imported_records_tsv"
done

monorepo_head="$(git -C "$dest_repo" rev-parse monorepo/import)"
echo "==> Merged monorepo ready at $dest_repo on branch monorepo/import ($monorepo_head)"

# Acceptance Check 1: Mirror extraction reproduces exact original `main` SHA for every imported dir.
echo "==> Running Acceptance Check 1: mirror extraction SHA equality"
mirror_results_tsv="$scratch_dir/meta/mirrors.tsv"
: > "$mirror_results_tsv"

for idx in "${!import_repos[@]}"; do
  repo="${import_repos[$idx]}"
  dir="${import_dirs[$idx]}"
  extract_dir="$scratch_dir/mirrors/$dir"

  if [[ "$extract_dir" != "$scratch_dir/mirrors/"* ]]; then
    echo "import.sh: internal error: unsafe extract_dir $extract_dir" >&2
    exit 1
  fi

  git clone --quiet --no-local "$dest_repo" "$extract_dir"
  git -C "$extract_dir" filter-repo --force --quiet \
    --preserve-commit-hashes \
    --subdirectory-filter "$dir"

  extracted_sha="$(git -C "$extract_dir" rev-parse HEAD)"
  expected_sha="$(GIT_OPTIONAL_LOCKS=0 git --no-optional-locks -C "$source_root/$repo" rev-parse refs/heads/main)"

  status="PASS"
  if [[ "$extracted_sha" != "$expected_sha" ]]; then
    status="FAIL"
  fi
  printf '%s\t%s\t%s\t%s\t%s\n' \
    "$repo" "$dir" "$expected_sha" "$extracted_sha" "$status" \
    >> "$mirror_results_tsv"

  if [[ "$status" != "PASS" ]]; then
    echo "FAIL: mirror extraction for $dir ($repo): expected $expected_sha, got $extracted_sha" >&2
    exit 1
  fi
  echo "    PASS: $dir -> $extracted_sha (matches $repo main)"
done

# Acceptance Check 2–5 + Step 6 report generation.
echo "==> Running Acceptance Checks 2–5 and writing $report_out"
mkdir -p "$(dirname "$report_out")"
python3 - \
  "$source_root" \
  "$scratch_dir" \
  "$dest_repo" \
  "$boardroom_head" \
  "$monorepo_head" \
  "$with_board_draw" \
  "$branches_out" \
  "$report_out" <<'PY'
import json
import os
import re
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

source_root = Path(sys.argv[1])
scratch_dir = Path(sys.argv[2])
dest_repo = Path(sys.argv[3])
boardroom_head = sys.argv[4]
monorepo_head = sys.argv[5]
with_board_draw = sys.argv[6] == "1"
branches_out = Path(sys.argv[7])
report_out = Path(sys.argv[8])

sources_info = json.loads((scratch_dir / "meta" / "sources.json").read_text(encoding="utf-8"))


def run_git(repo: Path, *args: str, ro: bool = False) -> str:
    env = os.environ.copy()
    cmd = ["git"]
    if ro:
        env["GIT_OPTIONAL_LOCKS"] = "0"
        cmd.append("--no-optional-locks")
    cmd.extend(["-C", str(repo), *args])
    res = subprocess.run(cmd, env=env, check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    return res.stdout


imported_rows = []
for line in (scratch_dir / "meta" / "imported.tsv").read_text(encoding="utf-8").splitlines():
    if not line.strip():
        continue
    repo, dname, filtered_sha, merge_sha, tag_cnt = line.split("\t")
    imported_rows.append(
        {
            "repo": repo,
            "dir": dname,
            "filtered_sha": filtered_sha,
            "merge_sha": merge_sha,
            "tag_count": int(tag_cnt),
        }
    )

mirror_rows = []
for line in (scratch_dir / "meta" / "mirrors.tsv").read_text(encoding="utf-8").splitlines():
    if not line.strip():
        continue
    repo, dname, expected_sha, extracted_sha, status = line.split("\t")
    mirror_rows.append(
        {
            "repo": repo,
            "dir": dname,
            "expected_sha": expected_sha,
            "extracted_sha": extracted_sha,
            "status": status,
        }
    )

# Acceptance Check 2: `git branch -a` shows only `monorepo/import` and no imported branches.
branch_a_out = run_git(dest_repo, "branch", "-a").rstrip("\n")
branch_lines = [b.strip() for b in branch_a_out.splitlines() if b.strip()]
check2_pass = branch_lines == ["* monorepo/import"]
if not check2_pass:
    sys.stderr.write(f"FAIL: Check 2 (git branch -a): got {branch_lines!r}\n")
    sys.exit(1)
print("    PASS: Check 2 (`git branch -a` shows only `* monorepo/import`)")

# Acceptance Check 3: `git log --follow` on three sample files per project reaches their first commit.
sample_specs = [
    ("boardroom", "", ["README.md", ".claude/skills/pcb-design/SKILL.md", "app/server/src/main.rs"]),
    ("circuit-composer", "composer", ["README.md", "Cargo.toml", "crates/ccomp/src/main.rs"]),
    ("pcbview", "pcbview", ["README.md", "pcbview/site.py", "web/app.js"]),
    ("ultraroute", "ultraroute", ["README.md", "core/crates/ultraroute/src/main.rs", "tools/pcbench.py"]),
]
if with_board_draw:
    sample_specs.append(("board-draw", "board-draw", ["src/lib.rs", "js/renderer.js", "Cargo.toml"]))

follow_results = []
for repo, prefix, files in sample_specs:
    commit_map: dict[str, str] = {}
    if prefix:
        cmap_path = scratch_dir / "clones" / repo / ".git" / "filter-repo" / "commit-map"
        for line in cmap_path.read_text(encoding="utf-8").splitlines():
            parts = line.strip().split()
            if len(parts) == 2 and parts[0] != "old":
                commit_map[parts[0]] = parts[1]

    for rel in files:
        src_log = run_git(
            source_root / repo,
            "log",
            "--follow",
            "--format=%H\t%ad\t%s",
            "--date=short",
            "refs/heads/main",
            "--",
            rel,
            ro=True,
        ).splitlines()
        if not src_log:
            sys.stderr.write(f"FAIL: Check 3: no history in source {repo} for {rel}\n")
            sys.exit(1)

        mono_path = f"{prefix}/{rel}" if prefix else rel
        mono_log = run_git(
            dest_repo,
            "log",
            "--follow",
            "--format=%H\t%ad\t%s",
            "--date=short",
            "monorepo/import",
            "--",
            mono_path,
        ).splitlines()
        if not mono_log:
            sys.stderr.write(f"FAIL: Check 3: no history in monorepo for {mono_path}\n")
            sys.exit(1)

        src_first_sha, src_first_date, src_first_subj = src_log[-1].split("\t", 2)
        mono_first_sha, mono_first_date, mono_first_subj = mono_log[-1].split("\t", 2)
        expected_mono_first = commit_map.get(src_first_sha, src_first_sha) if prefix else src_first_sha

        ok = (
            len(src_log) == len(mono_log)
            and mono_first_sha == expected_mono_first
            and src_first_date == mono_first_date
            and src_first_subj == mono_first_subj
        )
        follow_results.append(
            {
                "repo": repo,
                "mono_path": mono_path,
                "src_commits": len(src_log),
                "mono_commits": len(mono_log),
                "src_first_sha": src_first_sha,
                "mono_first_sha": mono_first_sha,
                "first_date": mono_first_date,
                "first_subj": mono_first_subj,
                "status": "PASS" if ok else "FAIL",
            }
        )
        if not ok:
            sys.stderr.write(
                f"FAIL: Check 3 ({mono_path}): src=({len(src_log)}, {src_first_sha}) "
                f"expected_mono={expected_mono_first} got=({len(mono_log)}, {mono_first_sha})\n"
            )
            sys.exit(1)

print(f"    PASS: Check 3 (`git log --follow` verified on {len(follow_results)} sample files)")

# Acceptance Check 4: Nothing from the "never imported" list is in the tree.
ls_files = run_git(dest_repo, "ls-files").splitlines()
spec_pat = re.compile(r"copperroute|PCBench|/target")
extra_pat = re.compile(
    r"copperroute|PCBench|(^|/)target($|/|-)|freerouting|(^|/)node_modules/|(^|/)library/cache/|(^|/)pcbview/demo/"
)
spec_hits = [p for p in ls_files if spec_pat.search(p)]
extra_hits = [p for p in ls_files if extra_pat.search(p)]
if spec_hits or extra_hits:
    sys.stderr.write(f"FAIL: Check 4 (never-imported files present): {spec_hits or extra_hits}\n")
    sys.exit(1)
print("    PASS: Check 4 (`git ls-files` contains nothing from the never-imported list)")

# Acceptance Check 5: Boardroom's own files are unchanged.
exclude_args = [f":!{r['dir']}" for r in imported_rows]
diff_cmd_args = ["diff", boardroom_head, "monorepo/import", "--", ".", *exclude_args]
diff_out = run_git(dest_repo, *diff_cmd_args)
if diff_out:
    sys.stderr.write(f"FAIL: Check 5 (Boardroom files changed):\n{diff_out}\n")
    sys.exit(1)
print("    PASS: Check 5 (Boardroom's own files unchanged)")

# Analyze > 5 MB files per project from `git filter-repo --analyze` (5 MB = 5,000,000 bytes; also show MiB).
THRESHOLD_BYTES = 5_000_000
active_projects = ["boardroom"] + [r["repo"] for r in imported_rows]
large_files_by_project: dict[str, dict] = {}

for proj in active_projects:
    adir = scratch_dir / "analysis" / proj
    # Current HEAD tracked blob sizes in source main for whether a blob is currently tracked vs historical.
    head_tree_out = run_git(
        source_root / proj, "ls-tree", "-r", "-l", "refs/heads/main", ro=True
    )
    head_blobs: dict[str, list[tuple[str, int]]] = {}
    head_large_tracked: list[dict] = []
    for tline in head_tree_out.splitlines():
        if not tline.strip():
            continue
        meta, path = tline.split("\t", 1)
        parts = meta.split()
        mode, otype, sha, size_s = parts[0], parts[1], parts[2], parts[3]
        if otype != "blob" or size_s == "-":
            continue
        sz = int(size_s)
        head_blobs.setdefault(sha, []).append((path, sz))
        if sz >= THRESHOLD_BYTES:
            head_large_tracked.append({"sha": sha[:12], "bytes": sz, "path": path})

    # Parse `blob-shas-and-paths.txt` from `git filter-repo --analyze`.
    blob_report = adir / "blob-shas-and-paths.txt"
    large_blobs_all: list[dict] = []
    if blob_report.is_file():
        for bline in blob_report.read_text(encoding="utf-8").splitlines():
            bline_s = bline.strip()
            if not bline_s or bline_s.startswith("===") or bline_s.startswith("Format:") or bline_s.startswith("SHA"):
                continue
            m = re.match(r"^([0-9a-f]{40})\s+(\d+)\s+(\d+)\s+(.+)$", bline_s)
            if not m:
                continue
            bsha, unpacked_s, packed_s, bpaths = m.groups()
            unpacked = int(unpacked_s)
            packed = int(packed_s)
            if unpacked >= THRESHOLD_BYTES:
                large_blobs_all.append(
                    {
                        "sha": bsha[:12],
                        "unpacked": unpacked,
                        "packed": packed,
                        "paths": bpaths,
                        "in_head": bsha in head_blobs,
                    }
                )

    # Parse `path-all-sizes.txt` for cumulative unpacked history >= 5 MB.
    path_report = adir / "path-all-sizes.txt"
    large_cum_paths: list[dict] = []
    if path_report.is_file():
        for pline in path_report.read_text(encoding="utf-8").splitlines():
            m = re.match(r"^\s*(\d+)\s+(\d+)\s+(<present>|\d{4}-\d{2}-\d{2})\s+(.+)$", pline)
            if not m:
                continue
            unpacked_s, packed_s, pstatus, ppath = m.groups()
            unpacked = int(unpacked_s)
            packed = int(packed_s)
            if unpacked >= THRESHOLD_BYTES:
                large_cum_paths.append(
                    {
                        "unpacked": unpacked,
                        "packed": packed,
                        "status": "present" if pstatus == "<present>" else f"deleted ({pstatus})",
                        "path": ppath,
                    }
                )

    large_files_by_project[proj] = {
        "head_tracked": sorted(head_large_tracked, key=lambda x: -x["bytes"]),
        "history_blobs": large_blobs_all,
        "cumulative_paths": large_cum_paths,
    }

total_monorepo_tags = len(
    [t for t in run_git(dest_repo, "tag", "-l").splitlines() if t.strip()]
)
total_monorepo_commits = int(run_git(dest_repo, "rev-list", "--count", "monorepo/import").strip())

# Render scripts/monorepo/import-report.md
now_iso = datetime.now(timezone.utc).astimezone().isoformat(timespec="seconds")
md: list[str] = []
md.append("# Monorepo Import Report")
md.append("")
md.append(f"- **Generated:** `{now_iso}`")
md.append(f"- **Scratch workspace:** `{scratch_dir}`")
md.append(f"- **Merged repository:** `{dest_repo}` (branch `monorepo/import`, HEAD `{monorepo_head}`)")
md.append(f"- **Total commits on `monorepo/import`:** {total_monorepo_commits}")
md.append(f"- **Total tags in merged repo:** {total_monorepo_tags}")
md.append(f"- **`board-draw` included:** {'yes (`--with-board-draw`)' if with_board_draw else 'no (Stage 1 default; supported via `--with-board-draw`)'}")
md.append("")

md.append("## 1. Source and Imported Heads")
md.append("")
md.append("| Project | Target Dir | Source `main` SHA | `origin/main` SHA | Local vs Remote | Source Commits | Source Tags (`main`) | Filtered Head SHA | Merge Commit on `monorepo/import` | Imported Tags |")
md.append("|---|---|---|---|---|---:|---:|---|---|---:|")

br = sources_info["boardroom"]
br_vs = "in sync" if br["ahead"] == 0 and br["behind"] == 0 else f"+{br['ahead']} / -{br['behind']}"
md.append(
    f"| `boardroom` | `.` (root) | `{br['main_sha']}` | `{br['origin_main_sha'] or 'none'}` | {br_vs} | "
    f"{br['main_commits']} | {br['main_tag_count']} | *(base branch)* | `{boardroom_head}` *(initial base)* | {br['main_tag_count']} |"
)

for row in imported_rows:
    s = sources_info[row["repo"]]
    vs = "in sync" if s["ahead"] == 0 and s["behind"] == 0 else f"**+{s['ahead']} ahead** / -{s['behind']}"
    md.append(
        f"| `{row['repo']}` | `{row['dir']}/` | `{s['main_sha']}` | `{s['origin_main_sha'] or 'none'}` | {vs} | "
        f"{s['main_commits']} | {s['main_tag_count']} | `{row['filtered_sha']}` | `{row['merge_sha']}` | {row['tag_count']} |"
    )

if not with_board_draw and "board-draw" in sources_info:
    bd = sources_info["board-draw"]
    bd_vs = "in sync" if bd["ahead"] == 0 and bd["behind"] == 0 else f"+{bd['ahead']} / -{bd['behind']}"
    md.append(
        f"| `board-draw` *(not merged in Stage 1)* | `board-draw/` | `{bd['main_sha']}` | `{bd['origin_main_sha'] or 'none'}` | {bd_vs} | "
        f"{bd['main_commits']} | {bd['main_tag_count']} | *(skipped without `--with-board-draw`)* | *(skipped)* | 0 |"
    )

md.append("")

# Local main ahead details
ahead_any = [repo for repo, info in sources_info.items() if info["ahead"] > 0]
if ahead_any:
    md.append("### Local `main` ahead of `origin/main` (included via local `--no-local` clone)")
    md.append("")
    for repo in ahead_any:
        info = sources_info[repo]
        md.append(
            f"- **`{repo}`**: local `main` (`{info['main_sha'][:7]}`) is **{info['ahead']} commit(s) ahead** of `origin/main` (`{info['origin_main_sha'][:7]}`):"
        )
        for c in info["ahead_commits"]:
            md.append(f"  - `{c}`")
    md.append("")

md.append("## 2. Unmerged Branches (Not Imported)")
md.append("")
md.append(f"Full machine-readable list written to `{branches_out}`:")
md.append("")
md.append("| Source Repo | Unmerged Local Branches | Unmerged Remote Branches | Unmerged Detached Worktrees |")
md.append("|---|---:|---:|---:|")
for repo, info in sources_info.items():
    loc_cnt = sum(1 for u in info["unmerged"] if u["kind"] == "local")
    rem_cnt = sum(1 for u in info["unmerged"] if u["kind"] == "remote")
    det_cnt = len(info["detached_unmerged"])
    md.append(f"| `{repo}` | {loc_cnt} | {rem_cnt} | {det_cnt} |")
md.append("")

for repo, info in sources_info.items():
    if info["unmerged"] or info["detached_unmerged"]:
        md.append(f"### `{repo}` unmerged refs")
        md.append("")
        md.append("| Ref | Commit | Last Commit Date | Checked-out Worktree |")
        md.append("|---|---|---|---|")
        for u in info["unmerged"]:
            wt_disp = ", ".join(f"`{w}`" for w in u["worktrees"]) if u["worktrees"] else "—"
            md.append(f"| `{u['kind']}:{u['name']}` | `{u['sha']}` | `{u['date']}` | {wt_disp} |")
        for d in info["detached_unmerged"]:
            md.append(f"| `detached:(HEAD)` | `{d['sha']}` | `{d['date']}` | `{d['worktree']}` |")
        md.append("")

md.append("## 3. Files and Blobs Over 5 MB (`git filter-repo --analyze`)")
md.append("")
md.append("Threshold: 5 MB (5,000,000 bytes / 4.77 MiB). Inspected via `git filter-repo --analyze` on each project's `main` clone.")
md.append("")
for proj in active_projects:
    lf = large_files_by_project[proj]
    md.append(f"### `{proj}`")
    md.append("")
    if not lf["head_tracked"] and not lf["history_blobs"] and not lf["cumulative_paths"]:
        md.append("- None (no single blob or cumulative path history >= 5 MB).")
        md.append("")
        continue

    if lf["head_tracked"]:
        md.append("**Currently tracked at `HEAD` (>= 5 MB single file):**")
        md.append("")
        md.append("| Path | Size (MB) | Size (MiB) | Blob SHA |")
        md.append("|---|---:|---:|---|")
        for item in lf["head_tracked"]:
            mb = item["bytes"] / 1_000_000
            mib = item["bytes"] / (1024 * 1024)
            md.append(f"| `{item['path']}` | {mb:.2f} | {mib:.2f} | `{item['sha']}` |")
        md.append("")
    else:
        md.append("- Currently tracked single files at `HEAD` >= 5 MB: **none**.")
        md.append("")

    hist_only = [b for b in lf["history_blobs"] if not b["in_head"]]
    if hist_only:
        md.append("**Historical/superseded blobs in git history (>= 5 MB, not at `HEAD`):**")
        md.append("")
        md.append("| Path(s) | Unpacked (MB) | Packed (MB) | Blob SHA |")
        md.append("|---|---:|---:|---|")
        for item in hist_only:
            md.append(
                f"| `{item['paths']}` | {item['unpacked']/1_000_000:.2f} | {item['packed']/1_000_000:.2f} | `{item['sha']}` |"
            )
        md.append("")

    if lf["cumulative_paths"]:
        md.append("**Paths with cumulative unpacked history >= 5 MB across commits (`path-all-sizes.txt`):**")
        md.append("")
        md.append("| Path | Cumulative Unpacked (MB) | Cumulative Packed (MB) | Status at `HEAD` |")
        md.append("|---|---:|---:|---|")
        for item in lf["cumulative_paths"]:
            md.append(
                f"| `{item['path']}` | {item['unpacked']/1_000_000:.2f} | {item['packed']/1_000_000:.2f} | {item['status']} |"
            )
        md.append("")

md.append("## 4. Stage 1 Acceptance Checks")
md.append("")

md.append("### Check 1: Mirror extraction reproduces exact original `main` SHA")
md.append("")
md.append("Extracted from `monorepo/import` with `git clone --no-local <monorepo> x && git -C x filter-repo --force --preserve-commit-hashes --subdirectory-filter <dir>`:")
md.append("")
md.append("| Directory | Source Repo | Source `main` SHA | Extracted `HEAD` SHA | Result |")
md.append("|---|---|---|---|---|")
for m in mirror_rows:
    md.append(
        f"| `{m['dir']}/` | `{m['repo']}` | `{m['expected_sha']}` | `{m['extracted_sha']}` | **{m['status']}** |"
    )
md.append("")

md.append("### Check 2: Only branch is `monorepo/import` (`git branch -a`)")
md.append("")
md.append("```text")
md.append(f"$ git -C {dest_repo} branch -a")
md.append(branch_a_out)
md.append("```")
md.append("")
md.append("- Result: **PASS** (no other local or remote/imported branches).")
md.append("")

md.append("### Check 3: `git log --follow` on three sample files per project reaches first commit")
md.append("")
md.append("| Project | Path in Monorepo | Commits (Source / Monorepo) | Source First SHA | Monorepo First SHA | First Commit Date & Subject | Result |")
md.append("|---|---|---|---|---|---|---|")
for f in follow_results:
    md.append(
        f"| `{f['repo']}` | `{f['mono_path']}` | {f['src_commits']} / {f['mono_commits']} | "
        f"`{f['src_first_sha'][:10]}` | `{f['mono_first_sha'][:10]}` | `{f['first_date']}` — {f['first_subj']} | **{f['status']}** |"
    )
md.append("")

md.append("### Check 4: Nothing from the \"never imported\" list is in the tree")
md.append("")
md.append("```text")
md.append(f"$ git -C {dest_repo} ls-files | grep -E 'copperroute|PCBench|/target'")
md.append("(empty — 0 matches)")
md.append("```")
md.append("")
md.append(f"- Total tracked entries in merged index (`git ls-files | wc -l`): `{len(ls_files)}`")
md.append("- Result: **PASS**.")
md.append("")

md.append("### Check 5: Boardroom's own files are unchanged")
md.append("")
excl_display = " ".join(f"'{a}'" for a in exclude_args)
md.append("```text")
md.append(f"$ git -C {dest_repo} diff {boardroom_head[:7]} monorepo/import -- . {excl_display}")
md.append("(empty — 0 lines of diff)")
md.append("```")
md.append("")
md.append("- Result: **PASS**.")
md.append("")

report_out.write_text("\n".join(md).rstrip() + "\n", encoding="utf-8")
PY

echo "==> All Stage 1 checks PASSED."
echo "    Monorepo:      $dest_repo (branch monorepo/import)"
echo "    Branches list: $branches_out"
echo "    Import report: $report_out"
