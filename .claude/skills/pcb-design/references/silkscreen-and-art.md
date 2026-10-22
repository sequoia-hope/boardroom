# Phase 9: silkscreen and art

Silkscreen is the board's user interface. Someone holding the board should know what it is, how to power it, and what every connector does, without the schematic.

## Board identity (required)

- Board name
- Revision and date (e.g. "rev B 2026-10")
- **Open source**: GitHub repo URL, and license (e.g. "CERN-OHL-S")
- **Work**: internal part number, company per policy; no public URLs
- Optional: author, a small QR code to the repo or docs

## Connectors (required, every one)

Each connector gets a reasoned label next to it:
- **Function**: "MOTOR", "CAN", "USB", "BATT IN"
- **Polarity / pinout** next to the pins: "+" and "−", signal names, pin 1 marker
- **Ratings**: "12–48 V DC", "60 V MAX", "5 A", "3.3 V LOGIC"
- **Warnings** where relevant: "NO REVERSE POLARITY PROTECTION", "HOT SURFACE"
Decide wording in a table in the gate file first (connector, label text, reason), then place.

## Designators

Policy from `brief.md`:
- **All**: every part, visible.
- **Selective**: connectors, ICs, test points, jumpers, LEDs, user-adjustable parts, anything referenced in docs.
- **None**: small hobby boards. Keep connector, LED and test point labels anyway. Generate an assembly drawing (fab layer) with designators so the board can still be debugged.

Every designator that is shown:
- adjacent to its own part, clearly associated (not closer to another part)
- **not under any part**, including its own
- **not crossed by vias or pads** (silk over exposed copper gets clipped by the fab)
- readable orientation: at most two rotations on the board (0° and 90°), consistent within a block
- consistent size (default 0.8–1.0 mm height, ≥ 0.15 mm line width; check JLC minimums)

## Overlaps: zero tolerance

No silk-to-silk, silk-to-pad, or silk-to-via overlap, and no silk over board edge. If two footprint outlines collide, **edit a board-local copy of the footprint** to trim the outline. Don't leave it.

Also: indicators for polarity (diodes, electrolytics, LEDs) and pin 1 on every IC and connector must remain visible after trimming.

## Art

Every board gets a little art in the art zone from `brief.md`.
- If the user supplies art (SVG/PNG), convert it to silk and/or copper/mask (KiCad Image Converter; Altium image import) at the reserved size.
- If Claude drafts it: propose 2–3 small concepts on the project page, related to the board's purpose, as simple line art. The user picks.
- Consider exposed copper (ENIG looks great), copper + mask openings, or silk on copper for contrast.
- Art follows the same overlap rules and stays out of keepouts and away from sensitive nets (exposed copper art is not grounded unless intended).

## Silkscreen check script (`scripts/silk_check.py`)

Computes the actual geometry (rendered text outline, not bounding box of the text anchor) and fails on:
- any silk-silk overlap
- silk over pads, vias, or exposed copper
- silk outside board or within copper-to-edge distance
- designator under a courtyard (including its own)
- designator whose nearest courtyard isn't its own part
- text below minimum height or line width
- connector without a label within X mm
- missing board identity items

## Gate checklist (phase 9)

- [ ] Connector label table with reasoning
- [ ] Board identity present
- [ ] Silk check passes (output pasted)
- [ ] Top and bottom silk renders reviewed by the stronger model
- [ ] Art placed and approved
- [ ] User reviewed silk render in PCB view
