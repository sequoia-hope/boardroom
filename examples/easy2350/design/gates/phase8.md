# Gate 8: simulation and SI

> **Stale (2026-10-31).** The board was re-routed with ultraroute 63679d9 from a bare board, with no locked copper (LESSONS.md, [routing]); this gate's evidence is for the old route. See design/status.json and decisions.md.


Status: **passed** (2026-10-24, final route)

`design/sim/si-review.md` is the independent SI/EMC review (fresh context) of
the routed board. Its triggers and waivers:

| Trigger (simulation.md) | Outcome |
|---|---|
| Crystal (XOSC) | No simulation: stray estimate in rp2350-core.md (≈ 0.1 pF/mm × 9.9 mm ≈ 1 pF, inside HDG's 3 pF allowance); bring-up check of XOSC start-up stands |
| USB full speed | Lumped (decisions.md): ≥ 4 ns edges, ≈ 26 mm of trace; geometry ≈ 106 Ω coupled / 70 Ω single (stackup.md) |
| QSPI at 133 MHz class | Lengths 7.2–17.0 mm (≤ 25 mm, flash.md), crosstalk estimate in si-review §4 |
| Switching regulator (RP2350 VREG) | Vendor copper copied unchanged (gate 6); no SPICE (decisions.md: no public models; Raspberry Pi validated this layout) |
| LDO hot-plug | Bring-up scope test, not simulated (decisions.md) |
| PDN | si-review §6 DC drop: ≈ 2 mV on 3V3 at 0.28 A |

## si-review findings against the final board

Final-board numbers from a pcbnew measurement script (edge-to-edge, same
layer; board mm).

| # | Sev | Finding | Final board | Status |
|---|---|---|---|---|
| F-1 | MAJOR | XIN 8.65 mm, 2 vias, 6 mm on L4 next to PIX12 | 9.86 mm on L1, 0 vias; 2.8 mm from PIX12, no other pixel net nearby | Fixed |
| F-2 | MAJOR | PIX12 25 mm under U1 | Pixels turned 180°: PIX12 2.3 mm, PIX23 2.9 mm, PIX34 2.3 mm, L1, no vias (pre-routes) | Fixed. An intermediate route ran PIX23 45 mm past XIN; that is what prompted the turn (decisions.md) |
| F-3 | MAJOR | USB_DP_C 12 mm loop past QSPI_SCLK | 7.92 mm; 2.63 mm from QSPI_SCLK | Fixed |
| F-4 | MINOR | U2 and C1 GND through the pour | pad vias at U2 and every capacitor | Fixed |
| F-5 | MINOR | LDO thermal: EP vias only | 6 locked GND vias round U3 | Fixed |
| F-6 | MINOR | QSPI_SCLK 17 mm at 0.12–0.14 mm from SD0 | SD0 ≥ 0.30 mm everywhere; SD3 1.1 mm within 0.2 mm (min 0.13) at the pin escape | Fixed |
| F-7 | NOTE | Escape vias 3–4 mm from a GND via | 4.0 mm at (6.5, 21.5), 2.6 mm at (11.0, 18.0) | Accepted: QSPI/USB returns change planes through the decouplers' pad vias; slow-slew default |
| F-8 | NOTE | GPIO23 4.8 mm at 0.38 mm from RUN | ≥ 0.68 mm everywhere | Fixed |
| F-9 | NOTE | XOUT necks 0.09 mm | XOUT not below class; 2.95 mm | Fixed |
| F-10 | NOTE | One 3V3 via at C3 | 2 vias | Fixed |
| F-11 | NOTE | DIFF_HS impedance number | stackup.md corrected | Fixed |
| F-12 | NOTE | USB_*_MCU limit | nets.csv 5 mm; 4.10 / 3.86 mm | Fixed |

`nets_check.py --rules --routed`: PASS (gate 7), including XIN ≤ 10 mm with no
vias and XOUT/XOUT_R ≤ 5 mm.

Re-review: the release reviewer (gate 10) checks these numbers on the final
board rather than a second SI pass; nothing changed except the fixes above.
