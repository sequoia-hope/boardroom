# Phase 8 — signal-integrity / EMC review: easy2350 rev a

Independent judgment pass (Claude, 2026-10-23) on `hw/easy2350/a/easy2350.kicad_pcb`
against `references/simulation.md`, `design/nets.csv`, `stackup.md`, `intent.md`,
`parts/*.md` and `decisions.md`. No circuit simulator was used (no ngspice in this
environment); every number below is a measurement of the board file through
`pcbnew` or a closed-form estimate whose formula and inputs are stated. Scripts:
`sim/scripts/` is not part of the repo; the measurement code lived in the session
scratchpad and is reproducible from the method notes in each table.

Stackup used for every estimate (stackup.md): L1 1 oz over 0.21 mm 7628 prepreg
(εr 4.4) to L2 GND; L4 1 oz over 0.21 mm to L3 3V3; 1.07 mm core between L2 and L3.
So an L1 trace is referenced to GND at 0.21 mm; an L4 trace is referenced to the
**3V3 plane** at 0.21 mm and sees GND only at 1.28 mm.

## 1. Trigger list (from nets.csv)

| Trigger (simulation.md) | Nets | Threshold | Applies? |
|---|---|---|---|
| DIFF_HS over length threshold | USB_DP_C/DM_C (20 mm), USB_DP/DM (20 mm), USB_DP_MCU/DM_MCU (3 mm) | per-net limits in nets.csv | Yes: USB_DP_MCU 4.10 and USB_DM_MCU 3.82 mm exceed their 3 mm limit (§3.1); others inside |
| SE_HS over length threshold | QSPI_* (25 mm) | 25 mm | No net over; longest 18.4 mm (§3.2) |
| CLOCK over length threshold / no vias | XIN, XOUT, XOUT_R (5 mm, "no vias") | 5 mm | **Yes: XIN 8.65 mm with 2 vias, 6 mm of it on L4** (§3.3) |
| SWITCH near ANALOG / CLOCK | VREG_LX vs XIN/XOUT/XOUT_R, VREG_AVDD, GPIO26–29 | 3 × trace height ≈ 0.63 mm | Checked (§4); LX is a 2.3 mm² zone with no tracks |
| POWER with drop budget | VBUS (500 mA, 4.5 V min at J1), +3V3 (±3 %, VREG_AVDD 58 mV margin), VLED (pixel 3.3 V min), +1V1 | IR drop | Estimated (§6) |
| Switching converter | RP2350 core regulator | vendor copper, L2 cut-out | Layout checked (§5); no SPICE model exists for the regulator IP |
| Thermal hot spot | U3 LDO 0.61 W | spreadsheet | Estimated (§7) |
| Hot-plug | VBUS LC ring vs U3 6.5 V abs max | closed form | Restated from ldo-3v3.md with this board's numbers (§8) |
| Added by this review | EMC of the pixel data/PWM, USB-C shield and ESD path, return-path vias at layer changes, crosstalk QSPI→USB, pixel data→RUN | — | §9 |

## 2. Method

- Lengths are shortest pad-to-pad paths through the track graph (not total copper
  of the net, which includes stubs such as the second USB-C contact and the crystal
  load-cap spurs). Vias are counted on that path.
- Aggressor/victim: for every segment pair on the same layer (or L1/L2, L4/L3), the
  minimum edge-to-edge gap, and the aggressor length that lies within 0.6 mm
  (≈ 3 h) and within 1.0 mm of the victim.
- Plane continuity: each HS/CLOCK track sampled every 0.05 mm; a sample counts as a
  gap if the filled L2 GND polygon (L1 tracks) or L3 3V3 polygon (L4 tracks) does not
  contain it, excluding samples within 0.55 mm of a via or through-hole (their own
  antipads).
- Impedance: Hammerstad–Jensen microstrip with 35 µm copper, εr 4.4; differential
  Zdiff ≈ 2 Z0 (1 − 0.48 e^(−0.96 s/h)).
- Crosstalk: saturated near-end coefficient Kb ≈ 1/(1 + (D/h)²) (D centre-to-centre),
  scaled by 2·L·tpd/tr when the coupled length is shorter than the edge
  (tpd ≈ 5.8 ps/mm). Lumped check: Cm × ΔV / C_victim for short runs. Edge rates:
  RP2350 GPIO/QSPI 1–2 ns (fast slew; the pads reset to slow slew, 2–4 ns), USB FS
  4–20 ns, WS2812/74LV1T125 outputs ≈ 2–3 ns on 5 V.

## 3. Length, layer, via and reference-plane table

### 3.1 USB (DIFF_HS, 90 Ω target, FS 12 Mb/s)

| Net | From → to | Path length | Layers | Vias | Width | Limit | L2 GND gap under L1 part | Verdict |
|---|---|---|---|---|---|---|---|---|
| USB_DP_C | J1.A6 → U2.3 (J1.B6 → U2.3) | **12.06 mm** (11.20) | L1 | 0 | 0.15/0.20 | 20 | none | inside limit, but see F-3: it detours left to x ≈ 105.5 past the flash pins |
| USB_DM_C | J1.A7 → U2.1 (J1.B7 → U2.1) | 5.80 mm (2.91) | L1 | 0 | 0.15/0.20 | 20 | none | OK |
| USB_DP | U2.4 → R5.1 | 10.00 mm | L1 1.7 + L4 8.3 | 2 | 0.20 | 20 | none | OK |
| USB_DM | U2.6 → R4.1 | 10.24 mm | L1 2.3 + L4 8.1 | 2 | 0.20 | 20 | none | OK |
| USB_DP_MCU | R5.2 → U1.52 | 4.10 mm | L1 | 0 | **0.15** | 3 | none | 1.1 mm over the 3 mm limit; electrically nil (see below) |
| USB_DM_MCU | R4.2 → U1.51 | 3.82 mm | L1 | 0 | **0.15** | 3 | none | 0.8 mm over |

Intra-pair skew: connector side 6.3 mm (A-pins) / 8.3 mm (B-pins); whole path
J1→U1: D+ 26.2 mm, D− 19.9 mm → 6.3 mm ≈ 37 ps. Against a ≥ 4 ns FS edge that is
< 1 % of the edge; no effect on eye or EOP timing. The pairs are not routed as
coupled pairs anywhere (gap ≫ 0.15 mm except at U2), so the class's "90 Ω diff" is
really two single-ended lines:

| Geometry | Z0 single | Zdiff (if coupled at 0.15 mm) |
|---|---|---|
| 0.20 mm over 0.21 mm | 70 Ω | 106 Ω |
| 0.15 mm over 0.21 mm (MCU segments, CLOCK) | 79 Ω | — |
| 0.10 mm (SE_HS, SIGNAL) | 92 Ω | — |

So the stackup.md estimate of "≈ 90 Ω differential" for 0.20/0.15 is low; the
built geometry is ≈ 106 Ω coupled, ≈ 70 Ω single. For FS with a 26 mm run
(electrical length 150 ps against 4–20 ns edges, 1/27 of the shortest edge) the
line is lumped and impedance does not matter, which is what intent.md says.
**Waived** for impedance; **not** waived for the USB_DP_C route (F-3).

### 3.2 QSPI (SE_HS, ≤ 25 mm, 75 MHz SDK default, 133 MHz max)

| Net | U1 → U5 | Path | Layers | Vias | L2/L3 gap | Verdict |
|---|---|---|---|---|---|---|
| QSPI_SCLK | 56 → 6 | 16.64 mm | L1 6.2 + L4 10.7 | 2 | none | OK |
| QSPI_SD0 | 57 → 5 | 18.43 mm | L1 4.5 + L4 14.2 | 2 | none | OK |
| QSPI_SD1 | 59 → 2 | 7.35 mm | L1 3.5 + L4 4.3 | 2 | none | OK |
| QSPI_SD2 | 58 → 3 | 6.83 mm | L1 2.4 + L4 4.7 | 2 | none | OK |
| QSPI_SD3 | 55 → 7 | 16.73 mm | L1 | 0 | none | OK |
| QSPI_SS | 60 → 1 | 7.95 mm (+4.3 mm to R7) | L1 | 0 | none | OK |

Bus skew 11.6 mm ≈ 67 ps against a 13.3 ns period (75 MHz) or 7.5 ns (133 MHz):
negligible. Longest line 18.4 mm = 108 ps one way; with the pads at their reset
slow-slew setting (2–4 ns) the lines are lumped; at fast slew (≈ 1 ns) the
round-trip 216 ps is still < tr/4, so no termination is needed. Z0 ≈ 92 Ω
(L1) / similar on L4 over the 3V3 plane; the RP2350 4 mA pad drive (≈ 50–80 Ω)
under-damps slightly — one small overshoot, inside the flash's −0.6/+VCC+0.4 V
(and −2/+2 V for < 20 ns). **Passes.**

### 3.3 Crystal (CLOCK, ≤ 5 mm, no vias, L2 solid)

| Net | From → to | Path | Layers | Vias | Width | L2 gap | Verdict |
|---|---|---|---|---|---|---|---|
| **XIN** | U1.21 → Y1.1 | **8.65 mm** | L1 2.6 + **L4 6.0** | **2** | 0.15 | none (but 6 mm is over L3, not L2) | **FAIL F-1** |
| XIN spur | Y1.1 → C20 | 2.13 mm | L1 | 0 | 0.15 | — | OK |
| XOUT | U1.22 → R6.2 | 4.76 mm | L1 | 0 | 0.15, necked to **0.09–0.11** for 1.5 mm | none | length OK; width below the 0.15 class (F-9) |
| XOUT_R | R6.1 → Y1.3 | 3.38 mm | L1 | 0 | 0.15 (0.11 at the pad) | none | OK |
| XOUT_R spur | Y1.3 → C21 | 4.66 mm | L1 | 0 | 0.15 | none | long for a load-cap spur; adds ≈ 0.3 pF to the XOUT side (see below) |

Stray capacitance added by the copper (tpd/Z0 ≈ 0.072 pF/mm for 0.15 mm on
0.21 mm; two vias ≈ 0.3 pF each): XIN side ≈ 0.62 + 0.6 (vias) + 0.15 (spur) ≈
1.4 pF; XOUT side ≈ 0.34 + 0.24 + 0.34 ≈ 0.9 pF. Both inside HDG's 3 pF stray
allowance; the 0.5 pF asymmetry is harmless to a 10 pF CL Pierce circuit. The
problem with XIN is not its capacitance but (a) the two vias and 6 mm of L4 run
breaking the design's own rule and the chip note ("keep layer 2 solid under the
XIN/XOUT traces", rp2350-core.md Calc 5), and (b) the neighbour it picks up on
L4: PIX12 (§4).

### 3.4 Pixel data and the other long signals

| Net | Path | Layers | Vias | Note |
|---|---|---|---|---|
| GPIO23 | U1.35 → U4.2 | 26.7 mm | L1 6 + L4 30 (net total 36 mm incl. header spur) | 3 | 3.3 V, 800 kb/s |
| LED_DIN | R10.2 → D3.DIN | 12.64 mm | L1 4.8 + L4 7.9 | 2 | 5 V logic behind 33 Ω |
| **PIX12** | D3.DOUT → D4.DIN | **25.30 mm** | L1 1.5 + **L4 23.8** | 2 | pixels are 3.7 mm apart; the trace goes 10 mm up under U1 and back (F-2) |
| PIX23 | D4 → D5 | 6.69 mm | L1 | 0 | OK |
| PIX34 | D5 → D6 | 5.99 mm | L1 | 0 | OK |

## 4. Aggressor / victim results

Pairs with any segment closer than 1.5 mm on the same or adjacent layer. Gap is
edge-to-edge; "par" is aggressor length within 0.6 mm / 1.0 mm of the victim.

| Aggressor → victim | Layer | Min gap | par < 0.6 | par < 1.0 | Estimate | Disposition |
|---|---|---|---|---|---|---|
| **PIX12 → XIN** | L4/L4 (ref. 3V3 plane) | **0.33 mm** | 1.6 mm | **8.8 mm** | NEXT ≈ 10 mV for a 5 V, 3 ns edge (lumped: Cm ≈ 0.04 pF into ≈ 19 pF); every LED refresh | **Fix (F-1/F-2).** 10 mV on a ≈ 0.5–1 Vpp Pierce input is ≈ 1–2 % amplitude modulation at 800 kHz: measurable as reference-clock jitter during pixel updates, not an outage. It violates the CLOCK guard rule, and the fix is free |
| **QSPI_SCLK → USB_DP_C** | L1/L1 | **0.30 mm** | 4.1 mm | 6.0 mm | ≈ 28 mV onto D+ at a 1 ns SCLK edge; FS receiver thresholds ±200 mV diff, 0.8/2.0 V SE | Signal OK; the ESD direction matters (F-3) |
| QSPI_SCLK → USB_DP_MCU | L1/L1 | 0.73 | 0 | 3.0 | < 10 mV | waived |
| QSPI_SD3 → USB_DP_MCU | L1/L1 | 0.51 | 2.3 | 4.4 | ≈ 9 mV | waived |
| QSPI_SD0 → QSPI_SCLK | L1/L1 | **0.12–0.14** | 17.3 mm | 20.8 mm | Kb(sat) 0.43 × 2·17 mm·5.8 ps / tr: ≈ 290 mV at tr 1 ns, ≈ 145 mV at 2 ns, ≈ 70 mV at 4 ns (reset slew) | MINOR F-6; flash VIH/VIL margins ≈ 1 V, and the SCLK edge lands mid-bit for read data |
| QSPI_SD3 → QSPI_SCLK | L1/L1 | 0.12 | 7.9 | 12.1 | ≈ half of the above | same |
| QSPI_SS → QSPI_SCLK | L1/L1 | 0.32 | 1.5 | 2.6 | ≈ 15 mV | waived (SS is static during transfers) |
| **GPIO23 → RUN** | L4/L4 | **0.38 mm** | 3.7 | 8.6 | NEXT ≈ 15 mV; lumped ≈ 36 mV (Cm ≈ 0.08 pF into ≈ 7 pF) | NOTE F-8. RUN is a Schmitt input held at 3.3 V by its pull-up: a 36 mV dip is 1/30 of the hysteresis |
| GPIO23 → GPIO25 | L1+L4 | 0.58 | — | 12.6 | ≈ 12 mV | waived (LED) |
| GPIO23 → SWDIO | L4/L4 | 0.36–0.99 | 0.7 | 20.7 | ≈ 10–20 mV | waived (SWD is 1–10 MHz, 3.3 V CMOS, debug only) |
| VREG_LX zone → victims | L1 | XIN 7.1, XOUT 6.9, XOUT_R 9.8, GPIO29 1.7, GPIO26 2.7, USB_DM_MCU 1.0, USB_DP_MCU 1.4, QSPI_SD3 2.6 mm | — | — | all ≥ 1 mm except the pins that are physically adjacent on the QFN (VREG_AVDD 0.6 mm, 1V1 at C15); same as the vendor layout | **Waived**: the LX zone is the vendor copper (2.27 mm²), there are no LX tracks, and the crystal is 7 mm away with solid L2 between |
| Anything → GPIO26–29 (ADC) | — | nearest aggressor 1.49 mm (GPIO23) | 0 | 0 | — | nothing to waive; intent.md "don't care" on ADC anyway |
| Anything → VREG_AVDD | L1 | 3.5 mm trace, LX zone 0.6 mm away (pin pitch) | — | — | filtered by 33 Ω / 4.7 µF (−60 dB at 3 MHz, rp2350-core.md) | waived |

Plane continuity: every USB, QSPI, CLOCK, pixel-data and SWD segment on L1 has
filled L2 GND beneath it; every L4 segment has filled L3 3V3 beneath it (one
0.55 mm exception on GPIO23 at the J3 header antipad). The only L2 opening is the
rule area under VREG_LX, 112.05–113.85 × 118.15–123.65 mm, which contains the LX
zone (112.35–113.55 × 118.45–123.35) and L1 pad 2 at (113.20, 119.30) with
0.3 mm to spare and nothing else: no HS/CLOCK track crosses it.

## 5. Core regulator (RP2350 §6.3.8)

- VREG_LX: zone only (no tracks), 2.27 mm², from pin 48 to L1 pad 2. The L2 cut-out
  covers it. L3 (3V3) is the next plane at 1.28 mm; the datasheet asks only for the
  adjacent plane to be cut.
- Under the cut-out on L4 runs the **VBUS** trace (x = 113.04, y 113.2–123.2) on its
  way to D2. It is on the far side of the solid L3 plane, so it is shielded from LX
  and does not re-create the capacitance the cut-out removes. NOTE only.
- 1V1 spokes: 9 vias, vendor geometry; 1V1 copper 26 mm total, ≥ 0.15 mm.
  VREG_AVDD: 3.37 mm to R3, 2.51 mm to C13 — inside the 10 mm limit.
- No SPICE: the regulator IP has no published model; the vendor copper is the
  validation. **Waived with that reasoning**, as `chips/rp2350.md` instructs.

## 6. DC drop / current density (copper only; connector and cable excluded)

| Path | Length / width / layer | R | I | Drop | Verdict |
|---|---|---|---|---|---|
| VBUS J1.A4 → U3.8 (LDO in) | 5.0 mm, 0.25 mm, L1 | 9.8 mΩ | 0.33 A (3V3 side + bleed) | 3 mV | OK |
| VBUS J1.A4 → U2.5 | 5.8 mm | 11 mΩ | — | — | OK |
| VBUS J1.A4 → D2.2 (LED rail) | **42.3 mm**, 0.25 mm, L1+L4, 2 vias | 86 mΩ | 0.16 A | **14 mV** | OK; VLED budget has 0.2 V to spare at 4.5 V in (rgb-leds.md) |
| VBUS J1.A4 → J3.20 (header) | 10.7 mm, 1 via | 23 mΩ | 0.05 A | 1 mV | OK |
| VLED D2.1 → farthest pixel | ≈ 16 mm, 0.19–0.25 mm | ≈ 40 mΩ | 36 mA (last pixel), 160 mA for the first 6 mm | ≈ 2 + 2 mV | OK |
| +3V3 U3.1 → C3 → via (114.7,116.45) → L3 plane → vias (106.5,123.2), (114.1,123.5) → U1 zone | 1.6 mm trace + 1 via + ≈ 1 square of 0.5 oz plane (1 mΩ/sq) + 2 vias | ≈ 3 + 1 + 1 + 1 mΩ | 0.28 A | ≈ 2 mV | OK; VREG_AVDD's 58 mV margin (rp2350-core.md) survives |
| Current density | 0.25 mm × 35 µm at 0.33 A = 38 A/mm² | — | — | ≈ 10 °C rise by IPC-2152 (stackup.md derivation) | OK |

All three LDO-rail paths funnel through **one** 0.3 mm via at (114.7, 116.45):
≈ 1 mΩ and well under its current rating, but a single point (F-10, NOTE).

## 7. LDO thermal (U3 AP7361C, U-DFN3030-8)

P = (5.5 − 3.3) V × 0.277 A ≈ 0.61 W worst case (intent.md).

| θJA assumption | TJ at 40 °C ambient |
|---|---|
| 70 °C/W (datasheet: 2" × 2", vias to a bottom plane) | 83 °C |
| 90 °C/W (this board: 21 × 51 mm, 3 EP vias to a 0.5 oz L2, no extra vias) | 95 °C |
| 110 °C/W | 107 °C |
| 139 °C/W | 125 °C design limit |

Board as built: the footprint's 3 EP vias (0.55 mm pads, 0.25 drill) are the only
GND vias within 4.5 mm of U3; the top GND copper within ±4.5 mm is 38 mm². ldo-3v3.md
asked for ≥ 50 mm² and ≥ 4 more vias; neither is met (F-5). With the planes the
board still spreads heat well (a 4-layer 10 cm² board is roughly 60–90 °C/W for a
3 × 3 DFN), so the estimate is 90–110 °C/W → TJ ≈ 95–107 °C at the simultaneous
worst case, 15–30 °C under the 125 °C design limit and 40–55 °C under the typical
thermal shutdown. Typical use (< 0.2 W) is far below. **Passes with margin that
depends on an assumption**: add the vias (cheap) and keep the bring-up thermal
measurement ldo-3v3.md already calls for.

## 8. VBUS hot-plug ring (restated for this board)

Direct VBUS capacitance 2.3 µF (C2 2.2 µF + C1 0.1 µF; the 5.2 µF LED rail sits
behind D2). With 0.5–1 µH of cable: Z0 = 0.47–0.66 Ω, f = 105–150 kHz. Undamped
peak 2 × 5.5 = 11 V; a 0.25–0.5 Ω cable gives ζ ≈ 0.2–0.5 → 6.5–8 V, and U2's VBUS
TVS (VBR ≥ 6 V) clamps the rest. U3's 6.5 V abs max is grazed for microseconds, as
ldo-3v3.md already records (MINOR there); the LED rail is protected by D2 + D7.
Nothing in the layout changes that analysis (VBUS trace inductance 5 mm ≈ 4 nH is
irrelevant). **Waived to ldo-3v3.md's bring-up test** (5.5 V stiff source,
scope on VBUS at C2).

## 9. Items the triggers miss

**ESD path and the USB-C shield.** J1's four shell legs are plated slots on GND
(all layers); shield → GND direct, the best choice for a device that gets
plugged often. U2's GND pin 2 reaches the L2 plane through the top pour: nearest
GND via/through-hole **3.2 mm** away; C1 (ESD-return 100 nF) 2.8 mm. usb-front-end.md
asked for two vias at U2's GND and one at C1: ≈ 3 mm of pour is 1–2 nH extra in the
8 kV return, i.e. 30–60 V of extra L·di/dt at 30 A/ns on top of the clamp — still
far below what the 27 Ω + RP2350 pad clamp absorb, but free to fix (F-4).
The larger ESD issue is geometric: **USB_DP_C runs 12 mm before it reaches the
clamp**, and 4 mm of that is 0.3 mm from QSPI_SCLK and passes the flash's pin row.
A contact discharge on D+ puts the full un-clamped edge (tens of amps per ns) on
that trace; capacitive and inductive coupling into SCLK at 0.3 mm over 4 mm is on
the order of volts for a nanosecond — enough to clock a spurious bit into the
flash during XIP and crash the running program (no damage, a reset recovers).
D− has no such exposure (2.9–5.8 mm, direct). **F-3.**

**Pixel data / PWM EMC.** The WS2812 PWM (≈ kHz, 36 mA steps per pixel) is local:
each pixel has 100 nF, VLED has 4.7 µF, and the VLED/GND loop is a few mm over
solid L2. The 800 kb/s data at 5 V with ≈ 2–3 ns edges has harmonics into the
hundreds of MHz; the radiating lengths are LED_DIN 12.6 mm (behind 33 Ω — good)
and **PIX12 25.3 mm with 24 mm on L4 running up under the RP2350 and back** for a
3.7 mm pixel pitch. That loop is also what couples into XIN. Re-route it along the
pixel row like PIX23/PIX34 (6–7 mm) and the net is a non-issue (F-2). Pixel DOUT
has no series resistor between pixels; the WS2812B's own driver is slow enough
(datasheet 15 pF CI, tpd 300 ns class) that 6 mm lines need none.

**Return-path vias at layer changes.** L1 references GND, L4 references 3V3, so
every L1↔L4 via moves the return current between planes. Nearest GND via or
through-hole to each signal via: USB 1.2–2.2 mm (one at 4.6 mm, U2.6), QSPI
0.9–4.1 mm, XIN 2.4–2.9 mm, pixel data 0.8–3.6 mm. For FS USB and slow-slew QSPI
the plane-to-plane capacitance plus the eleven 100 nF decouplers around U1 (11 GND
vias within 6 mm) close the loop acceptably; at fast-slew QSPI the 4 mm cases
(SCLK/SD0 vias at U1) add ≈ 2–3 nH to the return. NOTE F-7: a GND via beside the
SCLK/SD0 via pair at (106.1–106.9, 121.3–121.8) costs nothing.

**USB_DP_MCU / USB_DM_MCU at 0.15 mm, 4 mm.** The class says 0.20 mm; the QFN
escape necks them. Z0 79 vs 70 Ω over 4 mm (25 ps): invisible at FS. Waived.

**CC1/CC2, SWD, RUN to the header, GPIO to headers:** slow or static; nothing to
check beyond the GPIO23→RUN coupling above.

## 10. Findings

| # | Severity | Finding | Evidence | Suggested fix |
|---|---|---|---|---|
| F-1 | **MAJOR** | XIN routed 8.65 mm with 2 vias and 6 mm on L4 (over the 3V3 plane), against nets.csv (≤ 5 mm, no vias) and rp2350-core.md Calc 5 / chips/rp2350.md (L2 solid under the crystal traces). On L4 it shares 8.8 mm within 1 mm (0.33 mm min) with PIX12 | §3.3, §4 | Route XIN on L1 from pin 21 straight down to Y1.1 (7.4 mm pad-to-pad; the vendor spacing is 5.4 mm — move Y1/C20/C21 1–2 mm up if C6/R6 block the path, or pass between C6 and R6). Zero vias. If it must stay on L4, fix F-2 first and add GND vias at both XIN vias |
| F-2 | **MAJOR** | PIX12 is 25.3 mm (24 on L4) for a 3.7 mm pixel pitch, running up under U1 and back; it is the aggressor on XIN and the longest 5 V fast-edge radiator on the board | §3.4, §4 | Re-route D3.DOUT → D4.DIN along the pixel row on L1 or L4 (≤ 8 mm), as PIX23/PIX34 are. This alone removes the XIN coupling even before F-1 |
| F-3 | **MAJOR** | USB_DP_C (un-clamped, connector side of U2) is 12.1 mm for a 4.4 mm pad distance, loops left to x ≈ 105.5 past the flash pins and runs 4 mm at 0.30 mm from QSPI_SCLK. ESD on D+ couples into the flash clock before the clamp; usb-front-end.md wanted U2 ≤ 3 mm from J1 on a direct path | §3.1, §4, §9 | Route J1.A6/B6 → U2.3 directly (≈ 4.5 mm). If U2 pin 2's GND spoke blocks it, take pin 2 to GND by its own via (F-4) instead of the pour, or swap the two lines at the schematic (USBLC6-2 I/O1 and I/O2 are identical) so D+ uses pins 1/6 |
| F-4 | MINOR | U2 GND pin 2 and C1's GND pad reach L2 through 2.8–3.2 mm of top pour; no dedicated via | §9 | One 0.5/0.2 via at U2.2 and one at C1.2 (usb-front-end.md layout guidance) |
| F-5 | MINOR | LDO thermal relies on the 3 footprint EP vias; the ≥ 50 mm² pour with ≥ 4 extra vias that ldo-3v3.md required is not there (38 mm², 0 vias). Estimate TJ ≈ 95–107 °C worst case vs 125 °C limit | §7 | Add 4–6 GND vias in the pour at U3's pin-free ends (NC pins 2/3/6/7 can be left unconnected; the pour runs under them). Keep the bring-up measurement |
| F-6 | MINOR | QSPI_SCLK runs 17 mm at 0.12–0.14 mm from QSPI_SD0 (and 8 mm from SD3): ≈ 70–290 mV crosstalk depending on pad slew | §4 | Where the escape allows, open the gap to ≥ 0.2 mm; otherwise leave the QSPI pads at slow slew (bootrom/SDK default) and accept — margins are ≈ 1 V |
| F-7 | NOTE | Signal vias at U1's QSPI/USB escape are 3–4 mm from the nearest GND via; return current changes plane through decouplers | §9 | One GND via near (106.5, 121.5) and one near (111, 118) |
| F-8 | NOTE | GPIO23 (pixel data) runs 4.8 mm at 0.38 mm from RUN on L4: ≈ 36 mV on a Schmitt reset input | §4 | None required; if re-routing nearby, give RUN 0.6 mm |
| F-9 | NOTE | XOUT necks to 0.09–0.11 mm for 1.5 mm (class 0.15); XOUT_R's load-cap spur to C21 is 4.7 mm | §3.3 | Cosmetic; tidy when F-1 is done (bring C21 to the Y1.3 pad) |
| F-10 | NOTE | All LDO output current enters L3 through a single 0.3 mm via at (114.7, 116.45) | §6 | Add a second 3V3 via at C3 |
| F-11 | NOTE | DIFF_HS geometry is ≈ 106 Ω coupled / 70 Ω single, not the ≈ 90 Ω stackup.md estimates; pairs are not coupled anyway | §3.1 | Correct the number in stackup.md; no board change (FS, lumped) |
| F-12 | NOTE | USB_*_MCU 3.8–4.1 mm vs the 3 mm limit in nets.csv; 0.15 mm wide | §3.1 | Raise the limit to 5 mm in nets.csv; the resistors are at the chip as the HDG asks |

No BLOCKER. F-1/F-2/F-3 are each a few minutes of re-routing with no part or
schematic change (F-3 has an optional schematic swap); they should be done before
fab because they are the kind of thing that cannot be fixed on the bench.

## 11. Gate checklist (phase 8)

- [x] Trigger list generated from nets.csv (§1)
- [x] Stronger-model SI/EMC review added what the triggers miss (§9)
- [ ] Each triggered item has a result or a waiver in `decisions.md` — waivers to log:
  USB impedance (lumped at FS), regulator SPICE (vendor copper), hot-plug (ldo-3v3.md
  bring-up test), LX-zone proximity to VREG_AVDD/1V1 (QFN pin pitch, vendor layout)
- [x] Aggressor/victim check run (§4); findings F-1, F-2, F-3 to fix, rest waived/noted
- [ ] Design changes fed back: re-run the routing/DRC/sync checks and re-render after
  F-1…F-3; stale gates 7 and 9 re-run
