# Phase 2 review: schematic (independent visual + netlist review)

Reviewer: review2-sch, 2026-10-23. Inputs: `hw/easy2350/a/easy2350.kicad_sch`
rendered to PDF/PNG (whole sheet at 100 dpi, each region at 220 dpi), the
kicadxml netlist, `design/intent.md`, `design/parts/*.md` ("Schematic
connections"), `schematic-style.md`, `LESSONS.md`.

Checks run by the reviewer (not copied from the author):

- `kicad-cli sch erc --severity-all`: **0 errors, 0 warnings**.
- `schematic_style_check.py`: **PASS, 0 errors, 8 warnings**, all of the
  same kind: C4–C9, C17, C18 are 42–98 mm from the stacked IOVDD/DVDD pin
  they decouple, so placement relies on the `Decouples` field (see #1).
- Netlist vs. the six audits' connection tables: every listed pin-to-net
  binding matches (USB front end incl. U2 I/O1 = D−, I/O2 = D+; LDO incl.
  EN→VBUS and EP→GND; all 61 RP2350 pins; flash; crystal with R6/XOUT_R;
  regulator L1/R3/C13–C16; pixels, U4, D2, D7, R11, R12, R10; buttons;
  D1/R9; J2/J3 pin map incl. 7 GND pins, RUN at J3.10, 3V3 at J3.16, VBUS at
  J3.20; J4). Values, footprints, MPN and LCSC match the audits for every
  assembled part. J2/J3/J4/TP3 are `in_bom no`, as the audits require.
- Labels: 98 local labels, 0 global labels. Every label name is a net name
  in `nets.csv` style; all cross-block nets are labelled.

## Findings

| # | Severity | Finding (location) | Suggested fix |
|---|---|---|---|
| 1 | MINOR | The eight 3V3 decouplers C4–C11 are a row along the 3V3 bus between U3 and U1 (region "RP2350A, FLASH, CRYSTAL", top left), 40–100 mm from the pins they serve, because the project symbol keeps IOVDD ×6 and DVDD ×3 stacked. The style rule is "decoupling caps drawn at the IC's power pin"; the author substitutes a text note plus a `Decouples` field on every cap. Judgement: acceptable for this gate. The row is inside the block, on the wire that feeds U1, the note names which cap belongs to which pin, and the field is machine-checked at placement — that is a stronger binding than most hand-drawn schematics give. It still reads as a list rather than a circuit, and the style guide's own remedy (a project symbol with the power pins unstacked, or a separate power unit) is available since the symbol is already a project copy. | Either (a) unstack IOVDD 1/11/20/30/38/45 and DVDD 6/23/39 on the project symbol's top edge and hang each cap on its pin, or (b) keep the row but move it to abut U1's top edge directly above pins 1/49/53/54 so the eye reads "these caps are U1's". Record the choice in LESSONS.md so later boards don't re-litigate it. |
| 2 | MAJOR | **C28 straddles two regions.** The pixel-4 cap C28 (VLED, 100 n) is drawn at the far right of the RGB box, and its reference/value text ("C28 100n") lands *inside* the BUTTONS box, on top of that box's dashed left border and beside the `QSPI_SS` label of R7 (whole-sheet image ≈ x 820, y 895; btn crop top-left). A reader of the BUTTONS region sees a capacitor that has nothing to do with buttons. | Shift the whole RGB rail left (there is 25 mm of free space at the left of that box) or widen the RGB box and move the BUTTONS box right; C28 and its text must sit entirely inside the RGB box, next to D6's VDD pin. |
| 3 | MINOR | C1 (100 n, `Decouples: U2.5`, the audit's C_ESD_VBUS "at U2 pin 5") is drawn beside C2 at the LDO input, not at U2. U2's VBUS pin 5 rises to the VBUS wire with nothing beside it. The audit's layout rule is "C_ESD_VBUS within 1 mm of pin 5". | Move C1 to hang off the VBUS wire directly above U2 pin 5 (x ≈ 600 in the usb crop), so the drawing says what the field says. |
| 4 | MINOR | Text overlaps, all legible but unprofessional: (a) `VBUS` power symbol and `PWR_FLAG` at J1 A4 read as "VBUS PWR_FLAG" (one string); (b) `PWR_FLAG` and `1V1` at the 1V1 rail end read "PWR_FLAG1V1"; (c) U3's value "AP7361C-33FGE" collides with the `+3V3` power symbol text on pin 1; (d) `LED_DIN` label runs into D3's pin number 3 ("LED_DIN3"); (e) D7's value "BZT52B5V1" runs into C23's plates; (f) SW1/SW2 values "BOOTSEL"/"RESET" cross the switch lever; (g) `VREG_AVDD` label sits directly above the identical pin name inside U1 and reads as a doubled word. | Offset each PWR_FLAG 2.54 mm from the rail symbol it validates; move U3's value below the body; nudge the `LED_DIN` label 2.54 mm left; put D7's value on its left; set switch values below the symbol; drop the `VREG_AVDD` label (the wire is local to the block, R3 and C13 are on it) or move it to the R3 side. |
| 5 | MINOR | Crystal / SWD corner is cramped (core crop, around U1 pins 22–25, x 420–600, y 620–720): `XOUT_R` label touches R6's left end; C21's "15p" and R13's "100R" sit on adjacent lines just above the `SWCLK` label; R13/R14 references and values are placed above/below the stacked pair so "R13" reads as C21's neighbour. It is decodable but a reviewer has to stop and sort out which value belongs to which part. | Move R13/R14 down 5 mm so the SWD pair sits clear of the crystal; put R13's and R14's reference and value both to the right of each body; lengthen the XOUT_R wire so the label clears R6. |
| 6 | MINOR | Rail labels vs. power symbols are mixed: J3.16 uses a *label* `+3V3`, J3.20 a *label* `VBUS`, J4.2 a *label* `GND` (sch line 11049), while everywhere else the same nets use power symbols. The netlist is correct (same-sheet labels merge with the power nets), but a reader expects a power symbol for a rail on a connector and may assume a label named `+3V3` is a different net. | Replace those three labels with `+3V3`, `VBUS` (up) and `GND` (down) power symbols. |
| 7 | MINOR | C15 (COUT) has no `Decouples` field; the core audit flags the same and says to give it `U1.50`. C24 (the audit's C_U4, "at U4 pin 5") and C25–C28 (one per pixel VDD) also have no `Decouples` field, although the audit binds each to a pin. The style check only credits 17 of 26 caps. | Set `Decouples` = `U1.50` on C15, `U4.5` on C24, `D3.4`…`D6.4` on C25–C28 (C23 is bulk and legitimately unbound). |
| 8 | MINOR | Title block has no author (style: "board name, revision, date, author, sheet title"). Company field carries the repo URL, comment 1 the description, comment 2 the licence — good — but the person responsible is absent. | Add the author to `company` or `comment 3`, and set `comment 4` or the sheet title to the sheet's function. |
| 9 | MINOR | J2's GND symbol is drawn on the GPIO HEADERS box's left dashed border (hdr crop, x ≈ 14, y 575): the symbol and its wire overlap the frame. | Move the J2 GND bus 5 mm right, inside the frame. |
| 10 | NOTE | The RP2350 block's title "RP2350A, FLASH, CRYSTAL" and the four-line decoupling note immediately under it are set at the same x with ~1 mm between the title's baseline and the note's cap height; the note is a different font size and colour so it reads, but the note is the first thing a reader hits in the block. | Move the decoupling note to sit beside the C4–C11 row it explains (or under it), leaving the title alone. |
| 11 | NOTE | Designator sequence has gaps (C12, C19 absent; parts go C11→C13, C18→C20). Harmless, but a BOM reader will look for them. | Re-annotate before phase 3 if the generator can keep the `Decouples` bindings through it; otherwise leave and note it in the BOM. |
| 12 | NOTE | `parts/user-io.md` lists TP1 (VBUS), TP2 (3V3) and TP4 (GND) in its connection table; the schematic has only TP3 (1V1), which is what `intent.md` and `ldo-3v3.md` specify (VBUS/3V3/GND are probed on header pins 40/36/38). The schematic is right; the audit's table is stale. | Update `user-io.md`'s table to TP3 only (not a schematic change). |
| 13 | NOTE | The headers' current limits (3V3 out 100 mA, VBUS out 50 mA, no back-feed protection) that the intent requires on the silkscreen are not annotated at J2/J3. The USB-C, LDO and RGB blocks have good rating notes; the headers have only the pin-map note. | Add one line under J3: "3V3 out 100 mA max; VBUS out 50 mA max; no back-feed protection on VBUS". |

Answers to the review questions:

1. **Power path:** yes. J1 VBUS → (C1, C2) → U3 → 3V3 → C4–C11 row → U1 IOVDD/ADC/OTP/QSPI pins, R3 → VREG_AVDD, VREG_VIN with C14; U1 VREG_LX → L1 → 1V1 → C15–C18 → VREG_FB/DVDD and TP3; 3V3 → U5 VCC via power symbol; VBUS → D2 → VLED → U4 and each pixel; VBUS/3V3 to J3. Voltage runs top-to-bottom, the rail flows left-to-right.
2. **Signals:** yes. USB D± from J1 through U2 (in on one pin, out on the other, drawn in-line) → 27 Ω at the chip → U1; QSPI as six straight wires U5 → U1 with labels doubling as net names; XIN/XOUT with R6, C20/C21 at the crystal; SWD through R13/R14 to J4; GPIO23 → U4 → R10 → D3 → D4 → D5 → D6 left-to-right with the last DOUT NC-flagged; GPIO25 → R9 → D1; QSPI_SS and RUN through 1 k to the buttons.
3. **Grid/list look:** only the C4–C11 row (#1) and, to a lesser degree, C15–C18 — both are on the bus that feeds the chip, so they read as "the chip's decoupling", not as a parts dump. The headers are lists by nature and are drawn in physical top-view order with GND bussed, which is correct.
4. **Passives next to what they serve:** yes, with the two exceptions above (C1 at the LDO instead of U2 pin 5, #3; the stacked-pin decouplers, #1). R_CC at the CC pins, 27 Ω at U1, crystal caps at Y1, R12 at U4's input, C24 at U4 VCC, a 100 n at each pixel, R7/R8 at the chip end as the audit asks.
5. **Cramped/overlapping:** #2 (cross-region overlap, the one thing that must change), #4, #5, #9, #10.
6. **Would a senior engineer present this:** after fixing #2 and the text nudges, yes. The sheet is organised in flow order, every block has a title and a rating or layout note, nets are named consistently, and the symbol pin arrangement (power top, GND bottom, inputs left, outputs right) is right for U1, U5, U3, U4 and the pixels.
7. **Netlist vs. audits:** no missing or wrong connection, value, footprint, MPN or LCSC found. Field gaps: #7. Audit staleness: #12.

## Verdict

**PASS with required fixes before the gate closes: #2 (MAJOR).** The netlist
is correct against all six audits and ERC is clean at the strictest
severity; the drawing is a genuine circuit that a reader can follow from
the USB-C receptacle to every load and from every signal source to its
destination. The one defect that must be corrected is C28's text sitting
inside the BUTTONS region; the rest are text-placement and field-hygiene
items (#3–#9) that take minutes in the generator and should be done in the
same pass. The stacked-pin decoupling compromise (#1) is accepted for this
board because the note and `Decouples` field make the binding explicit and
checkable, but the choice should be written into LESSONS.md so it is a
decision, not an accident.
