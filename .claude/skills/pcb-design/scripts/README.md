# Check scripts

These are the automated checks the gates rely on. They're **specs, not yet implemented**. Build them in this order, since each one removes a category of babysitting:

| Priority | Script | Used at | Catches |
|----------|--------|---------|---------|
| 1 | `sync_check.py` | 6–10 | schematic / PCB / BOM mismatches in value, footprint, MPN, LCSC, DNP |
| 2 | `silk_check.py` | 9 | silk overlaps, silk on pads/vias, hidden or misassociated designators, unlabeled connectors |
| 3 | `nets_check.py` | 3, 5 | nets missing from `nets.csv`; derives widths (IPC-2152), via counts, clearances; checks tool net classes match |
| 4 | `placement_check.py` | 6 | passives in rows/grids, decaps far from power pins, crystals far from MCU, switching loop area, ratsnest score |
| 5 | `schematic_style_check.py` | 2 | grid-placed symbols, labels where wires belong, rotated power/ground symbols, missing fields |
| 6 | `current_check.py` | 7 | narrowest copper neck and via count per power net vs `nets.csv` |
| 7 | `plane_check.py` | 7 | GND plane splits/slots, high-speed traces crossing them |
| 8 | `crosstalk_check.py` | 8 | aggressor/victim parallel runs |
| 9 | `cpl_check.py` | 10 | renders CPL overlay for rotation/pin 1 verification |

## Conventions

- Input: KiCad files directly (S-expression parse) or IPC-2581 export (works for both KiCad and Altium; prefer this for tool-independent checks).
- Output: human-readable summary plus JSON; exit code non-zero on failure. Gate files paste the summary.
- Thresholds in a project-level `design/checks.yaml` with defaults here, so a project can loosen one with a logged reason.
- Every script has a small test fixture: one board that passes and one that fails for each rule.

## Note

The `assembly-doc-generator` IPC-2581 parsing work could be shared here: the same parser serves silk, placement, and sync checks for both tools.
