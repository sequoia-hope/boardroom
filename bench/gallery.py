#!/usr/bin/env python3
"""The gallery: every board of a tier run, each copper layer, ours beside the designer's, for the
owner to review and for the looks governance/ENGINEERING_CONSTITUTION.md asks for.

    gallery.py RUN/TAG [RUN/TAG ...] [--name NAME] [--before RUN/TAG] [--only id,...] [--jobs N]

Each RUN/TAG is a build's boards in a `tier.py` run (LAB/tier/RUN/TAG/ID/). The first is the main
one; others (our own boards' run, say) are added to the same pages. --before names the run the
main one is compared with: boards better and worse than there are listed first.

Writes LAB/gallery/NAME/ (NAME defaults to the first RUN): `index.html`, every board as a small
picture with what is open, to filter and sort; `board.html?id=ID`, one board, its layers to switch
on and off, ours and the designer's moved and zoomed together, what KiCad still calls unconnected
drawn as yellow lines and the errors our copper adds as red rings. Pours are drawn dim under the
tracks, so a track inside its own net's pour still shows. LAB/gallery/index.html goes to the
newest. LAB/gallery/flags.json ({id: a sentence}) marks boards the agent thinks look wrong.

A board already drawn from the same files is not drawn again. Book cores for a whole tier
(`reserve --here -c 8 -- python3 bench/gallery.py ... --jobs 8`): the lab is not in git.
"""
import argparse
import concurrent.futures
import html
import io
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import paths

LAB = paths.lab()
TRUTH = LAB / "pcbench" / "truth"
# A layer's colour: front, back, then the inner layers in turn.
COLOURS = {"F.Cu": "#e0483e", "B.Cu": "#4a8fe8"}
INNER = ["#3fbf6f", "#d9a531", "#b56be0", "#35c2c2", "#e07fb0", "#9aa83a"]
EDGE = "#c8c8c8"


def colour(layer, layers):
    inner = [l for l in layers if l not in COLOURS]
    return COLOURS.get(layer) or INNER[inner.index(layer) % len(INNER)]


def prepare(pcb, unfilled):
    """(copper layer names front to back, the board's box in mm) of a board, and a copy with
    every pour's fill taken out written to `unfilled`."""
    import pcbnew
    b = pcbnew.LoadBoard(str(pcb))
    # The layers' own names, not what the designer called them: the plotter and the colours go by these.
    names = [pcbnew.BOARD.GetStandardLayerName(l) for l in b.GetEnabledLayers().CuStack()]
    box = b.GetBoardEdgesBoundingBox()
    if box.GetWidth() <= 0 or box.GetHeight() <= 0:
        box = b.ComputeBoundingBox(False)
    for z in b.Zones():
        z.UnFill()
    pcbnew.SaveBoard(str(unfilled), b)
    mm = lambda v: v / 1e6
    return names, [mm(box.GetX()), mm(box.GetY()), mm(box.GetWidth()), mm(box.GetHeight())]


def plot(pcb, layers, out, stem, box, tint):
    """Each of `layers` of the board as OUT/STEM-LAYER.svg, coloured ink on nothing, cut to the
    box. The names drawn. (One layer a call: asked for several at once KiCad names each file by
    what the designer called the layer.)"""
    drawn = []
    for layer in layers:
        svg = out / f"{stem}-{layer}.svg"
        r = subprocess.run(["kicad-cli", "pcb", "export", "svg", "--mode-single", "--layers", layer, "--black-and-white",
                            "--page-size-mode", "0", "--exclude-drawing-sheet", "-o", str(svg), str(pcb)], capture_output=True, text=True)
        if r.returncode != 0 or not svg.exists():
            continue
        t = svg.read_text()
        t = re.sub(r'width="[^"]*" height="[^"]*" viewBox="[^"]*"', f'viewBox="{box[0]:.4f} {box[1]:.4f} {box[2]:.4f} {box[3]:.4f}"', t, count=1)
        # Holes are drawn white over the copper: black here, which adds nothing when the
        # layers are laid over each other.
        t = t.replace("#FFFFFF", "#\0").replace("#000000", tint(layer)).replace("#\0", "#000000")
        svg.write_text(t)
        drawn.append(layer)
    return drawn


def marks(report, bare):
    """(what KiCad calls unconnected, as pairs of places; its errors that are not on the bare
    board, or with no `bare` every error, as [kind, x, y])."""
    if not report.exists():
        return [], []
    d = json.loads(report.read_text())
    place = lambda i: [i["pos"]["x"], i["pos"]["y"]]
    lines = [[place(u["items"][0]), place(u["items"][1])] for u in d.get("unconnected_items", []) if len(u.get("items", [])) >= 2]
    key = lambda v: (v["type"], tuple(sorted(i["description"] for i in v.get("items", []))))
    had = {}
    if bare and bare.exists():
        for v in json.loads(bare.read_text()).get("violations", []):
            had[key(v)] = had.get(key(v), 0) + 1
    new = []
    for v in d.get("violations", []):
        if v.get("severity", "error") != "error":
            continue
        if had.get(key(v), 0) > 0:
            had[key(v)] -= 1
        elif v.get("items"):
            new.append([v["type"], *place(v["items"][0])])
    return lines, new


def errors(run_dir, truth, row):
    """A routed board's marks: what is unconnected, and KiCad's errors on it. Where the tier kept
    every error on the routed board (`kicad_errors`: the test set, judged by passing DRC
    outright) all of them; else those the routed copper adds to the bare board's, and only of
    the kinds the tier counted as added: an error the bare board has under other words is not
    ours."""
    report = run_dir / "routed.drc.json"
    if "kicad_errors" in row:
        return marks(report, None)
    lines, new = marks(report, next((p for p in (run_dir / "bare.drc.json", truth / "bare.drc.json") if p.exists()), None))
    return lines, [n for n in new if n[0] in (row.get("kicad_new") or {})]


def thumb(d, layers, box, width=360):
    """Ours, every layer laid over the others, as a small picture."""
    import cairosvg
    from PIL import Image, ImageChops
    h = max(1, round(width * box[3] / box[2]))
    img = Image.new("RGB", (width, h), (0, 0, 0))
    for layer in ["Edge.Cuts"] + layers[::-1]:
        f = d / f"ours-{layer}.svg"
        if f.exists():
            try:
                png = cairosvg.svg2png(url=str(f), output_width=width, output_height=h)
                one = Image.open(io.BytesIO(png)).convert("RGBA")
                flat = Image.new("RGB", one.size, (0, 0, 0))
                flat.paste(one, mask=one.split()[3])
                img = ImageChops.screen(img, flat)
            except Exception:
                pass
    img.save(d / "thumb.png", optimize=True)


def one(job):
    """One board drawn into its directory; the entry for boards.json."""
    bid, group, run_dir, out = job["id"], job["group"], Path(job["dir"]), Path(job["out"])
    d = out / "b" / bid
    d.mkdir(parents=True, exist_ok=True)
    ours = next((p for p in (run_dir / "routed.filled.kicad_pcb", run_dir / "routed.kicad_pcb") if p.exists()), None)
    truth = TRUTH / bid
    theirs = next((p for p in (truth / "designer.filled.kicad_pcb", truth / "designer.kicad_pcb") if p.exists()), None)
    entry = {"id": bid, "group": group, "row": job["row"], "before": job["before"], "layers": [], "sides": {}}
    if (truth / "designer.json").exists():
        j = json.loads((truth / "designer.json").read_text())
        # Every error KiCad finds on the designer's finished board, not only those its routing adds.
        report = truth / "designer.drc.json"
        count = sum(v.get("severity") == "error" for v in json.loads(report.read_text()).get("violations", [])) if report.exists() else sum((j.get("new_errors") or {}).values())
        entry["designer"] = {"open": j.get("unconnected"), "errors": count, "vias": j.get("vias"), "track_mm": round(j.get("wirelength") or 0)}
    if ours is None:
        entry["missing"] = "not routed: " + (job["row"].get("failed") or (f"stopped after {job['row']['stopped']} s" if "stopped" in job["row"] else "no board written"))
        return entry
    stamp = [str(p) + ":" + str(p.stat().st_mtime_ns) for p in (ours, theirs) if p]
    kept = d / "entry.json"
    if kept.exists():
        old = json.loads(kept.read_text())
        if old.get("stamp") == stamp:
            old.update({"group": group, "row": job["row"], "before": job["before"]})
            old["open"], old["new"] = errors(run_dir, truth, job["row"])
            return old
    box = None
    for side, pcb in (("ours", ours), ("designer", theirs)):
        if pcb is None:
            continue
        with tempfile.TemporaryDirectory() as tmp:
            bare = Path(tmp) / "unfilled.kicad_pcb"
            try:
                layers, its_box = prepare(pcb, bare)
            except Exception as e:
                entry["sides"][side] = {"failed": str(e)[-200:]}
                continue
            if box is None:
                pad = 0.02 * max(its_box[2], its_box[3]) + 0.5
                box = [its_box[0] - pad, its_box[1] - pad, its_box[2] + 2 * pad, its_box[3] + 2 * pad]
                entry["layers"], entry["box"] = layers, box
            tint = lambda l: EDGE if l == "Edge.Cuts" else colour(l, entry["layers"] if l in entry["layers"] else layers)
            tracks = plot(bare, layers + ["Edge.Cuts"], d, side, box, tint)
            pours = plot(pcb, layers, d, side + "-pour", box, tint)
            entry["sides"][side] = {"tracks": tracks, "pours": pours}
    if box is None:
        entry["missing"] = "could not be drawn"
        return entry
    entry["open"], entry["new"] = errors(run_dir, truth, job["row"])
    entry["colours"] = {l: colour(l, entry["layers"]) for l in entry["layers"]}
    thumb(d, entry["layers"], box)
    entry["stamp"] = stamp
    kept.write_text(json.dumps(entry))
    return entry


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("runs", nargs="+", help="RUN/TAG of a tier.py run")
    ap.add_argument("--name")
    ap.add_argument("--before", help="RUN/TAG the first run is compared with")
    ap.add_argument("--only")
    ap.add_argument("--jobs", type=int, default=2)
    a = ap.parse_args()
    name = a.name or a.runs[0].split("/")[0]
    out = LAB / "gallery" / name
    out.mkdir(parents=True, exist_ok=True)
    rows_of = lambda spec: json.loads((LAB / "tier" / spec.split("/")[0] / "results.json").read_text())["rows"][spec.split("/")[1]]
    before = rows_of(a.before) if a.before else {}
    only = set(a.only.split(",")) if a.only else None
    jobs = []
    for spec in a.runs:
        run, tag = spec.split("/")
        for bid, row in sorted(rows_of(spec).items()):
            if only is None or bid in only:
                jobs.append({"id": bid, "group": run, "dir": str(LAB / "tier" / run / tag / bid), "out": str(out), "row": row,
                             "before": before.get(bid) if spec == a.runs[0] else None})
    entries = []
    with concurrent.futures.ProcessPoolExecutor(a.jobs) as pool:
        for n, e in enumerate(pool.map(one, jobs, chunksize=1), 1):
            entries.append(e)
            if n % 25 == 0 or n == len(jobs):
                print(f"gallery: {n} of {len(jobs)}", file=sys.stderr, flush=True)
    flags = LAB / "gallery" / "flags.json"
    flagged = json.loads(flags.read_text()) if flags.exists() else {}
    # The pages read one small list; each board's marks stay in its own file.
    index = []
    for e in entries:
        (out / "b" / e["id"]).mkdir(parents=True, exist_ok=True)
        (out / "b" / e["id"] / "board.json").write_text(json.dumps({k: v for k, v in e.items() if k != "stamp"}))
        row, was = e["row"], e.get("before") or {}
        index.append({"id": e["id"], "group": e["group"], "layers": len(e.get("layers", [])), "missing": e.get("missing"),
                      "open": row.get("kicad_open", row.get("open")), "ours_open": row.get("open"), "new": sum((row.get("kicad_errors", row.get("kicad_new")) or {}).values()),
                      "was": was.get("kicad_open", was.get("open")) if was else None, "seconds": row.get("seconds"), "vias": row.get("vias"),
                      "designer": e.get("designer"), "flag": flagged.get(e["id"])})
    meta = {"name": name, "runs": a.runs, "before": a.before, "boards": index}
    (out / "boards.json").write_text(json.dumps(meta))
    for page in ("index.html", "board.html"):
        (out / page).write_text((HERE / "gallery" / page).read_text())
    names = sorted(p.name for p in (LAB / "gallery").iterdir() if (p / "boards.json").exists())
    (LAB / "gallery" / "index.html").write_text(
        f'<!doctype html><meta charset="utf-8"><meta http-equiv="refresh" content="0; url={html.escape(name)}/"><title>Gallery</title>'
        + "<ul>" + "".join(f'<li><a href="{html.escape(n)}/">{html.escape(n)}</a></li>' for n in names) + "</ul>")
    drawn = sum("missing" not in e for e in entries)
    print(f"gallery: {drawn} of {len(entries)} boards drawn; wrote {out}/index.html")


if __name__ == "__main__":
    main()
