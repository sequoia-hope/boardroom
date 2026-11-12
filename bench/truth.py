#!/usr/bin/env python3
"""Ground truth for the corpus: each board as its designer left it, and the same board bare.

    truth.py prepare [--tier T | --only ID,...] [--workers N] [--redo]
    truth.py report  [--tier T | --only ID,...]
    truth.py page    [--tier T | --only ID,...] [--boards N]

Why this exists. PCBench's `processed.kicad_pcb` is not the designer's board. Its cleaning
(PCBench/Scripts/Data_cleaning) deletes every copper zone, deletes every track and via on those
zones' nets, and routes those nets again with FreeRouting. So on a board that had pours, the
"designer's" ground and supply copper in `processed` is FreeRouting's, and the planes are gone.
The designer's board is `raw.kicad_pcb`.

`prepare` reads each board's raw file with pcbnew (which converts KiCad 4 and 5 files) and
writes, under LAB/pcbench/truth/ID/:

    designer.kicad_pcb   the raw board saved by this KiCad, its zones refilled by KiCad (or, on
                         the few boards its filler never returns on, with the file's own fills)
    bare.kicad_pcb       the same with every track and via removed and its zones kept, unfilled:
                         what a router is given
    truth.json           what was found: zones (net, layers, area, priority), tracks and vias,
                         board area, how it differs from `processed`, or the error

`page` writes LAB/planned/truth/: the counts, and for the boards where `processed` strays
furthest from the designer, each copper layer of the designer's board beside the same layer of
`processed`, to look at.

Nothing here enters the repository: the boards are their authors', under their licences
(`bench/README.md`). Run it through `~/.claude/bin/reserve`.
"""

import argparse
import concurrent.futures
import json
import os
import re
import subprocess
import sys
import uuid
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import paths

TIERS = paths.HERE / "pcbench_tiers.json"
# How long KiCad's zone filler gets on one board.
REFILL_S = 180

# Runs in its own process: pcbnew can crash on a board, and must not take the pool with it.
WORKER = r"""
import json, sys
import pcbnew
src, designer, bare = sys.argv[1:4]
b = pcbnew.LoadBoard(src)
mm2 = 1e-12
def layer_names(ls):
    return [b.GetLayerName(l) for l in ls.CuStack()]
zones = []
for z in b.Zones():
    if z.GetIsRuleArea():
        continue
    zones.append({
        "net": z.GetNetname(),
        "layers": layer_names(z.GetLayerSet()),
        "priority": z.GetAssignedPriority(),
        "outline_mm2": round(z.Outline().Area() * mm2, 3),
        "in_footprint": z.GetParentFootprint() is not None,
    })
tracks = [t for t in b.GetTracks()]
vias = sum(1 for t in tracks if t.Type() == pcbnew.PCB_VIA_T)
box = b.GetBoardEdgesBoundingBox()
info = {
    "copper_layers": b.GetCopperLayerCount(),
    "zones": zones,
    "tracks": len(tracks) - vias,
    "vias": vias,
    "footprints": len(b.GetFootprints()),
    "box_mm2": round(box.GetWidth() * box.GetHeight() * mm2, 3),
}
def filled_area(board):
    filled = {}
    for z in board.Zones():
        if z.GetIsRuleArea():
            continue
        for l in z.GetLayerSet().CuStack():
            name = board.GetLayerName(l)
            filled[name] = filled.get(name, 0.0) + z.GetFilledPolysList(l).Area() * mm2
    return {k: round(v, 3) for k, v in filled.items()}
# The fills as the file had them (KiCad converts old ones as it loads): kept if the refill fails.
info["filled_mm2"] = filled_area(b)
pcbnew.SaveBoard(designer, b)
for t in tracks:
    b.Remove(t)
for z in b.Zones():
    z.UnFill()
pcbnew.SaveBoard(bare, b)
print("TRUTH " + json.dumps(info), flush=True)
"""


# The designer's board refilled by this KiCad, in place. Apart from the first worker because the
# filler never returns on a few old boards; those keep the fills their file had.
REFILL = r"""
import json, sys
import pcbnew
path = sys.argv[1]
b = pcbnew.LoadBoard(path)
pcbnew.ZONE_FILLER(b).Fill(b.Zones())
mm2 = 1e-12
filled = {}
for z in b.Zones():
    if z.GetIsRuleArea():
        continue
    for l in z.GetLayerSet().CuStack():
        name = b.GetLayerName(l)
        filled[name] = filled.get(name, 0.0) + z.GetFilledPolysList(l).Area() * mm2
pcbnew.SaveBoard(path + ".refilled", b)
print("TRUTH " + json.dumps({k: round(v, 3) for k, v in filled.items()}), flush=True)
"""


def tier_ids(name):
    tiers = json.loads(TIERS.read_text())["tiers"]
    if name not in tiers:
        sys.exit(f"no tier {name} in {TIERS}: {', '.join(tiers)}")
    t = tiers[name]
    if isinstance(t, list):
        return sorted({b for part in t for b in tier_ids(part)})
    return sorted(t["boards"])


def chosen(a):
    pcbs = paths.corpus() / "PCBs"
    if a.only:
        ids = a.only.split(",")
    elif a.tier:
        ids = tier_ids(a.tier)
    else:
        ids = sorted(d.name for d in pcbs.iterdir() if d.is_dir())
    return [b for b in ids if (pcbs / b / "raw.kicad_pcb").exists()]


def fresh_uuids(path):
    """KiCad 5 boards number some footprints 0, 1, ...: each uuid that repeats gets a fresh one
    after its first use, as KiCad 9 and later refuse a board with duplicates."""
    seen = set()

    def fresh(m):
        if m[1] in seen:
            return f'(uuid "{uuid.uuid4()}")'
        seen.add(m[1])
        return m[0]

    path.write_text(re.sub(r'\(uuid "([^"]*)"\)', fresh, path.read_text()))


# Track length and via count per net name of a board, by pcbnew: the corpus's files come in
# several text layouts, so counting in the text misses whole boards.
COPPER = r"""
import json, sys
import pcbnew
b = pcbnew.LoadBoard(sys.argv[1])
out = {}
for t in b.GetTracks():
    e = out.setdefault(t.GetNetname(), [0.0, 0])
    if t.Type() == pcbnew.PCB_VIA_T:
        e[1] += 1
    else:
        e[0] += t.GetLength() * 1e-6
print("TRUTH " + json.dumps(out), flush=True)
"""


def copper_by_net(pcb, log):
    """{net name: [track mm, vias]} of a board file, or None if pcbnew cannot say."""
    try:
        with open(log, "w") as sink:
            subprocess.run([sys.executable, "-c", COPPER, str(pcb)], stdin=subprocess.DEVNULL, stdout=sink, stderr=subprocess.STDOUT, timeout=300)
    except subprocess.TimeoutExpired:
        return None
    line = next((l for l in Path(log).read_text(errors="replace").splitlines() if l.startswith("TRUTH ")), None)
    return json.loads(line[6:]) if line else None


def against_processed(b, zones):
    """How `processed` differs from the designer's board, on the nets the designer pours and on
    the others: track length and via count of each, by net name. `processed` is read as the
    harness prepared it (LAB/pcbench/boards/ID/board.kicad_pcb, saved by this KiCad)."""
    out = paths.lab() / "pcbench" / "truth" / b
    processed = paths.lab() / "pcbench" / "boards" / b / "board.kicad_pcb"
    if not processed.exists():
        return None
    d = copper_by_net(out / "designer.kicad_pcb", out / "copper.designer.log")
    p = copper_by_net(processed, out / "copper.processed.log")
    if d is None or p is None:
        return None
    pour_nets = {z["net"] for z in zones if z["net"]}

    def total(c, on_pour_nets, k):
        return round(sum(v[k] for n, v in c.items() if (n in pour_nets) == on_pour_nets), 2)

    def group(on):
        return {"mm_raw": total(d, on, 0), "mm_processed": total(p, on, 0), "vias_raw": total(d, on, 1), "vias_processed": total(p, on, 1)}

    return {"by": "pcbnew", "pour_nets": group(True), "other_nets": group(False)}


def differs(group):
    """More than 1% of track length, or any via, apart."""
    return abs(group["mm_raw"] - group["mm_processed"]) > 0.01 * max(group["mm_raw"], group["mm_processed"], 1.0) or group["vias_raw"] != group["vias_processed"]


def prepare_one(job):
    b, redo = job
    out = paths.lab() / "pcbench" / "truth" / b
    record = out / "truth.json"
    if record.exists() and not redo:
        info = json.loads(record.read_text())
        # An older record compared the files' text: compare again, without preparing again.
        if "error" not in info and (info.get("processed") or {}).get("by") != "pcbnew":
            info["processed"] = against_processed(b, info["zones"])
            record.write_text(json.dumps(info, indent=1))
        return b, info
    out.mkdir(parents=True, exist_ok=True)
    designer, bare = out / "designer.kicad_pcb", out / "bare.kicad_pcb"
    info = {"id": b}
    try:
        # pcbnew hangs on some old boards when its output is a pipe or it has a console to ask
        # on (it warns about legacy zone fills as it loads): give it a file and no input.
        log = out / "pcbnew.log"
        with open(log, "w") as sink:
            r = subprocess.run(
                [sys.executable, "-c", WORKER, str(paths.corpus() / "PCBs" / b / "raw.kicad_pcb"), str(designer), str(bare)],
                stdin=subprocess.DEVNULL, stdout=sink, stderr=subprocess.STDOUT, timeout=900,
            )
        text = log.read_text(errors="replace")
        line = next((l for l in text.splitlines() if l.startswith("TRUTH ")), None)
        if line is None or not designer.exists() or not bare.exists():
            info["error"] = ([l for l in text.strip().splitlines() if l.strip()] or [f"pcbnew exited {r.returncode}"])[-1][:300]
        else:
            info.update(json.loads(line[6:]))
            for f in (designer, bare):
                fresh_uuids(f)
            info["refilled"] = False
            if info["zones"]:
                refilled = Path(str(designer) + ".refilled")
                try:
                    with open(out / "refill.log", "w") as sink:
                        subprocess.run([sys.executable, "-c", REFILL, str(designer)], stdin=subprocess.DEVNULL, stdout=sink, stderr=subprocess.STDOUT, timeout=REFILL_S)
                    line = next((l for l in (out / "refill.log").read_text(errors="replace").splitlines() if l.startswith("TRUTH ")), None)
                    if line and refilled.exists():
                        refilled.replace(designer)
                        info["filled_mm2"], info["refilled"] = json.loads(line[6:]), True
                except subprocess.TimeoutExpired:
                    info["refill_error"] = f"KiCad's zone filler did not return in {REFILL_S} s; the file's own fills are kept"
                refilled.unlink(missing_ok=True)
            info["processed"] = against_processed(b, info["zones"])
    except subprocess.TimeoutExpired:
        info["error"] = "pcbnew took more than 900 s"
    record.write_text(json.dumps(info, indent=1))
    return b, info


def prepare(a):
    ids = chosen(a)
    workers = a.workers or int(os.environ.get("RESERVED_CORES", "4"))
    done = 0
    with concurrent.futures.ProcessPoolExecutor(workers) as pool:
        for b, info in pool.map(prepare_one, [(b, a.redo) for b in ids]):
            done += 1
            if "error" in info:
                print(f"{done}/{len(ids)} {b}: ERROR {info['error']}", flush=True)
            elif done % 25 == 0:
                print(f"{done}/{len(ids)}", flush=True)
    report(a)


def load(ids):
    out = []
    for b in ids:
        f = paths.lab() / "pcbench" / "truth" / b / "truth.json"
        if f.exists():
            out.append(json.loads(f.read_text()))
    return out


def summary(infos):
    ok = [i for i in infos if "error" not in i]
    poured = [i for i in ok if i["zones"]]
    multi = [i for i in ok if i["copper_layers"] > 2]
    inner = [i for i in multi if any(l not in ("F.Cu", "B.Cu") for z in i["zones"] for l in z["layers"])]
    rerouted = [i for i in poured if i.get("processed") and differs(i["processed"]["pour_nets"])]
    other_changed = [i for i in ok if i.get("processed") and differs(i["processed"]["other_nets"])]

    def plane_layers(i):
        # A layer whose poured copper covers at least half the board's box is a plane.
        return [l for l, a in i.get("filled_mm2", {}).items() if i["box_mm2"] > 0 and a >= 0.5 * i["box_mm2"]]

    return {
        "boards": len(infos),
        "prepared": len(ok),
        "errors": len(infos) - len(ok),
        "with_pours": len(poured),
        "multi_layer": len(multi),
        "multi_layer_with_inner_pours": len(inner),
        "with_a_plane_layer": sum(1 for i in ok if plane_layers(i)),
        "pour_net_copper_differs_in_processed": len(rerouted),
        "other_copper_differs_in_processed": len(other_changed),
        "not_refilled": sorted(i["id"] for i in poured if not i.get("refilled")),
    }


def report(a):
    infos = load(chosen(a))
    s = summary(infos)
    if a.json:
        print(json.dumps(s, indent=1))
        return
    print(f"{s['prepared']} of {s['boards']} boards prepared, {s['errors']} errors")
    print(f"  with pours: {s['with_pours']}; with a layer at least half poured: {s['with_a_plane_layer']}")
    print(f"  multi-layer: {s['multi_layer']}, of which poured on an inner layer: {s['multi_layer_with_inner_pours']}")
    print(f"  `processed` differs from the designer on the pour nets (track length by more than 1%, or vias): {s['pour_net_copper_differs_in_processed']}")
    print(f"  `processed` differs from the designer on the other nets (same test): {s['other_copper_differs_in_processed']}")
    if s["not_refilled"]:
        print(f"  KiCad could not refill {len(s['not_refilled'])} (file's own fills kept): {', '.join(s['not_refilled'][:8])}")
    for i in infos:
        if "error" in i:
            print(f"  ERROR {i['id']}: {i['error']}")


WIDTH = 900


def draw(pcb, layer, png):
    """One copper layer of a board with its outline, as a picture. True if drawn."""
    import cairosvg

    svg = png.with_suffix(".svg")
    for mode in ("2", "0"):
        r = subprocess.run(["kicad-cli", "pcb", "export", "svg", "--layers", f"{layer},Edge.Cuts", "--exclude-drawing-sheet",
                            "--page-size-mode", mode, "--black-and-white", "-o", str(svg), str(pcb)],
                           stdin=subprocess.DEVNULL, capture_output=True, text=True)
        if r.returncode != 0 or not svg.exists():
            continue
        try:
            cairosvg.svg2png(url=str(svg), write_to=str(png), output_width=WIDTH, background_color="white")
            svg.unlink()
            return True
        except Exception:
            continue
    return False


def page_one(job):
    b, layers, out = job
    truth = paths.lab() / "pcbench" / "truth" / b / "designer.kicad_pcb"
    processed = paths.lab() / "pcbench" / "boards" / b / "board.kicad_pcb"
    drawn = []
    for l in layers:
        stem = l.replace(".", "_")
        d = draw(truth, l, out / f"{b}.{stem}.designer.png")
        p = processed.exists() and draw(processed, l, out / f"{b}.{stem}.processed.png")
        drawn.append((l, d, p))
    return b, drawn


def page(a):
    import html

    infos = [i for i in load(chosen(a)) if "error" not in i]
    s = summary(load(chosen(a)))
    out = paths.lab() / "planned" / "truth"
    out.mkdir(parents=True, exist_ok=True)

    def strays(i):
        g = (i.get("processed") or {}).get("pour_nets")
        return abs(g["mm_raw"] - g["mm_processed"]) + 5 * abs(g["vias_raw"] - g["vias_processed"]) if g else 0

    # Half multi-layer, half two-layer, furthest apart first: both kinds of board are judged.
    ranked = sorted((i for i in infos if i["zones"]), key=strays, reverse=True)
    multi = [i for i in ranked if i["copper_layers"] > 2][: a.boards // 2]
    two = [i for i in ranked if i["copper_layers"] <= 2][: a.boards - len(multi)]
    shown = multi + two

    def layers_of(i):
        order = ["F.Cu"] + [f"In{k}.Cu" for k in range(1, i["copper_layers"] - 1)] + ["B.Cu"]
        return order[: i["copper_layers"]]

    workers = int(os.environ.get("RESERVED_CORES", "4"))
    with concurrent.futures.ProcessPoolExecutor(workers) as pool:
        pictures = dict(pool.map(page_one, [(i["id"], layers_of(i), out) for i in shown]))

    e = html.escape
    rows = []
    for i in shown:
        b, g, o = i["id"], i["processed"]["pour_nets"], i["processed"]["other_nets"]
        nets = sorted({z["net"] for z in i["zones"] if z["net"]})
        planes = ", ".join(f"{l} {100 * v / i['box_mm2']:.0f}%" for l, v in sorted(i["filled_mm2"].items()) if i["box_mm2"] > 0)
        rows.append(f"<h2 id='{e(b)}'>{e(b)}</h2>")
        rows.append(f"<p>{i['copper_layers']} copper layers. Pours on {e(', '.join(nets)) or 'no net'}; poured share of the board's box per layer: {e(planes)}."
                    f"{'' if i.get('refilled') else ' KiCad could not refill this board: the fills are the file&#39;s own.'}</p>")
        rows.append(f"<p>On the pour nets the designer drew <b>{g['mm_raw']:.0f} mm</b> of track and <b>{g['vias_raw']:.0f} vias</b>; "
                    f"<code>processed</code> has <b>{g['mm_processed']:.0f} mm</b> and <b>{g['vias_processed']:.0f} vias</b>. "
                    f"On the other nets: {o['mm_raw']:.0f} mm and {o['vias_raw']:.0f} vias against {o['mm_processed']:.0f} mm and {o['vias_processed']:.0f}.</p>")
        for l, d, p in pictures[b]:
            stem = l.replace(".", "_")
            rows.append(f"<h3>{e(l)}</h3><div class='pair'>")
            for kind, ok, label in (("designer", d, "the designer's board (raw), pours filled"), ("processed", p, "processed: what was called the designer's")):
                rows.append(f"<figure><img loading='lazy' src='{e(b)}.{stem}.{kind}.png' alt='{e(l)} {kind}'><figcaption>{label}</figcaption></figure>" if ok
                            else f"<figure><p class='none'>not drawn</p><figcaption>{label}</figcaption></figure>")
            rows.append("</div>")
    index = "".join(f"<li><a href='#{e(i['id'])}'>{e(i['id'])}</a> ({i['copper_layers']} layers)</li>" for i in shown)
    (out / "index.html").write_text(f"""<!doctype html><meta charset=utf-8><meta name=viewport content="width=device-width,initial-scale=1">
<title>Ground truth: the designers' boards</title>
<style>
body{{font:16px/1.5 system-ui,sans-serif;margin:1rem auto;max-width:1900px;padding:0 1rem;color:#111;background:#fff}}
h2{{margin-top:2.5rem;border-top:1px solid #ccc;padding-top:1rem;overflow-wrap:anywhere}}
.pair{{display:grid;grid-template-columns:1fr 1fr;gap:1rem}} @media (max-width:800px){{.pair{{grid-template-columns:1fr}}}}
figure{{margin:0}} img{{width:100%;height:auto;border:1px solid #ddd}} figcaption{{color:#555;font-size:.9rem}}
.none{{padding:3rem;text-align:center;color:#888;border:1px dashed #ccc}} table{{border-collapse:collapse}} td,th{{padding:.2rem .8rem;text-align:left;border-bottom:1px solid #eee}}
</style>
<h1>Ground truth: the designers' boards</h1>
<p>PCBench's <code>processed</code> boards were used as "the designer's routing". They are not: the corpus's cleaning removes
every copper pour, removes every track and via on the pours' nets, and routes those nets again with FreeRouting. The designer's
board is <code>raw</code>. These are the boards prepared from <code>raw</code> (<code>bench/truth.py</code>), counted, and the
ones where <code>processed</code> strays furthest, drawn layer by layer beside it.</p>
<table>
<tr><th>Boards looked at</th><td>{s['boards']}</td></tr>
<tr><th>Prepared from raw</th><td>{s['prepared']} ({s['errors']} failed)</td></tr>
<tr><th>With pours</th><td>{s['with_pours']}</td></tr>
<tr><th>With a layer at least half poured</th><td>{s['with_a_plane_layer']}</td></tr>
<tr><th>Multi-layer</th><td>{s['multi_layer']}, of which {s['multi_layer_with_inner_pours']} pour an inner layer</td></tr>
<tr><th>Pour-net copper differs in <code>processed</code></th><td>{s['pour_net_copper_differs_in_processed']} (track length by more than 1%, or via count)</td></tr>
<tr><th>Other nets' copper differs in <code>processed</code></th><td>{s['other_copper_differs_in_processed']}</td></tr>
<tr><th>KiCad could not refill</th><td>{len(s['not_refilled'])}</td></tr>
</table>
<h2>Boards drawn</h2><ul>{index}</ul>
{''.join(rows)}
""")
    print(f"wrote {out / 'index.html'}: {len(shown)} boards")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    for name in ("prepare", "report", "page"):
        s = sub.add_parser(name)
        s.add_argument("--tier")
        s.add_argument("--only")
        s.add_argument("--json", action="store_true")
        if name == "page":
            s.add_argument("--boards", type=int, default=10)
        if name == "prepare":
            s.add_argument("--workers", type=int)
            s.add_argument("--redo", action="store_true")
    a = ap.parse_args()
    {"prepare": prepare, "report": report, "page": page}[a.cmd](a)


if __name__ == "__main__":
    main()
