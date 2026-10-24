# easy2350: a worked example

An RP2350A development board (USB-C, 16 MB flash, all 30 GPIO on breadboard
headers, 21 × 51 mm; Pico-sized but **not** Pico pin-compatible: a Pico
carrier would put 5 V on GPIO29), designed with this template's `pcb-design` skill from
phase 0 to fab outputs. Read it gate by gate:

| Phase | Gate file | State |
|---|---|---|
| 0 Design intent | [`design/gates/phase0.md`](design/gates/phase0.md) | passed, signed off (added the addressable LEDs) |
| 1 Architecture and parts | [`design/gates/phase1.md`](design/gates/phase1.md) | passed |
| 2 Schematic | [`design/gates/phase2.md`](design/gates/phase2.md) | passed |
| 3 Net classification | [`design/gates/phase3.md`](design/gates/phase3.md) | passed |
| 4 Mechanical and layout brief | [`design/gates/phase4.md`](design/gates/phase4.md) | passed |
| 5 Stackup and rules | [`design/gates/phase5.md`](design/gates/phase5.md) | passed |
| 6 Placement | [`design/gates/phase6.md`](design/gates/phase6.md) | passed |
| 7 Routing | [`design/gates/phase7.md`](design/gates/phase7.md) | passed: DRC 0 errors, 0 unconnected |
| 8 Simulation and SI | [`design/gates/phase8.md`](design/gates/phase8.md) | passed |
| 9 Silkscreen and art | [`design/gates/phase9.md`](design/gates/phase9.md) | passed |
| 10 Release | [`design/gates/phase10.md`](design/gates/phase10.md) | fab files ready, release review says send; awaiting the user's approval |

Fab files: [`hw/easy2350/a/fab/`](hw/easy2350/a/fab/) (gerbers, JLC BOM and CPL,
STEP). Bring-up: [`design/bringup.md`](design/bringup.md). Board scripts:
[`scripts/board.py`](scripts/board.py) (placement, routing, finish, silk),
[`scripts/fab.py`](scripts/fab.py).

Everything a phase produced lives in [`design/`](design/): the intent, part
audits, decisions with their reasons, and each gate's evidence and review.
The project page is [`index.html`](index.html); from the repo root, run
`./serve.sh --port N` and open `/examples/easy2350/`.
