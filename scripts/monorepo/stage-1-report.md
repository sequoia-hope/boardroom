# Stage 1 Report — Monorepo Import Script

- **Branch:** `monorepo/stage-1`
- **Worktree:** `~/Software/boardroom/.claude/worktrees/monorepo-stage-1`
- **Scratch directory:** `<scratchpad>/stage1`
- **Produced monorepo:** `<scratchpad>/stage1/boardroom` (branch `monorepo/import`, HEAD `0d898e097e4ab76d3375ac106c59a4685530fed3`)

## What Changed (Commits on `monorepo/stage-1`)

1. `dff5fba` — `Stage 1: add monorepo import.sh, branches.txt, and import-report.md`
   - `scripts/monorepo/import.sh`: re-runnable history importer taking `<scratch-dir>` (and optional `--with-board-draw` for Stage 3). Clones `~/Software/{boardroom,circuit-composer,pcbview,ultraroute,board-draw}` (`main` only) with `git clone --no-local`, never modifying any source checkout or worktree; runs `git filter-repo --analyze` per project; rewrites `circuit-composer` → `composer/`, `pcbview` → `pcbview/`, `ultraroute` → `ultraroute/` (and `board-draw` → `board-draw/` behind `--with-board-draw`) with `git filter-repo --preserve-commit-hashes --to-subdirectory-filter <dir> --tag-rename '':'<dir>-'`; merges each into a Boardroom clone on `monorepo/import` with `--allow-unrelated-histories --no-ff`; runs all Stage 1 acceptance checks; writes `branches.txt` and `import-report.md`.
   - `scripts/monorepo/branches.txt`: every local and remote branch across all 5 source checkouts not merged into local `main`, with short SHA, commit date, and checked-out worktree (if any).
   - `scripts/monorepo/import-report.md`: source heads, local-ahead-of-remote report (`circuit-composer` `main` `bff8c19` is 3 commits ahead of `origin/main` `7ee8bfc`), imported heads, tag counts, `git filter-repo --analyze` > 5 MB summary, and acceptance check results.
2. Follow-up commit on `monorepo/stage-1` — `Stage 1: add stage-1-report.md` (`scripts/monorepo/stage-1-report.md`, this file).

First-parent commit log in the produced scratch monorepo (`.../scratchpad/stage1/boardroom`):

```text
$ git -C <scratchpad>/stage1/boardroom log --first-parent --oneline -n 6 monorepo/import
0d898e0 (HEAD -> monorepo/import) Import ultraroute history into ultraroute/
ad20f27 Import pcbview history into pcbview/
3e97497 Import circuit-composer history into composer/
48a41a5 Spec: one repo and one app for Boardroom, Composer, pcbview and UltraRoute
66be834 Remove bluebuzz: easy2350's USB audit stands on its own, ultraroute bumped to b38a039
2fd1f7a Hosted git (stage 2): partial mirrors, sign-in hints, links that travel, push state
```

## Stage 1 Acceptance Checks (Commands and Outputs)

In all commands below:

```sh
SCRATCH=<scratchpad>/stage1
REPO=$SCRATCH/boardroom
```

### 1. Extracting `<dir>` from `monorepo/import` gives exact source `main` SHA

Command:

```sh
for pair in "composer:circuit-composer" "pcbview:pcbview" "ultraroute:ultraroute"; do
  dir="${pair%%:*}"
  src="${pair#*:}"
  rm -rf "$SCRATCH/mirrors/$dir"
  git clone --quiet --no-local "$REPO" "$SCRATCH/mirrors/$dir"
  git -C "$SCRATCH/mirrors/$dir" filter-repo --force --quiet --preserve-commit-hashes --subdirectory-filter "$dir"
  src_sha="$(GIT_OPTIONAL_LOCKS=0 git --no-optional-locks -C "$HOME/Software/$src" rev-parse refs/heads/main)"
  ext_sha="$(git -C "$SCRATCH/mirrors/$dir" rev-parse HEAD)"
  echo "$dir ($src): source=$src_sha extracted=$ext_sha match=$([[ "$src_sha" == "$ext_sha" ]] && echo YES || echo NO)"
done
```

Output:

```text
composer (circuit-composer): source=bff8c19b9c8ae2fede9b159047cbb2d2b59f4e20 extracted=bff8c19b9c8ae2fede9b159047cbb2d2b59f4e20 match=YES
pcbview (pcbview): source=26ba32bb8edbc5761d84e3238a312a29cbcc1aca extracted=26ba32bb8edbc5761d84e3238a312a29cbcc1aca match=YES
ultraroute (ultraroute): source=13947cfb50601ebca4ffc590e4e346f2f9d33944 extracted=13947cfb50601ebca4ffc590e4e346f2f9d33944 match=YES
```

Also verified `--with-board-draw` in an isolated scratch subdirectory (`$SCRATCH/test-with-board-draw`, removed after verification):

```text
PASS: composer -> bff8c19b9c8ae2fede9b159047cbb2d2b59f4e20 (matches circuit-composer main)
PASS: pcbview -> 26ba32bb8edbc5761d84e3238a312a29cbcc1aca (matches pcbview main)
PASS: ultraroute -> 13947cfb50601ebca4ffc590e4e346f2f9d33944 (matches ultraroute main)
PASS: board-draw -> b45cc3f689dc7b5086be9dada3dd64381a3d010e (matches board-draw main)
```

Result: **PASS**.

### 2. Only branch is `monorepo/import`; `git branch -a` shows no imported branches

Command:

```sh
git -C "$REPO" branch -a
git -C "$REPO" remote -v
```

Output:

```text
* monorepo/import
```

(`git remote -v` output is empty.)

Result: **PASS**.

### 3. `git log --follow` on three sample files per project reaches their first commit

Command:

```sh
for p in \
  "README.md" \
  ".claude/skills/pcb-design/SKILL.md" \
  "app/server/src/main.rs" \
  "composer/README.md" \
  "composer/Cargo.toml" \
  "composer/crates/ccomp/src/main.rs" \
  "pcbview/README.md" \
  "pcbview/pcbview/site.py" \
  "pcbview/web/app.js" \
  "ultraroute/README.md" \
  "ultraroute/core/crates/ultraroute/src/main.rs" \
  "ultraroute/tools/pcbench.py"
do
  c="$(git -C "$REPO" log --follow --oneline monorepo/import -- "$p" | wc -l)"
  first="$(git -C "$REPO" log --follow --format='%h %ad %s' --date=short monorepo/import -- "$p" | tail -n 1)"
  echo "$p ($c commits) -> first: $first"
done
```

Output:

```text
README.md (10 commits) -> first: 9aff8c2 2026-10-22 easy_pcb: a board project template with the pcb-design skill and pcbview
.claude/skills/pcb-design/SKILL.md (3 commits) -> first: 9aff8c2 2026-10-22 easy_pcb: a board project template with the pcb-design skill and pcbview
app/server/src/main.rs (9 commits) -> first: 69aaa52 2026-10-29 Boardroom app, milestone 1: project list and overview
composer/README.md (31 commits) -> first: b727551 2026-10-27 Milestone 1: intent schema v0, validator, ccomp CLI; easy2350 annotated
composer/Cargo.toml (3 commits) -> first: b727551 2026-10-27 Milestone 1: intent schema v0, validator, ccomp CLI; easy2350 annotated
composer/crates/ccomp/src/main.rs (22 commits) -> first: b727551 2026-10-27 Milestone 1: intent schema v0, validator, ccomp CLI; easy2350 annotated
pcbview/README.md (5 commits) -> first: 9ee64fb 2026-09-28 pcbview: KiCad boards on the web, split out of servodrive's project pages
pcbview/pcbview/site.py (5 commits) -> first: 9ee64fb 2026-09-28 pcbview: KiCad boards on the web, split out of servodrive's project pages
pcbview/web/app.js (2 commits) -> first: 9ee64fb 2026-09-28 pcbview: KiCad boards on the web, split out of servodrive's project pages
ultraroute/README.md (7 commits) -> first: 126f0a4 2026-10-22 Start the ultraroute workspace repo: brief, status page, route-job tools, bluebuzz job
ultraroute/core/crates/ultraroute/src/main.rs (24 commits) -> first: b62919a 2026-10-24 Phase 2c step 3c: the ultraroute command and the Core router in the studio
ultraroute/tools/pcbench.py (26 commits) -> first: 68cff72 2026-10-26 Core: electrical validity from KiCad's own rules; soft via-avoid areas; PCBench harness
```

Every commit count matches `git log --follow` in the corresponding source repo `main`, and every earliest commit SHA matches the source repo's earliest commit SHA via `.git/filter-repo/commit-map` (`a1d4dcd` → `b727551` for `composer/`, `98e7f57` → `9ee64fb` for `pcbview/`, `78c0fcb`/`510678f`/`42f9c71` → `126f0a4`/`b62919a`/`68cff72` for `ultraroute/`).

Result: **PASS**.

### 4. Nothing from the "never imported" list is in the tree

Command:

```sh
git -C "$REPO" ls-files | grep -E 'copperroute|PCBench|/target'
```

Output:

```text
(empty — grep exited 1 with 0 matches across 1322 tracked files)
```

Result: **PASS**.

### 5. Boardroom's own files are unchanged (including `vendor/` submodules)

Command:

```sh
BR_HEAD="$(GIT_OPTIONAL_LOCKS=0 git --no-optional-locks -C ~/Software/boardroom rev-parse refs/heads/main)"
git -C "$REPO" diff "$BR_HEAD" monorepo/import -- . ':!composer' ':!pcbview' ':!ultraroute'
git -C "$REPO" ls-tree monorepo/import .gitmodules vendor/pcbview vendor/ultraroute
```

Output:

```text
(git diff output empty — 0 lines)
100644 blob 84c381a31fc257a6b04f2bc9ce9dd55342c80d7b	.gitmodules
160000 commit 26ba32bb8edbc5761d84e3238a312a29cbcc1aca	vendor/pcbview
160000 commit b38a039495387813147dca046ff48d2b13954a5f	vendor/ultraroute
```

Result: **PASS**.

### Re-runnability Verification

`scripts/monorepo/import.sh "$SCRATCH"` was executed twice against `$SCRATCH` (which also contains `agy-stage1.log`). Both runs exited `0`, passed all checks, left `agy-stage1.log` intact, and left all five original checkouts under `~/Software/` untouched.

## Anything Skipped and Why

- **`board-draw` merge in the default Stage 1 output:** skipped by design per `docs/monorepo-spec.md` (deferred to Stage 3); supported behind `--with-board-draw` and verified separately.
- **`vendor/{pcbview,ultraroute}` removal:** skipped in Stage 1 per Step 4 of Stage 1 (deferred to Stage 3).
- **"Proving nothing changed" build/test suite:** applies from Stage 2 onward (`docs/monorepo-spec.md`: *"Every stage from 2 on runs these and attaches the output to its report"*); Stage 1 moves files into subdirectories without modifying any file contents.

## Spec Drift / Nuances Found in `docs/monorepo-spec.md`

1. **Commit count drift ("What exists today"):** Boardroom `main` has **77** commits (`48a41a5`, which added `docs/monorepo-spec.md` on top of `66be834`) rather than 76.
2. **Merged worktree branches ("What exists today"):** `circuit-composer`'s `agy/shape-interface` (`f9f3cb9`) and `boardroom`'s `gui/review-tests` (`ac760ed`) are already ancestors of their respective `main` branches, so neither is unmerged; at run time only `ultraroute` had unmerged branches (20 local, 10 remote). Note that once this worktree commits on `monorepo/stage-1`, subsequent runs of `import.sh` before `monorepo/stage-1` is merged will also list `local:monorepo/stage-1` under `boardroom`.
3. **Branch isolation (`git clone` / `git fetch` vs `git branch -a`):** A plain `git clone --no-local` of a multi-branch checkout copies all local branches of the source repo into `remotes/origin/*`, and `git fetch <clone> main:refs/remotes/<dir>/main` leaves `remotes/<dir>/main` visible in `git branch -a` unless removed after merging. `import.sh` clones with `--single-branch --branch main`, renames `main` to `monorepo/import`, removes `origin`, and deletes `refs/remotes/<dir>/main` after each `--no-ff` merge so `git branch -a` shows strictly `* monorepo/import`.
4. **`git filter-repo --analyze` > 5 MB:** No project has any *single* blob >= 5 MB at `HEAD` or in `main` history (largest single blobs are 4.68 MB in `ultraroute`, 3.30 MB in `boardroom`, 2.65 MB in `circuit-composer`), while `boardroom` (14 paths, up to 32.55 MB unpacked for `examples/easy2350/viewer/easy2350-a/3d/board.glb`) and `ultraroute` (4 paths) have paths whose *cumulative* unpacked size across revisions exceeds 5 MB in `path-all-sizes.txt`. `import-report.md` reports both.
