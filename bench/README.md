# bench

The corpus harness, the look set and the scoreboards, from UltraRoute's `tools/` (as new
files). Python for now; the measures in `tidy.py`, `dfm.py`, `designer.py` and `netcheck.py`
are what the `examine` crate grows from.

| Tool | What it does |
|---|---|
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
- **Two known faults in the measure, not yet fixed** (found 2026-11-12): the corpus's input
  boards have the designers' pours removed, so comparisons with the designers on boards with
  planes are not to be trusted; and `tidy.py` measures direction in the board's x and y, so a
  board routed in a turned frame scores as off-angle. Fixing both is the first work of step 4
  (`docs/one-system-spec.md`), before any router code.
- Look at the designer's board beside ours before quoting any number.

Not yet run end to end from this directory: `pcbench.py run` with a router built here (there is
none yet), `lookset.py`, `scoreboard.py`, `lab.py`.
