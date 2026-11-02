# Independent layout review 4 — easy2350 rev A (gates 7, 8, 9)

Board: `/home/sequoia/Software/easy_pcb/examples/easy2350/hw/easy2350/a/easy2350.kicad_pcb` at commit 7410554 (ultraroute fe903ff, seed 0). Every number below is my own measurement with KiCad 9.0.8 `kicad-cli` / `pcbnew`; scripts, dumps and drawings are in `/tmp/claude-1000/-home-sequoia-Software-easy-pcb/7a2c7658-b6df-4f87-85c2-d6de8594fb26/scratchpad/review4/` (`dump.py`, `ref.py`, `geo.py`, `corner.py`, `power.py`, `decap.py`, `ldo.py`, `usb.py`, `planes.py`, `silk.py`, `av.py`, `joints.py`, `misc.py`, `corner_plot.py`; outputs `*.txt`; drawings `corner_F.png`, `corner_B.png`, `corner_inner.png` — ours beside the reference, same window, same scale). Coordinates are board mm from the outline's top-left corner (file origin 99.975, 99.975; outline 21.05 × 51.05); "rel" is mm from U1's centre (10.525, 26.525). Reference = `RP2350_60QFN_minimal.kicad_pcb` (2-layer), its U1 at (100, 100). No project file was edited.

## 1. Baseline

| Item | Measured |
|---|---|
| `kicad-cli pcb drc --severity-all --schematic-parity` | **0 errors, 0 unconnected, 0 parity issues**; 56 warnings, all `lib_footprint_mismatch` (board-local footprints). No dangling-track or isolated-copper warnings. |
| Vias | **143**: GND 57, +3V3 13, +1V1 8, VBUS 3, VLED 2, signals 60. 83 × 0.6/0.3, 60 × 0.5/0.2 |
| Tracks | 621 segments (F.Cu 437, B.Cu 184); In1/In2 planes only. 0 duplicate segments. **75 segments shorter than 0.1 mm** (31 on +3V3 at pin 53, 9 each on USB_DP_C / USB_DM_C), 25 shorter than 0.02 mm — router leftovers, cosmetic |
| Locked | 0 tracks, 0 vias, 0 zones; 16 footprints (J1–J4, U1, C14, C15, L1, TP3, SW1, SW2, D1, D3–D6) |
| Renders / fab | `design/renders/*.png`, `renders/gerbers/*.png`, `hw/easy2350/a/fab/*` all written 2026-11-02 08:02–08:03, after the .kicad_pcb (08:00:59); all committed in 7410554 |
| `pad_joints.py` | PASS, 205 SMD pads, 0 thin joints |
| `silk_check.py` | PASS, 58 texts, 0 errors, 0 warnings |
| `nets_check.py --rules --routed` | PASS. USB_DP_C 7.69, USB_DM_C 8.01, USB_DP 10.83, USB_DM 11.01 (≤ 20); USB_DP_MCU 3.91, USB_DM_MCU 3.79 (≤ 5, 0 vias); QSPI 5.40–14.53 (≤ 25; SCLK/SD0/SD3 2 vias, SD1/SD2/SS 0); XIN 8.19 (≤ 10, 0 vias); XOUT 2.95, XOUT_R 2.03 (≤ 5); VREG_LX and VREG_AVDD zone copper only, 0 tracks, 0 vias |
| `placement_check.py` | PASS |
| `plane_pad_vias.py GND,+3V3 1.0` | 47 of 74 plane-net pads have a via of their own within 1.0 mm (the 27 without: 10 U1 power pins fed by zones, the PGND pair's shared vias, and pads at 0.71–1.73 mm — see N3) |
| Corner zones vs reference (`corner.py`, fills compared after shifting the reference onto our U1) | 1V1 ring 5.727 vs 5.728 mm² (1 outline, sym. diff 0.001); COUT 2.698 vs 2.740 (ref-not-ours 0.042); LX 2.266 vs 2.317 (0.051); CIN 0.585 vs 0.586; PGND 1.757 vs 1.751; 3V3 band 8.281 vs 8.635 (ref-not-ours 0.359 in five 0.05–0.12 mm² bites at the pin-track entries, ours-not-ref 0.005). Outlines, priorities, 0.15 mm clearances, min widths and pad-connection modes identical |
| Corner vias vs reference | 8 of the 9 reference +1V1 vias present within 0.01 mm (N (0.49,−2.15)/(0.50,−5.80); S (0.20,2.15)/(0.20,5.80); W (−2.15,−0.80)/(−5.20,−1.00); E (2.15,−0.40)/(5.80,−0.40)); **(0.50,−5.20) absent** (logged 2026-11-02). PGND GND vias at (3.10,−4.90) and (3.10,−5.50): present, both inside the PGND fill, 100 % L2 copper round each. CFILT via (4.20,−3.86) vs ref (3.85,−3.85). Decoupler vias at (−2.10,−8.60), (−1.80,7.30), (1.60,7.40), (6.90,4.00) exact; ref (−7.70,…), (7.60,−3.50), (8.30,…) present within 0.04–0.74 mm |
| LX cut-out | LX fill over L2 copper **0.000 mm²**; L2 copper inside the cut-out area 0.000; L1 pad 1 (1V1) fully over L2 GND (1.190 of 1.190 mm²), pad 2 (LX) 0.000; L3 3V3 solid under the cut-out (6.07 of 6.17 mm²) |
| Under L1's courtyard (5.20 mm²) | F.Cu: LX zone 1.426 + 1V1 zone 1.322, nothing else; B.Cu: GND pour 1.944 + **VBUS track 0.206 + BOOTSEL (Net-(R7-Pad2)) track 0.213** |
| 1V1 ring interior | F.Cu: no foreign track or via; B.Cu under the ring outline: VBUS 1.34 mm², BOOTSEL 0.52 mm² of track (whole U1 courtyard on L4: VBUS 9.1 mm, BOOTSEL 8.9 mm, GPIO29 3.4, GPIO16 2.4) |
| F.Cu power connectivity (`power.py`, union of fills+tracks+pads) | +3V3: U1 pins 11/20/30/38/49/53/54 + band + CIN + C5/C6/C7/C8/C11/C14 in one 50.6 mm² piece fed by **5 vias**; pin 1 piece 2 vias; pins 44/45 piece 1 via. +1V1: ring piece (4 vias); pin 23+C16+S via; pin 39+C18+E via; pin 6+C17+W via; **COUT+L1.1+C15.1+pin 50 (FB) one piece with the N via**. GND: PGND piece holds exactly U1.47, C14.2, C15.2 and the two vias |
| Decoupler GND pad → nearest GND via in its own copper (`decap.py`) | all ≤ 1.73 mm: C4 0.71, C17 0.71, C5 0.71, C9 0.71, C13 0.71, C18 0.71, C11 0.72, C7 0.73, C6 0.74, C16 0.80, C2 0.88, C14 0.66, C15 0.59, C21 0.71, C22/C1/C3 0.71–1.13, **C20 1.73**, C24 1.31, D3.2 2.37 |
| LDO (`ldo.py`) | EP 1.5 × 2.25 with **3** thermal via pads 0.55/0.25; GND vias within 2/3/4/5 mm of the EP centre: 2/5/6/8; EP in the 366.7 mm² top GND piece (23 vias); L1 GND within 5 mm 40.4 of 78.4 mm²; L2 GND within 5 mm 65.6 of 78.4 mm² (void = J3 pin antipads + the USB_DP/VBUS via cluster at (12.8–14.8, 9.8–11.6)); C2 (in) 1.73 mm from pin 8, C3 (out) 2.03 mm from pin 1; VBUS/3V3 at the pins 0.25 mm tracks (+ the 0.51 mm² `plane 3V3 LDO out` zone); NC pins 2/3/6/7 bare; U3 10.7 mm from J1, 24.4 from Y1, ≥ 28 from the pixels |
| USB (`usb.py`) | DP_C/DM_C: F.Cu 4.34/3.39 + B.Cu 3.35/4.62 mm, 2 vias each (nearest GND via 0.72/2.47 and 2.30/4.20 mm), **coupled (edge gap ≤ 0.3) only 0.25 of 7.69 mm**; DP/DM: 2 vias each (GND via 1.70–2.49 mm), coupled 4.08 of 10.83 mm at 0.15–0.22 mm gap; DP_MCU/DM_MCU: F.Cu only, 0.15 mm, coupled 3.52 of 3.91 mm at 0.20–0.25 gap. Plane under the tracks continuous except each via's own 0.9 mm antipad |
| Planes (`planes.py`) | L2 GND 806.9 mm² (75.4 %), **one piece** + a 0.77 mm² slice of J4.2's own pad ring at the bottom edge (not floating: it is J4.2's copper, cut off by the VLED via (9.28,49.36) clearance). Largest voids: 7.61 mm² (LX cut-out merged with the R3 3V3-via and BOOTSEL-via antipads, (12.08–14.98, 17.41–23.83)), header-pin antipads 3.48 each, 4.59 (GPIO8–13 via cluster, (4.5–7.1, 27.7–31.8)). L3 3V3 763.3 mm², one piece. L4 GND 11 pieces, every one with ≥ 1 GND via or THT pad. L1 GND 19 pieces: **one floating sliver 0.03 mm² at (14.10–14.38, 22.37–22.76)**; every other piece has a via or a pad |
| Aggressor/victim (`av.py`, same layer, edge gap ≤ 0.3 mm) | QSPI_SS‖SD1 3.80 mm @ 0.127; SD0‖SCLK (B.Cu) 2.52 @ 0.159; SD3‖SCLK 2.21 @ 0.125; SD1‖SD2 1.93 @ 0.172; XIN‖XOUT 2.31 @ 0.25 (pin pitch). LX fill to XIN 7.15 mm, XOUT_R 11.0, QSPI_SCLK 3.10, USB_DM_MCU 1.10, USB_DP_MCU 1.50 |
| Silk (`silk.py`, renders) | 58 texts, all 0.8 mm / 0.15 mm; **40 of 40 header labels match their pad nets** (each 1.25 mm from its pad; RUN 1.30); "DIO GND CLK" centred on J4 (x 10.53), reads correctly from below (DIO over x 13.07 = SWDIO); identity "easy2350 rev A", "2026-10", "github.com/ sequoia-hope/ easy_pcb", "CERN-OHL-P-2.0"; limits "3V3 out 100mA", "VBUS out 50mA"; warnings "no VBUS in / with USB in", "3.3V logic only"; TP3 "1V1"; top "5V IN", "25", "BOOT", "SWD", "RST"; pin-1 / polarity dots visible in `top_silk.png` and `top.png`. No silk over a via in the renders |
| Gate files | `phase7.md`, `phase8.md`, `phase9.md` still carry the 2026-10-24 evidence under the 2026-10-31 "stale" banner; `status.json` gate 7/8 stale reasons still cite review 2 (2026-11-01) and ultraroute 770ef73 |

## 2. Findings

### BLOCKER
None.

### MAJOR
None. (Review 3's A1 and A2 are resolved; nothing measured on this route is electrically significant enough for MAJOR.)

### MINOR

**M1. The USB connector pair (USB_DP_C / USB_DM_C) is not routed as a pair, and D− carries a router staircase.**
- Location: J1 (10.5, 3.2) → U2 (11.2, 10.6). DM_C on B.Cu: (10.36,6.22)→(10.36,6.82)→(9.80,6.83)→(9.80,7.43)→(10.36,7.42)→(10.37,8.10): a 2.4 mm zig-zag that advances 1.9 mm. DP_C vias at (9.86,8.30) and (9.11,11.34); DM_C vias at (10.36,6.22) and (11.06,9.56) — the two lines change layer 2–3 mm apart.
- Measured: coupled length 0.25 of 7.69 mm (edge gap ≤ 0.3); 2 layer changes per line; nearest GND via to the four signal vias 0.72 / 2.47 / 2.30 / 4.20 mm; lengths 7.69 vs 8.01 (0.32 mm skew).
- Why it matters: `usb-front-end.md` asks for "90 Ω differential pairs, matched within 1 mm, no layer change except the D+/D− join at J1". At full speed (12 Mb/s, ≥ 4 ns edges, ≈ 50 ps of trace) none of this is measurable, and `stackup.md` already logs the pairs as "mostly uncoupled"; but the part note is not met and no decision says so.
- Fix (cheap): route DP_C/DM_C with `--diff-pairs` from the J1 pads (J1→U2 is 3.5 mm straight on F.Cu), or log in decisions.md that the J1→U2 and U2→R4/R5 segments may change layer and run loosely coupled at FS. Pull-tight the staircase either way.

**M2. CIN (pin 49, C14.1) is still fed through one 0.25 mm track, plus a 0.09 mm stub (previous B5, open).**
- Location: 3V3 via (10.24, 23.88) = rel (−0.29, −2.65) → 0.25 mm F.Cu track (0.05,−2.89)→(0.22,−2.71)→(1.51,−2.71) along the band's top edge into the CIN zone; a second 0.09 mm track (1.68,−2.91)→(1.60,−3.44) into pin 49. Two more 0.09 mm stubs run into pin 53 from (0.04,−2.91)/(0.04,−2.93).
- Measured: the CIN zone touches C14.1 (0.227 mm²) and U1.49 (0.173 mm²); its only feeds are the 0.25 mm track (≈ 1.9 mm from the via) and the 0.09 mm stub. The reference feeds CIN from its 985 mm² top 3V3 pour.
- Why it matters: DC is fine (≤ 0.15 A average; 0.25 mm × 1.9 mm ≈ 4 mΩ) and the ripple is supplied locally by CIN; the 0.09 mm stubs are at the fab minimum and redundant with the zone. A robustness gap relative to the reference, not a fault.
- Fix: a tongue of the L1 3V3 pour (`plane 3V3 L1 around U1`) to the CIN zone's right edge, or a 0.6/0.3 3V3 via beside C14.1 if the regulator-corner rule admits it; delete the three 0.09 mm +3V3 stubs (a `track_width (min 0.2mm)` rule for +3V3 on F.Cu stops the router laying them).

**M3. LDO exposed pad: 3 thermal vias, part note asks ≥ 5 (previous B3 / m5, open; unchanged since gate 6).**
- Location: U3 EP (15.625, 12.625).
- Measured: 3 via pads 0.55/0.25 in the EP; 5 GND vias within 3 mm, 6 within 4 mm; EP in the 366.7 mm² top GND piece with 23 vias (note item (b) ≥ 50 mm² / ≥ 4 vias met); L2 solid within 5 mm except 12.8 mm² of antipads. C2 1.73 mm and C3 2.03 mm from pins 8/1 (note asks ≤ 1.5; `placement_check` passes them). VBUS to pins 5/8 is 0.25 mm (note asks ≥ 0.4; `nets.csv`/dru need 0.2).
- Why it matters: `ldo-3v3.md` budgets ≈ 7 °C/W for five vias; three give ≈ 12 °C/W, ≈ +3 °C at 0.61 W. Works; the note's own number is not met.
- Fix: two more 0.25 mm drills in the EP footprint copy, or log the 3-via EP in decisions.md with the +3 °C.

**M4. Gate paperwork is for a different board.** `phase7.md`, `phase8.md`, `phase9.md` describe the 2026-10-24 route (Freerouting, locked copper); `status.json`'s gate 7/8 "stale_because" strings cite review 2 and ultraroute 770ef73; `design/sim/si-review.md` (phase 8's evidence) was written on the old route. Fix: rewrite the three gate files with this route's evidence (DRC, scripts, the corner comparison, the waiver) and update `status.json`.

**M5. One COUT via instead of the reference's two (previous B2; logged, accepted here).** The N spoke is fed by a single 0.6/0.3 via at (11.03, 20.73) = rel (0.50, −5.80); the reference adds (0.50, −5.20). All regulator output current (≤ 0.2 A) crosses that one via: ≈ 1 mΩ, ≈ 1 A capacity — fine. Keep the decision; state it in the gate-8 waiver.

### NOTE

- N1. **Regulator corner matches the reference** (`corner_F.png`/`corner_B.png`/`corner_inner.png`): every zone within 0.05 mm² (band 0.36), every via position within 0.01 mm, the LX zone wholly inside the L2 cut-out, L1 pad 1 over solid L2, PGND two vias in the fill on solid L2 (the hull between the LX pads is no longer cut, as decided 2026-11-02), CFILT with its own via 0.71 mm from C13.2, AVDD as a top zone only (R3.2, C13.1, U1.46; 0 vias, 0 tracks), FB (pin 50) joined to COUT by the vendor strip (same copper piece), TP3 at the pin-23 end (0.83 mm of 0.25 mm B.Cu from the S via), no foreign copper on F.Cu inside the ring or the LX/PGND/CIN/COUT zones. Differences from the reference: M5; B.Cu spokes are 0.6 mm zones instead of 0.6 mm tracks (same copper); 0.3 mm drills vs 0.25; the 4-layer stack puts solid L3 3V3 under the cut-out and L4 traffic under the chip (N2).
- N2. **L4 under U1 and L1**: VBUS (9.1 mm) and BOOTSEL (8.9 mm) run on B.Cu under U1's body, between the EP thermal vias, and 0.21 mm² of each lies under L1's courtyard. L2 GND is solid there (EP via pads join L2 over 100 % of their perimeter) and L3 3V3 is solid under the whole corner, so the coupling to the 1V1 ring and to LX is through two planes — negligible. The `U1 pad ring` rule is F.Cu-only by design (decisions 2026-11-01); if the chip note's "no foreign net crosses inside the pad ring" is meant for all layers, say so there.
- N3. **Decoupling**: every capacitor ground reaches a via in its own copper within 0.88 mm except C20.2 (crystal load cap, 1.73 mm through the 91.8 mm² pour — unchanged), C24.2 (1.31) and D3.2 (2.37, a pixel LED ground). C4/C17 (review 3 B1) fixed: 0.71 mm each to a via of their own.
- N4. **Floating copper**: a 0.03 mm² GND sliver on L1 at (14.10–14.38, 22.37–22.76), between C13 and the AVDD zone, with no via or pad. DRC does not flag it. Set the L1 GND zone's island removal to "below area" (e.g. 0.1 mm²) or remove it by hand.
- N5. **QSPI**: SCLK 14.53 mm with 2 vias (GND return vias 2.66/3.92 mm away); SD0 and SD3 also 2 vias; SD1/SD2/SS 0. Spread 5.40–14.53 mm (inside `flash.md`'s ± 5). Plane continuous apart from the vias' own antipads (SCLK ≈ 0.9 mm over other voids). Fine at the slow-slew default; `flash.md`'s "no vias in SCLK if avoidable" was not achieved.
- N6. **3V3 feed to the chip**: band, CIN and pins 11/20/30/38/49/53/54 are one copper piece with 5 vias; the via inside the pins-53/54 zone (−0.29, −2.65) carries the band's share — fine for 0.28 A.
- N7. **VBUS**: main path J1 → U3.8 and J1 → J3.20 on F.Cu at 0.25 mm; the LED branch hops L1→L4 at (13.34,10.30)/(14.32,10.37) (a 0.91 mm B.Cu hop between two vias) and back at (4.98,30.60): three single vias on a ≤ 0.2 A branch (was five). The track end at (14.22,10.65) sits 0.299 mm from the via centre (ring radius 0.30): connected over the full track width, by 1 µm — a snap would be tidier.
- N8. **Router artefacts**: 75 segments < 0.1 mm (31 on +3V3 fanning into the via at (10.24,23.88) and pin 53; 9 each at the USB_DP_C/DM_C vias). Cosmetic.
- N9. **Silkscreen** verified pin by pin (40/40), identity/limits/warnings present, all text 0.8/0.15 mm, no overlap, no silk over vias (silk_check and the renders agree). Top silk is sparse by policy (designators none) but carries the function labels and pin-1/polarity dots. "not Pico pinout" absent (logged 2026-11-01 as optional).
- N10. **Author's appended re-check (review-layout-2026-11-02.md, "Re-check on fe903ff")** verified: A1 resolved (ring one outline, no GND via inside), A2 resolved (two GND vias inside PGND), B2 logged, the L2 0.77 mm² piece is J4.2's own pad copper, DRC/duplicates/locks/scripts as stated, plane pads 47/74, USB 7.69/8.01/3.91/3.79. `sync_check` not run by me.

## 3. Review-3 findings re-verified on 7410554

| # | Finding | Verdict now | Measurement |
|---|---|---|---|
| A1 | GND via severed the 1V1 ring; S spoke on 6 mm of 0.25 mm wire | **Resolved** | ring fills as 1 outline (5.727 mm²) holding exactly the four spoke vias; no foreign copper on F.Cu inside it; pin 23 fed by the S spoke zone (0.6 × 3.65 mm) and the vendor pin strip; the only +1V1 track on the board is the 0.83 mm TP3 stub |
| A2 | PGND one router via outside its zone | **Resolved** | two 0.6/0.3 GND vias at (3.10,−4.90)/(3.10,−5.50), inside the PGND fill, 100 % L2 copper round each; PGND piece holds only U1.47, C14.2, C15.2; no GND track at pin 47 |
| B1 | C17/C4 grounds 5.3/3.2 mm of track | **Resolved** | C4.2 0.71 mm and C17.2 0.71 mm to a via in their own pour pieces (7.0 and 1.2 mm²) |
| B2 | one COUT via | **Open, logged** (M5) | via (0.50,−5.80) present, (0.50,−5.20) absent; decisions.md 2026-11-02 |
| B3 | LDO EP 3 vias, 5 GND vias ≤ 3 mm | **Open** (M3) | unchanged: 3 / 5 / 6 within EP / 3 mm / 4 mm |
| B4 | LX track inside its zone | **Resolved** | 0 VREG_LX tracks; zone only (2.266 mm²) |
| B5 | CIN fed by one 0.25 mm track | **Open, changed** (M2) | still one 0.25 mm track (≈ 1.9 mm), plus a new 0.09 mm stub into pin 49 |
| N1–N9 (review 3) | corner, band, joints, via-in-pad, VBUS, planes, foreign nets, A/V, silk | all re-measured above; via-in-pad still none; VBUS LED branch now 3 single vias (was 5); L2 largest void 7.61 mm² (was 9.8); foreign nets under U1 on L4 now VBUS 9.1 + BOOTSEL 8.9 + GPIO29 3.4 + GPIO16 2.4 mm (was BOOTSEL 7.4 + RUN 0.6) |

## 4. Gate-8 trigger assessment (simulation.md)

| Trigger | This route | Simulation needed? |
|---|---|---|
| DIFF_HS (USB) | 7.7–11.0 mm per segment (≤ 20), FS 12 Mb/s; two layer changes per line on the C and mid segments, loosely coupled (M1) | No: lumped at FS (≈ 50 ps of trace against ≥ 4 ns edges); log M1 |
| SE_HS (QSPI) | 5.4–14.5 mm ≤ 25; SCLK/SD0/SD3 2 vias; return vias 2.7–4.4 mm; plane continuous bar own antipads | No, at the slow-slew default |
| CLOCK (XIN) | 8.19 mm L1, 0 vias, L2 solid under Y1/C20/C21; 7.15 mm from the LX fill; only XIN/XOUT_R/GND inside the crystal area | No; bring-up XOSC check stands |
| SWITCH near CLOCK/ANALOG | LX fill to XIN 7.15 mm; to AVDD adjacent as the reference (PGND between); to USB_DM_MCU 1.10 mm (as the reference's pin-51 track) | No |
| Switching converter (RP2350 VREG) | Corner = reference in zones (≤ 0.05 mm² each, band 0.36), all via positions, cut-out, CFILT, AVDD, FB, PGND; deviations: one COUT via (M5), 0.3 mm drills, L3 3V3 under the cut-out and L4 tracks under U1/L1 behind solid L2 (N2) | **A waiver on "copper matches the validated reference" is now honest**, provided it names those deviations and cites the corner drawings beside the reference |
| POWER drop | 3V3 through the L3 plane and 5 vias ≈ 2 mV at 0.28 A; 1V1 entirely in reference copper | No |
| Thermal | LDO ≈ +3 °C over the datasheet case with 3 EP vias (M3) | Spreadsheet suffices |
| Aggressor/victim | run (`av.py`); QSPI mutual only, XIN‖XOUT at pin pitch | — |

## 5. Verdicts

- **Gate 7 (routing): pass on the copper.** DRC 0/0/0, parity 0, pad joints, lengths, via-in-pad, plane integrity, current capacity and the regulator corner all check out; review 3's A1/A2 and B1/B4 are fixed. Take M1 (USB C-pair) and M2 (CIN feed + 0.09 mm stubs) and N4 (sliver) in a cleanup pass or log them; M3 is a footprint/decision item; the gate cannot be marked passed until `phase7.md` is rewritten for this route (M4).
- **Gate 8 (SI / regulator): not yet, for paperwork only.** No simulation is needed (table above). The regulator waiver is justified by the measured match to the reference; write it in `decisions.md`/`phase8.md` with the deviations (one COUT via; L4 traffic under U1 behind L2/L3; 0.3 mm drills) and the corner drawings beside the reference, and retire `si-review.md`'s old-route numbers.
- **Gate 9 (silkscreen): pass as drawn.** 40/40 labels, identity, limits, warnings, pin-1 marks, no overlaps, no silk over vias; silk_check PASS. Rewrite `phase9.md` for this route and re-run silk_check if the gate-7 cleanup moves any via.

## Resolution (author, 2026-11-02, ultraroute 44dd222, seed 2)

| # | Resolution |
|---|---|
| M1 | Logged in decisions.md. The pin order forces D+ across D− between J1 and U2, and the link is full speed. ultraroute's fix/pair-layer (two matched F.Cu tracks, 0 vias) isn't merged yet; re-route when it lands |
| M2 | Logged in decisions.md. No room for a second via at C14.1 after routing. The 0.09 mm +3V3 stubs are gone on ultraroute 44dd222 (copper inside its own net's kept pour is no longer bridged) |
| M3 | Fixed: U3's project footprint has 5 EP vias (0.25 mm drill) in one column at 0.55 mm pitch, KiCad's paste unchanged |
| M4 | Gate files 7-9 to be rewritten on the final route |
| M5 | Already logged; stated in the gate-8 waiver |

Re-check after re-routing:
- DRC: 0 errors, 0 unconnected, parity 0; 56 lib_footprint_mismatch warnings.
- Ring 1 piece holding its 4 +1V1 vias; PGND 2 GND vias inside; LX 1 piece.
- L2 and L3 one piece each.
- 0 duplicate segments; 0 +3V3 tracks under 0.15 mm; 61 segments under 0.1 mm.
- pad_joints, silk_check, `nets_check --rules --routed`, placement_check and sync_check all pass.
- 48 of 74 plane pads have a via of their own within 1 mm.
