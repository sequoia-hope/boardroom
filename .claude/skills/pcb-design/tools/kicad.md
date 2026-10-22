# KiCad adapter

Default for the user's open-source boards. Check the installed version first (`kicad-cli version`); APIs and file formats differ between 8, 9 and 10.

## Automation surfaces

- **`kicad-cli`**: ERC, DRC (`kicad-cli pcb drc --severity-all --format json`), exports (schematic PDF/SVG, gerbers, drill, BOM, CPL, STEP, 3D renders on recent versions). Use it for every check and export, and paste its output into gate files.
- **IPC API (kipy, KiCad 9+)** or legacy `pcbnew` Python: placement, zones, via arrays, net classes, footprint edits. Prefer the IPC API where available.
- **File formats**: `.kicad_sch` / `.kicad_pcb` are S-expressions. Parsing them directly is fine for checks (style, silk, sync scripts). For writes, prefer the API; if editing files directly, use a proper S-expression library, preserve UUIDs, and re-open in KiCad to validate.
- **Custom DRC rules**: `.kicad_dru` file with net class and area conditions. Put every derived rule here with a comment citing its source.
- **Freerouting** for autorouting: export DSN, run Freerouting headless, import SES. Lock pre-routed tracks first. Always run it governed: `nice`, `-mt` of half the cores or fewer, a heap and memory cap, and a time budget (see `references/routing.md`).
- **Image converter / bitmap2component** for silk art.
- **JLC**: use the JLC fields (`LCSC`) on symbols and a rotation correction (e.g. the JLCPCB fabrication plugin or a per-part rotation offset field) for CPL.

## Schematic authoring

When generating a schematic programmatically, the coordinates are the design. Plan the layout per block on paper first (a sketch in the gate file: which symbols where, left to right), then place symbols at those positions on the 1.27 mm (50 mil) grid, then wires. Never place symbols in a loop over a list with a fixed x/y step: that is what produces the grid look.

## Board-local footprint edits

Copy the footprint into a project library (`<project>/lib/<project>.pretty`), edit, and re-link the symbol's footprint field in the schematic (schematic remains source of truth). Note it in `decisions.md`.
