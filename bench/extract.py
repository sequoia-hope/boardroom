#!/usr/bin/env python3
"""What the designers did, read from their boards and drawn to be looked at.

    extract.py look  [--set bench/lookset.json] [--only id,...] [--also ID,...]
    extract.py count [--tier T | --only ID,...]

Both run `examine::designer` (built with `cargo build --release -p examine --example designer`)
on the designer's board: which layers are planes, the frame of the routing in each
neighbourhood, the bundles, the repeats.

`look` does the look set (and with --also, more corpus boards by id) and writes LAB/planned/extraction/: every reading drawn over the
board's copper, with its numbers. These readings are guesses about intent; the pictures are how
a wrong one is found, so look before quoting `count`.

`count` does a tier of the corpus (the boards of `truth.py prepare`), keeps only the numbers in
LAB/pcbench/extraction.json, and `look` then shows their totals above the pictures.

Nothing here enters the repository: the boards are their authors' (`bench/README.md`).
"""

import argparse
import concurrent.futures
import html
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import paths
import truth

TOOL = paths.REPO / "target" / "release" / "examples" / "designer"
WIDTH = 1500


def designer_board(board):
    """The designer's board of a look-set entry."""
    if "pcbench" in board:
        return paths.lab() / "pcbench" / "truth" / board["pcbench"] / "designer.kicad_pcb"
    p = Path(board["designer"]).expanduser()
    return p if p.is_absolute() else paths.lab() / p


def run(pcb, out):
    r = subprocess.run([str(TOOL), str(pcb), str(out)], stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=600)
    if r.returncode != 0:
        return {"error": (r.stderr.strip().splitlines() or ["failed"])[-1][:300]}
    return json.loads((out / "designer.json").read_text())


def look_one(job):
    import cairosvg

    board, out = job
    pcb = designer_board(board)
    if not pcb.exists():
        return board["id"], {"error": f"no designer board at {pcb}"}
    out.mkdir(parents=True, exist_ok=True)
    x = run(pcb, out)
    if "error" not in x:
        for svg in out.glob("*.svg"):
            cairosvg.svg2png(url=str(svg), write_to=str(svg.with_suffix(".png")), output_width=WIDTH)
    return board["id"], x


def count_one(b):
    pcb = paths.lab() / "pcbench" / "truth" / b / "designer.kicad_pcb"
    if not pcb.exists():
        return {"id": b, "error": "no designer board (truth.py prepare)"}
    tmp = Path(tempfile.mkdtemp(prefix="extract-"))
    try:
        x = run(pcb, tmp)
    except subprocess.TimeoutExpired:
        x = {"error": "timed out"}
    finally:
        shutil.rmtree(tmp, ignore_errors=True)
    if "error" in x:
        return {"id": b, "error": x["error"]}
    f, bu, r = x["frames"], x["bundles"], x["repeats"]
    return {"id": b, "track_mm": round(x["track_mm"], 1), "roles": [l["role"] for l in x["layers"]],
            "off_board_axes": round(f["off_board_axes"], 4), "off_local_frame": round(f["off_local_frame"], 4), "turned": round(f["turned"], 4),
            "bundled": round(bu["bundled"], 4), "widest_bundle": max((len(q["nets"]) for q in bu["bundles"]), default=0),
            "parts": r["parts"], "repeated": r["repeated"], "largest_repeat": max((len(q["members"]) for q in r["repeats"]), default=0)}


def count(a):
    ids = truth.chosen(a)
    with concurrent.futures.ThreadPoolExecutor(int(os.environ.get("RESERVED_CORES", "4"))) as pool:
        rows = list(pool.map(count_one, ids))
    out = paths.lab() / "pcbench" / "extraction.json"
    out.write_text(json.dumps({"tier": a.tier, "boards": rows}, indent=1))
    print(f"wrote {out}: {len(rows)} boards, {sum('error' in r for r in rows)} failed")
    print(json.dumps(totals(rows), indent=1))


def totals(rows):
    ok = [r for r in rows if "error" not in r and r["track_mm"] > 0]
    multi = [r for r in ok if len(r["roles"]) > 2]

    def n(test, of=ok):
        return sum(1 for r in of if test(r))

    return {
        "boards": len(ok), "failed": len(rows) - len(ok), "multi_layer": len(multi),
        "multi_layer_with_a_plane": n(lambda r: "plane" in r["roles"], multi),
        "multi_layer_with_a_poured_signal_layer": n(lambda r: "poured_signal" in r["roles"], multi),
        "two_layer_with_a_poured_side": n(lambda r: len(r["roles"]) <= 2 and ("poured_signal" in r["roles"] or "plane" in r["roles"])),
        "off_board_axes_over_10pct": n(lambda r: r["off_board_axes"] > 0.10),
        "off_local_frame_over_10pct": n(lambda r: r["off_local_frame"] > 0.10),
        "turned_over_10pct": n(lambda r: r["turned"] > 0.10),
        "bundled_at_least_a_quarter": n(lambda r: r["bundled"] >= 0.25),
        "bundle_of_8_or_more_nets": n(lambda r: r["widest_bundle"] >= 8),
        "with_a_repeat": n(lambda r: r["repeated"] > 0),
        "repeat_of_4_or_more": n(lambda r: r["largest_repeat"] >= 4),
    }


ROLE = {"plane": "plane", "poured_signal": "poured and routed", "signal": "signal", "empty": "empty"}


def section(board, x):
    e = html.escape
    bid = board["id"]
    s = [f"<h2 id='{e(bid)}'>{e(board['title'])}</h2><p class='m'>{e(board['why'])}</p>"]
    if "error" in x:
        return "".join(s) + f"<p class='bad'>Not read: {e(x['error'])}</p>"

    def fig(name, caption):
        return f"<figure><a href='{e(bid)}/{name}.svg'><img loading='lazy' src='{e(bid)}/{name}.png' alt='{e(caption)}'></a><figcaption>{caption}</figcaption></figure>"

    s.append("<h3>Layers</h3><table><tr><th>Layer</th><th>Read as</th><th>Share of track</th><th>Poured</th><th>Pour net</th></tr>")
    for l in x["layers"]:
        s.append(f"<tr><td>{e(l['layer'])}</td><td>{ROLE[l['role']]}</td><td>{100 * l['track_share']:.0f}% ({l['track_mm']:.0f} mm)</td>"
                 f"<td>{100 * l['fill_share']:.0f}%</td><td>{e(l['fill_net'] or '')}</td></tr>")
    s.append("</table><div class='grid'>")
    for l in x["layers"]:
        if l["role"] != "empty":
            s.append(fig("layer." + l["layer"].replace(".", "_"), f"{e(l['layer'])}: {ROLE[l['role']]}. Pours orange, tracks blue."))
    s.append("</div>")

    f = x["frames"]
    s.append(f"<h3>Frames</h3><p>Track off the 45-degree grid of the board's own axes: <b>{100 * f['off_board_axes']:.0f}%</b>. "
             f"Off the grid of its own neighbourhood: <b>{100 * f['off_local_frame']:.0f}%</b>. "
             f"In neighbourhoods routed on a turned grid: <b>{100 * f['turned']:.0f}%</b>. Neighbourhoods are {f['cell_mm']:.1f} mm squares.</p>")
    s.append("<div class='one'>" + fig("frames", "Each neighbourhood's grid as a cross (thin arms: its diagonals). Grey: the board's axes. "
                                       "Red with degrees: turned. Dashed ring: the tracks there agree on no grid.") + "</div>")

    bu = x["bundles"]
    by = ", ".join(f"{k} nets: {v:.0f} mm" for k, v in sorted(bu["by_nets"].items(), key=lambda kv: int(kv[0])))
    s.append(f"<h3>Bundles</h3><p><b>{100 * bu['bundled']:.0f}%</b> of the track runs beside another net's, in {len(bu['bundles'])} bundles. {e(by)}</p><div class='grid'>")
    for l in x["layers"]:
        if l["tracks"]:
            s.append(fig("bundles." + l["layer"].replace(".", "_"), f"{e(l['layer'])}: each bundle one colour, labelled with its number of nets; grey is unbundled."))
    s.append("</div>")

    r = x["repeats"]
    s.append(f"<h3>Repeats</h3><p><b>{r['repeated']}</b> of the {r['parts']} parts with three or more pads are routed like another part of the same footprint.</p>")
    if r["repeats"]:
        s.append("<table><tr><th>Footprint</th><th>Alike</th><th>Of</th><th>Likeness</th><th>Parts</th></tr>")
        for q in r["repeats"][:12]:
            parts = ", ".join(q["members"][:8]) + (" ..." if len(q["members"]) > 8 else "")
            s.append(f"<tr><td>{e(q['footprint'])}</td><td>{len(q['members'])}</td><td>{q['of']}</td><td>{q['likeness']:.2f}</td><td>{e(parts)}</td></tr>")
        s.append("</table>")
    s.append("<div class='one'>" + fig("repeats", "Parts routed alike share a colour.") + "</div>")
    return "".join(s)


def look(a):
    boards = json.loads(Path(a.set).read_text())["boards"]
    if a.only:
        boards = [b for b in boards if b["id"] in a.only.split(",")]
    for b in a.also.split(",") if a.also else []:
        boards.append({"id": b, "title": b, "why": "from the corpus, for what was read on it", "pcbench": b})
    out = paths.lab() / "planned" / "extraction"
    out.mkdir(parents=True, exist_ok=True)
    with concurrent.futures.ProcessPoolExecutor(int(os.environ.get("RESERVED_CORES", "4"))) as pool:
        read = dict(pool.map(look_one, [(b, out / b["id"]) for b in boards]))

    e = html.escape
    counted = ""
    cj = paths.lab() / "pcbench" / "extraction.json"
    if cj.exists():
        c = json.loads(cj.read_text())
        t = totals(c["boards"])
        two = t["boards"] - t["multi_layer"]
        counted = f"""<h2>Counted on the {e(str(c['tier']))} tier</h2>
<p>The same readings on {t['boards']} designers' boards ({t['failed']} not read). Counts of a reading are only as good as the reading: see the pictures.</p>
<table>
<tr><th>Multi-layer boards with a plane layer</th><td>{t['multi_layer_with_a_plane']} of {t['multi_layer']}</td></tr>
<tr><th>Multi-layer boards with a layer both poured and routed</th><td>{t['multi_layer_with_a_poured_signal_layer']} of {t['multi_layer']}</td></tr>
<tr><th>Two-layer boards with a side at least half poured</th><td>{t['two_layer_with_a_poured_side']} of {two}</td></tr>
<tr><th>More than 10% of track off the board's 45-degree grid</th><td>{t['off_board_axes_over_10pct']}</td></tr>
<tr><th>More than 10% off its neighbourhood's grid</th><td>{t['off_local_frame_over_10pct']}</td></tr>
<tr><th>More than 10% of track in turned neighbourhoods</th><td>{t['turned_over_10pct']}</td></tr>
<tr><th>At least a quarter of the track in bundles</th><td>{t['bundled_at_least_a_quarter']}</td></tr>
<tr><th>A bundle of 8 or more nets</th><td>{t['bundle_of_8_or_more_nets']}</td></tr>
<tr><th>Any repeat</th><td>{t['with_a_repeat']}</td></tr>
<tr><th>A repeat of 4 or more parts</th><td>{t['repeat_of_4_or_more']}</td></tr>
</table>"""
    index = "".join(f"<li><a href='#{e(b['id'])}'>{e(b['title'])}</a></li>" for b in boards)
    (out / "index.html").write_text(f"""<!doctype html><meta charset=utf-8><meta name=viewport content="width=device-width,initial-scale=1">
<title>What the designers did</title>
<style>
body{{font:16px/1.5 system-ui,sans-serif;margin:1rem auto;max-width:1900px;padding:0 1rem;color:#111;background:#fff}}
h2{{margin-top:2.5rem;border-top:1px solid #ccc;padding-top:1rem;overflow-wrap:anywhere}} .m{{color:#555}} .bad{{color:#a00}}
.grid{{display:grid;grid-template-columns:1fr 1fr;gap:1rem}} .one{{max-width:1100px}} @media (max-width:800px){{.grid{{grid-template-columns:1fr}}}}
figure{{margin:0 0 1rem}} img{{width:100%;height:auto;border:1px solid #ddd}} figcaption{{color:#555;font-size:.9rem}}
table{{border-collapse:collapse;margin:.5rem 0 1rem}} td,th{{padding:.2rem .8rem;text-align:left;border-bottom:1px solid #eee;overflow-wrap:anywhere}}
</style>
<h1>What the designers did</h1>
<p>Four things read from each designer's board, with no schematic and no hints: which layers are planes, which way the routing
runs in each neighbourhood, which nets run side by side, and which parts are routed alike. These are what "like the designers"
will be scored on, and what a planner has to produce, so each is drawn over the copper here to be checked by eye
(<code>bench/extract.py</code>, <code>examine::designer</code>). The boards are the designers' own, from the corpus's raw files
(<a href="../truth/">ground truth</a>). Click a picture for its SVG.</p>
<p><b>Seen wrong so far</b> (2026-11-12, first reading):</p>
<ul>
<li>Bundles: two buses that run beside each other for a stretch are read as one (1Bitsy's 18 is a 12 and a 6). A wide supply track beside
a thin one counts as a bundle of two (the matrix backpack, pierre). Tracks leaving a row of connector pins at the pins' pitch join a bundle
they only run near (Shift-in's 7, the matrix backpack's 6). A bundle is not followed through vias to another layer.</li>
<li>Repeats: only parts with three or more pads are compared, so a repeated cell of two-pad parts is missed; a mirrored cell does not match
its original; the parts of one cell (a chip and its capacitor) are not yet grouped into the cell.</li>
<li>Frames: a neighbourhood on the edge between two frames is read as whichever has more track. A board with no outline is measured
against the box of its copper.</li>
<li>Layers: "plane" needs half the board poured. A layer poured in two or three islands for different supplies counts the same as one net's plane.</li>
</ul>
<ul>{index}</ul>
{counted}
{''.join(section(b, read[b['id']]) for b in boards)}
""")
    print(f"wrote {out / 'index.html'}: {len(boards)} boards, {sum('error' in v for v in read.values())} not read")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("look")
    s.add_argument("--set", default=str(paths.HERE / "lookset.json"))
    s.add_argument("--only")
    s.add_argument("--also")
    s = sub.add_parser("count")
    s.add_argument("--tier")
    s.add_argument("--only")
    a = ap.parse_args()
    if not TOOL.exists():
        sys.exit(f"extract: build {TOOL.relative_to(paths.REPO)} first: cargo build --release -p examine --example designer")
    {"look": look, "count": count}[a.cmd](a)


if __name__ == "__main__":
    main()
