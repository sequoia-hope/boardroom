# Phase 6: placement

Placement determines most of the board's quality. Good placement makes routing nearly automatic; bad placement can't be fixed by routing.

## Order

1. **Fixed items** from `brief.md`: outline, mounting holes, connectors, keepouts, art zone. Lock them.
2. **Blocks in floorplan order**: work from connectors inward, following power and signal flow. Place each block as a unit before moving to the next.
3. **Within a block, critical loops first**:
   - switching power stages: input cap, switch, diode/low-side FET, ground return as one tight loop; then inductor, then output caps; feedback divider near the IC's FB pin, routed away from the switch node
   - half-bridges: high-side and low-side FETs, bulk and ceramic caps across the bridge, shunt, all in the tightest possible loop
   - gate drivers right beside their FETs
   - crystals right at the oscillator pins
4. **ICs** in their block region, oriented so their pins face what they connect to.
5. **Supporting passives with their pins**: decoupling caps at each power pin (smallest value closest), pull-ups at their pins, termination resistors at the driver (series) or receiver (parallel).
6. **Test points and debug features.**
7. **Silkscreen space**: leave room for designators and connector labels now, not after routing.

**Follow the datasheet's layout guidelines** from each `parts/*.md`. If the datasheet shows a recommended layout, reproduce it.

## Rules

- **Never** place passives in a row, a grid, or any arrangement that is about tidiness rather than function. Every passive belongs to a pin.
- Align parts within a block where it costs nothing (neat rows of decoupling caps along an IC edge are fine), but function wins over neatness.
- Consistent orientation for polarized parts within a block helps assembly inspection.
- Keep switching nodes and high-dV/dt copper away from analog, clocks and board edges.
- Keep the ground return path in mind for every high-current loop: where does the current come back?
- Hot parts: spread out, near copper area and thermal vias, away from electrolytics and temperature-sensitive parts.
- Parts that are hand-soldered or reworked need clearance around them.
- Fiducials if assembly needs them (JLC generally doesn't for standard assembly, check).

## Placement check script (`scripts/placement_check.py`)

Fails on:
- passives whose position clusters on a single line or grid unrelated to their connected IC (detect: many same-type passives with collinear centroids and nearest-connected-pad distance above threshold)
- decoupling cap farther than X mm (default 3 mm) from the nearest power pin on its net
- crystal and load caps farther than X mm from oscillator pins
- courtyard overlaps
- parts outside board or in keepouts
- ratsnest: total and per-net ratsnest length as a placement quality score (compare variants)
- parts in `nets.csv` SWITCH/SWITCH_HI loops with loop area above threshold (approximate from pad positions)

## Visual review

Render top/bottom 2D and 3D with the ratsnest visible. Reviewer answers:
1. Does each block sit together, near its connectors, in a sensible flow?
2. Is every passive with the part it serves?
3. Are the critical loops tight?
4. Is noisy separated from sensitive?
5. Is there room for routing, vias, and silkscreen?
6. Would an experienced layout engineer place it like this?

## Gate checklist (phase 6)

- [ ] Fixed items match `brief.md` and are locked
- [ ] Placement check passes (output pasted)
- [ ] Datasheet layout guidelines followed per part (list)
- [ ] Visual review passed
- [ ] User reviewed placement render in PCB view
