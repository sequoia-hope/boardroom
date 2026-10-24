# Gate 5: stackup and rules

Status: **passed** (2026-10-23; rules revised in phase 7)

`design/stackup.md` gives the stack, JLC's 4-layer limits (capabilities page
fetched 2026-10-23) and the rule for each net class with its reason.
`scripts/project.py` writes them into `easy2350.kicad_pro`; `design/checks.toml`
holds the same fab limits for the check scripts.

| Check | Result |
|---|---|
| Every net class has rules | `nets_check.py --rules`: PASS, 0 errors, 0 warnings (gate 3) |
| Every rule traces to nets.csv or a fab limit | stackup.md tables |
| Via sizes avoid JLC surcharges | 0.5/0.2 signal, 0.6/0.3 power, 0.6/0.25 vendor copy: all ≥ 0.45 mm pads |

## Revised during routing (decisions.md, stackup.md)

- POWER track 0.4 → 0.25 mm (2× IPC-2152), signal 0.127 → 0.10 mm (JLC 0.09),
  signal vias 0.6/0.3 → 0.5/0.2: the QFN escape needed the room.
- POWER/SWITCH clearance 0.12 mm and CLOCK 0.15 mm: the vendor RP2350 copper
  is drawn to these.
- Board minimum track 0.09 mm: Freerouting necks tracks at QFN pads (the
  import step widens anything thinner).
