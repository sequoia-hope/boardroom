#!/usr/bin/env python3
"""The plan-then-draw prototype on the small two-layer look boards: for each layer, the plan as a
rubber-band sketch (crates/plan), the plan drawn as copper (crates/wire), and the designer's board.

    prototype.py [--set look|ours] [--only id,...] [--look RUN] [--no-kicad] [--cap SECONDS]

--only runs those boards again and leaves the rest of the page as it was. With WIRE_FIRST set
the router stops at its first plan and drawing (nothing laid again, no pours), and the page says
so of each board run that way: for a board the whole run does not finish.

Needs `cargo build --release -p wire --example wire` and `cargo build --release -p kicad-text`. The designer's pictures are the look set's
(lookset.py RUN, by default the baseline's). Writes LAB/planned/prototype/: index.html and, per
board, wires.json, routed.kicad_pcb, one plan.LAYER and wire.LAYER picture per layer, and KiCad's
verdict on the routed board (after a refill) beside its verdict on the bare one.

`--set ours` is the same on our own boards, into LAB/planned/ours/: each is stripped and routed
from its pads, and the picture beside it is the board's copper as it is in its file. Boards are
routed side by side, as many as $RESERVED_CORES (book them with `reserve`), each under a
memory cap and stopped after --cap seconds.
"""
import argparse
import collections
import concurrent.futures
import html
import json
import os
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
# The boards shown, and where each comes from: the four the prototype started on (router brief,
# section 7 item 5), and boards of other kinds from the corpus.
BOARDS = [
    ("synthetic", "Synthetic bus board: two buses between pin rows", "planned/bundles/synthetic_designer.kicad_pcb"),
    ("baofeng", "Baofeng interface: a small board of surface-mount parts", "pcbench/truth/Baofeng-Interface_BaofengInterface/bare.kicad_pcb"),
    ("orangepi", "Orange Pi Zero node: 164 parts, headers and logic", "pcbench/truth/Hardware_Playground_orange_pi_zero_node/bare.kicad_pcb"),
    ("piscium", "PISCIUM: a large through-hole board", "pcbench/truth/nichiden27_PISCIUM/bare.kicad_pcb"),
    ("saw", "SAW filter board: four layers, radio", "pcbench/truth/radio_saw_dcc6c/bare.kicad_pcb"),
    ("dps1200", "DPS-1200FB adapter: a power board", "pcbench/truth/DPS-1200FB_Adapter_Adapter/bare.kicad_pcb"),
    ("shift-in", "Shift-in 32 (HC165): through-hole logic", "pcbench/truth/Shift-in-32-HC165_shift-in/bare.kicad_pcb"),
    ("ulpi", "ULPI Pmod: a fine-pitch chip and two headers", "pcbench/truth/ULPI-Pmod_ULPI-Pmod/bare.kicad_pcb"),
    ("cal430fr", "CAL430FR: a dense two-layer board", "pcbench/truth/CAL430FR_CAL430F/bare.kicad_pcb"),
    ("kiwisdr", "KiwiSDR antenna board: radio, its tracks 1 mm wide by default", "pcbench/truth/KiwiSDR_PCB_ant/bare.kicad_pcb"),
    ("phone-rtty", "Phone RTTY interface: through-hole, with labels in copper", "pcbench/truth/phone_rtty_interface_phone_rtty_rev_b/bare.kicad_pcb"),
    ("domotics", "Domotics power supply: through-hole power, pins of several pads", "pcbench/truth/domotics_power-supply/bare.kicad_pcb"),
    ("induction", "Induction hob temperature sensor: 9 mm across and crowded", "pcbench/truth/induction-hob_temperature-sensor/bare.kicad_pcb"),
]
# Our own boards: (id, title, the board). Those not in this repository are copies in the lab
# (LAB/ours/ID/, from the old router's jobs).
OURS = [
    ("easy2350", "easy2350: the RP2350 example board, four layers", paths.REPO / "examples/easy2350/hw/easy2350/a/easy2350.kicad_pcb"),
    ("pendant-p1-66", "Round pendant, parallelogram p1-66: four layers", paths.REPO / "fixtures/parallelogram-p1-66.kicad_pcb"),
    ("earring", "Simple earring: 45 LEDs round a ring, four layers", paths.REPO / "fixtures/earring.kicad_pcb"),
    ("earring-v2a", "Earring v2a: 96 LEDs on a two-layer flex strip", LAB / "ours/earring-v2a/earring-v2a.kicad_pcb"),
    ("rgb-earring", "RGB earring (RP2040): two layers, 105 parts", LAB / "ours/rgb-earring/earring.kicad_pcb"),
    ("pendant-e", "Round pendant e: eight layers, 235 LEDs, five chip-scale drivers", LAB / "ours/pendant-e/round-pendant.kicad_pcb"),
    ("p2-132-usb6", "Round pendant, parallelogram p2-132-usb6: four layers, 132 LEDs", LAB / "ours/p2-132-usb6/parallelogram.kicad_pcb"),
]
# Copper text is shaped by the text helper, built beside the examples' directory; without it a
# label is a box that covers the pads beside its letters.
TEXT = paths.REPO / "target" / "release" / "kicad-text"


def kicad(src, out):
    """KiCad's verdict on the routed board in `out`, with what the bare board has already taken
    off: (unconnected, the bare board's unconnected, {error type: count}), or a message."""
    import pcbench
    routed = out / "routed.kicad_pcb"
    # The board's rules go with both: the project file and the custom rules beside it.
    for suffix in (".kicad_pro", ".kicad_dru"):
        if src.with_suffix(suffix).exists():
            shutil.copy(src.with_suffix(suffix), routed.with_suffix(suffix))
            shutil.copy(src.with_suffix(suffix), (out / "bare.kicad_pcb").with_suffix(suffix))
    try:
        if (src.parent / "bare.json").exists():
            bare = json.loads((src.parent / "bare.json").read_text())
            bare = (bare["unconnected"], bare["errors"])
        else:
            # The board stripped, as the prototype wrote it.
            bare = pcbench.drc(out / "bare.kicad_pcb", out, "bare")[:2]
        open_, errors, _ = pcbench.drc(routed, out, "routed")
    except pcbench.RefereeError as e:
        return str(e)
    new = {t: n - bare[1].get(t, 0) for t, n in errors.items() if n > bare[1].get(t, 0)}
    # An error that names no routed copper is the routing's all the same where the stripped
    # board has fewer of its kind: a net over its length rule names its two pads.
    reports = [out / "routed.drc.json", out / "bare.drc.json"]
    if all(r.exists() for r in reports) and not (src.parent / "bare.json").exists():
        kinds = [collections.Counter(v["type"] for v in json.loads(r.read_text()).get("violations", []) if v["severity"] == "error") for r in reports]
        for t, n in kinds[0].items():
            if n > kinds[1].get(t, 0):
                new[t] = max(new.get(t, 0), n - kinds[1].get(t, 0))
    return open_, bare[0], new


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--set", choices=["look", "ours"], default="look", help="the look boards, or our own")
    ap.add_argument("--cap", type=int, default=1800, help="seconds a board may take")
    ap.add_argument("--only")
    ap.add_argument("--look", default="truth-base-9512c88", help="the look-set page the designer's pictures come from")
    ap.add_argument("--no-kicad", action="store_true", help="leave KiCad's verdict out")
    a = ap.parse_args()
    if not WIRE.exists():
        sys.exit("prototype: build it first: cargo build --release -p wire --example wire")
    only = set(a.only.split(",")) if a.only else None
    e = html.escape
    ours = a.set == "ours"
    OUT = LAB / "planned" / ("ours" if ours else "prototype")
    # What the picture beside each layer is.
    beside = "the copper in the board file now" if ours else "the designer's copper"
    every = [(bid, title, Path(board) if ours else LAB / board) for bid, title, board in (OURS if ours else BOARDS)]
    boards = [b for b in every if not only or b[0] in only]
    env = dict(os.environ)
    if TEXT.exists():
        env.setdefault("ULTRAROUTE_TEXT_HELPER", str(TEXT))

    def route(job):
        bid, _, board = job
        cmd = ["systemd-run", "--user", "--scope", "-q", "-p", "MemoryMax=8G", "-p", "MemorySwapMax=0", "timeout", str(a.cap), WIRE, board, OUT / bid]
        return subprocess.run(cmd, capture_output=True, text=True, env=env)

    with concurrent.futures.ThreadPoolExecutor(int(os.environ.get("RESERVED_CORES", "1"))) as pool:
        ran = list(pool.map(route, boards))
    rows = {}
    for (bid, title, board), r in zip(boards, ran):
        out = OUT / bid
        if r.returncode == 124:
            rows[bid] = f"<h2>{e(title)}</h2><p class='bad'>not routed: it was stopped after {a.cap} s.</p>"
            continue
        if r.returncode != 0:
            rows[bid] = f"<h2>{e(title)}</h2><p class='bad'>the prototype failed: {e(r.stderr.strip()[-400:])}</p>"
            continue
        c = json.loads(r.stdout.strip().splitlines()[-1])
        (out / "counts.json").write_text(json.dumps(c, indent=1))
        figs = []
        for layer in c["layers"]:
            name = layer["layer"]
            for kind in ("plan", "wire"):
                svg = out / f"{kind}.{name}.svg"
                cairosvg.svg2png(url=str(svg), write_to=str(svg.with_suffix(".png")), output_width=1800)
            # The designer's board: the look set's picture of it, or one drawn here from the
            # designer's own file beside the bare board.
            look = f"../lookset/{a.look}/{bid}/designer-{name}"
            theirs = board if ours else board.parent / "designer.kicad_pcb"
            if (ours or not (OUT / f"{look}.png").exists()) and theirs.exists():
                import lookset
                if lookset.draw(theirs, name, out / f"designer-{name}"):
                    look = f"{bid}/designer-{name}"
            designer = f"<a href='{e(look)}.svg'><img loading='lazy' src='{e(look)}.png'></a>" if (OUT / f"{look}.png").exists() else "<p class='none'>no track here</p>"
            note = f"{layer['wires']} of {layer['strands']} strands drawn, {layer['bends']} bends" + (", a layer the designer poured" if layer["plane"] else "")
            pic = lambda kind: f"<a href='{e(bid)}/{kind}.{e(name)}.svg'><img loading='lazy' src='{e(bid)}/{kind}.{e(name)}.png'></a>"
            figs.append(f"<h3>{e(name)}</h3><div class='pair'><figure><figcaption>the plan</figcaption>{pic('plan')}</figure>"
                        f"<figure><figcaption>the plan drawn: {e(note)}</figcaption>{pic('wire')}</figure>"
                        f"<figure><figcaption>{beside}</figcaption>{designer}</figure></div>")
        why = "; ".join(f"{n}: {e(w)}" for w, n in c["why"].items())
        first = "<p class='bad'>The first pass only: the whole run was stopped unfinished, so nothing here was laid again and the nets with pours were not routed at all.</p>" if "WIRE_FIRST" in os.environ else ""
        faults = ", ".join(f"{n} {e(t)}" for t, n in c["faults"].items()) or "none"
        text = (f"<p>The plan: <b>{c['strands']}</b> strands with <b>{c['vias']}</b> vias, <b>{c['left']}</b> connections with no strand. "
                f"The drawing: <b>{c['wires']}</b> strands drawn" + (f" ({c['as_planned']} of them at the plan's own angles)" if c.get('as_planned') else "")
                + f", <b>{c['undrawn']}</b> not" + (f" ({why})" if why else "")
                + (f", and <b>{c['ties']}</b> spokes drawn by hand from pads the wires left too few to their pour" if c.get('ties') else "")
                + f". Our checker's faults in what was drawn: {faults}. Still open by our count: <b>{c['open']}</b>. "
                f"{c['seconds']['draw']:.1f} s in all.</p>")
        if not a.no_kicad:
            k = kicad(board, out)
            if isinstance(k, str):
                text += f"<p class='bad'>KiCad could not judge the drawn board: {e(k)}</p>"
            else:
                new = ", ".join(f"{n} {e(t)}" for t, n in sorted(k[2].items())) or "none"
                text += f"<p>KiCad, after a refill: <b>{k[0]}</b> unconnected (the bare board has {k[1]}), errors the drawn copper adds: <b>{new}</b>.</p>"
                (out / "kicad.json").write_text(json.dumps({"unconnected": k[0], "bare_unconnected": k[1], "new_errors": k[2]}, indent=1))
        rows[bid] = f"<h2>{e(title)}</h2>{first}{text}" + "".join(figs)
    # Each board's part of the page is kept beside its files: a run of some boards (--only)
    # leaves the others on the page as they were.
    for bid, row in rows.items():
        (OUT / bid).mkdir(parents=True, exist_ok=True)
        (OUT / bid / "row.html").write_text(row)
    rows = [rows.get(bid) or ((OUT / bid / "row.html").read_text() if (OUT / bid / "row.html").exists() else "") for bid, _, _ in every]
    page = f"""<!doctype html><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>Plan-then-draw prototype</title>
<style>body{{font:16px/1.5 system-ui;margin:1.5rem auto;max-width:130rem;padding:0 1rem;background:#111;color:#ddd}}a{{color:#9cf}}
.pair{{display:grid;grid-template-columns:repeat(auto-fit,minmax(20rem,1fr));gap:1rem}}figure{{margin:0}}img{{width:100%;background:#fff}}
figcaption,.m{{color:#999;font-size:14px}}.bad{{color:#f88}}.none{{color:#777}}</style>
<p><a href="../">← Review pages</a></p>
<h1>{'The new router on our own boards' if ours else 'Plan-then-draw prototype'}</h1>
{"<p>Each board is stripped of its tracks and vias and routed from its pads by the new router, as it is today. Beside each layer is the copper the board file has now. What the router does and how to read the pictures is told on <a href='../prototype/'>the prototype page</a>; in short:</p>" if ours else ""}
<p class="m">The new router's two stages. <b>The plan</b> (left): each layer's room is cut into triangles between the corners of
the pads, holes and board edge; a triangle side between two obstacles is a gap, and one that is at least half full is drawn, amber
to red at full. Each connection is a strand through the gaps it passes in order, pulled tight like a rubber band; strands never
cross on a layer, and a via is put in only where a connection wants one. Two connections of one net that go the same way share
one track and part where they part. A board that is not whole is planned again with each fine-pitch part's way out settled first:
for each row of its pads a row of vias a little way out, one for each pad, in the pads' order. A connection with no way has the connections in its way
lifted and laid again. <b>The plan drawn</b> (middle): each strand, alone, as pieces in 0/45/90 that each pass an end of the wire or
lie as near a corner as the wire may come to it, given the strands the plan has between; so wires that pass a corner together come
out side by side at even pitch. Every wire is put to the exact checker and kept only if it passes. Where a wire cannot be drawn, its
connection alone is planned again, narrower if the rules let it be and then barred from the gap it stuck in, and the rest stays as
drawn. A track is as wide as its net's class asks unless that width has no way: then it is halved, down to the least the board's
rules allow. A wire that may not reach the middle of its pad (a pad on the board's edge, say) stops where it comes onto the pad. <b>A net with a pour is left to its
pour</b>: the other nets are planned and drawn first, the pours are filled round them (pale, in the net's colour), and only what a pour
leaves apart is planned, to the nearest pad or to a via standing in the pour; a connection whose via the wires laid after it cut off
from its pour is laid again. A pad the pour joins by thermal spokes keeps as
many as the board's rules ask: where the wires shut in the pour a spoke ends in, so that KiCad would not count it, the pad is drawn
a spoke by hand (a short track straight out of it into open pour), or those wires are laid another way. A dashed red line is a strand still not drawn; a dashed
yellow one a connection the plan has no strand for. <b>{beside.capitalize()}</b> (right) is the board as {'it was routed before, by hand or by the old router' if ours else 'its designer routed it'}.</p>
{''.join(rows)}
"""
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / "index.html").write_text(page)
    print(f"wrote {OUT / 'index.html'}")


if __name__ == "__main__":
    main()
