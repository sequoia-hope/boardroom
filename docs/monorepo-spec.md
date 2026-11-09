# One repo, one app: Boardroom, Circuit Composer, pcbview and UltraRoute — spec (draft)

Status: draft, 2026-11-08. Written from the owner's request: "merge boardroom, circuit composer,
pcbview, and ultraroute into one app/repo; Google Antigravity agents will execute." Nothing here is
built. Choices the owner has not confirmed are marked **Recommended**, and every open one is under
**Open questions**. The facts under "What exists today" were surveyed on 2026-11-08 and will drift;
the import script re-checks them.

**Decided by the owner, 2026-11-08:** keep the full history of every project; import **`main`
only** (no other branches); pcbview is licensed **MIT**.

This spec **supersedes** `suite-spec.md`'s "Repositories stay separate, with pinned revisions"
(section "Normalizing the toolkit") for these four projects. Waffle Iron and `board-interface` stay
separate repos.

## The idea in one paragraph

The four projects become one repository, `sequoia-hope/boardroom`, with each project's full git
history kept under its own directory. They then become one app: one Cargo workspace, one npm
workspace, one toolchain pin, one `app/build.sh`, one server on one registered port, and one
window. Boardroom's server serves Composer and Studio itself instead of proxying two other
servers. The CLIs (`ultraroute`, `ccomp`, `pcbview`) stay CLIs, because the skill, the gates and
the benchmark drive them. Board repos keep working unchanged: `sequoia-hope/ultraroute` and
`sequoia-hope/pcbview` stay alive as read-only mirrors, extracted from the monorepo, with the **same
commit IDs** they have today, so every existing submodule pin still resolves.

## What exists today

| | Boardroom | Circuit Composer | pcbview | UltraRoute |
|---|---|---|---|---|
| Local checkout | `~/Software/boardroom` | `~/Software/circuit-composer` | `~/Software/pcbview` | `~/Software/ultraroute` |
| Commits on main | 76 | 87 | 9 | 459 (192 since 2026-11-01) |
| Branches | main + 1 worktree | main + `agy/shape-interface` | main | 141 local, 79 remote, 9 worktrees |
| `.git` size | 227 MB | 22 MB (all loose objects) | 1.7 MB | 21 MB |
| Language | Rust (axum server, GTK 4 window), Svelte 5 + Vite 8, Python skill scripts | Rust workspace (13 crates + `app/core` WASM), Svelte 5 + Vite 8, Python `tools/` | Python 3.11 (no packaging), plain ES modules, vendored three.js | Rust workspaces `core/` (9 crates) and `studio/` (core WASM + server), Svelte 5 + Vite 8, Python `tools/` |
| Rust toolchain | unpinned | unpinned (CI: stable) | — | `core/rust-toolchain.toml` 1.95.0 |
| Cargo layout | `app/server`, `app/gtk` each own lock and `target/` | one workspace at root | — | two workspaces, `core/` and `studio/` |
| Served by | `app/serve.sh` → `boardroom serve`, proj `boardroom` (8054, pinned) | `serve.sh` → `python3 -m http.server`, proj `circuit-composer` | `serve.sh`, proj `pcbview` | `serve.sh` → `studio-server`, proj `ultraroute` |
| CI | `pages.yml` | `pages.yml` | none | `pages.yml`, `test.yml` (clippy + `cargo test --release`, 1.95.0) |
| License | MIT | MIT | **none** | MIT; `core/crates/kicad-text` is **GPL-3.0-or-later** (separate process, boundary test); studio crates have no license field |

How they reference each other today:

- **Boardroom → pcbview, ultraroute:** git submodules `vendor/pcbview` (26ba32b) and
  `vendor/ultraroute` (b38a039), driven by `scripts/view.sh` and `scripts/route.sh`.
  `scripts/new-board.sh` adds the same submodules at the same SHAs to every new board repo.
  `app/web/scripts/draw.sh` separately sparse-clones ultraroute at **ce50b64** to build Studio's
  draw core and router WASM.
- **Boardroom → Composer, Studio at run time:** `app/server/src/apps.rs` proxies
  `/apps/composer/` to `proj port circuit-composer` and `/apps/studio/` to `proj port ultraroute`.
  Both must be running separately.
- **Composer → ultraroute:** `kicad-pcb = { git = ".../ultraroute", rev = "0e581c4…" }` in
  `crates/pcb` and `app/core`. `tools/route_board.sh` takes the router from `$ULTRAROUTE`.
  `tools/handcheck.py` defaults to `~/Software/ultraroute-lab/pcbench`.
- **Composer → Boardroom:** `tools/compose_kicad.py --easy-pcb DIR` (or `$EASY_PCB`) uses
  Boardroom's `scripts/kicad_write.py`. `app/web/src/lib/store.js` is a copy of Boardroom's
  `app/store/store.js`. Six `packages/*/manifest.toml` and two `boards/*/pcbview.toml` name
  `~/Software/easy_pcb`.
- **board-draw** (separate repo) is pinned at **two** revisions: d37b1f8 (ultraroute studio,
  Boardroom `app/web`) and b45cc3f (Composer). Cargo and npm pins must match each other.
- **board-interface** v0.2.2 is used by Boardroom's server and Composer (`crates/ccomp`,
  `crates/shape`). Composer's tests look for its golden files at `../../../board-interface/golden`.
- **Board repos outside this one** (`~/pcb/round-pendant`, simple-earring, robot-link) pin
  `vendor/pcbview` and `vendor/ultraroute` by SHA.
- **The ultraroute benchmark** (`tools/pcbench.py`) needs the untracked `PCBench/` clone (2.7 GB)
  inside the main checkout, prepared boards in `~/Software/ultraroute-lab/pcbench/` (26 GB), base
  binaries in `~/Software/ultraroute-lab/bin/`, KiCad 9, and `reserve`. Its `gate` tier (314
  boards) takes about 25 minutes on 15 cores and is how every ultraroute change is judged
  ("gate main X → Y: lost 0, won 1…").

Not tracked and never imported: ultraroute's `copperroute/`, `copperroute-main/` (GPL, 28 GB),
`freerouting/`, `PCBench/`, `target-topo/`, every `target*/`, `node_modules/`, `dist/`;
Composer's `library/cache/` (155 MB) and `bench/`; pcbview's `demo/`. ultraroute's untracked
`REVIEW.md` and `REMEDIATION.md` are an open question.

## The owner's direction

1. Merge Boardroom, Circuit Composer, pcbview and UltraRoute into one repo and one app.
2. Google Antigravity agents execute it. Claude writes the spec, launches the agents, audits each
   stage and commits.

## Principles

1. **Keep every commit.** Each project's history is imported under its directory with
   `git filter-repo --to-subdirectory-filter`. No history is squashed, stripped or re-authored.
   Stripping large blobs would change commit IDs and break the mirrors (below), so it is out of
   scope.
2. **Move first, change second.** Stage 1 moves files and changes nothing inside them. Every
   later stage changes one kind of thing, and proves the outputs did not change.
3. **Same inputs, same bytes.** A merge stage must not change a route, a floorplan or a viewer.
   The checks in "Proving nothing changed" compare outputs byte for byte against the pre-merge
   tools. If a stage cannot keep them identical (a toolchain bump), it stops and reports, and the
   change goes through ultraroute's gate like any router change.
4. **Board repos don't notice.** Existing submodule pins resolve, `scripts/{view,route}.sh` keep
   their interface, `new-board.sh` makes the same layout.
5. **The ultraroute work doesn't stop for long.** It is the busiest of the four (about 25 commits
   a day). The import is a script that can be re-run on the latest heads, so cutover is one short
   freeze, not a week.
6. **Rules stay where they were true.** No hardcoded ports, `reserve` for heavy jobs, never edit
   `vendor/` or `viewer/`, the GPL boundary (`kicad-text` and copperroute run as separate
   processes, never linked), KiCad files as the shared truth, each app writes only its own
   directory in a board project.

## The layout

**Recommended:** Boardroom stays at the root, where it is, and the others come in as top-level
directories with their inner layout untouched. This keeps `app/server`'s
`env!("CARGO_MANIFEST_DIR")/../..` repo-root logic, every path in the skill, and the Pages build
working, and it makes the mirrors a plain extraction of one directory.

```
boardroom/                      (sequoia-hope/boardroom)
  app/                          Boardroom: server, gtk, web, store        (unchanged)
  docs/ scripts/ template/ examples/ examples.toml
  .claude/skills/pcb-design/    (unchanged)
  .claude/skills/circuit-composer/   moved up from composer/.claude/skills/ (stage 3)
  composer/                     all of circuit-composer: crates/, app/, tools/, packages/, boards/, library/, schema/, docs/
  pcbview/                      all of pcbview: pcbview/, bin/, web/, tests/
  ultraroute/                   all of ultraroute: core/, studio/, tools/, jobs/, docs/, lab/, legacy/
  board-draw/                   all of board-draw (stage 3, Recommended)
  Cargo.toml                    one workspace (stage 4)
  package.json                  one npm workspace (stage 4)
  rust-toolchain.toml           one pin (stage 4)
  .github/workflows/            all CI, path-filtered (stage 5)
  vendor/                       removed in stage 3: pcbview and ultraroute are in the tree
```

Directory names: `composer/` (not `circuit-composer/`), `pcbview/`, `ultraroute/`,
`board-draw/`. The repo keeps the name `boardroom`; the app is Boardroom, and Composer and Studio
are its workspaces, as `platform-spec.md` already says.

## The mirrors: how board repos keep working

`sequoia-hope/ultraroute` and `sequoia-hope/pcbview` become read-only mirrors of `ultraroute/`
and `pcbview/`:

- A workflow (`.github/workflows/mirrors.yml`) extracts each directory on every push to main and
  fast-forwards the mirror's `main`. It never force-pushes; a non-fast-forward fails the job
  loudly. The extraction, used everywhere this spec says "extract":
  ```sh
  git clone --no-local <monorepo> x && cd x
  git filter-repo --force --preserve-commit-hashes --subdirectory-filter ultraroute
  ```
- **Tested 2026-11-08** in a scratch copy of all five repos: importing with
  `filter-repo --preserve-commit-hashes --to-subdirectory-filter <dir>`, merging into Boardroom,
  then extracting as above gives back **exactly** each original `main` SHA (pcbview 26ba32b,
  circuit-composer bff8c19, ultraroute 13947cf, board-draw b45cc3f), and a later commit touching
  two directories lands on each mirror as a fast-forward.
- **Two traps found in that test; don't use the obvious tools:**
  - Without `--preserve-commit-hashes`, filter-repo rewrites commit IDs quoted in commit messages
    ("From Boardroom's check of 5a279b1 …"), so every later SHA changes.
  - `git subtree split` does **not** work: it drops `--no-ff` merges whose tree equals a parent's
    (composer's 7ee8bfc, ultraroute's 76102c6 and others), so its output never matches.
- Commits that touch only `ultraroute/` appear in the mirror unchanged in content. Commits that
  also touch other directories appear with just their `ultraroute/` part. Commits that don't
  touch it are dropped.
- **Constraint this places on later stages:** `ultraroute/core/` and `pcbview/` must stay
  self-contained, with no path dependencies or file references outside their own directory, so
  the mirror builds on its own and board repos' `scripts/route.sh` and `scripts/view.sh` keep
  working. Studio, Composer and Boardroom may depend on them; never the other way round. Each
  stage's audit checks this by extracting and building the extracted tree.
- The mirrors carry no workflows after cutover (CI moves to the root in stage 5), so pushes to
  them run nothing. Their READMEs gain a first line saying where development happens.
- `sequoia-hope/circuit-composer` and `sequoia-hope/board-draw` are archived with a pointer, not
  mirrored: no board repo pins them. **Recommended**; see Open questions.

`scripts/new-board.sh` keeps adding `vendor/pcbview` and `vendor/ultraroute` submodules pointing
at the mirrors. It pins the mirror commit that matches the Boardroom commit it runs from: the
mirror commit whose tree is `git rev-parse HEAD:ultraroute` (tree IDs are equal by
construction), found with `git log --format='%H %T'` on a fetch of the mirror. If no mirror commit
has that tree (unpushed local work), it says so and pins the mirror's `main` instead, as
`new-board.sh` falls back today.

## Proving nothing changed

Every stage from 2 on runs these and attaches the output to its report. "Before" is built from
the original repos at the imported heads; "after" from the merged tree. Builds and the router go
through `~/.claude/bin/reserve`.

| Check | Before | After | Must be |
|---|---|---|---|
| Route easy2350 | `ultraroute route examples/easy2350/hw/…/easy2350.kicad_pcb -o a.kicad_pcb --planes In1.Cu,In2.Cu --seed 1 --threads 1 --timeout 240` with the old binary | same command, new binary | identical bytes |
| Route the pcbench `smoke` tier | `pcbench.py run --tier smoke` with the old binary | new binary | identical routed files and scores (`pcbench.py compare` clean) |
| Composer floorplan | `ccomp -C examples/easy2350 floorplan --seed 2` with old `ccomp` | new `ccomp` | identical `compose/` output |
| Composer tests | `cargo test` in old checkout | same tests in the merged workspace | same pass list |
| ultraroute tests | `cargo test --workspace --release` in `core/` | same | same pass list, clippy `-D warnings` clean |
| pcbview | `scripts/view.sh build examples/easy2350/pcbview.toml` with old pcbview | in-tree pcbview | identical `viewer/` except the `<meta name=generator>` commit |
| Boardroom | `app/build.sh`, `server/tests`, `npm run test:store`, `test:stale`, `test:board` | same | pass |
| Boardroom smoke | `npm run smoke` against the running app | same | pass |
| Mirror | extract `ultraroute` and `pcbview` (see "The mirrors") | build the extracted tree on its own | builds; stage 1 also: extracted HEAD = original main SHA |

Only stage 4's toolchain bump may change router output. If it does, it runs ultraroute's gate
(`pcbench.py ab <main> <new> --tier gate`, about 25 min on 15 cores, booked with `reserve`) and
the owner decides; see stage 4.

## Stages

Each stage is one Antigravity run on its own branch, audited by Claude before it is committed to
main. Stages 1–2 are built in a scratch clone and only become real at cutover. From stage 3 on,
work happens in the monorepo itself.

### Stage 0: preconditions (owner and Claude, not an agent)

1. Done 2026-11-08: the owner answered the blocking questions (see the top of this spec).
2. Every Claude and Antigravity session working in the four repos is told the date of cutover.
   Only `main` is imported, so each session either merges the branch it wants kept into `main`
   before cutover, or moves it afterwards with the `format-patch` / `git am --directory` recipe
   under "Cutover". Branches nobody moves stay in the old repos (archived or mirrored), not lost.
3. Uncommitted work is committed or stashed by its owner session. On 2026-11-08: Boardroom has
   four modified files and `docs/kicad-extension-spec.md` untracked; Composer's main is three
   commits ahead of `origin/main` with about 20 modified files.
4. The owner creates the mirror push credential (a deploy key with write access on each mirror,
   stored as a repo secret on `sequoia-hope/boardroom`).

### Stage 1: the import script (agent)

Write `scripts/monorepo/import.sh` and run it in a scratch directory. It must be re-runnable: at
cutover it runs again on the latest heads.

1. Fresh `git clone --no-local` of each source repo (from the local checkouts, so unpushed
   commits on `main` are included; the script prints any local `main` ahead of its remote, and
   lists unmerged branches in the report so nothing is dropped silently). Never run `filter-repo`
   in an original checkout.
2. In each clone except Boardroom:
   `git filter-repo --preserve-commit-hashes --to-subdirectory-filter <dir> --tag-rename '':'<dir>-'`.
   `--preserve-commit-hashes` is required (see "The mirrors").
3. In a clone of Boardroom, on a branch `monorepo/import`: for each project,
   `git fetch <clone> main:refs/remotes/<dir>/main`, then
   `git merge --allow-unrelated-histories --no-ff <dir>/main -m "Import <project> history into <dir>/"`.
   Import `main` only, with its tags; no other branches (owner, 2026-11-08).
4. Remove the `vendor/pcbview` and `vendor/ultraroute` submodule entries from Boardroom's own
   tree **only** in a separate later commit (stage 3), not here.
5. Write `scripts/monorepo/branches.txt`: every branch in each source repo (local and remote)
   that is **not** merged into its `main`, with its last commit date and the worktree it is
   checked out in, if any. This is the list stage 0 hands to the sessions; nothing is imported
   from it.
6. Write `scripts/monorepo/import-report.md`: source heads, imported heads, tag counts, tracked
   files over 5 MB per project (`git filter-repo --analyze`), and the checks below.

Acceptance:

- Extracting `<dir>` from `monorepo/import` (see "The mirrors") gives exactly the source repo's
  `main` SHA, for `ultraroute`, `pcbview`, `composer` and (stage 3) `board-draw`.
- The only branch is `monorepo/import`; `git branch -a` in the result shows no imported branches.
- `git log --follow` on three sample files per project reaches their first commit.
- Nothing from the "never imported" list is in the tree (`git ls-files | grep -E 'copperroute|PCBench|/target'` is empty).
- Boardroom's own files are unchanged (`git diff 66be834 monorepo/import -- . ':!composer' ':!pcbview' ':!ultraroute'` is empty, with 66be834 replaced by Boardroom's head at run time).

### Stage 2: everything builds where it now lives (agent)

Make every project build, test and serve from its new directory, changing only paths. No
dependency changes, no workspace changes.

- Fix references that assume a project is a repo root or a sibling folder: `../boardroom`,
  `../ultraroute`, `../../../board-interface/golden`, `~/Software/easy_pcb`, `$EASY_PCB` defaults,
  `~/Software/ultraroute-lab` stays as is (out of tree, unchanged), each project's `.gitignore`
  patterns anchored at `/` (for example ultraroute's `/core/target-*/` is still correct relative
  to `ultraroute/.gitignore`; check each).
- Each project's `serve.sh` and `launch.sh` keep working from the subdirectory. `serve.sh` still
  gets its port from `--port`, `$PORT` or `proj port`, and fails with none. `proj port` resolves
  by directory; find out from `~/scripts/README.md` how it resolves a nested directory and
  report it. **Do not edit `~/scripts/launcher.toml`**; registry changes are Claude's, at cutover.
- `pcbench.py` finds `PCBench/` at `ultraroute/PCBench/` in the main checkout. Add
  `$ULTRAROUTE_PCBENCH` to override it, as `$ULTRAROUTE_LAB` already overrides the lab.
- Run "Proving nothing changed".

### Cutover (Claude and the owner)

A freeze of about an hour, announced in stage 0.

1. Sessions stop committing to the four old repos.
2. Re-run `import.sh` on the latest heads, re-apply stage 2's commits on top (they touch only
   paths, so they rebase cleanly; resolve by hand otherwise), re-run the stage 1 and 2 checks.
3. Merge `monorepo/import` into Boardroom's `main` and push.
4. Turn on `mirrors.yml` and check it fast-forwards both mirrors to the same SHAs they already
   have (a no-op push).
5. Delete the workflows from the old repos' default branches, add the README pointer, archive
   `circuit-composer` and `board-draw`.
6. On this machine: move `PCBench/` (2.7 GB, same filesystem, `mv`) to
   `~/Software/boardroom/ultraroute/PCBench/`. Leave `~/Software/ultraroute-lab` where it is.
   Leave the old checkouts in place, renamed `~/Software/<name>.premonorepo` (the owner's
   convention, as `ssh_client.premonorepo`), until every in-flight branch has moved. Never delete
   them as part of this work.
7. `proj`: point `ultraroute` and `circuit-composer` at their new directories (or retire their
   web entries once stage 6 lands), keep `boardroom` on its port. Run `proj doctor`.
8. The build server: `git pull` its `~/Software/boardroom` clone. Its `~/Software/ultraroute` clone
   is left alone. The owner's global `~/.claude/CLAUDE.md` names ultraroute's server clone; Claude
   proposes the edit and the owner makes it.
9. Moving in-flight work from an old checkout: in the old worktree,
   `git format-patch origin/main..HEAD`, then in a monorepo worktree
   `git am --directory=ultraroute/ *.patch`.
10. Claude's per-project memories live under `~/.claude/projects/` keyed by path. Claude copies
    the ones still true into the Boardroom project's memory. Agents don't touch them.

### Stage 3: one copy of each shared thing (agent)

Each item is its own commit.

1. **`kicad-pcb` by path.** Composer's `crates/pcb` and `app/core` use
   `{ path = "../../ultraroute/core/crates/kicad-pcb" }` instead of the git rev. Composer currently
   pins 0e581c4; if `ultraroute/` main's `kicad-pcb` differs in behaviour Composer depends on,
   Composer's tests say so: fix Composer's call sites, not `kicad-pcb`, and report.
2. **board-draw in the tree** (**Recommended**). Import `board-draw` the stage 1 way into
   `board-draw/`. Studio, Composer and Boardroom's `app/web` depend on it by path (Cargo) and by
   npm workspace (JS). This settles the d37b1f8 / b45cc3f split on board-draw's current head
   b45cc3f: Studio and Boardroom move forward. Check Studio and Boardroom's board view visually
   with `npm run test:board` and a screenshot each, since `Renderer(canvas, { yDown })` changed
   between the two.
3. **`draw.sh` goes.** Boardroom's `app/web` builds Studio's draw core and router WASM from
   `ultraroute/studio/core` and `ultraroute/core/crates/web` in the tree, not a sparse clone at
   ce50b64. Same check as above.
4. **Boardroom's own `vendor/` goes.** Remove the `vendor/pcbview` and `vendor/ultraroute`
   submodules from this repo. `scripts/view.sh` and `scripts/route.sh` look for
   `vendor/<tool>` (a board repo), then the in-tree `pcbview/` or `ultraroute/core` (this repo),
   then `$PCBVIEW` / `$ULTRAROUTE` as now. `scripts/doctor.sh` accepts either. `examples/easy2350`
   uses the in-tree tools. `new-board.sh` changes as described under "The mirrors".
5. **`store.js` once.** Composer and Studio import `app/store/store.js` instead of their copies.
6. **The Composer skill** moves to `.claude/skills/circuit-composer/` at the root, so a session
   anywhere in the repo finds it. Ask the owner whether `new-board.sh` should copy it into board
   repos too (platform-spec says each project carries a copy of each skill it uses).
7. **`compose_kicad.py`** finds Boardroom's `scripts/kicad_write.py` in the same repo by default;
   `--easy-pcb` and `$EASY_PCB` keep working as overrides.

Acceptance: "Proving nothing changed", and the mirror check (extract, build alone).

### Stage 4: one workspace, one toolchain, one lockfile (agent)

1. **Toolchain.** One `rust-toolchain.toml` at the root (**Recommended: 1.97.1**, the pin
   `board-interface` and Waffle Iron already use, per `suite-spec.md`). ultraroute is on 1.95.0.
   Build the router with both and compare the easy2350 route and the `smoke` tier. If any output
   differs, **stop and report**: Claude runs the gate tier and the owner decides whether to accept
   the change or keep ultraroute on 1.95.0 (a nested `ultraroute/core/rust-toolchain.toml` and a
   separate workspace for it).
2. **Cargo workspace** at the root: `app/server`, `composer/crates/*`, `composer/app/core`,
   `ultraroute/core/crates/*`, `ultraroute/studio/core`, `ultraroute/studio/server`,
   `board-draw`. `app/gtk` is excluded (it needs GTK and WebKitGTK system libraries); it keeps its
   own lockfile and builds from `app/build.sh` when `pkg-config` finds them.
   - One `Cargo.lock`. Versions are unified to what the projects already use where they agree.
     Studio's exact pins (`serde =1.0.219`, `serde_json =1.0.151`) and `wasm-bindgen =0.2.129`
     (Composer and Studio agree) are kept. Any version change that reaches ultraroute's core
     crates is checked like the toolchain above.
   - Profiles move to the root: Composer's `[profile.dev.package.floorplan] opt-level = 3`,
     Studio's `[profile.release] opt-level = 3`. Check ultraroute core has none of its own.
   - One `target/` at the root. Every path that assumed another target directory changes:
     Studio server's `--core` default (`core/target/release/ultraroute`), `serve.sh` files,
     `scripts/route.sh`, `pcbench.py`'s binaries, `tools/route_board.sh`, docs. Find them with
     `git grep -n 'target/release\|target/debug'`. ultraroute agents' habit of separate
     `target-*` dirs (`CARGO_TARGET_DIR`) keeps working; root `.gitignore` ignores `/target*/`
     at every level the projects did.
   - The GPL boundary test (`kicad-pcb/tests/gpl_boundary.rs`) still passes, and no MIT crate
     gains a dependency on `kicad-text`.
3. **npm workspace** at the root: `app/web`, `composer/app/web`, `ultraroute/studio/web`,
   `board-draw`. One `package-lock.json`, one Vite (8.3.4), Svelte (5.57.2), Playwright (1.64.1),
   three.js version, and `.nvmrc` with Node 22.
4. **Licenses.** Root `LICENSE` (MIT) covers the repo. `ultraroute/core/crates/kicad-text/` keeps
   its GPL license file, and the root README says so. Studio crates get `license = "MIT"`.
   pcbview is MIT (owner, 2026-11-08): add the same `LICENSE` text to `pcbview/`, so its mirror
   carries it too.

Acceptance: "Proving nothing changed" with the new workspace build; `cargo test --workspace`
(booked with `reserve -c 12`) passes; the mirror check.

### Stage 5: CI (agent)

Move every workflow to the root `.github/workflows/`, with path filters so a Composer change
doesn't run the router suite:

- `test-rust.yml`: clippy `-D warnings` and `cargo test --workspace --release` (ultraroute's
  `test.yml` as the model, 45-minute timeout), plus `--features ultraroute/gpu` clippy.
- `test-web.yml`: Boardroom's `test:store`, `test:stale`, `test:board`; Composer's Playwright
  specs; Studio build.
- `test-pcbview.yml`: `python3 -m unittest discover pcbview/tests` (browser test skips without
  KiCad, as now).
- `pages.yml`: one site (see stage 7).
- `mirrors.yml`: as under "The mirrors", including the "extracted tree builds alone" check before it
  pushes.

PCBench is not in CI (too heavy, needs KiCad and 26 GB of lab data); it stays a local gate on
either build machine through `reserve`.

### Stage 6: one server (agent)

Boardroom's server serves the whole app on its one port. No other server needs to run.

1. **Studio becomes a library.** `ultraroute/studio/server` exposes its HTTP routes and WebSocket
   as an axum `Router` that Boardroom mounts at `/apps/studio/`. It uses tungstenite today; port
   the handlers to axum's WebSocket, keeping the message format byte for byte (Studio's
   `studio/core/tests/fixtures/easy2350-stream.jsonl` replays must still play). Keep a thin
   `studio-server` binary on the same library so ultraroute work can still run Studio alone
   (**Recommended**).
2. **The router stays a separate process**, as now: Studio starts `ultraroute route --live -`
   and relays its stream. The binary is found next to the `boardroom` binary in `target/`, then
   on `--ultraroute`, then `$ULTRAROUTE`. copperroute stays an optional separate process and is
   never linked.
3. **Composer is static plus CLI.** Boardroom serves `composer/app/dist/` at `/apps/composer/`
   (it is built with `base: './'`). Whatever Composer's pages fetch from its own origin today
   (`api/info`, examples) is served by Boardroom's existing store and snapshot routes or as files.
   Composer's actions that run `ccomp` go through Boardroom's job API (`shell-spec.md`).
4. **Remove the proxies** in `app/server/src/apps.rs` for `composer` and `studio`. Waffle Iron's
   static directory stays as it is.
5. **Headers.** Every page keeps `Cross-Origin-Opener-Policy: same-origin` and
   `Cross-Origin-Embedder-Policy: require-corp`, and Studio's in-tab router still gets
   `SharedArrayBuffer` in Chrome and in `boardroom-gtk` (`JSC_useSharedArrayBuffer=1`).
6. **One build:** `app/build.sh` builds the workspace, the WASM packages, the three web apps and
   the GTK window, and nothing else needs building. `app/serve.sh --port N` runs everything.

Acceptance, with only `app/serve.sh` running:
- `/apps/studio/?project=<easy2350>&board=easy2350&variant=…` opens, routes live, saves back to
  the project, and writes `apps.studio.<board>` in `status.json` (compare with the same flow
  before the stage).
- `/apps/composer/?store=…&board=easy2350` opens easy2350's floorplan and Save writes
  `compose/<board>/placement.json`.
- `npm run smoke`, `test:board` and `boardroom-gtk --snapshot out.png` pass. (Never run the GTK
  window right after a build that may have failed; it opens the old binary's window on the
  owner's screen.)
- `proj doctor` is clean after Claude updates the registry.

### Stage 7: one web app (agent)

**Recommended:** one Vite project with three entry points (Boardroom's shell, Composer, Studio)
sharing one `node_modules`, `board-draw`, `store.js` and a new `packages/suite-ui` (design tokens
and the panels every app repeats: inspector, checks list, jobs list, inbox item, selection-bus
client, as `shell-spec.md` already planned). The shell keeps opening Composer and Studio in tabs
(iframes on one origin) and in their own windows. Merging the three into a single page
application is not part of this spec; see Open questions.

- **Pages:** one site at `sequoia-hope.github.io/boardroom/`, with Composer at `/composer/` and
  Studio at `/studio/` (Studio keeps `coi-serviceworker.js`). The old Pages sites
  (`/ultraroute/`, `/circuit-composer/`) get a last deploy that redirects to the new paths,
  keeping query strings.
- **Deep links** that exist today keep working through redirects: `/apps/studio/studio/`,
  `/app/dist/`, `?project=&board=&variant=`, `&embed=1`.

Acceptance: the stage 6 list again, plus the Pages build run locally (`npm run build:pages` and
the snapshot step) and its three tests.

### Stage 8: docs (agent, then Claude)

- Root `README.md` becomes the map of one repo: what each directory is, how to build, run, test
  and bench, and the board repo layout (unchanged). The suite table keeps Waffle Iron and
  `board-interface` as separate repos.
- Root `CLAUDE.md` covers the whole repo. Each project's rules that were not in a CLAUDE.md
  (ultraroute's benchmark rules from `REMEDIATION.md` ground rules, `docs/pcbench.md`; Composer's
  from its README and skill) go into nested `ultraroute/CLAUDE.md` and `composer/CLAUDE.md`,
  which Claude Code loads when working there.
- `platform-spec.md`, `shell-spec.md`, `suite-spec.md` and `demo-spec.md` are updated where they
  describe separate repos, proxies or pins, with a note pointing here.
- `template/` and `new-board.sh` docs say where pcbview and ultraroute come from now.

## Instructions for the executing agents

Antigravity does not load `CLAUDE.md`, skills or Claude's memory, so every agent prompt starts
with this section and the stage it runs.

**Read first:** this spec; `CLAUDE.md`;
`~/.claude/CLAUDE.md` (ports, `reserve`, the build server); `README.md`; for stages
touching ultraroute, `ultraroute/README.md`, `ultraroute/docs/pcbench.md` and
`ultraroute/core/README.md`; for Composer, `composer/README.md` and `composer/docs/app-spec.md`.

**Rules:**

- Work on the branch named for the stage (`monorepo/stage-N`) in your own worktree. Commit as
  you go. **Never push, never force-push, never merge to main.** Claude audits and merges.
- Never modify the original checkouts (`~/Software/{circuit-composer,pcbview,ultraroute,board-draw}`)
  or their worktrees: clone them. Never delete anything outside your worktree and scratch dir.
- Never edit `~/scripts/launcher.toml`, `~/.config/boardroom/`, `~/.claude/`, GitHub settings or
  secrets. If a step needs one, stop and say so.
- Ports: `--port`, then `$PORT`, then `proj port`, then fail. Never a literal, never an ad-hoc
  `python3 -m http.server <number>`.
- Wrap builds, test suites, benchmarks and routing runs in `~/.claude/bin/reserve -c <n> -l "<label>" -- …`
  (4 for a build, 8–12 for a test suite). Your branch is not on the build server, so pass `--here`
  if `reserve` offers to send the job there. Exit code 75 means the command never ran: report
  it, don't rerun without `reserve`.
- Don't change behaviour while moving things. If a check in "Proving nothing changed" differs,
  stop and report the difference with the outputs; don't "fix" the test.
- Never edit files under `viewer/` by hand; rebuild them.
- Don't touch the GPL boundary: `kicad-text`, copperroute and freerouting are never linked into
  MIT code.

**Report** (written to `scripts/monorepo/stage-N-report.md` and printed at the end): what changed
(commit list), every check in the stage's acceptance list with the command and its output or a
digest, anything skipped and why, anything found that this spec gets wrong.

**Claude's audit** of each stage reruns the acceptance checks itself rather than trusting the
report, reads the diff, and checks the extracted mirror tree builds alone.

## Open questions

Questions 1–3 were answered on 2026-11-08: full history, `main` only, pcbview MIT.

4. **board-draw** folded in (**Recommended**: all three users are now in this repo) or kept as its
   own repo? And `board-interface` stays separate because Waffle Iron will depend on it
   (**Recommended**)?
5. **Old repos.** Mirrors for ultraroute and pcbview, archive circuit-composer and board-draw
   (**Recommended**)? Or archive all four and move board repos to submodules of the whole
   monorepo (heavier: Boardroom's `.git` is 227 MB)?
6. **Toolchain.** If 1.97.1 changes routes, accept after the gate, or keep ultraroute's core on
   1.95.0 in its own workspace?
7. **ultraroute's untracked `REVIEW.md` and `REMEDIATION.md`** (84 KB and 51 KB). Commit them
   under `ultraroute/docs/` at cutover, or leave them out?
8. **One page or three.** Stage 7 keeps Composer and Studio as their own pages in one build. Does
   the owner want them merged into one single-page app later?
9. **Composer's own boards** (`composer/boards/`, 112 MB on disk, earrings and the motor
   controller) stay where they are inside `composer/` (**Recommended** for now), or become
   Boardroom projects like `examples/easy2350`?
10. **proj entries.** After stage 6 nothing serves Studio or Composer alone. Keep the `ultraroute`
    and `circuit-composer` entries for their tmux launchers and agent sessions, pointed at the
    new directories (**Recommended**), or retire them?
11. **The Composer skill in board repos.** Should `new-board.sh` copy
    `.claude/skills/circuit-composer/` into new board repos alongside `pcb-design`?
