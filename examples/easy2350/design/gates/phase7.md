# Gate 7: routing

Status: **passed** (2026-10-24, final route after the release-review fixes)

`scripts/board.py route` then `scripts/board.py finish`:

1. `unroute`: the previous route's unlocked copper goes first, so the pad vias
   find their spots on the pre-route board;
2. zones for routing: L2 GND plane, L3 3V3 plane, VREG_LX cut-out, rule areas
   for the art zone (all fixed silk), the pixel centre strips and the long
   edges (no tracks within 0.7 mm);
3. locked pad vias (`stage_padvias`): GND at every capacitor, 6 round U3, U2,
   U4 and the pixels where they fit; 3V3 at every capacitor, 2 at C3, U3 and
   the flash (`--wired`: joined to the pad by its own stub);
4. locked critical pre-routes: XIN/XOUT/XOUT_R, USB-C D± at the receptacle,
   USB D± from U2 to the 27 Ω resistors, and the pixel chain hops;
5. `autoroute.py` (Freerouting, governed, clearances + 0.01 mm) and
   `finish_route.py` for what it leaves;
6. finish: pours, `via_nudge.py` (vias out of reservations), silk,
   `via_nudge.py` again (vias a label couldn't avoid), a second via at each
   VBUS layer change where it fits, stitching clear of the silk, fill, and
   removal of dangling stubs (a pass that would open a connection is refused).

## DRC

`kicad-cli pcb drc --severity-all` on the final board (2026-10-24 09:15):

```
** Found 53 DRC violations **
** Found 0 unconnected pads **
** Found 0 Footprint errors **
```

**0 errors, 0 unconnected.** All 53 items are `lib_footprint_mismatch`
warnings: board-local footprint copies (silk trimmed where it meets pads, the
edge or another part's silk) and the project-library parts.

## Checks

| Check | Result |
|---|---|
| `nets_check.py --rules --routed` | PASS, 0 errors, 0 warnings (below) |
| `placement_check.py` | PASS (gate 6) |
| `sync_check.py` with BOM and CPL | PASS, 58 / 58 / 54 / 54 |
| `silk_check.py` | PASS (gate 9) |
| Unit tests (`scripts/tests`) | 57 passed |

```
  routed length (mm, track copper on the net; zones not counted) against nets.csv
    net                 routed  limit vias
    VREG_LX               0.00    5.0    0  (zone copper)
    VREG_AVDD             3.53   10.0    0
    USB_DP_C              7.92   20.0    2
    USB_DM_C              7.75   20.0    2
    USB_DP                9.97   20.0    2
    USB_DM               10.06   20.0    2
    USB_DP_MCU            4.10    5.0    0
    USB_DM_MCU            3.86    5.0    0
    QSPI_SCLK            16.15   25.0    2
    QSPI_SD0             14.35   25.0    2
    QSPI_SD1              7.71   25.0    0
    QSPI_SD2              7.48   25.0    2
    QSPI_SD3             17.41   25.0    2
    QSPI_SS               7.82   25.0    0
    XIN                   9.86   10.0    0
    XOUT                  2.95    5.0    0
    XOUT_R                4.09    5.0    0

  PASS: 0 errors, 0 warnings
```

Measurements on the final board (pcbnew, board mm from the top-left):

| Item | Value |
|---|---|
| Vias | 138 (43 GND); L1 468 mm, L4 550 mm of track; L2/L3 planes only |
| Pixel chain | PIX12 2.3 mm, PIX23 2.9 mm, PIX34 2.3 mm, no vias; LED_DIN 7.5 mm, no vias |
| Under the pixel centre strips (L1) | no tracks, vias or pour |
| XIN to PIX12 | 2.8 mm; VLED 4.9 mm from Y1's centre |
| GPIO23 to RUN | ≥ 0.68 mm |
| QSPI_SCLK | 1.1 mm within 0.2 mm of SD3 (min 0.13), at the pin escape |
| Tracks under class width | +3V3/+1V1 0.15 mm at the RP2350 pins (vendor copper); USB 0.15 mm (pre-routes, vendor 27 Ω copper); VLED 0.187 mm for 1.5 mm at pad entries; GPIO8–10/18 0.09 mm at QFN pads (JLC 0.09). All above the derived widths (nets_check) |
| VBUS vias | 5: pairs at J3 pin 20 and at D2; one at (5.9, 11.9) where the LED branch leaves the L1 VBUS network (no room for a second). That branch carries ≤ 0.2 A against 0.8 A for one 0.3 mm via (decisions.md) |
| 3V3 into L3 | 12 vias, 2 at C3 |
| L4 under U1 | VBUS 11 mm, BOOTSEL 15 mm, GPIO12/19/20/23/25 and the vendor 1V1 spokes |
| Within 0.7 mm of the long edges | no tracks or vias (edge strips) |

## Independent review

`review-layout.md` (phases 6/7/9) and `review-release.md` (phase 10).

| Source | # | Finding | Resolution |
|---|---|---|---|
| layout | 3 | XIN through vias and L4 | Fixed: L1 pre-route, 0 vias, 9.86 mm |
| layout | 4 | USB D+ hairpin | Fixed: D± pre-routed at J1 and from U2 to the 27 Ω |
| layout | 6 | Merged antipads slot L2 under the escapes | Accepted: 2–3 mm slots, return detour ≤ 3 mm; QSPI at its slow-slew default |
| layout | 7 | Single vias at power transitions | Fixed: 2 × 3V3 at C3, 3V3 pad vias everywhere; VBUS twins where they fit |
| layout | 8 | VLED under the crystal | Fixed: 4.9 mm from Y1's centre |
| layout | 9 | VBUS / BOOTSEL on L4 under U1 | Accepted: L2 GND and L3 3V3 lie between them and the chip; both DC or static. The chip-note deviation is recorded here |
| layout | 10 | USB pair split | Fixed: U2 to the 27 Ω pre-routed side by side on L4, 10 mm, 2 vias each |
| layout | 13 | QSPI spread, SCLK vias | Accepted: 7.5–17.4 mm, all ≤ 25 mm |
| layout | 20 | Dangling stub | Fixed: `stage_clean` |
| release | 2 | BOOTSEL along the edge by J2's joints | Fixed: edge strips; nothing within 0.7 mm of the long edges |
| (author) | — | Copper under every pixel's centre strip | Fixed: pixels turned 180°, rule areas (gate 6) |
