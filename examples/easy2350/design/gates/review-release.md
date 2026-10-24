# Release review: easy2350 rev A (gate 10)

Reviewer: release review agent, fresh context, 2026-10-23. Inputs: the whole
`design/` package, `hw/easy2350/a/` (schematic, board, project, fab outputs),
the renders and the gerber renders. Every check below was re-run, not copied
from the gate files. Coordinates are board mm from the top-left corner, USB-C
at the top (brief.md frame); `file:line` where a document is the finding.

## Verdict

**Not yet. One ordering BLOCKER (L1 stock = 1, already known) and four MAJORs,
all cheap; nothing found that is wrong in the circuit.** The design, the
datasheet work and the RP2350 power-block transplant are sound, the fab files
are clean and self-consistent, and I would send this board after:

1. the user decides how the five L1 inductors are sourced (BLOCKER, order
   cannot be placed as drawn);
2. the BOOTSEL switch track is moved off the board edge / out from under the
   J2 solder joints, or that risk is waived in writing (MAJOR 1);
3. bringup.md's VLED numbers are corrected for the SM4007PL (MAJOR 2);
4. an assembly drawing a person can read is generated (MAJOR 3);
5. the CC-pin ESD gap against intent.md is waived in decisions.md (MAJOR 4).

Items 2–5 are documentation or a short re-route; none changes a part.

## Checks run

| Check | Command / method | Result |
|---|---|---|
| DRC, all severities | `kicad-cli pcb drc --severity-all` on `easy2350.kicad_pcb` | 53 violations, all `lib_footprint_mismatch` warnings (board-local silk trims and project-library copies); **0 errors, 0 unconnected** |
| DRC with schematic parity | `--schematic-parity` added | **0 parity issues** |
| ERC, all severities | `kicad-cli sch erc --severity-all` | **0 violations** (1 sheet) |
| Sync schematic = PCB = BOM = CPL | `sync_check.py hw/easy2350/a --bom … --cpl …` | PASS: 58 symbols / 58 footprints / 54 BOM / 54 CPL (J2, J3, J4, TP3 excluded as intended) |
| Net rules and routed lengths | `nets_check.py --rules --routed` | PASS, 0/0; XIN 9.86 mm ≤ 10, 0 vias; XOUT 2.95, XOUT_R 4.09; USB_*_MCU 4.10/3.86 ≤ 5; QSPI 7.15–16.98 ≤ 25 |
| Silk | `silk_check.py` | PASS, 0/0 (58 texts, pin-1 marks present, no silk over vias) |
| Placement | `placement_check.py` | PASS; every decoupler ≤ 4.1 mm from its pin; U3–Y1 20 mm, U3–pixels 26 mm |
| Stock, every LCSC number | `jlc_parts.py show` (25 parts), this session | All lifecycle normal. **C42411119 (L1): stock 1.** Lowest others: AP7361C 1460, ABM8 4014, RP2350A 5789 |
| Pin maps vs datasheets | pcbnew pad nets vs RP2350 Fig. 152, AP7361C p.2 (DFN: 1 OUT, 4 GND, 5 EN, 8 IN, EP GND), WS2812B-2020-V6 p.2 (1 DO, 2 GND, 3 DI, 4 VDD), USBLC6-2 Fig. 1, SN74LV1T125 (1 ~OE→GND) | All 61 U1 pins, U3, U4, U2, U5, D3–D6 correct; D1/D2/D7 cathodes on pad 1; Y1 crystal pads 1/3, GND 2/4 |
| Header pin map | pcbnew J2/J3 pad nets vs user-io.md table vs bottom silk | 40/40 match; bottom labels mirrored correctly; J4 "DIO GND CLK" is the correct order seen from below |
| Gerbers (independent parse) | gerbonara renders in `design/renders/gerbers/` + my own extent/aperture parse of each file | Outline closed (8 objects, 21.05 × 51.05); paste on top SMD pads only (215 flashes, 0 on bottom); bottom mask opens only 40 + 3 + 4 THT pads, TP3 and 2 NPTH; vias tented both sides; NPTH = 2 × Ø0.65 J1 pegs; PTH tools 0.2 / 0.25 / 0.3 / 0.6 (slots, G85) / 1.0 mm; 4 shell slots exported as G85 slots |
| CPL sanity | file read | 54 rows, all Top, KiCad absolute origin, six rows with negative rotation (−90) |
| Height | footprints vs intent.md 3.5 mm | Tallest non-exempt part is C3 0805 (≈ 1.3 mm) / U5 SOIC (1.75 mm). OK |
| VBUS attach capacitance | BOM | 2.3 µF direct + 5.2 µF behind D2 = 7.5 µF nominal ≤ 10 µF |
| Edge / pad proximity | pcbnew scan of all segments within 0.9 mm of the long edges | **BOOTSEL net runs 14 mm along the left edge at x 0.4–0.6 (MAJOR 1)**; GPIO28 5.8 mm at x 20.45 on L4 |
| L2 cut-out under LX | zone list | Rule area 1.8 × 5.5 mm on In1.Cu at the LX zone; L2 otherwise one plane (808 mm²); L3 one 3V3 plane (768 mm²) |
| U3 thermal copper | pcbnew | 3 EP vias (0.25 drill) + 9 GND vias within 3.5 mm of U3 — ldo-3v3.md's "≥ 4 more" met |

## Findings

Severity: BLOCKER = cannot order; MAJOR = fix or waive in writing before
ordering; MINOR = fix now if cheap, else rev B; NOTE = recorded.

| # | Sev | Finding (location) | Suggested fix |
|---|---|---|---|
| 1 | **BLOCKER** (to the order, not the design) | **L1 AOTA-B201610S3R3-101-T, C42411119: JLC stock 1** (re-checked this session; 191 at phase 1). Five assembled boards need five. decisions.md and phase10.md record it and defer to the user; the release checklist item "stock re-checked" therefore fails until a decision exists. No substitute: DS p.412 requires this polarity-marked part. | Pick one and log it: (a) JLC global-sourcing / pre-order of the same MPN (adds lead time); (b) leave L1 DNP at JLC and hand-fit five pieces from Mouser/DigiKey (2016 case, two pads, easy; pad 1 = 1V1 end gets the white dot); (c) wait for restock. (b) is the fastest for five boards and keeps the validated part. |
| 2 | **MAJOR** | **BOOTSEL switch net (`Net-(R7-Pad2)`) runs 14 mm along the left board edge on L1, outboard of the J2 pin row**: x = 0.6 from y 24.95 → 34.55 and x = 0.4 from y 22.65 → 24.75 (0.35–0.55 mm from the routed edge; 0.14 mm from the J2 pad copper of pins 9–14, i.e. the header pads the user hand-solders on this side, since J2 mounts from the bottom). DRC passes at the 0.10 mm rule, but JLC's outline tolerance is ±0.2 mm and the solder-mask dam between the pad opening and the track is 0.14 mm nominal. A bridge to pin 13 (GND) holds the board in BOOTSEL; a bridge to a GPIO pin drives the button net; an edge nick kills the BOOT button. GPIO28 does the same for 5.8 mm at x 20.45 on L4 (0.13 mm from J3's pads, but on the un-soldered side). | Re-route the R7 → SW1 link inboard of the header row (it is a slow net; the L4 channel beside the GPIO6–10 fan-out has room, or drop it to L4 at R7 and run under the F.Cu pour). Keep ≥ 0.3 mm from header pads and ≥ 0.5 mm from the edge. If the 45-min route cycle is not worth it for rev A, waive it in decisions.md and add "inspect the J2 pin 9–14 solder joints for bridges to the edge track" to bringup.md step 0. |
| 3 | **MAJOR** | **bringup.md has the wrong VLED numbers (Schottky figures; D2 is the SM4007PL silicon rectifier).** `design/bringup.md:24` expects "VLED = VBUS − 0.4…0.5 V" at idle; with ≈ 1 mA bleed the SM4007PL drops ≈ 0.5–0.65 V (rgb-leds.md: 0.475 V lower bound, ≈ 0.52 V typ per review1-io). `bringup.md:52` expects "VLED stays ≥ 4.3 V" at full white on a 500 mA port; at 160 mA the diode drops ≈ 0.85–1.0 V, so a good board reads ≈ 3.9–4.2 V and *fails the step*. `design/architecture.md:9` still names "D2 B5819W". The plan is the gate deliverable; a wrong pass criterion is worse than none. | bringup.md step 1: "VLED ≈ VBUS − 0.5…0.65 V (idle)"; step 4: "VLED ≥ 3.5 V at full white with VBUS ≥ 4.5 V at pin 40; expect ≈ 4.0–4.2 V from a 5.0 V port"; add the rgb-leds.md Open-question-1 test (VBUS 5.50 V idle → VLED ≤ 5.27 V; 4.50 V full white → VLED ≥ 3.5 V). Fix architecture.md. Also say "probe C2's VBUS end" rather than "U3 pin 8" (a 0.25 × 0.9 mm DFN pad). |
| 4 | **MAJOR** | **No usable assembly drawing.** `design/renders/assembly-top.pdf` plots the 21 × 51 mm board at about 1:1 on A4 landscape; the fab-layer designators are ≈ 0.4 mm and unreadable. The silk policy "none" (decisions.md) was justified by "designators on the fab layer for the assembly drawing", and bringup.md refers to parts by designator (U3 pin 8, C23, R6, TP3). With no designators on the board and no legible drawing, bring-up and any rework start with guesswork. | Re-plot the F.Fab + F.Cu pad outlines at 4:1 or 5:1 (`kicad-cli pcb export pdf --layers F.Fab,F.Cu,Edge.Cuts --scale 5`), or render a labelled PNG via `render.py` with the fab layer on, into `design/renders/assembly-top.{pdf,png}`; link it from bringup.md step 0. |
| 5 | **MAJOR** (waiver acceptable) | **intent.md:25 requires ±8 kV contact ESD "on every pin" of J1; CC1 and CC2 have none.** U2 protects D+, D− and VBUS; CC1/CC2 go only to R1/R2 (5.1 kΩ 0402, 50 V) at (12.93, 13.93) / (9.62, 13.62). A strike on CC lands on the resistor; if it opens, a C-to-C source never sees Rd and the board is dead on USB-C hosts (A-to-C cables still work). usb-front-end.md says "U2 is the protection for every signal pin", which is not true for CC. Not in decisions.md. | Either waive in decisions.md ("CC pins unprotected: nothing but Rd on the net; failure mode = lost Rd, not chip damage; Pico-class boards do the same") and amend usb-front-end.md, or add a 2-ch low-capacitance ESD array on CC1/CC2 in rev B (no room now). |
| 6 | MINOR | **"Not Pico pin-compatible" warning promised on silk is absent.** user-io.md:810 says silk must say "NOT Pico pin-compatible: pins 31–35, 37, 39 differ"; the bottom silk has the pin labels but no such line, and decisions.md does not record dropping it. On a Pico carrier, pin 39 (Pico VSYS, 5 V) is our GPIO29, a non-5 V-tolerant ADC pin; pin 33 (Pico AGND) shorts GPIO25. | Add "not Pico pinout" (12 mm at 0.8 mm) to the bottom if a via-free strip exists (the lower-left region x 3–8, y 44–48 is nearly empty on L4); otherwise log the omission in decisions.md and put the warning in README.md, first line of the pinout. |
| 7 | MINOR | **Stale documents contradict the built board** (all superseded by decisions.md, but a reader of the audit sees the old design): `design/nets.csv:27` "SWCLK … through 100 R at the chip" (no R13/R14); `nets.csv:2` "6.3 uF on VBUS" (7.5 µF); rp2350-core.md lists R13/R14, C10 and TP3 as a 1.5 mm pad; user-io.md lists TP1–TP4, TS-1187A, and "TestPoint_Pad_D1.5mm"; architecture.md D2 = B5819W. | One pass adding "superseded: see decisions.md <date>" lines; fix the two nets.csv reason strings. |
| 8 | MINOR | **KiCad board stackup is not the JLC04161H-7628 stack.** `easy2350-job.gbrjob` lists three 0.48 mm dielectrics (1.6 mm split evenly), while stackup.md and si-review.md assume 0.21 / 1.07 / 0.21 mm. JLC builds from its own stack regardless, so no fab effect, but the job file ships with the gerbers and the impedance figures in the package rest on the 0.21 mm number. | Set Board Setup → Physical Stackup to JLC's published 7628 thicknesses and re-export; or note in stackup.md that the .kicad_pcb stackup is placeholder. |
| 9 | MINOR | **Cost target not demonstrated** (intent.md:95–101: ≤ US$15/board at qty 5 on economic PCBA; intent.md:120 asks phase 1 to settle economic vs standard for the 0.4 mm QFN). phase10.md says it "can't be priced from here"; rp2350-core.md open question 5 is still open. My estimate: 9 extended fees ≈ $27 + 4-layer PCB ≈ $8–10 + assembly setup/stencil ≈ $10 + parts ≈ $5/board → ≈ $14–15/board economic, ≈ $19–20 standard. Borderline either way. | Get the JLC quote before the user signs; if economic PCBA refuses the 0.4 mm QFN-60, the intent's $20 fallback applies — record which in decisions.md. |
| 10 | MINOR | **Hand-fit inductor polarity** (if finding 1 takes path b, or at JLC's preview otherwise): Abracon's drawing (AOTA p.4) puts the white dot at the current-entry end; RP's Figure 28 and the vendor footprint put it at the 1V1 end (rp2350-core.md gotchas). The board follows RP: dot on pad 1 = 1V1 = the left pad at (11.83, 19.33), silk dot added there. Wrong orientation degrades 1V1 regulation at load without any obvious symptom. | bringup.md step 0: "L1 white dot at the left (1V1/COUT) end"; step 1: measure 1V1 at TP3 under a CoreMark-class load (DS p.413) on the first board. |
| 11 | NOTE | **CPL rotation: parts to check in JLC's preview, and why.** JLC's library orientation is defined per package by JLC, not by KiCad's footprint zero, so every polarised or keyed part can be off by a multiple of 90°. Highest consequence first: **D3–D6 WS2812B-2020 (0°)** — JLC's 2020 LED orientation is a known offender; a 180° error is silent until the chain is dark, and rework on 0.7 mm pads is miserable (the "reverse polarity resistant" claim gets no credit); **U1 QFN-60 (0°)** — pin 1 top-left; **L1 (0°)** — white dot on the 1V1 (left) pad, see #10; **U3 DFN-8 (90°)** — pin 1 dot, pins 1/8 toward C3/C2; **U4 SOT-23-5 (90°)** and **U2 SOT-23-6 (0°)** — SOT-23 is the classic JLC 180° offset; **U5 SOIC-8 (90°)** — pin 1 toward the top-left (dot at (4.02, 16.81)); **D2/D7 SOD-123 (90°)** — cathode band toward the bottom (VLED, pad 1 at y 33.78 / 38.72); **D1 0603 LED (90°)** — cathode (pad 1) at the bottom (17.23, 21.21), and KT-0603R's marked end is the cathode, opposite to its pin-① anode numbering; **Y1 3225 (0°)** — 0/180 harmless, 90 would short the crystal across GND; **J1 (180°)** — opening off the top edge. Six CPL rows carry −90°; JLC's importer accepts negatives in practice, but normalise to 270 to remove the question. | Check each in the JLC assembly preview against `design/renders/top.png`; normalise negative rotations in `cpl-jlc.csv`. |
| 12 | NOTE | 0.09 mm track necks at QFN pad entries (GPIO8–10/23) sit exactly on JLC's 4-layer minimum, and the 0.10 mm signal clearance is 0.01 over their 0.09. Zero fab margin, but the necks are ≤ 0.3 mm long and JLC builds this routinely (stackup.md). | None; if a board shows an open at a QFN pad, this is where to look. |
| 13 | NOTE | U3's three EP vias (0.25 drill, 0.55 pad) are untented in the pad with paste windows around them; some solder will wick. Standard for a DFN at JLC; the thermal case does not depend on the paste. | None. |
| 14 | NOTE | 3V3 ±3 % (intent.md:50) is met at ±2.82 % linear at the datasheet θJA and exceeded by 0.09 % at the whole-board thermal corner; RSS ±1.7 % (ldo-3v3.md). Real limits (IOVDD 3.63 V abs max; flash 3.0–3.6 V) keep ≥ 0.2 V. | Restate intent as "±3 % RSS, ±3.1 % linear worst case" at rev B; no part change. |
| 15 | NOTE | J1 uses KiCad's stock `USB_C_Receptacle_HRO_TYPE-C-31-M-12`; usb-front-end.md's MINOR (pads 0.22 mm short at the front vs the 2020 HRO drawing) asked for a project copy "or accept with a decision-log line"; decisions.md has no line. The stock footprint is used with C165948 on thousands of boards. | Add the decisions.md line. |
| 16 | NOTE | Silk pin-1/cathode marks are 0.3 mm dots added after the trim (decisions.md); bringup.md step 0 relies on them. They are at JLC's limit of what prints as a visible dot. | Fine; the assembly drawing (#4) is the backup. |
| 17 | NOTE | Things I looked for and found correct, so nobody re-checks them: D+/D− enter U2 on one pin of each channel and leave on the other (1→6, 3→4) with the connector-side nets pre-routed; SBU1/2 unconnected; U3 EN tied to IN, ADJ/NC open; U4 ~OE grounded and R12 holds its input low before 3V3 exists; R11 bleed + D7 clamp present on VLED; VREG_FB on 1V1 at COUT; PGND joins GND through the two adjacent vias; L2 cut-out covers the LX zone and L1 pad 2 with ≥ 0.3 mm; 9 GND vias around U3; QSPI_SS has no external pull-up (justified in flash.md and decisions.md); TP3 is on the bottom with a mask opening; bottom paste is empty; all 130 vias tented. |

## Answers to the review questions

1. **Send it?** Not today: the order cannot be placed with L1 at 1 piece, and
   findings 2–5 should be closed first. After those (an afternoon, no part
   change) — yes. The circuit, the datasheet audits and the layout of the
   parts that matter (regulator corner, crystal, USB, flash) check out against
   the files, not just the gate text.
2. **Single highest-risk issue:** L1 — both its supply (stock 1, no
   substitute) and its orientation (the two manufacturers' drawings disagree;
   a reversed part still "works" but regulates worse at load, and nothing on
   the bench flags it). Resolve the sourcing (finding 1), check the dot in
   the preview or at hand-fit (finding 10), and measure 1V1 under load on
   board 1.
3. **intent.md requirements not met:** line 25 (ESD on every J1 pin; CC1/CC2
   unprotected, finding 5); lines 95–101 (cost target not demonstrated,
   finding 9); line 106 (selective designators → none; waived in decisions.md
   with a sound reason, but the fallback it relies on — the assembly
   drawing — is unusable, finding 4). Line 50 (±3 %) is met by RSS and missed
   by 0.09 % linear at the thermal corner (recorded in ldo-3v3.md, NOTE 14).
4. **Bring-up pain:** the wrong VLED pass criteria (finding 3); no readable
   designator map (finding 4); TP3 on the bottom (recorded); probing "U3 pin 8"
   on a DFN (finding 3); the BOOT-button track under the hand-soldered header
   joints (finding 2) — if it bridges, the symptom is "always in BOOTSEL" or
   "a GPIO toggles the BOOT button", which a new user will not diagnose.
5. **Everything else** is in the table above.

## What is good

- The RP2350 power block is the vendor's copper to 0.01 mm, with the L2
  cut-out, the PGND two-via join and CFILT's own return all present; the
  three SI MAJORs from si-review (XIN, PIX12, USB_DP_C) are genuinely fixed
  on the final board (XIN 9.86 mm on L1 with no vias, PIX12 8.7 mm, D+ 7.9 mm).
- The datasheet work is unusually thorough and the one real electrical
  trap on this board — a Schottky letting VLED sit above the pixels' 5.3 V —
  was caught at phase 1 and fixed with a silicon diode, bleed and ±2 % clamp.
- Fab outputs are consistent with each other and with the schematic
  (parity 0, sync 54/54), paste and mask are right, drills and slots are
  sane, silk clears vias, and the bottom label map matches the netlist pin
  for pin.

---

# Re-check (2026-10-24)

Same reviewer, after the fixes and the from-scratch re-route. Everything was
re-measured on the new `easy2350.kicad_pcb` (09:15) and the fab outputs
regenerated from it (09:16); nothing below is taken from the summary of
changes.

## Checks re-run

| Check | Result |
|---|---|
| `kicad-cli pcb drc --severity-all --schematic-parity` | 53 `lib_footprint_mismatch` warnings only; **0 errors, 0 unconnected, 0 parity issues** |
| `kicad-cli sch erc --severity-all` | 0 (schematic unchanged since 2026-10-23 07:05) |
| `sync_check.py --bom --cpl` | PASS 58/58/54/54 |
| `nets_check.py --rules --routed` | PASS. XIN 9.86 mm / 0 vias (pre-route kept); USB_DP 9.97, USB_DM 10.06 (2 vias each, pre-routed); QSPI 7.5–17.4 mm; all inside limits |
| `silk_check.py`, `placement_check.py` | PASS, PASS (placement unchanged except the pixel block) |
| Stock | C42411119 (L1): **128** today; all others unchanged and normal |
| Edge scan (every segment and via within 0.9 mm of a long edge) | **None.** Rule areas "edge strip left/right" (0.7 × 51 mm, no tracks, no vias, all four layers) are present; finding #2 closed |
| Pixel block | D3–D6 at 180°: pad 3 (DIN) top-left, pad 1 (DOUT) bottom-right of each; PIX12 2.3 mm, PIX23 2.9 mm, PIX34 2.3 mm, LED_DIN 7.5 mm, **all on L1 with 0 vias**; four F.Cu rule areas 0.9 × 2.0 mm (no copper/tracks/vias) under the centre strips, and the F.Cu GND pour is absent under all four centres; caps C25–C28 turned so pad 1 (VLED) faces each VDD pad; R10 at (6.48, 42.62) between U4 and pixel 1's DIN side; CPL rows D3–D6 = 180 |
| XIN neighbours | nearest PIX12 copper 3.2 mm, PIX23 4.1 mm, VLED 2.2 mm; the 45 mm PIX23 run is gone |
| Gerbers (re-parsed) | F_Paste 215 flashes, B_Paste 0, B_Mask 50 (THT + TP3 + NPTH only), Edge closed; PTH 197 holes (138 vias + pins + EP vias), 4 G85 slots, 2 NPTH pegs |
| CPL | 54 rows, rotations all in 0–360 (C6/C13/C16/R3/R4/R5 now 270) |
| Vias | 138: GND 43, +3V3 12, +1V1 9, VBUS 5, VLED 3, signal 66. 9 GND vias within 3.5 mm of U3 |
| Silk | Pin-1 / cathode dots present beside U1, U4, L1, D2, D7, D3–D6 (board-level circles at the pad-1 corners); U5/U2/D1 keep footprint marks; U3 by its footprint corner line. Top labels 5V / 25 / BOOT / SWD / RST, bottom identity, limits and both warning lines present; all 40 pin labels unchanged |
| Documents | bringup.md: VLED idle "VBUS − 0.4…0.6 V", full white "≈ 4.0–4.2 V, pass ≥ 3.5 V", 5.5 V idle test added, probe C2 not U3 pin 8, L1 dot check in step 0, J2 joint check, 1V1 under CoreMark load in step 5; architecture.md D2 = SM4007PL; nets.csv rows 2 and 27 fixed; usb-front-end.md states the CC gap; decisions.md has the CC waiver, the edge strip, the CPL normalisation, the pixel turn, the Pico-line omission (searched, no room; README line 4 says it) and the single LED-branch VBUS via; stackup.md tells the orderer to select JLC04161H-7628; phase10.md's CPL table is ranked and at 0–360 |
| Assembly drawing | `assembly-top.png/.pdf` at ≈ 4:1, every designator legible, pad outlines and pin-1 chamfers shown (finding #4 closed) |

## Status of the 2026-10-23 findings

| # | Was | Now |
|---|---|---|
| 1 | BLOCKER: L1 stock 1 | **Closed for today**: 128 in stock. Order while it lasts; the sourcing fallback (hand-fit) stays valid if it drops again |
| 2 | MAJOR: BOOTSEL along the edge under J2's joints | **Closed**: 0.7 mm no-track/no-via strips both edges, re-routed; BOOTSEL net now 28.9 mm on L1/L4 inboard with 2 vias |
| 3 | MAJOR: wrong VLED bring-up numbers | **Closed** (values match rgb-leds.md) |
| 4 | MAJOR: unreadable assembly drawing | **Closed** |
| 5 | MAJOR: CC ESD vs intent.md:25 | **Waived in decisions.md with a sound reason** (nothing but Rd on the net; failure mode is loss of Rd, not chip damage; rev B option named) |
| 6 | MINOR: Pico warning on silk | **Waived with a sound reason** (no via-free strip left; README + bottom labels) |
| 7 | MINOR: stale documents | **Closed** (nets.csv fixed; superseded notes in rp2350-core.md and user-io.md; architecture.md fixed). flash.md needed none |
| 8 | MINOR: KiCad stackup ≠ JLC stack | **Accepted**: stackup.md now says to pick JLC04161H-7628 at order; the .kicad_pcb stackup stays placeholder (no fab effect) |
| 9 | MINOR: cost / economic-vs-standard | **Open by design**: needs JLC's quote at order time (gate 10 "needs_user"). Not a board defect |
| 10 | MINOR: L1 dot convention | **Closed** (bring-up step 0 and 5; CPL table) |
| 11 | NOTE: CPL preview list | **Closed** (ranked table in phase10.md, rotations normalised) |

## New findings on the re-routed board

| # | Sev | Finding (location) | Suggested fix |
|---|---|---|---|
| R1 | NOTE | Pixel 1 and 2 GND pads (D3 pad 2 at (5.54, 40.28), D4 pad 2 at (9.24, 40.28)) reach L2 only through the L1 pour and their caps' vias ≈ 2 mm away; the pads sit under the bottom silk reservations so no via fits. 36 mA each at 2 kHz PWM, local 100 nF present. Logged in decisions.md. | None for rev A; rgb-leds.md asked for ≥ 2 vias per GND pad, so rev B if the reservations move |
| R2 | NOTE | VBUS LED branch leaves L1 through a single 0.6/0.3 via at (5.9, 11.9) (≤ 0.2 A on that branch; the 0.75 A peak path J1 → U3/U2 stays on L1; pairs at D2 and J3 pin 20). Logged in decisions.md with the current argument, which is right. | None |
| R3 | NOTE | The re-route moved the VBUS trunk to D2 down the left side on L4 (x ≈ 4.7, y 3 → 31) under the J2 GPIO0–9 fan-out, referenced to the L3 3V3 plane. DC, 0.25 mm, and it no longer passes under U1 (the gate-7 chip-note deviation from the first route is gone). | None |
| R4 | NOTE | GPIO0 leaves J2 pin 1 on L1 to a via at ≈ (4.6, 2.9), passing between the pad and J1's rear shell slot at (6.21, 2.17). DRC-clean (≥ 0.10 mm), away from the hand-soldered pin row's edge side. | None |
| R5 | NOTE | Order-time items that remain the user's, not the board's: JLC quote (economic vs standard PCBA, ≤ US$15 target, intent.md:95–101); CPL preview check per phase10.md's table, D3–D6 (now 180°) and L1 first; select the JLC04161H-7628 stack; order while L1 stock holds. | — |

Nothing new at MINOR or above. The pixel change is an improvement on every
axis I measured (hop lengths 25 → 2–3 mm, no vias, no copper under the
polarity strips, XIN's nearest 5 V data trace moved from 0.33 mm to 3.2 mm),
and the edge strips removed the only layout item I rated MAJOR.

## Verdict

**Send it.** All five items that blocked the first review are closed or
waived with reasons I agree with; DRC/ERC/parity/sync/nets/silk/placement
all pass on the final files; the fab outputs are consistent with the board
and with each other; stock is sufficient today. Remaining actions are at
the JLC order form (quote, stack selection, pin-1 preview on the parts
listed in gate 10) and on the bench (bringup.md), and they are written down.
