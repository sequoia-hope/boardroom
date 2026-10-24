# Gate 6: placement

Status: **passed** (2026-10-24; pixels turned 180° at the release fixes)

Placement is code: `scripts/board.py place` sets every part from brief.md's
block plan, copies the RP2350 power block from the vendor board
(`transplant.py`), and draws the locked critical pre-routes (crystal, USB data,
PIX12; routing.md step 4a). The bottom art zone (pin labels, J4 order, URL) is
reserved with no-via rule areas before routing (silkscreen-and-art.md,
"Reserve the silk before routing").

## Checks

`placement_check.py hw/easy2350/a` (after the final route; placement unchanged by routing):

```
  Y1 pad 1 (/XIN) is 7.4 mm from its IC pin
  Y1 pad 3 (/XOUT_R) is 5.9 mm from its IC pin
  keep-apart U3-Y1: 20.0 mm (min 5)
  keep-apart U3-D3: 25.9 mm (min 5)
  keep-apart U3-J1: 2.0 mm (min 0.5)
  ratsnest (MST, all nets except GND): 816 mm over 72 nets

  decoupler -> pin, pad centre to pad centre
    C1    -> U2.5    VBUS        2.66 mm
    C11   -> U1.53   +3V3        4.07 mm
    C13   -> U1.46   /VREG_AVDD  2.52 mm
    C14   -> U1.49   +3V3        1.17 mm
    C15   -> U1.50   +1V1        2.13 mm
    C16   -> U1.23   +1V1        3.07 mm
    C17   -> U1.6    +1V1        2.57 mm
    C18   -> U1.39   +1V1        3.17 mm
    C2    -> U3.8    VBUS        1.73 mm
    C22   -> U5.8    +3V3        1.83 mm
    C24   -> U4.5    VLED        1.97 mm
    C25   -> D3.4    VLED        1.32 mm
    C26   -> D4.4    VLED        1.32 mm
    C27   -> D5.4    VLED        1.32 mm
    C28   -> D6.4    VLED        1.32 mm
    C3    -> U3.1    +3V3        2.03 mm
    C4    -> U1.1    +3V3        2.64 mm
    C5    -> U1.11   +3V3        2.58 mm
    C6    -> U1.20   +3V3        2.24 mm
    C7    -> U1.30   +3V3        2.47 mm
    C8    -> U1.38   +3V3        3.19 mm
    C9    -> U1.44   +3V3        2.64 mm

  PASS: 0 errors, 0 warnings
```

| Item | Evidence |
|---|---|
| Block flow follows brief.md | USB-C → ESD → LDO at the top, flash at the QSPI corner, power block above pins 46–50, crystal below U1, pixels and buttons at the bottom, SWD on the bottom edge (renders `top.png`, `bottom.png`) |
| RP2350 power block = vendor copy | review-layout.md "Vendor comparison": every part, zone and spoke via at the vendor's coordinates to 0.01 mm |
| Every decoupler at its pin | above, ≤ 4.1 mm centre to centre; the pixel caps now 1.3 mm from VDD; the 1–1.5 mm part-note targets missed by C2, C3, C24 are logged below |
| Crystal | Y1 7.4 / 5.9 mm from its pins; XIN limit 10 mm (decisions.md) and pre-routed with no vias |
| Keep-aparts | U3 20 mm from Y1, 26 mm from the pixels |
| No passives in a row | the only row is C25–C28, one per pixel, under the pixel they serve |
| Pixel orientation | D3–D6 turned 180°: each DOUT faces the next DIN across the gap, 2.3–2.9 mm L1 hops, nothing under a body (decisions.md 2026-10-24) |
| Audit layout rules drawn as rule areas | pixel centre strips (no copper on L1), VREG_LX cut-out, edge strips (no tracks within 0.7 mm of the long edges), art-zone reservations |
| Art zone reserved before routing | `stage_zones`: "plane silk …" rule areas under 29 header labels, J4's order, the URL, identity, limit and warning lines, and the top labels (5V, 25, BOOT, SWD, RST) |

## Independent review

`review-layout.md` (fresh-context reviewer, phases 6/7/9 together). Findings
that concern placement and their resolution:

| # | Sev | Finding | Resolution |
|---|---|---|---|
| 1 | MAJOR | U3 has no ground vias beyond the EP | Fixed: 6 locked pad vias round U3 before routing (`stage_padvias`) |
| 2 | MAJOR | Decouplers return through the top pour only | Fixed: a GND via at every capacitor's ground pad, U2 and U4 (C20 and D4 excepted, decisions.md) |
| 3 | MAJOR | C20/C21 placement forced XIN through vias | Fixed: C20/C21 re-placed, XIN pre-routed on L1 with no vias (9.4 mm); limit 10 mm with reasoning (decisions.md) |
| 11 | MINOR | R10 9 mm from the pixel it feeds | Partly: R10 sits between pixel 1's and 2's caps, LED_DIN 7.5 mm, no vias; a courtyard-clean search found no spot nearer pixel 1's DIN or U4 pin 4. C24 stays 2.0 mm from U4 pin 5 (nothing closer is free); SN74LV1T125 at 800 kb/s is not sensitive to 1 mm of supply trace |
| 12 | MINOR | C2 1.7 mm, C3 2.0 mm from U3 (guide ≤ 1.5) | Accepted: J1's shell and U2 bound C2, the 3V3 pad vias bound C3; 0.2–0.5 mm over a guide for a 300 mA LDO stable from 0 Ω ESR (ldo-3v3.md) |
| 15 | MINOR | Pixel caps 2.7 mm from VDD | Fixed: 1.3 mm after the pixels were turned |
| 17 | NOTE | No silk dot on L1 | Accepted: the top has no silk designators (policy none); JLC places from the CPL and the fab drawing |

Found at the release fixes, not by a review: the old pixel orientation put copper under all four centre strips (WS2812B-2020 p.2), and the rule had never been drawn as a keepout. Fixed by the turn and the rule areas.

Re-review of the fixes: `review-release.md` and its re-check.
