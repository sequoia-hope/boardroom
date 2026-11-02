# Independent layout review 2 — easy2350 rev A (gates 7, 8, 9)

Board reviewed: `/home/sequoia/Software/easy_pcb/examples/easy2350/hw/easy2350/a/easy2350.kicad_pcb` (route of 2026-11-01, ultraroute 0b0f336 + `board.py regzones`). Everything below is my own measurement with KiCad 9.0.8 `pcbnew`/`kicad-cli`; scripts and crops are in `/tmp/claude-1000/-home-sequoia-Software-easy-pcb/7a2c7658-b6df-4f87-85c2-d6de8594fb26/scratchpad/review2/` (`dump.py`, `fills2.py`, `geo2.py`, `necks.py`, `necks2.py`, `conn.py`, `planes.py`, `xtalk.py`, `silk.py`, `crop_*.png`). Coordinates are board mm from the top-left corner. No project file was edited.

Baseline: `kicad-cli pcb drc --severity-all`: 0 errors, 0 unconnected, 54 `lib_footprint_mismatch` warnings (board-local footprints), schematic parity 0. 138 vias (51 GND), 16 footprints locked, no locked copper. All nets.csv length/via limits met (XIN 8.0 mm L1 no vias; USB 7.7/7.4, 10.8/11.0, 3.6/3.6 mm; QSPI 5.3–14.1 mm; LX zone; AVDD 4.5 mm).

## Findings

### BLOCKER
None in the "board cannot work" sense. The two MAJORs below block gate 7 under the project's own rules (chips/rp2350.md checks; JLC minimum feature).

### MAJOR

**A1. RP2350 1V1/FB distribution is still router wire; the reference ring and FB path are not realised (previous M3, not fixed in substance).**
- Location: U1 regulator corner (10.7–14.1, 17.8–24.2) and under U1.
- Evidence (pcbnew, `geo2.py`): +1V1 = 50 segments, 38.0 mm of 0.20/0.25 mm track, 8 vias (was 35 mm / 8 vias at M3). The vendor zones are present and filled (ring `transplant 3` 5.2 mm², regulator zone `transplant 5` 1.8 mm² covering L1 pad 1 and C15.1), but:
  - **No 1V1 zone touches any DVDD pin.** Zone/pad intersection for U1 pins 6, 23, 39, 50 = 0 for every 1V1 zone (`necks2.py`). The ring fill is touched by exactly one junction: the pin-6 track and via C at (8.4–8.8, 24.6–24.9). Pins 23, 39 and 50 bypass the ring entirely (their vias at (10.545,29.235), (13.255,26.125), (12.922,24.192) sit 0.28–0.53 mm outside the ring fill). The ring is a dead-end appendage.
  - **The regulator zone is joined to the rest of 1V1 only by a 0.25 mm F.Cu wire that leaves the zone's top edge at (12.152,18.488) (0.013 mm overlap with the outline), runs over the top of L1's body to vias A (12.922,17.858) and B (14.043,17.858), then on L4**: path B runs at x 12.467 from y 19.6 to 21.7 directly under the LX zone / L2 cut-out (L3 is the only shield), then to via H (12.922,24.192) = pin 50's feed, on to via D (13.255,26.125) = pin 39, and on to via E (10.545,29.235) = pin 23 / C16 / TP3. Path A runs diagonally under L1's body on L4 to via C = pin 6 + ring.
  - **VREG_FB (pin 50) therefore reaches COUT through ≈10 mm and 2 vias** (pin 50 → 1.4 mm F.Cu → via H → 1.5 mm L4 → 2.1 mm L4 under the cut-out → 1.7 mm → via B → 1.1 mm F.Cu → 1.0 mm F.Cu → zone). Regulator zone fill to ring fill: 2.90 mm apart; zone does not contain pin 50 (its outline ends at y 21.625, pin 50's pad starts at 22.65). DS §6.3.8 "FB fed from the output of COUT" and chips/rp2350.md "1V1 is never a thin wire between the spoke vias and the ring" both fail. The 2026-10-23 phase-8 waiver "vendor copper copied unchanged, validated by Raspberry Pi" no longer describes this copper.
  - Electrically: DC fine (≈25 mΩ to pin 23 at ≤0.4 A, 5–10 mV, and FB compensates up to pin 50); the deviation is reference compliance and the FB/switching-loop geometry, which is exactly what the user's standing rule forbids leaving to the router.
- Fix (zones, no locked copper): (1) extend `transplant 5` with a tongue down to pin 50's pad (x ≈ 11.5–11.65, clear of the CIN zone's left edge at 11.775/12.025), so FB is zone copper from COUT; (2) extend the ring outline with tongues to the inner ends of pins 6, 23, 39, 50 (ring ±2.2 → pad ends at ±3.06), so the pins are joined under the chip as the vendor's traces+spoke vias do; (3) give the router a width it must honour outside the pad ring: `.kicad_dru` `track_width (min 0.5mm)` for `+1V1 && !A.intersectsArea('plane U1 ring')`, and a `disallow via` for +1V1 inside the L2 cut-out area on B.Cu so the spoke does not run under LX. Then re-route and re-check with `necks2.py`'s "zones touching pins" test.

**A2. Four pads are joined to their only track by a copper neck below JLC's 0.09 mm minimum (new; router pad entry).**
- Evidence (`necks2.py`, `conn.py`; chord = width of the copper lens where the track's round end-cap, whose centre lies outside the pad polygon, crosses the pad edge):
  - **J1.B7 USB_DM_C**: track end (11.175,6.445) is 0.090 mm outside the pad's end; overlap 0.00054 mm², neck **0.087 mm**. A7 is properly connected; if B7 opens, D− works in one plug orientation only.
  - **U1.4 GPIO2**: end (6.622,24.858) 0.030 mm outside the pad; neck **0.079 mm**, overlap 0.0008 mm².
  - **U1.8 GPIO5**: end (6.622,26.468) 0.028 mm outside; neck **0.082 mm**.
  - **U5.6 QSPI_SCLK**: end (6.202,10.332) 0.041 mm outside the SOIC pad's left edge; lens 0.009 mm deep, neck **0.058 mm**. It is saved only because the via at (6.027,10.332) has its ring overlapping the pad edge by 0.038 mm (≈0.27 mm chord) — an incidental join the rules do not ask for.
  - Also marginal: U1.50 +1V1 (FB) end 0.107 mm outside, neck 0.128 mm of a 0.25 mm track (above the fab minimum, but it is the regulator's only feedback joint — see A1); R6.1 XOUT_R neck 0.131 of 0.15.
  - 202 pads have track joints; 13 have overlap < 0.003 mm², 6 < 0.001 mm². DRC cannot see this (connectivity is boolean).
- Fix: tracks must end inside the pad (at least half the track width inside the outline). Ask ultraroute to terminate at the pad centre-line/inside the polygon rather than at its boundary; meanwhile add a check (the `necks2.py` test: end-cap centre outside the pad polygon with chord < class width) to `nets_check --routed`, and fail the seed judge on it.

### MINOR

**B1. Via annular rings overlap SMD pads (residual of M1).** The new rule `physical_hole_clearance (min 0.1mm)` stops holes in pads but 30 vias now have their 0.15 mm ring overlapping a pad by 0.01–0.047 mm, hole edge 0.10–0.14 mm from the pad edge (e.g. U1.17/24/26/31/37/41/43, U2.1/3/4/6, R4.1, R5.1, C13.1, C27.1, D3.3, SW1.2, R8.2; list in `necks2.py` output). Tented, so no wicking, but a 0.10 mm mask dam between hole and pad opening is at JLC's limit. Fix: raise the rule to 0.2 mm (ring then clears the pad by 0.05 mm).

**B2. VREG_AVDD still goes to L4 under the regulator corner (previous m1).** Pin 46 → (13.693,23.038) via → 4.5 mm on B.Cu at x 12.85–13.6, y 20.7–23.0 (inside the L2 cut-out's footprint; L3 is the shield, and the L3 antipads of the GND vias at (13.605,22.245)/(13.9,21.288) lie 0.05–0.4 mm from it) → via (14.217,20.622) → C13.1. The vendor runs it 2.5 mm on L1. The direct L1 path is blocked by the CFILT/PGND vias. Fix: `disallow via` for VREG_AVDD, or move C13.2's via to the right of C13.

**B3. LDO thermal path short of ldo-3v3.md (previous m5, unchanged).** U3 EP has 3 in-pad vias (0.55/0.25) against ≥5 asked; 3 GND vias within 4 mm of the EP ((16.898,15.163), (16.658,9.963), (16.925,13.132)) against ≥4 more; the connected top pour is the 110 mm² right-hand pour (31.6 mm² within ±5 mm of U3). ≈ +5 °C/W on the via leg, ≈ +3 °C at 0.61 W: works, but the part note is not met and the EP-via footprint change was never made. The in-pad vias are through-hole pads (not tented) — expect paste wicking.

**B4. LDO output capacitor reaches the pin only through the L3 plane.** U3.1 (14.65,14.025) → 0.85 mm → via (14.225,14.761) → L3 → via (14.47,17.133) → 1.13 mm → C3.1 (14.275,16.025). No same-layer copper between the LDO output and its 10 µF (2.4 mm apart). AP7361C stability is tolerant, but this is 2 vias in the output-cap loop. Fix: a small L1 3V3 zone patch from U3.1 to C3.1 (zones survive the route).

**B5. Crystal load-cap ground (previous m6, unchanged).** C20.2 (6.945,37.375) reaches L2 by a 2.4 mm 0.25 mm track to the via at (6.526,35.002); nearest GND via 2.0 mm. C21.2 and Y1's own GND pads are fine (0.7–1.2 mm). C17.2 (pin-6 100 nF) likewise reaches its via (2.861,23.002) only through a 2.5 mm pour strip (GND L1 poly 10). Fix: a pad via for each (room exists at C20 if the URL reservation is notched).

**B6. 3V3 band under U1 does not do the vendor's job.** `transplant 0` is cut into 4 fragments (3.1/2.1/1.0/0.7 mm²) by the 1V1 feeds and via D/E, and its fill touches **none** of U1's 3V3 pins; pins 1/11/20/30/44/45/53/54 are fed by the L1 3V3 pour, pin 49 by the CIN zone, and **pin 38 by nothing but a 0.2 mm track to one via (13.095,26.895)**. Electrically acceptable with the whole-board L3 plane; note it or extend the band outline to the pad ends.

**B7. SWD pin names only on the bottom.** Top has "SWD" (7.5,48.1); "DIO GND CLK" is on B.Silk, invisible when the board is on a breadboard with the probe plugged in from the top. brief.md §10 asked for the names on the top. Also "RST" where the brief said "RESET" (fine). Log or fix.

**B8. VREG_LX class width not enforced.** The router drew a redundant 0.2 mm LX track (12.957,20.177)→(12.525,23.087) inside the LX zone; SWITCH class is 0.40 mm and there is no `track_width (min)` rule for it. The zone itself necks to 0.26 mm between the CIN and PGND zones (vendor geometry, as copied). Harmless now, but add `track_width (min 0.4mm)` or `disallow track` for LX so the zone is the only copper.

### NOTE
- N1. L2 plane: largest hole 15.1 mm² at (12.1–15.1, 16.6–25.8) — the LX cut-out merged with 10 via antipads (1V1, 3V3, GPIO27/29, AVDD) (was 13 mm²); F.Cu GPIO26–29 cross it for ≤0.5 mm. Next: 4.2 mm² merged antipads under the QSPI escape (7.1–9.3, 19.8–22.9). L3: nothing larger than header/ via clusters. No HS trace crosses a split of consequence (`planes.py`).
- N2. Foreign nets on B.Cu under U1 (previous m7): GPIO23, BOOTSEL (Net-(R7-Pad2)), VREG_AVDD. VBUS and GPIO27 gone. L2+L3 between them and the die; the `plane U1 ring` rule area is F.Cu only, so this is by design.
- N3. Under Y1 on B.Cu: SWCLK (x 10.65), SWDIO (11.49), GPIO15, GPIO17, BOOTSEL run the length of the crystal body; L2 is solid there (no via antipad under Y1 or the XIN/XOUT traces). The `plane crystal` rule area is F.Cu only. Acceptable.
- N4. Layer-change vias of QSPI/USB are 2–4.5 mm from the nearest GND via (SCLK 3.0, SD0 4.2, SD2 3.8, USB_DM_C at J1 4.5 mm); return changes planes through decoupler vias. Fine for FS USB and QSPI at the slow-slew default; the same as the previous gate's F-7 acceptance.
- N5. VBUS (previous m4): single via (9.002,12.258) where the D2/VLED branch leaves L1 (≤0.2 A), pair at (5.747,28.3/29.0). 0.25 mm everywhere; the 0.2 mm dru rule holds. 3V3 from the LDO enters L3 through one 0.3 mm via (0.28 A, OK per nets.csv).
- N6. Decoupler vias otherwise good: every other cap's GND pad has a via within 0.7–1.3 mm; every 3V3 cap within 0.7–1.5 mm; no floating pour fragment on L1 or L4 (every fragment has a via or a through pad) — previous m3 fixed.
- N7. Aggressor/victim (`xtalk.py`, 0.6 mm window, same layer): only GPIO23‖RUN 1.5 mm at 0.21 mm centre spacing on B.Cu near (14.6,31.9), and XIN‖XOUT 0.9 mm at the 0.4 mm pin pitch. Nothing to fix.
- N8. PGND: zone `transplant 2` joins the plane only through its two vias (13.9,21.288) (inside) and (13.605,22.245) (ring 0.15 mm into the fill, 1.0 mm apart; vendor 0.6). L1 GND pour is 0.315 mm from the PGND fill, 0.637 mm from the LX fill, nothing under L1's body on L1. CFILT C13.2 has its own via (14.181,22.411) in a 0.29 mm² pour island. L2 cut-out contains the whole LX fill and all four corners of L1 pad 2; L2 GND sampled absent under LX, present at x 12.0/12.2 and 13.9. These parts of §6.3.8 are met.
- N9. Paperwork: decisions.md's last entry is 2026-10-31; the 2026-11-01 regzones decision, the via-in-pad and crystal dru rules and the silk changes are unlogged. phase7/8/9.md still describe the Freerouting board.
- N10. Silk (gate 9): all 40 header labels match the pad nets (checked each); "DIO GND CLK" reads correctly from below; identity (name, rev A, 2026-10, 3-line URL, CERN-OHL-P-2.0), both limits and both warnings present, 0.8 mm / 0.15 mm, mirrored on B.Silk; "5V IN" at (4.4,3.8) beside J1 (2.8 mm from J2.2, but no pin labels on the top); designators none per decisions.md with the assembly drawing. Crops of the bottom show labels clear of vias; pin-1 dots at U1, U3, U4, L1 (1V1 end), D3–D6 visible in the renders.

## Previous review's findings (review-layout-2026-10-31.md) — verdicts

| # | Verdict | My measurement |
|---|---|---|
| M1 via holes in pads | **Fixed; residual B1** | 0 via holes inside/at SMD pads (hole-to-pad ≥0.103 mm everywhere); 30 rings overlap pads by ≤0.047 mm because the rule is 0.1 mm |
| M2 PIX12 through Y1 | **Fixed** | PIX12 1.27 mm L1 D3.1→D4.3, 3.3 mm from C20/XIN; crystal rule area and dru rule present |
| M3 1V1/FB router-laid | **Not fixed** (A1) | Zones exist, but 38 mm of 0.2/0.25 mm 1V1 wire, 8 vias; ring touched at one corner only; pins 23/39/50 bypass it; FB ≈10 mm, 2 vias, on L4 under the cut-out |
| M4 LX 0.2 mm track | **Fixed** (B8 cosmetic) | LX is the zone, 2.27 mm², contains pin 48 and L1 pad 2, inside the L2 cut-out; redundant 0.2 mm track inside it |
| M5 "5V" reads as J2 pin 2 | **Fixed** | "5V IN" vertical at (4.4,3.8) beside J1 |
| m1 AVDD on L4 under LX | **Not fixed** (B2) | 4.5 mm on B.Cu inside the cut-out footprint, 2 vias |
| m2 CFILT shares PGND | **Fixed** | own via (14.181,22.411), 0.9 mm outside PGND, pour 0.3 mm clear |
| m3 floating GND fragment | **Fixed** | no L1/L4 GND fragment without a via or through pad |
| m4 VBUS one via | **Not fixed, acceptable** | one via at (9.002,12.258) on the ≤0.2 A branch; pair at D2 |
| m5 LDO 3 GND vias | **Not fixed** (B3) | 3 within 3 mm, 4 within 4 mm; 3 EP vias of ≥5 |
| m6 C20.2 GND 2.0 mm | **Not fixed** (B5) | 2.4 mm track to via; nearest via 2.0 mm |
| m7 foreign nets under U1 L4 | **Partly** | VBUS, GPIO27 gone; GPIO23, BOOTSEL, AVDD remain (N2) |
| m8 QSPI spread / SD1 vias | **Fixed** | 5.3–14.1 mm (8.8 mm spread, within flash.md's ±5); SD1 and SS 0 vias; SCLK/SD0/SD2/SD3 2 each |
| m9 footprint silk over vias | **Fixed as far as renders show** | no via under footprint silk in the top render/crops; silk_check now tests it |
| n1 L2 hole 13 mm² | Larger: 15.1 mm² (10 antipads merged) | — |
| n2–n5 | Hold | USB OK (4 vias/line, pair-coupled on L4), planes OK, currents OK, pin map correct |
| n6 fab margins | Hold | 0.09 mm necks at QFN pads, 0.5/0.2 vias; plus A2 necks below 0.09 |
| n7 tool gaps | Closed, but a new gap: no check for track-end-outside-pad (A2) | — |

## Gate 8 trigger assessment (simulation.md) for this route
- DIFF_HS (USB): inside limits; FS 12 Mb/s, ≥4 ns edges vs ≈22 mm (≈130 ps) per line, D+/D− change layer together at J1 and U2, 0.35 mm centre spacing on L4 → **lumped estimate suffices; no simulation.**
- SE_HS (QSPI): 5.3–14.1 mm ≤ 25, 75 MHz default, 1–2 ns edges → **estimate suffices**; note N4 (return vias) and the slow-slew default.
- CLOCK: XIN 8.0 mm L1 no vias (stray ≈0.8 pF of HDG's 3 pF) → **estimate suffices**; bring-up XOSC check stands.
- SWITCH near CLOCK/ANALOG: LX vs AVDD only via the L4 run (B2); LX to XIN 12 mm → **estimate suffices**, better removed by B2's fix.
- Switching converter: no SPICE model exists; the previous waiver rested on "vendor copper unchanged", **which is no longer true (A1)**. The waiver must be re-justified against geometry that matches the reference, i.e. after A1 is fixed.
- Power drop: 3V3 ≈2 mV; 1V1 ≈5–10 mV to pin 23 at 0.4 A (FB at pin 50 compensates part) → estimate suffices.
- Thermal: LDO ≈ +3 °C over the datasheet case with 3 EP vias → estimate suffices.
- Aggressor/victim script: run (N7), nothing to resolve.

## Verdicts
- **Gate 7 (routing): not yet.** A1 (1V1/FB/ring is router wire; reference not realised; M3 stands) and A2 (sub-minimum pad necks at J1.B7, U1.4, U1.8, U5.6) must be fixed and re-routed; take B1, B2, B5, B8 in the same pass since each is a one-line rule or a zone tongue. DRC/parity/currents/planes otherwise pass.
- **Gate 8 (SI): not yet.** No simulation is needed for USB, QSPI, crystal or power (estimates above suffice), but the regulator waiver is void for this copper and phase8.md still describes the old board; re-state the trigger table and waiver after gate 7's re-route, with the regulator corner rendered beside the reference.
- **Gate 9 (silkscreen): passes as drawn** (labels, map, identity, warnings, pin-1 marks all verified) **but is route-dependent**: re-run silk placement and silk_check after the gate-7 re-route; log B7 (SWD names bottom-only) in decisions.md or add them on the top.
