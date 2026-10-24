# Design intent: easy2350

Phase 0. Every item has a number or an explicit "don't care".

## Purpose

A minimal RP2350A development board, made with this template from phase 0
to fab outputs as a worked example. It does what a Raspberry Pi Pico 2 does,
with fewer parts: USB-C for power and USB device, the RP2350A with its flash
and crystal, BOOTSEL and RESET buttons, one plain LED, a chain of four small
addressable RGB LEDs, and every GPIO on two 0.1" headers that fit a
breadboard. Someone learning the flow reads its `design/`
folder gate by gate; someone who orders it gets a board they can program from
the Pico SDK or MicroPython.

## Open source or confidential

Open source. Hardware under CERN-OHL-P-2.0; repository
`github.com/sequoia-hope/easy_pcb` (path `examples/easy2350`) on the silkscreen.

## External interfaces

| Connector / link | Direction | Signal | Voltage (normal / worst case) | Current (cont. / peak) | Mates with |
|---|---|---|---|---|---|
| J1 USB-C receptacle, 16-pin USB 2.0 type | in | VBUS, D+/D−, CC1/CC2 | 5.0 V (4.75–5.25 at the host, 4.5 V min at J1 after cable drop) / 5.5 V; above 6 V (a non-compliant charger) don't care: the board may be damaged; ±8 kV contact ESD (IEC 61000-4-2 level 4) on every pin | 500 mA max total (USB default sink); VBUS bulk capacitance ≤ 10 µF (USB 2.0 attach limit) | any USB-C cable to a host or charger; sink only, 5.1 kΩ Rd on CC, asks for nothing above default USB power |
| USB data | bidirectional | USB 2.0 full speed, 12 Mb/s | 3.3 V signalling | — | host PC |
| J2, J3 GPIO headers, 2 × 1×20, 0.1" pitch, 17.78 mm (700 mil) apart | bidirectional | GPIO0–29 (GPIO25 shared with D1), RUN, 3V3, VBUS, and GND on the remaining 7 pins. **Not** Pico 2 pin-compatible (the Pico hides GPIO23/24/25/29 and has 3V3_EN, ADC_VREF, AGND); exact map in phase 2 | 3.3 V logic; 0–3.3 V promised (some pins are 5 V tolerant per the datasheet; not promised) / 5 V on a GPIO, or an external supply on the 3V3 pin, not protected: don't care, stated on silk | 3V3 out: 100 mA; VBUS out: 50 mA; GPIO 12 mA per pin, 50 mA aggregate | breadboard or female headers |
| J4 SWD, 1×3 0.1" (SWCLK, GND, SWDIO) | bidirectional | SWD | 3.3 V | — | Raspberry Pi Debug Probe (3-pin JST-SH adapter) or jumper wires |
| SW1 BOOTSEL, SW2 RESET | in | buttons | 3.3 V; finger ESD reaches QSPI_SS and RUN unprotected, as on the Pico: don't care | — | finger |
| D1 status LED | out | on GPIO25, like the Pico | 3.3 V | 2 mA | eye |
| RGB LED chain, 4 addressable pixels in a 2 × 2 mm (or smaller) package | out | WS2812-protocol data from GPIO23 (shared with the header pin); the chain's last DOUT is not brought out | data: 3.3 V logic in, level-shifted on board to what the LEDs need; LED supply from VBUS | 160 mA max at full white (4 × 3 channels × 12 mA, plus 10 %); firmware may limit it, the budget does not assume so | eye; visible from the top |

## Power

- Input: USB VBUS only, 4.75–5.25 V from a USB 2.0 host, up to 5.5 V from a
  Type-C source (vSafe5V), 4.5 V min at J1 after cable drop. No battery, no second
  input. Powering the board from the header's VBUS pin while USB is plugged in
  is not supported (no backfeed protection); the silkscreen says so.
- 3V3 rail: one LDO from VBUS. Load budget:

  | Load | Typ | Max |
  |---|---|---|
  | RP2350A (both cores 150 MHz, USB active, core regulator from 3V3; typ from datasheet Table 1693, ~16 mA for `hello_usb`) | 20 mA | 100 mA |
  | GPIO aggregate source current | — | 50 mA |
  | QSPI flash (read, SDK default clock) | 10 mA | 25 mA |
  | LED | 2 mA | 2 mA |
  | header 3V3 out | — | 100 mA |
  | **3V3 total** | ~35 mA | **277 mA** |

  3V3 is 3.3 V ±3 %, ripple under 50 mVpp.
- RGB LED rail: from VBUS, 160 mA max (4 pixels at full white, +10 %). The
  rail must stay inside the LEDs' supply rating for every VBUS in the input
  range (4.5–5.5 V at J1), and the data line must meet the LEDs' input-high
  threshold at that supply with margin.
- From VBUS in total: 277 mA (3V3) + 160 mA (RGB LEDs) + 50 mA (header VBUS
  out) + quiescent ≈ 490 mA, inside the 500 mA default sink limit. The
  silkscreen states both header limits.

  The LDO sees 4.5–5.5 V in, so its dropout must be under 1.0 V at 280 mA.
  It dissipates up to (5.5 − 3.3) V × 0.28 A ≈ 0.62 W: phase 1 must show the
  junction stays under its rating at 40 °C ambient (a SOT-23-5 won't).
- 1V1 core: the RP2350's own switching regulator, laid out per the
  datasheet (§6.3.8) and the vendor's minimal board.
- Sequencing: none beyond the RP2350's own power-on reset.

## Environment

Bench and desk: 0–40 °C ambient, no enclosure, still air. Handled by people
and plugged in many times, so USB-C pins get ESD protection. Header pins are
not protected (development board convention, stated on the silkscreen).

## Performance targets

- RP2350 at its rated 150 MHz from a 12 MHz crystal.
- USB 2.0 full speed device (12 Mb/s). At full speed a run under 15 mm needs no controlled impedance: 90 Ω differential is a target, tolerance don't care.
- QSPI flash: 16 MB (128 Mbit), boots at the bootrom's default clock and runs
  at the SDK default.
- ADC on GPIO26–29: don't care beyond the RP2350's own; ADC_AVDD decoupled as
  the vendor design does (tied to 3V3 with its own 100 nF; no RC filter).
- RGB LEDs: 800 kb/s WS2812 protocol, as fast as the RP2350 PIO drives it;
  colour accuracy and matching between pixels don't care.

## Mechanical

- Board 21 × 51 mm (Pico footprint), 1.6 mm thick, corner radius 1 mm.
- Headers on the two long edges, 17.78 mm apart; USB-C on one short edge,
  SWD on the other.
- Mounting holes: count and positions don't care; phase 4 decides (M2,
  2.2 mm NPTH if any). Matching the Pico's 4-hole pattern: don't care.
- All parts on top. Bottom has no parts so the board sits flat on a
  breadboard. Top height: 3.5 mm max except the USB-C receptacle and buttons.

## Manufacturing

Fab and assembler: JLCPCB, standard PCBA (economic if the RP2350A's 0.4 mm
QFN allows). Quantity: 5 assembled. Cost target: under US$15 per assembled
board at quantity 5, before shipping and tax, assuming economic PCBA
including the extended-part fees (RP2350A, USB-C, ESD array, inductor,
crystal, flash: about US$3 per board at quantity 5). If the QFN forces standard
PCBA, the target becomes US$20. Headers are not assembled (the
user solders them, or not). JLC basic parts where a basic part fits.
Layer count is decided in phase 4 (expected 4 for the RP2350 regulator and USB).

## Serviceability

Hobby development board. Designators on silk for connectors, ICs, buttons,
LED and test points (selective policy). Test points on VBUS, 3V3, 1V1 and GND:
VBUS, 3V3 and GND are header pins 40, 36 and 38 (a probe clips on them as well
as on a pad); 1V1 has its own pad (TP3). SWD header for debugging. No rework
features beyond that.

## Compliance

None formal (development board; no CE/FCC marking). USB-C behaviour follows
the Type-C spec for a sink: 5.1 kΩ Rd on each CC pin.

## Known risks and open questions

- RP2350 core regulator layout: copy the vendor's (see `chips/rp2350.md`).
- 0.4 mm-pitch QFN-60 at JLC: check economic vs standard assembly in phase 1.
- LDO thermal at 280 mA from 5.5 V (above).
- The RGB LEDs' supply and data-high ratings against a 4.5–5.5 V VBUS: phase 1
  picks the supply path and level shifter from the LED datasheet.
- 21 mm width with a USB-C receptacle, buttons and two header rows: phase 4
  confirms it fits on 4 layers; if not, the width grows, not the risk. Two
  6 × 6 mm tactile switches will crowd it; prefer small (3 × 4 mm or
  side-actuated) switches in phase 1.
