#!/usr/bin/env python3
"""pad_joints.py -- every SMD pad joined to its copper by at least a track's width.

    pad_joints.py hw/main/a            check the board, exit 1 on a thin joint
    pad_joints.py hw/main/a --json f   also write the findings as JSON

A track that ends just outside a pad touches it only where its round end-cap
crosses the pad edge: a lens whose width (the chord) can be far under the
fab's minimum track, and KiCad's DRC can't see it, since connectivity is
yes or no (review-layout 2026-11-01 A2: J1.B7, U1.4, U1.8 and U5.6 at
0.058-0.087 mm). A pad passes when any one of its joints is sound:

  * a track whose end lies inside the pad, or reaches at least half its width
    in (the joint is then the track's full width);
  * a zone fill of its net covering the pad's centre;
  * a via of its net whose ring overlaps the pad by at least the minimum.

Otherwise, if its only joints are lenses, it fails when the widest lens is
narrower than the fab's minimum track (checks.toml [fab] min_track_mm, 0.09 by
default). Run it at gate 7 after routing, beside DRC.
"""
import argparse
import json
import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import pcbnew  # noqa: E402


def board_path(arg: str) -> Path:
    p = Path(arg)
    if p.is_dir():
        pcbs = sorted(p.glob("*.kicad_pcb"))
        if len(pcbs) != 1:
            sys.exit(f"pad_joints: expected one .kicad_pcb in {p}, found {len(pcbs)}")
        return pcbs[0]
    return p.with_suffix(".kicad_pcb")


def min_track(pcb: Path) -> float:
    try:
        from kicad_files import load_checks, repo_root
        return float(load_checks(repo_root(pcb.parent))["fab"].get("min_track_mm", 0.09))
    except Exception:
        return 0.09


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("project")
    ap.add_argument("--json")
    args = ap.parse_args()
    pcb = board_path(args.project)
    floor = min_track(pcb)
    b = pcbnew.LoadBoard(str(pcb))
    mm = pcbnew.ToMM
    tracks = [t for t in b.GetTracks() if t.Type() != pcbnew.PCB_VIA_T]
    vias = [t for t in b.GetTracks() if t.Type() == pcbnew.PCB_VIA_T]
    zones = [z for z in b.Zones() if not z.GetIsRuleArea()]
    bad, checked = [], 0
    for fp in b.GetFootprints():
        for pad in fp.Pads():
            net = pad.GetNetname()
            if pad.GetAttribute() != pcbnew.PAD_ATTRIB_SMD or not net or net.startswith("unconnected"):
                continue
            layer = pad.GetLayer()
            poly = pad.GetEffectivePolygon(layer)
            c = pad.GetPosition()
            reach = max(pad.GetSizeX(), pad.GetSizeY()) / 2 + pcbnew.FromMM(1.0)
            sound, lenses = False, []
            for t in tracks:
                if t.GetNetname() != net or t.GetLayer() != layer:
                    continue
                ends = [e for e in (t.GetStart(), t.GetEnd()) if math.dist((e.x, e.y), (c.x, c.y)) < reach]
                if not ends:
                    continue
                w = t.GetWidth()
                d = min(math.sqrt(poly.SquaredDistance(e)) for e in ends)   # 0 when the end is inside
                if d <= w / 4:                   # inside, or the cap reaches half its width in
                    sound = True
                    break
                if d < w / 2:
                    lenses.append(2 * math.sqrt((w / 2) ** 2 - d ** 2))
            if not sound:
                sound = any(z.GetNetname() == net and z.IsOnLayer(layer)
                            and z.GetFilledPolysList(layer).Contains(c) for z in zones)
            if not sound:
                for v in vias:
                    if v.GetNetname() == net and v.IsOnLayer(layer):
                        d = math.sqrt(poly.SquaredDistance(v.GetPosition()))
                        if v.GetWidth(layer) / 2 - d >= pcbnew.FromMM(floor):
                            sound = True
                            break
            checked += 1
            if not sound and lenses and mm(max(lenses)) < floor:
                bad.append({"pad": f"{fp.GetReference()}.{pad.GetNumber()}", "net": net,
                            "neck_mm": round(mm(max(lenses)), 3),
                            "at": [round(mm(c.x), 3), round(mm(c.y), 3)]})
    print(f"pad_joints: {pcb.name}: {checked} SMD pads, joints under {floor} mm:")
    for f in bad:
        print(f"  {f['pad']:8s} {f['net']:16s} neck {f['neck_mm']:.3f} mm at ({f['at'][0]}, {f['at'][1]})")
    print(f"\n  {'FAIL' if bad else 'PASS'}: {len(bad)} thin pad joints")
    if args.json:
        Path(args.json).write_text(json.dumps(bad, indent=1))
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
