# U3 — AP7361C-33FGE-7 3.3 V 1 A LDO, U-DFN3030-8 Type E (VBUS → 3V3), its capacitors, and the VBUS capacitance tally

Datasheet: datasheets/ap7361c.pdf (Diodes DS37274 Rev. 5-2, October 2020; the page header still
carries an "Advance product information" watermark). Read: all 22 pages, including the
performance curves on pp.6–9, the ESR chart on p.12 and the U-DFN3030-8 drawings on pp.1, 17, 20
(rendered; their text does not extract). Datasheet page numbers are its own "n of 22".
MPN: AP7361C-33FGE-7   Supplier: C151007 AP7361C-33FGE-7 (DIODES, DFN-8-EP(3x3)): extended, JLC stock 1460, lifecycle normal, checked 2026-10-23
Ordering decode (p.13): "33" = 3.3 V, **"FGE" = U-DFN3030-8 (Type E)**, "-7" = 7" reel, 3000/reel.
Marking "SZ" + date code for the 3.3 V U-DFN part (p.14).

**Package change, 2026-10-23 (team lead):** SOT-223 (AP7361C-33E-13, C500795) → U-DFN3030-8,
because the SOT-223's 8.9 × 7.3 mm courtyard does not fit the 14.2 mm between the header rows. Same
die and datasheet; what changes is the pinout (EN appears), the thermal path (exposed pad instead of
a tab) and the footprint. Everything below is for the DFN part.

## Pinout (U-DFN3030-8 Type E, top view)

Pin table p.2 (U-DFN3030-8 column) and pin drawing p.1:

| Pin | Name | Function (p.2) | Our net |
|---|---|---|---|
| 1 | OUT | regulator output, ≥ 2.2 µF ceramic | 3V3 |
| 2 | NC | no connection | — (no-connect) |
| 3 | ADJ/NC | ADJ on the adjustable version only; NC on fixed parts | — (no-connect; never tie it anywhere) |
| 4 | GND | ground | GND |
| 5 | EN | enable, active high; 3 MΩ internal pull-down (p.5) | VBUS (tied to IN) |
| 6 | NC | no connection | — (no-connect) |
| 7 | NC | no connection | — (no-connect) |
| 8 | IN | regulator input, ≥ 1 µF ceramic | VBUS |
| EP | GND | exposed pad, drawn as "GND" on the p.1 pin drawing; Note 9 (p.6) mounts it "with thermal vias to bottom layer ground plane" | GND |

EN: "If this feature is not used, the EN pin should be tied to IN pin to keep the regulator output
on at all time" (p.11). Left open, the 3 MΩ pull-down holds the part off. The datasheet raises no
objection to tying EN to IN, and the 3V3 rail has no sequencing need (intent), so EN goes to VBUS.
OUT (pin 1) and IN (pin 8) are on opposite sides at the same end of the package.

## Footprint

Package (p.17): D = E = 2.95–3.05, A 0.57–0.63, pitch e 0.65, lead b 0.20–0.30, lead length L
0.30–0.60, exposed pad D2 2.15–2.35 × E2 1.40–1.60 (2.25 × 1.50 typ), chamfer on the EP's pin-1 corner.
Diodes' suggested land (p.20): lead pads X 0.30 × Y 0.65 at pitch C 0.65, pad rows Y2 2.75 apart
centre to centre (pads span 1.05–1.70 mm from centre), EP land C1 2.35 × Y1 1.60.

**Exact KiCad match: `Package_DFN_QFN:DFN-8-1EP_3x3mm_P0.65mm_EP1.5x2.25mm`.** Its description
cites Diodes' U-DFN3030-8-Type-E package file. Comparison:

| Feature | Diodes land (p.20) | KiCad EP1.5x2.25mm | Verdict |
|---|---|---|---|
| Lead pads | 0.30 × 0.65, centres ±1.375 (span 1.05–1.70) | 0.25 × 0.90, centres ±1.40 (span 0.95–1.85) | KiCad is narrower (0.25 = b nominal) and longer at both ends. Covers L 0.30–0.60 better at the inner end (lead inner end 0.90–1.20 from centre). OK |
| Pitch, pin order | 0.65; 1–4 down one side, 5–8 up the other | same, pin 1 at (−1.4, −0.975) | OK |
| EP | 2.35 × 1.60 | 2.25 × 1.50 (= package typ), pin 9 | 0.05 mm smaller per side; lead-to-EP copper gap 0.20 mm (Diodes 0.25). OK for JLC |
| EP paste | not given | 4 windows 0.60 × 0.91, ≈ 65 % coverage | OK |
| Courtyard | — | 4.2 × 3.5 mm | Fits the 14.2 mm space |

There is no `_ThermalVias` version of this footprint in KiCad 9 (the 1.2 × 2.15 and 1.6 × 2.56
variants have one but don't match this package). **Project footprint needed only to add vias**:
copy `DFN-8-1EP_3x3mm_P0.65mm_EP1.5x2.25mm` into `hw/easy2350/a/lib/easy2350.pretty/` and add five
pad-9 through-hole vias, 0.2–0.25 mm drill / 0.45 mm pad, at (±0.40, ±0.70) and (0, 0). KiCad's
paste windows (0.60 × 0.91 at (±0.375, ±0.56)) sit on those spots, and the paste gaps (0.15 and
0.21 mm) are narrower than any via, so redraw the EP paste as windows that stay ≥ 0.1 mm clear of
the vias, keeping ≈ 50 % coverage. Lead pads, EP copper and courtyard stay as KiCad has them.
Alternatively phase 6 places free vias in the EP; the copy is cleaner because the vias move with
the part. Untented vias in a DFN pad wick solder; plugged or capped vias in pad (if the fab option
is offered for the stackup) or the small 0.2 mm drill keeps that small (assumption: general
assembly practice, not from this datasheet).

Symbol: KiCad has no FGE symbol; the team lead makes a project symbol from the pin table above and
in Schematic connections.

Supporting data: `samsung-cl21a106kaynnng.pdf` (Samsung characteristic sheet for CL21A106KAYNNN*,
the 10 µF 0805 25 V X5R; the G and E suffixes differ only in reel type, catalog p.4) and
`samsung-cl05a105kp5nnnc.pdf` (1 µF 0402 10 V X5R, used only to reject 0402 parts); catalog
`samsung-mlcc.pdf` p.4 for part-number decoding. Sources in `datasheets/sources.csv`.

Operating conditions (intent.md): VIN = VBUS 4.5–5.5 V at J1 (≥ 6 V don't care); 3V3 load
277 mA max, ~35 mA typ; 3.3 V ±3 %, ripple < 50 mVpp; 0–40 °C still air; 21 × 51 mm board.

## Absolute max vs our worst case

| Parameter | Rating (page) | Our worst case | Margin |
|---|---|---|---|
| VIN abs max | 6.5 V (p.4) | 5.5 V steady; hot-plug ring ≈ 6.5–8 V for µs with a short low-resistance cable (**assumption**, Calculations) | 1.0 V steady; **marginal during hot-plug** (MINOR) |
| VIN recommended | 2.2–6.0 V (p.4) | 4.45–5.5 V at the pin | OK |
| EN | ≤ VIN + 0.3 V (p.4) | EN tied to IN: EN = VIN at every instant, including the hot-plug ring | OK by construction |
| OUT, ADJ | ≤ VIN + 0.3 V (p.4) | ADJ/NC unconnected. OUT 3.3 V with VBUS present. With VBUS absent and someone driving the header 3V3 pin, OUT > VIN + 0.3: intent lists "external supply on the 3V3 pin" as not protected, don't care | OK as specified; silk states it |
| IOUT recommended | 0–1.0 A (p.4) | 0.277 A | 3.6× |
| Current limit | ≥ 1.1 A, 1.5 A typ (p.5); ≈ 1.4 A over temperature (p.8) | 0.277 A | OK |
| Short-circuit (foldback) current | 400 mA typ when VOUT < 15 % (p.5, p.11); ≈ 330 mA over temperature (p.8) | Start-up load ≪ 330 mA (RP2350 held until its own POR) | OK, start-up does not latch in foldback |
| TJ | −40…+150 °C operating (p.4); thermal shutdown 150 °C typ, 20 °C hysteresis, no min (p.5, p.12) | 83 °C at 40 °C ambient on the datasheet's θJA (Calculations); design limit 125 °C (25 °C below a typical-only shutdown) | 42 °C to the design limit on the datasheet figure |
| PD (U-DFN3030-8, TA = 25 °C) | 1700 mW (p.4) | 610 mW | OK (and see thermal) |
| TA recommended | −40…+85 °C (p.4) | 0–40 °C | OK |
| ESD | HBM > 2 kV, MM > 200 V (p.4) | Not user-touched; VBUS reaches it through U2's TVS | OK |

## Required external parts (from datasheet)

- **CIN: "Bypass to ground through at least 1µF ceramic capacitor"** (pin table, p.2); "A 1μF
  ceramic capacitor is recommended ... may be increased without limit ... as close as possible to
  the device" (p.10) → C_LDO_IN 2.2 µF 16 V X5R 0603 (C23630), raised from 1 µF after the gate-1
  review (#9).
- **COUT: "at least 2.2μF" ceramic** (p.2, p.10); stable for ESR 0–10 Ω at any load (p.12 chart);
  larger improves transients (p.10) → C_LDO_OUT 10 µF 25 V X5R 0805 (C15850).
- The characterisation conditions use CIN = COUT = 4.7 µF (p.5).
- **EN tied to IN** (p.11; see Pinout). EN VIH ≥ 1.0 V, VIL ≤ 0.3 V (p.5): EN follows VBUS, so the
  part turns on once VBUS passes ~1 V and is fully on long before VIN reaches 4.5 V.
- Exposed pad to GND with thermal vias (p.6 note 9; p.1 drawing). Nothing else: fixed output, no
  ADJ divider (pin 3 NC), no minimum load (p.11 "No Load Stability").

## Calculations

### Dissipation and junction temperature

PD = (VIN − VOUT) × IOUT + VIN × IQ (p.12 gives the first term; IQ 80 µA max, p.5, adds 0.4 mW).

| Case | VIN | IOUT | PD |
|---|---|---|---|
| Worst (intent) | 5.5 V | 277 mA | (2.2 V × 0.277 A) = **0.61 W** |
| USB 2.0 host nominal | 5.0 V | 277 mA | 0.47 W |
| Typical (`hello_usb`, intent) | 5.0 V | 35 mA | 0.06 W |

θJA = **70 °C/W** for U-DFN3030-8 (Type E), "mounted on 2"x2", FR-4 substrate PCB, with minimum
recommended pad on top layer and thermal vias to bottom layer ground plane" (p.6, note 9; the
number of vias and copper weight are not stated). TJ = TA + PD × θJA (p.12):

- TJ = 40 + 0.61 × 70 = **83 °C** worst case (with the lead's rounded 0.62 W: 83.4 °C) → 42 °C under
  our 125 °C design limit, 67 °C under the 150 °C operating limit (p.4). The intent's phase-1
  requirement ("the junction stays under its rating at 40 °C ambient") is met.
- θJA allowed for TJ ≤ 125 °C: (125 − 40)/0.61 = **139 °C/W**, i.e. the DFN can be twice as badly
  cooled as Diodes' test board and still pass. Allowed PD at 40 °C for 150 °C (p.12 formula):
  (150 − 40)/70 = 1.57 W.
- The DFN beats the SOT-223 (110 °C/W, minimum pad, no vias) only because of the vias: all of its
  heat leaves through the EP. Without them θJA would be far worse than 70 °C/W.

**Copper and vias the EP needs (phase 4/6 must provide).** Diodes' 70 °C/W assumes the suggested
land (EP 2.35 × 1.60, p.20) with vias to a ground plane on a 2" × 2" (51 × 51 mm, 2581 mm²) board.
Ours is 21 × 51 mm (1071 mm², 41 % of that), so we lean on the inner plane, not on board area:
(a) EP with ≥ 5 vias (0.2–0.25 mm drill) to the L2 GND plane (footprint above). A plated via
0.25 mm drill, 18 µm plating, through the ≈ 0.2 mm L1–L2 dielectric is ≈ 0.2 mm / (385 W/mK × π ×
0.25 mm × 0.018 mm) ≈ 37 °C/W each, ≈ 7 °C/W for five (assumption: stackup dielectric ≈ 0.2 mm;
phase 4 confirms); (b) top-layer GND copper leaving the EP at both of its pin-free ends (the EP's
±y ends, between the pin columns: up to 1.5 mm wide with 0.2 mm clearance to pins 1/8 and 4/5),
joined to pin 4 and to a top GND pour of ≥ 50 mm² stitched with ≥ 4 more vias to L2; (c) VBUS to
pins 8/5 and 3V3 from pin 1 ≥ 0.4 mm wide; (d) L2 GND solid under and around U3 (no splits within
5 mm); (e) U3 at least ~5 mm from J1 (80 °C rated), Y1 and the pixels.

**Whole-board caveat (assumption-based estimate, MINOR).** The 70 °C/W treats U3 as the only heat
source on a larger test board. On our 21 × 51 mm board at the simultaneous worst case (5.5 V in,
277 mA 3V3, four pixels at full white) the board dissipates roughly 0.61 W (U3) + 0.82 W (pixels,
160 mA × ≈ 4.8–5 V) + ≈ 0.1 W (LED-rail D2 or regulator, rgb-leds.md) + ≈ 0.2 W (RP2350, flash)
≈ 1.7 W. With still-air convection plus radiation of h ≈ 15 W/m²K on both faces (assumption,
textbook order of magnitude for a small board; the cable and headers conduct some heat away), the
board averages ≈ 1.7 / (15 × 2 × 21 × 51 × 10⁻⁶) ≈ 50 °C above ambient. U3's junction then sits
above that by its local rise, junction → EP → vias → L2 plane, ≈ 20–30 °C/W (assumption) × 0.61 W
≈ 12–18 °C → TJ ≈ 100–110 °C in this corner, still under the 125 °C design limit. It is a corner:
typical use is < 0.2 W total. Actions: phase 6 places U3 away from the pixels; bring-up measures
U3's case temperature at 5.5 V, 277 mA load and full-white pixels. If it exceeds ~95 °C case, the
fix is a firmware brightness cap or a lower header-3V3 limit (intent change).

### Dropout at our current

VDROPOUT, 2.6 V ≤ VOUT ≤ 3.3 V, IOUT = 300 mA: 90 mV typ, **140 mV max** (TA = 25 °C, p.5;
defined at 2 % output drop, note 7). The dropout-vs-current curve (p.8) is linear in current:
≈ 390 mV at 1 A, 85 °C → **≈ 108 mV at 277 mA, 85 °C** (typical). The junction stays ≤ 85 °C at the
datasheet θJA (83 °C) but may reach ≈ 110 °C in the whole-board corner; we take the 25 °C maximum
scaled by the 85 °C/25 °C curve ratio (1.17) and add margin for > 85 °C: ≤ 1.5 × 140 = **210 mV**
(assumption).

VIN needed = 3.3 V + 0.21 V = 3.51 V. VIN available = 4.5 V at J1 − ≈ 50 mV (0.5 A through J1
contacts and VBUS copper, assumption ≤ 0.1 Ω) = 4.45 V. **Headroom 0.94 V over the worst dropout;
the intent's "dropout under 1.0 V at 280 mA" is met with 0.79 V to spare (0.21 V vs 1.0 V).**

### Output accuracy against 3.3 V ±3 %

The temperature coefficient is a junction property, so it is applied from the 25 °C test point to
the junction temperature, not to ambient (gate-1 review #5).

| Term | Value (page) | At TJ = 83 °C (datasheet θJA) | At TJ ≈ 110 °C (whole-board corner) |
|---|---|---|---|
| Initial accuracy | ±1 % at 25 °C, 100 mA, VIN = 4.3 V (p.5) | ±1.00 % | ±1.00 % |
| Line regulation | 0.2 %/V max, −40…85 °C (p.5); our VIN 4.45–5.5 V vs the 4.3 V test point → ≤ 1.2 V | ±0.24 % | ±0.24 % |
| Load regulation | ±1 % for 1 mA–1 A (p.5); curve p.9 shows +0.1 % at 277 mA, 85 °C | ±1.00 % (spec) | ±1.00 % (spec) |
| Temperature | ±100 ppm/°C **typ, no max** (p.5), × (TJ − 25 °C) | ±0.58 % | ±0.85 % |
| **Sum (worst-case linear)** | | **±2.82 %** → 3.207–3.393 V | **±3.09 %** → 3.198–3.402 V |
| RSS | | ±1.6 % | ±1.7 % |

The reviewer's figure, **±3.06 %**, is the same sum at the SOT-223's 107 °C. With the DFN the sum
is ±2.82 % at the datasheet junction temperature and ±3.09 % at our whole-board corner estimate.
The intent's ±3 % is therefore met on the datasheet thermal figure and exceeded by ≤ 0.1 % in the
corner, on a linear stack of limits. That is acceptable because:
- the real limits have ≥ 0.2 V margin at either extreme: RP2350 IOVDD/QSPI_IOVDD abs max 3.63 V
  (Table 1680, p.1336) vs 3.402 V; flash 3.0–3.6 V for 133 MHz (W25Q p.61) vs 3.198–3.402 V;
- the tempco is typical only, and the linear sum adds four independent limits at their extremes;
  RSS is ±1.7 %;
- our load swing (≤ 277 mA from the 100 mA test point) uses a fraction of the ±1 % band, which is
  specified for 1 mA–1 A (p.9 curve: +0.1 % at 277 mA, 85 °C).
Recommendation for intent.md (lead's call): "3.3 V ±3 % (RSS); ±3.1 % linear worst case".

### Ripple and transients (< 50 mVpp)

- Load step 100 → 500 mA, VIN 4.3 V, COUT 4.7 µF: ≈ 20 mV undershoot and ≈ 20 mV overshoot,
  recovering in ≈ 100 µs (p.7, read from the 20 mV/div trace). Our steps are ≤ 277 mA into
  ≈ 8 µF effective (below), so smaller. Inside 50 mVpp.
- Line: PSRR 75 dB at 1 kHz, 55 dB at 10 kHz (p.5): 100 mV of VBUS ripple at 10 kHz becomes 0.2 mV.
  Line step 4.3 → 5.3 V in 5 µs gives a few mV at 3.3 V out (p.6 trace).

### Capacitors and DC-bias derating

**C_LDO_OUT — 10 µF 25 V X5R 0805, CL21A106KAYNNNE (C15850, basic).** Samsung's DC-bias curve
(samsung-cl21a106kaynnng.pdf, item 3; digitised): −18 % at 3.3 V, −36 % at 5 V. At 3.3 V: 8.2 µF;
with −10 % tolerance 7.4 µF; X5R ±15 % over its range (catalog p.5), we use ≈ −5 % for 0–40 °C
(TCC curve, item 2, is flat to within a few % there) → **worst ≈ 7 µF ≥ 2.2 µF required (3×)**.
Rated 25 V vs 3.3 V. Rejected: 4.7 µF 0402 10 V (C23733): no curve for it, and the 0402 1 µF 10 V
proxy sheet (samsung-cl05a105kp5nnnc.pdf) loses ≈ 56 % at 3.3 V; a 4.7 µF 0402 would likely fall
near or below 2.2 µF. The RP2350's own 3V3 decouplers (core audit) add to it.

**C_LDO_IN — 2.2 µF 16 V X5R 0603, CL10A225KO8NNNC (C23630, basic; `jlc_parts.py show`:
"C23630 CL10A225KO8NNNC (Samsung Electro-Mechanics, 0603): basic, JLC stock 3183306, lifecycle
normal, checked 2026-10-23").** Decoded from the catalog (p.4): A = X5R, 225 = 2.2 µF, K = ±10 %,
O = 16 V, 8 = 0.80 mm thick. Raised from 1 µF 50 V 0603 (C15849) after the gate-1 review (#9), so
that both the LDO's "at least 1 µF" (p.2) and the USB 2.0 minimum of 1 µF directly on VBUS (spec,
from memory) hold with margin. No DC-bias curve was obtainable for it (Samsung's site returns HTTP
500 to scripts). Assumption: ≤ 50 % loss at 5.5 V (34 % of its rating; the measured 0805 25 V part
loses 36 % at 20 % of rating, the 0402 10 V part 72 % at 50 %) → ≥ 1.1 µF, ≈ 1.0 µF after −10 %
tolerance, plus C_ESD_VBUS 0.1 µF on the same net. Rejected: 1 µF 0402 25 V (C52923; the 10 V 0402
proxy loses ≈ 72 % at 5 V); 10 µF 0805 at the input (VBUS budget). CIN is not a stability element
(p.10, p.12).

### VBUS capacitance tally against the 10 µF attach limit

The limit (USB 2.0 §7.2.4.1, spec, from memory): at most 10 µF ∥ 44 Ω at the device end of the
cable, where the 10 µF "represents any bypass capacitor directly connected across VBUS ... plus
any capacitive effects visible through the regulator"; more needs inrush limiting. What it
constrains physically is the charge drawn at attach (10 µF × 5 V = 50 µC).

**Caps behind D2 count.** At attach VBUS rises from 0 and D2 is forward-biased for the whole rise,
so the LED rail's caps charge through it from the same cable at the same time; the diode only
stops them discharging back. They are inrush load, charged to VBUS − VF. D2 is now an SM4007PL
silicon rectifier (rgb-leds.md; VF ≤ 1.1 V at 1 A, sm4007pl.pdf p.1; ≈ 0.6–0.7 V at mA currents,
assumption), with a 4.7 kΩ bleed and a BZT52B5V1 5.1 V zener on VLED (neither adds capacitance).

| Item | Nominal | Charge at attach, VBUS 5.5 V (upper bound: nominal C, no DC-bias loss, +10 %) |
|---|---|---|
| C_ESD_VBUS 100 nF X7R 16 V (usb-front-end.md) | 0.10 µF | 0.6 µC |
| C_LDO_IN 2.2 µF X5R 16 V 0603 | 2.2 µF | 13.3 µC |
| C_VLED 4.7 µF 0603 (rgb-leds.md) | 4.7 µF | 25.8 µC (charged to ≈ 5.0 V behind D2) |
| 4 × 100 nF at the pixels + 100 nF at U4 (rgb-leds.md) | 0.50 µF | 2.7 µC |
| **Total** | **7.5 µF** (2.3 µF directly on VBUS, 5.2 µF behind D2) | **≤ 42 µC ≈ 7.7 µF-equivalent at 5.5 V; limit 10 µF / 55 µC** |

**Inside the limit nominally (7.5 µF of 10 µF) and by charge, even with no DC-bias credit.** DC-bias
loss on the 0603 parts only lowers it further. The direct-on-VBUS part (2.3 µF nominal) also meets
the ≥ 1 µF minimum (spec, from memory; gate-1 review #9).

(The earlier assumption of a 10 µF 0805 on the LED rail gave 11.5 µF nominal and ≈ 52 µC by the
integrated Samsung curve, over the limit; rgb-leds.md now uses 4.7 µF.) D2 conducts forward, so
C_LDO_IN and C_ESD_VBUS also feed pixel current steps through it.

**Behind U3 (for transparency; owned by the core audit):** 3V3 holds C_LDO_OUT (≈ 8 µF) plus the
RP2350 decoupling (several µF), roughly 15 µF effective → ≈ 50 µC drawn through U3 in its
≈ 150–200 µs start-up (p.5, p.12). By the "visible through the regulator" wording (from memory)
this also counts, and nothing on this board limits it. Intent claims no formal compliance and
states the limit as VBUS bulk; Open question 3 asks the user which reading to hold.

### Hot-plug ring on VBUS (vs 6.5 V abs max)

Plugging a live cable into ≈ 5–6 µF (effective) of ceramic forms a series LC. Assumptions:
cable inductance 0.5–1 µH for 1 m; loop resistance 0.07 Ω (20 AWG power pair) to 0.4 Ω
(28 AWG) plus contacts. Z0 = √(L/C) = 0.29–0.45 Ω; ζ = R/(2Z0).

- Thin cable (0.4 Ω): ζ ≈ 0.45–0.7 → overshoot ≤ 20 % → ≤ 6.6 V from a 5.5 V source, ≤ 6.0 V
  from a 5.0 V host. Marginal at the 5.5 V corner.
- Thick short cable (0.1 Ω): ζ ≈ 0.11–0.17 → 60–70 % overshoot → ≈ 9 V unclamped (the reviewer
  got 9.2 V for 6 µF, 1 µH), with ≈ 3–4 A available into a clamp. U2's VBUS TVS conducts above
  VBR ≥ 6 V (Table 2, p.2), but **ST specifies no clamping voltage at amps for the VBUS pin**: the
  VCL figures (12 V at 1 A, 17 V at 5 A, p.2) are for the I/O pins. **Assumption:** using ST's
  worked-example values (VBR 6.1 V, Rd 0.5 Ω, p.4 — "if in first approximation, we assume"), the
  clamp holds the peak at ≈ 6.6–8 V for 1–4 A for a few µs. That bound is not datasheet-backed. U3's VIN abs max is 6.5 V (p.4); the package change does not
  change it (same die, one abs-max table for all packages). EN rides on IN, so EN ≤ VIN + 0.3 V
  holds through the ring; the DFN adds no new exposure.
- **VLED after the ring.** C_VLED charges through D2 to (VBUS peak − VF) ≈ 5.9–6.4 V and cannot
  discharge back through D2. rgb-leds.md now clamps VLED with a BZT52B5V1 zener (C19077393, ±2 %,
  no added capacitance). The decay constants are stated once, there: above the zener's ≈ 5 V knee
  τ ≤ 0.16 ms, so VLED is back under the pixels' 5.3 V within ≈ 0.26 ms of the ring; below the knee
  the bleed alone gives τ_bleed = 4.7 kΩ × 2.7 µF effective ≈ 13 ms. (This replaces the 24 ms
  figure an earlier version of this file gave, which used 5.2 µF nominal and had no zener.) The
  rgb-leds audit owns the pixel consequence.

Every bus-powered board with a 6–6.5 V-rated LDO on VBUS lives with this and it rarely fails (the
energy, ½LI², is µJ-class), but the margin is not demonstrated (MINOR). Bring-up test: 5.5 V stiff
bench supply, short 3 A-rated C-to-C cable, scope U3 pin 8 and VLED on attach, ten plugs. If the
peak at U3 exceeds 6.5 V, the review suggests a dedicated 5 V-standoff TVS (SMF5.0A class) on VBUS.
Note that such a TVS has VBR ≥ 6.4 V and clamps well above 6.5 V at amps too (from memory of that
part family); it adds energy capacity, not a lower clamp. The fixes that lower the peak are damping
(a series-R or ESR element in the VBUS bulk, inside the 10 µF budget) or a regulator rated for a
higher VIN; the measurement decides. Note that cutting capacitance raises the
overshoot ratio (less damping) while cutting its energy; the TVS clamp is what bounds it either way.

## Fine print / gotchas

- **Package suffix sets the pinout** (p.2): FGE (U-DFN3030-8) is 1 OUT / 4 GND / 5 EN / 8 IN / EP GND;
  SOT223 "E", SOT223R "ER", SOT89-5 "Y5", SO-8EP "SP" and SO-8EPR "SPR" are all different. Pin the
  MPN AP7361C-33FGE-7 in the BOM; a "same part, other package" substitution is a different
  footprint and symbol.
- **EN must not float**: 3 MΩ internal pull-down (p.5) turns the regulator off if EN is left open.
- **Pin 3 (ADJ/NC)**: the fixed part's pin 3 is listed as ADJ/NC; leave it unconnected rather than
  tie it to GND or OUT.
- **The EP is the thermal path.** Without vias the DFN is worse than the SOT-223 it replaced.
- **Thermal shutdown is a typical figure only** (150 °C typ, p.5), and the part cycles if it gets
  there (p.12). Hence our 125 °C design limit rather than 150 °C.
- **Dropout is specified at 25 °C only** (p.5); the 85 °C curve (p.8) is typical. We used an
  assumption above 85 °C; the margin (0.94 V) dwarfs it.
- **θJA is for the minimum pad and an unstated board size** (p.6 note 9). Our figure inherits that
  uncertainty; see the whole-board caveat.
- **Output discharge resistor 100 Ω** (p.5): pulls 3V3 down when EN goes low, i.e. when VBUS
  falls below ≈ 0.3–1 V at unplug. Harmless.
- **Foldback** to ≈ 330–400 mA below 15 % of VOUT (p.5, p.8, p.11): a hard short on the header 3V3
  pin draws ≈ 0.4 A × 5.5 V ≈ 2.2 W in U3 until thermal shutdown cycles it. Survivable by design
  (p.12) but hot; silk says 3V3 out 100 mA.
- **Advance-information watermark** on the datasheet pages: the product is in production (JLC stock,
  lifecycle normal) but the document says it may change. Recheck the revision at phase 10.
- **Backfeed**: OUT > VIN + 0.3 V (p.4) when the header 3V3 pin is driven with VBUS absent. Intent
  accepts this; it is on the silk.
- **Unplug**: OUT can also exceed VIN + 0.3 V briefly at every unplug, not only via the header.
  VBUS (2.3 µF direct; the LED rail is isolated by D2) can collapse faster than 3V3 (≈ 15 µF) when
  the VBUS-side load is the heavier one; 3V3 then discharges back through U3's pass element into
  VBUS. The reverse charge is bounded by the 3V3 rail's ≈ 50 µC; this is the normal condition of
  every bus-powered LDO board, and the datasheet rates the pin voltage without forbidding reverse
  conduction (gate-1 review #7). No change.

## Schematic connections

Project symbol (KiCad has no FGE symbol) — pin table for it:

| Pin | Name | Electrical type | Net | Symbol side (inputs left, outputs right) |
|---|---|---|---|---|
| 1 | OUT | power output | 3V3 | right |
| 2 | NC | no connect | — | hidden or NC flag |
| 3 | ADJ/NC | no connect (fixed part) | — | hidden or NC flag |
| 4 | GND | power input | GND | bottom |
| 5 | EN | input | VBUS | left, below IN |
| 6 | NC | no connect | — | hidden or NC flag |
| 7 | NC | no connect | — | hidden or NC flag |
| 8 | IN | power input | VBUS | left |
| 9 (EP) | GND | power input (passive) | GND | bottom, stacked with pin 4 |

Footprint pin 9 is the exposed pad in `DFN-8-1EP_3x3mm_P0.65mm_EP1.5x2.25mm` (and in the project
copy with vias). Fields: MPN AP7361C-33FGE-7, LCSC C151007.

| Passive | Value | Voltage | Dielectric / tol | LCSC | Footprint | Connects |
|---|---|---|---|---|---|---|
| C_LDO_IN | 2.2 µF | 16 V | X5R ±10 % (CL10A225KO8NNNC) | C23630 (basic) | `Capacitor_SMD:C_0603_1608Metric` | VBUS – GND at U3 pin 8 |
| C_LDO_OUT | 10 µF | 25 V | X5R ±10 % (CL21A106KAYNNNE) | C15850 (basic) | `Capacitor_SMD:C_0805_2012Metric` | 3V3 – GND at U3 pin 1 |

The capacitor choices hold for the DFN: same die, same CIN ≥ 1 µF and COUT ≥ 2.2 µF requirements
(p.2, p.10), same ESR window (p.12). Nothing in the package change touches the VBUS tally.

## Layout guidance

From the datasheet:
- CIN "as close as possible to the device"; "a wide copper trace is required for both IN and GND
  pins" (p.10). COUT "as close as possible to OUT and GND pins" (p.10).
- Thermal: power dissipation and "proper sizing of the thermal plane ... is critical" (p.12).

Ours (phase 4/6 must provide):
- EP vias and top GND copper as in "Copper and vias the EP needs" above: ≥ 5 EP vias to L2, copper
  leaving the EP at its two pin-free ends into a ≥ 50 mm² top GND pour with ≥ 4 more vias; pin 4
  joined to it.
- C_LDO_IN within 1.5 mm of pin 8 (its GND end into the same pour); C_LDO_OUT within 1.5 mm of
  pin 1. Pins 1 and 8 are at the same end of the package on opposite sides, so the two caps sit
  either side of that end.
- EN (pin 5) to IN (pin 8): short trace along the pin-5/8 side, outside the package.
- Pins 2, 3, 6, 7: no copper beyond their pads.
- VBUS (≥ 0.4 mm) and 3V3 (≥ 0.4 mm, or a pour) traces; final widths from `nets.csv` (277 mA).
- U3 ≥ 5 mm from J1 (80 °C part), Y1 (frequency drift) and the pixels (shared heat); not under the
  RP2350's regulator corner.
- Test points: by design decision VBUS, 3V3 and GND are probed on header pins 40, 36 and 38;
  only 1V1 has a pad (TP3, core audit). Keep the VBUS and 3V3 runs from U3 to those header pins
  short enough that the bring-up hot-plug and thermal measurements at the header represent U3's
  pins; for the hot-plug scope shot, probe U3 pin 8 / C_LDO_IN directly.
- Placement note for phase 6 (gate-1 review #11): if the bring-up thermal measurement fails, the
  planned mitigation is a firmware cap on pixel brightness; layout keeps U3 away from the pixels so
  that it is the last resort.

## Open questions

1. ~~SOT-223 tab connection~~ Moot after the package change; the DFN's EP is drawn as GND (p.1).
2. ~~LED-rail bulk~~ Resolved: rgb-leds.md uses C_VLED 4.7 µF + 0.5 µF; VBUS total 6.3 µF nominal.
3. **Does the 10 µF limit include 3V3 capacitance charged through U3?** (USB 2.0 wording from
   memory says yes; intent says "VBUS bulk". No compliance is claimed.) User decision; record in
   decisions.md.
4. **Thermal at the simultaneous worst case** (5.5 V, 277 mA, full-white pixels): datasheet θJA
   gives 83 °C; a whole-board estimate gives ≈ 100–110 °C. Bring-up measurement.
5. **Hot-plug peak** at U3 pin 8 with a thick short cable: measure at bring-up (6.5 V abs max; the
   clamp level is an assumption, see Calculations).
6. C_LDO_IN (2.2 µF 16 V 0603) DC-bias curve was not obtainable; ≥ 1.0 µF effective is an assumption.
7. Stackup (phase 4): confirm the L1–L2 dielectric (≈ 0.2 mm assumed for the via estimate) and
   whether plugged/capped via-in-pad is available for the EP vias.

## Findings

| Rank | Finding | Action |
|---|---|---|
| NOTE | VBUS capacitance 7.5 µF nominal (2.3 µF direct + 5.2 µF behind D2), ≤ 42 µC at attach (upper bound), inside 10 µF; ≥ 1 µF direct minimum met. Caps behind D2 count: they charge through it at attach. (Was over with the earlier 10 µF LED bulk; rgb-leds.md now uses 4.7 µF.) | — |
| MINOR | Hot-plug ring with a low-resistance cable can exceed U3's 6.5 V abs max; U2's VBUS TVS bounds it, but its clamp level at amps is not specified (≈ 6.6–8 V is an assumption from ST's worked example). VLED behind D2 is clamped back under 5.3 V within ≈ 0.26 ms by the BZT52B5V1 (rgb-leds.md). | Accept for U3 (common practice); scope VBUS and VLED at bring-up; pixel consequence in rgb-leds.md. |
| MINOR | Thermal (DFN): TJ = 83 °C at 40 °C ambient on the datasheet θJA (70 °C/W, with vias); ≈ 100–110 °C in the whole-board worst-case estimate. Depends entirely on the EP vias. | Project footprint with ≥ 5 EP vias; copper rules above; bring-up measurement. |
| NOTE | C_LDO_IN raised to 2.2 µF 16 V X5R 0603 (C23630, basic, verified 2026-10-23) per gate-1 review #9: ≥ 1.0 µF effective (assumed DC bias) meets the "at least 1 µF" of p.2 and the USB ≥ 1 µF minimum. | — |
| NOTE | Package changed to U-DFN3030-8 (AP7361C-33FGE-7, C151007) to fit between the headers. Pinout 1 OUT, 2/6/7 NC, 3 ADJ/NC, 4 GND, 5 EN, 8 IN, EP GND (p.1, p.2); EN tied to IN (p.11). KiCad footprint `DFN-8-1EP_3x3mm_P0.65mm_EP1.5x2.25mm` matches (cites Diodes' Type E drawing); project copy only to add EP vias. No KiCad symbol: project symbol from the pin table. | Pin the MPN. |
| MINOR | Accuracy (gate-1 review #5): tempco applied to TJ gives ±2.82 % linear at the datasheet TJ (83 °C) and ±3.09 % at the whole-board corner (≈ 110 °C), vs the intent's ±3 %; RSS ±1.7 %. Real limits (IOVDD 3.63 V abs max, flash 3.0–3.6 V) keep ≥ 0.2 V margin. | Accept; lead may restate intent as ±3 % RSS / ±3.1 % linear. |
| NOTE | Dropout ≤ 0.21 V at 277 mA (assumed worst, junction up to ≈ 110 °C) against 1.15 V available; accuracy in the MINOR row above. | — |
| NOTE | U3 is an extended part (setup fee), stock 1460 (quantity 5 needs 5). | — |
