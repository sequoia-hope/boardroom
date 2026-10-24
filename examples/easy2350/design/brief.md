# Layout brief: easy2350

Phase 4. Decided before placing parts. Coordinates are mm from the board's
top-left corner, viewed from the top, USB-C at the top edge (y = 0).
`scripts/board.py outline` builds the outline and connectors; renders of the
finished board are in `design/renders/`.

## 1. Outline

21.0 × 51.0 mm, 1.6 mm thick, 1.0 mm corner radius (intent.md). Pico
footprint, so Pico cases and breadboard spacing fit.

## 2. Mounting holes

**None.** Intent leaves count and position to this phase. The usable width
between the header rows is 15.4 mm, and every square millimetre of it is
needed for the RP2350 block, the regulator and the buttons. The Pico's 2.1 mm
holes would sit where the flash and the LDO go. The board is held by its
headers (breadboard or socket). Logged in decisions.md.

## 3. Connectors

| Ref | Where | Facing | Notes |
|---|---|---|---|
| J1 USB-C | top edge, centred (x 10.5) | out of the top edge | Mouth overhangs the edge 0.46 mm (usb-front-end.md); pads end at y 8.5. Nothing taller than 1 mm within 1 mm of the plug opening |
| J2 1×20 | left long edge, x 1.61, pins y 1.37 → 49.63 | down: mounted from the bottom, plastic under the board | Board pins 1–20 |
| J3 1×20 | right long edge, x 19.39, pin 1 at y 49.63 | down, from the bottom | Board pins 21–40, pin 40 (VBUS) next to USB-C as on the Pico |
| J4 SWD 1×3 | bottom edge, centred, pins at x 7.96 / 10.5 / 13.04, y 49.63 | up | SWCLK, GND, SWDIO left to right |

Headers and J4 are not assembled (intent.md). Their pads stay bare on both
sides, so nothing may be placed over them on either side.

## 4. Assembly sides

**Top only** (intent.md: the bottom sits flat on a breadboard). One
assembly pass at JLC, economic PCBA if the 0.4 mm QFN allows (parts/rp2350-core.md).

## 5. Layer count and stack

**4 layers**, JLC's standard 1.6 mm stack (JLC04161H-7628): 1 oz outer, 0.5 oz
inner, 0.21 mm prepreg L1–L2. Reasons: the RP2350's regulator needs a solid
ground under it and a layer-2 cut-out under LX (chips/rp2350.md); 30 GPIO fan
out to two headers 17.78 mm apart; USB and QSPI want an unbroken reference.
The vendor minimal board is 2 layers, but it is 3× the area.

| Layer | Use |
|---|---|
| L1 F.Cu | parts, short signals, the RP2350 power zones copied from the vendor layout |
| L2 In1.Cu | **GND plane, unbroken** except the cut-out under VREG_LX and L1's LX pad |
| L3 In2.Cu | **+3V3 plane, whole board** (phase 7: a partial pour added after routing left 3V3 islands) |
| L4 B.Cu | GPIO fan-out, VBUS and VLED runs, GND pour in the gaps |

Stack details and rules are phase 5 (`stackup.md`).

## 6. Density

Compact: 0402 passives, 0603 only where a part needs it (C2, C23, D1),
0805 for the 10 µF on 3V3. Test points: header pins 40 (VBUS), 36 (3V3) and
38 (GND), and a 1.5 mm pad for 1V1 (TP3) at DVDD pin 23. No isolation jumpers: the board is simple enough to bring up by
rail, and the LDO, LED rail and RP2350 can be checked at their test points.

## 7. Floorplan

The power and signal flow runs from the USB-C at the top to the LEDs and
buttons at the bottom. Usable area: x 2.5–18.5 between the header pads.

| Region (y) | Block | Placement intent |
|---|---|---|
| 0–8.5 | J1 USB-C | — |
| 8.5–13 | USB front end: U2 ESD in line with D+/D−, CC resistors, C1 | U2 straight below J1's data pads; D± flow J1 → U2 → down to U1 |
| 8.5–18, right | 3V3 LDO U3 (SOT-223), C2, C3 | Right of the USB pair, tab to a GND pour (≥ 100 mm², ≥ 6 vias, ldo-3v3.md); ≥ 5 mm from Y1 and the pixels |
| 8.5–18, left | Flash U5 (SOIC-8, rotated), C22 | Next to the QSPI pins at U1's top-left; QSPI ≤ 10 mm |
| 17–32 | RP2350 U1 at (10.5, 25) and its power block | Vendor layout transplanted relative to U1 (chips/rp2350.md): regulator above pins 46–50, 27 Ω at the USB pins, decouplers at each pin per their Decouples field |
| 32–38 | Y1, its 1 k and 15 pF, SWD 100 Ω | Under U1's XIN/XOUT pins, as the vendor board (Y1 at U1 + 12.4 mm) |
| 38–48 | RGB pixels D3–D6 in a row, U4, D2, R11, C23; SW1, SW2; D1 | Pixels visible, spread across the width; buttons at the edges where a finger reaches them |
| 48–51 | J4 SWD | — |

Noisy: VREG_LX (top-right of U1) and the pixels' PWM. Sensitive: Y1, the
QSPI lines, USB. LX stays inside the vendor geometry; the crystal is on the
opposite side of U1 from LX; the pixels are below the crystal with ground
between.

## 8. Thermal

- U3 AP7361C: up to 0.62 W (ldo-3v3.md). Tab on a top-layer GND pour of at
  least 100 mm² with 6+ vias to L2; keep it ≥ 5 mm from Y1 and the pixels.
- D2 SM4007PL: 0.16 W. Pads on generous copper.
- Pixels: 4 × 0.18 W at full white. Spread them out; GND pads to the plane.
- RP2350: its EP to GND through the footprint's thermal vias.

## 9. Art zone

No art on the top: there is no room. The **bottom** has no parts, so it
carries the pin labels (GP0 … GP29, GND, 3V3, VBUS, RUN beside every header
pad, as the Pico does) and the board identity block.

## 10. Silkscreen

- Designators: **selective** (intent.md): connectors, ICs, buttons, LEDs and
  test points on the top; passives none.
- Top: "BOOT" and "RESET" at the buttons, "SWD" with pin names at J4, "5V IN"
  at USB-C, the header limits ("3V3 100 mA, VBUS 50 mA OUT").
- Bottom: header pin labels; name, rev, date, repo URL, CERN-OHL-P-2.0;
  "3.3 V logic, not 5 V tolerant" and "do not power VBUS with USB connected".

## 11. Keepouts

- Layer 2 under VREG_LX and L1's LX pad (rule area, chips/rp2350.md).
- No copper under the pixels' centre strip on L1 (rgb-leds.md).
- Nothing within the USB-C plug's overhang.

## 12. Test and debug

VBUS, 3V3 and GND on header pins 40, 36 and 38; TP3 (1V1) is a 1.0 mm pad on
the bottom under DVDD pin 23, never on L1 or COUT (review1-core #5). SWD on J4.
BOOTSEL and RESET buttons.
