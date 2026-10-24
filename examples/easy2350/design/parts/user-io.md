# User I/O — SW1/SW2 buttons, D1 status LED, J2/J3 GPIO headers, J4 SWD, TP1–TP4

> **Superseded at layout (decisions.md, 2026-10-23):** the buttons are TS-1088-AR02016 (not TS-1187A); only TP3 (1V1) exists as a pad, VBUS/3V3/GND are header pins 40/36/38 (TP1, TP2, TP4 dropped); the bottom silk does not carry the "not Pico pin-compatible" line (no via-free strip left; README and intent.md say it). The tables below keep the audit as it was done.

Page numbers: PDF page for the small datasheets. For rp2350-datasheet.pdf, the
number printed in the page footer, which is the PDF page − 1. The resistors in
series with SW1/SW2 (BOOTSEL to QSPI_SS, RUN) are audited in `rp2350-core.md`.
This file gives only the switch ratings.

---

## SW1 BOOTSEL, SW2 RESET — XUNPU TS-1088-AR02016 tactile switch

Datasheet: datasheets/ts-1088.pdf (XunPu manufacture drawing TS-1088-ARXXXXX, rev A, 1 sheet). Read: all.
MPN: TS-1088-AR02016   Supplier: C720477 TS-1088-AR02016 (XUNPU, SMD,4x3mm): basic, JLC stock 530961, lifecycle normal, checked 2026-10-23
Suffix decode (drawing's part-number key):
- **A** = stainless-steel spring
- **R** = no locating post, so no board holes are needed
- **020** = H 2.0 mm
- **16** = 160 gf
Footprint: `Button_Switch_SMD:SW_SPST_TS-1088-xR020` (the KiCad description cites this LCSC datasheet). **Verified pad by pad against "PCB welding drawing":**

| Item | Drawing | KiCad | Match |
|---|---|---|---|
| Pad width (x) | (5.50 − 3.40)/2 = 1.05 mm | 1.05 mm | yes |
| Pad centre x | ±(5.50 + 3.40)/4 = ±2.225 mm | ±2.225 mm | yes |
| Pad height (y) | 2.00 mm | 2.0 mm | yes |
| Pad centre y | 0 (pads on the body centreline) | 0 | yes |
| Pin count / numbering | 2 terminals, ① left and ② right in the top view; circuit diagram ①—switch—② | pad "1" at −2.225, pad "2" at +2.225 | yes |
| Internally connected pins | **None.** Just two terminals; it is not a 4-pin part with joined pairs | 2 pads | yes |
| Locating post | R = none | no NPTH | yes |

Body 3.90 × 3.00 mm, 5.00 mm across the terminals, actuator Ø1.80 mm.
Symbol: `Switch:SW_Push` (2 pins) maps 1:1 to the pads.

### Ratings vs use
| Parameter | Drawing | Our use | OK |
|---|---|---|---|
| Rating | 50 mA, 12 V DC | 3.3 V. Current is at most 3.3 V / 1 kΩ ≈ 3.3 mA through the series resistor (rp2350-core.md), only while pressed | yes, 15× |
| Contact resistance | ≤ 100 mΩ | In series with 1 kΩ | yes |
| Insulation resistance | ≥ 100 MΩ | — | yes |
| Dielectric strength | ≥ 250 V AC / 1 min | 3.3 V | yes |
| Operating force | 160 gf ± 50 gf (code 16; table gives the ±50 gf tolerance) | Finger press | yes |
| Travel | 0.2 ± 0.1 mm | — | — |
| Height | H = 2.0 mm (code 020) | Limit 3.5 mm (buttons exempt anyway) | yes |
| Operating temperature | Not stated on the drawing | 0–40 °C | assumed OK (desk use) |

### Fine print / gotchas
- The drawing gives no mechanical life, bounce time, temperature range or reflow profile. None matters for BOOTSEL/RESET: RUN is level-sensitive and BOOTSEL is sampled at boot. Reflow follows JLC's standard profile (assumption: it is a basic part they assemble routinely).
- Two terminals only, so the schematic symbol and footprint are both 2-pin. There is no second pad pair to tie together.
- Finger ESD reaches QSPI_SS and RUN through these switches unprotected. intent.md accepts this ("don't care").
- Silk: "BOOT" and "RST" labels next to each.

### Rejected: XKB TS-1187A-B-A-B (C318884)
Rejected for space at phase-4 placement. The lead found its 7.6 × 5.7 mm courtyard doesn't fit in the 14.2 mm between the header rows alongside the RGB pixels: 5.1 × 5.1 mm body, 6.5 mm terminal span, four pads in two internally joined pairs. Electrically it was fine: 50 mA / 12 V, 160 gf, 1.5 mm tall (datasheets/ts-1187a.pdf, kept for the record). The TS-1088 is basic too, so there is no cost change.

---

## D1 — KT-0603R red status LED on GPIO25

Datasheet: datasheets/kt-0603r.pdf (Hubei KENTO, 0603-0.6 red, rev A.0, 2018-12-06, 11 pp.). Read: all (pp. 7–11 are handling cautions).
MPN: KT-0603R   Supplier: C2286 KT-0603R (Hubei KENTO Elec, 0603): basic, JLC stock 7919309, lifecycle normal, checked 2026-10-23
Footprint: `LED_SMD:LED_0603_1608Metric`, pads 0.875 × 0.95 mm at ±0.7875 mm (gap 0.7 mm). The datasheet suggests 0.70 × 0.70 mm pads with a 0.70 mm gap (p.2). Same gap, KiCad's pads slightly larger, fine for a 1.6 × 0.8 × 0.6 mm body.
**Polarity:** p.2 marks pin ① "+" (anode). The green corner mark is at the pin ② end, which is the cathode. KiCad `Device:LED` and the footprint use pad 1 = K, pad 2 = A. So **KiCad pad 1 ↔ LED pin ② (marked end)**. Check the JLC CPL rotation (Open question 3).

### Absolute max vs worst case (p.3)
| Parameter | Rating | Worst case | OK |
|---|---|---|---|
| IF DC | 25 mA | 1.9 mA | yes |
| Pd | 40 mW | 1.9 mA × 2.23 V = 4.2 mW | yes |
| VR | 5 V | never reverse-biased (GPIO 0–3.3 V, LED to GND) | yes |
| Topr | −40 to +85 °C | 0–40 °C | yes |
| Derating | 25 mA flat to 25 °C, falling to ≈ 22 mA at 40 °C (p.5 curve) | 1.9 mA | yes |

### Calculations — series resistor R_LED1

**Inputs.**
- IOVDD = 3.3 V ± 3 % = 3.20–3.40 V (intent.md).
- RP2350 GPIO VOH min = 2.62 V at IOVDD = 3.3 V with IOH = the selected drive strength; default drive is 4 mA (rp2350-datasheet p.1336, Table 1683; PADS_BANK0 DRIVE reset = 1 → 4 mA, p.717).
- The pin model I use is a straight line from IOVDD at 0 mA to 2.62 V at 4 mA: source resistance ≤ (3.20 − 2.62)/4 mA = 145 Ω worst case (assumption: linear interpolation of the one guaranteed point; Fig. 154, p.1340, shows typical parts far stiffer).
- LED Vf: only specified at 20 mA, 1.8–2.4 V (p.3; bins VE–VJ, p.4). The typical IV curve (p.5, 25 °C) reads 1.86 V at 1 mA, 1.87 V at 1.5 mA, 1.93 V at 4 mA, 2.09 V at 20 mA. Shifting the curve by the 20 mA bin spread (−0.29 / +0.31 V) gives 1.58–2.18 V at 1–2 mA (assumption: bin offset is the same at low current). Temperature: no data. Assume −2 mV/°C (assumption), +0.05 V at 0 °C, −0.03 V at 40 °C. **Vf = 1.55–2.23 V.**

**I = (V_pin − Vf) / R:**

| R (basic 0402 1 %) | I max (IOVDD 3.40, no sag, Vf 1.55, R −1 %) | I typ (3.30, Vf 1.87) | I min (IOVDD 3.20, 145 Ω sag, Vf 2.23, R +1 %) |
|---|---|---|---|
| 100 Ω (C25076) | 18.7 mA | 14.3 mA | 3.8 mA |
| 470 Ω (C25117) | 3.98 mA | 3.0 mA | 1.56 mA |
| **1 kΩ (C11702)** | **1.87 mA** | **1.43 mA** | **0.84 mA** |

**Chosen: 1 kΩ.** It is the only candidate that stays within intent's 2 mA LED budget at every corner. 470 Ω runs at 3 mA typical, up to 4 mA.
Brightness: relative intensity is ≈ linear in IF, 0.5 at 10 mA and 1.0 at 20 mA (p.5). At 1.43 mA that is ≈ 0.07 × (145–300 mcd) = 10–21 mcd, and ≥ 6 mcd at the 0.84 mA corner. Plenty for a status LED viewed from 0.5 m indoors (assumption: an indicator needs a few mcd).
Resistor power: 1.87² mA² × 1 kΩ = 3.5 mW.
GPIO25 sources ≤ 1.9 mA, inside the 4 mA drive setting and the 50 mA aggregate (intent) / 100 mA IIOVDD_MAX (p.1337).

### Fine print
- GPIO25 is also header pin 33. A user's load on it adds to the LED's 1.9 mA.
- MSL/handling: "use a series resistor" (p.11, 11.6.5). Done. Static-sensitive (p.10). Reflow 260 °C/10 s max (p.2).

---

## J2, J3 — GPIO headers, 2 × 1×20 0.1", not assembled

Footprint: `Connector_PinHeader_2.54mm:PinHeader_1x20_P2.54mm_Vertical` (pads 1.7 × 1.7 mm, drill 1.0 mm). Symbol: `Connector_Generic:Conn_01x20`. **Exclude from BOM and position files** (DNP; user-soldered). No MPN; generic 0.1" header. No datasheet is needed for a footprint-only part. Assume a 1 A per pin rating for any generic 0.1" header the user fits (assumption); the highest load here is 100 mA on 3V3.

Geometry:
- Rows 17.78 mm apart on a 21 mm board: row centre 1.61 mm from each long edge, pad edge 0.76 mm from the board edge.
- Pins 1 to 20 span 48.26 mm on a 51 mm board, leaving 1.37 mm to each short edge from the end pin centres (0.52 mm to the pad edge).
- Both edge clearances are tight but legal at JLC (≥ 0.3 mm copper-to-edge, assumption). Phase 4 checks them in DRC.

### Pin map

Board pins are numbered like a DIP, viewed from the top with USB-C at the top: pins 1–20 run down the left edge (J2), and pins 21–40 run up the right edge (J3). So **J3 pin n = board pin 20 + n**, and J3 pin 1 is at the SWD end.

Inputs to the map:
- **RP2350A QFN-60 pin order** (rp2350-datasheet §1.2.1.1, Fig. 2, p.15). Going counter-clockwise from the top-left:
  - Left edge, top→bottom: IOVDD 1, GPIO0–3 (2–5), DVDD 6, GPIO4–7 (7–10), IOVDD 11, GPIO8–11 (12–15).
  - Bottom edge, left→right: GPIO12–15 (16–19), IOVDD 20, XIN 21, XOUT 22, DVDD 23, SWCLK 24, SWDIO 25, RUN 26, GPIO16–18 (27–29), IOVDD 30.
  - Right edge, bottom→top: GPIO19–25 (31–37), IOVDD 38, DVDD 39, GPIO26–29 (40–43), ADC_AVDD 44, IOVDD 45.
  - Top edge: USB_DM 51, USB_DP 52, QSPI 53–60, VREG 46–50.
- With the chip's USB/QSPI edge facing the USB-C end and its left edge facing J2, GPIO0–15 fall in order down J2 and GPIO16–29 in order up J3. Every GPIO trace runs straight out to its own header pin with no crossings.
- **Power and GND are put at the Raspberry Pi Pico's positions** wherever the counts allow. This costs nothing in routing, and it means 33 of 40 pins (GPIO0–22, RUN, 3V3, VBUS and all 7 GND) sit where a Pico pinout card says they are.

| Board pin | Header pin | Signal | Pico (for reference) | | Board pin | Header pin | Signal | Pico (for reference) |
|---|---|---|---|---|---|---|---|---|
| 1 | J2.1 | GPIO0 | GP0 | | 40 | J3.20 | **VBUS** | VBUS |
| 2 | J2.2 | GPIO1 | GP1 | | 39 | J3.19 | GPIO29 (ADC3) | VSYS ⚠ |
| 3 | J2.3 | **GND** | GND | | 38 | J3.18 | **GND** | GND |
| 4 | J2.4 | GPIO2 | GP2 | | 37 | J3.17 | GPIO28 (ADC2) | 3V3_EN ⚠ |
| 5 | J2.5 | GPIO3 | GP3 | | 36 | J3.16 | **3V3** | 3V3(OUT) |
| 6 | J2.6 | GPIO4 | GP4 | | 35 | J3.15 | GPIO27 (ADC1) | ADC_VREF ⚠ |
| 7 | J2.7 | GPIO5 | GP5 | | 34 | J3.14 | GPIO26 (ADC0) | GP28 ⚠ |
| 8 | J2.8 | **GND** | GND | | 33 | J3.13 | GPIO25 (+ D1 LED) | AGND ⚠ |
| 9 | J2.9 | GPIO6 | GP6 | | 32 | J3.12 | GPIO24 | GP27 ⚠ |
| 10 | J2.10 | GPIO7 | GP7 | | 31 | J3.11 | GPIO23 (+ RGB DIN) | GP26 ⚠ |
| 11 | J2.11 | GPIO8 | GP8 | | 30 | J3.10 | RUN | RUN |
| 12 | J2.12 | GPIO9 | GP9 | | 29 | J3.9 | GPIO22 | GP22 |
| 13 | J2.13 | **GND** | GND | | 28 | J3.8 | **GND** | GND |
| 14 | J2.14 | GPIO10 | GP10 | | 27 | J3.7 | GPIO21 | GP21 |
| 15 | J2.15 | GPIO11 | GP11 | | 26 | J3.6 | GPIO20 | GP20 |
| 16 | J2.16 | GPIO12 | GP12 | | 25 | J3.5 | GPIO19 | GP19 |
| 17 | J2.17 | GPIO13 | GP13 | | 24 | J3.4 | GPIO18 | GP18 |
| 18 | J2.18 | **GND** | GND | | 23 | J3.3 | **GND** | GND |
| 19 | J2.19 | GPIO14 | GP14 | | 22 | J3.2 | GPIO17 | GP17 |
| 20 | J2.20 | GPIO15 | GP15 | | 21 | J3.1 | GPIO16 | GP16 |

(USB-C end at the top row, SWD end at the bottom row.) Count: 30 GPIO, RUN, 3V3, VBUS, 7 GND = 40.

Notes on the map:
- **GND spacing:** every signal is within 5 pins of a GND on its own row. The worst stretch is board pins 29–37 (between GND 28 and GND 38). A layout with GND evenly every 6 pins was considered: it shortens that stretch to 5 pins, but moves GND and 3V3 away from the Pico positions. That turns a Pico accessory's ground into a short on our 3V3 (Pico GND 38 ↔ our 3V3). Rejected.
- **RUN at pin 30** is the one signal out of package order. On the chip RUN (26) sits between SWDIO and GPIO16, on the far edge, so its trace crosses GPIO16–22 to reach the header. It is a slow, DC signal and can take an inner layer.
- **⚠ Pico differences** are pins 31–35, 37 and 39. Two of them can hurt on a Pico carrier board:
  - Pin 39 is Pico VSYS; carriers may feed 5 V there. Here it is GPIO29, which is an ADC pin, not 5 V tolerant: abs max IOVDD + 0.5 V (p.1335).
  - Pin 33 is Pico AGND, which would short GPIO25 (the LED pin) to ground when driven high.
  Silk must say **"NOT Pico pin-compatible: pins 31–35, 37, 39 differ"**. intent.md already says the board is not compatible.
- **Pin 39 GPIO29 sits next to pin 40 VBUS.** A solder bridge or a slipped probe puts 5 V on a non-FT pin. Same exposure as any header GPIO (intent: "5 V on a GPIO … not protected: don't care").
- Power at the USB end (36 3V3, 38 GND, 40 VBUS), so VBUS from J1 and 3V3 from the LDO are short runs. Silk on the power pins: "3V3 OUT 100 mA MAX", "VBUS 50 mA MAX — do not power the board here with USB plugged in" (intent).

---

## J4 — SWD header, 1×3 0.1", not assembled

Reference: datasheets/rpi-debug-connector.pdf (Raspberry Pi "3-pin Debug Connector Specification", RP-003139-SP-4, 4 pp.). Read: all.
Footprint: `Connector_PinHeader_2.54mm:PinHeader_1x03_P2.54mm_Vertical`. Symbol: `Connector_Generic:Conn_01x03`. Exclude from BOM and position files.
**Pin order: 1 SWCLK, 2 GND, 3 SWDIO.** This matches the spec's pin numbering, pin 1 SC (serial clock), 2 GND, 3 SD (p.1). It is also the Pico's 3-pin SWD order. The Raspberry Pi Debug Probe's 0.1" jumper cable carries these three signals. The wire colours are not in this spec (Open question 4); label the silk "SWCLK GND SWDIO".
- The spec says **100 Ω series resistors at the target on SC and SD, placed very close to the IC pins**, "not strictly required but highly recommended" (p.2) for slew limiting, short-circuit and ESD current limiting. **Adopted in rp2350-core.md:** R_SWCLK and R_SWDIO, 100 Ω 0402 (C25076, basic), at U1 pins 24/25. Nets: U1.24 SWCLK_MCU → R_SWCLK → **SWCLK** → J4.1, and U1.25 SWDIO_MCU → R_SWDIO → **SWDIO** → J4.3.
- Voltage: host at 3.3 V ± 10 %, target pins must be 3.3 V failsafe (p.2). The RP2350 SWD pins at IOVDD 3.3 V are fine; their failsafe behaviour when the board is unpowered is in the core audit.
- Placement: the SWD short edge (intent), pin 1 marked.

---

## TP1–TP4 — test points VBUS, 3V3, 1V1, GND

Footprint: **`TestPoint:TestPoint_Pad_D1.5mm`**: a Ø1.5 mm SMD circle on F.Cu + F.Mask only, no paste, footprint attribute `exclude_from_pos_files exclude_from_bom` (checked in the KiCad library file). Symbol: `Connector:TestPoint`. The library symbol has in_bom = yes, so **tick "Exclude from BOM" on each TP symbol** so the BOM and footprint agree.
Why 1.5 mm:
- A passive scope probe tip (≈ 0.5–1 mm, assumption) or a DMM probe needs a target ≥ 1 mm to land on without slipping onto neighbours.
- 2.0 mm and larger cost space on a 21 mm board for no gain.
- 1.0 mm is easy to miss by hand.
- Bare pads (no part) also cost nothing at JLC.
- No hook-clip capture is possible on any SMD pad. Use header GND pins (once fitted) for clip leads.

Placement:
- **TP_GND within ≈ 5 mm of TP_1V1**, so a probe's ground spring can span them for an SMPS ripple measurement (assumption: typical ground-spring reach).
- TP_1V1 at the core regulator's output cap, not out on a stub. 1V1 "never leaves the chip's corner" (architecture.md).
- TP_3V3 at the LDO output cap. TP_VBUS after the ESD/fuse point, near J1.
- Silk label at each: "VBUS", "3V3", "1V1", "GND".

---

## Schematic connections

| Ref (role) | Pin | Net |
|---|---|---|
| SW1 BOOTSEL | 1 / 2 | GND / BOOTSEL side (net and its series resistor per rp2350-core.md, to QSPI_SS) |
| SW2 RESET | 1 / 2 | GND / RUN (series resistor per rp2350-core.md) |
| D1 KT-0603R | 2 (A) | LED1_A |
| | 1 (K) | GND |
| J2 pin n (n = 1–20) | — | board pin n per the map (GPIO0…GPIO15, GND at 3, 8, 13, 18) |
| J3 pin n (n = 1–20) | — | board pin 20 + n per the map (GPIO16…GPIO29, RUN at J3.10, 3V3 at J3.16, VBUS at J3.20, GND at J3.3, J3.8, J3.18) |
| J4 | 1 / 2 / 3 | SWCLK / GND / SWDIO (connector side of R_SWCLK / R_SWDIO; the MCU side is SWCLK_MCU / SWDIO_MCU, see rp2350-core.md) |
| TP1 | 1 | VBUS |
| TP2 | 1 | 3V3 |
| TP3 | 1 | 1V1 |
| TP4 | 1 | GND |

Note: GPIO23 also goes to U4.A and R_DIN_PD (rgb-leds.md). GPIO25 also goes to R_LED1.

| Function name | Between | Value | Rating | LCSC | Footprint |
|---|---|---|---|---|---|
| R_LED1 | GPIO25 – LED1_A | 1 kΩ 1 % | 1/16 W (3.5 mW used) | C11702 (basic) | Resistor_SMD:R_0402_1005Metric |

Parts on the BOM from this block: SW1, SW2 (C720477), D1 (C2286), R_LED1. All basic. (The SWD resistors are on rp2350-core.md's BOM.) J2, J3, J4 and TP1–TP4 are excluded from the BOM.

## Layout guidance
- SW1/SW2 at a long-edge position reachable with a finger, clear of the RGB pixels (don't press LED lenses, ws2812b-2020-v6 p.10). 3.9 × 3.0 mm body, 5.0 mm across the terminals, pads 1.05 × 2.0 mm at ±2.225 mm. Keep 0.5 mm courtyard clearance. Their series resistors sit at the RP2350 end, not the switch end (core audit).
- D1 visible from the top, near its GPIO25 header pin or the chip. R_LED1 next to D1.
- Header pads are THT: keep vias and SMD pads ≥ 0.3 mm from their 1.7 mm pads. The bottom is part-free (intent), so the header pins' bottom-side annular rings are fine.
- Fan the GPIOs straight out in package order: J2 from the chip's left and bottom-left, J3 from bottom-right and right.
- J4 at the short edge opposite USB-C. Route SWCLK/SWDIO short from U1 pins 24/25.
- TPs: see placement above.

## Open questions
1. ~~Button footprint vs board width~~: resolved. The TS-1187A didn't fit; changed to the TS-1088-AR02016 (3.9 × 3.0 mm).
2. **Pico VSYS/AGND conflict** (pins 39, 33): accepted with silk warning. User sign-off at gate 2, since intent says only "not pin-compatible".
3. **D1 orientation in JLC CPL**: datasheet pin ① = anode (p.2), opposite to KiCad pad 1 = cathode. Check the JLC placement preview.
4. **Debug Probe cable colours** for the 0.1" adapter cable: not in the connector spec. Confirm against the Debug Probe product brief before writing silk or README text that names colours.
5. ~~SWD series resistors~~: resolved. Adopted in rp2350-core.md (R_SWCLK, R_SWDIO 100 Ω).
