# Stackup and rules: easy2350

Phase 5. Rules are written into the project by `scripts/project.py`; checked
with `nets_check.py hw/easy2350/a --rules`.

## Stackup

JLCPCB 4-layer, 1.6 mm, JLC04161H-7628 (their default, no impedance-control
fee). Thicknesses are JLC's published stackup for this code; the capabilities
page (fetched 2026-10-23) gives only the 7628 εr 4.4, so the thicknesses
below are an assumption to re-check in JLC's stackup tool at order time.

The stackup table inside `easy2350.kicad_pcb` (Board Setup → Physical
Stackup) is still KiCad's default of three 0.48 mm dielectrics, and the
gerber job file carries it. JLC builds the stackup chosen on the order form,
not the job file: **select JLC04161H-7628 when ordering** (review-release #8).
Nothing on this board is impedance-controlled, so the table changes no rule.

| Layer | Copper | Dielectric below | Use (brief.md §5) |
|---|---|---|---|
| L1 F.Cu | 1 oz (35 µm) | 7628 prepreg ≈ 0.21 mm, εr 4.4 | parts, short signals, RP2350 power zones |
| L2 In1.Cu | 0.5 oz | core ≈ 1.07 mm | GND plane, cut-out under VREG_LX only |
| L3 In2.Cu | 0.5 oz | 7628 prepreg ≈ 0.21 mm | 3V3 plane, whole board (no routing) |
| L4 B.Cu | 1 oz | — | Signals and VBUS/VLED, 1V1 spokes (vendor), GND pour in the gaps |

## Fab limits (JLC 4-layer, 1 oz, capabilities page 2026-10-23)

| Item | JLC | Our rule | Why the margin |
|---|---|---|---|
| Track / space | 0.09 / 0.09 mm | 0.09 track / 0.10 space board minimum | Classes are 0.127 and up; Freerouting necks a track to 0.095 where it enters a 0.4 mm-pitch QFN pad, which JLC builds |
| Via hole / pad | 0.15 / 0.25 mm; holes 0.2–0.25 with a pad < 0.45 cost extra | min hole 0.2, min via pad 0.45; signal vias 0.5 / 0.2, power vias 0.6 / 0.3 | 0.5 / 0.2 is the smallest via with no surcharge and a 0.15 mm ring; the QFN escape needs it. The vendor RP2350 layout uses 0.6 / 0.25 |
| Annular ring | 0.15 abs min (0.20 recommended) | 0.15 | Every via, and the DFN's EP vias (0.55 / 0.25) |
| Hole to hole | 0.20 mm | 0.25 mm | |
| Via hole to track | 0.20 mm | 0.20 mm hole clearance | |
| Copper to routed edge | ≥ 0.20 mm | 0.30 mm | Header pads are 0.76 mm from the long edges, USB-C pads further |
| Silk width / height | ≥ 0.15 mm / 1.0 mm recommended | 0.15 mm / 0.8 mm | 0.8 mm is legible at JLC (checks.toml default) |
| Solder mask bridge | 0.10 mm | KiCad default (0) with JLC's own mask DFM | The QFN's 0.2 mm gaps take a 0.10 bridge |

## Net classes (from nets.csv)

`nets_check.py` derives 0.127 mm minimum width for every power net (IPC-2152,
10 °C rise, ≤ 0.5 A), one via each (two for VBUS and GND) and a 0.10 mm
clearance (IPC-2221 B2 at ≤ 5.5 V, floored at the fab limit). The classes
are wider where the job needs it:

| Class | Track | Clearance | Nets | Reason |
|---|---|---|---|---|
| Default / SIGNAL | 0.10 | 0.10 | GPIO, RUN, SWD, CC, pixel data | The 0.4 mm-pitch QFN escape and the header fan-out; above JLC's 0.09 / 0.09. 0.127 left the router 2–3 GPIO short (phase 7) |
| POWER | 0.25 | 0.12 | VBUS, +3V3, +1V1, VLED, GND, VREG_AVDD | 2× the IPC-2152 width; pours carry GND and 3V3. 0.4 mm took the inner-layer channels the GPIO fan-out needs (phase 7, first routing). 0.25 mm at 0.5 A over 30 mm drops ≈ 30 mV on 1 oz |
| SWITCH | 0.40 | 0.12 | VREG_LX | Vendor zone geometry; class only matters if anything routes it |
| DIFF_HS | 0.20, gap 0.15 | 0.15 | USB_*_C, USB_*, USB_*_MCU | ≈ 106 Ω differential over L2 at 0.21 mm (si-review.md §3; the routes are mostly uncoupled, ≈ 70 Ω single-ended). Full speed (12 Mb/s, ≥ 4 ns edges, ≈ 26 mm of trace) is lumped and tolerates it (intent.md) |
| SE_HS | 0.10 | 0.12 | QSPI_* | Over solid L2, ≤ 25 mm |
| CLOCK | 0.15 | 0.15 | XIN, XOUT, XOUT_R | ≤ 5 mm; 0.15 because the vendor layout puts R2 0.18 mm from a 1V1 via |

## Thermal spokes

Minimum resolved spokes is 1 (KiCad's default is 2): every part is reflowed,
and the 0402 pads around the RP2350 have room for only one.

## Special rules (keep-outs and zones)

- **L2 cut-out under VREG_LX** and L1's LX pad (chips/rp2350.md, RP2350 DS
  §6.3.8). Rule area on In1.Cu, no copper.
- **L2 solid** under Y1, its caps and the XIN/XOUT traces (review1-core #4).
- **U3 exposed pad**: 3 vias in the pad (footprint), a ≥ 50 mm² top GND pour
  from its pin-free ends with ≥ 4 more vias (ldo-3v3.md).
- The vendor RP2350 power zones keep their own priorities, clearances and
  connection styles (`scripts/board.py place` transplants them).
