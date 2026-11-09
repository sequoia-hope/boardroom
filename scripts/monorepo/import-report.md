# Monorepo Import Report

- **Generated:** `2026-11-08T19:41:11-08:00`
- **Scratch workspace:** `<scratchpad>/stage1`
- **Merged repository:** `<scratchpad>/stage1/boardroom` (branch `monorepo/import`, HEAD `0d898e097e4ab76d3375ac106c59a4685530fed3`)
- **Total commits on `monorepo/import`:** 635
- **Total tags in merged repo:** 0
- **`board-draw` included:** no (Stage 1 default; supported via `--with-board-draw`)

## 1. Source and Imported Heads

| Project | Target Dir | Source `main` SHA | `origin/main` SHA | Local vs Remote | Source Commits | Source Tags (`main`) | Filtered Head SHA | Merge Commit on `monorepo/import` | Imported Tags |
|---|---|---|---|---|---:|---:|---|---|---:|
| `boardroom` | `.` (root) | `48a41a519e749caeeabae6c404c092cf8d57f84d` | `48a41a519e749caeeabae6c404c092cf8d57f84d` | in sync | 77 | 0 | *(base branch)* | `48a41a519e749caeeabae6c404c092cf8d57f84d` *(initial base)* | 0 |
| `circuit-composer` | `composer/` | `bff8c19b9c8ae2fede9b159047cbb2d2b59f4e20` | `7ee8bfce71e852efa201b45d9de52e1e834173ad` | **+3 ahead** / -0 | 87 | 0 | `9b60e60153a3ae98f336a96f24af0b8515cfc237` | `3e974972bc71127e6796591c7cd0c038ba1d568a` | 0 |
| `pcbview` | `pcbview/` | `26ba32bb8edbc5761d84e3238a312a29cbcc1aca` | `26ba32bb8edbc5761d84e3238a312a29cbcc1aca` | in sync | 9 | 0 | `d5786510cb16add5a3eee7f03e8a42ae135fc70a` | `ad20f27dd020c6d0fdf07b1add34f45949301df0` | 0 |
| `ultraroute` | `ultraroute/` | `13947cfb50601ebca4ffc590e4e346f2f9d33944` | `13947cfb50601ebca4ffc590e4e346f2f9d33944` | in sync | 459 | 0 | `5707e787871d7e4a3303f4434b9a3017c880941f` | `0d898e097e4ab76d3375ac106c59a4685530fed3` | 0 |
| `board-draw` *(not merged in Stage 1)* | `board-draw/` | `b45cc3f689dc7b5086be9dada3dd64381a3d010e` | `b45cc3f689dc7b5086be9dada3dd64381a3d010e` | in sync | 3 | 0 | *(skipped without `--with-board-draw`)* | *(skipped)* | 0 |

### Local `main` ahead of `origin/main` (included via local `--no-local` clone)

- **`circuit-composer`**: local `main` (`bff8c19`) is **3 commit(s) ahead** of `origin/main` (`7ee8bfc`):
  - `bff8c19 Power contracts: cited power_in and power_out figures the supply rule can match`
  - `2e8f5ac Spec: supply selection, the Supplies panel and the Library workspace`
  - `82cd03e compose_kicad.py names its Boardroom checkout; the skill says what compile's widths mean`

## 2. Unmerged Branches (Not Imported)

Full machine-readable list written to `~/Software/boardroom/.claude/worktrees/monorepo-stage-1/scripts/monorepo/branches.txt`:

| Source Repo | Unmerged Local Branches | Unmerged Remote Branches | Unmerged Detached Worktrees |
|---|---:|---:|---:|
| `boardroom` | 0 | 0 | 0 |
| `circuit-composer` | 0 | 0 | 0 |
| `pcbview` | 0 | 0 | 0 |
| `ultraroute` | 20 | 10 | 0 |
| `board-draw` | 0 | 0 | 0 |

### `ultraroute` unmerged refs

| Ref | Commit | Last Commit Date | Checked-out Worktree |
|---|---|---|---|
| `local:agy-bluebuzz` | `84433d7` | `2026-10-26 10:24:11 -0700` | — |
| `local:agy-webgpu` | `f779408` | `2026-10-25 15:57:21 -0700` | — |
| `local:boxed-prerebase-7dd54b7` | `7dd54b7` | `2026-11-03 06:23:12 -0800` | — |
| `local:explore/escape-vias` | `93d5021` | `2026-10-28 00:42:46 -0700` | — |
| `local:explore/narrow` | `efea627` | `2026-11-03 02:27:22 -0800` | — |
| `local:explore/restarts` | `286bd2f` | `2026-11-05 14:39:42 -0800` | `~/Software/ultraroute/.claude/worktrees/explore-restarts` |
| `local:explore/text` | `d2b9b4f` | `2026-11-03 06:18:42 -0800` | — |
| `local:explore/text-noany` | `432ddbe` | `2026-11-03 11:57:40 -0800` | — |
| `local:explore/text-nobridge` | `774cfee` | `2026-11-03 12:50:59 -0800` | — |
| `local:feat/square-bundles` | `43601c2` | `2026-11-08 16:21:14 -0800` | `~/Software/ultraroute/.claude/worktrees/square-both` |
| `local:fix/cap-joints` | `e42caf1` | `2026-11-01 07:30:08 -0800` | — |
| `local:fix/crowded-pads` | `6d165d7` | `2026-11-05 11:05:27 -0800` | `~/Software/ultraroute/.claude/worktrees/crowded` |
| `local:fix/pad-entry-depth` | `d1f4658` | `2026-11-02 01:40:36 -0800` | — |
| `local:fix/qfn-escape` | `0a47a9e` | `2026-11-06 19:17:37 -0800` | `~/Software/ultraroute/.claude/worktrees/fine-pitch-fanout` |
| `local:fix/stagger` | `7f0c638` | `2026-11-07 10:53:03 -0800` | `~/Software/ultraroute/.claude/worktrees/stagger` |
| `local:kicad/5-lengths` | `e3d5cce` | `2026-10-31 12:30:34 -0700` | — |
| `local:merge/webgpu` | `d3625b7` | `2026-10-26 14:53:45 -0700` | — |
| `local:remed/12-global` | `1b6660a` | `2026-10-30 04:07:31 -0700` | — |
| `local:worktree-agent-a37a0c195ba85c5b6` | `b089393` | `2026-10-24 21:49:23 -0700` | — |
| `local:worktree-agent-a756c8162f4281e4a` | `6928e93` | `2026-10-24 21:03:10 -0700` | — |
| `remote:origin/explore/narrow` | `efea627` | `2026-11-03 02:27:22 -0800` | — |
| `remote:origin/explore/restarts` | `286bd2f` | `2026-11-05 14:39:42 -0800` | — |
| `remote:origin/feat/square-bundles` | `43601c2` | `2026-11-08 16:21:14 -0800` | — |
| `remote:origin/fix/cap-joints` | `e42caf1` | `2026-11-01 07:30:08 -0800` | — |
| `remote:origin/fix/crowded-pads` | `6d165d7` | `2026-11-05 11:05:27 -0800` | — |
| `remote:origin/fix/pad-entry-depth` | `d1f4658` | `2026-11-02 01:40:36 -0800` | — |
| `remote:origin/fix/qfn-escape` | `0a47a9e` | `2026-11-06 19:17:37 -0800` | — |
| `remote:origin/fix/stagger` | `7f0c638` | `2026-11-07 10:53:03 -0800` | — |
| `remote:origin/kicad/5-lengths` | `27c0039` | `2026-10-31 12:03:33 -0700` | — |
| `remote:origin/remed/12-global` | `1b6660a` | `2026-10-30 04:07:31 -0700` | — |

## 3. Files and Blobs Over 5 MB (`git filter-repo --analyze`)

Threshold: 5 MB (5,000,000 bytes / 4.77 MiB). Inspected via `git filter-repo --analyze` on each project's `main` clone.

### `boardroom`

- Currently tracked single files at `HEAD` >= 5 MB: **none**.

**Paths with cumulative unpacked history >= 5 MB across commits (`path-all-sizes.txt`):**

| Path | Cumulative Unpacked (MB) | Cumulative Packed (MB) | Status at `HEAD` |
|---|---:|---:|---|
| `examples/easy2350/design/renders/top.png` | 22.91 | 22.85 | present |
| `examples/easy2350/design/renders/bottom.png` | 22.78 | 22.76 | present |
| `examples/easy2350/design/renders/F.Cu.png` | 18.40 | 18.35 | present |
| `examples/easy2350/design/renders/B.Cu.png` | 17.92 | 17.88 | present |
| `examples/easy2350/design/renders/In2.Cu.png` | 13.15 | 13.06 | present |
| `examples/easy2350/viewer/easy2350-a/3d/board.glb` | 32.55 | 11.44 | present |
| `examples/easy2350/design/renders/In1.Cu.png` | 10.98 | 10.87 | present |
| `examples/easy2350/design/renders/3d-bottom.png` | 7.27 | 7.27 | present |
| `examples/easy2350/design/renders/assembly-top.png` | 29.64 | 5.84 | present |
| `examples/easy2350/design/renders/3d-top.png` | 5.77 | 5.77 | present |
| `examples/easy2350/hw/easy2350/a/easy2350.kicad_pcb` | 19.35 | 2.61 | present |
| `examples/easy2350/hw/easy2350/a/fab/easy2350.step` | 24.39 | 1.39 | present |
| `examples/easy2350/viewer/easy2350-a/sch/easy2350.svg` | 30.56 | 0.33 | present |
| `examples/easy2350/viewer/easy2350-a/sch/easy2350.pdf` | 6.05 | 0.29 | present |

### `circuit-composer`

- None (no single blob or cumulative path history >= 5 MB).

### `pcbview`

- None (no single blob or cumulative path history >= 5 MB).

### `ultraroute`

- Currently tracked single files at `HEAD` >= 5 MB: **none**.

**Paths with cumulative unpacked history >= 5 MB across commits (`path-all-sizes.txt`):**

| Path | Cumulative Unpacked (MB) | Cumulative Packed (MB) | Status at `HEAD` |
|---|---:|---:|---|
| `jobs/earring-locked-routed/input/earring.kicad_pcb` | 6.32 | 1.18 | present |
| `jobs/bluebuzz-routed/input/bluebuzz-rm2.kicad_pcb` | 6.04 | 1.14 | deleted (2026-11-06) |
| `jobs/earring-routed/input/earring.kicad_pcb` | 6.15 | 0.99 | present |
| `core/crates/ultraroute/src/lib.rs` | 10.62 | 0.12 | present |

## 4. Stage 1 Acceptance Checks

### Check 1: Mirror extraction reproduces exact original `main` SHA

Extracted from `monorepo/import` with `git clone --no-local <monorepo> x && git -C x filter-repo --force --preserve-commit-hashes --subdirectory-filter <dir>`:

| Directory | Source Repo | Source `main` SHA | Extracted `HEAD` SHA | Result |
|---|---|---|---|---|
| `composer/` | `circuit-composer` | `bff8c19b9c8ae2fede9b159047cbb2d2b59f4e20` | `bff8c19b9c8ae2fede9b159047cbb2d2b59f4e20` | **PASS** |
| `pcbview/` | `pcbview` | `26ba32bb8edbc5761d84e3238a312a29cbcc1aca` | `26ba32bb8edbc5761d84e3238a312a29cbcc1aca` | **PASS** |
| `ultraroute/` | `ultraroute` | `13947cfb50601ebca4ffc590e4e346f2f9d33944` | `13947cfb50601ebca4ffc590e4e346f2f9d33944` | **PASS** |

### Check 2: Only branch is `monorepo/import` (`git branch -a`)

```text
$ git -C <scratchpad>/stage1/boardroom branch -a
* monorepo/import
```

- Result: **PASS** (no other local or remote/imported branches).

### Check 3: `git log --follow` on three sample files per project reaches first commit

| Project | Path in Monorepo | Commits (Source / Monorepo) | Source First SHA | Monorepo First SHA | First Commit Date & Subject | Result |
|---|---|---|---|---|---|---|
| `boardroom` | `README.md` | 10 / 10 | `9aff8c2091` | `9aff8c2091` | `2026-10-22` — easy_pcb: a board project template with the pcb-design skill and pcbview | **PASS** |
| `boardroom` | `.claude/skills/pcb-design/SKILL.md` | 3 / 3 | `9aff8c2091` | `9aff8c2091` | `2026-10-22` — easy_pcb: a board project template with the pcb-design skill and pcbview | **PASS** |
| `boardroom` | `app/server/src/main.rs` | 9 / 9 | `69aaa52f40` | `69aaa52f40` | `2026-10-29` — Boardroom app, milestone 1: project list and overview | **PASS** |
| `circuit-composer` | `composer/README.md` | 31 / 31 | `a1d4dcd19b` | `b7275512b8` | `2026-10-27` — Milestone 1: intent schema v0, validator, ccomp CLI; easy2350 annotated | **PASS** |
| `circuit-composer` | `composer/Cargo.toml` | 3 / 3 | `a1d4dcd19b` | `b7275512b8` | `2026-10-27` — Milestone 1: intent schema v0, validator, ccomp CLI; easy2350 annotated | **PASS** |
| `circuit-composer` | `composer/crates/ccomp/src/main.rs` | 22 / 22 | `a1d4dcd19b` | `b7275512b8` | `2026-10-27` — Milestone 1: intent schema v0, validator, ccomp CLI; easy2350 annotated | **PASS** |
| `pcbview` | `pcbview/README.md` | 5 / 5 | `98e7f57cc9` | `9ee64fbd13` | `2026-09-28` — pcbview: KiCad boards on the web, split out of servodrive's project pages | **PASS** |
| `pcbview` | `pcbview/pcbview/site.py` | 5 / 5 | `98e7f57cc9` | `9ee64fbd13` | `2026-09-28` — pcbview: KiCad boards on the web, split out of servodrive's project pages | **PASS** |
| `pcbview` | `pcbview/web/app.js` | 2 / 2 | `98e7f57cc9` | `9ee64fbd13` | `2026-09-28` — pcbview: KiCad boards on the web, split out of servodrive's project pages | **PASS** |
| `ultraroute` | `ultraroute/README.md` | 7 / 7 | `78c0fcb132` | `126f0a4ca9` | `2026-10-22` — Start the ultraroute workspace repo: brief, status page, route-job tools, bluebuzz job | **PASS** |
| `ultraroute` | `ultraroute/core/crates/ultraroute/src/main.rs` | 24 / 24 | `510678f6e0` | `b62919a3fd` | `2026-10-24` — Phase 2c step 3c: the ultraroute command and the Core router in the studio | **PASS** |
| `ultraroute` | `ultraroute/tools/pcbench.py` | 26 / 26 | `42f9c71c5f` | `68cff72f33` | `2026-10-26` — Core: electrical validity from KiCad's own rules; soft via-avoid areas; PCBench harness | **PASS** |

### Check 4: Nothing from the "never imported" list is in the tree

```text
$ git -C <scratchpad>/stage1/boardroom ls-files | grep -E 'copperroute|PCBench|/target'
(empty — 0 matches)
```

- Total tracked entries in merged index (`git ls-files | wc -l`): `1322`
- Result: **PASS**.

### Check 5: Boardroom's own files are unchanged

```text
$ git -C <scratchpad>/stage1/boardroom diff 48a41a5 monorepo/import -- . ':!composer' ':!pcbview' ':!ultraroute'
(empty — 0 lines of diff)
```

- Result: **PASS**.
