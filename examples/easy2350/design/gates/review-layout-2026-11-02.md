# Independent layout review 3 — easy2350 rev A (gates 7, 8, 9)

Board: `examples/easy2350/hw/easy2350/a/easy2350.kicad_pcb` (commit ac760ed, ultraroute d791d9b, zones from `board.py regzones`). Every number below is my own measurement with KiCad 9.0.8 `kicad-cli`/`pcbnew`; scripts and crops are in `<scratchpad>/review3/` (`dump.py`, `geo.py`, `pgnd.py`, `necks.py`, `necks2.py`, `necks3.py`, `checks.py`, `misc.py`, `ref.py`, `crop_ours_corner.png`, `crop_ref_corner.png`, `crop_ours_u1.png`, `crop_ref_u1.png`, `crop_ours_L2.png`, `crop_ours_ldo.png`). Coordinates are board mm from the top-left corner of the outline (file origin 100,100). Reference positions are mm relative to U1's centre (10.525, 26.525). No project file was edited.

## 1. Baseline

| Item | Measured |
|---|---|
| `kicad-cli pcb drc --severity-all --schematic-parity` | **0 errors, 0 unconnected, 0 parity issues**; 56 warnings, all `lib_footprint_mismatch` (board-local footprints) |
| Vias | **145**: GND 58, +3V3 15, +1V1 8, VBUS 7, VLED 2, signals 55. Sizes 90 × 0.6/0.3, 55 × 0.5/0.2 |
| Tracks | 650 segments; F.Cu and B.Cu only, In1/In2 are planes (802.8 / 762.2 mm² filled, one outline each) |
| Locked items | 0 tracks, 0 vias, 0 zones; 16 footprints (J1–J4, U1, C14, C15, L1, TP3, SW1, SW2, D1, D3–D6) |
| nets.csv lengths/vias | all met (nets_check): USB_DP_C/DM_C 10.1/8.0 (≤20), USB_DP/DM 11.0/11.3 (≤20), USB_*_MCU 3.2/3.3 (≤5, 0 vias), QSPI 6.4–15.8 (≤25), XIN 8.18 mm L1 0 vias (≤10), XOUT 2.95, XOUT_R 2.03 (≤5), VREG_LX zone + 3.09 mm track (≤5), VREG_AVDD zone only, 0 vias |
| Skill scripts | `pad_joints` PASS (205 pads, 0 thin); `silk_check` PASS; `nets_check --rules --routed` PASS; `placement_check` PASS; `plane_pad_vias GND,+3V3 1.0`: 52 of 74 pads have a via of their own within 1 mm |
| Fab limits | min track 0.09 mm (signal entries at U1's 0.4 mm pads), 0.2 mm GND/3V3 entries at U1, VBUS ≥ 0.25 everywhere; via rings 0.15 mm; no via hole or ring over any SMD pad (only U3's EP thermal vias, exempt) |

## 2. Findings

### BLOCKER
None (nothing I measured stops the board from working).

### MAJOR

**A1. The 1V1 ring is cut in two by a router GND via inside it; the pin-23 spoke is fed through 6 mm of 0.25 mm wire on L4.**
- Location: U1 ring, GND via at (12.32, 27.55) = (+1.79, +1.03) rel. U1, fed by a 0.25 mm F.Cu GND track from the EP centre (10.5, 26.5).
- Evidence (`pgnd.py`, `checks.py`, `crop_ours_u1.png`): zone `transplant 3 +1V1` fills as **2 outlines** — poly 1 (3.59 mm², holds the N, W and E spoke vias at (10.98, 24.34), (8.37, 25.69), (12.64, 26.09)) and poly 0 (1.22 mm², bbox 9.6–12.7 × 27.9–28.7, holds **only** the S spoke via (10.69, 28.64), no track touches it). The reference ring fills as one outline (5.73 mm²) containing exactly the four spoke vias and nothing else. Because the ring is broken, the router joined the S via to the E via with 6.15 mm of 0.25 mm B.Cu track under the chip ((12.64, 26.09) → (11.08, 24.62)/(11.08, 26.48) → (10.69, 28.64)); that wire is the only feed for DVDD pin 23, C16 and TP3. Spoke vias themselves sit at the reference positions within 0.05 mm (N (0.45, −2.19)/(0.48, −5.84); S (0.16, 2.12)/(0.19, 5.79); W (−2.16, −0.84)/(−5.21, −1.01); E (2.12, −0.44)/(5.79, −0.41)); each B.Cu spoke zone has a via inside its fill at both ends, and each F.Cu pin zone touches its DVDD pin (6, 23, 39, 50). So the structure of review 2's A1 is in place; what breaks it is this one via. chips/rp2350.md check "1V1 is never a thin wire between the spoke vias and the ring" fails for the S spoke.
- Fix: a `.kicad_dru` rule on a rule area equal to the ring outline (±2.2 mm, `transplant 3`): `disallow via` for `A.NetName != '+1V1'` and `disallow track` on F.Cu for nets other than +1V1 (GND reaches the EP through its own nine thermal via pads; it needs no router via there). Add to `autoroute.py`'s judge: the ring zone must fill as one outline containing four +1V1 vias. Re-route.

**A2. PGND reaches the ground plane through one via, not two adjacent vias, and that via is outside the PGND zone.**
- Location: (13.92, 21.04) = (+3.40, −5.49) rel. U1.
- Evidence: zone `transplant 2 GND` (PGND, 1.76 mm², touches U1.47, C14.2, C15.2) contains **no GND via** (reference: two, at (3.1, −4.9) and (3.1, −5.5), inside the fill). The only plane connection is the router's 0.6/0.3 via at (13.92, 21.04), whose centre lies 0.02 mm outside the fill edge (x 13.9); it is reached from pin 47 by a 0.2 mm track (12.90, 23.06) → (12.91, 22.05) → (13.92, 21.04). The via's full ring sits on L2 copper (0.282 of 0.283 mm²), and L2 copper is present at both reference via spots, so there is room. A GND stub (13.25, 21.19) → (13.46, 21.37) inside the zone is a leftover. RP2350 DS §6.3.8 / chips/rp2350.md: PGND "taken to the GND plane at ONE point through two adjacent vias"; it carries the regulator's switched return.
- Fix: a stitch step that drops two 0.6/0.3 GND vias inside the PGND zone at the reference spots (13.63, 21.03) and (13.63, 21.63) (a `stitch.py --zone 'transplant 2 GND' --count 2` or equivalent), or a judge rule "PGND fill contains ≥ 2 GND vias"; keep the router from laying GND tracks from pin 47 (the zone already joins it).

### MINOR

**B1. C17 and C4 grounds are far from any via (previous B5/m6, not fixed; C17 regressed).** C17.2 (3.54, 25.5; DVDD pin-6 100 nF) and C4.2 (3.54, 23.1; IOVDD pin-1 100 nF) sit in L1 GND pour fragment 11 (0.76 mm², no via, no through pad). Its only exit is a 2.84 mm 0.25 mm track from (3.73, 22.80) to the via at (4.39, 20.04); C17.2 first runs 2.4 mm of track to C4.2. Ground path: C4 ≈ 3.2 mm, **C17 ≈ 5.3 mm** of 0.25 mm track (review 2 had 2.5 mm of pour). C20.2 (crystal load cap) now 1.73 mm to a via (was 2.0); C7.2 (pin-30 cap) 3.2 mm through the 115 mm² pour. Fix: pad vias at C17.2/C4.2 (the status says no room; a 0.5/0.2 via fits between C4 and C17 at x ≈ 3.5 if the GP7 label reservation is notched 0.3 mm).

**B2. COUT has one via to the N spoke; the reference has two.** Regulator zone `transplant 5` holds one +1V1 via at (11.00, 20.69) = (0.48, −5.84); the reference's second at (0.5, −5.2) (would be (11.0, 21.3)) is absent, and the spot is inside the zone fill. Fix with the same stitch step as A2.

**B3. LDO thermal path still short of ldo-3v3.md (previous B3/m5, partly improved).** EP: 3 footprint vias 0.55/0.25 (note asks ≥ 5; footprint unchanged since gate 6). GND vias within 3 mm of the EP centre (15.6, 12.6): 5 (was 3); within 4 mm: 6; the connected L1 pour is the 115 mm² right-hand pour with 14 vias, 40.7 mm² of it within ±5 mm of the EP. Estimate unchanged: ≈ +3 °C at 0.61 W over the datasheet case; works, part note not met.

**B4. VREG_LX track inside its zone (previous B8, not fixed).** 3.09 mm of 0.20 mm LX track (12.86, 20.12) → (12.51, 20.47) → (12.50, 23.06) lies inside the LX zone fill. Harmless; add `disallow track` for `/VREG_LX` (or `track_width (min 0.4mm)`) so the zone is the only copper, as the reference.

**B5. CIN / VREG_VIN fed by one 0.25 mm track.** Pin 49 and C14.1 are joined by zone `transplant 1` (0.58 mm²), which has no via; the 3V3 feed is one 0.25 mm track to the via at (13.00, 23.92), 2.3 mm. The reference feeds CIN from its outer 3V3 pour. DC is fine (≤ 0.15 A avg), and CIN carries the ripple; noted because the pin-49 branch has no second path. Fix (cheap): a second 3V3 via beside C14.1 or a tongue of the L1 3V3 pour to the CIN zone.

### NOTE

- N1. Regulator corner otherwise matches the reference (`crop_ours_corner.png` vs `crop_ref_corner.png`): LX zone (2.27 mm²) contains pin 48 and L1.2 and lies entirely inside the L2 cut-out (0.000 mm² outside; L2 GND inside the cut-out 0.000 mm²); under L1's courtyard only the LX and 1V1 zones (1.43 + 1.32 mm²), no GND pour; CFILT C13.2 has its own via (14.94, 22.60), 0.71 mm; VREG_AVDD is the F.Cu zone only (0 tracks, 0 vias; previous m1/B2 fixed); FB (pin 50) joins the COUT zone on F.Cu (union of the two zone fills is one outline; the FB strip is the vendor's 0.15 mm width). Extra path not in the reference: a 0.9 mm 0.25 mm F.Cu track from pin 50's zone to the N ring via (11.18, 24.13)→(10.98, 24.34); harmless (same node).
- N2. 3V3 band `transplant 0` (8.92 mm², fed by 3 L3 vias) touches no pad directly but merges with the vendor pin zones for pins 11, 20, 30, 38, 44, 53, 54 (each pin's fill intersection 0.07–0.17 mm²); pins 1 and 45 by the L1 3V3 pour; pin 49 per B5. Previous B6 (pin 38 on one track) fixed: pin 38 → zone → C8.
- N3. Pad joints (previous A2): fixed. Per-pad union of track copper vs pad polygon (`necks2.py`): every pad's joint chord ≥ its narrowest track width within 0.01 mm; the previous offenders now measure J1.B7 0.179 (0.2 track), U1.4/U1.8 0.090 (0.09 tracks), U5.6 ≥ 0.1. pad_joints PASS confirmed.
- N4. Via rings over SMD pads (previous B1): none; the 0.2 mm `physical_hole_clearance` rule holds (only U3's EP thermal vias, exempt).
- N5. VBUS: main path J1 → U3.8 and J1 → J3.20 stays on F.Cu at 0.25 mm (no via within 3 mm of U3.8); J1's left pair (A9/B4) hops L1→L4→L1 through (8.16, 8.31)/(9.01, 9.01). The LED branch to D2 changes layer through five single vias ((5.69, 8.14), (2.89, 9.89), (2.96, 15.35), (2.85, 25.15), (5.97, 29.87)), ≤ 0.2 A each against 0.8 A per via: acceptable as logged (m4/N5), but it is five, not "three", single transitions.
- N6. Planes: L2 one outline, largest void 9.8 mm² at (12.0–15.8, 17.7–24.4) = LX cut-out merged with the corner's via antipads (was 15.1); next 7.4 mm² under J3 pins 11–12, 4.0 mm² under the QSPI escape (5.3–9.5, 19.7–22.3). L3 largest void 8.5 mm² at the J2 column (0.6–3.4, 5.4–10.4). QSPI/USB tracks cross antipad voids for 0.1–0.5 mm (QSPI_SD1 B.Cu 0.96 mm, USB_DP_C B.Cu 0.61 mm); layer-change vias 2–4.5 mm from a GND via. Same acceptance as before (FS USB, QSPI at the slow-slew default).
- N7. Foreign nets on B.Cu under U1's body: BOOTSEL (Net-(R7-Pad2)) 7.4 mm (6.3 mm inside the ring footprint), RUN 0.6 mm; nothing else (m7 partly, by design: the ring rule is F.Cu only). Crystal area: only XIN/XOUT_R/GND on F.Cu; PIX12 is a 1.3 mm hop D3.1→D4.3 (M2 fixed).
- N8. Aggressor/victim (same layer, edge gap < 0.3 mm): USB pairs run coupled at 0.15 mm (as intended, 12.6/6.0/5.9 mm); QSPI_SCLK‖SD0 1.5 mm at 0.12 mm and SCLK‖SD3 1.4 mm at 0.21 mm at the pin escape; XIN‖XOUT 1.65 mm at 0.25 mm (pin pitch). Nothing to fix.
- N9. Silkscreen: all 40 header labels match their pad nets (checked pin by pin against J2/J3 pad nets; RUN's label sits 0.35 mm below its pad centre, inside the hole cut for the vendor via); "DIO GND CLK" reads correctly from below (DIO over x 13.04 = SWDIO); identity (easy2350 rev A, 2026-10, github.com/ sequoia-hope/ easy_pcb, CERN-OHL-P-2.0), both limits and both warnings present, all 0.8 mm / 0.15 mm; top: "5V IN" vertical beside J1 at (4.4, 3.75), "25" at D1, "BOOT", "SWD", "RST"; pin-1/polarity dots at U1, U3, U4, L1, D3–D6, D2/D7 visible in `top.png` and the top-silk gerber; no silk over a via in the renders; SWD pin names bottom-only and "not Pico pinout" absent are both logged in decisions.md.
- N10. Paperwork: decisions.md now carries the 2026-11-01 regzones, dru, and silk decisions (previous N9 closed); phase7/8/9.md still carry the 2026-10-24 evidence under a "stale" banner and must be rewritten for this route.

## 3. Previous findings re-verified

| # (review) | Finding | Verdict | My measurement |
|---|---|---|---|
| M1 (10-31) | via holes in 12 SMD pads | **Fixed** | 0 via holes or rings over any SMD pad; rule 0.2 mm |
| M2 | PIX12 between Y1's pads | **Fixed** | PIX12 1.3 mm L1 hop; only XIN/XOUT_R/GND inside the crystal area |
| M3 / A1 (11-01) | 1V1/FB router wire, no ring/spokes | **Mostly fixed; residual = new A1** | spokes/zones at reference positions ±0.05 mm, vias at both ends of every spoke, FB zone joins COUT; ring split by a GND via, S spoke fed by 6.15 mm of 0.25 mm B.Cu wire; 1V1 track total 13.1 mm (was 38) |
| M4 | LX 0.2 mm track, no width rule | **Fixed (B4 cosmetic)** | LX zone 2.27 mm² in the L2 cut-out; 3.09 mm of 0.2 mm track inside it |
| M5 | "5V" reads as J2 pin 2 | **Fixed** | "5V IN" vertical at (4.4, 3.75) beside J1 |
| m1 / B2 | VREG_AVDD on L4 under LX | **Fixed** | AVDD is the F.Cu zone only, 0 vias, 0 tracks; dru `via_count (max 0)` |
| m2 | CFILT shares PGND | **Fixed** | C13.2 own via (14.94, 22.60), 0.71 mm; PGND fill touches only U1.47, C14.2, C15.2 |
| m3 | floating L1 GND fragment | **Fixed** | no unconnected item; fragment 11 (C4/C17) is track-connected (see B1) |
| m4 / N5 | VBUS single via on the LED branch | **Not fixed, acceptable** | five single vias on the ≤ 0.2 A branch; main path on L1 |
| m5 / B3 | LDO GND vias | **Partly** | 5 GND vias within 3 mm (was 3), 6 within 4 mm; EP still 3 vias of ≥ 5 |
| m6 / B5 | C20.2 GND 2.0 mm | **Partly / regressed elsewhere** | C20.2 1.73 mm; C17.2 ≈ 5.3 mm and C4.2 ≈ 3.2 mm of track to a via (B1) |
| m7 / N2 | foreign nets under U1 on L4 | **Partly** | BOOTSEL 7.4 mm, RUN 0.6 mm remain; VBUS, GPIO27, AVDD gone |
| m8 | QSPI spread / SD1 vias | **Fixed** | 6.4–15.8 mm (9.5 mm spread, inside ±5); SD0/SD2/SS 0 vias, SCLK/SD1/SD3 2 each |
| m9 | footprint silk over vias | **Fixed** | silk_check (with footprint-silk test) passes; none visible in renders |
| A2 (11-01) | four pad joints < 0.09 mm | **Fixed** | every pad joint ≥ its track width (J1.B7 0.179/0.2; U1.4, U1.8 0.090/0.09; U5.6 ≥ 0.1) |
| B1 | via rings over 30 pads | **Fixed** | 0 rings over SMD pads |
| B4 | LDO output cap only through L3 | **Fixed** | zone `plane 3V3 LDO out` (0.51 mm²) joins U3.1 and C3.1 on L1 |
| B6 | 3V3 band touches no pin; pin 38 on one track | **Fixed in effect** | pin zones merge with the band; pin 38 → zone → C8 (N2) |
| B7 | SWD names bottom only | **Logged** | decisions.md 2026-11-01 |
| B8 | LX track inside zone | **Not fixed** | 3.09 mm (B4) |
| N1 (11-01) | L2 void 15.1 mm² | Improved | 9.8 mm² |
| N8 (11-01) | PGND two vias, pour clearances | **Regressed → A2** | PGND now one via, outside the fill; L1 GND pour 0.24 mm above the LX zone's top edge (12.5–13.9, 17.95–18.21), none under L1's body |

## 4. Gate 8 trigger assessment (simulation.md)

| Trigger | This route | Simulation needed? |
|---|---|---|
| DIFF_HS (USB) | 8–11 mm per segment, ≤ 20 mm; FS 12 Mb/s, ≥ 4 ns edges vs ≈ 130 ps of trace; D+/D− change layer together, coupled at 0.15 mm on L4 | No: lumped estimate suffices |
| SE_HS (QSPI) | 6.4–15.8 mm ≤ 25; 2 vias on SCLK/SD1/SD3; voids crossed ≤ 1 mm; return vias 2–4.5 mm | No: estimate suffices at the slow-slew default; note N6 |
| CLOCK (XIN) | 8.18 mm L1, 0 vias (stray ≈ 0.8 pF of HDG's 3 pF); nearest aggressor PIX12 3 mm, LX 13 mm | No; bring-up XOSC check stands |
| SWITCH near CLOCK/ANALOG | LX to AVDD: AVDD on F.Cu right of PGND as the reference; LX to XIN 13 mm; GPIO26–29 escapes cross the L2 void ≤ 0.5 mm | No: estimate suffices |
| Switching converter (RP2350 VREG, no SPICE model) | Geometry now matches the reference in zones, cut-out, spoke vias, AVDD/CFILT, **except A1 (ring severed, S spoke on 0.25 mm wire), A2 (PGND one via), B2 (one COUT via)** | A waiver on "copper matches the validated reference" is **not yet justified**; it becomes justified once A1/A2 are fixed and the corner render is compared again (B2 and the pin-50 ring stub are small enough to log) |
| POWER drop | 3V3 through the L3 plane ≈ 2 mV at 0.28 A; 1V1 to pin 23 through 6 mm of 0.25 mm wire ≈ 12 mΩ (≈ 1–2 mV) | No; fixed by A1 anyway |
| Thermal | LDO ≈ +3 °C over the datasheet case (B3) | Spreadsheet estimate suffices |
| Aggressor/victim script | Run (N8); nothing to resolve | — |

## 5. Verdicts

- **Gate 7 (routing): not yet.** DRC, parity, lengths, pad joints, via-in-pad, plane integrity and current capacity all pass, and review 2's A1/A2 are fixed in substance. What blocks is the regulator corner: A1 (a GND via severs the 1V1 ring; pin 23's spoke hangs on 6 mm of 0.25 mm L4 wire) and A2 (PGND joined to the plane by one via outside its zone). Both are one dru rule plus a two-via stitch step and a re-route; take B1, B2, B4 and B5 in the same pass.
- **Gate 8 (SI): not yet.** No simulation is needed for USB, QSPI, crystal, power or thermal (estimates above). The regulator waiver depends on the corner matching the reference, which it will after A1/A2; restate the trigger table and the waiver against the re-routed corner (render it beside the reference), and rewrite phase8.md, which still describes the 2026-10-24 board.
- **Gate 9 (silkscreen): passes as drawn** (labels, pin map, identity, warnings, pin-1 marks, no overlaps or silk over vias all verified), **but is route-dependent**: re-run the silk stage and silk_check after the gate-7 re-route and re-check the renders.
## Re-check on ultraroute fe903ff (seed 0), 2026-11-02

The route above was re-done with the reference zones protected by `.kicad_dru` rules (`board.py regzones`), the vendor vias placed by `refvias`, and USB at the chip on its own class.

- A1: the 1V1 ring ("transplant 3 +1V1") fills as one piece, and no GND via is inside it. Resolved.
- A2: PGND ("transplant 2 GND") has 2 GND vias inside its outline. Resolved.
- B2: COUT's second +1V1 via is still left out on purpose, because it blocks USB_DM_MCU. Logged in decisions.md.
- B1, B3, B4, B5 are not re-reviewed yet.
- New note: inner GND (L2) has a 0.77 mm² piece at the bottom edge. It is attached to J4.2 (GND) but cut off from the rest of the plane by the clearance around a VLED via. It is tied to GND, not floating.
- Checks:
  - DRC: 0 errors, 0 unconnected, parity 0; 56 lib_footprint_mismatch warnings.
  - 0 duplicate segments; nothing locked except the 16 footprints.
  - pad_joints, silk_check, `nets_check --rules --routed`, placement_check and sync_check all pass.
  - Plane pads with a via of their own within 1.0 mm: 47 of 74.
  - USB: DP_C 7.69, DM_C 8.01, DP_MCU 3.91, DM_MCU 3.79 mm.
