# bench

The corpus harness, the look set and the scoreboards, from UltraRoute's `tools/` (as new
files). Python for now; the measures in `tidy.py`, `dfm.py`, `designer.py` and `netcheck.py`
are what the `examine` crate grows from.

| Tool | What it does |
|---|---|
| `truth.py` | **the designers' boards**: prepares each corpus board from its raw file (the designer's own copper and pours, refilled by KiCad) and the same board bare with its pours kept; counts how `processed` differs; draws both side by side |
| `netlists.py` | which boards ship a netlist that is theirs: every fetched `.net` compared with the designer's board pad for pad (`examine::netmatch`) |
| `pcbench.py` | prepare, run, report, compare, rejudge, tiers, ab over the PCBench corpus; judged by `kicad-cli` DRC after a KiCad refill |
| `pcbench_tiers.json`, `pcbench_timeouts.json`, `pcbench_variants.json`, `variants/`, `apply_variant.py` | the tiers (smoke, focus, gate, full), per-board time, our own patched boards |
| `rulebench.py` | boards with rule files, judged the same way |
| `refill_zones.py` | refill a board's zones through KiCad before DRC (our own fill is a preview) |
| `lookset.py`, `lookset.json` | the designer's board beside ours, as pages to look at |
| `scoreboard.py` | the scores of a run against the designers' |
| `pcbench_schematics.py` | survey and fetch the corpus boards' schematics and netlists |
| `lab.py` | record an experiment in the lab |
| `tidy.py`, `dfm.py`, `designer.py`, `netcheck.py` | the measures: tidiness, acute angles and slots, hot loops on a routed board, whether a designer routes a net within the rules |

## Outside data

None of it is in this repository, and nothing is guessed (`paths.py`): each path comes from its
environment variable, else the same key in `~/.config/boardroom/config.toml`, else the tool
stops and says which is missing.

| Variable | Config key | What |
|---|---|---|
| `BOARDROOM_LAB` | `bench_lab` | the lab directory: prepared boards, runs, review pages |
| `BOARDROOM_CORPUS` | `bench_corpus` | the PCBench checkout. Many of its boards are unlicensed or not redistributable: **it and the schematics fetched for it never enter this repository** |
| `BOARDROOM_ROUTER` | `bench_router` | the router binary to run, with `kicad-text` beside it. The baseline is the frozen old router as a pinned binary; it is never built here |

## Rules

- Runs go through `~/.claude/bin/reserve`, and under a memory cap.
- Compare two routers with `pcbench.py ab`, which interleaves them in one pool; anything else
  biases the slower side.
- **`processed` is not the designer's board** (found 2026-11-12, worse than "the pours were
  removed"). PCBench's cleaning deletes every copper pour, deletes every track and via on the
  pours' nets, and routes those nets again with FreeRouting. So what the old harness called the
  designer's routing is FreeRouting's on every ground and supply net that had a pour. The
  designer's board is `raw`; `truth.py` prepares it. Until the harness runs on those boards
  (`LAB/pcbench/truth/ID/{designer,bare}.kicad_pcb`), no comparison with "the designers" on a
  board with pours is to be trusted, and the old router's results were measured on boards with
  the planes taken away.
  Gate tier, 2026-11-12 (`truth.py report --tier gate`): 314 of 314 prepared; 281 have pours, 210
  have a layer at least half poured, 60 of the 61 multi-layer boards pour an inner layer; on 270
  the pour nets' copper in `processed` is not the designer's; on the other nets the two agree
  within 5% of track length on all but 37 boards. Pictures: `LAB/planned/truth/`.
  Netlists, same day (`netlists.py --tier gate`): 215 of the 314 ship a `.net` beside the board
  in its source repository; it is the board's on 156 (at least 95% of its nets are the same
  pads), close on 37 (80% to 95%), and not on 22 (another revision or variant).
- **What changes when the harness runs on the raw-based boards** (from the UltraRoute session,
  read from the code, not yet run):
  - Unconnected is KiCad's count, unfiltered. With the designers' pours back, a pad the
    designer's own pour does not reach counts against every run: judge `designer.kicad_pcb`
    with the same judge first and take its unconnected count per board as the floor.
  - The published baselines (FreeRouting, PcbRouter, RL-MCTS wirelength and vias) and every
    stored run and figure (273 or 274 of 315 on the gate tier, 1,009 of 1,182 on the corpus)
    were made on `processed`, where pour nets are tracks. They do not carry over.
  - The tiers were chosen on `processed`, stratified on whether a board has zones; nearly every
    raw-based board has them, so the gate and noisy lists need choosing again.
  - The frozen router takes `--strip-all` (removes tracks and vias, keeps zones) and
    `--auto-planes`, so it can run on `bare.kicad_pcb` as it is.
- **Not yet fixed:** `tidy.py` measures direction in the board's x and y, so a board routed in a
  turned frame scores as off-angle.
- pcbnew hangs on some old boards when its output is a pipe or it has a console, and its zone
  filler never returns on a few: `truth.py` gives it files, no input and a time limit.
- Look at the designer's board beside ours before quoting any number.

Not yet run end to end from this directory: `pcbench.py run` with a router built here (there is
none yet), `lookset.py`, `scoreboard.py`, `lab.py`.
