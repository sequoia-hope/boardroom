# J1, U2 — USB-C front end (receptacle, CC pull-downs, ESD array, 27 Ω series pair)

Block 1 of `architecture.md`: J1 USB-C receptacle (nets USB_DP_C / USB_DM_C) → U2 USBLC6-2SC6
in-line on D+/D− with its VBUS pin on VBUS and a 100 nF at that pin → the pair (USB_DP / USB_DM)
runs to U1 → 27 Ω series resistors at U1 (nets USB_DP_MCU / USB_DM_MCU) → U1 pins 52/51. Net
names follow `nets.csv`. 5.1 kΩ Rd from CC1 and CC2 to GND. VBUS goes on
to U3 (LDO, `ldo-3v3.md`), D2 (LED rail, `rgb-leds.md`) and the header VBUS pin.

| Part | Function | Datasheet | Supplier | Footprint |
|---|---|---|---|---|
| J1 | HRO TYPE-C-31-M-12, 16-pin USB 2.0 Type-C receptacle, SMT + 4 through-hole shell legs | datasheets/type-c-31-m-12.pdf (HRO drawing, 1 sheet, rev A, drawn 2020-12-08). Read: all of it (rendered at 400 dpi; the PDF has no extractable text) | C165948 TYPE-C-31-M-12 (Korean Hroparts Elec, SMD): extended, JLC stock 327344, lifecycle normal, checked 2026-10-23 | `Connector_USB:USB_C_Receptacle_HRO_TYPE-C-31-M-12` — pin map matches; land pattern differs in detail from this drawing (see Footprint check) |
| U2 | ST USBLC6-2SC6, 2-line ESD array + VBUS TVS, SOT23-6L | datasheets/usblc6-2.pdf (Doc ID 11265 Rev 5, Oct 2011). Read: all 14 pages | C7519 USBLC6-2SC6 (ST, SOT-23-6L): extended, JLC stock 35557, lifecycle normal, checked 2026-10-23 | `Package_TO_SOT_SMD:SOT-23-6` — matches suffix "SC6" = SOT23-6L (p.10) |
| C_ESD_VBUS | 100 nF 16 V X7R ±10 % 0402, Samsung CL05B104KO5NNNC | samsung-mlcc.pdf (part numbering p.4) | C1525: basic, JLC stock 35873924, lifecycle normal, checked 2026-10-23 | `Capacitor_SMD:C_0402_1005Metric` |
| R_CC1, R_CC2 | 5.1 kΩ ±1 % 62.5 mW 50 V 0402, Uni-Royal 0402WGF5101TCE | JLC description | C25905: basic, JLC stock 4259185, lifecycle normal, checked 2026-10-23 | `Resistor_SMD:R_0402_1005Metric` |
| R_USB_DP, R_USB_DM | 27 Ω ±1 % 62.5 mW 50 V 0402, Uni-Royal 0402WGF270JTCE | JLC description ("27Ω ... ±1%") | C25100: **extended**, JLC stock 56936, lifecycle normal, checked 2026-10-23 | `Resistor_SMD:R_0402_1005Metric` |

Also read for this audit: `hardware-design-with-rp2350.pdf` (Release 3, build 2026-08-20) §5.1 USB
(PDF p.16) and the Appendix A schematic (PDF p.21); `rp2350-datasheet.pdf` (Release 8, build
2026-10-01) Table 1678 USB pins (PDF p.1335), Table 1680 absolute maximum ratings (p.1336),
Table 1681 ESD (p.1336–1337), Table 1684 USB IO characteristics (p.1338). Page numbers in this file
are PDF page numbers; the Raspberry Pi documents print a number one lower.

Operating conditions (intent.md): VBUS 4.5–5.5 V at J1, above 6 V don't care; 500 mA total;
USB 2.0 full speed device, 12 Mb/s; IEC 61000-4-2 ±8 kV contact on every USB-C pin; 0–40 °C;
plugged in many times by people.

**USB and Type-C specification numbers in this file are from memory** (the specs are not in
`datasheets/`). Each is marked "(spec, from memory)". None of them changes a part choice; they are
the yardsticks.

## Footprint check against the HRO drawing

Measured from the drawing's "RECOMMEND P.C.B LAYOUT (COMPONENT SIDE)" view (tolerance ±0.05 mm)
and compared with the KiCad 9 footprint, both in the footprint's coordinates (origin at the
centre, +y towards the mouth; the two Ø0.6 locating holes at y = −2.60):

| Feature | Drawing | KiCad footprint | Verdict |
|---|---|---|---|
| Pin order across the row (left → right, top view) | A1/B12, A4/B9, B8, A5, B7, A6, A7, B6, A8, B5, B4/A9, B1/A12 | A1+B12 −3.25, A4+B9 −2.45, B8 −1.75, A5 −1.25, B7 −0.75, A6 −0.25, A7 +0.25, B6 +0.75, A8 +1.25, B5 +1.75, A9+B4 +2.45, A12+B1 +3.25 | Same order. OK |
| Signal pad width / pitch | 8 × 0.30, 0.50 pitch; A5–A8 2.50, B8–B5 3.50 apart | 0.30 wide, 0.50 pitch, same spans | OK |
| Power/GND pad width and centres | 4 × 0.60; VBUS pads 4.80 apart (±2.40), GND pads 6.40 apart (±3.20) | 0.60 wide at ±2.45 and ±3.25 | 0.05 mm further out in KiCad; at the edge of the drawing's ±0.05 tolerance. Accept |
| Pad length / position | not dimensioned directly: 1.64 from pad rear end to the locating-hole line and 0.50 from pad front end to it → pads y = −4.24 … −3.10 (≈ 1.14 long) | 1.45 long, y = −4.77 … −3.32 | KiCad pads extend 0.53 mm further back and stop 0.22 mm short at the front. **Mismatch** (Findings) |
| Locating holes (NPTH) | 2 × Ø0.60, 5.78 apart; the part's pegs are Ø0.50 | Ø0.65, ±2.89 | OK (0.15 mm clearance on a Ø0.5 peg) |
| Rear shell legs (plated slots) | pad 0.90 × 2.00, drill 0.60 × 1.70, centred 0.50 behind the locating-hole line (y ≈ −3.10); 8.65 apart | pad 1.0 × 2.1, drill 0.6 × 1.7 at (±4.32, −3.13) | OK |
| Front shell legs (plated slots) | pad 0.90 × 1.70, drill 0.60 × 1.40, 4.18 in front of the rear legs | pad 1.0 × 1.6, drill 0.6 × 1.2 at (±4.32, +1.05) | Drill 0.2 mm shorter than drawn. The front legs are 0.80 wide (side view) → 0.4 mm clearance in a 1.2 slot. Accept |
| Board edge | 5.79 from the locating-hole line → y = +3.19 | fab outline to y = +3.65 (body 7.35 long, drawing top view) | The mouth overhangs the board edge by ≈ 0.46 mm when the edge is where HRO draws it. Phase 4 puts the edge at y = +3.19 in footprint coordinates |
| Leg length below the seating plane | 1.01 (side view, "4-1.01") | — | Legs do not pass through a 1.6 mm board; plated slots fill with solder. OK |

The KiCad footprint's description cites an older HRO drawing (2018); this one is rev A of
2020-12-08. Every contact lands on copper in both, and the footprint is widely used with C165948,
but the front 0.22 mm of each SMT foot (if the foot reaches the drawn pad's front end) sits off the
KiCad pad. Recommendation: a project-local copy in `hw/easy2350/a/lib/easy2350.pretty/` with the
SMT pads redrawn to the drawing (y −4.45 … −3.10: the drawing's pad plus 0.2 mm of rear toe for an
inspectable fillet), everything else as KiCad. Phase 3 library work; flagged MINOR below.

## What the drawing says about J1

- Pin map (drawing table): A1, A12, B1, B12 GND; A4, A9, B4, B9 VBUS; A5 CC1; B5 CC2; A6 DP1;
  A7 DN1; B6 DP2; B7 DN2; A8 SBU1; B8 SBU2. Matches the KiCad symbol
  `Connector:USB_C_Receptacle_USB2.0_16P` (S1 = shield) pin for pin.
- Ratings (drawing note 4): **5 A, 20 V**; insulation 100 MΩ min; dielectric withstand 100 V AC
  for 1 min. Operating temperature **−30 to +80 °C** (note 5). Durability 10 000 cycles (note
  3-3); insertion 0.5–2.0 kgf, removal 0.8–2.0 kgf.
- Materials: contacts copper alloy, Au on the mating area, gold flash on the solder area; shell
  and mid-plate stainless steel (note 1, 2). Reflow: 260 °C peak for 10 s (note 6).
- No ESD rating is given for the receptacle. U2 protects VBUS, D+ and D−; the shell takes the
  strike to GND. **CC1/CC2 have no ESD device**, only R_CC1/R_CC2 to GND (found at the release
  review; waived against intent's "every pin" in decisions.md, 2026-10-23).

## Absolute max vs our worst case

| Parameter | Rating | Our worst case | Margin |
|---|---|---|---|
| J1 current (drawing note 4-1) | 5 A | 0.5 A total (intent), shared by 4 VBUS and 4 GND contacts | 10× |
| J1 voltage (note 4-1) | 20 V | 5.5 V; >6 V don't care | OK |
| J1 temperature (note 5) | −30 … +80 °C | 0–40 °C ambient; board-level heating from U3 and the LEDs (ldo-3v3.md) keeps the connector region well under 80 °C unless U3 sits next to it | OK; keep U3 off the connector |
| U2 Tj (Table 1, p.2) | −40 … +125 °C | ≤ 60 °C | OK |
| U2 peak pulse (Table 1, p.2) | IEC 61000-4-2 15 kV contact and 15 kV air; MIL-STD-883G 25 kV | intent: ±8 kV contact (level 4) | OK; the front page says "level 4 compliance guaranteed at device level" (p.1) |
| U2 VBUS (pin 5) working voltage | VRM 5.25 V (IRM 150 nA max, 25 °C, p.2); VBR ≥ 6 V at 1 mA (p.2). No higher DC rating is given | 5.5 V (vSafe5V upper limit, spec, from memory) | Above VRM by 0.25 V: leakage unspecified at 5.5 V but bounded below 1 mA by VBR ≥ 6 V at 1 mA. Harmless against a 500 mA budget. Above 6 V the TVS conducts hard; intent: don't care |
| U2 I/O pins | rail-to-rail to VBUS and GND | 0–3.3 V signals, VBUS ≥ 4.5 V present whenever USB is | Steering diodes stay reverse-biased. OK |
| RP2350 USB_DP/USB_DM (Table 1678, p.1335: type "USB IO", USB_OTP_VDD domain) | Not FT pads. Table 1680 (p.1336) gives −0.5 … IOVDD + 0.5 V for standard pads; reading that row with USB_OTP_VDD = 3.3 V gives 3.8 V | 3.3 V signalling; ESD residual (Calculations) | OK for signals. Note: the bluebuzz audit used the FT 5.5 V row for these pins; the datasheet does not type them FT |
| RP2350 ESD (Table 1681, p.1336) | HBM 2 kV all pins (4 kV for FT pins only), CDM 500 V | IEC ±8 kV contact at J1 | Not alone: U2 + 27 Ω required, present |
| R_CC1/R_CC2 (0402, 62.5 mW, 50 V) | 62.5 mW | Default-Rp source: 0.42 V across → 35 µW; a non-compliant 5.5 V straight onto CC: 5.5²/5.1 k = 5.9 mW | 10× |
| R_USB_DP/DM (62.5 mW) | 62.5 mW | FS signalling, ≈ 3.3 V into ≈ 45 Ω + 15 kΩ host pull-down; mW-class average | OK |
| C_ESD_VBUS 16 V X7R | 16 V | 5.5 V; hot-plug ring to ≈ 7 V (ldo-3v3.md) | 2.3× at 7 V. OK |

## Required external parts (from datasheets)

- **27 Ω series resistor on each of USB_DP and USB_DM, placed close to the RP2350** — RP2350
  datasheet Table 1678 ("27Ω series resistor required for USB operation", p.1335); hardware guide
  §5.1 ("placed close to the chip, in order to meet the USB impedance specification", p.16) →
  R_USB_DP, R_USB_DM 27 Ω 1 %.
- **No external D+ pull-up and no pull-downs** — the RP2350 has them inside (guide p.16; RPU2
  0.873–1.548 kΩ, RPU1&2 1.398–3.063 kΩ, RPD 14.25–15.75 kΩ, Table 1684 p.1338).
- **100 nF on U2's VBUS pin** — ST's layout figure shows "CBUS = 100nF" at pin 5 (Fig. 18, p.9;
  same 100 nF on +VCC in Fig. 15, p.8) → C_ESD_VBUS 100 nF X7R 16 V.
- **Rd = 5.1 kΩ on each CC pin to GND** for a Type-C sink (Type-C spec Rd 5.1 kΩ ±10 %, sink CC
  termination table in §4.11, spec, from memory; intent.md "Compliance") → R_CC1, R_CC2
  5.1 kΩ 1 %, one per pin.
- **SBU1/SBU2 unconnected** — USB 2.0 sink, no alternate mode.

## Calculations

### CC pull-downs

- Rd: spec 5.1 kΩ ±10 % (spec, from memory); ours 5.1 kΩ ±1 % (C25905) → 5.05–5.15 kΩ, well
  inside. The value does not depend on how much current the sink wants; a sink with Rd only is
  entitled to default USB power, 500 mA for USB 2.0 (spec, from memory), which is the intent's
  budget. Nothing reads CC, so the board never draws more than default power, consistent with
  intent ("asks for nothing above default USB power").
- CC voltage the source sees (spec, from memory: default-current Rp is 56 kΩ to 5 V, or 80 µA):
  5 V × 5.1/(56 + 5.1) = 0.42 V, or 80 µA × 5.1 kΩ = 0.41 V — inside the source's "attached,
  default" window. With a legacy A-to-C cable the cable's own 56 kΩ Rp does this; also fine.
- Two resistors, one per CC pin, never shared: the plug connects only one CC line through, which
  one depends on orientation. Nothing else on CC1/CC2.
- Dissipation and voltage: worst 5.9 mW, 5.5 V (table above) on a 62.5 mW, 50 V part.

### USB full-speed termination and the pair

- Driver impedance: USB 2.0 full-speed wants 28–44 Ω per driver including the series resistor
  (spec, from memory); the RP2350 states 27 Ω is required (Table 1678), so its own driver supplies
  the rest. 27 Ω 1 % (C25100, **extended**, the only 27 Ω 0402 that showed up; the guide's value is
  not negotiable, so pay the fee).
- Differential impedance: 90 Ω target (guide p.16; spec 90 Ω ±15 %, from memory). Full-speed edges
  are 4–20 ns (spec, from memory), i.e. ≥ 0.6 m of FR-4 per edge, so an on-board run under ~40 mm
  is electrically short; intent.md already says "under 15 mm, tolerance don't care". The guide's
  0.8 mm / 0.15 mm geometry is for a 2-layer 1 mm board and does not apply to our 1.6 mm board; on 4
  layers with GND on L2 the pair is narrow (stackup.md, phase 4, sets the numbers in `nets.csv`).
- Line capacitance: U2 adds 2.5 pF typ / 3.5 pF max I/O-to-GND, 0.015 pF mismatch, 1.2–1.7 pF
  I/O-to-I/O at 1.65 V (Table 2, p.2). Full-speed tolerates far more than this (transceiver input
  capacitance limit 20 pF, spec, from memory). OK.

### ESD path and residual at the RP2350

ST's own worked example (p.4) for IEC 61000-4-2 level 4 contact (8 kV, 330 Ω): Ip ≈ 8 kV / 330 Ω
= 24 A; with VBR 6.1 V, Rd(TVS) 0.5 Ω, VT 1.1 V, Rd(diode) 0.5 Ω: **VCL+ = +31.2 V, VCL− = −13 V
before parasitic inductance**. ST then shows that 6 nH in each of the I/O and GND paths (10 mm ×
0.5 mm tracks) adds L·di/dt = 6 nH × 24 A/ns = **144 V each**, giving ≈ +319 V / −301 V (p.4–5).
Layout is therefore the parameter that matters, not the part.

What reaches U1: the residual at U2 drives current into U1's pad clamp through the 27 Ω. Assuming
the pad clamps near USB_OTP_VDD + 0.7 ≈ 4 V (assumption: no pad clamp curve in the datasheet):

- Good layout (parasitics small): (31.2 − 4) V / 27 Ω ≈ **1.0 A** peak for ~1 ns, falling with
  the IEC waveform. An HBM 2 kV event (the pad's rating, Table 1681) is 2 kV / 1.5 kΩ ≈ 1.33 A peak
  (JS-001 network, standard's values). Comparable or less → survivable.
- ST's "unsuitable" 10 mm layout: (319 − 4) / 27 ≈ 12 A → far beyond HBM class. Not survivable.

So U2 must sit within a few mm of J1 with its GND pin on short vias, and the 27 Ω stays at U1
(that is also where the guide wants it). Same arrangement as the vendor's Pico-class designs.

### VBUS at U2

- C_ESD_VBUS is the local return for the steered ESD current in the first nanoseconds and the
  first HF decoupler on VBUS (p.9). 100 nF X7R 16 V 0402 at 5 V: X7R loses little at 31 % of
  rating; counted as 0.09 µF in the VBUS tally (ldo-3v3.md, assumption −10 %).
- The VBUS TVS inside U2 (VBR ≥ 6 V) is also the only clamp on VBUS for hot-plug ringing; the
  hot-plug analysis and the 10 µF tally live in `ldo-3v3.md`, which owns the VBUS capacitance.
- No surge protection (IEC 61000-4-5) on VBUS or the data lines: U2 is an ESD part (VCL quoted at
  5 A 8/20 µs, p.2). Intent asks for ESD only. Recorded, not a defect.

### Shell

Shell (S1, 4 plated legs) straight to GND. The board has no chassis; the shell is the metal a
charged finger or cable touches first, and the lowest-impedance path to the GND plane is what
keeps the strike out of the pins. The vendor's minimal design does the same with its Micro-B shield
(Appendix A, p.21: shield pins 6–11 to GND). The alternative (shell to GND through 1 MΩ ∥ 1–4.7 nF,
"isolated chassis") would let the shell rise during a strike and couple it into the contacts;
rejected for a bus-powered board with no other earth.

## Fine print / gotchas

- **U2 is in-line, not a stub.** Pins 1 and 6 are both I/O1 and pins 3 and 4 both I/O2 (Fig. 1,
  p.1). Each line enters on one pin of its channel and leaves on the other, so the strike passes the
  clamp before it reaches the trace onward. Our schematic puts D− through I/O1 (pin 1 in from J1,
  pin 6 out) and D+ through I/O2 (pin 3 in, pin 4 out); ST's Fig. 18 (p.9) shows the opposite
  assignment. **The channel choice is arbitrary**: the two channels are identical (Fig. 1; Table 2,
  p.2 gives one set of numbers for "any I/O pin"). Because the connector-side and MCU-side nets have
  different names (USB_DM_C → pin 1, pin 6 → USB_DM), DRC does catch a route that bypasses U2; still
  check visually at the routing gate that each line enters one pin and leaves the other.
- **SOT23-6L vs SOT-666 pinout**: both packages use the same pin numbers (Fig. 1, p.1). The SC6
  marking is "UL26" (Table 5, p.13). Footprint: ST recommends 0.60 × 1.20 mm pads, 0.95 pitch,
  2.30 mm row pitch, 3.50 overall (Fig. 22, p.12); KiCad SOT-23-6 has 0.60 × 1.325 at ±1.1375
  (3.60 overall). Equivalent. OK.
- **VRM is 5.25 V, not 5.5 V** (p.2; rev 4 changed it "as specified in USB standard", p.13).
  Type-C sources may sit at 5.5 V (vSafe5V, spec, from memory). Leakage then rises above the
  150 nA spec but stays below 1 mA (VBR ≥ 6 V at 1 mA). Irrelevant at 500 mA; noted so nobody
  measures µA standby on VBUS and blames the LDO.
- **Leakage rises with temperature** (Fig. 4, p.3, about ×10 by 75 °C typical). Irrelevant here.
- **The D+/D− pads are interleaved in this receptacle**: A6 (D+) sits between B7 (D−) and A7 (D−),
  and A7 between A6 and B6 (D+). The two D+ pads and the two D− pads cannot both be joined on the
  top layer between the 0.2 mm pad gaps; one pair joins on top beyond the pad ends, the other
  through a via pair to an inner layer. Join stubs ≤ 2 mm (≈ 10 ps at FS; irrelevant to the eye).
  Both orientations must work: test with the plug both ways at bring-up.
- **SBU1/SBU2 (A8/B8) no-connect.** Put a no-connect flag on them in the schematic.
- **RP2350 USB pads are not 5 V tolerant** (Table 1678 type "USB IO", not "FT"). Nothing on the
  board puts more than 3.3 V on them in normal use; the bluebuzz reasoning about FT tolerance does
  not carry over.
- **No VBUS sensing**: the RP2350 has no VBUS-detect pin wired here; it is bus-powered, so it only
  runs (and only pulls up D+) when VBUS is present. Powering the board from the header VBUS pin
  with a USB cable attached to an unpowered host is "not supported" (intent).
- The 27 Ω go at U1 (guide p.16), not at J1; the guide also wants "a solid, uninterrupted area of
  ground copper, stretching the entire length of the track" under the pair (p.16).

## Schematic connections

| Part / pin | Net | Note |
|---|---|---|
| J1 A1, A12, B1, B12 | GND | |
| J1 A4, A9, B4, B9 | VBUS | |
| J1 A5 | CC1 | |
| J1 B5 | CC2 | |
| J1 A6, B6 | USB_DP_C | both orientations |
| J1 A7, B7 | USB_DM_C | both orientations |
| J1 A8, B8 | — | no-connect flag (SBU1/SBU2) |
| J1 S1 (shield, 4 legs) | GND | direct bond |
| U2 1 (I/O1, connector side) | USB_DM_C | D− enters here |
| U2 6 (I/O1, MCU side) | USB_DM | D− leaves here |
| U2 3 (I/O2, connector side) | USB_DP_C | D+ enters here |
| U2 4 (I/O2, MCU side) | USB_DP | D+ leaves here |
| U2 5 (VBUS) | VBUS | |
| U2 2 (GND) | GND | |
| U1 52 (USB_DP) | USB_DP_MCU | Table 1678, p.1335 |
| U1 51 (USB_DM) | USB_DM_MCU | |

| Passive | Value | Voltage / power | Dielectric / tol | LCSC | Footprint | Connects |
|---|---|---|---|---|---|---|
| C_ESD_VBUS | 100 nF | 16 V | X7R ±10 % | C1525 (basic) | C_0402_1005Metric | VBUS – GND at U2 pin 5 |
| R_CC1 | 5.1 kΩ | 62.5 mW, 50 V | ±1 % | C25905 (basic) | R_0402_1005Metric | CC1 – GND |
| R_CC2 | 5.1 kΩ | 62.5 mW, 50 V | ±1 % | C25905 (basic) | R_0402_1005Metric | CC2 – GND |
| R_USB_DP | 27 Ω | 62.5 mW, 50 V | ±1 % | C25100 (extended) | R_0402_1005Metric | USB_DP – USB_DP_MCU |
| R_USB_DM | 27 Ω | 62.5 mW, 50 V | ±1 % | C25100 (extended) | R_0402_1005Metric | USB_DM – USB_DM_MCU |

Symbols: `Connector:USB_C_Receptacle_USB2.0_16P`, `Power_Protection:USBLC6-2SC6` (pins 1/6 I/O1,
3/4 I/O2, 5 VBUS, 2 GND — matches Fig. 1, p.1), `Device:R`, `Device:C`. D− on I/O1 and D+ on I/O2
is the schematic's choice; the swap relative to ST's Fig. 18 is harmless (Fine print).

## Layout guidance

From the datasheets:
- U2 "as close as possible to the disturbance source (generally the connector)" (p.6); tracks from
  data lines to I/O pins, from VBUS to pin 5 and from pin 2 to the GND plane "as short as
  possible" (p.5, Fig. 7 "optimized layout"). Target: U2 within 3 mm of the J1 signal pads, pin 2
  with two vias straight to the L2 GND plane, C_ESD_VBUS within 1 mm of pin 5 with its own GND via.
- Route each line in on one pin of its channel and out on the other (Fig. 18, p.9): in our
  schematic D− J1 → pin 1, pin 6 → R_USB_DM; D+ J1 → pin 3, pin 4 → R_USB_DP.
- 27 Ω at U1, pair over solid GND for its whole length (guide p.16).

Ours:
- Order along the path: J1 pads → U2 → pair → R_USB_DP/DM within 2 mm of U1 pins 51/52 → U1.
  Pair rules for `nets.csv`: USB_DP_C/USB_DM_C, USB_DP/USB_DM and USB_DP_MCU/USB_DM_MCU as 90 Ω differential pairs
  (impedance class from stackup.md), matched within 1 mm, no layer change except the D+/D− join at
  J1, no plane split under it, away from VREG_LX and the crystal.
- R_CC1/R_CC2 within 2 mm of A5/B5, each with its own GND via.
- VBUS from the four J1 VBUS pads to U2 pin 5 and on to U3/D2/header as a short trace or pour,
  ≥ 0.4 mm on 1 oz for 0.5 A (the `nets.csv` calculation sets the final width).
- Board edge at y = +3.19 mm in footprint coordinates (drawing: 5.79 mm from the locating-hole line);
  the mouth then overhangs the edge by ≈ 0.46 mm. Keep components ≥ 1 mm from the shell slots.
- Shell legs: plated slots on GND, connected to the planes on every layer (no thermal reliefs on the
  inner planes is fine; JLC solders the legs in reflow, so keep the outer-layer connection relieved
  enough to wet).
- Keep U3 (the hottest part) away from J1: the receptacle is rated to 80 °C.
- Silkscreen at J1 (silkscreen rules): "USB-C 5V 500mA".

## Open questions

1. Phase 3: make the project-local J1 footprint per the 2020 drawing (pad y −4.45 … −3.10) or
   accept KiCad's with a logged decision. The pad-length figures are derived from two dimensions,
   not a dimensioned pad length; a reviewer should re-measure the drawing.
2. Does the board need to read the CC advertisement (a 3 A source)? Intent says no: default power
   only. Recorded here so nobody adds CC sensing without changing intent.
3. Bring-up test: enumerate with the plug in both orientations, on a USB-A host through an A-to-C
   cable and on a USB-C host.

## Findings

| Rank | Finding | Action |
|---|---|---|
| MINOR | KiCad's `USB_C_Receptacle_HRO_TYPE-C-31-M-12` SMT pads are 1.45 mm long at y −4.77…−3.32; the 2020 HRO drawing's land is ≈ 1.14 mm at y −4.24…−3.10 (derived). KiCad's pads are short by 0.22 mm at the front; the front shell slots are drilled 1.2 mm vs 1.4 mm drawn (0.4 mm clearance on the 0.8 mm leg remains). | Project-local footprint per the drawing (phase 3), or accept with a decision-log line. |
| MINOR | U2's VBUS working voltage is 5.25 V (p.2), below the 5.5 V Type-C upper limit; leakage at 5.5 V is unspecified but < 1 mA (VBR ≥ 6 V at 1 mA). | None; recorded. |
| NOTE | J1 pin map, 5 A / 20 V rating and −30…+80 °C range now verified from the HRO drawing; U2 pinout, ratings and layout guidance verified from ST's datasheet. Both were UNVERIFIED in the bluebuzz audit. | — |
| NOTE | RP2350 USB pads are type "USB IO" (Table 1678), not FT; their limit is the standard-pad row of Table 1680, not 5.5 V. | — |
| NOTE | ESD residual into U1 through 27 Ω is ≈ 1 A peak with a tight U2 layout (≤ HBM 2 kV class), ≈ 12 A with ST's "unsuitable" 10 mm layout. | U2 ≤ 3 mm from J1, two GND vias (Layout guidance). |
| NOTE | 27 Ω (C25100) is an extended part; required by the RP2350 (Table 1678, guide p.16). | Pay the fee. |
| NOTE | Gate-1 review #2/#3: connection table now uses the `nets.csv` names (USB_DP_C/USB_DM_C at J1, USB_DP/USB_DM between U2 and the 27 Ω, USB_DP_MCU/USB_DM_MCU at U1) and the schematic's channel assignment (D− on I/O1 1→6, D+ on I/O2 3→4). Channel choice is arbitrary. | Visual in-line check at the routing gate. |
