# Gate 2: schematic

Status: **passed** (2026-10-23; changed in phase 7 — see the end)

Schematic: `hw/easy2350/a/easy2350.kicad_sch`, drawn by `scripts/schematic.py`
(one A3 sheet). Render: `design/renders/schematic.pdf`.

## Checklist (references/schematic-style.md)

| Item | Evidence | State |
|---|---|---|
| ERC 0 errors, every warning explained | `** ERC messages: 0  Errors 0  Warnings 0` | done |
| Style check passes | below | done (warnings explained) |
| Every part cross-checked against its audit | review1-*.md compared the netlist with each audit's Schematic connections; review2-sch.md item 7 | done |
| Value, footprint, MPN, supplier PN on every symbol | style check `fields` rule; sync check at phase 6 | done |
| Visual review passed | review2-sch.md (Fable, rendered sheet) | done after fixes |
| PCB view regenerated | `scripts/view.sh build pcbview.toml` at the end of phase 7 | done |

## Style check

```
schematic_style_check: /home/sequoia/Software/easy_pcb/examples/easy2350/hw/easy2350/a/easy2350.kicad_sch
  1 sheet(s), 58 parts, 22 of 25 capacitors name the pin they decouple

  WARNINGS (7)
    [decoupler] C4 is drawn 93 mm from U1.1, a stacked pin: the drawing can't put it beside that pin, so placement relies on the Decouples field
    [decoupler] C5 is drawn 84 mm from U1.11, a stacked pin: the drawing can't put it beside that pin, so placement relies on the Decouples field
    [decoupler] C6 is drawn 75 mm from U1.20, a stacked pin: the drawing can't put it beside that pin, so placement relies on the Decouples field
    [decoupler] C7 is drawn 67 mm from U1.30, a stacked pin: the drawing can't put it beside that pin, so placement relies on the Decouples field
    [decoupler] C8 is drawn 58 mm from U1.38, a stacked pin: the drawing can't put it beside that pin, so placement relies on the Decouples field
    [decoupler] C17 is drawn 42 mm from U1.6, a stacked pin: the drawing can't put it beside that pin, so placement relies on the Decouples field
    [decoupler] C18 is drawn 51 mm from U1.39, a stacked pin: the drawing can't put it beside that pin, so placement relies on the Decouples field

  PASS: 0 errors, 7 warnings
```

The 7 decoupler warnings are the RP2350's stacked IOVDD/DVDD pins: a drawing
can't put a cap beside a pin that shares its point with five others, so each
cap carries a `Decouples` field naming its pin, and `placement_check.py`
holds the board to it. The reviewer accepted this (review2-sch.md #1).

## Visual review (review2-sch.md)

| # | Sev | Finding | Resolution |
|---|---|---|---|
| 1 | MINOR | Decoupler row away from stacked pins | Accepted: `Decouples` field + note (above) |
| 2 | MAJOR | C28's text straddled the BUTTONS box | Fixed: RGB box widened, buttons block moved right |
| 3 | MINOR | C1 drawn at the LDO, not at U2 | Fixed: C1 hangs above U2 pin 5 |
| 4 | MINOR | Text overlaps (PWR_FLAGs, U3 value, D7/C23, switch values) | Fixed; (d) LED_DIN and (g) VREG_AVDD label left as is (legible, net names needed) |
| 5 | MINOR | Crystal/SWD corner cramped | SWD resistors removed in phase 7; corner now clear |
| 6 | MINOR | Rail labels on J3/J4 instead of power symbols | Disputed: a power symbol on J3.16 would cross the J3 GND bus; labels merge with the power nets (tools/kicad.md) |
| 7 | MINOR | Decouples missing on C15, C24, C25–C28 | Fixed; C24 moved next to U4 on the board |
| 8 | MINOR | No author in the title block | Fixed |
| 9 | MINOR | J2 GND bus on the box border | Fixed |

## Changed after the gate (phase 7)

C10 (ADC_AVDD) and R13/R14 (SWD 100 Ω) removed; D1/R9 moved on the board;
TP3 a 1.0 mm pad on the bottom. ERC and the style check above are from after
those changes.
