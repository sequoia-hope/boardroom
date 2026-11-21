#!/usr/bin/env python3
"""A look at one place on one board: ours beside the designer's, same place, same scale, one row
for each copper layer. The picture governance/ENGINEERING_CONSTITUTION.md section 3 asks for
before a cause is settled on, a success claimed or the owner asked anything.

    look.py RUN/TAG ID [X Y HALF] [--layers F.Cu,B.Cu] [--no-pours] [-o OUT.png] [--width PX]

RUN/TAG is a build's boards in a `tier.py` run; ID the board. X Y is the middle of the place in
the board's own mm (as KiCad's reports and ours give them) and HALF how far it reaches each way;
without them, the whole board. Pours are drawn dim under the tracks. What KiCad still calls
unconnected on ours is drawn in yellow, the errors our copper adds ringed in red.

Writes OUT.png (default LAB/gallery/looks/ID[-X-Y].png) and prints its path: open it and look.
The board's layers are drawn once into LAB/gallery/looks/b/ID/ and kept.
"""
import argparse
import io
import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import gallery

LAB = gallery.LAB


def cut(svg, box, width, dim):
    """The file's picture of the box, `width` across, on black."""
    import cairosvg
    from PIL import Image
    h = max(1, round(width * box[3] / box[2]))
    t = re.sub(r'viewBox="[^"]*"', f'viewBox="{box[0]:.4f} {box[1]:.4f} {box[2]:.4f} {box[3]:.4f}"', Path(svg).read_text(), count=1)
    one = Image.open(io.BytesIO(cairosvg.svg2png(bytestring=t.encode(), output_width=width, output_height=h))).convert("RGBA")
    flat = Image.new("RGB", one.size, (0, 0, 0))
    flat.paste(one, mask=one.split()[3].point(lambda v: int(v * dim)))
    return flat


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("run")
    ap.add_argument("id")
    ap.add_argument("place", nargs="*", type=float, help="X Y HALF, mm")
    ap.add_argument("--layers")
    ap.add_argument("--no-pours", action="store_true")
    ap.add_argument("-o", "--out")
    ap.add_argument("--width", type=int, default=900)
    a = ap.parse_args()
    from PIL import Image, ImageChops, ImageDraw
    run, tag = a.run.split("/")
    rows = json.loads((LAB / "tier" / run / "results.json").read_text())["rows"][tag]
    if a.id not in rows:
        sys.exit(f"look: {a.id} is not in {a.run}")
    out = LAB / "gallery" / "looks"
    e = gallery.one({"id": a.id, "group": run, "dir": str(LAB / "tier" / run / tag / a.id), "out": str(out), "row": rows[a.id], "before": None})
    if "box" not in e:
        sys.exit(f"look: {a.id}: {e.get('missing')}")
    if a.place and len(a.place) != 3:
        sys.exit("look: a place is X Y HALF")
    box = [a.place[0] - a.place[2], a.place[1] - a.place[2], 2 * a.place[2], 2 * a.place[2]] if a.place else e["box"]
    layers = a.layers.split(",") if a.layers else e["layers"]
    d = out / "b" / a.id
    sides = [s for s in ("ours", "designer") if e["sides"].get(s, {}).get("tracks")]
    w, gap, head = a.width, 12, 26
    h = max(1, round(w * box[3] / box[2]))
    sheet = Image.new("RGB", (len(sides) * (w + gap) - gap, len(layers) * (h + head + gap) - gap), (18, 21, 27))
    pen = ImageDraw.Draw(sheet)
    px = lambda p: ((p[0] - box[0]) * w / box[2], (p[1] - box[1]) * w / box[2])
    for r, layer in enumerate(layers):
        for c, side in enumerate(sides):
            img = Image.new("RGB", (w, h), (0, 0, 0))
            for stem, dim in ((f"{side}-pour-{layer}", 0.3), (f"{side}-{layer}", 1.0), (f"{side}-Edge.Cuts", 1.0)):
                f = d / f"{stem}.svg"
                if f.exists() and not (a.no_pours and "-pour-" in stem):
                    img = ImageChops.screen(img, cut(f, box, w, dim))
            if side == "ours":
                ink = ImageDraw.Draw(img)
                for p, q in e.get("open", []):
                    ink.line([px(p), px(q)], fill=(255, 226, 74), width=2)
                for _, x, y in e.get("new", []):
                    cx, cy = px((x, y))
                    ink.ellipse([cx - 9, cy - 9, cx + 9, cy + 9], outline=(255, 59, 48), width=2)
            x0, y0 = c * (w + gap), r * (h + head + gap)
            pen.text((x0 + 4, y0 + 5), f"{'Ours' if side == 'ours' else 'Designer'}  {layer}", fill=(221, 225, 231))
            sheet.paste(img, (x0, y0 + head))
    name = a.id + (f"-{a.place[0]:g}-{a.place[1]:g}" if a.place else "")
    path = Path(a.out) if a.out else out / f"{name}.png"
    sheet.save(path)
    print(path)


if __name__ == "__main__":
    main()
