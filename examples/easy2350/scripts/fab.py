#!/usr/bin/env python3
"""fab.py -- JLCPCB fabrication and assembly files for easy2350.

    python3 scripts/fab.py          writes hw/easy2350/a/fab/

  gerbers/ + drill   kicad-cli, every copper, mask, paste, silk and edge layer
  easy2350-gerbers.zip
  bom-jlc.csv        Comment, Designator, Footprint, LCSC: one row per LCSC part,
                     only parts that are built (not DNP, not excluded from BOM)
  cpl-jlc.csv        Designator, Mid X, Mid Y, Layer, Rotation: from KiCad's
                     position file, SMD parts that are built
  easy2350.step      3D model of the assembled board

Then check them against the schematic:

    python3 .claude/skills/pcb-design/scripts/sync_check.py hw/easy2350/a \\
        --bom hw/easy2350/a/fab/bom-jlc.csv --cpl hw/easy2350/a/fab/cpl-jlc.csv

JLC's rotation convention differs from KiCad's for some packages; the
assembly preview on their site is the check (references/sync-and-release.md).
"""
import csv
import shutil
import subprocess
import sys
import zipfile
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT.parents[1] / ".claude/skills/pcb-design/scripts"))
from kicad_files import load_board, load_schematic  # noqa: E402

HW = ROOT / "hw/easy2350/a"
PCB, SCH = HW / "easy2350.kicad_pcb", HW / "easy2350.kicad_sch"
OUT = HW / "fab"
LAYERS = "F.Cu,In1.Cu,In2.Cu,B.Cu,F.Mask,B.Mask,F.Paste,B.Paste,F.Silkscreen,B.Silkscreen,Edge.Cuts"


def run(*cmd):
    r = subprocess.run(cmd, capture_output=True, text=True)
    if r.returncode:
        sys.exit(f"{' '.join(cmd)}\n{r.stdout}\n{r.stderr}")


def natural(ref):
    import re
    return [int(t) if t.isdigit() else t for t in re.split(r"(\d+)", ref)]


def main():
    if OUT.exists():
        shutil.rmtree(OUT)
    gerb = OUT / "gerbers"
    gerb.mkdir(parents=True)
    run("kicad-cli", "pcb", "export", "gerbers", "--layers", LAYERS, "--subtract-soldermask",
        "--use-drill-file-origin", "-o", str(gerb) + "/", str(PCB))
    run("kicad-cli", "pcb", "export", "drill", "--format", "excellon", "--excellon-separate-th",
        "--generate-map", "--map-format", "gerberx2", "-o", str(gerb) + "/", str(PCB))
    with zipfile.ZipFile(OUT / "easy2350-gerbers.zip", "w", zipfile.ZIP_DEFLATED) as z:
        for f in sorted(gerb.iterdir()):
            z.write(f, f.name)

    sch = load_schematic(SCH)
    built = {r: p for r, p in sch.parts.items()
             if not p.dnp and not p.exclude_from_bom and not p.exclude_from_board}
    rows = defaultdict(list)
    for r, p in built.items():
        rows[(p.value, p.footprint.split(":")[-1], p.fields.get("LCSC", ""))].append(r)
    with (OUT / "bom-jlc.csv").open("w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["Comment", "Designator", "Footprint", "LCSC"])
        for (value, fp, lcsc), refs in sorted(rows.items(), key=lambda kv: natural(min(kv[1], key=natural))):
            w.writerow([value, ",".join(sorted(refs, key=natural)), fp, lcsc])

    pos = OUT / "pos.csv"
    run("kicad-cli", "pcb", "export", "pos", "--format", "csv", "--units", "mm", "--side", "both",
        "--exclude-dnp", "--smd-only", "-o", str(pos), str(PCB))
    board = load_board(PCB)
    with pos.open() as f, (OUT / "cpl-jlc.csv").open("w", newline="") as g:
        w = csv.writer(g)
        w.writerow(["Designator", "Mid X", "Mid Y", "Layer", "Rotation"])
        for row in csv.DictReader(f):
            ref = row["Ref"]
            fp = board.footprints.get(ref)
            if ref not in built or fp is None or "exclude_from_pos_files" in fp.attrs:
                continue
            rot = float(row["Rot"]) % 360          # JLC's preview wants 0-360; KiCad writes -90
            w.writerow([ref, row["PosX"], row["PosY"], "Top" if row["Side"] == "top" else "Bottom",
                        f"{rot:.6f}"])
    pos.unlink()
    # 3D model for the enclosure / mechanical check; parts with no model in
    # KiCad's library (J1, L1, the pixels) are missing from it
    run("kicad-cli", "pcb", "export", "step", "--subst-models", "--force", "-o", str(OUT / "easy2350.step"), str(PCB))
    print(f"fab: {OUT}")
    for f in sorted(OUT.iterdir()):
        print(f"  {f.name}")


if __name__ == "__main__":
    main()
