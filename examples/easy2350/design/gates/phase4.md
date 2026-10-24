# Gate 4: mechanical and layout brief

Status: **passed** (2026-10-23)

`design/brief.md` covers the twelve items of references/intent-and-brief.md
(outline, holes, connectors, sides, layers, density, floorplan, thermal, art
zone, silkscreen, keepouts, test and debug).

## Decisions made here (decisions.md)

- 21 × 51 mm, no mounting holes; headers J2/J3 mounted from the bottom.
- 4 layers, JLC04161H-7628: L2 GND plane, L3 3V3 plane (changed in phase 7
  from "3V3 pour plus fan-out"), signals on L1/L4.
- Test points: header pins 40/36/38 for VBUS/3V3/GND, TP3 for 1V1.
- U3 in the DFN-8 package and TS-1088 switches, for space.

## Review

Not separately reviewed: the brief's decisions are exercised by placement
and routing (gates 6 and 7), whose reviews see the result. A gap the brief
missed (the area budget) was caught when placing, not here; the brief now
records the area budget in its decisions.

## Sign-off

- [x] User: "add small addressible leds to the demo board, then proceed with
  development" (2026-10-23) — recorded in gate 0 as covering the example's
  later sign-off gates. The user reviews the finished board.
