# easy2350 rev A: bring-up

Do these in order on the first board. Stop at the first step that fails. Test
points: header pin 40 VBUS, pin 36 3V3, pin 38 GND (J3, bottom labels), TP3
1V1 (bottom pad). Coordinates are board mm from the top-left, USB-C at the top.

## 0. Before power

- Look at the board under a lens with the assembly drawing
  (`design/renders/assembly-top.pdf`, designators ≈ 4:1) beside it: U1
  (QFN-60) and U3 (DFN-8) for bridges, the pin-1 dots at U1, U3, U4, the
  pixels and the diode cathodes (gate 9). JLC places from the CPL, so this is
  where a rotation error shows. L1's polarity dot must be at its **left** end
  (pad 1, 1V1, at (11.8, 19.3); chips/rp2350.md follows Raspberry Pi's
  footprint, whose convention differs from Abracon's drawing): reversed, the
  regulator still works but regulates worse under load.
- After soldering the headers, check J2's joints: no bridge to a neighbour.
- Resistance to GND, meter on ohms: VBUS > 10 kΩ, 3V3 > 1 kΩ (it charges),
  1V1 > 100 Ω. A short here means stop.

## 1. First power, current-limited

Feed 5.0 V into header pin 40 (VBUS) from a bench supply limited to 100 mA,
USB unplugged (never both: silk "no VBUS in with USB in").

| Measure | Expect | Source |
|---|---|---|
| Supply current, BOOT not pressed, blank flash | ≈ 15–30 mA (RP2350 in the bootrom waiting for USB, LDO quiescent, pixels idle ≈ 1 mA each) | rp2350-core.md, rgb-leds.md |
| 3V3 at pin 36 | 3.30 V ± 2 % | ldo-3v3.md (AP7361C accuracy) |
| 1V1 at TP3 | 1.10 V (VREG default) | rp2350-core.md |
| VLED (D2 cathode, C23) | VBUS − 0.4…0.6 V (≈ 4.5 V at 5.0 V in: D2 is a silicon rectifier at its 1 mA bleed) | rgb-leds.md |
| VLED with VBUS at 5.5 V (rgb-leds.md open question 1) | ≤ 5.13 V; the 5.1 V zener D7 clamps at ≤ 5.27 V | rgb-leds.md |

Then raise the limit to 500 mA.

## 2. USB and the bootrom

Plug USB (supply disconnected). The board should enumerate as **RP2350
BOOTSEL** mass storage on its own (blank flash). If not: hold BOOT, tap RST.

- Scope XIN/XOUT is not needed if USB enumerates: USB needs the crystal.
  If it doesn't enumerate, probe XOUT at R6 with a ×10 probe: 12 MHz,
  ≥ 1 V p-p (decisions.md: XIN limit raised to 10 mm; this is the check).
- Hot-plug ring (decisions.md): scope VBUS at C2 (U3's input cap; U3's DFN pads
  are too small to probe) and VLED at C23, 1 µs/div,
  plug in a short thick cable ten times. Note the peak. Over 6.5 V at U3 is
  outside its absolute maximum: add a TVS in rev B.

## 3. Flash and firmware

- Drag a UF2 that blinks GPIO25 (D1, labelled "25") and prints over USB CDC.
- Flash ID read (picotool info -a): W25Q128JV, 16 MB.
- SWD on J4 with a Raspberry Pi Debug Probe: pin order on the bottom silk
  ("DIO GND CLK", seen from below); openocd `target/rp2350.cfg`.

## 4. Pixels

- Firmware drives 4 pixels on GPIO23 (through U4, R10). One at a time white,
  then all four at full white for 10 s: current ≈ 160 mA more. VLED reads
  ≈ 4.0–4.2 V from a 5.0 V port (D2 drops ≈ 0.84 V at 160 mA); pass if
  ≥ 3.5 V (the pixels' 3.3 V minimum plus margin; worst case 3.52 V at 4.5 V
  in, rgb-leds.md).
- Wrong colour order or nothing after pixel 1: check PIX12's via pair under
  the pixel row (critical pre-route).

## 5. Thermal and limits

- 30 min at full pixel white with a 100 mA load on header 3V3: U3's case
  ≤ 85 °C at 25 °C ambient (ldo-3v3.md estimates TJ ≈ 95–107 °C worst case at
  40 °C).
- 3V3 under 100 mA header load: ≥ 3.2 V.
- 1V1 at TP3 under a CPU-heavy firmware load (CoreMark or similar, both cores):
  1.10 V ± 3 %, no audible whine. Low or noisy here, with L1's dot correct,
  points at the inductor.

## Record

Write the results in `design/bringup-results.md` with the board number, and
any rev B items in decisions.md.
