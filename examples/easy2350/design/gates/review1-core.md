# Gate 1 review — RP2350A core block (`parts/rp2350-core.md`)

Independent review, 2026-10-23. Reviewer did not write the audit. Everything
below was re-derived from the PDFs in `design/datasheets/`, the vendor KiCad
archive (`RP-006440-DD-2`), the schematic netlist
(`kicad-cli sch export netlist --format kicadxml`) and `jlc_parts.py`; the
audit's numbers were not trusted.

Page numbers are printed page numbers (DS pdf page = printed + 1; HDG pdf page
= printed + 1).

## Verdict

No BLOCKER. One MAJOR (the schematic does not do what the audit says it does
about the power pins). The datasheet reading, the arithmetic, the part choices,
the footprints and the sourcing all check out. Pass the gate once #1 is
resolved one way or the other and the MINORs are either fixed or logged.

## Findings

| # | Severity | Finding (with source) | Suggested fix |
|---|---|---|---|
| 1 | **MAJOR** | **Schematic contradicts the audit on the U1 symbol.** The audit (Pin map section, Deviation 7, Schematic connections: "wire each decoupler to its own U1 pin (this is why the symbol's power pins are unstacked)") specifies a project symbol with IOVDD and DVDD unstacked. The project symbol `hw/easy2350/a/lib/easy2350.kicad_sym` still has IOVDD 11/20/30/38/45 and DVDD 23/39 as **hidden passive pins stacked** on pins 1 and 6; the eight 3V3 decouplers (C4–C11) and C16–C18 are drawn as banks on the rails with a text note giving the pin assignment. Electrically fine (ERC clean, netlist correct), but nothing in the schematic binds C10 to pin 44, C11 to 53/54, C16 to pin 23, and the audit's own rationale (LESSONS: edit the symbol if the library one fights readability) is not implemented. | Either unstack the pins as the audit says (the vendor `RP2350_60QFN` symbol in the archive already does; same pin numbers, checked) and draw each decoupler at its pin, or amend the audit (Deviation 7 and Schematic connections) to describe the note-based scheme. Do not leave the two disagreeing. Whichever is chosen, placement (phase 6) needs the cap→pin table; today it exists only in the audit and the note. |
| 2 | MINOR | **Net names do not match `nets.csv` or the audit.** XIN and XOUT are auto-named `Net-(U1-XIN)` / `Net-(U1-XOUT)` in the netlist, but `design/nets.csv` lists `XIN` and `XOUT` (class CLOCK, 5 mm limit); a class check keyed on names will miss them. The audit's proposed `BOOTSEL_SW`, `RESET_SW`, `SWCLK_MCU`, `SWDIO_MCU` (open question 8) are neither labelled (`Net-(R7-Pad2)`, `Net-(R8-Pad2)`, `Net-(U1-SWCLK)`, `Net-(U1-SWDIO)`) nor in `nets.csv`, so the 100 Ω stubs at pins 24/25 have no length rule. | Add net labels XIN, XOUT, SWCLK_MCU, SWDIO_MCU, BOOTSEL_SW, RESET_SW in the schematic; add the four stub nets to `nets.csv` (SWCLK_MCU/SWDIO_MCU: SIGNAL, short; BOOTSEL_SW/RESET_SW: SIGNAL). |
| 3 | MINOR | **Gate checklist "every datasheet listed in `sources.csv`" not met.** `sources.csv` has no row for the Fenghua 0402CG150J500NT (C1548) or the UNI-ROYAL 0402WG series (C11702 / C25105 / C25076), which the audit cites as FH p.4–5 and UR p.4–5 (audit open question 7). The Samsung per-part data (DC bias, TCC, |Z|/ESR) that Calcs 2–4 rest on is web-only and not saved. | Add `fh-0402cg-mlcc.pdf,C1548` and `uniroyal-0402wg.pdf,C11702` (both fetch with `jlc_parts.py fetch`; verified today that C25105 fetches a 456 kB PDF). Save the Samsung pages (their "Download This Page (.PDF)" button) or the extracted JSON for CL05A475MP5NRN and CL05B104KO5NNN under `datasheets/` and cite the URL + date. |
| 4 | MINOR | **Crystal: the audit's layer-2 advice points the wrong way.** Calc 5 / open question 10 propose "avoiding ground pour on layer 2 directly under XIN/XOUT" to keep stray near HDG's assumed 3 pF (HDG p.13). Re-estimate: a 0.2 mm trace over a ~0.2 mm prepreg is roughly 0.08–0.1 pF/mm, so the ~3 mm XIN/XOUT runs add ≈ 0.3 pF each — inside the slack of an assumption that already includes the pin and pad capacitance, and small against the 10 pF CL (designed 10.5 pF). Removing the reference plane under the oscillator traces would raise their coupling to VREG_LX (5 mm away) and QSPI_SCLK, which matters more than 0.3 pF. | Keep layer 2 solid under Y1, C20, C21, R6 and the XIN/XOUT traces; confine the layer-2 cut-out to the VREG_LX zone and L1's LX pad as DS p.412 Figure 27 requires. Keep the bring-up check (XOSC STATUS_STABLE at 0 °C and 40 °C). |
| 5 | MINOR | **TP3 (1V1 test point) is not placed by the audit.** intent.md requires a 1V1 test point; the schematic has TP3 on +1V1; the chip note and the audit's layout section say 1V1 "never leaves this area and is never a thin wire between spoke and ring", but say nothing about where a 1.5 mm pad attaches. A stub off L1/COUT would sit in the high-current loop (DS p.411 "keep the loop area … as small … as possible"). | Attach TP3 at the pin-23 end (next to C16 / the S spoke via at (0.20, 5.80)), never at L1 pad 1 or COUT. Add a line to the audit's layout table. |
| 6 | MINOR | **33 Ω tolerance mis-stated.** The audit calls C25105 (0402WGF330JTCE) "33 Ω ±5 %" and lists it as Deviation 5 from the reference's 1 %. UNI-ROYAL's code (datasheet fetched for C25105, §2.3): 7th character F = ±1 %; "330J" = 330 × 10⁻¹ = 33 Ω. The part is **±1 %**, the same as REF's R5. | Correct the Parts table, Calc 3 and Deviation 5 (the 33 Ω row of Deviation 5 disappears; the 1 kΩ row was already 1 %). No hardware change. |
| 7 | NOTE | **Inductor polarity — confirmed, and the disagreement is real.** Vendor `.kicad_pcb`: L1 at (2.00, −7.20) rel. to U1, rotation 0, **pad 1 net +1V1, pad 2 net /VREG_LX**; `L_pol_2016` puts the silk/fab dot on pad 1 (x = −0.7). Our netlist: L1.1 = +1V1, L1.2 = /VREG_LX; our footprint is byte-identical to the vendor's. DS Figure 28 (p.413): current enters "+", leaves "−", orientation dot at the "−" end — in a buck the inductor current leaves toward VOUT, so dot = 1V1 end, consistent with Figure 25 (p.410, dot at the VOUT end of Lx) and the vendor board. AOTA p.4 draws the dot at the "+" terminal with I flowing + → −, i.e. dot at the current-entry end. The two sheets disagree; the vendor board is the validated artefact, so following RP is right. | Keep as drawn. Carry open question 1 into the fab/assembly notes: check the JLC placement preview shows the white dot on pad 1 (the 1V1 / COUT side), and measure 1V1 under load on a first article. Consider asking RP/Abracon which drawing is wrong, for the LESSONS file. |
| 8 | NOTE | **Calculations re-derived — all agree.** Ripple 72.9 / 75.2 mA (3.20 / 3.40 V), Ipk 238 mA, DCR loss 5.6 mW, CIN RMS 95 mA, output ripple ≈ 1.1 mV; VREG_AVDD drop 7.26 mV → 3.193 V (57.7 mV over 3.135 V; with R at +1 % still 57.6 mV); RC corner 1.98 kHz typ / 2.51 kHz worst, 61.5 dB at 3 MHz, τ 80 µs; BOOTSEL/RUN node 0.100–0.038 V vs VIL 0.8 V (RPU 32–86 kΩ, DS Table 1683); 3.3 mA / 10.9 mW with the button held; Tj 53.4 °C (θJA 40.542, Table 1670); regulator input 113 mA at 200 mA/59 % (Table 1689); ESL 0.26 nH from the 4.53 MHz |Z| minimum; SWD 16.5 mA / 27 mW. Samsung data re-fetched 2026-10-23 and parsed from the page JSON: CL05A475MP5NRN −9.5 % at 1.1 V, −48.3 % at 3.3 V, −49.7 % at 3.4 V, R 4.8 mΩ and \|Z\| 12.0 mΩ at 3 MHz, \|Z\| minimum 6.1 mΩ at 4.53 MHz; CL05A475MQ5NRN −14.6 % / −59.4 %; CL05B104KO5NNN −3.9 % at 3.3 V, R 24.9 mΩ at 3 MHz, \|Z\| minimum at 32.7 MHz. One slip: Calc 2's "−1.5 % at 0 °C" has the sign and end of range swapped — Samsung's TCC is **+1.5 % at 0 °C and −1.5 % at 40 °C**; the worst case over 0–40 °C is at 40 °C, same magnitude, no change to any result. | Fix the TCC wording in Calc 2. |
| 9 | NOTE | **Required-parts list independently rebuilt from DS §6.1 (pp.400–401), §6.3.7–6.3.8 (pp.407–413), §8.2.1.1 (p.507), §5.2.8 (p.345), §7.3 (p.452) and HDG pp.5–9, 13–14:** L 3.3 µH ±20 % shielded, DCR ≤ 250 mΩ, Isat ≥ 1.5 A, polarity-marked; COUT 4.7 µF ±20 %, ESR ≤ 250 mΩ, ESL ≤ 6 nH; CIN ≥ 4.7 µF, ESR ≤ 50 mΩ; VREG_AVDD RC 33 Ω + 4.7 µF (mandatory, p.411); 100 nF at the two DVDD pins nearest the regulator and 4.7 µF at the farthest (p.400; pin distances from pin 48: 39 → 3.3 mm, 6 → 6.0 mm, 23 → 7.2 mm, so pin 23 is the far one); 100 nF at USB_OTP_VDD and at ADC_AVDD (p.401); 100 nF per IOVDD pin (HDG p.8); ABM8-272-T3 with 2 × 15 pF and 1 kΩ at IOVDD = 3.3 V (HDG p.13); 1 kΩ BOOTSEL (HDG p.10) or ≥ ~4.7 kΩ (DS p.345); RUN tied high if unused (p.452). Every item is present in the netlist with the audited value, voltage and dielectric (CL05A475MP5NRNC 10 V X5R ×4; CL05B104KO5NNNC 16 V X7R ×8 on 3V3 + ×2 on 1V1; 0402CG150J500NT C0G 50 V ×2; 1 kΩ ×3; 100 Ω ×2; 33 Ω ×1). AOTA p.1 confirmed: 3.3 µH, DCR 115/140 mΩ, Isat 2.8/2.4 A, Irms 2.3/2.1 A, SRF 21 MHz min, 20 V, −40…125 °C incl. self-heating, MSL 1; p.2 curve ≈ 3.2 µH at 0.8 A. ABM8 p.1 identical to DS Table 629. | None. |
| 10 | NOTE | **Footprints verified against the drawings.** `RP2350-QFN-60-…_ThermalVias` and `C_0402_1005Metric_small_pads` in the project library are identical to the vendor's (diff, ignoring uuids). U1 pads 0.875 × 0.20 at 3.4375 give 7.75 outer / 6.00 inner spans, 0.40 pitch, 3.40 EP: exactly DS Figure 148 (p.1325). Crystal: ABM8 p.2 bottom view has the element across #1/#3 (diagonal); KiCad `Crystal_SMD_3225-4Pin` pads 1 (−1.1, +0.85) and 3 (+1.1, −0.85) are the same diagonal (the ABM8 top view rotated 180°), pads 2/4 to GND as in the vendor netlist; land 1.4 × 1.2 at ±1.1 / ±0.85 vs ABM8 1.4 × 1.1 at ±1.1 / ±0.8. `L_pol_2016` pads 0.7 × 1.7 at ±0.7 cover AOTA's 0.6 ± 0.2 terminals (nominal 0.4–1.0 from centre) with a 0.05 mm toe past the nominal body edge (audit says "no outside toe"; immaterial). | Cosmetic only: the copied `L_pol_2016` still carries `(property "Sheetfile" "RP2350_60QFN_minimal.kicad_sch")`, the fab value text "4.7u" and a 3D-model path under `KICAD7_3DMODEL_DIR`; clean these when convenient. |
| 11 | NOTE | **DS Table 1679 (p.1334) error confirmed:** it lists IOVDD as "11, 20, 30, 38, 45, 54" and QSPI_IOVDD as "54"; Figure 152, the vendor symbol and our symbol all have pin 1 = IOVDD, 54 = QSPI_IOVDD. Our netlist has U1.1 on +3V3. | None. |
| 12 | NOTE | **HDG Figure 6 (p.8) disagrees with the vendor KiCad files:** the figure shows C8, C10, C11 all as 100 nF on +1V1, whereas the KiCad archive has C10 = 4.7 µF at pin 23 and DS §6.1.3 (p.400) asks for 4.7 µF at the far DVDD pin. The audit follows DS + KiCad (C16 = 4.7 µF at pin 23), which is right. | Record the discrepancy in the audit's gotchas so the next reader doesn't "correct" it from the HDG figure. |
| 13 | NOTE | **VREG_AVDD current assumption.** Calc 3 uses 212–220 µA (Table 1693). DS p.409 says the regulator "will consume approximately 400 µA" (context: when left running with an external DVDD). If that figure applied to the AVDD pin, the drop would be 13.2 mV → 3.187 V, still 52 mV above 3.135 V. Margin stands either way. | Optional: mention the 400 µA sentence in Calc 3 as the bounding case. |
| 14 | NOTE | **RUN resistor topology** differs slightly from the vendor's: REF is RUN → SW2 → R4 1 kΩ → GND; ours is RUN → R8 1 kΩ → SW2 → GND. Electrically identical for the divider; ours puts the resistor between the finger and the pin, which is the better side for ESD. BOOTSEL matches REF (R6 at the QSPI_SS side). | Reword "as REF" in the RUN paragraph. |
| 15 | NOTE | **CIN / COUT in-circuit capacitance** (audit open question 3). Agreed with the audit: DS p.412's "at least 4.7 µF" / "4.7 µF ±20 %" are met as nominal ratings; in circuit CIN ≈ 2.4 µF typ at 3.3 V and COUT ≈ 4.25 µF typ / 3.35 µF worst at 1.1 V. The vendor's 6.3 V part (HDG p.19: "4.7u 6.3V X5R 20%") would be ≈ 1.9 µF and ≈ 4.0 µF on the same basis, so the validated board is no better. No change. | Leave as logged; do not add a second CIN (it would break the copied geometry). |
| 16 | NOTE | **Sourcing re-run 2026-10-23 with `jlc_parts.py show`** on all ten LCSC numbers: library type, lifecycle and MPN match the audit's lines; stock moved slightly (L1 191, Y1 3383, 4.7 µF 2 708 184). Three extended parts in this block (U1, L1, Y1) plus the 27 Ω pair (C25100), as stated. L1's 191 pieces remain the thin spot (open question 2). | Re-check L1 stock at order time. |

## What was checked and found clean

- Netlist vs the audit's Schematic connections tables: every U1 pin on the
  stated net; L1.1 = +1V1, L1.2 = VREG_LX; VREG_FB (50) on +1V1; PGND (47) on
  GND; Y1.1 = XIN, Y1.3 = XOUT_R, Y1.2/4 = GND; R6 1 kΩ between XOUT and XOUT_R;
  R13/R14 100 Ω between U1.24/25 and J4.1/3; R7 1 kΩ QSPI_SS → SW1; R8 1 kΩ
  RUN → SW2; R3 33 Ω 3V3 → VREG_AVDD with C13 4.7 µF; C14 (CIN, small pads) on
  3V3, C15 (COUT, small pads) and C16 (4.7 µF) on 1V1; eight 100 nF on 3V3 for
  U1, two on 1V1. ERC: 0 errors, 0 warnings.
- Absolute-maximum table: IOVDD/QSPI_IOVDD 3.63 V abs max = recommended max
  (Table 1680 / 1688) — the audit's warning to `ldo-3v3.md` about overshoot is
  justified. FT pins 3.63 V with IOVDD = 0 V; GPIO26–29 are "Digital IO /
  Analogue", not FT (Table 1674). RPU 32–86 kΩ, IIOVDD_MAX 100 mA,
  IQSPI_IOVDD_MAX 20 mA (Table 1683). Regulator: IMAX 200 mA, ILIMIT 260/500/800
  mA, fsw 3 MHz, δVOUT ±3 %, efficiency 59 % at 200 mA / 3.3 V (Table 1689).
- Vendor positions in the audit's layout table (L1, C6/C7/C9/R5, C10, R2,
  C3/C4, Y1, C12, C17, R7/R8) re-read from the `.kicad_pcb` relative to U1 at
  (100, 100): all match to 0.01 mm, rotations included.
- Pin-1 and symbol pin numbering of the project `RP2350A` symbol against DS
  Figure 152 and the vendor `RP2350_60QFN` symbol: identical.

## Files read

`design/parts/rp2350-core.md`; `design/intent.md`; `design/nets.csv`;
`design/datasheets/sources.csv`; `.claude/skills/pcb-design/chips/rp2350.md`;
`references/datasheet-audit.md`; `hw/easy2350/a/easy2350.kicad_sch`,
`lib/easy2350.kicad_sym`, `lib/easy2350.pretty/*`; vendor archive
`RP2350_60QFN_minimal.kicad_sch` / `.kicad_pcb` / `.pretty`; DS pp.344–345,
400–414, 450–452, 506–508, 1324–1325, 1330–1344 (text and rendered figures 25,
28, 148); HDG pp.5–10, 13–14, 18–19 and Appendix A; AOTA pp.1–4 (rendered);
ABM8 pp.1–2 (rendered); UNI-ROYAL 0402WG datasheet (fetched for C25105);
Samsung product pages for CL05A475MP5NRN, CL05A475MQ5NRN, CL05B104KO5NNN
(fetched 2026-10-23, JSON parsed).
