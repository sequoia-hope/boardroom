# Boardroom

> **Boardroom** is becoming the main window of a board design suite with Circuit Composer and
> UltraRoute Studio: see [`docs/platform-spec.md`](docs/platform-spec.md) and
> [`docs/shell-spec.md`](docs/shell-spec.md). The app is in [`app/`](app/): build it with
> `app/build.sh`, run it with `app/serve.sh --port N`, and open its window with
> `app/server/target/release/boardroom open --port N`. A read-only, single-window version runs on
> [GitHub Pages](https://sequoia-hope.github.io/boardroom/). Below is the board-project template
> it grew from (formerly easy_pcb).

A starting point for designing a circuit board with [Claude Code](https://claude.com/claude-code) and KiCad.

You describe the board. Claude works through it in phases, from requirements to fab outputs. Each phase ends at a **gate**: a checklist with real tool output as evidence, reviewed by a separate fresh-context agent before the work moves on. A project page shows where things stand and what Claude needs from you, with a schematic / PCB / 3D viewer of the board ([pcbview](https://github.com/sequoia-hope/pcbview)) that also works on a phone.

## What's in here

```
.claude/skills/pcb-design/   the design skill: phases, gate checklists, review prompts, lessons
CLAUDE.md                    tells Claude to use the skill in this repo
design/                      intent, decisions, net classes, gates, status (filled in as you go)
hw/                          KiCad projects, one per board variant: hw/<board>/<variant>/
pcbview.toml                 which boards the viewer builds
index.html                   the project page: status plus the embedded board viewer
vendor/pcbview/              the viewer, pinned as a git submodule
vendor/ultraroute/           the autorouter, pinned as a git submodule (scripts/route.sh builds it)
scripts/doctor.sh            checks KiCad, Python and the rest are installed
scripts/view.sh              builds the viewer
serve.sh                     serves the project page
examples/easy2350/           a worked example: an RP2350A board taken through the gates
```

New to the flow? Read [`examples/easy2350/`](examples/easy2350/) first: it shows what each gate
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
| 7 | Routing: pours, short critical routes, autorouter for the rest | |
| 8 | Simulation and signal integrity | |
| 9 | Silkscreen and art | review render |
| 10 | Release: fab outputs, sync and rotation checks, final review | **yes** |

The details are in `.claude/skills/pcb-design/SKILL.md` and its `references/`.

## Start a board

1. **Make your copy.** Click *Use this template* on [GitHub](https://github.com/sequoia-hope/easy_pcb) to create your own repo, then clone it:
   ```sh
   git clone --recurse-submodules https://github.com/<you>/<your-board> my-board
   cd my-board
   ```
   If you cloned without `--recurse-submodules`, run `git submodule update --init`.

2. **Check your tools.**
   ```sh
   scripts/doctor.sh
   ```
   You need KiCad 9+ (`kicad-cli`), Python 3.11+ and numpy. KiCad's Python module and ngspice are recommended. Routing needs Rust (`cargo`): the autorouter, [ultraroute](https://github.com/sequoia-hope/ultraroute), is pinned as a submodule and `scripts/route.sh` builds it on first use.

3. **Serve the project page** in a second terminal, on any free port:
   ```sh
   ./serve.sh --port 8000
   ```
   and open `http://localhost:8000/`. To check from your phone, use the machine's address on your network instead of `localhost`.

4. **Start Claude Code** in the repo and describe the board:
   ```sh
   claude
   > Let's design a board: a USB-C powered controller for two 12 V DC motors, about 50 × 80 mm.
   ```
   Claude starts at phase 0 and asks about everything the description leaves open. It stops for your sign-off at phases 0, 4 and 10, and whenever `design/status.json` has a question for you, the page shows it under **Needs you**.

## Rebuilding the viewer

Claude does this at every gate. To run it yourself:

```sh
scripts/view.sh build pcbview.toml       # all boards -> viewer/, and the one marked embed into index.html
scripts/view.sh models pcbview.toml      # which parts have no 3D model, and why
```

`viewer/` is gitignored. To publish the page with GitHub Pages, remove that line from `.gitignore` and commit the build.

## Updating pcbview and ultraroute

```sh
git -C vendor/pcbview pull
git add vendor/pcbview && git commit -m "Bump pcbview"
git -C vendor/ultraroute pull                  # scripts/route.sh rebuilds on the next run
git add vendor/ultraroute && git commit -m "Bump ultraroute"
```

A new ultraroute can route a board differently: re-run phase 7 and its gate
after bumping it, rather than mixing a new router with an old route.

## The skill learns

When you correct Claude, it adds the correction to `.claude/skills/pcb-design/LESSONS.md` as a rule, and reads that file at the start of every session. Commit it with the rest of the project. If a lesson applies to every board, consider sending it upstream.

## License

MIT, see `LICENSE`. The license covers this template; the boards you make from it are yours to license as you like.
