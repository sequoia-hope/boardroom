#!/usr/bin/env python3
"""The "like the designers" scoreboard: the designers' own routing beside a run's on the same
boards, one row per measure.

    scoreboard.py OURS_RUN [--out DIR] [--jobs N]

OURS_RUN is a pcbench run of the designers' boards (`pcbench.py run --source truth`); the other
column is each board as its designer routed it (LAB/pcbench/truth/ID/designer.kicad_pcb, judged
by `pcbench.py judge-truth`). A run of PCBench's `processed` boards is refused: what stood beside
it as the designers' routing was FreeRouting's on every poured net (`bench/README.md`).

The measures of direction, planes, bundles and repeats are `examine::designer::like` (built with
`cargo build --release -p examine --example like`): angles are measured in each neighbourhood's
frame, not the board's x and y. The rest are `tidy.py`'s and the judge's, read from the saved
results. Sums run over the boards both sides wrote, clean or not, so the two columns describe the
same boards. Writes index.html and scoreboard.json to DIR (default LAB/planned/scoreboard/RUN).
Routes nothing and asks KiCad nothing.

The second scoreboard, "sound" (coupling between roles, reference planes, loop areas, width
against current), needs each net's role and is not built.
"""
import argparse
import html
import json
import re
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import paths
from tidy import ratios

LAB = paths.lab()
RUNS = LAB / "pcbench" / "runs"
TRUTH = LAB / "pcbench" / "truth"
TOOL = paths.REPO / "target" / "release" / "examples" / "like"

REPAIR = re.compile(r"^(mended|bridged|split) (\d+) ", re.M)
SUMS = ("track_mm", "vias", "on_planes_mm", "off_own_frame_mm", "off_designer_frame_mm", "off_board_axes_mm", "bundled_mm", "along_axis_mm", "on_axes_mm", "parts", "repeated")


def like_of(designer, ours):
    r = subprocess.run([str(TOOL), str(designer), str(ours)], stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=900)
    return json.loads(r.stdout) if r.returncode == 0 else None


def board_of(run, bid):
    """One board: the run's result, the designer's, and the two beside each other."""
    d = RUNS / run / bid
    if not (d / "result.json").exists() or not (TRUTH / bid / "designer.json").exists():
        return None
    ours, theirs = json.loads((d / "result.json").read_text()), json.loads((TRUTH / bid / "designer.json").read_text())
    out = d / "out.kicad_pcb"
    if ours.get("code") != 0 or not out.exists() or "referee_error" in theirs:
        return None
    like = like_of(TRUTH / bid / "designer.kicad_pcb", out)
    if like is None:
        return None
    log = d / "log.txt"
    ours["repairs"] = sum(int(n) for _, n in REPAIR.findall(log.read_text(errors="replace"))) if log.exists() else None
    # The designer's board against the bar a route is held to: nothing more unconnected than it
    # has (true of itself) and no error its copper adds.
    theirs["success"] = not theirs.get("new_errors")
    return {"ours": ours, "designers": theirs, "like": like}


def column(rows, side):
    """One side's figures over the boards: `side` is "designers" or "ours"."""
    like = [r["like"]["designer" if side == "designers" else "ours"] for r in rows]
    res = [r[side] for r in rows]
    s = {k: sum(x[k] for x in like) for k in SUMS}
    deep = [x for x, r in zip(like, rows) if sum(1 for _, mm in r["like"]["designer"]["by_layer"] if mm > 0) >= 3]
    planed = [x for x, r in zip(like, rows) if r["like"]["plane_layers"]]
    held = [r["tidy"] for r in res if r.get("tidy")]
    keys = {k for t in held for k, v in t.items() if isinstance(v, (int, float))}
    t = ratios({k: sum(x.get(k) or 0 for x in held) for k in keys}) if held else {}
    per = lambda x, y: x / y if y else None
    add = lambda k: sum(r.get(k) or 0 for r in res)
    return {
        "boards": len(rows),
        "clean": sum(bool(r.get("success")) for r in res),
        "on_planes_mm": round(sum(x["on_planes_mm"] for x in planed)),
        "boards_with_planes": len(planed),
        "off_own_frame": per(s["off_own_frame_mm"], s["track_mm"]),
        "off_designer_frame": per(s["off_designer_frame_mm"], s["track_mm"]),
        "off_board_axes": per(s["off_board_axes_mm"], s["track_mm"]),
        "vias": s["vias"],
        "bundled": per(s["bundled_mm"], s["track_mm"]),
        "bus_bundled": t.get("bus_bundled"),
        "along_axis_3plus": per(sum(x["along_axis_mm"] for x in deep), sum(x["on_axes_mm"] for x in deep)),
        "boards_3plus": len(deep),
        "repeated": per(s["repeated"], s["parts"]),
        "crossings_per_100mm": t.get("crossings_per_100mm"),
        "turns": t.get("turns"),
        "doglegs_per_m": t.get("doglegs_per_m"),
        "detour": t.get("detour"),
        "track_mm": round(s["track_mm"]),
        "repairs_per_board": round(add("repairs") / len(rows), 1) if rows and side == "ours" else None,
        "necks": add("necks"),
        "acute": add("acute"),
        "slots": add("slots"),
    }


ROWS = [  # key, label, how it is measured, how to print
    ("clean", "Boards clean: nothing unconnected beyond the designer's own, no error added by the routed copper", "KiCad DRC after a refill", "n"),
    ("on_planes_mm", "Track on the layers the designer kept as planes, mm", "examine: a layer half poured with little track", "n"),
    ("off_own_frame", "Off the 45-degree grid of its own neighbourhood", "examine, in the neighbourhood's frame", "%"),
    ("off_designer_frame", "Off the grid the designer used there", "examine, in the designer's frame", "%"),
    ("off_board_axes", "Off the grid of the board's x and y (the old score, blind to turned frames)", "examine", "%"),
    ("vias", "Vias", "", "n"),
    ("bundled", "Track in a bundle, beside another net's", "examine", "%"),
    ("bus_bundled", "Bus members beside a sibling", "tidy.py", "%"),
    ("along_axis_3plus", "Along its layer's axis, boards with track on 3+ layers", "examine, in the neighbourhood's frame", "%"),
    ("repeated", "Parts of 3+ pads routed like another of their footprint", "examine", "%"),
    ("crossings_per_100mm", "Crossings of another layer's track, per 100 mm", "tidy.py", "f"),
    ("turns", "Turns per 10 mm", "tidy.py", "f"),
    ("doglegs_per_m", "Doglegs per metre", "tidy.py", "f"),
    ("detour", "Detour over the shortest tree", "tidy.py", "f"),
    ("track_mm", "Track length, mm", "", "n"),
    ("repairs_per_board", "Repairs at write (mends, bridges, splits), per board", "the router's log", "f"),
    ("necks", "Necks", "KiCad, connection width", "n"),
    ("acute", "Acute angles", "dfm.py", "n"),
    ("slots", "Same-net slots", "dfm.py", "n"),
]


def show(v, how):
    if v is None:
        return "-"
    return f"{100 * v:.1f}%" if how == "%" else f"{v:,}" if how == "n" else f"{v:.2f}"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("ours")
    ap.add_argument("--out")
    ap.add_argument("--jobs", type=int, default=4)
    a = ap.parse_args()
    if not TOOL.exists():
        sys.exit(f"scoreboard: build {TOOL.relative_to(paths.REPO)} first: cargo build --release -p examine --example like")
    info = json.loads((RUNS / a.ours / "run.json").read_text())
    if info.get("source") != "truth":
        sys.exit(f"scoreboard: {a.ours} routed PCBench's processed boards, which are not the designers' (bench/README.md); route with --source truth")
    ids = sorted(p.name for p in (RUNS / a.ours).iterdir() if (p / "result.json").exists())
    with ThreadPoolExecutor(a.jobs) as pool:
        boards = dict(zip(ids, pool.map(lambda i: board_of(a.ours, i), ids)))
    both = [i for i in ids if boards[i]]
    rows = [boards[i] for i in both]
    cols = {side: column(rows, side) for side in ("designers", "ours")}
    results = [json.loads((RUNS / a.ours / i / "result.json").read_text()) for i in ids]
    data = {
        "ours_run": a.ours, "commit": info.get("commit"), "binary": info.get("binary"), "recipe": info.get("recipe"), "tier": info.get("tier"),
        "boards_in_run": len(ids), "clean_in_run": sum(bool(r.get("success")) for r in results),
        "boards_compared": len(both), "columns": cols,
        "boards": {i: {"ours": {k: boards[i]["ours"].get(k) for k in ("success", "unconnected", "floor", "new_errors", "necks", "repairs")},
                       "designers": {k: boards[i]["designers"].get(k) for k in ("unconnected", "new_errors", "necks")},
                       "like": boards[i]["like"]} for i in both},
    }
    out = Path(a.out) if a.out else LAB / "planned" / "scoreboard" / a.ours
    out.mkdir(parents=True, exist_ok=True)
    (out / "scoreboard.json").write_text(json.dumps(data, indent=1))
    e = html.escape
    table = "".join(
        f"<tr><td>{e(label)}</td><td>{show(cols['designers'][k], how)}</td><td>{show(cols['ours'][k], how)}</td><td class=how>{e(by)}</td></tr>"
        for k, label, by, how in ROWS)
    # The boards where ours strays furthest on the measures the old benchmark could not see.
    planes = sorted((i for i in both if boards[i]["like"]["plane_layers"]), key=lambda i: -boards[i]["like"]["ours"]["on_planes_mm"])[:25]
    plane_rows = "".join(
        f"<tr><td>{e(i)}</td><td>{e(', '.join(boards[i]['like']['plane_layers']))}</td><td>{boards[i]['like']['designer']['on_planes_mm']:.0f}</td><td>{boards[i]['like']['ours']['on_planes_mm']:.0f}</td>"
        f"<td>{boards[i]['like']['designer']['vias']}</td><td>{boards[i]['like']['ours']['vias']}</td><td>{'yes' if boards[i]['ours'].get('success') else 'no'}</td></tr>" for i in planes)
    (out / "index.html").write_text(f"""<!doctype html><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>Like the designers: {e(a.ours)}</title>
<style>body{{font:16px/1.5 system-ui,sans-serif;margin:1rem auto;max-width:70rem;padding:0 1rem;color:#111;background:#fff}}
table{{border-collapse:collapse;width:100%;margin:.5rem 0 1.5rem}}td,th{{padding:.3rem .6rem;border-bottom:1px solid #ddd;text-align:left;overflow-wrap:anywhere}}
td:nth-child(2),td:nth-child(3),th:nth-child(2),th:nth-child(3){{text-align:right;font-variant-numeric:tabular-nums;white-space:nowrap}}.how{{color:#666;font-size:.85rem}}
.wide{{overflow-x:auto}}</style>
<p><a href="../../">← Review pages</a></p>
<h1>Like the designers: <code>{e(a.ours)}</code></h1>
<p>The designers' own boards (from the corpus's raw files, pours and all) beside this run's routing of the same boards with the
tracks and vias taken out and the pours kept. Router <code>{e(Path(str(info.get('binary'))).parent.name)}</code>, recipe <code>{e(str(info.get('recipe')))}</code>,
tier {e(str(info.get('tier')))}: {data['clean_in_run']} of {len(ids)} boards clean. The table adds up the {len(both)} boards both sides wrote, clean or not.
{cols['designers']['boards_with_planes']} of them have a layer the designer kept as a plane; {cols['designers']['boards_3plus']} have track on three or more layers.</p>
<div class=wide><table><tr><th>Measure</th><th>Designers</th><th>Ours</th><th>Measured by</th></tr>{table}</table></div>
<p>Look at the boards before quoting these: <a href="../../lookset/">the look sets</a>. Per-board figures: <a href="scoreboard.json">scoreboard.json</a>.
The second scoreboard, "sound" (coupling between roles, reference planes, loop areas, width against current), needs each net's role and is not built.</p>
<h2>Track on the designers' planes</h2>
<p>The {len(planes)} boards where this run laid most track on a layer the designer kept as a plane.</p>
<div class=wide><table><tr><th>Board</th><th>Designer's plane layers</th><th>Designer's track there, mm</th><th>Ours, mm</th><th>Designer's vias</th><th>Ours</th><th>Ours clean</th></tr>{plane_rows}</table></div>
""")
    print(f"{len(both)} boards compared; wrote {out}/index.html")
    for k, label, _, how in ROWS:
        print(f"  {label}: designers {show(cols['designers'][k], how)}, ours {show(cols['ours'][k], how)}")


if __name__ == "__main__":
    main()
