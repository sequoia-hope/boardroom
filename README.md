# Boardroom

**Boardroom is the home of a board design suite**: KiCad boards designed with
[Claude Code](https://claude.com/claude-code), from requirements to fab outputs. It holds the
design skill, the board project template, and the app that lists your projects and launches the
other suite apps. It also defines the format every board repo in the suite follows.

**Try it:** [sequoia-hope.github.io/boardroom](https://sequoia-hope.github.io/boardroom/) is a
read-only copy of the app, open on the worked example, easy2350.

**For an agent pointed here:** this README is the map. It says what each project in the suite is,
where its repo and live page are, what a board repo looks like, and how to make one that works
with the viewer, the router and the other apps. Follow the links for detail. The specs under
[`docs/`](docs/) are the full design; [`CLAUDE.md`](CLAUDE.md) holds the rules for this repo, and
[`template/CLAUDE.md`](template/CLAUDE.md) holds the rules for a board repo.

## The suite

| Project | What it does | Repo | Live page | How a board repo uses it |
|---|---|---|---|---|
| **Boardroom** | Design skill, project template and phase gates. The app is the suite's main window: project list, status, the other apps in tabs | [sequoia-hope/boardroom](https://github.com/sequoia-hope/boardroom) | [app demo](https://sequoia-hope.github.io/boardroom/) | **`scripts/new-board.sh` makes a board repo** from `template/`, the skill and the pinned submodules |
| **pcbview** | Schematic, PCB and 3D viewer for KiCad boards, laid out after Altium 365's. Output is a static site that runs on GitHub Pages | [sequoia-hope/pcbview](https://github.com/sequoia-hope/pcbview) | in use: [robot-link](https://sequoia-hope.github.io/robot-link/), [round-pendant](https://sequoia-hope.github.io/round-pendant/) | **Git submodule** at `vendor/pcbview`, built by `scripts/view.sh` |
| **UltraRoute** | KiCad autorouter (`ultraroute route`, Rust, MIT), judged by KiCad's own DRC. **UltraRoute Studio** is its live, interactive view | [sequoia-hope/ultraroute](https://github.com/sequoia-hope/ultraroute) | [Studio](https://sequoia-hope.github.io/ultraroute/) (routes in the browser, WASM) | **Git submodule** at `vendor/ultraroute`, built by `scripts/route.sh` on first use |
| **Circuit Composer** | The stage between schematic and routing: verified chip packages, cited intent files, floorplanning, locked critical blocks, rules and cost grids for the router | [sequoia-hope/circuit-composer](https://github.com/sequoia-hope/circuit-composer) | [app](https://sequoia-hope.github.io/circuit-composer/) | **A tool you install** (`ccomp` CLI and its app). It reads `design/nets.csv` and writes under `compose/` |
| **Waffle Iron** | Parametric mechanical CAD in the browser: enclosures and mechanics around the board | [sequoia-hope/waffle-iron](https://github.com/sequoia-hope/waffle-iron) | [app](https://sequoia-hope.github.io/waffle-iron/) | **A linked project**, usually its own repo. Shares the board outline through `interface/` |
| **board-interface** | Rust crate that reads, writes, hashes and compares the shared board outline files (`board-shape.dxf` + `.json`) | [sequoia-hope/board-interface](https://github.com/sequoia-hope/board-interface) | | **Used by the apps, not the board repo.** Composer and Waffle Iron pin a tag of it |
| **board-draw** | Rust/WASM and WebGL2 board renderer shared by Studio and Composer, so both draw a board the same way | [sequoia-hope/board-draw](https://github.com/sequoia-hope/board-draw) | | **Used by the apps, not the board repo** |

**Submodule or not:** a board repo pins only the tools its own files depend on, **pcbview and
ultraroute**, as git submodules. A route depends on the exact router version, so pinning it per
project is deliberate. Circuit Composer and Waffle Iron are apps that open the board repo. They
are not vendored into it. board-interface and board-draw are libraries inside those apps.

### How the pieces connect

The apps talk through the board repo's files, never through each other's internals.
**KiCad files are the shared truth** for the schematic and the board. Each app owns its own
directory and doesn't write in another's.

```
requirements ─▶ Boardroom skill: phases 0–3 ─▶ hw/<board>/<variant>/*.kicad_sch, design/nets.csv
                                                   │
                     Circuit Composer (ccomp) ◀────┘  intent, packages, floorplan, locked blocks
                                │                     ─▶ compose/<board>/, placed .kicad_pcb
                                ▼
                     UltraRoute (route.sh / Studio)   routes the rest ─▶ routed .kicad_pcb
                                │
                                ▼
          Boardroom skill: phases 8–10, pcbview build ─▶ viewer/, fab outputs, index.html

          Waffle Iron ◀──── interface/<board>/board-shape.{dxf,json} ────▶ Circuit Composer
                       (board outline: one side drives, the other is locked until it takes over)
```

## A board repo

Every board in the suite lives in a repo with this layout. `scripts/new-board.sh` assembles it
from `template/` plus the copied skill (`.claude/skills/pcb-design/`), shared scripts
(`scripts/{doctor,view,route}.sh`), and pinned submodules (`vendor/{pcbview,ultraroute}`):

```
CLAUDE.md                    tells Claude to use the skill and follow its gates
.claude/skills/pcb-design/   the design skill, vendored: phases, gate checklists, review prompts, LESSONS.md
design/
  status.json                headline state: phase, what Claude is doing, what it needs from you, gates, checks
  intent.md                  requirements, each numbered (phase 0)
  nets.csv                   net classes (phase 3; Circuit Composer imports it)
  decisions.md               why each choice was made
  gates/                     one file per passed gate, with real tool output as evidence
  links.toml                 links to Waffle Iron projects (suite spec; planned)
hw/<board>/<variant>/        KiCad projects, one per board variant, e.g. hw/main/a/
compose/<board>/             Circuit Composer's files (written by Composer only)
route/<board>/               UltraRoute Studio's jobs and route recipe
interface/<board>/           board-shape.dxf + board-shape.json, the shared outline (written only by board-interface)
pcbview.toml                 which boards the viewer builds, and which one index.html embeds
index.html                   the project page: status from design/status.json, plus the embedded viewer
vendor/pcbview/              submodule: the viewer
vendor/ultraroute/           submodule: the autorouter
scripts/doctor.sh            checks KiCad, Python and the rest are installed
scripts/view.sh              builds the viewer
scripts/route.sh             builds and runs the router
serve.sh                     serves the project page: ./serve.sh --port N
```

Rules an agent must keep:

- **Never edit `vendor/` or `viewer/` by hand.** Bump a submodule to change a tool, and rebuild
  the viewer to change `viewer/`.
- **Update `design/status.json` and rebuild the viewer at every gate.** The Boardroom app and
  `index.html` both read `status.json`. Its version 1 schema is in
  [`docs/shell-spec.md`](docs/shell-spec.md#statusjson-version-1). Each app writes only under its
  own `apps.<name>` key.
- **Ports come from the caller.** `serve.sh` takes `--port`, then `$PORT`, and fails if it has
  neither. Don't hardcode one.
- **Corrections become lessons.** When the person corrects Claude, the rule goes into
  `.claude/skills/pcb-design/LESSONS.md`.

Repos made this way: [easy2350](examples/easy2350/) (in this repo, the worked example),
[simple-earring](https://github.com/sequoia-hope/simple-earring).

## Start a board

1. **Make your repo.** Clone Boardroom with its submodules, then run `scripts/new-board.sh`:
   ```sh
   git clone --recurse-submodules https://github.com/sequoia-hope/boardroom
   cd boardroom
   scripts/new-board.sh ~/pcb/my-board --name "My board"
   cd ~/pcb/my-board
   ```
   Pass `--github <you>/<my-board>` (and `--private` if private) to create the GitHub repo and
   push in the same step:
   ```sh
   scripts/new-board.sh ~/pcb/my-board --name "My board" --github <you>/<my-board>
   ```
   If `~/.config/boardroom/projects.toml` exists, `new-board.sh` adds the new folder to it.

2. **Check your tools.**
   ```sh
   scripts/doctor.sh
   ```
   You need KiCad 9+ (`kicad-cli`), Python 3.11+ and numpy. KiCad's Python module and ngspice
   are recommended. Routing needs Rust (`cargo`).

3. **Serve the project page** in a second terminal:
   ```sh
   ./serve.sh --port N
   ```
   and open `http://localhost:N/`. To check from your phone, use the machine's address on your
   network instead of `localhost`.

4. **Start Claude Code** in the repo and describe the board:
   ```sh
   claude
   > Let's design a board: a USB-C powered controller for two 12 V DC motors, about 50 × 80 mm.
   ```
   Claude starts at phase 0 and asks about everything the description leaves open. It stops for
   your sign-off at phases 0, 4 and 10, and whenever `design/status.json` has a question for you,
   the page shows it under **Needs you**.

New to the flow? Read [`examples/easy2350/`](examples/easy2350/) first. It shows what each gate
produces on a real board.

## The phases

| # | Phase | You sign off? |
|---|-------|---|
| 0 | Design intent: every requirement gets a number | **yes** |
| 1 | Architecture and parts, each audited against its datasheet | |
| 2 | Schematic, drawn to read like a person drew it | |
| 3 | Net classification | glance |
| 4 | Mechanical and layout brief: outline, connectors, layers | **yes** |
| 5 | Stackup and design rules | |
| 6 | Placement | review render |
| 7 | Routing: pours, short critical routes, the autorouter for the rest | |
| 8 | Simulation and signal integrity | |
| 9 | Silkscreen and art | review render |
| 10 | Release: fab outputs, sync and rotation checks, final review | **yes** |

Each phase ends at a **gate**: a checklist with real tool output as evidence, reviewed by a
separate fresh-context agent before the work moves on. The details are in
`.claude/skills/pcb-design/SKILL.md` and its `references/`.

## The viewer: pcbview

pcbview turns a KiCad project into a static site with schematic, PCB and 3D tabs and a part
pane. It runs on a phone and needs no server code. List your boards in `pcbview.toml`, one
`[[boards]]` entry per variant:

```toml
title = "My board"
out = "viewer"
back = { href = "../index.html", label = "Project page" }
repo = "https://github.com/<you>/<your-board>"   # open-source projects only

[[boards]]
id = "main-a"
name = "Main board, variant A"
pcb = "hw/main/a/main.kicad_pcb"
sch = "hw/main/a/main.kicad_sch"
embed = "index.html"                 # the board the project page shows
```

Then build it. Claude does this at every gate:

```sh
scripts/view.sh build pcbview.toml       # all boards -> viewer/, and the one marked embed into index.html
scripts/view.sh models pcbview.toml      # which parts have no 3D model, and why
```

Every option is in [pcbview's README](https://github.com/sequoia-hope/pcbview#the-config-file).

**Publishing on GitHub Pages:** `viewer/` is gitignored by default. To publish, remove that line
from `.gitignore`, add an empty `.nojekyll` file at the root, commit the build, and turn on Pages
for the branch root in the repo's settings. `index.html` and `viewer/` are plain static files,
so nothing else is needed. [robot-link](https://github.com/sequoia-hope/robot-link) and
[round-pendant](https://github.com/sequoia-hope/round-pendant) are published this way.

## Routing: UltraRoute

Phase 7 lays pours and short critical routes by hand, then routes the rest with ultraroute.
`scripts/route.sh` builds the pinned submodule into `build/` on first use, and the skill's
`autoroute.py` runs it seed by seed and judges each result with KiCad's DRC. Missing router
features go to the [ultraroute project](https://github.com/sequoia-hope/ultraroute) as requests,
not as workaround scripts in the board repo. To watch or steer a route, open the board in
[UltraRoute Studio](https://sequoia-hope.github.io/ultraroute/).

## Composition: Circuit Composer

Circuit Composer covers the stage between schematic and routing. Install it from
[its repo](https://github.com/sequoia-hope/circuit-composer#build-and-run). Then:

```sh
ccomp import easy-pcb design/nets.csv --board <board> \
    --netlist hw/<board>/<variant>/<board>.kicad_sch -o draft.json   # phase-3 net classes -> a draft intent file
ccomp -C . floorplan --seed 2               # place the project's only board
ccomp check package <board>.kicad_pcb --package <pkg> --binding <binding.json>
```

It keeps its files under `compose/<board>/` and hands UltraRoute a placed board, with critical
blocks locked (KiCad's own `(locked yes)`) and rules and cost grids for the rest.

## Mechanical: Waffle Iron and the shared outline

A Waffle Iron project and a Boardroom project **stay linked for as long as the product is being
developed**. The board outline can be drawn in Waffle Iron or in Circuit Composer. One side drives
it at a time, the other shows it locked, and the drive changes hands only when someone takes it
over. The outline crosses in two files that only
[board-interface](https://github.com/sequoia-hope/board-interface) writes:

```
interface/<board>/board-shape.dxf     outline, cutouts, holes, keepouts, each with a stable ID
interface/<board>/board-shape.json    revision, parent, author, driver, thickness, geometry hash
```

Each side records the other's commit, either pinned or following a branch, and sees when it is
behind. Today Waffle Iron imports a `.kicad_pcb` one way. The two-way outline is specified and
partly built. [`docs/suite-spec.md`](docs/suite-spec.md) has the full design, and its "What exists
today" table says what works now.

## The Boardroom app

The app lists your board projects, shows each one's status, gates and renders, and opens
Composer, UltraRoute Studio and Waffle Iron in tabs on one origin.

```sh
app/build.sh                                  # build server, page and desktop window
app/serve.sh --port N                         # serve it
app/gtk/target/release/boardroom-gtk          # desktop window (GTK 4 / WebKitGTK); app/gtk/install-desktop.sh adds a launcher
app/server/target/release/boardroom open      # or a Chrome app window
```

The project list is `~/.config/boardroom/projects.toml`, and machine paths, such as a Waffle Iron
build, are in `~/.config/boardroom/config.toml`. Any folder with `design/status.json` opens as a
project. A passed gate records what it judged (`design/gates/phaseN.inputs.json`, written by
`gate_inputs.py record N`), and the app marks it stale with the reason when one of those inputs
changes. The Versions tab shows linked projects and what a release was built against.

The Terminal tab runs a real `claude` session in the project folder. The server owns it, so it
survives closing the page. It accepts only this machine, and only Boardroom's own page. With
`waffle_relay = "uv run --project <waffle-iron>/relay waffle-mcp-relay"` in `config.toml`, the
session gets Waffle Iron's tools. The relay's port is `proj port boardroom-waffle-relay`.

The read-only copy on [GitHub Pages](https://sequoia-hope.github.io/boardroom/) is built
by `.github/workflows/pages.yml` from snapshots of the projects in `examples.toml`.

## Specs

| Spec | Covers |
|---|---|
| [`docs/platform-spec.md`](docs/platform-spec.md) | The suite: Boardroom, Composer, UltraRoute, and the file seam between them |
| [`docs/shell-spec.md`](docs/shell-spec.md) | The Boardroom app, the project format, `status.json` v1, the vendored skill |
| [`docs/demo-spec.md`](docs/demo-spec.md) | The whole workflow running on GitHub Pages |
| [`docs/suite-spec.md`](docs/suite-spec.md) | Waffle Iron with Boardroom: linked projects, the shared outline, version tracking |
| [`docs/monorepo-spec.md`](docs/monorepo-spec.md) | Merging Boardroom, Circuit Composer, pcbview and UltraRoute into one repo and one app, staged for Antigravity agents (draft) |

## Updating pcbview and ultraroute

```sh
git -C vendor/pcbview pull
git add vendor/pcbview && git commit -m "Bump pcbview"
git -C vendor/ultraroute pull                  # scripts/route.sh rebuilds on the next run
git add vendor/ultraroute && git commit -m "Bump ultraroute"
```

A new ultraroute can route a board differently. After bumping it, re-run phase 7 and its gate
rather than mixing a new router with an old route.

## The skill learns

When you correct Claude, it adds the correction to `.claude/skills/pcb-design/LESSONS.md` as a
rule, and reads that file at the start of every session. Commit it with the rest of the project.
If a lesson applies to every board, send it upstream to this repo.

## License

MIT, see `LICENSE`. The license covers Boardroom and its template. The boards you make from it are
yours to license as you like.
