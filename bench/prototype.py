#!/usr/bin/env python3
"""The plan-then-draw prototype on the small two-layer look boards: for each layer, the plan as a
rubber-band sketch (crates/plan), the plan drawn as copper (crates/wire), and the designer's board.

    prototype.py [--only id,...] [--look RUN] [--no-kicad]

Needs `cargo build --release -p wire --example wire`. The designer's pictures are the look set's
(lookset.py RUN, by default the baseline's). Writes LAB/planned/prototype/: index.html and, per
board, wires.json, routed.kicad_pcb, one plan.LAYER and wire.LAYER picture per layer, and KiCad's
verdict on the routed board (after a refill) beside its verdict on the bare one.
"""
import argparse
import html
import json
import shutil
import subprocess
import sys
from pathlib import Path

import cairosvg

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import paths

LAB = paths.lab()
WIRE = paths.REPO / "target" / "release" / "examples" / "wire"
OUT = LAB / "planned" / "prototype"
# The boards the prototype starts on (router brief, section 7 item 5), and where each comes from.
BOARDS = [
    ("synthetic", "Synthetic bus board", "planned/bundles/synthetic_designer.kicad_pcb"),
    ("ulpi", "ULPI Pmod", "pcbench/truth/ULPI-Pmod_ULPI-Pmod/bare.kicad_pcb"),
    ("shift-in", "Shift-in 32 (HC165)", "pcbench/truth/Shift-in-32-HC165_shift-in/bare.kicad_pcb"),
    ("cal430fr", "CAL430FR", "pcbench/truth/CAL430FR_CAL430F/bare.kicad_pcb"),
]


def kicad(src, out):
    """KiCad's verdict on the routed board in `out`, with what the bare board has already taken
    off: (unconnected, the bare board's unconnected, {error type: count}), or a message."""
    import pcbench
    routed = out / "routed.kicad_pcb"
    if src.with_suffix(".kicad_pro").exists():
        shutil.copy(src.with_suffix(".kicad_pro"), routed.with_suffix(".kicad_pro"))
    try:
        if (src.parent / "bare.json").exists():
            bare = json.loads((src.parent / "bare.json").read_text())
            bare = (bare["unconnected"], bare["errors"])
        else:
            shutil.copy(src, out / "bare.kicad_pcb")
            bare = pcbench.drc(out / "bare.kicad_pcb", out, "bare")[:2]
        open_, errors, _ = pcbench.drc(routed, out, "routed")
    except pcbench.RefereeError as e:
        return str(e)
    return open_, bare[0], {t: n - bare[1].get(t, 0) for t, n in errors.items() if n > bare[1].get(t, 0)}


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--only")
    ap.add_argument("--look", default="truth-base-9512c88", help="the look-set page the designer's pictures come from")
    ap.add_argument("--no-kicad", action="store_true", help="leave KiCad's verdict out")
    a = ap.parse_args()
    if not WIRE.exists():
        sys.exit("prototype: build it first: cargo build --release -p wire --example wire")
    only = set(a.only.split(",")) if a.only else None
    e = html.escape
    rows = []
    for bid, title, board in BOARDS:
        if only and bid not in only:
            continue
        out = OUT / bid
        r = subprocess.run([WIRE, LAB / board, out], capture_output=True, text=True)
        if r.returncode != 0:
            rows.append(f"<h2>{e(title)}</h2><p class='bad'>the prototype failed: {e(r.stderr.strip()[-400:])}</p>")
            continue
        c = json.loads(r.stdout.strip().splitlines()[-1])
        (out / "counts.json").write_text(json.dumps(c, indent=1))
        figs = []
        for layer in c["layers"]:
            name = layer["layer"]
            for kind in ("plan", "wire"):
                svg = out / f"{kind}.{name}.svg"
                cairosvg.svg2png(url=str(svg), write_to=str(svg.with_suffix(".png")), output_width=1800)
            look = f"../lookset/{a.look}/{bid}/designer-{name}"
            designer = f"<a href='{e(look)}.svg'><img loading='lazy' src='{e(look)}.png'></a>" if (OUT / f"{look}.png").exists() else "<p class='none'>the designer drew no track here</p>"
            note = f"{layer['wires']} of {layer['strands']} strands drawn, {layer['bends']} bends" + (", a layer the designer poured" if layer["plane"] else "")
            pic = lambda kind: f"<a href='{e(bid)}/{kind}.{e(name)}.svg'><img loading='lazy' src='{e(bid)}/{kind}.{e(name)}.png'></a>"
            figs.append(f"<h3>{e(name)}</h3><div class='pair'><figure><figcaption>the plan</figcaption>{pic('plan')}</figure>"
                        f"<figure><figcaption>the plan drawn: {e(note)}</figcaption>{pic('wire')}</figure>"
                        f"<figure><figcaption>the designer's copper</figcaption>{designer}</figure></div>")
        why = "; ".join(f"{n}: {e(w)}" for w, n in c["why"].items())
        faults = ", ".join(f"{n} {e(t)}" for t, n in c["faults"].items()) or "none"
        text = (f"<p>The plan: <b>{c['strands']}</b> strands with <b>{c['vias']}</b> vias, <b>{c['left']}</b> connections with no strand; "
                f"{c['settled']['lost']} strands lost and {c['settled']['overfull']} gaps over their room once the unused via sites are taken out. "
                f"The drawing: <b>{c['wires']}</b> strands drawn, <b>{c['undrawn']}</b> not" + (f" ({why})" if why else "")
                + f". Our checker's faults in what was drawn: {faults}. Still open by our count: <b>{c['open']}</b>. "
                f"{c['seconds']['plan']:.1f} s to plan, {c['seconds']['draw'] * 1000:.0f} ms to draw.</p>")
        if not a.no_kicad:
            k = kicad(LAB / board, out)
            if isinstance(k, str):
                text += f"<p class='bad'>KiCad could not judge the drawn board: {e(k)}</p>"
            else:
                new = ", ".join(f"{n} {e(t)}" for t, n in sorted(k[2].items())) or "none"
                text += f"<p>KiCad, after a refill: <b>{k[0]}</b> unconnected (the bare board has {k[1]}), errors the drawn copper adds: <b>{new}</b>.</p>"
                (out / "kicad.json").write_text(json.dumps({"unconnected": k[0], "bare_unconnected": k[1], "new_errors": k[2]}, indent=1))
        rows.append(f"<h2>{e(title)}</h2>{text}" + "".join(figs))
    page = f"""<!doctype html><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>Plan-then-draw prototype</title>
<style>body{{font:16px/1.5 system-ui;margin:1.5rem auto;max-width:130rem;padding:0 1rem;background:#111;color:#ddd}}a{{color:#9cf}}
.pair{{display:grid;grid-template-columns:repeat(auto-fit,minmax(20rem,1fr));gap:1rem}}figure{{margin:0}}img{{width:100%;background:#fff}}
figcaption,.m{{color:#999;font-size:14px}}.bad{{color:#f88}}.none{{color:#777}}</style>
<p><a href="../">← Review pages</a></p>
<h1>Plan-then-draw prototype</h1>
<p class="m">The new router's first two stages on the four smallest look boards. <b>The plan</b> (left): each layer's room is cut
into triangles between the corners of the pads, holes and board edge; a triangle side between two obstacles is a gap, and one that is at
least half full is drawn, amber to red at full; each connection is a strand through the gaps it passes in order, pulled tight like a rubber band, and strands
never cross on a layer. <b>The plan drawn</b> (middle): each strand as the track with fewest bends, in 0/45/90, that crosses its
gaps in their order and keeps its side of every obstacle and of the strands beside it; every wire is put to the exact checker as it
is drawn and kept only if it passes. A dashed red line is a strand with no such path (or whose path the checker refused); a dashed yellow one a connection the plan has
no strand for. Nothing is mended afterwards.</p>
<p class="m">Seen wrong so far. In the plan: a connection takes any way round, however long, before it takes a via (the loops
round the end of a pad row on the synthetic board, most of CAL430FR); fine-pitch pads walled in by their neighbours' strands find
no via site; ground and supply connections are planned as strands where the designer used a pour. In the drawing: where the plan
is direct the copper looks drawn by hand, and a bundle comes out nested at even pitch; a strand planned the long way round is drawn
the long way round. A thin magenta line is a path the search found and the exact checker refused: the search holds a wire off the
corners and wires beside its own gaps only, so near crowded pads it can pass too close to something it was never told of. Wires
are drawn in the plan's order, each as late-turning as it may be, so the first of a bundle sets the shape for the rest.</p>
{''.join(rows)}
"""
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / "index.html").write_text(page)
    print(f"wrote {OUT / 'index.html'}")


if __name__ == "__main__":
    main()
