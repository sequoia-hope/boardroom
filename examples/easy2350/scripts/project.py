#!/usr/bin/env python3
"""project.py -- write the design rules and net classes into easy2350.kicad_pro.

Every number comes from design/stackup.md (fab limits, JLC 4-layer) or
design/nets.csv (per-class widths and clearances). nets_check.py --rules
checks the result against nets.csv.
"""
import json
from pathlib import Path

HW = Path(__file__).resolve().parents[1] / "hw/easy2350/a"

# Board minimums: JLC 4-layer 1 oz capabilities (checked 2026-10-23), with margin.
RULES = {
    "min_clearance": 0.1,             # JLC 0.09
    "min_track_width": 0.09,          # JLC 0.09: Freerouting necks 0.127 down to 0.095 entering QFN pads
    "min_via_diameter": 0.45,         # a 0.2/0.25 hole needs a >= 0.45 pad or JLC charges extra
    "min_via_annular_width": 0.15,    # JLC absolute minimum (0.5/0.2 signal via)
    "min_through_hole_diameter": 0.2,
    "min_hole_to_hole": 0.25,         # JLC 0.2
    "min_hole_clearance": 0.2,        # JLC via hole to track 0.2
    "min_copper_edge_clearance": 0.3, # JLC 0.2 to a routed edge
    "min_silk_clearance": 0.15,       # JLC pad to silk 0.15
    "min_text_height": 0.8,
    "min_text_thickness": 0.15,
    "solder_mask_to_copper_clearance": 0.0,
    "min_microvia_diameter": 0.2,
    "min_microvia_drill": 0.1,
    "min_resolved_spokes": 1,         # reflowed 0402s by the RP2350 fit one thermal spoke, not two
}

# Net classes from nets.csv. Widths are at least nets_check.py's IPC-2152 result
# (0.127 mm for every class here) and sized for the job; see stackup.md.
CLASSES = [
    # name, track, clearance, via dia, via drill, diff width, diff gap
    ("Default", 0.1, 0.1, 0.5, 0.2, 0.2, 0.15),     # 0.5/0.2: smallest via without a JLC surcharge
    ("SIGNAL", 0.1, 0.1, 0.5, 0.2, 0.2, 0.15),      # 0.1/0.1: QSPI-pitch escapes (JLC: 0.09/0.09)
    ("POWER", 0.25, 0.12, 0.6, 0.3, 0.2, 0.15),   # 0.25 = 2x the IPC-2152 width; 0.12: vendor copper
    ("SWITCH", 0.4, 0.12, 0.6, 0.3, 0.2, 0.15),
    ("DIFF_HS", 0.2, 0.15, 0.5, 0.2, 0.2, 0.15),
    ("SE_HS", 0.1, 0.12, 0.5, 0.2, 0.2, 0.15),
    ("CLOCK", 0.15, 0.15, 0.5, 0.2, 0.2, 0.15),   # vendor R2 sits 0.18 from the 1V1 spoke via
]
PATTERNS = [
    ("POWER", "VBUS"), ("POWER", "+3V3"), ("POWER", "+1V1"), ("POWER", "VLED"), ("POWER", "GND"),
    ("POWER", "/VREG_AVDD"), ("SWITCH", "/VREG_LX"),
    ("DIFF_HS", "/USB_D*"),
    ("SE_HS", "/QSPI_*"),
    ("CLOCK", "/XIN"), ("CLOCK", "/XOUT*"),
    ("SIGNAL", "/GPIO*"), ("SIGNAL", "/CC*"), ("SIGNAL", "/RUN"), ("SIGNAL", "/SW*"),
    ("SIGNAL", "/LED_DIN"), ("SIGNAL", "/PIX*"),
]


def main():
    pro = HW / "easy2350.kicad_pro"
    data = json.loads(pro.read_text()) if pro.exists() else {}
    data.setdefault("meta", {"filename": pro.name, "version": 3})
    ds = data.setdefault("board", {}).setdefault("design_settings", {})
    ds.setdefault("rules", {}).update(RULES)
    ds.setdefault("defaults", {})
    data["net_settings"] = {
        "classes": [{"name": n, "track_width": tw, "clearance": cl, "via_diameter": vd, "via_drill": vdr,
                     "diff_pair_width": dw, "diff_pair_gap": dg, "diff_pair_via_gap": 0.25,
                     "microvia_diameter": 0.3, "microvia_drill": 0.1, "line_style": 0, "wire_width": 6,
                     "bus_width": 12, "pcb_color": "rgba(0, 0, 0, 0.000)",
                     "schematic_color": "rgba(0, 0, 0, 0.000)",
                     "priority": 2147483647 if n == "Default" else i}
                    for i, (n, tw, cl, vd, vdr, dw, dg) in enumerate(CLASSES)],
        "meta": {"version": 4},
        "netclass_patterns": [{"netclass": c, "pattern": p} for c, p in PATTERNS],
    }
    pro.write_text(json.dumps(data, indent=2) + "\n")
    print(f"wrote {pro}")


if __name__ == "__main__":
    main()
