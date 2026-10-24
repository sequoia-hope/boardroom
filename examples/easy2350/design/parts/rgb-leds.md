# RGB LEDs — D2 VLED diode, U4 level shifter, D3–D6 WS2812B-2020-V6 pixels

Block: VBUS → D2 → VLED → U4 (3.3 V → VLED logic) and four pixels chained
DIN → DOUT. Page numbers: PDF page for the small datasheets. For
rp2350-datasheet.pdf, the number printed in the page footer, which is the PDF
page − 1.

> **CHANGED in this audit — D2 is no longer the B5819W Schottky.** At idle the
> pixels and U4 draw only microamps (pixel Io ≤ 1 µA, p.3; U4 ICC ≤ 10 µA,
> p.7), and at microamps a 40 V Schottky drops almost nothing: its leakage
> alone is microamps at 40 °C (b5819w.pdf p.2, Reverse Characteristics). So VLED
> sits at ≈ VBUS = 5.5 V, above the pixel's 5.3 V maximum (ws2812b-2020-v6.pdf
> p.3). **D2 is now a silicon rectifier, MDD SM4007PL (C64898, basic,
> SOD-123FL), plus a 4.7 kΩ bleed resistor on VLED** so the diode always carries
> ≥ 1 mA and its drop is bounded. VLED is then 3.52–5.13 V for VBUS 4.5–5.5 V.
> Both ends are inside the pixel's 3.3–5.3 V range. Calculations are below.

---

## D2 — SM4007PL silicon rectifier, VBUS → VLED (replaces B5819W)

Datasheet: datasheets/sm4007pl.pdf (MDD SM4001PL–SM4007PL, Rev 2025A4, 3 pp.). Read: all.
Rejected part, kept for the record: datasheets/b5819w.pdf (JSCJ B5817W–B5819W, rev D, Mar 2015, 4 pp.).
MPN: SM4007PL   Supplier: C64898 SM4007PL (MDD(Microdiode Semiconductor), SOD-123FL): basic, JLC stock 4711963, lifecycle normal, checked 2026-10-23
Footprint: `Diode_SMD:D_SOD-123F` (pads 1.1 × 1.1 mm at ±1.4 mm, outer extent 3.9 mm). The datasheet's suggested pads (p.3) are 1.2 × 1.2 mm at ±1.6 mm (C = 3.2 mm, E = 4.4 mm). Package length including leads is 3.45–4.00 mm (p.1), so the KiCad pads (ending at ±1.95 mm) can be 0.05 mm short of the toe on a maximum-length part. **Use a project copy `D_SOD-123F_MDD` with the datasheet's 1.2 × 1.2 mm pads at ±1.6 mm.** KiCad pad 1 is the cathode (Device:D pin 1 = K). The cathode is the end with the bar on the body (p.1, "Polarity symbol marking on body"). **Courtyard:** with pads reaching x = ±2.2 mm, the project footprint's courtyard must grow to **±2.45 × ±1.15 mm**, which is ≥ 0.25 mm beyond the pads per KLC (gate-1 review finding 4; the stock D_SOD-123F's ±2.2 mm courtyard only cleared its smaller pads). The lead edits the footprint.
Symbol: `Device:D`, value `SM4007PL`.

### Why the B5819W fails
| Check | B5819W (b5819w.pdf) | Result |
|---|---|---|
| Vf at 160 mA, 40 °C | ≈ 0.32 V (p.2 forward curve: 0.34 V at 25 °C, ≈ 0.25 V at 100 °C) | OK |
| Vf at idle (≈ 15 µA total load) | Not on the curve, which stops at 0.1 A. Reverse leakage at VR = 1 V is ≈ 5 µA at 25 °C and ≈ 50 µA at 75 °C (p.2), so the saturation current is about the same as the load. Vf → a few tens of mV | **VLED ≈ 5.45 V at VBUS 5.5 V > 5.3 V max. FAIL** |
| Fix with a bleed? | Extrapolating the curve's ≈ 90 mV/decade down from 0.29 V at 0.1 A (40 °C), Vf reaches 0.2 V only at ≈ 10 mA. That is 50 mW wasted, and it still depends on extrapolation | Rejected |

`jlc_parts.py search` alternatives considered: 1N4148W (C81598, basic) has IF(AV) 150 mA (its datasheet p.1), below the 160 mA budget, so it is rejected. M7 (C95872, basic, SMA) has the same electrical behaviour as SM4007PL but a 4.3 × 2.6 mm body. **SM4007PL** is basic, in SOD-123FL, rated 1 A.

### Absolute max vs our worst case
| Parameter | Rating (p.1) | Our worst case | Margin |
|---|---|---|---|
| I(AV) | 1.0 A | 0.16 A (LED budget) + 1.1 mA bleed + U4 | 6× |
| IFSM (8.3 ms) | 25 A | Hot-plug charging of VLED caps (≈ 5.2 µF). Peak is set by cable inductance and ESR, well under 25 A (assumption: a USB cable of ≥ 0.1 Ω) | OK |
| VRRM | 1000 V | Reverse bias only if VLED > VBUS (unplug): < 5.5 V | OK |
| TJ | −55 to +150 °C | ≈ 69 °C (below) | OK |

### Calculations

**Forward drop.** Typical-curve readings (Fig. 3, p.2): 25 °C: 0.807 V at 0.1 A, 0.854 V at 0.2 A, 0.987 V at 1.0 A. 100 °C: 0.707 V at 0.1 A. So the tempco is (0.807 − 0.707)/75 = −1.33 mV/°C, and the slope in the lowest decade is 0.047 V / log10(2) = 156 mV/decade. Maximum Vf is 1.1 V at 1 A (p.1), 0.113 V above typical. The datasheet gives no minimum.

*Low end (VBUS 4.5 V, full white 160 mA, 0 °C):*
Vf typ(160 mA, 25 °C) = 0.807 + 0.156 × log10(1.6) = 0.839 V. At 0 °C: + 25 × 1.33 mV = 0.872 V. Adding the 0.113 V typ-to-max spread seen at 1 A (assumed to carry down to 160 mA): **Vf ≤ 0.985 V**.
VLED min = 4.5 − 0.985 = **3.52 V**, which is ≥ 3.3 V pixel minimum (p.3). Margin 0.22 V. Typical at 4.5 V in: 3.63 V.

*High end (VBUS 5.5 V, LEDs dark, 40 °C):*
The minimum diode current is the bleed resistor: R_VLED_BLEED = 4.7 kΩ ±1 % at VLED ≥ 4.9 V gives ≥ 1.03 mA (pixels and U4 add more; ignored). Vf typ(0.1 A, 40 °C) = 0.807 − 15 × 1.33 mV = 0.787 V. Going down two decades to 1 mA, the slope is at most 156 mV/decade. The measured slope at 0.1 A includes series-resistance drop, which only shrinks at lower current, and the junction term is ≤ 2·VT·ln10 = 124 mV/decade at 40 °C for ideality n ≤ 2 (assumption: standard PN-junction physics, not in the datasheet). So Vf typ(1 mA) ≥ 0.787 − 2 × 0.156 = 0.475 V. The datasheet gives no minimum Vf, so assume units run up to 0.1 V below the typical curve (assumption, mirroring the +0.113 V max spread): **Vf ≥ 0.375 V**.
VLED max = 5.5 − 0.375 = **5.125 V**, which is ≤ 5.3 V pixel maximum (p.3). Margin 0.175 V.
Without the bleed, the idle current is microamps (≈ 5 decades below the curve) and this bound collapses to < 0 V. **The bleed resistor is required, not optional.**

**Bleed resistor R_VLED_BLEED.** 4.7 kΩ: P = 5.125² / 4.7 k = 5.6 mW in a 0402 rated 62.5 mW (UNI-ROYAL 0402WGF series rating; assumption, no resistor datasheet downloaded). It costs about 1.1 mA from VBUS, which goes into the board's 500 mA tally (≈ 490 mA in intent.md). It also discharges VLED after unplug, so the pixels' power-on reset sees a clean ramp on re-plug.

**VLED decay constant (the one value for this board; ldo-3v3.md should reference it).** Effective VLED capacitance:
- C_VLED 4.7 µF at −50 % DC bias = 2.35 µF (assumption, no Samsung bias curve downloaded).
- Five 100 nF X7R (C_PIX1–4, C_U4) at −20 % = 0.40 µF (assumption).
- Total **C_eff ≈ 2.7 µF**.

**τ_bleed = 4.7 kΩ × 2.7 µF ≈ 13 ms.** The gate-1 review's 15 ms comes from a 3.2 µF derating guess, and ldo-3v3.md's 24 ms used the 5.2 µF nominal. 13 ms replaces both. Since D_VLED_CLAMP was added, τ_bleed only governs VLED below the zener knee (≈ 5 V). Above it the clamp discharges VLED with τ_clamp ≤ 0.16 ms (see D_VLED_CLAMP).

**Dissipation.** P = 0.16 A × 0.985 V = 0.158 W worst. RθJA = 180 °C/W (p.1, note 3; the note's "0.375 in lead length" text is copied from an axial part, so treat the figure as indicative) gives ΔT = 28 °C, so TJ ≈ 68 °C at 40 °C ambient. Limit 150 °C. OK.

**Reverse leakage.** ≤ 10 µA at 1000 V, 25 °C (p.1). The only reverse bias here is a fraction of a volt during unplug. Irrelevant.

### Fine print
- "The curve above is for reference only" (p.2): every Vf figure above is typical. That is why the margins include an assumed ±0.1 V unit spread. **Measure VLED on the first boards at idle with VBUS set to 5.5 V on a bench supply** (Open question 1).
- "Single phase half-wave 60 Hz … for capacitive load current derate by 20 %" (p.1): applies to rectifier duty. Here it is DC at 16 % of rating.

---

## D_VLED_CLAMP — BZT52B5V1 5.1 V ±2 % zener, VLED → GND (added at gate-1 review)

Datasheet: datasheets/bzt52b5v1.pdf (R+O / Zhuhai Hongjiacheng BZT52B2V4–BZT52B51, Rev 2.1, 4 pp.). Read: all.
MPN: BZT52B5V1   Supplier: C19077393 BZT52B5V1 (R+O, SOD-123): preferred, JLC stock 1182670, lifecycle normal, checked 2026-10-23
Footprint: `Diode_SMD:D_SOD-123`. Symbol: `Device:D_Zener` (pin 1 K → VLED, pin 2 A → GND). The cathode is the end with the line on the body (p.1).

**Why this zener.** The review proposed MM3Z5V1 (C19077455) or BZT52C5V1 (C173407). Both are C-grade ±5 % parts, and their maximum Vz at IZT = 5 mA exceeds the pixels' 5.3 V limit:

| Part | Vz at IZT = 5 mA, 25 °C (min / max) | Max Vz at 0 °C (tempco max) | Verdict |
|---|---|---|---|
| MM3Z5V1 (mm3z datasheet p.2) | 4.8 / 5.4 V | 5.47 V | max > 5.3 V: rejected |
| BZT52C5V1 (MDD, p.3 table) | 4.85 / 5.36 V | ≈ 5.43 V | max > 5.3 V: rejected (also extended) |
| **BZT52B5V1** (p.2) | **5.00 / 5.20 V** | **5.27 V** | max ≤ 5.3 V: **chosen** (preferred, no fee) |

The same R+O family as the MM3Z, in SOD-123 rather than SOD-323. ±2 % "B" grade. Datasheets of the two rejected parts were read but are not kept in `datasheets/`.

### Ratings (p.1–3) vs our worst case
| Parameter | Rating | Our worst case | OK |
|---|---|---|---|
| Vz at IZT = 5 mA | 5.00–5.20 V (25 °C) | — | — |
| Tempco at IZT | −2.7 to +1.2 mV/°C (p.2); Fig. 2 (p.3) ≈ +1 mV/°C typical at 5.1 V | 0–40 °C: Vz range **4.96–5.27 V** at 5 mA | yes |
| ZZT at 5 mA / ZZK at 1 mA | ≤ 60 Ω / ≤ 480 Ω | — | — |
| IR | ≤ 2 µA at VR = 2 V | — | — |
| PD | 500 mW at 25 °C, derated linearly to 0 at 150 °C (Fig. 4 p.3): 440 mW at 40 °C | ≤ 52 mW steady (below), ≈ 20 µJ per hot-plug dump | yes |
| RθJA | 357 °C/W (typical) | +19 °C at 52 mW | yes |
| Tj | 150 °C | ≈ 60 °C | yes |

### Calculations

**Where the clamp's maximum voltage matters.**
1. After a hot-plug ring, the clamp discharges C_VLED from the ring-left level (≈ 6.0–6.5 V) down to its own voltage.
2. In steady state, it backstops D2's typical-only forward curve.
In both cases the clamp current is ≤ 5 mA from the moment VLED is within a few hundred mV of Vz. Vz falls monotonically with current below IZT (Fig. 1 p.3), so VLED ≤ Vz(5 mA, max) = **5.27 V at 0 °C** and 5.22 V at 40 °C, both ≤ 5.3 V. Margin is thin at 0 °C (30 mV), but it holds at every corner.

**Hot-plug dump and time above 5.3 V.** Linear clamp model above IZT with Z ≤ ZZT = 60 Ω. Fig. 3 (p.3) shows Zz falling with current, so using 60 Ω at higher currents is conservative (assumption).
- Starting at 6.5 V with the max-Vz part, I0 = 5 mA + (6.5 − 5.27)/60 = 25.5 mA.
- VLED reaches the 5 mA point (≤ 5.27 V) after t = C_eff × Z × ln(I0 / 5 mA) = 2.7 µF × 60 Ω × ln(5.1) = **0.26 ms**, with τ_clamp ≤ 0.16 ms.
- A typical part has Zz of a few ohms at tens of mA (Fig. 3), giving tens of µs.
- Before: 2–4 ms above 5.3 V (gate-1 review finding 1).
- The only remaining excursion above 5.3 V is the ring itself (µs) and this ≤ 0.26 ms tail.
- Dump energy: excess charge ≤ 2.7 µF × 1.2 V ≈ 3.3 µC at ≈ 6 V ≈ 20 µJ. That is negligible against the transient thermal impedance (Fig. 2, MDD; R+O gives none) and the 500 mW rating.

**Steady-state leakage at VLED ≈ 5.0–5.13 V.** The clamp is in parallel with the bleed, so VLED settles where D2's drop meets the zener knee: VBUS − Vf(D2, I_bleed + I_z) = Vz(I_z). Bounding model:
- Zener at its minimum voltage (4.96 V at 40 °C), held constant at every current, with no impedance credit. This overstates the current.
- D2 on the lower-bound curve from the D2 section: 0.787 V at 0.1 A, 156 mV/decade, 40 °C, with and without the assumed −0.1 V unit spread.

| VBUS | D2 typical curve | D2 −0.1 V unit |
|---|---|---|
| ≤ 5.25 V (USB 2.0 host max) | **0 mA**: VLED settles ≤ 4.88 V, below 4.96 V. Only knee/leakage current, < 0.5 mA by Fig. 1 | 0 mA |
| 5.5 V (Type-C vSafe5V max) | **1.6 mA**, VLED 4.96 V, 8 mW | **10.4 mA**, VLED 4.96 V, 52 mW |

- Typical part, typical VBUS 5.0 V: VLED ≈ 4.5 V, so the zener is far below its knee and leakage is µA-level (IR ≤ 2 µA at 2 V, p.2; Fig. 1 p.3).
- The worst 10.4 mA only flows with all three at once: VBUS at 5.5 V, LEDs dark, and the low tolerance corners.
- **The VBUS budget is not affected.** With LEDs at full white, D2 drops ≥ 0.8 V, so VLED ≤ 4.7 V and the zener is off. Clamp current and LED current never add: total VBUS ≤ 490 − 160 + 10.4 ≈ 340 mA with LEDs dark.
- **The pixel supply minimum is not affected:** the clamp only removes charge above ≈ 4.96 V.

**Effect on the D2 high-end bound.** It used to rest on the assumed ±0.1 V D2 unit spread (5.125 V). Now it is also capped by the clamp at ≤ 5.27 V (0 °C) / 5.22 V (40 °C), for any D2 unit. That replaces an assumption with a datasheet number.

### Fine print
- **Not an ESD/TVS part.** Zener datasheets give no surge or IEC 61000-4-2 rating. The VBUS TVS (U2, usb-front-end.md) still takes the fast spike. This part only drains what D2 lets through into C_VLED.
- Fig. 1 (p.3) is pulsed at 25 °C, typical.
- MSL 1, 260 °C peak (p.1). No special handling.

---

## U4 — SN74LV1T125DBVR single buffer, 3.3 V → VLED logic

Datasheet: datasheets/sn74lv1t125.pdf (TI SCLS745A, Dec 2013 rev. Feb 2014, 21 pp. incl. addendum and DBV drawings). Read: all.
MPN: SN74LV1T125DBVR   Supplier: C473338 SN74LV1T125DBVR (TI, SOT-23-5): extended, JLC stock 161593, lifecycle normal, checked 2026-10-23. Addendum p.12: ACTIVE, MSL 1, −40 to 125 °C, marking NEZ3/NEZJ/NEZS.
Footprint: `Package_TO_SOT_SMD:SOT-23-5`. DBV0005A (p.18–19): 5 pads 1.1 × 0.6 mm, pitch 0.95 mm, row span (1.9 mm between pad-row centres per side, 2.6 mm body), 1.45 mm max height. KiCad pads are 1.325 × 0.6 mm at the same pitch, slightly longer than TI's example. OK.
Symbol: **KiCad has no 74LV1T125 symbol.** `74xGxx:SN74LVC1G125DBV` has the identical pinout: 1 ~OE, 2 A, 3 GND, 4 Y, 5 VCC, the same as LV1T125 p.1 "DCK or DBV package (top view)". Its footprint field is already `Package_TO_SOT_SMD:SOT-23-5`. Use it with value `SN74LV1T125DBVR`.

### Absolute max / recommended vs our worst case
| Parameter | Datasheet | Our worst case | Margin |
|---|---|---|---|
| VCC abs max | −0.5 to 7.0 V (p.6) | VLED ≤ 5.13 V | OK |
| VCC recommended | 1.6–5.5 V (p.6) | 3.52–5.13 V | OK |
| VI abs max | −0.5 to 7.0 V, independent of VCC (p.6) | GPIO23 ≤ IOVDD + 0.5 = 3.9 V | OK |
| VI recommended | 0–5.5 V (p.6) | 0–3.4 V | OK |
| VO | −0.5 to VCC + 0.5 (high/low); −0.5 to 4.6 V powered off (p.6) | Only the pixel DIN on it, ±1 µA | OK |
| IO | ±25 mA abs, IOH −8 mA rec. at 5 V (p.6) | ≈ 1 µA DC; edge charging of 15 pF | OK |
| Δt/Δv input edge | ≤ 20 ns/V (p.6) | RP2350 edge into ≈ 5 pF: a few ns over 3.3 V (≈ 1–2 ns/V; RP2350 slew not specified numerically, assumption) | OK |
| TA | −40 to 125 °C (p.6) | 0–40 °C | OK |

### Required external parts
- **Decoupler 100 nF X7R at VCC (pin 5) to GND (pin 3).** The datasheet shows no application circuit. This is standard CMOS practice (assumption, not a datasheet requirement) → C_U4 100 nF 16 V X7R 0402 (C1525).
- **~OE (pin 1) tied to GND.** OE is active low (function table p.3: OE = L → Y = A), and footnote (1) p.3 says "Not recommend to floating OE pin".
- **Input pull-down R_DIN_PD 100 kΩ on GPIO23 at U4.A.** The p.6 footnote says "All unused inputs … must be held at VCC or GND". Before 3V3 is up, VLED is already powered (D2 conducts as soon as VBUS appears), but GPIO23 is unpowered and floats. After reset the RP2350 pad has its pull-down on (§9.3 p.539, PADS_BANK0 GPIO23 PDE reset = 1, p.717), but not before IOVDD exists. A floating CMOS input could toggle and clock garbage into the pixels at power-up. 100 kΩ costs 33 µA when GPIO23 is high. → C25741.
- **Series resistor R_DIN 33 Ω** from U4.Y to the first pixel DIN. See calculation.

### Calculations

**VLED range** (from D2): 3.52–5.13 V.

**U4 input threshold vs RP2350 output.** U4 VIH max over −40..125 °C (p.7): 1.48 V at VCC 3.6 V, 2.03 V at 4.5–5.0 V, 2.11 V at 5.5 V. The table has no row between 3.6 and 4.5 V. Assume VIH there is ≤ the 4.5 V value (assumption: monotonic in VCC, which every tabulated row shows). Worst case is **VIH = 2.11 V**.
RP2350 VOH min = 2.62 V at IOVDD = 3.3 V at the rated drive current (rp2350-datasheet p.1336, Table 1683). U4 draws ≤ 1 µA (p.7, II), so the real VOH is higher still. **Margin ≥ 0.51 V.** OK.
U4 VIL min = 0.65 V at VCC 3.0–3.6 V over temperature, 0.8 V at 4.5–5.5 V (p.7). RP2350 VOL max = 0.5 V (p.1336). **Margin ≥ 0.15 V** (at VLED near 3.5 V). Small but positive. The RP2350's real VOL into a 1 µA load is far below 0.5 V, since the spec is at 4 mA.

**U4 output vs pixel input.** Pixel VIH min = 0.55 × VDD, VIL max = 0.7 V (ws2812b-2020-v6.pdf p.3). U4 VOH ≥ VCC − 0.1 V at IOH = −20 µA, and VOL ≤ 0.1 V at 20 µA (p.7). Pixel input current ≤ 1 µA (p.3).
- At VLED = 5.13 V: needs ≥ 2.82 V, gets ≥ 5.03 V. Margin 2.2 V.
- At VLED = 3.52 V: needs ≥ 1.94 V, gets ≥ 3.42 V. Margin 1.5 V.
- Low: 0.1 V vs 0.7 V. Margin 0.6 V.
- R_DIN drops 33 µV at 1 µA. Negligible.

**Why the buffer is needed at all:** the RP2350's 2.62 V VOH min is below the pixel's 0.55 × 5.13 = 2.82 V. The old WS2812B-2020 (V1.3) spec of VIH = 2.7 V fixed would also fail. Driving the pixel straight from GPIO23 works only by luck.

**Series resistor R_DIN.** U4's output impedance is (5.0 − 4.5) V / 8 mA ≈ 63 Ω at VCC 5 V, and (4.5 − 3.95) / 8 mA ≈ 69 Ω at 4.5 V, both over temperature (p.7). The trace to the first pixel is < 20 mm (assumption from the 21 × 51 mm board). With tpd ≈ 6–7 ps/mm, the line is electrically short for edges of ≥ 1 ns, so termination is not needed for signal integrity.
The resistor is chosen for three smaller reasons:
- It damps the LC ringing of the via/trace and the pixel's 15 pF input (p.4, CI). The pixel's VI abs min is −0.3 V (p.3), and undershoot from a fast edge should not reach it.
- It limits current into the pixel's input clamp if DIN ever sees a voltage above its VDD. That can't happen in normal use, because U4 and the pixels share VLED.
- It gives a cut point and probe point.
33 Ω × 15 pF = 0.5 ns added edge time, nothing against a 220 ns minimum T0H. → **R_DIN 33 Ω 0402 1 % (C25105).**

**Propagation delay.** tpd ≤ 8.0 ns at 3.3 V / 15 pF and ≤ 6.5 ns at 5 V / 15 pF over temperature (p.8). Rise/fall skew is not specified separately. Worst-case pulse-width distortion is therefore bounded by tpd max − tpd min ≈ 6.5 − 2.7 = 3.8 ns at 5 V and 8.0 − 4.0 = 4 ns at 3.3 V (using the typ column as a floor; assumption). That is negligible against the pixel's ±80 ns T0H window (220–380 ns) and ±210 ns T1H window (580–1000 ns) (p.5).

**Supply current.** ICC ≤ 10 µA static (p.7). ΔICC ≤ 1.5 mA with the input at 3.4 V and VCC = 5.5 V (p.7, "one input at 0.3 V or 3.4 V"). U4 draws up to ≈ 1.5 mA while GPIO23 is high. WS2812 data idles low, so the average is ≈ duty × 1.5 mA, < 1 mA. Power ≤ 5.13 V × 1.5 mA = 8 mW. θJA 206 °C/W (p.6) gives < 2 °C rise.

**Dynamic.** Cpd = 14 pF (p.8): 14 pF × 5.1² × 0.8 MHz ≈ 0.3 mW. Negligible.

### Fine print / gotchas
- **Input with VLED unpowered while 3V3 is up.** This can only happen if someone powers the board through the header 3V3 pin. Intent calls that unsupported but possible, and the LDO's reverse path is the LDO auditor's question. VI abs max is 7.0 V regardless of VCC (p.6), and the inputs are "5.0 V tolerant" (p.1). The datasheet gives no explicit Ioff/partial-power-down spec for the input. The inputs are specified to 5.5 V at VCC = 1.8 V, which implies no clamp to VCC, but that is an inference. The output tolerates −0.5 to 4.6 V powered off (p.6). Acceptable for an unsupported case.
- During normal power-up VLED rises with VBUS, before the LDO's 3V3. So U4 is powered first and its input is held low by R_DIN_PD. Good ordering.
- GPIO23 is also a header pin. Anything a user puts on it is buffered into the pixel chain, which harmlessly lights pixels at random. R_DIN_PD (100 kΩ) is a light load on the header pin. Note it on the silk or in the README: "GPIO23 drives the RGB chain".
- The datasheet's application data is thin: no layout section, no decoupling value. The 100 nF is convention.

### Layout guidance (none in datasheet; standard practice)
- C_U4 on the same side, within 1 mm of pins 5 and 3, with GND returned straight to the plane.
- Pin 1 (~OE) to GND by a short trace to pin 3's pad or a GND via.
- R_DIN at pin 4. R_DIN_PD at pin 2.
- U4 next to D3 (the first pixel), so LED_DIN is the short, fast net and the 3.3 V GPIO23 run (slow edges, set by the RP2350 pad) is the long one.

---

## D3–D6 — Worldsemi WS2812B-2020-V6 addressable RGB pixels

Datasheet: datasheets/ws2812b-2020-v6.pdf (Worldsemi WS2812B-2020-V6, 10 pp., undated). Read: all. Compared with the older datasheet WS2812B-2020 V1.3 (Adafruit 4684, 9 pp.; not in `datasheets/`, used only to note differences).
MPN: WS2812B-2020-V6   Supplier: C52917434 WS2812B-2020-V6 (Worldsemi, SMD2020-4P): extended, JLC stock 831738, lifecycle normal, checked 2026-10-23. Four per board.
Footprint: `LED_SMD:LED_WS2812B-2020_PLCC4_2.0x2.0mm`. **Verified pad by pad against p.2:**

| Item | V6 datasheet p.2 "PCB Solder Pad" | KiCad footprint | Match |
|---|---|---|---|
| Pad size | 0.7 × 0.7 mm | 0.7 × 0.7 mm | yes |
| Vertical gap between pads | 0.40 mm → centres ±0.55 mm | y = ±0.55 mm | yes |
| Horizontal inner gap | 1.13 mm → centres ±(1.13 + 0.7)/2 = ±0.915 mm | x = ±0.915 mm | yes |
| Pin numbering (top view) | DI 3 top-left, VDD 4 bottom-left, GND 2 top-right, DO 1 bottom-right (p.2 "Pin Arrangement") | 1 (−0.915, −0.55), 2 (−0.915, +0.55), 3 (+0.915, +0.55), 4 (+0.915, −0.55) | yes: the same counter-clockwise order 1→2→3→4, rotated 180°. The KiCad pin-1 chamfer lands on the DO corner |
| Polarity mark | Hatched strip on the underside, offset toward the GND/DO (pins 1, 2) side (p.2 back view and pad drawing) | Pins 1/2 on the −x side, with the chamfer at pin 1 | consistent |
| Body | 2.20 × 2.00 mm, 0.84 mm high, ±0.15 (p.2) | Fab 2.2 × 2.0 | yes |

**The V6 pinout and pads are identical to the old WS2812B-2020 V1.3** (same pin table and the same 0.7/0.40/1.13 pad drawing, old p.2). The KiCad footprint, made from V1.3, is therefore valid. The underside polarity strip is metal in the back view (p.2). Do not put vias or exposed copper under the body between the pad columns.
Symbol: `LED:WS2812B-2020` (DOUT 1, VSS 2, DIN 3, VDD 4). Its pin numbers match the V6 pin table (p.2), and its default footprint is the one above.
Height 0.84 mm ≤ 3.5 mm. OK.

### Absolute max vs our worst case
| Parameter | V6 (p.3) | Our worst case | Margin |
|---|---|---|---|
| VDD "Maximum Rated Values" | +3.3 to +5.3 V | VLED 3.52–5.13 V | +0.22 / −0.17 V |
| VI (DIN) | −0.3 to VDD + 0.7 V | 0 to VLED (U4 output on the same rail) | OK |
| Topt | −40 to +85 °C | 0–40 °C ambient + self-heating (below) | see thermal |
| Tstg | −40 to +105 °C (bulk) | — | — |

The table is titled "Maximum Rated Values" but reads like an operating range. It is treated as both: VLED must stay inside 3.3–5.3 V at all times. That drove the D2 change.

### Required external parts (from datasheet)
- **Cn = 100 nF from VDD to GND at every pixel** ("Typical Application Circuit", p.6: "Cn is the filter capacitor for the LED's VDD pin, typically set to 100 nF"). → C_PIX1…C_PIX4, 100 nF 16 V X7R 0402 (C1525). **This differs from V1.3**, which said "peripheral circuit don't need to add filter capacitor" (old p.5).
- No series resistor on DIN or DOUT in the application circuit (p.6). DOUT → next DIN is direct.
- **Bulk on VLED (my choice): C_VLED 4.7 µF 16 V X5R 0603, CL10A475KO8NNNC (C19666).** Reasoning:
  - The pixels PWM all three channels at a 2 kHz "port scanning frequency" (p.1). With all four at full white, the VLED load steps between ≈ 0 and 144 mA every PWM period (4 × 3 × 12 mA, p.1 "Operating current per channel is 12 mA").
  - The local 100 nF caps handle each pixel's own edge. The bulk holds VLED while the upstream path responds: D2's dynamic resistance is ≈ 0.47 Ω (0.047 V per 0.1 A, SM4007PL Fig. 3), and the cable inductance is unknown.
  - DC-bias derating is not in the downloaded Samsung catalogue (samsung-mlcc.pdf has ratings, no bias curves). Assume −50 % at 5 V for a 16 V 0603 X5R → ≈ 2.3 µF effective (assumption).
  - Hold-up for a 0.15 V dip at a 144 mA step: t = C·ΔV/I = 2.3 µF × 0.15 / 0.144 = 2.4 µs. That is long enough for the upstream path (cable inductance, VBUS caps through D2) to take over (assumption: cable/connector loop settles in ~1 µs).
  - The VBUS attach budget limits more than this. On this branch: 4.7 + 4 × 0.1 + 0.1 (C_U4) = **5.2 µF nominal, 5.7 µF at +10 % tolerance**, all of it seen by VBUS at attach because D2 does not limit inrush. If the VBUS tally (another auditor) exceeds 10 µF, drop C_VLED to 2.2 µF. With effective C ≈ 1.1 µF the hold-up is ≈ 1.1 µs, still acceptable.
  - 100 nF 16 V X7R 0402 at 5.1 V bias: about −20 % (assumption, no curve) → ≈ 80 nF effective. The datasheet says only "typically 100 nF".

### Calculations

**Supply range.** VLED = 3.52–5.13 V (D2 section) vs 3.3–5.3 V. OK both ends.

**Current at full white.** 12 mA per channel (p.1; LED table p.4 rates IV at 12 mA, colour mix at 36 mA). 4 × 36 = 144 mA. Intent budgets 160 mA (+10 %). Quiescent ≤ 1 µA per pixel (p.3, "no signal transmission; RGB channels are off"); V1.3 said < 0.6 mA (old p.3).

**Power and thermal.** All the supply-to-LED headroom is burnt in the pixel's constant-current sinks, so pixel power = VDD × 36 mA:
- 0.185 W per pixel at VLED 5.13 V
- 0.127 W at 3.52 V
- 0.74 W for the chain at 5.13 V.
The datasheet gives no thermal resistance, power rating or junction limit, only Topt −40..85 °C (p.3). 0.185 W in a 2 × 2 mm package with only four 0.7 mm pads to conduct heat is significant: at a guessed 300–400 °C/W pad-to-air, that is +55–75 °C (assumption, no data). **Firmware should cap brightness**, e.g. ≤ 50 % white for continuous use, until measured (Open question 2). The diode's drop helps here: it removes 0.4–1.0 V of headroom from the pixels.

**Data timing vs RP2350 PIO** (V6 p.5, identical to V1.3 p.4):
| | Spec | Example PIO setting at 150 MHz sys clk (6.67 ns resolution) |
|---|---|---|
| T0H | 220–380 ns | 300 ns |
| T0L | 580–1000 ns | 950 ns |
| T1H | 580–1000 ns | 650 ns |
| T1L | 580–1000 ns | 600 ns |
| Bit period | T0H+T0L, T1H+T1L ≥ 1.25 µs | 1.25 µs (800 kb/s) |
| RES | ≥ 280 µs low | firmware: ≥ 300 µs |

All of these are achievable: 1.25 µs is ~188 PIO cycles at 150 MHz, or set the clock divider to give whole-cycle steps. **Caution:** **T1L has a 580 ns minimum in this datasheet.** A ws2812 PIO program written for the classic 5050 WS2812B timing can drive T1L as short as ≈ 375 ns, which is out of spec here. Firmware must use timings inside all four windows (Open question 3). FMAX min 600 kb/s and Fosc 800 kHz typ (p.3–4). The U4 delay above is negligible.
Data order is GRB, MSB first (p.5). Each pixel strips 24 bits and reshapes the rest (p.1, p.5). DIN→DOUT tPLZ ≤ 300 ns (p.3), so four pixels add ≤ 1.2 µs latency. Irrelevant.

**Logic levels.** Covered under U4: VIH 0.55 × VDD, VIL 0.7 V, II ±1 µA, CI 15 pF (p.3–4). Between pixels, DOUT and DIN share VLED, so levels are inherently compatible.

**Reset / power-on.** "Built-in power-on reset and power-off reset circuits" (p.1). The datasheet does not say what colour a pixel shows after power-on reset. Expect dark, but firmware should send a frame of zeros at boot. R_DIN_PD keeps DIN low while the RP2350 is unpowered, so no data is clocked in at plug-in. R_VLED_BLEED discharges VLED in ~13 ms after unplug, so a quick re-plug still gives the POR a ramp from near 0 V.

**Reverse-polarity claim.** "Resistant to reverse polarity connection without damage" (p.1; V1.3 says the same). No voltage, current or duration is given, so the claim gets **no design credit**. On this board polarity is fixed by the footprint. The claim only means that a pixel rotated 180° in assembly might survive, not that it works. Check the CPL rotation for C52917434 at JLC before ordering (Open question 4).

### Fine print / gotchas
- **MSL 5a** (p.8): 24 h floor life after opening. Bake at 70–75 °C ≥ 24 h before SMT (p.9, 2.3.1). JLC handles MSL parts, but this is a reason to place the LEDs in one assembly pass with no rework.
- Reflow peak 245 °C, ≤ 6 min room-to-peak, TAL 45–90 s (p.9). These are tighter than a typical 260 °C profile. JLC's standard profile is the assembler's to confirm (Open question 4).
- Do not press on the lens; do not clean with solvents or ultrasound (p.8, p.10). Keep the buttons away from the LEDs so fingers don't land on them.
- **Differences V1.3 → V6:**
  - VDD min 3.7 → 3.3 V.
  - VIH 2.7 V fixed → 0.55 × VDD.
  - Topt min −25 → −40 °C.
  - Quiescent < 0.6 mA → ≤ 1 µA.
  - The 100 nF per pixel is now in the application circuit.
  - Luminous intensity bins changed (V6 p.4).
  - Pinout, pads, timing and tPLZ are unchanged.
  - With VDD min 3.3 V, the D2 drop at 4.5 V in is fine. V1.3's 3.7 V min would have failed at 3.52 V, so **do not substitute a V1.3-spec WS2812B-2020 (e.g. C965555)** without revisiting D2.

### Layout guidance (from datasheet + practice)
- C_PIX at each pixel's VDD (pin 4) / GND (pin 2) pads, ≤ 1 mm away (p.6 places Cn at each LED).
- No vias or copper under the body's centre strip (p.2 metal polarity mark).
- Chain order D3 → D4 → D5 → D6. Short DOUT→DIN hops on the top layer.
- Feed VLED as a pour or a ≥ 0.4 mm trace. 160 mA through 30 mm of 0.3 mm 1 oz copper drops ≈ 8 mV, so it is not critical.
- Give each GND pad as much F.Cu as the pitch allows, with ≥ 2 vias to the GND plane. It is the main heat path.
- Spread the four pixels at least 5 mm apart (assumption: better than clustering for heat). Keep them on the top, visible, away from the buttons.
- D2 and C_VLED between the VBUS entry and the pixel group.
- D_VLED_CLAMP next to R_VLED_BLEED and C_VLED, with a short, wide GND return (≥ 2 vias). It carries the hot-plug dump current (tens to hundreds of mA for < 0.3 ms).

---

## Schematic connections

| Ref (role) | Pin | Net |
|---|---|---|
| D2 SM4007PL | 2 (A) | VBUS |
| | 1 (K) | VLED |
| U4 SN74LV1T125DBVR | 1 ~OE | GND |
| | 2 A | GPIO23 |
| | 3 GND | GND |
| | 4 Y | LED_DIN_BUF |
| | 5 VCC | VLED |
| D3 WS2812B-2020-V6 (pixel 1) | 3 DIN | LED_DIN |
| | 1 DOUT | PIX12 |
| | 4 VDD | VLED |
| | 2 GND | GND |
| D4 (pixel 2) | 3 DIN / 1 DOUT / 4 VDD / 2 GND | PIX12 / PIX23 / VLED / GND |
| D5 (pixel 3) | 3 DIN / 1 DOUT / 4 VDD / 2 GND | PIX23 / PIX34 / VLED / GND |
| D6 (pixel 4) | 3 DIN / 1 DOUT / 4 VDD / 2 GND | PIX34 / no-connect / VLED / GND |

| Function name | Between | Value | Rating / dielectric | LCSC | Footprint |
|---|---|---|---|---|---|
| R_VLED_BLEED | VLED – GND | 4.7 kΩ 1 % | 1/16 W (5.6 mW used) | C25900 (basic) | Resistor_SMD:R_0402_1005Metric |
| D_VLED_CLAMP | K → VLED, A → GND | BZT52B5V1, 5.1 V ±2 % zener | 500 mW (≤ 52 mW steady) | C19077393 (preferred) | Diode_SMD:D_SOD-123 |
| C_VLED | VLED – GND | 4.7 µF ±10 % | 16 V X5R (≈ 2.3 µF at 5 V, assumed) | C19666 (basic) | Capacitor_SMD:C_0603_1608Metric |
| C_U4 | VLED – GND at U4 pin 5 | 100 nF ±10 % | 16 V X7R | C1525 (basic) | Capacitor_SMD:C_0402_1005Metric |
| R_DIN_PD | GPIO23 – GND at U4 pin 2 | 100 kΩ 1 % | 1/16 W | C25741 (basic) | Resistor_SMD:R_0402_1005Metric |
| R_DIN | LED_DIN_BUF – LED_DIN | 33 Ω 5 % | 1/16 W | C25105 (basic) | Resistor_SMD:R_0402_1005Metric |
| C_PIX1…C_PIX4 | VLED – GND at each pixel | 100 nF ±10 % | 16 V X7R | C1525 (basic) | Capacitor_SMD:C_0402_1005Metric |

Extended parts in this block: U4 (C473338) and D3–D6 (C52917434, one line item). D2 is basic; D_VLED_CLAMP is a JLC "preferred" part (no extended fee).

## Open questions

1. **Measure VLED** on the first board at idle (LEDs dark, GPIO23 low) with VBUS from a bench supply at 5.50 V, and at 4.50 V with full white. The D2 margins (+0.175 V high, +0.22 V low) rest on a typical-only forward curve and an assumed ±0.1 V unit spread.
2. **Pixel thermal**: no θ or power rating in the V6 datasheet. Measure the pixel case temperature at full white for 10 min at 40 °C ambient (or 25 °C + 15). Until then, firmware limits continuous full white.
3. **Firmware timing**: confirm the PIO program's T1L ≥ 580 ns (and the other three windows). The common 5050 timing may not meet it.
4. **JLC assembly**: CPL rotation for C52917434 (pin 1 = DO corner) and for SM4007PL (cathode bar), MSL 5a handling, and the 245 °C peak.
5. ~~VBUS hot-plug overshoot~~: resolved by D_VLED_CLAMP (BZT52B5V1). Time above 5.3 V drops from 2–4 ms to ≤ 0.26 ms after the µs-scale ring. Still scope VBUS and VLED at attach on the first board (A-to-C cable, live source), as ldo-3v3.md also asks.
6. ~~VBUS capacitance tally~~: resolved. audit-power's total is 6.3 µF nominal including this block's 5.2 µF; attach charge ≤ 35 µC vs the 55 µC limit (ldo-3v3.md). C_VLED stays 4.7 µF.

## Findings

- **MAJOR (gate-1 review finding 1) — hot-plug left VLED above the pixels' 5.3 V rating for 2–4 ms. RESOLVED.**
  - **Cause:** D2 and C_VLED peak-detect the VBUS ring. VLED is left at ≈ 6.0–6.5 V and decayed only through the bleed (τ_bleed ≈ 13 ms).
  - **Fix chosen by the lead:** D_VLED_CLAMP, a BZT52B5V1 (C19077393), across VLED.
    - VLED is back under 5.3 V within ≤ 0.26 ms of the ring.
    - Worst-case Vz ≤ 5.27 V at ≤ 5 mA across 0–40 °C.
    - Steady draw: 0 mA at VBUS ≤ 5.25 V, ≤ 10.4 mA at 5.5 V with LEDs dark.
  - **C-grade parts rejected:** the reviewer's MM3Z5V1 and BZT52C5V1 reach 5.36–5.47 V at 5 mA.
  - **Alternative not taken:** a 5.0 V LDO for VLED (AP2210N-5.0, C460341, extended) was evaluated. It would regulate VLED outright, at the cost of an extended part and fee.
- **MINOR — D2 margins rest on a typical-only forward curve** (Open question 1). The high end is now also capped by D_VLED_CLAMP (≤ 5.27 V); the low end (3.52 V at VBUS 4.5 V, full white) still rests on the curve.
- **MINOR (review finding 4) — project footprint `D_SOD-123F_MDD` courtyard** must be ±2.45 × ±1.15 mm (noted in the D2 section; the lead edits the footprint).
- **MINOR (review finding 5) — one VLED decay constant:** τ_bleed = 4.7 kΩ × 2.7 µF ≈ 13 ms (D2 section), plus τ_clamp ≤ 0.16 ms above the zener knee. ldo-3v3.md should reference these.
- **NOTE — U4 level shifter, pixel footprint and decoupling verified.** No change needed.
