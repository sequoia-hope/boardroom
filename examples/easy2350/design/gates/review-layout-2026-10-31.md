# Independent layout review, 2026-10-31 (gates 7-9)

Reviewer: fresh-context subagent (stronger model), artifacts and checklists only.
Board: commit 14c4e00, ultraroute 770ef73, routed from bare pads, no locked copper,
no RP2350 reference pours. Coordinates are board mm from the top-left corner.

Verdict then: gate 7 not yet (M1, M2, M4 routing defects; M3 needs zones or a
waiver), gate 8 not yet (phase8.md closures false for that copper), gate 9 close
(M5, m9).

## Findings

| # | Sev | Finding | Resolution (2026-11-01) |
|---|---|---|---|
| M1 | MAJOR | Router fan-out vias drilled inside 12 SMD pads (U2 pins 1/3/4/6, U1 pins 16/31/45, D3.1, C5, C13, C18, R8) and touching 7 more; invisible to DRC | `.kicad_dru` rule "no via hole in an SMD pad" (physical_hole_clearance 0.1 mm, exposed pads of U1/U3 exempt); ultraroute 0b0f336 honours it |
| M2 | MAJOR | PIX12 routed between Y1's pads, 0.17 mm from XIN, 15 mm detour | Rule area round Y1 (+0.5 mm) and dru rule: only XIN, XOUT_R, GND |
| M3 | MAJOR | RP2350 power copper router-laid: +1V1 35 mm of 0.20-0.25 mm wire, 8 vias; no 1V1 ring, 3V3 band or spokes; VREG_FB reaches COUT through a 9 mm 2-via loop | User chose the reference pours as zones (2026-11-01): `board.py regzones` places Raspberry Pi's six power zones relative to U1, unlocked; router joins them |
| M4 | MAJOR | VREG_LX 0.20 mm its whole length in a 0.6 mm channel, no width rule | VREG_LX is the reference zone again (priority 5, 0.15 mm clearance); L2 cut-out covers it |
| M5 | MAJOR | Top "5V" at (4.2, 3.75) reads as a label for J2 pin 2 (GP1) | Removed; J1 labelled "5V IN" beside the receptacle by the silk stage |
| m1 | MINOR | VREG_AVDD dropped to L4 under the switch node | Re-check on the new route |
| m2 | MINOR | CFILT (C13) shares the PGND island | Re-check: PGND is the reference zone again |
| m3 | MINOR | Floating 2.6 mm² GND L1 fragment by the USB front end | Re-check |
| m4 | MINOR | VBUS returns to L1 through one via on the D2/VLED branch | Re-check |
| m5 | MINOR | LDO U3: only 3 GND vias within 3 mm of the EP (ldo-3v3.md asks >= 4 more) | Re-check |
| m6 | MINOR | Crystal-area grounds: C20.2 nearest GND via 2.0 mm | Re-check |
| m7 | MINOR | Foreign nets under U1 on L4 (VBUS, BOOTSEL, GPIO23, GPIO27) | Re-check |
| m8 | MINOR | QSPI spread 9 mm (flash.md asked ±5), SD1 two vias for no reason | Re-check |
| m9 | MINOR | Footprint silk over vias (U5, SW2, D4-D6, Y1, D1, U1) | Silk stage trims footprint silk at router vias; stitch.py keeps off all silk; silk_check now tests footprint silk against vias |
| n1-n7 | NOTE | L2 regulator hole 13 mm² with merged antipads; USB good apart from M1; plane integrity good; currents meet nets.csv; silk pin map correct; fab margins at JLC limits (0.09 mm tracks, 0.15 mm via rings); tool gaps (via-in-pad, footprint silk, chip-note checks) | Tool gaps closed as above |
