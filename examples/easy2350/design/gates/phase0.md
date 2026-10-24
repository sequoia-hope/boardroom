# Gate 0: design intent

Status: **passed** (user sign-off 2026-10-23, with one change: add addressable LEDs)

## Checklist (references/intent-and-brief.md, phase 0)

| Item | Where in intent.md | State |
|---|---|---|
| Purpose | Purpose | done |
| Open source or confidential | Open source: CERN-OHL-P-2.0, repo on silk | done |
| Every external interface: direction, signal, voltage normal/worst, current, mate | External interfaces table: J1 USB-C, USB data, J2/J3 headers, J4 SWD, SW1/SW2, D1 | done |
| Power: input range, every rail and load, sequencing, battery, backfeed | Power: VBUS 4.5–5.25 V at J1, 3V3 budget 327 mA max, 1V1 internal, no battery, header VBUS backfeed not supported | done |
| Environment: temperature, enclosure, airflow, ESD | Environment: 0–40 °C, open, still air, USB pins ESD-protected, headers not | done |
| Performance targets | Performance targets | done |
| Mechanical: size, mounting, heights, connector access | Mechanical: 21 × 51 mm, all parts top, 3.5 mm height | done |
| Manufacturing: fab, quantity, cost, sides, hand assembly | Manufacturing: JLCPCB, 5 assembled, US$15 (economic) / US$20 (standard), headers not assembled | done |
| Serviceability | Serviceability: selective designators, 4 test points, SWD | done |
| Compliance | Compliance: none formal; Type-C sink Rd | done |
| Known risks and open questions | Known risks | done |

## Independent review

Reviewer: stronger model, fresh context, given `intent.md` and the phase 0
checklist only (review-protocol.md, gate 0). 2026-10-22.

| # | Severity | Finding | Resolution |
|---|---|---|---|
| 1 | MAJOR | USB input current (300 mA) contradicts the loads (3V3 330 mA + VBUS out 300 mA ≈ 650 mA, over the 500 mA default sink limit) | Fixed: total 500 mA from VBUS; 3V3 out 150 mA and VBUS out 150 mA; total ≈ 480 mA shown in Power |
| 2 | MAJOR | GPIO source current missing from the 3V3 budget | Fixed: 50 mA aggregate line in the budget; header row says 12 mA per pin, 50 mA aggregate |
| 3 | MAJOR | Header pin map undefined; not Pico 2 pin-compatible but not stated | Fixed: remaining 7 pins are GND, "not Pico 2 pin-compatible" stated, GPIO25 shared with D1; exact map in phase 2 |
| 4 | MINOR | Inrush is set by the device's capacitance, not limited by the host | Fixed: VBUS bulk ≤ 10 µF |
| 5 | MINOR | Cable drop: 4.5 V at J1, LDO dropout must suit | Fixed: 4.5 V min at J1, dropout < 1.0 V at 330 mA, SOT-23-5 ruled out |
| 6 | MINOR | Miswired 9–20 V source unaddressed | Fixed: above 6 V is an explicit don't care |
| 7 | MINOR | 3V3 tolerance and ripple missing | Fixed: ±3 %, < 50 mVpp |
| 8 | MINOR | Mounting hole position conflicts with the QFN strip | Fixed: count and position don't care, phase 4 decides |
| 9 | MINOR | 90 Ω needs a tolerance | Fixed: don't care under 15 mm at full speed |
| 10 | MINOR | Flash clock 133 MHz vs SDK default | Fixed: SDK default clock |
| 11 | MINOR | Cost target holds only on economic PCBA with extended-part fees | Fixed: assumption stated, US$20 if standard PCBA |
| 12 | MINOR | USB-C variant must carry D+/D− | Fixed: 16-pin USB 2.0 type |
| 13 | MINOR | External supply on the 3V3 pin | Fixed: don't care, stated on silk |
| 14 | NOTE | RP2350 typical current generous | Fixed: typ 20 mA, citing datasheet Table 1693 |
| 15 | NOTE | Some GPIO are 5 V tolerant | Noted in the header row; not promised |
| 16 | NOTE | Button ESD reaches QSPI_SS and RUN | Explicit don't care, as on the Pico |
| 17 | NOTE | Two 6 × 6 mm tactiles crowd 21 mm | Added to risks: small or side-actuated switches in phase 1 |

No BLOCKERs. All MAJORs fixed; no re-review needed for wording-level fixes,
but the user's sign-off covers the final text.

## Sign-off

- [x] **User approves intent.md** (2026-10-23: "add small addressible leds to the
  demo board, then proceed with development")

### Change at sign-off

The user added a chain of small addressable LEDs. intent.md now has an RGB LED
row in the interface table, an LED rail in Power, and a new budget: header
3V3 out 150 → 100 mA and header VBUS out 150 → 50 mA, so 277 mA (3V3) + 160 mA
(LEDs) + 50 mA (VBUS out) stays under 500 mA. The LDO's dissipation drops to
0.62 W at the 5.5 V Type-C maximum. Also corrected while editing: the input
range now says 5.5 V is in spec for a Type-C source (vSafe5V), and ADC_AVDD
says what the vendor design actually does (100 nF, no RC filter; checked in its
schematic netlist). These are numbers inside the approved structure, so they
are not re-reviewed separately; phase 1's reviewer checks them against the
datasheets.

The user also said to proceed with development: the remaining user-sign-off
gates on this example (4 and 10) are recorded and continued, not stopped at.
