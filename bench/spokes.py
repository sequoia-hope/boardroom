#!/usr/bin/env python3
"""Which thermal spokes a bare board has: our model beside KiCad's own fill.

    spokes.py SET [--only id,...|@FILE] [--bin PATH] [--list KIND] [--json OUT]

SET is a tier (smoke, focus, guard, gate): the designers' boards stripped, with the fills KiCad
wrote on the empty board (LAB/pcbench/truth/ID/bare.kicad_pcb). For every spoke a pad joined by
thermal reliefs could have (four to a pad and layer), the board model says whether it keeps
room for it (`crates/board/src/spokes.rs`; `cargo build --release -p board --example
spoke_trials`), and KiCad's fill says whether it is there: whether the spoke's axis is copper
both at the middle of the pad's gap and just off the pad. The middle is where KiCad's own
check looks (it counts where the fill's outline crosses the pad's outline grown by half the
gap), so it also counts a spoke cut off short of its pad, which joins nothing: those are
counted apart here ("stubs"), as spokes neither side has.

Prints how many spokes both have, neither has, and only one has, with what our model made of
those (PadTooNarrow, NoPour, Cut, or Kept where KiCad has none), and the boards with most of
each. --list KIND ("ours only" or "KiCad only") prints each such spoke, for looking at.

A pad that KiCad gives more spokes than we keep room for is taken for starved before routing
and left unguarded; one we keep room for that KiCad has not is guarded for nothing, and the
room is lost to the router. Run under `reserve` on a tier: a board takes about a second.
"""
import argparse
import collections
import concurrent.futures
import json
import os
import re
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import paths
import pcbench

LAB = paths.lab()


def fills(text):
    """KiCad's filled outlines by layer: (box, points) each. A fill with holes is one outline
    that runs in and out of them, so even-odd crossing counts hold."""
    out = collections.defaultdict(list)
    for m in re.finditer(r'\(filled_polygon\s*\(layer "([^"]+)"\)', text):
        start = text.index("(pts", m.end())
        depth, i = 0, start
        while True:
            c = text[i]
            depth += c == "("
            depth -= c == ")"
            if depth == 0:
                break
            i += 1
        pts = [(float(a), float(b)) for a, b in re.findall(r"\(xy ([\d.-]+) ([\d.-]+)\)", text[start:i])]
        if len(pts) >= 3:
            xs, ys = [p[0] for p in pts], [p[1] for p in pts]
            out[m[1]].append(((min(xs), min(ys), max(xs), max(ys)), pts))
    return out


def inside(x, y, pts):
    c = False
    x1, y1 = pts[-1]
    for x2, y2 in pts:
        if (y1 > y) != (y2 > y) and x < x1 + (y - y1) * (x2 - x1) / (y2 - y1):
            c = not c
        x1, y1 = x2, y2
    return c


def one(job):
    """A board's spokes: (pad, layer, angle, x, y, what we made of it, whether KiCad has it
    joining its pad, whether KiCad has copper at the middle of the gap)."""
    binary, bid, board = job
    try:
        ours = subprocess.run([binary, str(board)], capture_output=True, text=True, timeout=600)
        if ours.returncode != 0:
            return bid, None
        copper = fills(board.read_text())
    except (OSError, subprocess.TimeoutExpired):
        return bid, None
    rows = []
    for line in ours.stdout.splitlines():
        pad, layer, angle, x, y, px, py, made = line.split("\t")
        x, y, px, py = float(x), float(y), float(px), float(py)
        at = lambda x, y: any(b[0] <= x <= b[2] and b[1] <= y <= b[3] and inside(x, y, pts) for b, pts in copper.get(layer, []))
        mid = at(x, y)
        rows.append((pad, layer, float(angle), x, y, made, mid and at(px, py), mid))
    return bid, rows


def kind(row):
    ours, kicad = row[5] == "Kept", row[6]
    return "both" if ours and kicad else "ours only" if ours else "KiCad only" if kicad else "neither"


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("set", help="a tier (smoke, focus, guard, gate)")
    ap.add_argument("--only", help="these boards of the tier only: ids with commas between, or @FILE with one id a line")
    ap.add_argument("--bin", default=str(paths.REPO / "target" / "release" / "examples" / "spoke_trials"), help="the built spoke_trials example")
    ap.add_argument("--list", choices=["ours only", "KiCad only"], help="print each spoke of this kind")
    ap.add_argument("--json", help="write every spoke of every board here")
    a = ap.parse_args()
    if not Path(a.bin).is_file():
        sys.exit(f"spokes: build it first: cargo build --release -p board --example spoke_trials ({a.bin})")
    ids = pcbench.tier_ids(a.set)
    if a.only:
        only = {l.strip() for l in Path(a.only[1:]).read_text().splitlines() if l.strip()} if a.only.startswith("@") else set(a.only.split(","))
        ids = [b for b in ids if b in only]
    jobs = [(a.bin, bid, LAB / "pcbench" / "truth" / bid / "bare.kicad_pcb") for bid in ids]
    jobs = [j for j in jobs if j[2].exists()]
    with concurrent.futures.ProcessPoolExecutor(int(os.environ.get("RESERVED_CORES", "4"))) as pool:
        res = dict(pool.map(one, jobs))
    if a.json:
        Path(a.json).write_text(json.dumps(res))
    unread = sorted(b for b, rows in res.items() if rows is None)
    total, why, per = collections.Counter(), collections.Counter(), collections.defaultdict(collections.Counter)
    stubs = 0
    for bid, rows in res.items():
        for row in rows or []:
            k = kind(row)
            total[k] += 1
            stubs += row[7] and not row[6]
            per[k][bid] += 1
            if k in ("ours only", "KiCad only"):
                why[(k, row[5])] += 1
    n = sum(total.values())
    print(f"{a.set}: {len(res) - len(unread)} boards, {n} spokes a pad could have: both have {total['both']}, neither {total['neither']}, "
          f"only KiCad {total['KiCad only']}, only ours {total['ours only']}; {stubs} more are stubs in KiCad's fill" + (f"; agree on {100 * (total['both'] + total['neither']) / n:.1f}%" if n else "")
          + (f"; {len(unread)} boards not read: {', '.join(unread[:5])}" if unread else ""))
    for (k, made), count in why.most_common():
        print(f"  {k:10} {count:6}  ours: {made}")
    for k in ("KiCad only", "ours only"):
        print(f"  most {k}: " + ", ".join(f"{b} {c}" for b, c in per[k].most_common(6)) + f" ({len(per[k])} boards)")
    if a.list:
        for bid, rows in sorted(res.items()):
            for row in rows or []:
                if kind(row) == a.list:
                    print(f"  {bid:44} {row[0]:10} {row[1]:7} {row[2]:5.0f}  ({row[3]:.3f}, {row[4]:.3f})  {row[5]}")


if __name__ == "__main__":
    main()
