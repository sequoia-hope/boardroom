# Gate 9: silkscreen and art

Status: **passed** (2026-10-24, final board)

`scripts/board.py silk` (run by `finish`, before stitching so the vias keep off
the text). All text with a fixed place (pin labels, J4's order, URL, identity,
limits, warnings, and the top's 5V, 25, BOOT, SWD, RST) is reserved before
routing with no-via rule areas; `via_nudge.py` moves any via left under a
label that couldn't be reserved (J2 pins 1–11, beside the flash escape). Policy and layout: brief.md §10, with the changes logged in
decisions.md (designators none; URL wrapped; limits and warnings as separate
lines; VBUS label narrowed; BOOT/SWD spacing; pin-1 dots).

## Checks

`silk_check.py hw/easy2350/a` (KiCad DRC with every silk rule forced to error,
plus the stroke-level checks):

```
silk_check: /home/sequoia/Software/easy_pcb/examples/easy2350/hw/easy2350/a
  58 parts, 0 designators shown (policy: none), 4 connectors, 58 board silk texts
  designator rotations: none

  PASS: 0 errors, 0 warnings
```

| Item | Evidence |
|---|---|
| Zero overlaps (silk/silk, silk/pad, silk/edge) | silk_check above; DRC (gate 7) has no silk item |
| No silk over vias or tented plated holes | silk_check `silk-over-via`: none. The art zone was reserved with no-via rule areas before routing; stitching vias keep 0.1 mm off text strokes |
| Every connector labelled | J1 "5V"; J4 "SWD" (top) and "DIO GND CLK" (bottom, pin order seen from below); J2/J3 a label at every one of the 40 pins on the bottom, matching the schematic pin map (review-layout checked all 40) |
| Buttons, LED, test point | "BOOT", "RST", "25" (GPIO25 LED), "1V1" at TP3 (bottom) |
| Pin-1 / polarity marks | silk_check `pin1`: none missing. Dots at U1, U4, D3–D6 (pin 1 = DOUT, now bottom-right), the D2/D7 cathodes and L1's 1V1 end (decisions.md); U3 keeps its footprint's pin-1 corner line (no room for a dot); U2/U5/D1 keep their footprint marks |
| Board identity | bottom: "easy2350 rev A", "2026-10", "github.com/ sequoia-hope/ easy_pcb" (three lines), "CERN-OHL-P-2.0" |
| Limits and warnings | bottom: "3V3 out 100mA", "VBUS out 50mA", "3.3V logic only", "no VBUS in / with USB in". "Not Pico pinout" (review-release #6) found no room; README and intent.md carry it (decisions.md) |
| Text size | all 0.8 mm high, 0.15 mm stroke (JLC minimums, checks.toml); VBUS 0.65 mm wide |

Renders: `design/renders/top.png`, `bottom.png` (silk painted over copper by
`render.py`), `3d-top.png`, `3d-bottom.png`.

## Independent review

`review-layout.md` silk findings:

| # | Sev | Finding | Resolution |
|---|---|---|---|
| 5 | MAJOR | Silk over vias on the bottom (and one on top); silk_check missed it | Fixed: art zone reserved before routing; strokes tested against vias in the silk stage and in silk_check (`silk-over-via`) |
| 14 | MINOR | "5V" and "25" read as header labels; "BOOT SWD" reads as one phrase | Partly: BOOT and SWD pushed apart (SWD can't sit over J4, decisions.md). "5V" stays beside J1 and "25" beside D1: the top has no pin labels, so nothing competes with them |
| 17 | NOTE | No silk dot on L1 | Fixed: dot at the 1V1 end |

Found at this gate by the author, not the review: the silk trim had removed or
weakened the pin-1 or cathode marks of nine parts. Fixed, and `silk_check.py` now fails it.
