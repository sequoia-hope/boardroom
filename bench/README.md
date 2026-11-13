# bench

The corpus harness, the look set and the scoreboards, from UltraRoute's `tools/` (as new
files). Python for now; the measures in `tidy.py`, `dfm.py`, `designer.py` and `netcheck.py`
are what the `examine` crate grows from.

| Tool | What it does |
|---|---|
| `truth.py` | **the designers' boards**: prepares each corpus board from its raw file (the designer's own copper and pours, refilled by KiCad) and the same board bare with its pours kept; counts how `processed` differs; draws both side by side |
| `netlists.py` | which boards ship a netlist that is theirs: every fetched `.net` compared with the designer's board pad for pad (`examine::netmatch`) |
| `extract.py` | **what the designers did**: plane layers, the frame of the routing per neighbourhood, bundles and repeats, read from the designer's board (`examine::designer`); `look` draws each over the look set, `count` totals a tier |
| `pcbench.py` | prepare, run, report, compare, rejudge, tiers, ab over the PCBench corpus; judged by `kicad-cli` DRC after a KiCad refill. `--source truth` routes the designers' boards (pours kept), after `judge-truth` has judged each designer's own board for its floor |
| `pcbench_tiers.json`, `pcbench_timeouts.json`, `pcbench_variants.json`, `variants/`, `apply_variant.py` | the tiers (smoke, focus, gate, full), per-board time, our own patched boards |
| `rulebench.py` | boards with rule files, judged the same way |
| `refill_zones.py` | refill a board's zones through KiCad before DRC (our own fill is a preview) |
| `lookset.py`, `lookset.json` | the designer's board beside ours, as pages to look at |
| `scoreboard.py` | "like the designers": a `--source truth` run beside the designers' own boards, measured by `examine::designer::like` (angles in each neighbourhood's frame, track on the designers' planes, bundles, repeats) and `tidy.py` |
| `prototype.py` | the plan-then-draw prototype (`crates/plan`) on the four small look boards: the plan as a rubber-band sketch beside the designer's board, per layer |
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
  What the designers did, same day (`extract.py count --tier gate`, a first reading: the
  pictures and the list of what it reads wrongly are at `LAB/planned/extraction/`): 53 of the
  61 multi-layer boards have a plane layer (half poured, little track) and 35 a layer both
  poured and routed; 159 of the 253 two-layer boards have a side at least half poured. Ten
  boards have more than 10% of their track off the 45-degree grid of the board's axes, and on
  those it is mostly off every grid (drawn at any angle): routing on a turned grid is rare in
  the corpus (the ErgoDone's thumb cluster at 20 degrees; motionhexa, outside it, at 30). 167
  boards have a quarter or more of their track in bundles and 83 a bundle of eight or more
  nets; 59 have parts routed alike, 25 four or more of them.
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
- **Angles are measured in a frame.** `tidy.py` measures direction in the board's x and y, so a
  board routed in a turned frame scores as off-angle; the scoreboard and the look set use
  `examine::designer::like` instead (each neighbourhood's own 45-degree grid, and the designer's
  grid there). A hexagonal lattice is not a 45-degree grid at any turn: motionhexa's designer
  reads 89% off the board's axes and still 29% off its own frame.
- **The honest baseline** (2026-11-12, run `truth-base-9512c88`: the frozen router `sqpad-9512c88`,
  `--recipe default --auto-planes`, 120 CPU s, no escalation, the gate tier's 314 designers' boards;
  pages `LAB/planned/scoreboard/truth-base-9512c88/` and `LAB/planned/lookset/truth-base-9512c88/`).
  215 of 314 clean (52 open, 37 with new errors, 10 both; 44 boards gain `starved_thermal`). Summed
  over the 314, designers against the router: track off its neighbourhood's grid 1.3% against 16.8%;
  track on the 77 boards' layers the designer kept as planes 1.9 m against 13.3 m (`--auto-planes`
  takes only inner layers at least 60% poured); in a bundle 33% against 23%; bus members beside a
  sibling 27% against 8%; vias 17,345 against 12,762 (the designers stitch their pours); track
  336 m against 303 m. Do not compare with the `processed` figures (273 or 274 of 315): other boards.
- **The designers' own boards mostly fail the judge** (`pcbench.py judge-truth --tier gate`): KiCad
  finds an error added by the routed copper on 225 of the 314 (edge clearance on 140, mask bridges
  on 67, starved thermals on 65, clearance on 55, hole clearance on 43, shorts on 35) and something
  unconnected on 63. Checked the same day: 310 of the 314 are KiCad 4 or 5 files, and on every
  one the edge clearance (0.5 mm), hole clearance (0.25 mm), annular width (0.1 mm) and
  two-spoke minimum are the defaults KiCad wrote when it converted them; those KiCads had no
  such checks, nor the mask-bridge one. Only the clearance is the designer's (the file's own net
  class). Leaving out the kinds their KiCad did not check, 75 of the designers' boards still
  have an error (clearance on 55, shorts on 35, many of them against a footprint's own copper),
  and the baseline is 252 of 314 clean, not 215: 35 of its failures are starved thermals alone.
  The judge is unchanged (any new error fails); which count is the gate is the owner's to say.
- The bare board's pours are filled (by KiCad, on the empty board): the frozen router reads planes
  from fills, and given outlines only it took none.
- pcbnew hangs on some old boards when its output is a pipe or it has a console, and its zone
  filler never returns on a few: `truth.py` gives it files, no input and a time limit.
- Look at the designer's board beside ours before quoting any number.

Not yet done: the tiers were chosen on `processed` and need choosing again from full runs of the
designers' boards (`pcbench.py tiers` now refuses to mix the two; only the gate tier is prepared
from raw); the "sound" scoreboard; `pcbench.py run` with a router built here (there is none yet);
`lab.py` has not been run from this directory.
