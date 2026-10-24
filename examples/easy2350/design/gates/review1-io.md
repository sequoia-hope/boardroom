# Gate 1 independent review — RGB LEDs and user I/O

Scope: `design/parts/rgb-leds.md` (D2 SM4007PL + 4.7 kΩ bleed → VLED, U4 SN74LV1T125, D3–D6
WS2812B-2020-V6) and `design/parts/user-io.md` (SW1/SW2 TS-1187A, D1 KT-0603R + R_LED1, J2/J3
pin map, J4 SWD, TP1–TP4). Reviewer did not write either audit. Every datasheet was re-read
(`pdftotext`, figure pages rendered as images), every calculation recomputed, footprints read
from the `.kicad_mod` files, sourcing re-run with `jlc_parts.py show` on 2026-10-23, and the
schematic netlist exported with `kicad-cli` and compared pin by pin. ERC: 0 errors, 0 warnings.

Page numbers: PDF page of the small datasheets; printed page for rp2350-datasheet.pdf.

## Verdict on the VLED approach

The steady-state analysis holds up. Re-derived from sm4007pl.pdf Fig. 3 (p.2): 25 °C curve
reads 0.81 V at 0.1 A and 0.99 V at 1.0 A; 100 °C reads 0.70 V at 0.1 A; the audit's
readings (0.807 / 0.987 / 0.707) are within the resolution of the plot. The extrapolation to
1 mA is conservative: published forward curves for the 1N4007 family (same die class, which is
all this part is) put Vf(1 mA, 25 °C) at ≈ 0.55 V, and the low-current tempco is ≈ −2 mV/°C, so
Vf(1 mA, 40 °C) ≈ 0.52 V typical. That gives VLED ≈ 4.98 V typical at VBUS 5.5 V idle, and
≈ 5.1 V with a −0.1 V unit; the audit's bound of 5.125 V is consistent and the 5.3 V limit is
met with ≈ 0.2 V. Low end: Vf(160 mA, 0 °C) ≤ 0.985 V → VLED ≥ 3.52 V ≥ 3.3 V. Confirmed.

What does not hold up is **hot-plug**. VLED's 5.2 µF is ≈ 80 % of all VBUS capacitance, so it
rings with the cable inductance through D2; at the ring peak D2 current is ≈ 0, so C_VLED is
left at ≈ Vpeak − 0.5 V and can only decay through the bleed. With the TVS-bounded peak of
6.5–7 V from ldo-3v3.md (A-to-C cable, source already live): VLED ≈ 6.0–6.5 V, τ = 4.7 kΩ ×
≈ 3.2 µF (DC-bias-derated) ≈ 15 ms, time above 5.3 V = τ·ln(6.2/5.3) ≈ **2–4 ms per plug-in**,
against a pixel "Maximum Rated Value" of 5.3 V (ws2812b-2020-v6.pdf p.3). The audit knows this
(Open question 5) but leaves it unbounded. That is the one MAJOR in this block.

Alternatives, with numbers:

| Option | VLED at idle, VBUS 5.5 V | VLED at 160 mA, VBUS 4.5 V | Hot-plug | Cost | Verdict |
|---|---|---|---|---|---|
| **As drawn** (SM4007PL + 4.7 kΩ) | ≈ 5.0 typ, ≤ 5.13 bound | 3.52–3.63 | 6.0–6.5 V for 2–4 ms | 2 basic parts | Steady state OK, hot-plug not |
| Pixels from 3V3 | 3.20–3.40 V | same | none | −2 parts | **Worse**: 3.20 V min < 3.3 V pixel min (p.3), and U3 dissipation rises to (5.5 − 3.3) × (0.277 + 0.144) = 0.93 W → TJ ≈ 40 + 0.93 × 110 = 142 °C, over ldo-3v3.md's 125 °C limit |
| Schottky B5819W | ≈ 5.45 V | 4.18 V | worse | — | Fails the 5.3 V limit at idle (audit's analysis confirmed) |
| 4.0 V LDO for VLED (e.g. ME6209A40M3G C83509, SOT-23, 250 mA; extended, stock 3896) | 3.92–4.08 V | 3.92–4.08 V if dropout ≤ 0.4 V at 160 mA (to verify) | input sees the ring, output does not | +1 extended line item (+ fee), −D2, −bleed; 0.24 W in SOT-23 (θJA ≈ 250 °C/W → +60 °C) | Clean and regulated; blue-channel headroom at 4.0 V is thinner (don't care per intent); another hot part |
| **As drawn + 5.1 V zener across VLED** (MM3Z5V1 C19077455, SOD-323, "preferred", stock 382k; or BZT52C5V1 C173407) | unchanged (zener off: D2 at ≥ 1.1 mA drops ≥ 0.45 V, so VLED ≤ 5.05 V typ) | unchanged | the 5.2 µC excess (5.2 µF × 1 V) is dumped at ≈ 100–200 mA in tens of µs; VLED then ≤ Vz | +1 part, ≈ 0 mW | **Recommended minimum fix**: bounds both the hot-plug hold and the unit-spread assumption at near-zero cost |
| 4.7 V zener instead of 5.1 V | ≈ 4.9 V, zener conducts ≈ 5–8 mA (≈ 40 mW) continuously at 5.5 V in | unchanged | VLED ≤ 5.0 V | +1 part, +8 mA from VBUS | Hard guarantee; pay 40 mW. Choose this if the pixel supplier confirms 5.3 V is a true abs max |

Recommendation: keep D2 + bleed and add the 5.1 V zener (fetch its datasheet; check Vz at
1–2 mA, max Vz at IZT is 5.36 V for a ±5 % part, below 5.3 V at the ≤ 2 mA it carries here).
The 4.0 V LDO is the cleaner architecture if the designer prefers a regulated rail; it is not
clearly better once its fee, heat and dropout check are counted. The level shifter stays either
way (RP2350 VOH min 2.62 V < 0.55 × 5.13 = 2.82 V).

## Findings

| # | Severity | Finding (with page) | Suggested fix |
|---|---|---|---|
| 1 | MAJOR | **Hot-plug leaves VLED above the pixel's 5.3 V maximum for milliseconds.** C_VLED (5.2 µF nominal, behind D2) charges to the VBUS ring peak minus ≈ 0.5 V and discharges only through R_VLED_BLEED: with the TVS-bounded 6.5–7 V peak from ldo-3v3.md, VLED ≈ 6.0–6.5 V, τ ≈ 15 ms, ≈ 2–4 ms above 5.3 V (ws2812b-2020-v6.pdf p.3) on every A-to-C or live-source plug-in. rgb-leds.md Open question 5 names the issue but no part bounds it. | Add a 5.1 V zener VLED–GND next to R_VLED_BLEED (MM3Z5V1 C19077455, SOD-323, preferred; or BZT52C5V1 C173407), with its datasheet in `datasheets/` and Vz at 1–2 mA checked ≤ 5.3 V. Or a 4.0 V LDO (table above). Record the choice in decisions.md. |
| 2 | MAJOR | **TP2 (3V3) is missing from the schematic.** The netlist has TP1 (VBUS), TP3 (1V1), TP4 (GND) only; no test point is on net +3V3. intent.md Serviceability requires test points on VBUS, 3V3, 1V1 and GND; user-io.md lists TP2 = 3V3. | Add TP2 `Connector:TestPoint`, footprint `TestPoint:TestPoint_Pad_D1.5mm`, on +3V3 at C_LDO_OUT, Exclude from BOM. |
| 3 | MINOR | **Net names in the schematic do not match the audits / nets.csv.** U4.4–R_DIN is `Net-(R10-Pad1)` (audit: LED_DIN_BUF); D1 anode is `Net-(D1-A)` (audit: LED1_A); SW1/SW2 resistor nets are `Net-(R7-Pad2)` / `Net-(R8-Pad2)` (core audit: BOOTSEL_SW / RESET_SW); U1-side SWD nets are `Net-(U1-SWCLK)` / `Net-(U1-SWDIO)` (audits: SWCLK_MCU / SWDIO_MCU). `nets.csv` lists LED_DIN but not LED_DIN_BUF. | Label the nets as the audits name them (or rename in the audits) and add the missing rows to `design/nets.csv`, so the sync check in phase 2 can match them. |
| 4 | MINOR | **Project footprint `D_SOD-123F_MDD`: courtyard equals the pad edge.** Pads 1.2 × 1.2 mm at ±1.6 mm reach x = ±2.2 mm; the courtyard rectangle is also ±2.2 × ±1.15 mm (copied from the stock D_SOD-123F whose pads end at ±1.95). KLC wants ≥ 0.25 mm beyond pads. Silk cathode bar is at the pad-1 end, matching Device:D pin 1 = K and the body bar (sm4007pl.pdf p.1): correct. Pad geometry matches the datasheet pad layout A = B = 1.2, C = 3.2, E = 4.4 (p.3): correct. | Widen the courtyard to ±2.45 × ±1.15 mm. |
| 5 | MINOR | **rgb-leds.md and ldo-3v3.md disagree on the VLED decay constant** (13 ms with 2.7 µF effective vs 24 ms with 5.2 µF nominal). Only matters for finding 1, where the derated value (≈ 15 ms) is the right one. | After the fix for finding 1, state one τ in rgb-leds.md and reference it from ldo-3v3.md. |
| 6 | MINOR | **D3–D6 symbol Datasheet field points to the V1.3 Adafruit PDF**, not the V6 datasheet the audit relies on (VDD min 3.3 V, VIH 0.55·VDD, Cn 100 nF differ between the two). A reader of the schematic opens the wrong document. | Set the Datasheet field to the LCSC C52917434 datasheet URL (or the repo path). |
| 7 | MINOR | **J2/J3/J4 and TP1/3/4 are only `exclude_from_bom`.** "Not assembled" also needs exclude-from-position-files; that is a footprint attribute set in the PCB, which does not exist yet. The TestPoint footprint already carries it; the PinHeader footprints do not. | Phase 4 checklist item: set "Exclude from position files" on J2, J3, J4 footprints, and mark them DNP so the JLC BOM/CPL exporter skips them. |
| 8 | NOTE | **D2 steady-state bounds re-derived and accepted.** Fig. 3 readings confirmed; Vf(1 mA, 40 °C) ≈ 0.52 V typical by 1N4007-class curves → VLED ≈ 4.98 V typ at 5.5 V in; audit's 5.125 V bound holds with ≈ 0.18 V margin. Low end 3.52 V ≥ 3.3 V confirmed. Dissipation 0.158 W; TJ ≈ 68 °C on the (axial-copied) 180 °C/W. Bleed 5.6 mW in a 1/16 W 0402. | Keep the first-board measurement (Open question 1). |
| 9 | NOTE | **U4 level margins recomputed.** VIH max 2.11 V at VCC 5.5 V (sn74lv1t125.pdf p.7; 3.6→4.5 V gap assumed monotonic) vs RP2350 VOH min 2.62 V at 4 mA (rp2350-datasheet p.1336, Table 1683; ≈ 3.2 V at the ≤ 1 µA actually drawn): ≥ 0.51 V. VIL min 0.65 V (VCC 3.0–3.6) vs RP2350 VOL max 0.5 V at 4 mA: ≥ 0.15 V, in practice ≈ 0.6 V. U4 VOH ≥ VCC − 0.1 V at −20 µA vs pixel VIH 0.55·VDD: ≥ 1.5 V. tpd ≤ 8 ns (p.8) vs 220 ns T0H minimum (ws2812b p.5). ΔICC 1.5 mA with A at 3.4 V (p.7) covered. All OK, including at the hot-plug VLED (VCC abs max 7 V, p.6). | — |
| 10 | NOTE | **Symbol easy2350:SN74LV1T125** pins 1 ~OE / 2 A / 3 GND / 4 Y / 5 VCC match the DBV pinout (p.1); netlist: 1→GND, 2→GPIO23, 3→GND, 4→R_DIN, 5→VLED. Pin names are "~" (blank), so the schematic shows no A/Y/OE labels. | Name the pins A, Y, ~OE for readability. |
| 11 | NOTE | **WS2812B-2020 footprint verified against V6 p.2**: pads 0.7 × 0.7, centres ±0.915 / ±0.55 (1.13 gap, 0.40 gap); pin order DI 3 / VDD 4 / GND 2 / DO 1 is the same cyclic order as KiCad 1→4, rotated 180°, chamfer at the DO corner. `LED:WS2812B-2020` symbol pins 1 DOUT / 2 VSS / 3 DIN / 4 VDD match the V6 pin table. D6 DOUT carries a no-connect flag. The 4 × 100 nF per pixel required by V6 p.6 are present (C25–C28) plus C24 at U4. | JLC CPL rotation check stays an open question (audit Open question 4). |
| 12 | NOTE | **TS-1187A footprint verified against the drawing**: pads 1.0 × 0.75 mm at ±3.0 / ±1.875 ((7.0−5.0)/2, (4.5−3.0)/2, (7.0+5.0)/4, (4.5+3.0)/4); A–B (top pair) and C–D (bottom pair) are the internally joined pairs, KiCad pads "1" both at y = −1.875 and "2" both at +1.875. SW1 → R7 (1 kΩ) → QSPI_SS, SW2 → R8 (1 kΩ) → RUN, other side GND. Ratings 50 mA / 12 V DC vs ≤ 3.3 mA / 3.3 V. | — |
| 13 | NOTE | **D1 / R_LED1 recomputed**: I = (3.40 − 1.55)/0.99 kΩ = 1.87 mA max; (3.20 − 2.23)/(1.01 kΩ + 145 Ω) = 0.84 mA min; 1.43 mA typ. Curve p.5 confirms Vf ≈ 1.85 V at 1 mA, 2.12 V at 20 mA. Inside intent's 2 mA. Polarity (p.2 drawing, zoomed): triangle points from ① to ②, "+" at ① → ① anode, ② cathode under the green corner mark; KiCad pad 1 = K ↔ LED pin ②, as the audit says. Footprint gap 0.70 mm matches the suggested pads. | Keep the CPL-rotation check (user-io Open question 3). |
| 14 | NOTE | **Header pin map matches the netlist pin for pin** (J2.1–20: GPIO0, GPIO1, GND, GPIO2, GPIO3, GPIO4, GPIO5, GND, GPIO6, GPIO7, GPIO8, GPIO9, GND, GPIO10–13 with GND at 13 and 18, GPIO14, GPIO15; J3.1–20: GPIO16, GPIO17, GND, GPIO18–21, GND, GPIO22, RUN, GPIO23, GPIO24, GPIO25, GPIO26, GPIO27, 3V3, GPIO28, GND, GPIO29, VBUS). J4: 1 SWCLK, 2 GND, 3 SWDIO via the 100 Ω pair, matching the RPi debug-connector spec p.1–2. GPIO23 also to U4.2 + R12 100 kΩ; GPIO25 also to R9 1 kΩ → D1. | — |
| 15 | NOTE | **Pixel-timing caution is correct**: V6 p.5 gives T1L 580 ns–1 µs; the pico-examples `ws2812.pio` defaults (T1 = 2, T2 = 5, T3 = 3 at 8 cycles per bit) produce T1L ≈ 375 ns. Out of spec on paper; firmware item. | Carry into the README / firmware notes. |
| 16 | NOTE | **Sourcing re-run 2026-10-23**: C64898 basic 4.71 M; C473338 extended 161 k; C52917434 extended 832 k; C318884 basic 1.17 M; C2286 basic 7.94 M; C25900, C19666, C1525, C25741, C25105, C11702 all basic, ≥ 2.2 M. All lifecycle normal. Every datasheet used is in `sources.csv`. | — |
| 17 | NOTE | **VBUS attach tally** (ldo-3v3.md): the block's 5.2 µF makes the board total 6.3 µF nominal, inside 10 µF; a zener per finding 1 adds nothing. | — |

Gate checklist for this scope: every abs max checked ✓; every value has a recomputed
calculation ✓; MLCC derating assumed (no curve for C19666) — acceptable, flagged in the audit;
footprints match MPN packages ✓ (finding 4 is a courtyard nit); sourcing confirmed ✓;
datasheets in sources.csv ✓; two schematic defects (findings 1 and 2) to clear before the gate.
