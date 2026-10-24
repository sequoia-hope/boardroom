# U1 — RP2350A core block (core regulator, decoupling, crystal, BOOTSEL/RUN)

> **Superseded at layout (decisions.md, 2026-10-23):** C10 (separate ADC_AVDD cap) removed, C9 serves pins 44/45; the SWD series resistors R13/R14 removed; TP3 is a 1.0 mm pad on the bottom, not 1.5 mm. The tables below keep the audit as it was done.

Status: audit by Claude, 2026-10-23; gate-1 review `gates/review1-core.md` applied 2026-10-23 (findings #1–#6 fixed in this file; NOTEs #8, #12–#14 folded in).

Scope: U1 RP2350A; its core switching regulator's external parts (L1, CIN, COUT,
VREG_AVDD RC); all U1 decoupling; Y1 with its series resistor and load caps; the
BOOTSEL and RUN resistors; placement of the 27 Ω USB series pair (value is in
`usb-front-end.md`). SW1/SW2 themselves are in `user-io.md`; the flash and its
QSPI_SS pull-up footprint are in `flash.md`.

## Sources read

| Tag | File | Revision | Read |
|---|---|---|---|
| **DS** | `datasheets/rp2350-datasheet.pdf` | RP2350 Datasheet, build 1 Oct 2026, 1371 pp. Printed page numbers (PDF page = printed + 1). | §5.2.8 BOOTSEL boot (pp.344–345); §6.1 power supplies (pp.400–401); §6.3 core regulator incl. §6.3.7 application circuit and §6.3.8 external components and layout (pp.405–414); §7.3 chip-level resets (pp.450–452); §7.6.1 POR (p.462); §8.2 XOSC (pp.506–508); §14.1 QFN-60 package, thermal, footprint (pp.1324–1325); §14.4–14.7 marking, storage, solder profile (pp.1328–1329); §14.8 pinout and pin tables (pp.1330–1335); §14.9 electrical specifications incl. power consumption (pp.1335–1344). Package and footprint drawings and Figures 25, 26, 28 rendered and read as images. |
| **HDG** | `datasheets/hardware-design-with-rp2350.pdf` | Hardware design with RP2350, release 3, build 20/08/2026, 25 pp. Printed page numbers. | All of it, including the Appendix A schematic. Note: the crystal specification table promised on HDG p.14 is **missing** in this build (page rendered: the sentence "which has the following specifications:" is followed by nothing). DS Table 629 (p.507) has it. |
| **AOTA** | `datasheets/aota-b201610.pdf` | Abracon AOTA-B201610S3R3-101-T, revision A, 9/13/2024, 6 pp. | All (graphs and mechanical page rendered). |
| **ABM8** | `datasheets/abm8-272-t3.pdf` | Abracon ABM8-272-T3, drawing 456603, rev IR, issue 16 Nov 2023, 4 pp. | All (mechanical page rendered and zoomed). |
| **SAM** | `datasheets/samsung-mlcc.pdf` | Samsung MLCC catalogue (2014 cover, PDF built 2015), 84 pp. PDF page numbers. | Part numbering system (p.4), temperature characteristics (p.5), lineups for the parts used (pp.19, 21, 25), reliability table (pp.54–56). It has no DC-bias curves. |
| **SAMWEB** | `https://product.samsungsem.com/mlcc/<part>.do` for CL05A475MP5NRN, CL05A475MQ5NRN, CL05B104KO5NNN, CL21A106KAYNNN | Samsung's per-part characteristic data, embedded in each page as JSON, retrieved 2026-10-23. Samsung labels it "typical data for design reference only". Conditions: 25 °C, 1 kHz. | DC-bias, temperature, impedance and ESR series extracted by script and interpolated. Not saved in `datasheets/` (open question 7). |
| **FH** | `datasheets/fh-0402cg.pdf` (Fenghua "General Series MLCC" spec, 30 pp.; `sources.csv`: C1548) | as fetched from LCSC | Ordering code (p.4), C0G temperature coefficient (p.5). |
| **UR** | `datasheets/uniroyal-0402wg.pdf` (UNI-ROYAL thick-film chip resistors, V.3 12 Feb 2019, 9 pp.; `sources.csv`: C11702; C25105, C25076 and C25100 link the same PDF) | as fetched from LCSC | Part-number code (p.2, §2.3–2.4.3), ratings and derating (p.5). |
| **RPDBG** | `datasheets/rpi-debug-connector.pdf` | Raspberry Pi 3-pin Debug Connector Specification, RP-003139-SP-4, 4 pp. (added by the user-io audit) | Pinout (p.1), termination resistors (p.2), voltages (pp.2–3). |
| **REF** | `~/pcb/RP-006440-DD-2-RP2350A Minimal Board Kicad archive/` | Raspberry Pi RP2350A Minimal board, KiCad files | Netlist exported with `kicad-cli sch export netlist --format kicadxml`; the `.kicad_pcb` read with pcbnew (footprint positions relative to U1, zones, vias); the `.pretty` footprints and the vendor symbol library read. |

## Parts in this block

Designators: ours are written bare (C16, R3); the vendor reference board's are written "REF" plus its designator (REF C10). The two sets overlap, so read every bare designator as ours.

| Function | Value | MPN | LCSC | Footprint | Supplier (`jlc_parts.py show`, 2026-10-23) |
|---|---|---|---|---|---|
| U1 MCU | RP2350A, QFN-60 7×7, 0.4 mm | RP2350A | C42411118 | REF `RP2350-QFN-60-1EP_7x7_P0.4mm_EP3.4x3.4mm_ThermalVias`, copied into the project library (see Footprints) | `C42411118 RP2350A (Raspberry Pi, QFN-60-EP(7x7)): extended, JLC stock 5924, lifecycle normal, checked 2026-10-23` |
| L1 | 3.3 µH, polarity-marked, 2016 | AOTA-B201610S3R3-101-T | C42411119 | REF `L_pol_2016`, copied into the project library | `C42411119 AOTA-B201610S3R3-101-T (ABRACON, 0806): extended, JLC stock 191, lifecycle normal, checked 2026-10-23` |
| C14 (CIN), C15 (COUT) | 4.7 µF X5R 10 V ±20 % 0402 | CL05A475MP5NRNC | C23733 | REF `C_0402_1005Metric_small_pads` (as the reference's C6/C7) | `C23733 CL05A475MP5NRNC (Samsung Electro-Mechanics, 0402): basic, JLC stock 2672206, lifecycle normal, checked 2026-10-23` |
| C13 (CFILT), C16 | 4.7 µF X5R 10 V ±20 % 0402 | CL05A475MP5NRNC | C23733 | `Capacitor_SMD:C_0402_1005Metric` | as above |
| R3, RFILT | 33 Ω ±1 % 1/16 W 0402 | 0402WGF330JTCE | C25105 | `Resistor_SMD:R_0402_1005Metric` | `C25105 0402WGF330JTCE (UNI-ROYAL, 0402): basic, JLC stock 2123664, lifecycle normal, checked 2026-10-23` |
| 10 × 100 nF decouplers (IOVDD ×6, ADC_AVDD, USB_OTP_VDD+QSPI_IOVDD, DVDD6, DVDD39) | 100 nF X7R 16 V ±10 % 0402 | CL05B104KO5NNNC | C1525 | `Capacitor_SMD:C_0402_1005Metric` | `C1525 CL05B104KO5NNNC (Samsung Electro-Mechanics, 0402): basic, JLC stock 35870034, lifecycle normal, checked 2026-10-23` |
| Y1 crystal | 12 MHz, CL 10 pF, 3225 4-pad | ABM8-272-T3 | C20625731 | `Crystal:Crystal_SMD_3225-4Pin_3.2x2.5mm` | `C20625731 ABM8-272-T3 (ABRACON, SMD3225-4P): extended, JLC stock 3393, lifecycle normal, checked 2026-10-23` |
| C20, C21 | 15 pF C0G 50 V ±5 % 0402 | 0402CG150J500NT | C1548 | `Capacitor_SMD:C_0402_1005Metric` | `C1548 0402CG150J500NT (FH, 0402): basic, JLC stock 1038894, lifecycle normal, checked 2026-10-23` |
| R6, R7, R8 | 1 kΩ ±1 % 1/16 W 0402 | 0402WGF1001TCE | C11702 | `Resistor_SMD:R_0402_1005Metric` | `C11702 0402WGF1001TCE (UNI-ROYAL, 0402): basic, JLC stock 19090406, lifecycle normal, checked 2026-10-23` |
| R13, R14 (SWD source termination at the target) | 100 Ω ±1 % 1/16 W 0402 | 0402WGF1000TCE | C25076 | `Resistor_SMD:R_0402_1005Metric` | `C25076 0402WGF1000TCE (UNI-ROYAL, 0402): basic, JLC stock 7444247, lifecycle normal, checked 2026-10-23` |
| R5, R4 (value audited in `usb-front-end.md`; placement only here) | 27 Ω 0402 | 0402WGF270JTCE | C25100 | `Resistor_SMD:R_0402_1005Metric` | `C25100 0402WGF270JTCE (UNI-ROYAL, 0402): extended, JLC stock 56936, lifecycle normal, checked 2026-10-23` |

Changes against the part list I was given:
- **None of the listed parts is wrong.** CL05A475MP5NRNC is **10 V** (SAM p.4: voltage code P = 10 V; SAM p.19 lineup row 32: "CL05A475MP5NRN 4.7 µF 10 V 20 % 0.65"; SAMWEB: "Rated Voltage 10.0Vdc", X5R, mass production).
- C1548 is a **Fenghua** 0402CG150J500NT, not Samsung: C0G (CG), 15 pF, J = ±5 %, 50 V (FH p.4 ordering code; FH p.5: C0G 0 ± 30 ppm/°C, −55 to 125 °C).
- The 10 µF 0805 C15850 is **not used in this block**. The reference's 10 µF parts (C1 VBUS, C5 and C19 on 3V3) belong to the LDO and USB blocks. Its DC-bias data is under Calculations for those audits.
- **Added: 2 × 100 Ω (R13, R14) in series with SWCLK and SWDIO at U1 pins 24/25.** RPDBG p.2 asks for them; the user-io audit proposed them. Not in REF. See Required parts and Calc 9.
- **Added: one extra 100 nF (C10) on ADC_AVDD pin 44.** The reference shares one capacitor (its C17) between pins 44 and 45; DS p.401 asks for 100 nF at ADC_AVDD, and intent.md asks for "its own 100 nF". See Deviations.
- Three extended parts in this block: U1, L1, Y1. The 27 Ω pair is also extended (C25100) but is counted in `usb-front-end.md`.

## Pin map checked

Against DS Figure 152 (p.1330), Tables 1674–1679 (pp.1332–1335), the stock `MCU_RaspberryPi:RP2350A` symbol and the REF netlist:
IOVDD 1, 11, 20, 30, 38, 45; DVDD 6, 23, 39; XIN 21; XOUT 22; SWCLK 24; SWDIO 25; RUN 26; ADC_AVDD 44; VREG_AVDD 46; VREG_PGND 47; VREG_LX 48; VREG_VIN 49; VREG_FB 50; USB_DM 51; USB_DP 52; USB_OTP_VDD 53; QSPI_IOVDD 54; QSPI_SD3 55, SCLK 56, SD0 57, SD2 58, SD1 59, SS 60; GPIO0–3 = 2–5, GPIO4–7 = 7–10, GPIO8–15 = 12–19, GPIO16–18 = 27–29, GPIO19–25 = 31–37, GPIO26–29 = 40–43; GND = exposed pad (61). **All three agree**, except one datasheet error:

- **DS Table 1679 (p.1334) is wrong:** it lists IOVDD as "11, 20, 30, 38, 45, 54" and QSPI_IOVDD as "54". Figure 152 (p.1330), the stock symbol and the REF netlist all have **pin 1 = IOVDD** and **pin 54 = QSPI_IOVDD**. We follow the figure. Pin 1 must be on 3V3.

Symbol: the stock `MCU_RaspberryPi:RP2350A` stacks pins 1/11/20/30/38/45 (IOVDD) on one location and 6/23/39 (DVDD) on another, with all but one of each hidden. Electrically that is fine, but no decoupler can be drawn on "its" pin, so wiring alone cannot say which pin C16 serves. REF's own symbol (`MCU_RaspberryPi_RP2350.kicad_sym`, `RP2350_60QFN`) draws every power pin separately. **As built:** the schematic keeps the stock stacking and binds each decoupler to its pin with a `Decouples` field instead (Deviation 6; table under Schematic connections).

## Absolute max vs our worst case

The 3V3 rail is 3.3 V ±3 % = 3.20–3.40 V (intent.md). The `ldo-3v3.md` audit has to show the LDO holds that. Ambient is 0–40 °C.

| Parameter | Abs max (DS Table 1680, p.1335) | Recommended (DS Table 1688, pp.1340–1341) | Our worst case | Margin |
|---|---|---|---|---|
| IOVDD, QSPI_IOVDD | −0.5 / 3.63 V | 1.62–3.63 V | 3.40 V | OK, 0.23 V. Abs max equals the recommended max, so an LDO overshoot above 3.63 V is damage, not just out of spec: `ldo-3v3.md` must cover load-step overshoot. |
| VREG_VIN | not listed | 2.7–5.5 V | 3.20–3.40 V | OK |
| VREG_AVDD | not listed | **3.135**–3.63 V | 3.20 V − 33 Ω × 0.22 mA = **3.193 V** (current from DS Table 1693, p.1344: 216–220 µA) | OK, **58 mV** above min. The block's tightest margin. |
| USB_OTP_VDD | not listed | **3.135**–3.63 V | 3.20 V | OK, 65 mV |
| ADC_AVDD | not listed | 1.62–3.63 V; ADC compromised below 2.97 V (note c) | 3.20–3.40 V | OK |
| DVDD (from the on-chip regulator) | −0.5 / 1.21 V | 1.05–1.16 V; transients ±100 mV (note b) | 1.1 V ± 3 % = 1.067–1.133 V (δVOUT, DS Table 1689, p.1341) | OK. Firmware can lift VOUT above 1.16 V only after disabling the 1.3 V limit (DS p.406, p.1342 warning). |
| VPIN, FT pins (GPIO0–25, RUN, SWCLK, SWDIO) | −0.5 / 5.5 V with IOVDD = 3.3 V; **3.63 V with IOVDD = 0 V** | — | 3.3 V logic | OK while the board is powered. A 5 V signal on a header pin of an **unpowered** board exceeds 3.63 V. intent.md: don't care, stated on silk. |
| VPIN, standard pins (GPIO26–29 / ADC0–3, QSPI) | −0.5 / IOVDD + 0.5 V | 0 … ADC_AVDD (ADC range, DS Table 1685, p.1338) | 3.3 V logic | OK. **GPIO26–29 are not 5 V tolerant** (DS Table 1674, pp.1332–1333: "Digital IO / Analogue", not FT). Tell `user-io.md` and the silk. |
| IIOVDD_MAX (sum of GPIO source current) | 100 mA (DS Table 1683, p.1337) | — | 50 mA aggregate budget (intent.md) | OK |
| IQSPI_IOVDD_MAX | 20 mA | — | Flash I/O plus 3.3 mA if BOOTSEL is held while SS drives high (see calc) | OK |
| Tj | −40 / 125 °C | ambient −40 / 85 °C (DS Table 1682, p.1336) | 53 °C (see calc) | OK |
| ESD | HBM 2 kV (4 kV on FT pins), CDM 500 V (DS Table 1681, pp.1335–1336) | — | Header pins unprotected (intent.md) | Accepted by intent |
| L1 current | Isat 2.8 A typ / 2.4 A ("Max" column); Irms 2.3 A typ / 2.1 A (AOTA p.1) | DS p.412: Isat ≥ 1.5 A | 237 mA normal peak; ≤ 0.8 A in current limit | OK |
| L1 temperature | −40 to +125 °C including self-heating (AOTA p.1) | — | ≈ 40 °C + ~1 °C | OK |
| L1 operating voltage | 20 V (AOTA p.1) | — | 3.4 V | OK |
| MLCC rated voltage | 10 V (4.7 µF), 16 V (100 nF), 50 V (15 pF) | — | 3.40 V | ×2.9, ×4.7, ×14.7. All meet the ≥ 1.5–2× rule. |
| Resistors | 1/16 W at ≤ 70 °C ambient, 50 V working (UR p.5) | — | ≤ 10.9 mW (1 kΩ with 3.3 V across it) | OK |
| Moisture / reflow | RP2350 QFN-60 MSL1, Tp 260 °C (DS pp.1328–1329); AOTA MSL1, 260 °C ≤ 10 s (AOTA p.1, p.5); ABM8 MSL1 (ABM8 p.1) | — | JLC lead-free reflow | OK |

## Required external parts (from the datasheets) vs ours

**Core regulator** (DS §6.1.3, 6.1.6, 6.1.7, pp.400–401; §6.3.7 Figure 22, pp.407–408; §6.3.8 Figures 25–27, pp.409–412; §6.3.8.2 component values, p.412; HDG §2.1, pp.5–7):

| Requirement | Source | Ours | Verdict |
|---|---|---|---|
| L: fully shielded, 3.3 µH ±20 %, DCR ≤ 250 mΩ, Isat ≥ 1.5 A, **polarity-marked**, placed as Figure 26; AOTA-B201610S3R3-101-T recommended | DS p.412; HDG pp.6–7 | AOTA-B201610S3R3-101-T: shielded (AOTA p.1 features), 3.3 µH tolerance code "M" (the sheet doesn't define the letter; M is the usual code for ±20 %), DCR 115 mΩ typ / 140 mΩ max, Isat 2.8 / 2.4 A, white polarity dot (AOTA p.1, p.4) | **Meets every line.** It is the exact part RP names. |
| COUT 4.7 µF ±20 %, ESR ≤ 250 mΩ, ESL ≤ 6 nH, VREG_FB taken from COUT | DS p.412 | CL05A475MP5NRNC; ESR 4.8 mΩ at 3 MHz; self-resonance 4.5 MHz at 0 V ⇒ part ESL ≈ 0.26 nH (SAMWEB; calc below). VREG_FB pin 50 on 1V1 at COUT. | OK |
| CIN ≥ 4.7 µF, ESR ≤ 50 mΩ, at VREG_VIN, own GND path to the QFN pad | DS p.401, p.411, p.412 | CL05A475MP5NRNC, 4.8 mΩ at 3 MHz | OK at nominal. At 3.3 V bias it is ≈ 2.4 µF effective; see Calculations and NOTE-2. |
| VREG_AVDD **must** be RC filtered as Figure 25: 33 Ω + 4.7 µF | DS p.411; HDG p.7 ("an RC filter of 33 Ω and 4.7 μF is adequate") | R3 33 Ω + C13 4.7 µF | OK |
| VREG_VIN and VREG_AVDD powered up together; a combined supply is allowed when VREG_VIN is in 3.135–3.63 V | DS p.401, p.407 (Figure 22) | Both from 3V3; AVDD lags by the RC τ ≈ 80 µs | OK, same as REF |
| VREG_PGND to main GND at one point | DS p.411; HDG p.5 | Pin 47 on net GND; single-point join in layout | OK (layout item) |
| DVDD: 100 nF at the two DVDD pins nearest the regulator, 4.7 µF at the farthest | DS §6.1.3 p.400 | 100 nF at pin 6, 100 nF at pin 39, 4.7 µF at pin 23 | OK. Pins 39 and 6 are nearest the regulator pins 46–50; pin 23 is on the far (bottom) edge. REF does the same (C8, C11, C10). |
| "We recommend a second 4.7 µF on VOUT … bottom edge (DVDD pin 23) … don't place this near LX/COUT" | DS p.412 | C16 is that capacitor | OK; the same part meets both rules |
| Inductor orientation: "the inductor has to be the right way around" | DS p.413; HDG pp.6–7 | REF `L_pol_2016`, pad 1 (dot) = 1V1, pad 2 = VREG_LX, as in REF | OK, but see Gotchas (Abracon's drawing) and open question 1 |

**Supply decoupling** (DS §6.1.4–6.1.5, p.401; HDG §2.2.1, pp.8–9):
- "100 nF capacitor per power pin" (HDG p.8). REF fits 7 × 100 nF for its 9 3.3 V pins. From the REF board: REF C13 → pin 1, REF C15 → pin 11, REF C18 → pin 20, REF C16 → pin 30, REF C14 → pin 38, REF C17 → pins 45 **and 44**, REF C12 → pins 53 **and 54**. HDG p.8 documents only the 53/54 sharing ("as there is not a lot of room on that side of the device, and the components and layout of the regulator take precedence").
- USB_OTP_VDD: "should be decoupled with a 100nF capacitor close to the chip's USB_OTP_VDD pin" (DS p.401). Ours: one 100 nF (C11) at the 53/54 junction, as REF. The pins are 0.4 mm apart, and the 27 Ω pair and the regulator fix that corner when the block is copied. OK, same as REF.
- ADC_AVDD: "should be decoupled with a 100nF capacitor close to the chip's ADC_AVDD pin" (DS p.401). intent.md: "tied to 3V3 with its own 100 nF; no RC filter". Ours: **dedicated C10**. REF shares REF C17 with pin 45, so intent.md's "as the vendor design does" is not quite what REF does. Placement fallback is in open question 4.
- IOVDD pins 1, 11, 20, 30, 38, 45: one 100 nF each, as REF.
- No bulk capacitor is required at U1 beyond these. REF's C19 10 µF on +3V3 sits 22 mm away by its connectors. The 3V3 bulk is `ldo-3v3.md`'s.

**Crystal** (DS §8.2.1.1 Table 629, p.507; HDG §4, pp.13–14):
- ABM8-272-T3 is the recommended part. "Pico 2 has been specifically tuned for the ABM8-272-T3" (DS p.507; HDG p.14). Ours: ABM8-272-T3. **Identical.**
- 2 × 15 pF to GND, 1 kΩ in series with XOUT, at IOVDD = 3.3 V (HDG p.13; REF C3, C4, R2). Ours: identical values. C0G 50 V for the caps, as REF (HDG Appendix A, p.19: "15p … C0G 50V"). R6 is ±1 %, as REF's (HDG p.19 "1%").
- Crystal parameters: 12.000 MHz fundamental AT, ±30 ppm tolerance, ±30 ppm stability, ESR ≤ 50 Ω, C0 ≤ 3.0 pF, CL 10 pF, drive 10 µW typ / 200 µW max, −40 to +85 °C (ABM8 p.1, identical to DS Table 629, p.507).
- 12 MHz is also what the bootrom assumes for USB boot (DS p.345; DS Table 1686 note, p.1338).

**BOOTSEL** (DS §5.2.8, pp.344–345; HDG p.10, p.18):
- The bootrom samples QSPI CSn shortly after reset, with the pin pulled up and output disabled. "You must drive the chip select low with a sufficiently low impedance to overcome the internal pull-up. A 4.7 kΩ resistance to ground is a good intermediate value" (DS p.345). HDG p.10 and REF use **1 kΩ (R6)** to a button to GND, placed "close to the flash chip". Ours: R7 1 kΩ, as REF (calc below).
- QSPI SD1 low selects the USB bootloader; SD1 high selects UART boot (DS p.345). Nothing in this block touches SD1. `flash.md` must confirm nothing pulls SD1 high.

**RUN** (DS §7.3.3, p.452; DS Table 1677, p.1334; HDG p.18):
- RUN is "Digital In (FT)" with a pull-up at reset. "If RUN is not used, it should be tied high." Ours is used, with the internal pull-up only. REF: RUN → SW2 → REF R4 1 kΩ → GND, no external pull-up or capacitor. Ours: RUN → R8 1 kΩ → SW2 → GND. Same divider and values; ours puts the resistor between the pin and the button the finger touches, which is the better side for ESD (gate-1 review #14).
- Double-tapping RUN sets CHIP_RESET.DOUBLE_TAP; the bootrom acts on it only if OTP BOOT_FLAGS1.DOUBLE_TAP is set (DS p.452; bootrom "Check BOOTSEL" step, DS p.339). A bouncing button can't cause an unexpected BOOTSEL on a blank chip.

**SWD** (RPDBG p.2; DS Table 1687, p.1339):
- "100 Ohm resistors on the SC/RX and SD/TX IC pins at the target … placed very close to the IC pins"; "not strictly required but highly recommended unless the platform is very low cost and/or space constrained". They give "slew limiting (better for signal integrity and EMC), short-circuit and ESD current limiting". REF has none: its SWCLK/SWD go straight from U1 to J3/J4.
- **Adopted:** R13 and R14, 100 Ω (C25076, basic), at pins 24/25. Our J4 is a bare 0.1" header that people will also drive with jumper wires, so the short-circuit and ESD limiting is worth two basic 0402s.
- Cost in speed: RPDBG p.2 says "~30MHz performance should still be achievable" through 200 Ω total. RP2350 SW-DP runs up to 50 MHz (DS Table 1687), but the Debug Probe's SWCLK sits well below 30 MHz (assumption: debugprobe/OpenOCD defaults, not in `datasheets/`).

**USB** (DS Table 1673, p.1332 "USB operation requires external 27Ω series resistors"; DS Table 1678, p.1334; HDG p.15; HDG p.19 note "Make sure R7 and R8 are close to RP2350"; REF R7/R8 are our R5/R4): value audited in `usb-front-end.md`. Here, **placement only:** at pins 51/52 at the REF positions (Layout).

## Calculations

**1. Core regulator inductor ripple and peak.**
fsw = 3 MHz typ (DS Table 1689, p.1342). VIN = 3.20–3.40 V, VOUT = 1.1 V, L = 3.3 µH.
- D = VOUT/VIN = 0.344 (3.20 V) … 0.324 (3.40 V).
- ΔI(p-p) = (VIN − VOUT)·D / (fsw·L): at 3.20 V, 2.10 × 0.344 / (3 MHz × 3.3 µH) = **73 mA**; at 3.40 V, 2.30 × 0.324 / 9.9 = **75 mA**.
- At IMAX = 200 mA (DS Table 1689, p.1341): Ipk = 200 + 75/2 = **238 mA**.
- Current limit: 240–300 mA at start-up; 260 / 500 / 800 mA min/typ/max running (DS Table 1689, p.1341). Worst inductor current in a fault or limit event: **0.8 A**.
- AOTA p.2 "Saturation Current" curve: L ≈ 3.3 µH at 0.24 A, ≈ 3.1–3.2 µH at 0.8 A (read from the graph, < 6 % roll-off). Isat (−30 %) = 2.4 A, 10× the normal peak, 3× the current-limit maximum. DS p.413: "an inductor with low roll-off is required for best operation". Met.
- DCR 140 mΩ max (AOTA p.1): loss at 200 mA = 0.2² × 0.14 = **5.6 mW**; the drop is inside the loop (VREG_FB at COUT). AOTA p.2 temperature-rise curve: < 2 °C at 0.24 A.
- SRF 21 MHz min (AOTA p.1), 7× fsw. OK.

**2. CIN and COUT effective capacitance and ripple** (CL05A475MP5NRNC; SAMWEB DC-bias data at 25 °C, 1 kHz; tolerance ±20 % from the part number, SAM p.4; X5R ±15 % over −55 to 85 °C, SAM p.5).

| Bias | ΔC (SAMWEB typ) | C typ | C worst (−20 % tol, −1.5 % at 40 °C per SAMWEB TCC; it is +1.5 % at 0 °C) |
|---|---|---|---|
| 1.1 V (COUT, C16) | −9.5 % | **4.25 µF** | 3.35 µF |
| 3.3 V (CIN, CFILT) | −48.3 % | **2.43 µF** | 1.92 µF |
| 3.4 V | −49.7 % | 2.36 µF | 1.86 µF |

- Comparison with the reference's 6.3 V part (HDG p.19: "4.7u 6.3V X5R 20%", maker not stated). Samsung's 6.3 V equivalent CL05A475MQ5NRN (SAM p.19 row 33) loses −14.6 % at 1.1 V and −59.4 % at 3.3 V (SAMWEB). **Our 10 V part keeps more capacitance on both rails than that 6.3 V equivalent** (the reference's actual 6.3 V part is unknown).
- COUT "4.7 µF ±20 %" (DS p.412): ours is a 4.7 µF ±20 % part. The DS doesn't say whether ±20 % is nominal or in-circuit; at 1.1 V ours is 4.25 µF typ, within 20 % of 4.7.
- CIN "at least 4.7 µF" (DS p.412): met as a nominal rating, **not** as in-circuit capacitance (2.43 µF typ). REF's 6.3 V part would be ≈ 1.9 µF on the same basis. We match the validated circuit. NOTE-2.
- ESR at 3 MHz: **4.8 mΩ** (SAMWEB R curve, 0 V). Limits: ≤ 50 mΩ (CIN), ≤ 250 mΩ (COUT). OK.
- ESL: |Z| minimum at 4.53 MHz with 0 V bias ⇒ L = 1/((2π × 4.53 MHz)² × 4.7 µF) = **0.26 nH** (part only; |Z| at 100 MHz = 114 mΩ ⇒ 0.18 nH, consistent). COUT limit ≤ 6 nH (DS p.412). The mounting adds to this, so keep the REF small-pad footprint and placement.
- CIN RMS ripple ≈ IOUT·√(D(1−D)) = 0.2 A × √(0.344 × 0.656) = **95 mA**; loss 0.095² × 4.8 mΩ = 43 µW. Negligible.
- Output ripple ≈ ΔI/(8·fsw·C) + ΔI·ESR = 0.075/(8 × 3 MHz × 4.25 µF) + 0.075 × 0.0048 = 0.74 + 0.36 = **≈ 1.1 mV** (typ C). Against the ±100 mV DVDD transient allowance (DS Table 1688 note b, p.1341). C16 and the two 100 nF add to this.

**3. VREG_AVDD RC filter (33 Ω + 4.7 µF).**
- DC drop: 33 Ω × 220 µA (DS Table 1693, p.1344, USB boot) = **7.3 mV**. With RUN held low it is 187 µA (DS Table 1692, p.1344) → 6.2 mV. VREG_AVDD worst = 3.20 − 0.0073 = **3.193 V ≥ 3.135 V**.
- Corner: fc = 1/(2π·R·C). C = 2.43 µF typ → **1.98 kHz**; C = 1.92 µF worst → 2.51 kHz; unbiased 4.7 µF → 1.03 kHz.
- Attenuation at fsw = 3 MHz, ideal single pole: 20·log(3 MHz / 2.51 kHz) = **61.5 dB** worst. Above self-resonance the cap is limited by |Z| ≈ 12 mΩ at 3 MHz (SAMWEB, 0 V), giving 20·log(33 / 0.012) ≈ 69 dB, so the 61.5 dB pole estimate is the binding one.
- Power-up lag: τ = 33 Ω × 2.43 µF = **80 µs** (155 µs unbiased). DS p.401 asks that VREG_VIN and VREG_AVDD come up "together"; this lag is the reference design's own and much shorter than any LDO soft-start.
- R power: 33 Ω × (0.22 mA)² = 1.6 µW. The part is ±1 % (0402WG**F**330**J**TCE: 7th character F = ±1 %, then "330" with multiplier J = 10⁻¹ ⇒ 33.0 Ω; UR p.2 §2.3–2.4.3), the same tolerance as REF's R5. At +1 % the drop is still 7.3 mV.
- Bounding case: DS p.409 says a regulator left running "will consume approximately 400 µA until it's shut down" (context: external DVDD, regulator idle). If all of that flowed through VREG_AVDD the drop would be 13.2 mV → 3.187 V, still 52 mV above 3.135 V.

**4. Decoupler DC bias** (SAMWEB, 25 °C):
- CL05B104KO5NNNC 100 nF X7R 16 V: −3.9 % at 3.3 V → **96 nF**; +0.1 % at 1.1 V → 100 nF. ESR 25 mΩ at 3 MHz, self-resonance 33 MHz (0 V). Rating margin 16/3.4 = 4.7×.
- CL05A475MP5NRNC at pin 23 (1.1 V): 4.25 µF typ (above).
- For other audits: CL21A106KAYNNNE 10 µF X5R 25 V 0805 (SAM p.21 row 26): −30.5 % at 3.3 V (6.95 µF), −50.3 % at 5.0 V (4.97 µF), −54.8 % at 5.5 V (4.52 µF) (SAMWEB). Relevant to the ≤ 10 µF VBUS attach limit: a nominal 10 µF part counts at its unbiased value for that limit, so the USB audit must decide which figure it uses.

**5. Crystal load capacitors** (HDG p.13 method).
- CL = C20·C21/(C20 + C21) + Cstray = 15/2 + 3 = **10.5 pF** against the 10 pF spec (ABM8 p.1). Cstray = 3 pF is **HDG's assumption** (p.13: "We'll assume a value of 3 pF"), not a measurement.
- With 15 pF ±5 % (J, FH p.4): 7.13–7.88 + 3 = 10.1–10.9 pF.
- C0G: 0 ± 30 ppm/°C (FH p.5) ⇒ over 0–40 °C the caps move < 0.1 %. Negligible.
- Our board is 4 layers, so ground will sit closer under the XIN/XOUT tracks than on REF's 2-layer board (HDG p.15: "almost the entirety of the bottom copper layer is devoted to ground"). Stray could exceed 3 pF. Estimate (gate-1 review #4): a 0.2 mm trace over ~0.2 mm prepreg adds roughly 0.08–0.1 pF/mm (**assumption**; the stackup isn't fixed yet), so the ~3 mm XIN/XOUT runs at the REF geometry (Y1 5.4 mm below U1's edge) add ≈ 0.3 pF each. That is small against CL = 10 pF and inside the slack of HDG's 3 pF. **Keep layer 2 solid under Y1, C20, C21, R6 and the XIN/XOUT/XOUT_R traces.** Removing the reference plane would raise their coupling to VREG_LX (≈ 5 mm away) and QSPI_SCLK, which matters more than 0.3 pF. The frequency effect can't be computed: ABM8 p.1 gives no motional capacitance C1, so pulling (ppm/pF) is unknown. Required accuracy: crystal ±30 ppm tolerance + ±30 ppm stability; the USB 2.0 full-speed ±2500 ppm data-rate tolerance comes from the USB 2.0 specification, which is not in `datasheets/` (**assumption by reference**). A pF-level CL error is orders of magnitude inside that budget.
- Drive level: 10 µW typ / 200 µW max (ABM8 p.1). "1 kΩ series resistor (R2) [REF R2, our R6] is a good value to prevent the crystal being over-driven and being damaged when using an IOVDD level of 3.3 V" (HDG p.13). Our IOVDD is 3.3 V and the values are the reference's. The XOSC drive amplitude isn't published, so drive level can't be computed: **accepted on HDG's validation, not calculated.**
- R6 power: below µW. Not a concern.
- XOSC start-up: STARTUP_DELAY = (12 MHz × 1 ms)/256 ≈ 47; the SDK default "is sufficient for the Hardware design with RP2350" (DS pp.507–508).

**6. BOOTSEL resistor (1 kΩ).**
- Button pressed during the sample: internal pull-up RPU = 32–86 kΩ at IOVDD = 3.3 V (DS Table 1683, p.1337). V(QSPI_SS) = 3.3 × 1/(1 + RPU) = 3.3/33 … 3.3/87 = **0.100 … 0.038 V** < VIL max 0.8 V (DS Table 1683, p.1336). Margin 0.7 V. (The 4.7 kΩ that DS p.345 suggests would give 0.42 V worst, also valid.)
- Button held while U1 drives SS high: I = 3.3 V / 1 kΩ = **3.3 mA**. That is below the 4 mA default drive (DS Table 1673, p.1331), where VOH ≥ 2.62 V (DS Table 1683, p.1336). Whether 2.62 V is a valid high for the flash's /CS is checked in `flash.md`. QSPI_IOVDD total 20 mA limit: OK.
- R power worst: 3.3² / 1 kΩ = **10.9 mW** < 62.5 mW (1/16 W, UR p.5). OK.
- Why 1 kΩ and not 4.7 kΩ: it is the validated reference value (HDG p.10, REF R6). Both meet VIL; 1 kΩ has more margin against a finger-ESD or noisy button lead.

**7. RUN resistor (1 kΩ).**
- Pressed: V(RUN) = 3.3 × 1/(1 + 32…86) = **0.10 … 0.04 V** < 0.8 V (DS Table 1683; RUN is FT with pull-up, DS Table 1677, p.1334). OK.
- RUN is also on a header pin (intent.md). If something outside drives RUN high while the button is pressed: 3.3 mA, 10.9 mW in the resistor. That is the resistor's job. OK.
- No external pull-up or capacitor (as REF; the resistor is on the pin side of the switch, unlike REF). The DS gives no minimum RUN pulse width; a human press is milliseconds.

**9. SWD series resistors (100 Ω).**
- Normal use with a Raspberry Pi Debug Probe: it has its own 100 Ω at the host (RPDBG p.2), so a fault or contention current is ≤ 3.3 V / 200 Ω = **16.5 mA**, and our resistor dissipates 16.5² mA² × 100 Ω = **27 mW** < 62.5 mW (UR p.5).
- Worst case, an unterminated 3.3 V driver (jumper wire from another board) fighting SWDIO driven low: the current is limited by our 100 Ω plus both drivers' output resistance. The RP2350 pin is specified only at 2–12 mA (DS Table 1683, VOL ≤ 0.5 V at the selected drive), so the current isn't bounded by any datasheet figure. If it were 33 mA (100 Ω alone), the resistor would dissipate 109 mW, above its 62.5 mW rating, but only while firmware drives the line into a short. SWDIO is driven only during SWD read turnarounds. Accepted.
- Rise time: 200 Ω × (pin + trace capacitance, a few pF; **assumption**, the DS gives no SWD pin capacitance) ⇒ τ ≈ 1–2 ns, negligible at ≤ 30 MHz.
- ESD: 100 Ω in series ahead of an FT pad rated HBM 4 kV (DS Table 1681). Helps; not quantified.

**8. Junction temperature.**
- θJA = 40.542 °C/W, still air (DS Table 1670, p.1325).
- P: intent.md budgets 100 mA at 3.3 V for the RP2350 = 0.33 W. That over-states die dissipation, since part of IOVDD current leaves through GPIO loads. The measured `hello_usb` case is 52.7 mW total (DS Table 1693, p.1344).
- Tj = 40 + 0.33 × 40.5 = **53.4 °C** worst-budget, against 125 °C. OK. The LDO's 0.62 W (intent.md) will warm the board; `ldo-3v3.md` and placement should keep U3 away from U1 and Y1.
- Regulator input current at its 200 mA maximum: 200 mA × 1.1 V / (3.3 V × 0.59) = **113 mA** from 3V3 (59 % efficiency at 200 mA, 3.3 V, DS Table 1689, p.1342). The intent's 100 mA RP2350 budget covers realistic use (both cores at 150 MHz: the single-core CoreMark case draws 11.0 mA at VREG_VIN, DS Table 1693). A pathological 200 mA core load would need about 115 mA, still inside the 277 mA rail total.

## Fine print / gotchas

- "The most critical part of an RP2350 PCB layout is the core voltage regulator. This should be placed first on any board design and these guidelines must be strictly followed." (DS p.409). "Designers should follow the above schematic Figure 25 and layout Figure 26 as closely as possible" (DS p.411). HDG p.7: "if you choose not to use our example, then you do so at your own risk."
- "Raspberry Pi is still working with the regulator IP vendor to fully verify and qualify the regulator and custom inductor." (DS p.413).
- **Inductor polarity: the Abracon and RP drawings disagree on what the dot means.** DS Figure 28 (p.413): current enters at "+" and leaves at "−", and the orientation indicator is at the **"−" (current-out) end**. DS Figure 25 (p.410) draws the dot at the VOUT end of Lx. REF's `L_pol_2016` has the dot at pad 1, and REF ties pad 1 to +1V1. These three agree: **dot at the 1V1 end**. AOTA p.4 ("Marking (White Ink polarity dot)") shows the dot next to the "+" terminal, with the current arrow going from + to −, which reads as current *entering* at the dot. **We follow RP** (DS figures plus the validated REF footprint and netlist). Don't re-derive the orientation from the Abracon sheet. Open question 1 covers checking it at assembly.
- Regulator modes: "The regulator must be in normal mode before the core supply current is allowed to exceed 1mA" (DS p.406). "The regulator can't be relocked after it's been unlocked" (DS p.406). VOUT is limited to 1.3 V unless DISABLE_VOLTAGE_LIMIT is set (DS p.406); "RP2350 might not operate reliably with … DVDD at a voltage other than 1.1 V" (DS p.406). Firmware items; the SDK defaults are safe.
- "VREG_AVDD also powers the chip's power-on reset and brownout detection blocks, so it must be powered even if the on-chip voltage regulator is not used" (DS p.401).
- POR threshold DVDD 0.924–0.99 V, assertion delay 3–10 µs (DS Table 570, p.462). VOUT_OK asserts at 87–93 % of VOUT (DS Table 1689, pp.1341–1342).
- ADC_AVDD before DVDD causes "small transient currents … This will not damage the chip … acceptable behaviour" with a single 3.3 V supply (DS p.401). The voltage on ADC inputs must not exceed IOVDD (DS p.401).
- USB_OTP_VDD "must always be provided, even in applications where the USB PHY is never used" (DS p.401).
- Pin reset states (DS Tables 1674–1677, pp.1332–1334): every GPIO resets with a **pull-down**, so D1 (GPIO25) starts off and U4's input (GPIO23) starts low. QSPI_SS, SD2, SD3, RUN, SWCLK and SWDIO reset with a pull-up; SCLK, SD0 and SD1 with a pull-down. Tell `rgb-leds.md`: the LED data line idles low through reset.
- HDG p.14 promises a crystal specification table that is missing from this build; DS Table 629 (p.507) and ABM8 p.1 agree with each other.
- HDG p.7's recap (and the Figure 7 render on p.8) calls the 33 Ω "R3"; the REF KiCad schematic designates it **REF R5**; ours is R3. Same part.
- **HDG Figure 6 (p.8) shows REF C8, C10 and C11 all as 100 nF on +1V1, but the REF KiCad files have REF C10 = 4.7 µF at pin 23.** DS §6.1.3 (p.400) asks for 4.7 µF at the DVDD pin farthest from the regulator (distances from pin 48: pin 39 ≈ 3.3 mm, pin 6 ≈ 6.0 mm, pin 23 ≈ 7.2 mm). We follow DS + KiCad: C16 = 4.7 µF at pin 23. Don't "correct" it from the HDG figure.
- DS p.412: "COUT … must be placed between VREG_VIN and VREG_PGND as close to the pins as practically possible". In Figure 26 (p.411) and REF, COUT sits between L1 and CIN above pins 48–50. Follow the figure and REF coordinates.
- "Don't place any of CIN/LX/COUT on the opposite side of the PCB" (DS p.411). intent.md puts every part on top. Consistent.
- "With the exception of the two voltage regulator supplies (VREG_VIN and VREG_AVDD), which should be powered up together, RP2350's power supplies may be powered up or down in any order" (DS p.401). One 3V3 rail for everything satisfies this.
- SWD: "Always connect ground directly between the SWD probe and RP2350" (DS p.1339). J4 order is 1 SWCLK, 2 GND, 3 SWDIO (RPDBG p.1: pin 1 SC, 2 GND, 3 SD; `user-io.md`).
- RPDBG p.2 requires "3.3V failsafe pads" at the target: the pins must survive 3.3 V + 10 % while unpowered. SWCLK and SWDIO are FT, and DS Table 1680 (p.1335) allows 3.63 V at IOVDD = 0 V. OK.
- Crystal pads: the crystal element connects pads 1 and 3 only; pads 2 and 4 are not connected to it (ABM8 p.2 bottom view, read at 400 dpi). REF ties 2 and 4 to GND. "The chamfer could be located on pin #1, 3, or 4" (ABM8 p.2), so the chamfer is **not** a pin-1 mark. Because 1/3 and 2/4 are diagonal pairs, a 180° rotation is harmless; a **90° rotation would connect the crystal across GND pads**. Check rotation at the assembly preview.

## Deviations from the reference design (REF / HDG)

| # | REF | Ours | Why |
|---|---|---|---|
| 1 | 2-layer board, solid GND on the bottom under the regulator | 4 layers (intent.md, expected) | DS p.412 then **requires** cutting away layer-2 copper under the LX node (Figure 27). Layout item; REF had no inner layers to cut. |
| 2 | 4.7 µF "6.3V X5R 20%" 0402 (HDG p.19) | CL05A475MP5NRNC, **10 V** X5R 20 % 0402 | Basic JLC part. Keeps more capacitance at 1.1 V and 3.3 V than Samsung's 6.3 V equivalent (Calc 2). Same size and footprint. |
| 3 | ADC_AVDD pin 44 shares REF C17 with IOVDD pin 45 | Dedicated C10 100 nF | DS p.401 "should be decoupled with a 100nF capacitor close to the chip's ADC_AVDD pin"; intent.md "its own 100 nF". One basic 0402. If placement can't fit it beside pin 44 without blocking the GPIO26–29 fan-out, revert to REF's sharing and record it (open question 4). |
| 4 | REF R9 (DNF) / REF R10 (0 Ω) let GPIO0 act as a second QSPI chip select; REF R1 10 kΩ DNF pull-up on the flash CS | QSPI_SS straight to the flash; no REF R9/R10 equivalent | HDG p.11: the REF R9/R10 option serves RP2354 parts with in-package flash. RP2350A has none. REF R1 (the DNF pull-up) is `flash.md`'s decision. |
| 5 | 100 nF "25V" X7R (HDG p.19) | 16 V X7R (C1525, basic) | −3.9 % at 3.3 V (Calc 4); 4.7× rating margin. |
| 6 | Vendor symbol `RP2350_60QFN` draws every power pin separately, so each decoupler is wired to its own pin | Project copy of `MCU_RaspberryPi:RP2350A` **keeping** the stacked IOVDD (1/11/20/30/38/45) and DVDD (6/23/39) pins. The decouplers are drawn as banks on the 3V3 and 1V1 rails, and each carries a **`Decouples` field naming its U1 pin** (table under Schematic connections). The field travels to the PCB, where the placement checks read it. | Same netlist; the pin binding moves from schematic wiring to a field the PCB tools can check. (Gate-1 review #1: this row used to propose unstacking; the schematic uses the field instead, and the row now describes what the schematic does.) |
| 7 | SWCLK/SWD straight from U1 to J3/J4 | 100 Ω in series at U1 pins 24/25 | RPDBG p.2 "highly recommended" at the target; short-circuit and ESD limiting on a bare header. |
| 8 | Micro-USB, NCP1117 LDO, Würth buttons, 2 × 18 headers | USB-C, other LDO, other buttons, 1 × 20 headers | Not this block; see the other audits. Nothing in the RP2350 core changes because of them. |

Everything else in this block is the reference's values, parts and topology: L1 part, CIN/COUT/CFILT values, 33 Ω, DVDD decoupling, ABM8-272-T3 with 2 × 15 pF and 1 kΩ, 1 kΩ BOOTSEL and RUN resistors, 27 Ω placement.

## Footprints

**U1. Recommendation: REF's `RP2350-QFN-60-1EP_7x7_P0.4mm_EP3.4x3.4mm_ThermalVias`**, copied into the project library.

| | DS Figure 148 (p.1325) recommended land | REF footprint | KiCad `QFN-60-1EP_7x7mm_P0.4mm_EP3.4x3.4mm` | KiCad `…_ThermalVias` |
|---|---|---|---|---|
| Signal pads | 0.20 wide, 0.875 long, outer span 7.75, inner span 6.00, row length 5.80, pitch 0.40 | 0.20 × 0.875 at 3.4375 from centre ⇒ 7.75 / 6.00. **Exact.** | 0.20 × 0.80 at 3.45 ⇒ 7.70 / 6.10 | as plain |
| Exposed pad | 3.40 × 3.40 | 3.40 × 3.40 | 3.40 × 3.40 | 3.40 × 3.40 |
| Thermal vias | not specified | 9 × 0.6 mm pad / 0.25 mm drill | none | 9 × 0.5 / 0.2 mm drill |
| EP paste | not specified | 4 × 1.2 × 1.2 = 50 % of EP | 9 × 0.91² = 64 % | 4 × 1.32² = 60 % |
| Pin 1 | top-left, counter-clockwise (DS Figure 152) | (−3.44, −2.8) | (−3.45, −2.8) | same |

Package (DS Figure 147, p.1324): body 7 × 7, terminal b = 0.13–0.23 (0.18 nom), L = 0.35–0.45, D2 = E2 = 3.35–3.45, pitch 0.40. All four footprints fit the package; the stock pads are 0.05–0.075 mm shorter than RP's recommendation, which matters little for soldering. **Reasons for REF's:** (a) it is RP's own recommended land exactly; (b) the transplanted 1V1 ring (±2.2) and 3V3 band (±2.8) zones were drawn against these pad ends; (c) its 0.25 mm via drill is easier to fab than the stock 0.2 mm. Its EP vias are untented and have no paste over them. Check with the stackup (phase 4) that JLC builds 0.25 mm drills; `checks.toml`'s default via_drill_mm is 0.3 (open question 6).

**L1. Recommendation: REF's `L_pol_2016`**, copied into the project library. Pads 0.7 × 1.7 at x = ±0.7 (span 0.35–1.05 mm from centre). Silk dot and fab dot on pad 1, which is +1V1. AOTA p.4 gives a different recommended land: pads 1.0 × 1.6 with a 1.0 gap (span 0.5–1.5 mm). The body is 2.00 ± 0.20 × 1.60 ± 0.20 × 1.0 max and the terminals 0.60 ± 0.20 long (AOTA p.4; DS Figure 29, p.414), so nominal terminals span 0.4–1.0 mm from centre. REF's pads cover them, with no outside toe. Use REF's: it is the land RP validated, and Abracon's would move the pad centres 0.3 mm outward under the copied LX and 1V1 zones. Cosmetic fixes after copying: its value text says "4.7u" and its 3D model points at `AOTA-B201610SR47MT.STEP` under `KICAD7_3DMODEL_DIR`.

**CIN/COUT. REF's `C_0402_1005Metric_small_pads`** (pads 0.47 × 0.53 at ±0.515), as REF's C6/C7. The chip note says so too.

**Y1: `Crystal:Crystal_SMD_3225-4Pin_3.2x2.5mm` is correct** (REF uses the same). Pads 1.4 × 1.2 at (±1.1, ±0.85). ABM8 p.2 land: 1.4 × 1.1 pads, 0.8 mm horizontal gap (⇒ ±1.1 centres, matching), 1.6 mm vertical. With 1.1 mm pads, 1.6 can only be the centre spacing (an outer span would overlap the pads), against KiCad's 1.7. KiCad's land is 0.1 mm taller per pad and 0.05 mm further out per side; the gap stays 0.5 mm. Fine. Pads 1 and 3 are diagonal in both, matching the crystal element.

**Passives:** stock `C_0402_1005Metric` (0.56 × 0.62 at ±0.48) and `R_0402_1005Metric` (0.54 × 0.64 at ±0.51). REF uses these for every 0402 except C6/C7.

## Layout guidance (for placement, phase 6)

Coordinates are mm relative to U1's centre, KiCad Y-down (negative Y is the top edge, pins 46–60), U1 at 0°. They come from the REF `.kicad_pcb`, read by script on 2026-10-23, and agree with `chips/rp2350.md`. Transplant them as the chip note says; if U1 is rotated, rotate the whole set.

| Ours | REF | Serves | Position | Rot | Notes |
|---|---|---|---|---|---|
| L1 | L1 | VREG_LX → 1V1 | (2.00, −7.20) | 0 | Pad 2 (LX) on the right, pad 1 (dot, 1V1) on the left |
| C15 (COUT) | C7 | 1V1 at VREG_FB | (2.00, −5.55) | 0 | small pads |
| C14 (CIN) | C6 | pin 49 | (2.00, −4.60) | 0 | small pads |
| C13 (CFILT) | C9 | pin 46 | (4.20, −5.05) | −90 | own GND via, not shared with CIN/COUT |
| R3 (RFILT) | R5 | 3V3 → pin 46 | (4.20, −6.90) | −90 | |
| R5 | R7 | pin 52 | (−1.10, −7.40) | −90 | value per `usb-front-end.md` |
| R4 | R8 | pin 51 | (−0.10, −7.40) | −90 | |
| C11 | C12 | pins 53 + 54 | (−2.10, −7.40) | 90 | |
| C4 | C13 | pin 1 | (−6.48, −3.40) | 180 | |
| C17 | C8 | pin 6 | (−6.48, −1.00) | 180 | |
| C5 | C15 | pin 11 | (−6.48, 1.40) | 180 | |
| C6 | C18 | pin 20 | (−1.80, 6.08) | −90 | |
| C16 (4.7 µF) | C10 | pin 23 | (0.80, 6.88) | −90 | away from LX and COUT (DS p.412) |
| R6 | R2 | pin 22 | (−0.40, 6.90) | −90 | |
| C20 | C3 | XIN | (−1.40, 9.00) | 180 | |
| C21 | C4 | XOUT_R | (1.58, 9.00) | 0 | |
| Y1 | Y1 | | (0.00, 12.40) | 0 | |
| C7 | C16 | pin 30 | (5.69, 4.00) | 0 | |
| C8 | C14 | pin 38 | (7.08, 0.40) | 0 | |
| C18 | C11 | pin 39 | (7.08, −0.60) | 0 | |
| C9 | C17 | pin 45 (REF: also 44) | (6.48, −3.00) | 0 | |
| C10 | — (new) | pin 44 | as close to pin 44 as the GPIO26–29 fan-out allows; between C9 and C18 (REF C17 and REF C11 positions) | — | placement decides; fallback: share C9 as REF |

Copper (REF zones and vias, read from the `.kicad_pcb`; see `chips/rp2350.md` for the full list):
- Top-layer zones: VREG_LX pin 48 → L1 pad 2 (priority 5); 1V1 L1 pad 1 → COUT → spoke vias (priority 6); 3V3 pin 49 → CIN (priority 7); PGND as a **GND-net** zone from pin 47 to the CIN/COUT grounds (priority 4), bounding box (2.30, −5.80)–(3.40, −3.15); under the chip a 1V1 ring ±2.2 (priority 3, thermal) and a 3V3 band ±2.8 (priority 1, solid). Every zone in this block has 0.15 mm clearance.
- PGND reaches the GND plane at **one point through two adjacent vias at (3.10, −4.90) and (3.10, −5.50)** (DS p.412: "from the high-current GND at one single point (using 2 adjacent vias to reduce the impedance)").
- CFILT: "must also have a low impedance and short-as-possible path back to the QFN GND pad (don't share any GND vias with the CIN/COUT high current GND)" (DS p.412).
- CIN "needs its own separate GND via / low impedance path back to the RP2350 GND pad" (DS p.411).
- VREG_FB "should be fed from the output of COUT, avoiding routing directly underneath LX" (DS p.412).
- "On the top layer make sure to cut away any extra copper underneath the inductor, cut back copper near the VREG_LX trace" (DS p.411–412). **Layer 2: a rule-area cut-out under the VREG_LX zone and L1's LX pad only** (DS p.412, Figure 27). Required on our 4-layer board. Layer 2 stays solid everywhere else, including under Y1, C20, C21, R6 and the crystal traces (Calc 5).
- 1V1 spoke vias 0.6/0.25 mm: N (0.49, −2.15) → (0.50, −5.80) and (0.50, −5.20) at COUT; S (0.20, 2.15) → (0.20, 5.80) at pin 23; W (−2.15, −0.80) → (−5.20, −1.00) at pin 6; E (2.15, −0.40) → (5.80, −0.40) at pin 39. 1V1 never leaves this area and is never a thin wire between spoke and ring.
- **TP3 (1V1 test point)** attaches at the DVDD pin 23 end, beside C16 / the S spoke via at (0.20, 5.80). **Never at L1 pad 1 or COUT (C15):** a stub there sits in the high-current switching loop (DS p.411, "keep the loop area … as small … as possible"). Gate-1 review #5.
- Each decoupler sits at its pin on the top side, with its GND straight to a via. "It is important to place decoupling close to the power pins" (HDG p.8).
- Crystal: "Try and keep the layout as short as possible" (HDG p.13). XIN/XOUT/XOUT_R stay short and away from VREG_LX, QSPI_SCLK and USB. R6 sits at pin 22, so the long section is XOUT_R. Load-cap grounds go straight to a via.
- SWD: R13 and R14 as close to pins 24/25 as the crystal and DVDD parts allow ("placed very close to the IC pins", RPDBG p.2). Pins 24/25 are at (0.4, 3.45) and (0.8, 3.45), directly above C16 (0.80, 6.88). The REF parts at that edge (C16, R6, C6, Y1 and its caps) keep their positions. Put the two resistors just right of C16, about x = 1.6–2.4, y = 5–6, with short top-layer stubs from pins 24/25 passing between pin 23's spoke via (0.20, 5.80) and RUN (pin 26 at x = 1.2). Then SWCLK/SWDIO run to J4 with a GND return beside them (DS p.1339). Placement confirms the fit.
- RUN: on the board it goes to header pin 30 (J3.10) and crosses the GPIO16–22 fan-out (`user-io.md`), so plan an inner-layer route from pin 26.
- QSPI_SS/BOOTSEL: R7 sits on the QSPI_SS line **near the flash**, as HDG p.10 says for REF R1/R6 ("placed close to the flash chip, so we avoid additional lengths of copper tracks which could affect the signal"); the button lead hangs off its far side (net BOOTSEL_SW). Any stub on QSPI_SS itself stays short.
- RUN: R8 near SW2 (REF puts R4 by the button). RUN is a slow net, routed after the critical ones.
- **Unlabelled local stubs (gate-1 review #2).** SWCLK_MCU (U1.24–R13), SWDIO_MCU (U1.25–R14), BOOTSEL_SW (R7–SW1) and RESET_SW (R8–SW2) have no net labels in the schematic. They are 2.5–7.6 mm local connections, which the schematic style guide allows, and they appear in the netlist as `Net-(U1-SWCLK)`, `Net-(U1-SWDIO)`, `Net-(R7-Pad2)` and `Net-(R8-Pad2)`. Their only constraint is placement: **R13/R14 at U1 pins 24/25, R7 at the QSPI_SS line near the flash, R8 at SW2.** XIN and XOUT are labelled.
- USB 27 Ω pair at pins 51/52 ("placed close to the chip", HDG p.15; Appendix A note p.19). Route from there as a pair over unbroken ground per `usb-front-end.md`.
- Thermal: the EP's 9 vias to the GND plane are the main heat path (θJB 12.6 °C/W, DS Table 1670). Keep the inner GND plane under the EP solid, apart from the LX cut-out at the corner.
- After routing (chip note checks): every RP2350 power zone is present, filled and connected; no foreign net crosses inside the pad ring; the layer-2 cut-out exists; render the regulator corner beside REF and compare.

## Open questions

1. **Inductor orientation at assembly.** RP's figures and footprint put the dot at the 1V1 end; Abracon's own drawing (AOTA p.4) seems to say the opposite (Gotchas). We follow RP. At the JLC order, check in the placement preview that L1's dot lands on footprint pad 1 (1V1), and check a first-article board. A wrong orientation still produces an output, but regulation suffers "especially at higher output currents" (DS p.413). Proposed test: measure 1V1 at the TP under a CoreMark load.
2. **L1 stock: 191 at JLC** (2026-10-23). Five boards need 5 plus attrition, so it is fine today, but there is no drop-in alternative: DS p.412 requires a polarity-marked part, and AOTA-B201610S3R3-101-T is the only one RP names. Re-check stock at order time.
3. **CIN "at least 4.7 µF".** Nominal ours is 4.7 µF; in circuit at 3.3 V it is ≈ 2.4 µF typ (Calc 2). That is more than REF's 6.3 V part would keep, so no change is proposed. If the reviewer reads the DS as in-circuit capacitance, the fix is a second 4.7 µF beside CIN, which would break the copied geometry. Not recommended without RP's word.
4. **C10 placement.** Does a dedicated 0402 fit beside pin 44 without blocking GPIO26–29 (pins 40–43), which must reach the J2/J3 headers? On 4 layers those can drop to vias, so it probably fits. If it doesn't, share C9 exactly as REF does and log the decision.
5. **JLC economic vs standard PCBA for the 0.4 mm-pitch QFN-60** (intent.md known risk). None of the sources here says; the JLC lookup returns stock and library type only. Settle it from JLC's assembly-capability page or a quote before gate 1 closes. The cost target in intent.md depends on it.
6. **Via drill 0.25 mm** in REF's EP thermal vias and 1V1 spokes, against `checks.toml`'s default `via_drill_mm = 0.3`. Phase 4 must confirm JLC's 4-layer minimum and either allow 0.25 mm in this region (logged) or enlarge the vias. Enlarging changes the copied spoke geometry.
7. **Datasheets:** FH and UR are now in `datasheets/` (`fh-0402cg.pdf`, `uniroyal-0402wg.pdf`) and cited. Samsung's per-part characteristic data (SAMWEB), which Calcs 2–4 rest on, is still web-only. Save the pages ("Download This Page (.PDF)") or the extracted JSON for CL05A475MP5NRN and CL05B104KO5NNN under `datasheets/` and add them to `sources.csv`.
8. **Net names.** As built: U1.52/51 → USB_DP_MCU/USB_DM_MCU → R5/R4 → USB_DP/USB_DM (connector side, `usb-front-end.md`). XIN, XOUT, XOUT_R, VREG_LX and VREG_AVDD are labelled. SWCLK_MCU, SWDIO_MCU, BOOTSEL_SW and RESET_SW stay unlabelled local stubs (Layout); if `nets.csv` wants entries for them, key them on the auto-names.
9. **3V3 limits for `ldo-3v3.md`:** the LDO must hold ≥ 3.20 V at full load so VREG_AVDD keeps its 58 mV margin over 3.135 V, and must never exceed 3.63 V including load-step overshoot, because that is IOVDD's **absolute maximum**, not only its operating limit.
10. **Crystal stray on 4 layers** (Calc 5) is an estimate; layer 2 stays solid under the crystal. HDG p.13: "Any deviation from the crystal circuit shown here … will require extensive testing". Our circuit is identical but the layer stack is not. Bring-up check: confirm the XOSC starts (STATUS_STABLE) at room temperature and, if practical, at 0 °C and 40 °C (the intent's range).

## Schematic connections

Real designators from the schematic (`hw/easy2350/a/easy2350.kicad_sch`; netlist exported 2026-10-23 and checked against this section). All passives are 0402 unless stated. Every passive pin not on the listed signal net goes to GND.

**U1 RP2350A** (LCSC C42411118; footprint: project copy of REF `RP2350-QFN-60-1EP_7x7_P0.4mm_EP3.4x3.4mm_ThermalVias`; symbol: project copy of `MCU_RaspberryPi:RP2350A`, IOVDD and DVDD pins stacked as in the stock symbol)

| Pin | Name | Net |
|---|---|---|
| 1 | IOVDD | 3V3 |
| 2 | GPIO0 | GPIO0 |
| 3 | GPIO1 | GPIO1 |
| 4 | GPIO2 | GPIO2 |
| 5 | GPIO3 | GPIO3 |
| 6 | DVDD | 1V1 |
| 7 | GPIO4 | GPIO4 |
| 8 | GPIO5 | GPIO5 |
| 9 | GPIO6 | GPIO6 |
| 10 | GPIO7 | GPIO7 |
| 11 | IOVDD | 3V3 |
| 12 | GPIO8 | GPIO8 |
| 13 | GPIO9 | GPIO9 |
| 14 | GPIO10 | GPIO10 |
| 15 | GPIO11 | GPIO11 |
| 16 | GPIO12 | GPIO12 |
| 17 | GPIO13 | GPIO13 |
| 18 | GPIO14 | GPIO14 |
| 19 | GPIO15 | GPIO15 |
| 20 | IOVDD | 3V3 |
| 21 | XIN | XIN |
| 22 | XOUT | XOUT |
| 23 | DVDD | 1V1 |
| 24 | SWCLK | SWCLK_MCU (unlabelled stub to R13) |
| 25 | SWDIO | SWDIO_MCU (unlabelled stub to R14) |
| 26 | RUN | RUN |
| 27 | GPIO16 | GPIO16 |
| 28 | GPIO17 | GPIO17 |
| 29 | GPIO18 | GPIO18 |
| 30 | IOVDD | 3V3 |
| 31 | GPIO19 | GPIO19 |
| 32 | GPIO20 | GPIO20 |
| 33 | GPIO21 | GPIO21 |
| 34 | GPIO22 | GPIO22 |
| 35 | GPIO23 | GPIO23 (headers + U4 input with 100 kΩ pull-down, `rgb-leds.md`) |
| 36 | GPIO24 | GPIO24 |
| 37 | GPIO25 | GPIO25 (headers + D1 through 1 kΩ, ≤ 1.9 mA, `user-io.md`) |
| 38 | IOVDD | 3V3 |
| 39 | DVDD | 1V1 |
| 40 | GPIO26_ADC0 | GPIO26 |
| 41 | GPIO27_ADC1 | GPIO27 |
| 42 | GPIO28_ADC2 | GPIO28 |
| 43 | GPIO29_ADC3 | GPIO29 |
| 44 | ADC_AVDD | 3V3 |
| 45 | IOVDD | 3V3 |
| 46 | VREG_AVDD | VREG_AVDD |
| 47 | VREG_PGND | GND (single-point join in layout) |
| 48 | VREG_LX | VREG_LX |
| 49 | VREG_VIN | 3V3 |
| 50 | VREG_FB | 1V1 |
| 51 | USB_DM | USB_DM_MCU |
| 52 | USB_DP | USB_DP_MCU |
| 53 | USB_OTP_VDD | 3V3 |
| 54 | QSPI_IOVDD | 3V3 |
| 55 | QSPI_SD3 | QSPI_SD3 |
| 56 | QSPI_SCLK | QSPI_SCLK |
| 57 | QSPI_SD0 | QSPI_SD0 |
| 58 | QSPI_SD2 | QSPI_SD2 |
| 59 | QSPI_SD1 | QSPI_SD1 |
| 60 | QSPI_SS | QSPI_SS |
| 61 (EP) | GND | GND |

All GPIO0–GPIO29 nets also go to the J2/J3 headers (`user-io.md`); SWCLK/SWDIO (connector side of R13/R14) to J4 pins 1/3; RUN to header pin 30 (J3.10).

**Decoupler → pin binding.** The stock symbol stacks IOVDD (1/11/20/30/38/45) and DVDD (6/23/39), so the schematic can't wire a capacitor to "its" pin. Each decoupler carries a `Decouples` field naming the pin it serves instead. The field travels to the PCB, and the placement checks read it to require each capacitor at its pin (positions in the Layout table).

| Ref | Net | `Decouples` | Value | LCSC | Footprint | REF part at that pin |
|---|---|---|---|---|---|---|
| C4 | 3V3 | U1.1 | 100 nF X7R 16 V | C1525 | `Capacitor_SMD:C_0402_1005Metric` | C13 |
| C5 | 3V3 | U1.11 | 100 nF X7R 16 V | C1525 | same | C15 |
| C6 | 3V3 | U1.20 | 100 nF X7R 16 V | C1525 | same | C18 |
| C7 | 3V3 | U1.30 | 100 nF X7R 16 V | C1525 | same | C16 |
| C8 | 3V3 | U1.38 | 100 nF X7R 16 V | C1525 | same | C14 |
| C9 | 3V3 | U1.45 | 100 nF X7R 16 V | C1525 | same | C17 |
| C10 | 3V3 | U1.44 | 100 nF X7R 16 V | C1525 | same | none (REF shares C17) |
| C11 | 3V3 | U1.53 (shared with 54) | 100 nF X7R 16 V | C1525 | same | C12 |
| C17 | 1V1 | U1.6 | 100 nF X7R 16 V | C1525 | same | C8 |
| C18 | 1V1 | U1.39 | 100 nF X7R 16 V | C1525 | same | C11 |
| C16 | 1V1 | U1.23 | 4.7 µF X5R 10 V | C23733 | same | C10 |
| C13 | VREG_AVDD | U1.46 | 4.7 µF X5R 10 V (CFILT) | C23733 | same | C9 |
| C14 | 3V3 | U1.49 | 4.7 µF X5R 10 V (CIN) | C23733 | REF `C_0402_1005Metric_small_pads` | C6 |
| C15 | 1V1 | **none set** (COUT) | 4.7 µF X5R 10 V (COUT) | C23733 | REF `C_0402_1005Metric_small_pads` | C7 |

C15 has no `Decouples` field in the netlist exported 2026-10-23. Its position is fixed by L1 and the copied regulator geometry. If the placement check treats a missing field as an error, give it `U1.50` (VREG_FB, the pin DS p.412 says is fed from COUT).

**Core regulator**

| Ref | Pin 1 | Pin 2 | Value | LCSC | Footprint |
|---|---|---|---|---|---|
| L1 | 1V1 (pad 1, dot) | VREG_LX (pad 2) | AOTA-B201610S3R3-101-T 3.3 µH | C42411119 | project copy of REF `L_pol_2016` |
| R3 | 3V3 | VREG_AVDD | 33 Ω 1 % | C25105 | `Resistor_SMD:R_0402_1005Metric` |

C13 (CFILT), C14 (CIN) and C15 (COUT) are in the table above.

**Crystal**

| Ref | Pins → nets | Value | LCSC | Footprint |
|---|---|---|---|---|
| Y1 | 1 → XIN; 3 → XOUT_R; 2 → GND; 4 → GND | ABM8-272-T3 12 MHz | C20625731 | `Crystal:Crystal_SMD_3225-4Pin_3.2x2.5mm` |
| R6 | 2 → XOUT; 1 → XOUT_R | 1 kΩ 1 % | C11702 | `Resistor_SMD:R_0402_1005Metric` |
| C20 | 1 → XIN; 2 → GND | 15 pF C0G 50 V | C1548 | `Capacitor_SMD:C_0402_1005Metric` |
| C21 | 1 → XOUT_R; 2 → GND | 15 pF C0G 50 V | C1548 | `Capacitor_SMD:C_0402_1005Metric` |

**BOOTSEL and RUN** (the switches are audited in `user-io.md`)

| Ref | Pin 1 | Pin 2 | Value | LCSC | Footprint |
|---|---|---|---|---|---|
| R7 | QSPI_SS | BOOTSEL_SW (unlabelled, `Net-(R7-Pad2)`) | 1 kΩ 1 % | C11702 | `Resistor_SMD:R_0402_1005Metric` |
| SW1 BOOTSEL | BOOTSEL_SW | GND | — | `user-io.md` | `user-io.md` |
| R8 | RUN | RESET_SW (unlabelled, `Net-(R8-Pad2)`) | 1 kΩ 1 % | C11702 | `Resistor_SMD:R_0402_1005Metric` |
| SW2 RESET | RESET_SW | GND | — | `user-io.md` | `user-io.md` |

No external pull-up or capacitor on RUN (internal pull-up, as REF). The QSPI_SS pull-up footprint, if any, is `flash.md`'s.

**SWD series resistors**

| Ref | Pin 1 | Pin 2 | Value | LCSC | Footprint |
|---|---|---|---|---|---|
| R13 | SWCLK (J4.1) | SWCLK_MCU (U1.24; unlabelled, `Net-(U1-SWCLK)`) | 100 Ω 1 % | C25076 | `Resistor_SMD:R_0402_1005Metric` |
| R14 | SWDIO (J4.3) | SWDIO_MCU (U1.25; unlabelled, `Net-(U1-SWDIO)`) | 100 Ω 1 % | C25076 | `Resistor_SMD:R_0402_1005Metric` |

**USB series resistors (placement only here; value and part in `usb-front-end.md`)**

| Ref | Pin 1 (connector side) | Pin 2 (U1 side) | Value | LCSC | Footprint |
|---|---|---|---|---|---|
| R5 | USB_DP | USB_DP_MCU (U1.52) | 27 Ω | C25100 | `Resistor_SMD:R_0402_1005Metric` |
| R4 | USB_DM | USB_DM_MCU (U1.51) | 27 Ω | C25100 | `Resistor_SMD:R_0402_1005Metric` |

**Test point:** TP3 on 1V1 (`TestPoint_Pad_D1.5mm`), attached at the pin-23 end (Layout).

Block totals: 1 × RP2350A, 1 × L, 1 × crystal; 10 × 100 nF (C1525), 4 × 4.7 µF (C23733), 2 × 15 pF (C1548), 3 × 1 kΩ (C11702), 1 × 33 Ω (C25105), 2 × 100 Ω (C25076); plus the 2 × 27 Ω counted in `usb-front-end.md`.
