# Independent layout review: easy2350 rev A (phases 6, 7, 9)

Reviewer: layout review agent, 2026-10-23. Board `hw/easy2350/a/easy2350.kicad_pcb`.
Coordinates are board mm from the top-left corner (USB-C at y = 0), as in brief.md;
"rel U1" means relative to U1's centre (10.53, 26.53), the vendor layout's frame.

## Checks run

| Check | Result | My reading |
|---|---|---|
| `kicad-cli pcb drc --severity-all` | 53 violations: 52 `lib_footprint_mismatch` (expected, board-local silk trims), 1 `track_dangling` (+3V3, 0.007 mm stub at (16.5, 23.1)); 0 unconnected | Clean apart from the stub (#20) |
| `placement_check.py` | PASS. Y1 pads 7.4 / 5.9 mm from pins; U3–Y1 20 mm, U3–D3 26 mm, U3–J1 2.0 mm; every decoupler ≤ 4.1 mm from its pin | OK; U3–J1 deviation is logged in decisions.md |
| `silk_check.py` | PASS: 0 errors | **False pass**: 20 silk texts on B.Silkscreen and one on F.Silkscreen overlap vias (#5). The script misses silk-over-via |
| `sync_check.py` | PASS, 58/58 | OK |
| `nets_check.py --rules` | PASS | Does not enforce `length_limit` (XIN 10.6 mm vs 5, #3) or "no vias" notes |
| Vendor comparison (pcbnew, both boards) | L1, C14/C15 (CIN/COUT), C13 (CFILT), R3, R4/R5 (27 Ω), C11, the 9 spoke vias, PGND vias (3.1, −4.9)/(3.1, −5.5), zone outlines and priorities all at the vendor's rel-U1 coordinates to 0.01 mm; L1 polarity dot on the 1V1 side as the vendor | Transplant is faithful |

## RP2350 chip-note compliance (chips/rp2350.md)

| Rule | Status |
|---|---|
| CIN, L1, COUT on the chip side | Yes, F.Cu |
| 1V1 ring (prio 3) and 3V3 band (prio 1) under the chip, filled | Yes, 5.7 mm² and 8.6 mm² filled |
| 1V1 spokes: 4 ring vias + far vias, 0.6 mm B.Cu wires | Yes, all 9 vias at vendor positions, 0.6 mm on B.Cu |
| VREG_LX (prio 5), 1V1 (prio 6), 3V3 (prio 7), PGND (prio 4) zones | Present and filled; outlines match the vendor |
| PGND to GND at one point through two adjacent vias; CFILT on its own GND path | Yes: two GND vias inside the PGND zone at (13.62, 21.03)/(13.62, 21.62); C13's GND is a separate 1.4 mm² pour fragment with its own via at (14.38, 22.68) |
| L2 cut-out under LX and L1's LX pad | Yes: rule area rel x 1.55–3.35, y −8.35…−2.85 covers the LX zone (1.85–3.05, −8.05…−3.15) with ≥ 0.3 mm margin; the L2 fill has a 1.8 × 5.5 mm hole there |
| No other copper under L1 on the top | Yes: only the LX and 1V1 zones under the courtyard; the outer GND pour is cut back around the corner |
| 1V1 never a thin wire between spokes and ring | Yes (0.6 mm B.Cu; 0.2 mm top stubs only pin-to-via as the vendor) |
| No foreign net inside the pad ring | **Top: yes. Bottom: no** — VBUS and the BOOTSEL net cross under U1 on B.Cu (#9) |
| Escape directions free; USB pin-to-27 Ω copied | Yes |

## Findings

| # | Severity | Finding (location) | Suggested fix |
|---|---|---|---|
| 1 | MAJOR | **U3 (LDO) has no ground vias beyond the three 0.25 mm EP vias.** ldo-3v3.md requires a ≥ 50 mm² top GND pour with ≥ 4 more vias at the pin-free ends (brief §8 said 6+); the nearest GND via to U3 is 5.8 mm away at (10.4, 15.2). The 0.62 W worst case rests on this copper. The pour is there (U3 sits in the 404 mm² top GND pour) but it reaches L2 only through distant vias. | Add 6 × 0.6/0.3 GND vias in the pour around U3: above it at y ≈ 10.0 (x 14.5–17.5) and to its right at x ≈ 17.5–18.0 (y 11.5–14.5), plus two beside pad 4. Free copper exists in both places (render `f_usb`). |
| 2 | MAJOR | **Decouplers return to ground through the top pour only; most have no GND via.** Nearest GND via to the ground pad: C4 (IOVDD pin 1) 5.0 mm, C18 (DVDD 39) 4.7 mm, C8 (IOVDD 38) 3.7 mm, C9 (IOVDD 45 / ADC_AVDD 44) 3.2 mm, C17 (DVDD 6) 2.7 mm, C2 (LDO in) 6.8 mm, C3 (LDO out) 5.6 mm, C22 (flash) 3.9 mm, C23 (VLED bulk) 2.9 mm, C25–C28 (pixels) 2.3–6.2 mm, C1 (U2 VBUS) 2.8 mm. The vendor board gives each decoupler a via. The top pour around U1 is sliced by GPIO escapes, so the real return path is longer than the straight distance. (C5, C6, C7, C11, C13–C16 are fine at ≤ 1.2 mm.) | One 0.5/0.2 GND via at the ground pad of each of the listed caps (the stitch script's pad mode, or by hand). Priority: C4, C8, C9, C17, C18, C2, C3, C22. |
| 3 | MAJOR | **XIN is routed through two vias and 6 mm of B.Cu** (via at (9.63, 31.56) below pin 21, via at (8.34, 36.73) by C20); total 10.6 mm. nets.csv: CLOCK, limit 5 mm, "no vias". XOUT_R is 8.2 mm (limit 5). The 15 pF caps C20/C21 sit 3.6 mm either side of Y1 (2.5 mm from its pads), not between Y1 and the chip as the vendor's C3/C4, which is what forced the detour. | Place C20/C21 between U1 and Y1 beside the XIN/XOUT_R pads (vendor geometry, Y1 + (−1.4, −3.4)/(+1.6, −3.4)), route XIN and XOUT_R on F.Cu only. There is GND pour room left of R6/C16 for the XIN run. |
| 4 | MAJOR | **USB D+ hairpin.** USB_DP_C is 13.1 mm against USB_DM_C 5.8 mm: from J1's pad it runs left to x = 6.0 at y 10.7, turns, and comes back along y 11.18 to U2 pin 1 — a 7 mm U-turn 0.5 mm from itself, beside U5's QSPI pins (render `f_usb`). Autorouter artifact on the one controlled pair. | Hand-route DP_C straight from the J1 pad pair to U2 pin 1 (≈ 3 mm). |
| 5 | MAJOR | **Silk over vias on the bottom (and one on top).** My collision check (text stroke outline vs via pads): the six identity/warning lines at x 7.1–13.9 cross 25+ vias and U1's EP vias; pin labels VBUS, GP29, GP27, RUN, GP22, GP21, GP0, GP2, GP8, GND (J2 pin 13), GP12 and "DIO GND CLK" each cross one or two; "SWD" (top) touches the via at (6.38, 48.13). The phase-9 rule is zero tolerance; `silk_check.py` reported PASS, so it does not test silk-to-via. | Move the identity block onto copper-free bottom area (the lower third, x 3–18, y 44–49 has only J4 and a few tracks, or between the GPIO fan-outs) and nudge the listed pin labels 0.3–0.5 mm. Fix `silk_check.py` to include vias on both sides. |
| 6 | MINOR | **Merged via antipads form slots in the L2 ground plane under the QFN escapes**, and signals cross them: 3.0 × 3.3 mm at (5.9–8.9, 28.6–32.0) under GPIO9–15; 2.4 × 2.8 mm at (5.4–7.8, 20.9–23.7) under QSPI_SCLK, SD0, SD1, SS; 1.6 × 1.2 at (6.4, 17.6) under SD1/SD2; 2.5 × 2.6 at (14.3–16.8, 24.1–26.6) under GPIO26–29; 2.2 × 2.0 at (10.2–12.5, 30.9–32.8) under RUN/SWCLK/SWDIO. Same clusters make matching holes in L3. Cause: 0.2 mm plane clearance on 0.5 mm vias (0.9 mm antipads) at 0.6–0.7 mm spacing. 2–3 mm gaps are small for QSPI edges but the SE_HS rule says none. | Set the L2/L3 zone clearance to 0.15 mm for vias (or a via-specific rule), and spread the GPIO/QSPI escape via clusters to ≥ 1.0 mm pitch where room allows. Re-check hole sizes after refill. |
| 7 | MINOR | **Single via at every power-plane transition.** The whole 3V3 rail enters L3 through one 0.6/0.3 via at (14.73, 16.48) beside C3; U1's top 3V3 zones are fed from L3 by two vias (6.52, 23.18) and (14.12, 23.54); VBUS changes layer through one via at each end of its B.Cu run ((17.46, 3.58), (4.82, 29.88)). nets.csv asks two vias for VBUS; current-wise all are inside a 0.3 mm via's rating (≤ 0.5 A). | Add a second via at C3 for 3V3 and one beside each VBUS via. Cheap insurance against a single plated hole. |
| 8 | MINOR | **VLED trunk runs under the crystal.** The 0.19 mm VLED track from D4 to D5 at y 39.68 (x 9.8–14.2) passes 1.9 mm below Y1's body and 1.3 mm from C21; it carries the pixels' 2 kHz PWM steps (≤ 144 mA). Brief §7 intended "ground between". | Route the D4→D5 VLED link along y ≈ 43 (with the D5→D6 link) instead, keeping it south of the pixel row. |
| 9 | MINOR | **Foreign nets under U1 on B.Cu**: VBUS (to D2/VLED) crosses diagonally under the chip, rel (2.5, −3.3) → (0.8, −1.6) → (0.1, 0.7) → (−5.7, 3.3), threading between the EP's thermal vias, and the BOOTSEL net (Net-(R7-Pad2)) runs rel (−2.5, −5.9) → (−1.7, −0.5). Two planes separate them from the top, so electrically benign; it fails the chip note's check as written and breaks the B.Cu ground under the EP via field. | Route VBUS to D2 down the left channel beside the GPIO fan-out, or accept and record the deviation in the phase-7 gate with the plane argument. |
| 10 | MINOR | **USB pair split between U2 and the 27 Ω**: DP and DM drop to B.Cu at vias 3 mm apart ((12.69, 12.29) vs (11.28, 9.68)) and run 8 mm on B.Cu referenced to the 3V3 plane, with no GND return via beside them. Full speed tolerates it. | Keep both on F.Cu from U2 to R4/R5 (the vendor's pin-to-resistor copper already exists from R4/R5 to the chip), or at least pair the vias. |
| 11 | MINOR | **R10 (33 Ω LED_DIN series) is at the board corner (4.43, 49.22)**, 6 mm from U4 pin 4 and 8 mm from D3: Net-(U4-Y) 9.5 mm with 2 vias, LED_DIN 12.7 mm with 2 vias. rgb-leds.md: R_DIN at U4 pin 4; U4 next to D3 so LED_DIN is the short net. C24 (U4's 100 nF) is 2.0 mm from pin 5 (guide: ≤ 1 mm), placed in the pixel-cap row for tidiness. | Move R10 next to U4 pin 4 (above U4, between the switches) and C24 beside U4 pin 5. |
| 12 | MINOR | **LDO caps slightly far**: C2 1.7 mm from pin 8, C3 2.0 mm from pin 1 (guide ≤ 1.5 mm); VBUS enters U3 pin 8 through a 0.19 mm, 1.8 mm neck at (14.25, 11.2–13.1) (class 0.25, guide ≥ 0.4). | Slide C2 and C3 toward the package; widen the pin-8 neck to 0.25 mm (pad entry allows it). |
| 13 | MINOR | **QSPI lengths spread 9 mm** (SD1 7.8 mm … SD3 16.7 mm; flash.md asked ± 5 mm) and QSPI_SCLK has 2 vias / 10.7 mm on B.Cu (guide: avoid vias on SCLK). All ≤ 25 mm. | Route SCLK on F.Cu if the escape allows; otherwise accept at the SDK default clock. |
| 14 | MINOR | **Top labels ambiguous.** "5V" at (4.09, 3.78) sits beside J2 pin 2 (GP1), 2.5 mm from the header pad and 2 mm from J1's body, so reads as a header-pin label; "25" at (17.23, 17.97) sits between J3 pins 13 and 14 and reads the same way; "BOOT" and "SWD" are 0.6 mm apart and read as one phrase "BOOT SWD". The SWD pin names are only on the bottom (brief §10 wanted them at J4 on top). | Put "5V IN" against J1's shell outline (x ≈ 7–8, y ≈ 1, inside the outline and clear of the J2 pads) so it belongs to the connector; put "25" directly above D1 at x 17.6; move "SWD" to the right of J4 (x 14.5, y 48.1) with "CLK GND DIO" if 0.6 mm text is acceptable, else keep names on the bottom and say "SWD ↓". |
| 15 | MINOR | **Pixel decouplers 2.7 mm from VDD**: C25–C28 sit below the pixels at y 42.62 while pad 4 (VDD) is the top-right pad at y 40.28; the feed wraps around the package. | Rotate each pixel's cap to the right of its VDD pad (x + 1.6, y 40.3) if the D3→D4 data links allow; else accept (800 kb/s, local 100 nF is a filter not a timing cap). |
| 16 | NOTE | **Bottom GND pour is 9 fragments**, four of 5–9 mm² with a single via each; useless but harmless copper. | Delete fragments < 10 mm² or leave. |
| 17 | NOTE | **No silk polarity mark on L1** (vendor footprint has only a fab dot). JLC places from the CPL rotation and the fab drawing. | Add a 0.3 mm silk dot at the 1V1 end if the mask clearance allows (L1's body is 2.0 × 1.6 mm; courtyard edge has 0.25 mm). |
| 18 | NOTE | **Autorouter necks below class width**: XOUT 0.09/0.112 mm at pin 22 (CLOCK 0.15), VLED 0.125–0.15 mm at C28/D6 pad entries (POWER 0.25, 36 mA there), +3V3 0.15 mm stubs to pins 1/45/49 and to C11 (expected per chip note). All above JLC's 0.09 mm. | Widen XOUT at the pad to 0.15; the rest is acceptable. |
| 19 | NOTE | Header GND pins and GND vias join L2/L3 solid (no via reliefs; TH reliefs only on pads): correct. 44 GND vias, 0.6/0.3 power vias, 0.5/0.2 signal vias as stackup.md. | — |
| 20 | NOTE | DRC `track_dangling`: a 0.007 mm +3V3 stub at (16.5, 23.1) by C9. | Delete the segment. |
| 21 | NOTE | **Tool gaps found by this review**: `silk_check.py` does not detect silk over vias (#5); `nets_check.py` does not enforce `length_limit` (#3). placement_check reports distances but has no via-at-decoupler test (#2). | Add the three tests; they would have caught #2, #3 and #5. |

## What is good

- The RP2350 power block is a clean, verified copy of the vendor layout: every part, zone, spoke via and the PGND join at the vendor's coordinates; L2 cut-out correct; nothing but LX/1V1 copper under L1; the outer pour cut back around the corner.
- Block flow matches the brief: USB-C → ESD → LDO at the top, flash at the QSPI corner (≤ 17 mm), regulator above pins 46–50, crystal below, pixels and buttons at the bottom, SWD on the bottom edge. Each decoupler is at its own pin (≤ 4 mm); no tidiness rows except the pixel caps, which belong to the pixels.
- L2 is one unbroken plane apart from the LX cut-out and the header antipads; L3 is one 3V3 plane; no signal crosses a real split (only merged antipads, #6).
- Current paths: VBUS 0.25 mm, VLED 0.25 mm trunk, 1V1 0.6 mm spokes, 3V3 by plane — all above the 0.127 mm IPC-2152 derived minimum; VBUS to pin 40 and 3V3 to pin 36 are through-hole pins on the planes.
- Bottom pin labels match the schematic's J2/J3 map pin for pin (checked all 40 against pad nets), mirrored correctly; identity block has name, rev, date, repo, licence and the three warnings from brief §10.
- Keep-aparts met: U3 20 mm from Y1, 26 mm from the pixels; LX corner is on the opposite side of U1 from Y1.

## Verdict

**Not passed yet — 5 MAJOR, all cheap to fix, no BLOCKER.** Nothing found would stop the board from working, but #1 (LDO thermal vias) and #2 (decoupler ground vias) undercut the thermal and decoupling cases the part notes were written around, #3 and #4 are routing of the two nets the rules single out (crystal, USB) in a way the rules forbid, and #5 fails the silk gate's zero-tolerance rule while the check script said PASS. Fix 1–5, re-run DRC and the checks (after fixing silk_check), re-render, and this review can be closed on the renders. The MINORs are worth doing in the same pass where they are a via or a short re-route (#7, #8, #11, #12, #14); #6, #9, #10, #13, #15 may be accepted with a line in the phase-7 gate.
