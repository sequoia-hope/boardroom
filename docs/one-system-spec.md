# One system: Boardroom as one program with many crates — spec

Status: 2026-11-12. Agreed between the Boardroom, UltraRoute and Circuit Composer sessions. Nothing here is built. It merges UltraRoute's draft
(`ultraroute/docs/spec/one-system.md`, f558eb6) and Boardroom's, and is the one spec. It
**replaces** `monorepo-spec.md` (2026-11-08) in approach; that spec's inventory of what exists is
still accurate. Why and how the router is rebuilt is in `ultraroute/docs/spec/from-zero.md`, which
moves here when the router crates exist.

## Decided by the owner (2026-11-12)

- The goal is cleanliness. Boardroom is **one project with many crates** and **one program**. No
  directory is shaped like a former repo (`composer/`, `ultraroute/`).
- **No old git history comes in.** Code from the other repos appears here as new files. Boardroom
  keeps its own history and is restructured in place. The other repos stay where they are as the
  record of theirs.
- **UltraRoute is rewritten from zero.** The current router is frozen in its own repo as a
  baseline the benchmark can run. None of its code has to survive.
- **Three modes of one app.** **Boardroom** is the file manager. **Circuit Composer** is a view
  inside it that can also pop out as its own window: a board is built from blocks, its outline
  decided, its placement solved. **UltraRoute** takes that board and closes all the open copper.
- **Some pieces are modules to be used alone.** The router must run against someone's own board
  without the rest of the flow.
- **Nothing has to keep working during the migration.** There are no users; all board work is
  paused until the migration is done and a router that works exists. No byte-identical outputs.
- **One binary**, `boardroom`, with `compose`, `route`, `check` and the rest as commands.
- **pcbview comes in** as the `view` command. Boardroom's copy is the standard and the old repo is
  deprecated. Everything board-related (tools, experiments, benchmarks) runs from Boardroom.
- **Every stage can be seen** (below).
- Composer's eight boards become projects, not source. UltraRoute's public replay site stays on
  the old repo as a record. Existing board repos stay on their old pins until the new router is
  ready.
- Claude does the migration, and then writes the new router, in the Boardroom session; no
  Antigravity. That session's context will be cleared along the way, so this spec, the handoff
  notes and the merged memories are what it works from. All sessions agree on this
  spec before the UltraRoute and Composer sessions close.

## What exists today (surveyed 2026-11-12)

- Composer reads boards through ultraroute's `kicad-pcb` (git dependency at 0e581c4), so there is
  one board reader already. Its `pcb` crate is a second in-memory board model on top of it.
- Composer's `intent` is the schema of `intent.json` (per net: class, voltage, current, noise,
  bandwidth, impedance, match group, maximum length, each with strength and provenance; blocks
  with roles, domains, regions, protection paths). `recognise` drafts it from a netlist or a
  board's pads. `verify` measures loops, Kelvin sense, protection, return path, decoupling.
- Composer has no schematic reader, and nearly all its board writing is Python through pcbnew.
  `compile` turns `intent.json` into `routing.json` and `.kicad_dru`.
- Boardroom links none of the others. It proxies Composer's and Studio's servers
  (`app/server/src/apps.rs`), vendors ultraroute and pcbview as submodules, and sparse-clones
  ultraroute a second time (ce50b64, `app/web/scripts/draw.sh`) to build the WASM its Board tab
  draws with, together with `board-draw` (pinned d37b1f8). Its server is one 12,000-line crate.
- Today's board view goes board → stream lines → scene, through the old router's driver crate.
  `studio/core` (about 1,100 lines) never sees a `Board`. board-draw's meshing and picking are
  cleanly separate.
- Sizes (lines of Rust): Composer 29,600 in 13 crates and `app/core`; ultraroute `kicad-pcb`
  7,900 and `board` 7,200 (coming), `grid`, `ultraroute`, `morph`, `topo` 46,800 (not coming);
  board-draw 2,100; pcbview 2,700 of Python and 1,500 of JS.

## The layout

One Cargo workspace, one npm workspace, one toolchain pin (1.97.1, as board-interface and Waffle
Iron use). Crates are named for what they do.

```
boardroom/
  Cargo.toml  rust-toolchain.toml  package.json
  crates/
    # Foundation: files and the board
    kicad/        the only reader and writer of KiCad files: board, project, rules (.kicad_dru,
                  net classes), netlists; schematics (current and legacy) when added
    board/        the one in-memory board: items, nets, layers, rules, geometry, copper per net
    check/        the rule checker that agrees with KiCad
    shape/        the board outline document (uses board-interface)
    scene/        plain drawing data: primitives, layers, colours; no dependencies

    # Understanding: shared by Composer and UltraRoute
    intent/       intent.json; extended with the layer plan, frames by region and repeats
    recognise/    drafts intent from a netlist, a schematic or a board; from a finished board
                  recovers what its designer chose
    library/  interfaces/      cited claims, typed interfaces

    # Judging
    examine/      the two scoreboards: like the designers, and sound

    # Composer: blocks, outline, placement
    compose/  pinsolve/  floorplan/  blockgen/

    # UltraRoute: the open copper (all new)
    plan/         the whole-board plan: which gaps each connection passes, in what order
    wire/         wires, frames, the wire fit, one writer
    route/        plan, repeats, wire, check, as one call
    reshape/      change a routed board and keep it routed (open, see below)

    # The program
    render/       scene to mesh, picking, the WebGL2 shader; view(board) -> scene; its JS package
    project/      a board project on disk: boardroom.toml, status.json, gates, staleness, versions
    server/       HTTP, jobs, terminals, hosted git; calls the crates above
    boardroom/    the binary, kept thin: every command is a function in a library crate
    wasm/         one WASM crate for the browser: render, the live floorplanner
    gtk/          boardroom-gtk, the native window (not a default workspace member)
  web/            one Svelte app: Projects, Compose, Route
  view/           pcbview's Python and web files, behind `boardroom view`
  bench/          the corpus harness, the look set, the scoreboards
  packages/       Composer's chip packages
  library/        Composer's research data, beside packages/ (crates find it as packages/../library)
  gpl/kicad-text/ GPL text outlines: its own binary, run as a process, never linked
  skills/         pcb-design and circuit-composer (linked from .claude/skills/)
  template/  examples/  docs/
```

### Dependency rules

- A crate may depend only on crates in groups above it, and:
- **`plan`, `wire`, `route`, `reshape` and `examine` depend only on Foundation and
  Understanding.** Never on a Composer crate, `render`, `project`, the server or the web app. This
  is what makes the router usable alone, and lets `examine` score any board, a designer's from the
  corpus included.
- **The generators stay independent of the judge.** `blockgen` does not depend on `examine`: a
  generator that ranked with the verifier would search out its blind spots and then be signed off
  by them. The role types `blockgen` takes from Composer's `verify` today (`BuckRoles`) move into
  `intent`; `blockgen` keeps ranking with its own estimate.
- `board` does not know how it is drawn. Stage crates may emit `scene` data (that is how each
  offers a picture without depending on `render`); `view(board)` lives in `render`.
- `boardroom route board.kicad_pcb` works on a bare board with no project and no `intent.json`:
  the router calls `recognise` for roles when there is no intent.
- A workspace test reads `cargo metadata` and fails when any of this is broken.
- `board-interface` stays its own repo and a tagged git dependency (v0.2.2 today), because Waffle
  Iron uses it. The GPL boundary holds: `kicad-text` is a process, never linked.

### What comes from where

| Crate | From | Notes |
|---|---|---|
| `kicad` | ultraroute `kicad-pcb`, all of it; Composer's netlist reader | gains writing of `.kicad_dru` and net classes from intent (`dru.rs`, `rules.rs` to build on) |
| `board` | ultraroute `board`: `geom`, `model` (with `Block`, a KiCad group kept whole, which is how locked power blocks arrive), `custom`, `fill`, plain connectivity from `connect`; `ZoneArea` (the board's pours) moved from `regions` into the model; Composer `pcb` (1,100 lines) merged in as a module that keeps its API (parts, pads with nets, `pad("REF.PIN")`, the copper-per-net graph) on top of board's items, so its 49 call sites move unchanged at first | left behind: `work` (and the calls into it from `connect`), `probe`, `spokes`, the rest of `regions` (`Region`, `ClearUnder`, `Antipad` and the model's fields of those types), and the joint notions in `connect` that served the old repair passes |
| `check` | ultraroute `board::check` (665 lines) | drop its one call into `regions` and five into `work` |
| `scene`, `render` | board-draw; a new direct board → scene function | `studio/core` is not ported: its scene is built from the old router's stream |
| `shape`, `intent`, `recognise`, `library`, `interfaces`, `compose`, `pinsolve`, `floorplan`, `blockgen` | Composer | `ccomp`'s commands become `boardroom compose …` |
| `examine` | Composer `verify`; from ultraroute `tools/`: the style features of `tidy.py`, `dfm.py` (acute angles, same-net slots), `designer.py` (hot loops on a routed board), `netcheck.py` (whether a designer routes a net within the rules); new physical measures | the design of the "sound" scoreboard is `ultraroute/docs/spec/verification.md`, which moves here with `from-zero.md` |
| `plan`, `wire`, `route`, `reshape` | written here | `from-zero.md` |
| board-reading fixes in Composer `pcb` | into `kicad` and `board`, with their tests | the exposed-mask `(layers "B.Cu" "B.Mask")` rewrite, paste-only pads, KiCad 10 zone net names (these three are not in ultraroute's `kicad-pcb`), and the stacked-via join |
| what `compile` did | Composer `compile`, split up, each piece with its tests | `merge_dru` and diff-pair skew rules → `kicad`; `ipc2221_width_mm` and "`min_mm` only from a trusted figure" → the router's reading of intent; regions derived from package constraints and the board's geometry (`via_count <= 0`, `unreferenced_length_mm`: layers with no ground fill beside them, rectangles over voids in the ground fill) → `route`, or a function in `examine` or `check` that `route` calls; `check_regions` → `check` or `examine`. History, not a rule: Composer sent voids as small rectangles because the old router's per-cell region raster stalled on 683-vertex polygons; in the new router a void is an obstacle or priced area in the plan's gap graph, and needing polygons simplified would be a router defect |
| Python kept from Composer `tools/` | under `packages/tools/` and `bench/` | must come: `import_altium` (pcbnew's importer), `library_fetch`, `library_import`, `library_quotes`, package authoring (`circuit_check`, `package_circuit`, `package_nets`, `kicad_pins`, `rp2350_mux`), `extract_layout`, `lift_layout`, `compose_from_netlist`; into `bench/`: `handcheck`, `fp_bench`, `bench_buck`, `compare_buck`, `pcbgolf_score`; kept until Rust covers them: `compose_kicad` (schematics, through the skill's `kicad_write.py`), `verify_routed`. Replaced by `kicad`: `apply_layout`, `apply_placement`, `prepare_route`. Gone: `route_board.sh` |
| `project`, `server`, `boardroom`, `gtk` | Boardroom `app/server`, `app/gtk` | `project` is the project model out of the server, so commands work without HTTP |
| `wasm` | Composer `app/core` | |
| `view/` | pcbview | Python as it is at first |
| `bench/` | ultraroute `tools/`: `pcbench.py` with its tiers, timeouts and variants JSON and `variants/` patches, `apply_variant.py`, `rulebench.py`, `refill_zones.py`, `lookset.py` and `lookset.json`, `scoreboard.py`, `pcbench_schematics.py`, `lab.py` | the lab (`ultraroute-lab`) stays a directory beside the repo |

Not coming: ultraroute's `grid`, `ultraroute`, `morph`, `topo`, `lab-films`, `web`, `studio`,
`legacy/`, `jobs/`, logs, and `tools/{failview,replays_index,search,shuffle_check,add_block}.py`; Composer's `compile` as a crate and its file format, `pcb` as a separate model, `boards/` (178 MB),
`library/cache/`; every Python tool the Rust `kicad` writer replaces; `sequoia-hope/board-draw` as
a separate repo (archived).

**Data that must never enter the repository:** the PCBench corpus (`ultraroute/PCBench/`; many
boards are unlicensed and marked not redistributable) and the schematics fetched for it
(`ultraroute-lab/pcbench/schematics/`). The harness finds both relative to the ultraroute checkout
today. In Boardroom they are two configured paths, and the harness fails loudly when either is
unset, as with ports.

Known limits carried over, to be in UltraRoute's handoff note: `check` covers clearance, edge,
hole, hole-to-hole and keepouts; necks, connection width and most solder-mask checks are judged
only by `kicad-cli`. Item order in the file feeds `ItemId`. The model has no wire or plan. `fill` (zone fill) is ours,
not KiCad's, which is why the benchmark judge refills zones through KiCad before DRC.

## What changes in kind

- **The seam between Composer and the router is `intent.json` and the board, nothing else.** The
  router reads intent itself. `compile`, `routing.json` and `--regions` go away; regions return
  through intent, except those that need the board's geometry, which the router derives (see the
  table). Block roles for hot loops are already in intent (`Block.roles`). `Block` gains a `group`
  field naming the KiCad group that holds its locked layout (today `<block type>_<anchor>`), so
  the router keeps that copper and scores it by roles without matching groups by part sets. This
  is unproven end to end: the committed routed rp2350-battery board has 65 locked items and no
  `(group …)`, and group names (`core_reg`), intent block ids (`mcu/core_reg`) and recognise's
  names (`<type>_<inductor>`) do not match today. The `kicad` writer needs a test that groups
  survive a read and write, and one naming has to be chosen.
- **Rules still reach KiCad.** `kicad` writes `.kicad_dru` and net classes from intent, called by
  `compose` and `check`, because `kicad-cli` DRC stays the outside judge and the boards are opened
  in KiCad.
- **Recognition is one crate used three ways:** to draft intent for a new design, to give the
  router roles on a board that has none, and to read a designer's finished board for the
  scoreboard.
- **Board writing moves to Rust** (`kicad`): applying a block, a placement or a route is one call,
  not a pcbnew script.
- **The server calls functions.** No proxies, no subprocesses for our own tools, no second clone.
- **Every action is a function first,** then a CLI command and an HTTP route over it, so a person
  in the window and Claude in a terminal drive the same code, and tests call the function.

## The program

- **One server, one port** (`--port`, `$PORT`, `proj port`, else fail). Waffle Iron's static
  directory is the one outside app still served.
- **One web app, three modes,** one origin, one build:
  - **Projects** is today's Boardroom window: the project list, pipeline and gates, staleness,
    versions, docs, renders, terminals, the board view.
  - **Compose** holds Composer's workspaces (Circuit, Board, Shape, Copper, Rules) and its checks.
  - **Route** holds the router's stages: understanding, plan, copper, and the scoreboards.
- **A mode is a route in the app, so popping out is free:** `/p/<project>/compose` in the main
  window, the same URL with `?window=1` in a new one (no project list around it). In the GTK shell
  that is a second WebKit window on the same server; in a browser, `window.open`. Windows on one
  project stay in step through the server's event stream, and selection crosses windows on a
  `BroadcastChannel`.
- **The viewer.** pcbview is `boardroom view`. Its Python lives under `view/` and is run by the
  binary at first; its web part joins the npm workspace. At cutover the `pcb-design` skill, the
  board projects' view scripts and anything else that names the old pcbview folder are pointed
  here. The Board tab (`render`) and pcbview are two board views; making them one is later work.
- **The pcb-design skill** stays as it is (5,800 lines of Python); its calls to `scripts/*.sh`
  become `boardroom` commands.

## Every stage can be seen

The owner asked that what exists before copper is laid be kept, be something a board can be
changed in, and be drawn, "so I can see it and so you can see it so you can debug it". Each
stage's result is a saved file with a picture (as `scene` data), in the app and as static pages in
the look set, and looking at those pictures is part of every review:

- **Understanding:** nets coloured by role, blocks outlined, repeats marked with their cell, each
  region's frame drawn as its axes, the layer plan (plane or signal, and which way).
- **Plan:** the gaps between obstacles as a graph, each gap with its room and how much is used
  (full gaps in red), every connection as a strand through its gaps in order: the board as a
  rubber-band sketch, before any copper.
- **Copper:** the drawn wires, each traceable back to its strand.
- **Differences:** the same three for the designer's board beside ours, and before and after a
  reshape.

The plan is kept with the board (`route/<board>/plan.json`) and is what reshaping works on.

## Reshaping (open: the owner has not settled where it sits)

Making room for a mounting hole, changing an outline, squeezing a routed board. Proposed: a crate
`reshape` beside the router's, and a Reshape tool usable from Compose and from Route. A reshape is
not a repair pass: the board keeps its plan, the geometry changes, the copper is drawn again from
the same plan, and only wires whose gap no longer has room are planned again. Today's `morph` is
not brought over; what it can do is the list of operations `reshape` must offer.

## Projects: where boards live

- **A project is a folder with a `boardroom.toml` at its root.** A whole repo or a folder in one;
  it can hold several boards (`hw/<board>/<variant>/`). The file names the boards and the Boardroom
  version the project was last built with. It replaces `pcbview.toml` and scattered per-tool
  settings over time.
- **The machine's list is `~/.config/boardroom/projects.toml`,** managed by `boardroom add`,
  `remove`, `list`. A project never has to be inside this repo.
- **A project holds data, not tools.** No submodules, no copied scripts. It uses the installed
  `boardroom` and records its version; a small script in the project installs it when missing
  (tidiness of that has low priority). `scripts/{new-board,doctor,route,view}.sh` become
  `boardroom new`, `doctor`, `route`, `view`.
- **Every project carries its view as output:** the static `viewer/` that `boardroom view` builds
  is committed, so the board can be looked at on GitHub or Pages with nothing installed.
- **Each mode writes only its own folder:** KiCad files in `hw/`, Composer's in `compose/`, the
  router's in `route/`, the shared outline in `interface/`, the design record in `design/`. KiCad
  files remain the shared truth.
- **Sample boards:** a few small ones in `examples/` for tests and demos. Real boards used as
  samples are listed in `examples.toml` by repo URL and revision and cloned on demand into a cache.
- **Composer's eight boards:** two are designs, bolt-earring and flame-earring, which become
  projects in their own repos (a small fixture is cut from bolt-earring first, since `pinsolve`
  and `shape` tests read it). rp2350-battery and rp2350-matrix are fixtures and go to `examples/`.
  `examples/easy2350` cannot serve Composer's easy2350 tests as it stands (no intent, composition,
  floorplan, binding, shape or netlist files, and its board is a later re-route with 16 of 58
  parts locked), so Composer's files and its own pinned board come in under
  `examples/easy2350/compose/easy2350/`. rp2350-flight-controllers, rp2350-motor-controller and
  pcbgolf are mined results and a challenge board: evidence for packages, kept under `bench/` or a
  mining area, not projects. Boards and files from others are read in place, never copied in.
- Existing board repos (round-pendant, simple-earring, robot-link) keep their old pins and scripts
  until the new router is ready; nothing rewrites them.

## What "done" means for a step

The bar is on what has arrived, not on what was there before:

- A crate that has come in builds, its own tests come with it and pass, clippy is clean. (On
  arrival `interfaces` has 3 clippy warnings and `pinsolve` 6; fixing them is part of bringing
  them in.)
- `check` still agrees with `kicad-cli` DRC on the boards its tests cover.
- The dependency test passes.
- `main` builds at every commit. App features may be missing or broken mid-migration; each step's
  commit message says what is broken. No long-lived branch.
- At the end of step 5: easy2350 composes, floorplans and passes its verifier; the Boardroom and
  Composer web tests, `npm run smoke` and `boardroom-gtk --snapshot` pass; `proj doctor` is clean.

Heavy builds and suites go through `~/.claude/bin/reserve`.

## Order

0. **Clear the ground.** This spec is committed; `monorepo-spec.md` is marked superseded;
   Boardroom's uncommitted files are committed. `origin/main` is two commits ahead of local `main`
   (the unused stage-1 import script and reports under `scripts/monorepo/`): pull, then remove
   that directory.
1. **Before the other sessions close** (below).
2. **Workspace.** Root workspace and toolchain pin. `app/server` becomes
   `crates/{project,server,boardroom}`, `app/gtk` becomes `crates/gtk`, `app/web` becomes `web/`.
   pcbview comes in as `view/`. The Studio and Composer proxies, both `vendor/` submodules and
   `draw.sh` are removed. Boardroom cannot route or draw a board until later steps; accepted.
3. **Foundation.** `kicad`, `board`, `check`, `gpl/kicad-text`, `scene`, then `render` with a
   direct board → scene function; the Board tab is rebuilt on it.
4. **Understanding and judging.** `intent`, `recognise`, `library`, `interfaces`, `examine`,
   `bench/`. This comes before the rest of Composer because the ground-truth work of
   `from-zero.md` (designers' pours restored, schematics, designer extraction, the two
   scoreboards) is the first work after the foundation and needs `intent` and `recognise`. The
   frozen router the bench compares against is a pinned binary by path
   (`ultraroute-lab/bin/<name>/ultraroute`, with `kicad-text` beside it); the old router is never
   built inside Boardroom. Whoever writes the new router should also do this step, since it
   defines what the router is judged by.
5. **Composer.** `shape`, `compose`, `pinsolve`, `floorplan`, `blockgen`; `boardroom compose …`;
   the Compose mode; `wasm` with the live floorplanner; `packages/` and the Composer skill; Python
   board writers replaced as `kicad` learns to write.
6. **UltraRoute.** `plan`, `wire`, `route` written here from zero; the Route mode; then `reshape`.
7. **Projects and retirement.** `boardroom.toml`, `boardroom new` and friends, the template
   without submodules; `sequoia-hope/circuit-composer`, `board-draw` and `pcbview` archived with a
   pointer; `sequoia-hope/ultraroute` stays as the frozen baseline; `proj` entries for
   `circuit-composer` and `pcbview` retired, `ultraroute` kept while the baseline is in use.

Steps 4 and 5 can overlap. Step 6 starts as soon as step 4's scoreboards exist.

## Before the other sessions close

Each of the UltraRoute and Composer sessions, before it ends:

1. Commits or deliberately discards everything uncommitted and untracked, and says which.
   Done. UltraRoute (d98e953): its review and remediation plan and two research notes are
   committed; `jobs/earring-v2a/` is left untracked on purpose, to become a Boardroom project;
   `jobs/motionhexa-unified/` is another person's board and is never committed anywhere.
   Composer's tree is clean and pushed at c558971, `main` only.
2. Lists its branches and worktrees holding work not on `main`, one line each on whether it
   matters. (UltraRoute has 13 local branches; the tips of 131 deleted on 2026-11-09 are in
   `ultraroute-lab/deleted-branches-2026-11-09.txt`.)
3. Writes a handoff note in its `docs/` for each crate that moves: what is solid, what is known
   wrong, what no test covers, what it would do differently in the move. For UltraRoute, also the
   from-zero state.
4. Says which of its Claude memories are no longer true. All of them were merged into Boardroom's
   memory on 2026-11-12 (owner's decision; prefixed `ur-` and `cc-`); the Boardroom session copies
   again at cutover to pick up later changes, then removes the ones named as stale.
   Done: UltraRoute's are `ultraroute/docs/handoff/` (`router-brief.md`, `2026-11-12.md`);
   Composer's are `circuit-composer/docs/handoff/` (c558971, 20 files, start with `README.md`).
   Read the note for a crate before bringing it in.
5. Names the rules it works by that are written nowhere, so they go into Boardroom's `CLAUDE.md`
   (the benchmark rules; "look at pictures before trusting a table"). From Composer so far: judge
   a placement or floorplan change over several seeds, not one; every figure in a package is cited
   with a quote that `library_quotes` checks; a hard limit is set only from a figure someone
   stands behind, recognised guesses set only soft ones; others' boards and files are read in
   place with their licences recorded.

## Open questions for the owner

1. **Where `reshape` sits** (above).
2. **`packages/`:** in this repo for now (**recommended**, 4.5 MB), or their own repo from the
   start, since Composer's spec has them published openly and versioned per chip family?
