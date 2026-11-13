#!/usr/bin/env python3
"""The plan-then-draw prototype on the small two-layer look boards: for each, the plan as a
rubber-band sketch (crates/plan) beside the designer's board, per layer.

    prototype.py [--only id,...] [--look RUN]

Needs `cargo build --release -p plan --example plan`. The designer's pictures are the look set's
(lookset.py RUN, by default the baseline's). Writes LAB/planned/prototype/: index.html and, per
board, plan.json and one plan.LAYER.svg and .png per layer. Routes nothing: there is no copper.
"""
import argparse
import html
import json
import subprocess
import sys
from pathlib import Path

import cairosvg

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import paths

LAB = paths.lab()
PLAN = paths.REPO / "target" / "release" / "examples" / "plan"
OUT = LAB / "planned" / "prototype"
# The boards the prototype starts on (router brief, section 7 item 5), and where each comes from.
BOARDS = [
    ("synthetic", "Synthetic bus board", "planned/bundles/synthetic_designer.kicad_pcb"),
    ("ulpi", "ULPI Pmod", "pcbench/truth/ULPI-Pmod_ULPI-Pmod/bare.kicad_pcb"),
    ("shift-in", "Shift-in 32 (HC165)", "pcbench/truth/Shift-in-32-HC165_shift-in/bare.kicad_pcb"),
    ("cal430fr", "CAL430FR", "pcbench/truth/CAL430FR_CAL430F/bare.kicad_pcb"),
]


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--only")
    ap.add_argument("--look", default="truth-base-9512c88", help="the look-set page the designer's pictures come from")
    a = ap.parse_args()
    if not PLAN.exists():
        sys.exit("prototype: build the planner first: cargo build --release -p plan --example plan")
    only = set(a.only.split(",")) if a.only else None
    e = html.escape
    rows = []
    for bid, title, board in BOARDS:
        if only and bid not in only:
            continue
        out = OUT / bid
        r = subprocess.run([PLAN, LAB / board, out], capture_output=True, text=True)
        if r.returncode != 0:
            rows.append(f"<h2>{e(title)}</h2><p class='bad'>the planner failed: {e(r.stderr.strip()[-400:])}</p>")
            continue
        counts = json.loads(r.stdout.strip().splitlines()[-1])
        figs = []
        for layer in counts["layers"]:
            name = layer["layer"]
            svg = out / f"plan.{name}.svg"
            cairosvg.svg2png(url=str(svg), write_to=str(svg.with_suffix(".png")), output_width=1800)
            look = f"../lookset/{a.look}/{bid}/designer-{name}"
            designer = f"<a href='{e(look)}.svg'><img loading='lazy' src='{e(look)}.png'></a>" if (OUT / f"{look}.png").exists() else "<p class='none'>the designer drew no track here</p>"
            note = f"{layer['strands']} strands, {layer['triangles']} triangles" + (", a layer the designer poured: strands avoid it" if layer["plane"] else "")
            figs.append(f"<h3>{e(name)}</h3><div class='pair'><figure><figcaption>the plan: {e(note)}</figcaption><a href='{e(bid)}/plan.{e(name)}.svg'><img loading='lazy' src='{e(bid)}/plan.{e(name)}.png'></a></figure>"
                        f"<figure><figcaption>the designer's copper</figcaption>{designer}</figure></div>")
        why = "; ".join(f"{n}: {e(w)}" for w, n in counts["why"].items())
        rows.append(f"<h2>{e(title)}</h2><p><b>{counts['planned']}</b> strands planned with <b>{counts['vias']}</b> vias, <b>{counts['left']}</b> connections left"
                    + (f" ({why})" if why else "") + f". {counts['seconds'] * 1000:.0f} ms.</p>" + "".join(figs))
    page = f"""<!doctype html><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>Plan-then-draw prototype</title>
<style>body{{font:16px/1.5 system-ui;margin:1.5rem auto;max-width:110rem;padding:0 1rem;background:#111;color:#ddd}}a{{color:#9cf}}
.pair{{display:grid;grid-template-columns:repeat(auto-fit,minmax(20rem,1fr));gap:1rem}}figure{{margin:0}}img{{width:100%;background:#fff}}
figcaption,.m{{color:#999;font-size:14px}}.bad{{color:#f88}}.none{{color:#777}}</style>
<p><a href="../">← Review pages</a></p>
<h1>Plan-then-draw prototype: the plan</h1>
<p class="m">The new router's first stage on the four smallest look boards, before any copper exists. Each layer's room is cut into
triangles between the corners of the pads, holes and board edge. A triangle side between two obstacles is a gap: grey when empty,
green to red by how full it is. Each connection is a strand, a coloured line through the gaps it passes in order, pulled tight
like a rubber band; strands never cross on a layer. A grey disc that is not a pad is a via the plan put in, at one of a grid
of sites. Dashed red is a connection with no strand yet. Nothing here is drawn as tracks, and nothing is checked by KiCad.</p>
<p class="m">Seen wrong so far: a connection takes any way round, however long, before it takes a via; strands wobble between
the unused via sites; fine-pitch pads walled in by their neighbours' strands find no via site near enough; ground and supply
connections are planned as strands where the designer used a pour.</p>
{''.join(rows)}
"""
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / "index.html").write_text(page)
    print(f"wrote {OUT / 'index.html'}")


if __name__ == "__main__":
    main()
