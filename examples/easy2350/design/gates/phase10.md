# Gate 10: release

Status: **ready for release; awaiting the user's approval** (2026-10-24)

Fab outputs: `python3 scripts/fab.py` → `hw/easy2350/a/fab/`:
`easy2350-gerbers.zip` (gerbers + Excellon drill, 4 layers), `bom-jlc.csv`,
`cpl-jlc.csv` (54 placements, top only), `easy2350.step`. Schematic and
assembly drawing: `design/renders/schematic.pdf`, `assembly-top.pdf`.

## Checklist

| Item | Result |
|---|---|
| Earlier gates current | Gates 6–9 written against this board (2026-10-23 final route + finish). Gates 0–5 unchanged by layout work; their later revisions are noted in the gate files |
| Sync check with BOM and CPL | PASS (below) |
| DRC on the final files | 0 errors, 0 unconnected; 53 `lib_footprint_mismatch` warnings, all board-local footprint copies or project-library parts (gate 7) |
| ERC | gate 2; schematic unchanged since |
| Stock re-checked | 2026-10-24, every LCSC part (below): all in stock, lifecycle normal; L1 128 pieces (it read 1 the day before) |
| CPL rotation | Needs JLC's assembly preview at order time (list below) |
| Gerbers reviewed outside KiCad | `gerber_render.py` (gerbonara + headless Chrome), `design/renders/gerbers/` (below) |
| Bring-up plan | `design/bringup.md` |
| Final independent review | `review-release.md`: "send it" on the re-check (below) |
| Git tag | not tagged: nothing in this repo is committed yet; tag `easy2350-revA` when the user commits |
| User approves release | **pending** |

### Sync

```
sync_check: /home/sequoia/Software/easy_pcb/examples/easy2350/hw/easy2350/a
  schematic easy2350.kicad_sch: 58 symbols (0 excluded from board)
  board easy2350.kicad_pcb: 58 footprints (0 board-only)
  BOM bom-jlc.csv: 54 designators
  CPL cpl-jlc.csv: 54 placements
  54 parts to build; required fields: LCSC
  stock not checked: re-check every LCSC part at release (phase 10)

  PASS: 0 errors, 0 warnings
```

### Stock (`jlc_parts.py show`, 2026-10-24)

```
C1525 CL05B104KO5NNNC (Samsung Electro-Mechanics, 0402): basic, JLC stock 35853814, lifecycle normal, checked 2026-10-24
C23630 CL10A225KO8NNNC (Samsung Electro-Mechanics, 0603): basic, JLC stock 3164828, lifecycle normal, checked 2026-10-24
C15850 CL21A106KAYNNNE (Samsung Electro-Mechanics, 0805): basic, JLC stock 9140209, lifecycle normal, checked 2026-10-24
C23733 CL05A475MP5NRNC (Samsung Electro-Mechanics, 0402): basic, JLC stock 2672757, lifecycle normal, checked 2026-10-24
C23733 CL05A475MP5NRNC (Samsung Electro-Mechanics, 0402): basic, JLC stock 2672757, lifecycle normal, checked 2026-10-24
C1548 0402CG150J500NT (FH, 0402): basic, JLC stock 1048653, lifecycle normal, checked 2026-10-24
C19666 CL10A475KO8NNNC (Samsung Electro-Mechanics, 0603): basic, JLC stock 3886187, lifecycle normal, checked 2026-10-24
C2286 KT-0603R (Hubei KENTO Elec, 0603): basic, JLC stock 7833602, lifecycle normal, checked 2026-10-24
C64898 SM4007PL (MDD(Microdiode Semiconductor), SOD-123FL): basic, JLC stock 4700539, lifecycle normal, checked 2026-10-24
C52917434 WS2812B-2020-V6 (Worldsemi, SMD2020-4P): extended, JLC stock 829340, lifecycle normal, checked 2026-10-24
C19077393 BZT52B5V1 (R+O, SOD-123): preferred, JLC stock 1182376, lifecycle normal, checked 2026-10-24
C165948 TYPE-C-31-M-12 (Korean Hroparts Elec, SMD): extended, JLC stock 326056, lifecycle normal, checked 2026-10-24
C42411119 AOTA-B201610S3R3-101-T (ABRACON, 0806): extended, JLC stock 128, lifecycle normal, checked 2026-10-24
C25905 0402WGF5101TCE (UNI-ROYAL, 0402): basic, JLC stock 4234139, lifecycle normal, checked 2026-10-24
C25105 0402WGF330JTCE (UNI-ROYAL, 0402): basic, JLC stock 2217795, lifecycle normal, checked 2026-10-24
C25100 0402WGF270JTCE (UNI-ROYAL, 0402): extended, JLC stock 46522, lifecycle normal, checked 2026-10-24
C11702 0402WGF1001TCE (UNI-ROYAL, 0402): basic, JLC stock 19429365, lifecycle normal, checked 2026-10-24
C25900 0402WGF4701TCE (UNI-ROYAL, 0402): basic, JLC stock 14702148, lifecycle normal, checked 2026-10-24
C25741 0402WGF1003TCE (UNI-ROYAL, 0402): basic, JLC stock 7879401, lifecycle normal, checked 2026-10-24
C720477 TS-1088-AR02016 (XUNPU, SMD,4x3mm): basic, JLC stock 521909, lifecycle normal, checked 2026-10-24
C720477 TS-1088-AR02016 (XUNPU, SMD,4x3mm): basic, JLC stock 521909, lifecycle normal, checked 2026-10-24
C42411118 RP2350A (Raspberry Pi, QFN-60-EP(7x7)): extended, JLC stock 5789, lifecycle normal, checked 2026-10-24
C7519 USBLC6-2SC6 (ST, SOT-23-6L): extended, JLC stock 42329, lifecycle normal, checked 2026-10-24
C151007 AP7361C-33FGE-7 (DIODES, DFN-8-EP(3x3)): extended, JLC stock 1459, lifecycle normal, checked 2026-10-24
C473338 SN74LV1T125DBVR (TI, SOT-23-5): extended, JLC stock 161558, lifecycle normal, checked 2026-10-24
C97521 W25Q128JVSIQ (Winbond, SOIC-8-208mil): basic, JLC stock 63269, lifecycle normal, checked 2026-10-24
C20625731 ABM8-272-T3 (ABRACON, SMD3225-4P): extended, JLC stock 3906, lifecycle normal, checked 2026-10-24
```

Nine extended parts (RP2350A, AP7361C, USBLC6-2, SN74LV1T125, ABM8 crystal,
TYPE-C-31-M-12, AOTA inductor, WS2812B-2020, 27 Ω 0402; no basic 27 Ω 0402
exists). On economic PCBA that is a ≈ US$3 setup fee each per order,
≈ US$5.40 per board at quantity 5, before part and board cost. The total
against intent's US$15 target needs JLC's quote; it can't be priced from here.
The reviewer's estimate is ≈ US$14–15 on economic PCBA and ≈ US$19–20 on
standard. Whether economic PCBA accepts the 0.4 mm QFN (intent.md, rp2350-core
open question 5) shows on the quote: get it before ordering.

**L1 (AOTA-B201610S3R3-101-T, C42411119) stock moves fast**: 191 at phase 1,
1 at the first release check (2026-10-23), 128 today. It is the inductor
Raspberry Pi specifies for the RP2350 regulator, polarity-marked, with no
drop-in substitute (chips/rp2350.md). Order soon; if it has run out again,
use JLC global sourcing or a pre-order of the same MPN, or mark L1 DNP and
hand-fit five from Mouser or Digi-Key.

### CPL rotation: check in JLC's preview

KiCad 9 footprints at the CPL rotation; JLC's own package orientation differs
for some. Check pin 1 or polarity for each of these in the preview (silk dots
mark pin 1 on the board for U1, U3, U4, D2–D7, L1):

Ranked by consequence (review-release #11); rotations are 0–360 (fab.py
normalises KiCad's −90 to 270):

| Ref | Part | CPL rot | What to check |
|---|---|---|---|
| D3–D6 | WS2812B-2020 | 180 | pin 1 (DOUT, now bottom-right) chamfer at the dot. JLC's 2020-LED orientation is a known offender; a 180° error is silent until the pixels stay dark, and rework is on 0.7 mm pads |
| U1 | RP2350A QFN-60 | 0 | pin 1 at the top-left corner (dot) |
| L1 | AOTA 2016 polarised | 0 | dot at the left end (pad 1, 1V1). Abracon's drawing and Raspberry Pi's footprint disagree on the dot; the board follows Raspberry Pi |
| U3 | AP7361C DFN-8 | 90 | pin 1 at its footprint corner line (no room for a dot), EP centred |
| U4 | SN74LV1T125 SOT-23-5 | 90 | pin 1 dot; SOT-23 parts are the classic 180° offset |
| U2 | USBLC6-2SC6 SOT-23-6 | 0 | pin 1 dot (same) |
| U5 | W25Q128JV SOIC-8 208 mil | 90 | pin 1 dot |
| D2, D7 | SM4007PL / BZT52B5V1 SOD-123(FL) | 90 | band (cathode) toward the bottom, VLED |
| D1 | KT-0603R LED 0603 | 90 | cathode at the bottom; KT-0603R's marked end is the cathode, the opposite of its pin-① numbering |
| Y1 | ABM8 3225 | 0 | pin 1 corner; a 90° error shorts across GND pads |
| J1 | TYPE-C-31-M-12 | 180 | opening toward the board edge |

### Gerbers (gerbonara)

```
gerber_render: hw/easy2350/a/fab/gerbers (board easy2350)
  top_silk             easy2350-F_Silkscreen.gto          490 objects
  top_paste            easy2350-F_Paste.gtp               215 objects
  top_mask             easy2350-F_Mask.gts                260 objects
  top_copper           easy2350-F_Cu.gtl                  977 objects
  inner_1_copper       easy2350-In1_Cu.g1                 198 objects
  inner_2_copper       easy2350-In2_Cu.g2                 198 objects
  bottom_copper        easy2350-B_Cu.gbl                  486 objects
  bottom_mask          easy2350-B_Mask.gbs                 50 objects
  bottom_paste         easy2350-B_Paste.gbp                 0 objects
  bottom_silk          easy2350-B_Silkscreen.gbo         2965 objects
  drill_easy2350-PTH   easy2350-PTH.drl                   197 objects
  drill_easy2350-NPTH  easy2350-NPTH.drl                    2 objects
  outline              easy2350-Edge_Cuts.gm1               8 objects
  12 layer images in design/renders/gerbers
```

Outline closed (8 objects, the four corner arcs and edges); 197 plated holes =
138 vias + 59 plated pins and EP vias; 2 unplated (J1 pegs); paste only on top
SMD pads (bottom paste empty, nothing assembled there); silk present on both
sides; L2 and L3 solid planes with antipads; L1/L4 pours present.

## Final independent review

`review-release.md` (fresh-context reviewer, full package).

**2026-10-23: not yet.** One ordering BLOCKER (L1 stock 1) and four MAJORs:
BOOTSEL along the board edge by J2's hand-soldered joints, bring-up VLED
criteria for the wrong diode, an unreadable assembly drawing, and CC pins
without ESD (unlogged). Six MINORs and NOTEs (Pico silk line, stale docs,
KiCad stackup table, cost unproven, L1 dot convention, CPL list).

**Fixes:** edge strips and a full re-route; bring-up rewritten from the
audits; assembly drawing at ≈ 4:1; CC ESD waived with reasons; docs fixed;
stackup to select at order; CPL normalised and ranked. Found while fixing:
copper under every pixel's centre strip, so the pixels were turned 180° with
pre-routed 2–3 mm hops and no-copper rule areas (gates 6 and 7).

**2026-10-24 re-check: send it.** DRC (with schematic parity), ERC, sync,
nets, silk and placement checks re-run on the final files, all clean; every
earlier finding closed or soundly waived; new items NOTE only (pixel 1/2 GND
via the pour, the single VBUS via on the LED branch, GPIO0's escape by J1,
and the order-time actions below).

## Before ordering (user)

1. Get the JLC quote: economic or standard PCBA for the 0.4 mm QFN, and the
   cost against intent's US$15.
2. Select stackup JLC04161H-7628 on the order form.
3. Check pin 1 / polarity in JLC's assembly preview for the parts above,
   D3–D6 and L1 first.
4. Order while L1 is in stock (128 today), or plan the fallback above.
