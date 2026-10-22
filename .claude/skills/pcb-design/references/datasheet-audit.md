# Phase 1: architecture and datasheet audit

## Block diagram first

Before picking parts, draw the block diagram (Mermaid in `design/architecture.md`): connectors → protection → power conversion → loads, and signal paths between blocks. Every block will become a schematic sheet or region and a placement region.

## One audit file per part

`design/parts/<ref-or-function>.md` for every IC, connector, FET, magnetic, crystal, protection device, and for each **group** of passives that share a job (e.g. "buck output caps"). Generic 0.1 µF decouplers can share one file.

**Download the datasheet into `design/datasheets/` and read it.** Not the product page, not memory. If it can't be fetched, stop and ask the user to attach it. Read the whole thing for ICs: the fine print lives in footnotes, application sections, and layout guidelines.

### Template

```markdown
# U3 — DRV8353RS gate driver

Datasheet: datasheets/drv8353.pdf (rev E, 2023). Read: all sections.
MPN: DRV8353RSRTAR   Supplier: JLC C2846925 (extended, 412 in stock, checked 2026-10-17)
Footprint: VQFN-48 7x7 EP, matches package suffix "RTA" — verified against mechanical drawing p.112

## Absolute max vs our worst case
| Parameter | Abs max | Our worst case | Margin |
| VM | 102 V | 60 V (48 V + regen) | OK |

## Required external parts (from datasheet)
- VCP cap 1 µF 16 V X7R between VCP and VM (p.48) → C21
- CPH-CPL flying cap 47 nF, rated ≥ VM (p.48) → C22, 100 V
- ...

## Calculations
Show formula, datasheet page, inputs, result, chosen value, why.
- Gate drive current: Qg = 92 nC, target rise 100 ns → I = 0.92 A → IDRIVEP = 1 A (p.31)
- ...

## Fine print / gotchas
- Every footnote, "must", "should", "do not", recommended-vs-required distinction.

## Layout guidance from datasheet
- Copy the relevant layout recommendations; these feed placement (phase 6).

## Open questions
```

### What "calculations" means

Anything where a value is chosen: feedback dividers, current-sense shunts and their power, LED resistors, timing components, inductor selection (ripple, saturation current at max load + ripple/2, temperature), output cap ripple current and ESR, bulk cap ripple current, gate resistors, pull-ups for bus speed and capacitance, crystal load caps (including stray), fuse ratings, TVS standoff vs clamp voltage vs downstream abs max, connector current per pin with derating, MOSFET losses (conduction + switching) and junction temperature.

**Capacitors**: check DC-bias derating for MLCCs (a 10 µF 0603 at 12 V may be 3 µF), voltage rating with margin (≥1.5–2× for MLCC, more for electrolytics on switching rails), dielectric (C0G for timing/filters, X7R/X5R for decoupling, never Y5V), ripple current rating for bulk caps, and lifetime vs temperature for electrolytics.

**Resistors**: power with derating, voltage rating for high-voltage dividers, pulse rating for inrush/snubber parts, tolerance and tempco for sense resistors and references.

## Sourcing

Default to JLCPCB assembly. Prefer basic parts; each extended part adds a setup fee, so note it. Record LCSC number, stock count, and the date checked. Flag anything with low stock or "not recommended for new designs". For work projects, follow the user's internal part library instead.

## Gate checklist (phase 1)

- [ ] Block diagram exists and every block maps to parts
- [ ] Every non-trivial part has an audit file with datasheet in `datasheets/`
- [ ] Every abs max checked against worst case from `intent.md`
- [ ] Every chosen value has a shown calculation
- [ ] MLCC derating checked
- [ ] Every footprint matches the exact package suffix of the MPN
- [ ] Sourcing confirmed with date
- [ ] Independent reviewer re-read each datasheet and recomputed each calculation (see review-protocol.md)
