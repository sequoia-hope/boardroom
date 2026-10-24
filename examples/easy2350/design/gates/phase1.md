# Gate 1: architecture and parts

Status: **passed** (2026-10-23)

## Checklist (references/datasheet-audit.md)

| Item | Evidence | State |
|---|---|---|
| Block diagram, every block mapped to parts | `design/architecture.md` | done |
| Audit file per part with datasheet in `datasheets/` | `parts/`: rp2350-core, usb-front-end, ldo-3v3, flash, rgb-leds, user-io; 18 PDFs listed in `datasheets/sources.csv` | done |
| Abs max vs worst case from intent.md | each audit's "Absolute max" table | done |
| Every chosen value has a calculation | each audit's "Calculations" | done |
| MLCC derating | ldo-3v3.md (measured Samsung curve for the 0805 10 µF), others marked as assumptions | done, assumptions flagged |
| Footprint matches the exact package suffix | each audit; project footprints for the RP2350 QFN, L_pol_2016, the AP7361C DFN with EP vias, the MDD SOD-123F | done |
| Sourcing with `jlc_parts.py show` and date; nothing flagged | below | done |
| Every datasheet in `sources.csv` | `jlc_parts.py fetch-list design/datasheets/sources.csv` re-downloads all | done |
| Independent review re-read each datasheet | review1-core.md, review1-power.md, review1-io.md (Fable, fresh context) | done |

## Sourcing (`jlc_parts.py show`, every LCSC number in the schematic)

```
C11702 0402WGF1001TCE (UNI-ROYAL, 0402): basic, JLC stock 19551700, lifecycle normal, checked 2026-10-23
C151007 AP7361C-33FGE-7 (DIODES, DFN-8-EP(3x3)): extended, JLC stock 1460, lifecycle normal, checked 2026-10-23
C1525 CL05B104KO5NNNC (Samsung Electro-Mechanics, 0402): basic, JLC stock 36100897, lifecycle normal, checked 2026-10-23
C1548 0402CG150J500NT (FH, 0402): basic, JLC stock 1036788, lifecycle normal, checked 2026-10-23
C15850 CL21A106KAYNNNE (Samsung Electro-Mechanics, 0805): basic, JLC stock 9248341, lifecycle normal, checked 2026-10-23
C165948 TYPE-C-31-M-12 (Korean Hroparts Elec, SMD): extended, JLC stock 328494, lifecycle normal, checked 2026-10-23
C19077393 BZT52B5V1 (R+O, SOD-123): preferred, JLC stock 1182670, lifecycle normal, checked 2026-10-23
C19666 CL10A475KO8NNNC (Samsung Electro-Mechanics, 0603): basic, JLC stock 3054109, lifecycle normal, checked 2026-10-23
C20625731 ABM8-272-T3 (ABRACON, SMD3225-4P): extended, JLC stock 3393, lifecycle normal, checked 2026-10-23
C2286 KT-0603R (Hubei KENTO Elec, 0603): basic, JLC stock 7929696, lifecycle normal, checked 2026-10-23
C23630 CL10A225KO8NNNC (Samsung Electro-Mechanics, 0603): basic, JLC stock 3180533, lifecycle normal, checked 2026-10-23
C23733 CL05A475MP5NRNC (Samsung Electro-Mechanics, 0402): basic, JLC stock 2705617, lifecycle normal, checked 2026-10-23
C25100 0402WGF270JTCE (UNI-ROYAL, 0402): extended, JLC stock 56916, lifecycle normal, checked 2026-10-23
C25105 0402WGF330JTCE (UNI-ROYAL, 0402): basic, JLC stock 2252017, lifecycle normal, checked 2026-10-23
C25741 0402WGF1003TCE (UNI-ROYAL, 0402): basic, JLC stock 8021980, lifecycle normal, checked 2026-10-23
C25900 0402WGF4701TCE (UNI-ROYAL, 0402): basic, JLC stock 14793583, lifecycle normal, checked 2026-10-23
C25905 0402WGF5101TCE (UNI-ROYAL, 0402): basic, JLC stock 4317647, lifecycle normal, checked 2026-10-23
C42411118 RP2350A (Raspberry Pi, QFN-60-EP(7x7)): extended, JLC stock 5929, lifecycle normal, checked 2026-10-23
C42411119 AOTA-B201610S3R3-101-T (ABRACON, 0806): extended, JLC stock 187, lifecycle normal, checked 2026-10-23
C473338 SN74LV1T125DBVR (TI, SOT-23-5): extended, JLC stock 161583, lifecycle normal, checked 2026-10-23
C52917434 WS2812B-2020-V6 (Worldsemi, SMD2020-4P): extended, JLC stock 831632, lifecycle normal, checked 2026-10-23
C64898 SM4007PL (MDD(Microdiode Semiconductor), SOD-123FL): basic, JLC stock 4710462, lifecycle normal, checked 2026-10-23
C720477 TS-1088-AR02016 (XUNPU, SMD,4x3mm): basic, JLC stock 530245, lifecycle normal, checked 2026-10-23
C7519 USBLC6-2SC6 (ST, SOT-23-6L): extended, JLC stock 38186, lifecycle normal, checked 2026-10-23
C97521 W25Q128JVSIQ (Winbond, SOIC-8-208mil): basic, JLC stock 64250, lifecycle normal, checked 2026-10-23
```

9 extended line items (JLC setup fee each), the rest basic or preferred.

## Independent review

Three reviewers (stronger model, fresh context, datasheets and audits only):

| Review | BLOCKER | MAJOR | Resolution |
|---|---|---|---|
| review1-core.md | 0 | 1 (audit vs. stacked-pin symbol) | Decouples field on each cap binds it to its pin; audit amended. MINORs: XIN/XOUT labelled, passive datasheets added, L2 solid under the crystal, TP3 at pin 23's end, 33 Ω is ±1 % |
| review1-power.md | 0 | 2 (no 3V3 test point; connector-side USB nets unnamed) | 3V3/VBUS/GND test points are header pins (decisions.md); USB_DP_C/USB_DM_C labelled. MINORs: C2 2.2 µF, accuracy budget at TJ, clamp numbers marked as assumptions |
| review1-io.md | 0 | 2 (VLED hot-plug above 5.3 V; TP2 missing) | D7 BZT52B5V1 clamp added (rgb-leds.md); test points as above. MINORs fixed or noted |

## Changes after review (also in decisions.md)

- U3 SOT-223 → AP7361C-33FGE-7 DFN (space); SW1/SW2 → TS-1088 (space).
- D2 Schottky → SM4007PL + 4.7 kΩ bleed (VLED ≤ 5.3 V at idle); D7 clamp.
- At routing (phase 7): C10 dropped (C9 serves pins 44/45 as the vendor's
  C17 does), SWD series resistors dropped. Both are deviations from
  rp2350-core.md, logged in decisions.md.
