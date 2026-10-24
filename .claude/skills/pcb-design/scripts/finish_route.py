#!/usr/bin/env python3
"""finish_route.py -- route the last few connections the autorouter left.

    finish_route.py hw/main/a                       every unrouted signal, on F.Cu and B.Cu
    finish_route.py hw/main/a --layers F.Cu,B.Cu,In2.Cu --grid 0.05
    finish_route.py hw/main/a --dry-run             report what it would route

Freerouting usually gets a dense board to within one or two connections and
then stalls (references/routing.md). This is a plain maze router (A* on a
grid) for those last ones, not a replacement for it:

  * it reads `kicad-cli pcb drc` for unconnected items, skips nets that have
    a plane on an inner layer (stitch.py joins those), and routes each
    remaining pair, shortest first
  * obstacles are every other net's pads, tracks, vias and zone copper on
    each routing layer, grown by the larger of the two nets' clearances and
    half the track width; vias also need room on every copper layer except
    other nets' planes (the plane fill flows around them), clear of holes,
    keepouts and the board edge
  * track width, via size and clearance come from the net's class in the
    project file
  * the route is drawn as tracks and vias in a group named "finish_route"
    (or --tag); a --clear re-run removes that group first
  * --nets routes only the named nets, and --lock locks them: a maze-routed
    pre-route for a net the autorouter handles badly, drawn before it runs

Run it after autorouting and before the outer-layer pours (a ground pour is an
obstacle to every other net), then add the pours, stitch and refill, and run
DRC: the grid is coarse next to KiCad's own checks, and DRC is the judge.
"""
from __future__ import annotations

import argparse
import fnmatch
import heapq
import json
import math
import re
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
from kicad_files import find_project, load_pro  # noqa: E402

TAG = "finish_route"


def drc_unconnected(pcb: Path) -> list[dict]:
    with tempfile.TemporaryDirectory() as tmp:
        out = Path(tmp) / "drc.json"
        subprocess.run(["kicad-cli", "pcb", "drc", "--format", "json", "--severity-error", "-o", str(out),
                        str(pcb)], capture_output=True, text=True)
        return json.loads(out.read_text()).get("unconnected_items", [])


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("project", type=Path)
    ap.add_argument("--layers", default="F.Cu,B.Cu")
    ap.add_argument("--grid", type=float, default=0.05, help="mm")
    ap.add_argument("--via-cost", type=float, default=40, help="in grid steps")
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--clear", action="store_true", help="remove an earlier run's tracks first")
    ap.add_argument("--rip", type=int, default=3,
                    help="rip-up rounds: when a connection fails, remove the routing of the nets crowding "
                         "its ends, route it first, then re-route them")
    ap.add_argument("--nets", default="", help="comma-separated: route only these nets (no rip-up of others)")
    ap.add_argument("--lock", action="store_true", help="lock what is drawn (a pre-route the autorouter keeps)")
    ap.add_argument("--tag", default=TAG, help="group name; a --clear run removes this group only")
    ap.add_argument("--first", default="", help=argparse.SUPPRESS)
    ap.add_argument("--rip-nets", default="", help=argparse.SUPPRESS)
    ap.add_argument("--child", action="store_true", help=argparse.SUPPRESS)
    args = ap.parse_args()
    if not args.child and not args.dry_run:
        return drive(args)
    import pcbnew

    _, _, pcb_path = find_project(args.project)
    todo = drc_unconnected(pcb_path)

    b = pcbnew.LoadBoard(str(pcb_path))
    ds = b.GetDesignSettings()
    FromMM, ToMM = pcbnew.FromMM, pcbnew.ToMM
    pro = load_pro(pcb_path.with_suffix(".kicad_pro")).get("net_settings", {})
    classes = {c["name"]: c for c in pro.get("classes", [])}
    patterns = [(p["pattern"], p["netclass"]) for p in pro.get("netclass_patterns", [])]

    def cls(name):
        for pat, c in patterns:
            if fnmatch.fnmatchcase(name, pat) and c in classes:
                return classes[c]
        return classes.get("Default", {})

    def clr(name):
        return max(FromMM(cls(name).get("clearance", 0.2)), ds.m_MinClearance)

    # Snapshot (pcbnew proxies go bad after Remove()), then drop an earlier run.
    groups = list(b.Groups())
    tracks = list(b.GetTracks())
    old = [t for t in tracks if t.GetParentGroup() is not None and t.GetParentGroup().GetName() == args.tag]
    old_groups = [gr for gr in groups if gr.GetName() == args.tag]
    if old and not args.clear and not args.child:
        sys.exit(f"{len(old)} items from an earlier finish_route run are on the board; "
                 "--clear removes them first")
    zones = list(b.Zones())
    pads = [p for f in b.GetFootprints() for p in f.Pads()]
    layers = [b.GetLayerID(n) for n in args.layers.split(",")]
    copper = [l for l in range(pcbnew.PCB_LAYER_ID_COUNT) if pcbnew.IsCopperLayer(l) and b.IsLayerEnabled(l)]
    plane_nets = {z.GetNetname() for z in zones if not z.GetIsRuleArea()
                  and any(z.IsOnLayer(l) for l in copper if l not in (pcbnew.F_Cu, pcbnew.B_Cu))}
    # Nets with any zone (a copied regulator layout, a pour) connect through
    # copper this script can't reason about: report them, never route them.
    zone_nets = {z.GetNetname() for z in zones if not z.GetIsRuleArea()} - plane_nets
    keepouts = [z for z in zones if z.GetIsRuleArea() and z.GetDoNotAllowCopperPour()]
    if keepouts:
        print(f"  note: {len(keepouts)} no-pour rule areas present; zones they cover can't fill, so "
              "their nets may look unconnected")

    # Board grid.
    edge = b.GetBoardEdgesBoundingBox()
    g = FromMM(args.grid)
    x0, y0 = edge.GetLeft(), edge.GetTop()
    nx, ny = edge.GetWidth() // g + 1, edge.GetHeight() // g + 1
    xs = x0 + np.arange(nx) * g
    ys = y0 + np.arange(ny) * g
    outline = pcbnew.SHAPE_POLY_SET()
    b.GetBoardPolygonOutlines(outline)

    def rasterize(poly: "pcbnew.SHAPE_POLY_SET", mask: np.ndarray):
        """Set mask cells whose centres fall inside poly (even-odd over outlines and holes)."""
        for i in range(poly.OutlineCount()):
            rings = [poly.COutline(i)] + [poly.CHole(i, h) for h in range(poly.HoleCount(i))]
            bb = poly.COutline(i).BBox()
            ix0 = max(0, (bb.GetLeft() - x0) // g)
            ix1 = min(nx, (bb.GetRight() - x0) // g + 2)
            iy0 = max(0, (bb.GetTop() - y0) // g)
            iy1 = min(ny, (bb.GetBottom() - y0) // g + 2)
            if ix0 >= ix1 or iy0 >= iy1:
                continue
            X, Y = np.meshgrid(xs[ix0:ix1], ys[iy0:iy1])
            inside = np.zeros(X.shape, bool)
            for ring in rings:
                pts = np.array([(ring.CPoint(k).x, ring.CPoint(k).y) for k in range(ring.PointCount())], float)
                if len(pts) < 3:
                    continue
                qx, qy = pts[:, 0], pts[:, 1]
                rx, ry = np.roll(qx, -1), np.roll(qy, -1)
                for ax, ay, bx, by in zip(qx, qy, rx, ry):
                    if ay == by:
                        continue
                    cond = (ay > Y) != (by > Y)
                    xcross = ax + (Y - ay) * (bx - ax) / (by - ay)
                    inside ^= cond & (X < xcross)
            mask[iy0:iy1, ix0:ix1] |= inside

    def item_poly(item, layer, grow):
        poly = pcbnew.SHAPE_POLY_SET()
        item.TransformShapeToPolygon(poly, layer, int(grow), FromMM(0.005), pcbnew.ERROR_OUTSIDE)
        return poly

    routed = []
    pending = []
    for u in todo:
        items = u.get("items", [])
        if len(items) != 2:
            continue
        nets = [re.search(r"\[([^\]]+)\]", it.get("description", "")) for it in items]
        if not all(nets) or nets[0].group(1) != nets[1].group(1) or nets[0].group(1) in plane_nets:
            continue
        if nets[0].group(1) in zone_nets:
            print(f"  {nets[0].group(1)}: has zones; not routed here (check the zone fill)")
            continue
        pending.append((nets[0].group(1), items))
    if args.nets:
        want = {n.lstrip("/") for n in args.nets.split(",")}
        pending = [p for p in pending if p[0].lstrip("/") in want]
    pending.sort(key=lambda p: math.dist((p[1][0]["pos"]["x"], p[1][0]["pos"]["y"]),
                                         (p[1][1]["pos"]["x"], p[1][1]["pos"]["y"])))
    if old and args.clear and not args.rip_nets:
        for item in old + old_groups:
            b.Remove(item)
        tracks = [t for t in tracks if t not in old]
        pcbnew.SaveBoard(str(pcb_path), b)
        print(f"  removed {len(old)} items from an earlier run")
    if args.rip_nets:
        rip = set(args.rip_nets.split(","))
        gone = [t for t in tracks if t.GetNetname() in rip and not t.IsLocked()]
        for item in gone:
            b.Remove(item)
        pcbnew.SaveBoard(str(pcb_path), b)
        print(f"RIPPED {len(gone)}")
        return 0
    first = set(args.first.split(",")) if args.first else set()
    pending.sort(key=lambda p: p[0] not in first)
    if not pending:
        print("finish_route: nothing to route (plane and zone nets excluded)")
        print("RESULT 0 0")
        return 0

    group = None
    if not args.dry_run:
        group = pcbnew.PCB_GROUP(b)
        group.SetName(args.tag)
        b.Add(group)
    added = []

    for name, items in pending:
        net = b.FindNet(name)
        c = cls(name)
        w = FromMM(c.get("track_width", 0.15))
        vdia, vdrill = FromMM(c.get("via_diameter", 0.6)), FromMM(c.get("via_drill", 0.3))
        mycl = clr(name)
        others = [i for i in pads + tracks + added if i.GetNetCode() != net.GetNetCode()]

        def blocked_for(grow_track, grow_via):
            track_mask = {l: np.zeros((ny, nx), bool) for l in layers}
            via_mask = np.zeros((ny, nx), bool)
            for item in others:
                need = max(mycl, clr(item.GetNetname()))
                for l in copper:
                    if not item.IsOnLayer(l):
                        continue
                    if l in track_mask:
                        rasterize(item_poly(item, l, need + grow_track), track_mask[l])
                    rasterize(item_poly(item, l, need + grow_via), via_mask)
            for z in zones:
                if z.GetIsRuleArea():
                    for l in copper:
                        if z.IsOnLayer(l):
                            poly = pcbnew.SHAPE_POLY_SET(z.Outline())
                            poly.Inflate(int(grow_via), pcbnew.CORNER_STRATEGY_ROUND_ALL_CORNERS, FromMM(0.01))
                            if z.GetDoNotAllowVias():
                                rasterize(poly, via_mask)
                            if z.GetDoNotAllowTracks() and l in track_mask:
                                rasterize(poly, track_mask[l])
                    continue
                if z.GetNetCode() == net.GetNetCode():
                    continue
                for l in copper:
                    if not z.IsOnLayer(l):
                        continue
                    fill = pcbnew.SHAPE_POLY_SET(z.GetFilledPolysList(l))
                    if z.GetNetname() in plane_nets and l not in (pcbnew.F_Cu, pcbnew.B_Cu):
                        continue   # an inner plane flows around a new via
                    fill.Inflate(int(max(mycl, clr(z.GetNetname())) + grow_track),
                                 pcbnew.CORNER_STRATEGY_ROUND_ALL_CORNERS, FromMM(0.01))
                    if l in track_mask:
                        rasterize(fill, track_mask[l])
                    rasterize(fill, via_mask)
            # board edge
            inner_t = pcbnew.SHAPE_POLY_SET(outline)
            inner_t.Inflate(-int(ds.m_CopperEdgeClearance + grow_track), pcbnew.CORNER_STRATEGY_CHAMFER_ALL_CORNERS,
                            FromMM(0.01))
            ok = np.zeros((ny, nx), bool)
            rasterize(inner_t, ok)
            for l in track_mask:
                track_mask[l] |= ~ok
            inner_v = pcbnew.SHAPE_POLY_SET(outline)
            inner_v.Inflate(-int(ds.m_CopperEdgeClearance + grow_via), pcbnew.CORNER_STRATEGY_CHAMFER_ALL_CORNERS,
                            FromMM(0.01))
            okv = np.zeros((ny, nx), bool)
            rasterize(inner_v, okv)
            via_mask |= ~okv
            return track_mask, via_mask

        track_mask, via_mask = blocked_for(w / 2, vdia / 2)
        # holes: a via needs hole-to-hole room from every other hole (any net)
        for item in pads + tracks + added:
            hole = (item.GetDrillValue() if item.Type() == pcbnew.PCB_VIA_T else
                    item.GetDrillSizeX() if item.Type() == pcbnew.PCB_PAD_T and item.HasHole() else 0)
            if hole:
                p = item.GetPosition()
                r = hole / 2 + vdrill / 2 + ds.m_HoleToHoleMin
                ix, iy = int((p.x - x0) / g), int((p.y - y0) / g)
                k = int(r / g) + 1
                Y, X = np.ogrid[max(0, iy - k):min(ny, iy + k + 1), max(0, ix - k):min(nx, ix + k + 1)]
                via_mask[max(0, iy - k):min(ny, iy + k + 1), max(0, ix - k):min(nx, ix + k + 1)] |= \
                    ((X - ix) ** 2 + (Y - iy) ** 2) * g * g < r * r

        def cells_of(it):
            """Start/goal cells: inside the item's copper, on each routing layer it is on."""
            p = it["pos"]
            at = pcbnew.VECTOR2I(FromMM(p["x"]), FromMM(p["y"]))
            cand = [i for i in pads + tracks + added if i.GetNetCode() == net.GetNetCode() and i.HitTest(at, 0)]
            out = set()
            for item in cand[:1]:
                for l in layers:
                    if not item.IsOnLayer(l):
                        continue
                    m = np.zeros((ny, nx), bool)
                    rasterize(item_poly(item, l, -FromMM(0.01)), m)
                    for iy, ix in zip(*np.nonzero(m)):
                        out.add((l, ix, iy))
            return out

        start, goal = cells_of(items[0]), cells_of(items[1])
        for l, ix, iy in start | goal:      # the item's own copper is never an obstacle to itself
            track_mask[l][iy, ix] = False
        if not start or not goal:
            print(f"  {name}: could not find the items on a routing layer")
            continue
        gx = np.mean([c[1] for c in goal])
        gy = np.mean([c[2] for c in goal])

        def h(ix, iy):
            dx, dy = abs(ix - gx), abs(iy - gy)
            return max(dx, dy) + (math.sqrt(2) - 1) * min(dx, dy)

        frontier = [(h(ix, iy), 0.0, (l, ix, iy)) for l, ix, iy in start]
        heapq.heapify(frontier)
        came = {s: None for s in start}
        cost = {s: 0.0 for s in start}
        found = None
        steps = [(1, 0, 1), (-1, 0, 1), (0, 1, 1), (0, -1, 1),
                 (1, 1, math.sqrt(2)), (1, -1, math.sqrt(2)), (-1, 1, math.sqrt(2)), (-1, -1, math.sqrt(2))]
        while frontier:
            _, c0, cur = heapq.heappop(frontier)
            if cur in goal:
                found = cur
                break
            if c0 > cost.get(cur, 1e18):
                continue
            l, ix, iy = cur
            nbrs = []
            for dx, dy, sc in steps:
                jx, jy = ix + dx, iy + dy
                if 0 <= jx < nx and 0 <= jy < ny and not track_mask[l][jy, jx]:
                    nbrs.append(((l, jx, jy), sc))
            if not via_mask[iy, ix]:
                for l2 in layers:
                    if l2 != l and not track_mask[l2][iy, ix]:
                        nbrs.append(((l2, ix, iy), args.via_cost))
            for nxt, sc in nbrs:
                nc = c0 + sc
                if nc < cost.get(nxt, 1e18):
                    cost[nxt] = nc
                    came[nxt] = cur
                    heapq.heappush(frontier, (nc + h(nxt[1], nxt[2]), nc, nxt))
        if not found:
            print(f"  {name}: no route on {args.layers} at {args.grid} mm")
            ends = [pcbnew.VECTOR2I(FromMM(it["pos"]["x"]), FromMM(it["pos"]["y"])) for it in items]
            crowd = set()
            for item in tracks + added:
                if item.IsLocked() or item.GetNetCode() == net.GetNetCode() or item.GetNetname() in plane_nets \
                        or item.GetNetname() in zone_nets:
                    continue
                for e in ends:
                    if item.GetEffectiveShape(item.GetLayer() if item.Type() != pcbnew.PCB_VIA_T
                                              else pcbnew.F_Cu).Collide(pcbnew.SHAPE_CIRCLE(e, FromMM(0.9)), 0):
                        crowd.add(item.GetNetname())
            print(f"FAILED {name} CROWD {','.join(sorted(crowd))}")
            continue
        path = []
        n = found
        while n is not None:
            path.append(n)
            n = came[n]
        path.reverse()
        # compress into segments; a layer change is a via
        segs, vias = [], []
        run = [path[0]]
        for a, c in zip(path, path[1:]):
            if c[0] != a[0]:
                segs.append(run)
                vias.append(a)
                run = [c]
                continue
            if len(run) >= 2:
                d0 = (run[-1][1] - run[-2][1], run[-1][2] - run[-2][2])
                if (c[1] - a[1], c[2] - a[2]) != d0:
                    segs.append(run)
                    run = [a]
            run.append(c)
        segs.append(run)
        length = sum(math.dist((a[1], a[2]), (c[1], c[2])) for a, c in zip(path, path[1:]) if a[0] == c[0]) * args.grid
        print(f"  {name}: {length:.1f} mm, {len(vias)} via(s)")
        routed.append(name)
        if args.dry_run:
            continue

        def pt(cell):
            return pcbnew.VECTOR2I(int(x0 + cell[1] * g), int(y0 + cell[2] * g))
        for run in segs:
            if len(run) < 2:
                continue
            t = pcbnew.PCB_TRACK(b)
            t.SetStart(pt(run[0]))
            t.SetEnd(pt(run[-1]))
            t.SetWidth(int(w))
            t.SetLayer(run[0][0])
            t.SetNet(net)
            t.SetLocked(args.lock)
            b.Add(t)
            group.AddItem(t)
            added.append(t)
        for cell in vias:
            v = pcbnew.PCB_VIA(b)
            v.SetPosition(pt(cell))
            v.SetWidth(int(vdia))
            v.SetDrill(int(vdrill))
            v.SetNet(net)
            v.SetLocked(args.lock)
            b.Add(v)
            group.AddItem(v)
            added.append(v)

    if not args.dry_run:
        pcbnew.SaveBoard(str(pcb_path), b)
    print(f"finish_route: {len(routed)} of {len(pending)} connections routed; run DRC")
    print(f"RESULT {len(routed)} {len(pending)}")
    return 0 if len(routed) == len(pending) else 1


def drive(args) -> int:
    """Rounds of: route what is unconnected; for each failure, rip up the nets
    crowding its ends and try again with it first. Each step is its own process
    (pcbnew loads one board per process and its proxies break after Remove())."""
    me = [sys.executable, __file__, str(args.project), "--child", "--layers", args.layers,
          "--grid", str(args.grid), "--via-cost", str(args.via_cost), "--tag", args.tag] + \
        (["--nets", args.nets] if args.nets else []) + (["--lock"] if args.lock else [])
    if args.nets:
        args.rip = 0                                # only the named nets: nothing else is ripped up
    first, ripped = [], set()
    clear = ["--clear"] if args.clear else []
    for rnd in range(args.rip + 1):
        r = subprocess.run(me + clear + (["--first", ",".join(first)] if first else []),
                           capture_output=True, text=True)
        clear = []
        out = "\n".join(l for l in r.stdout.splitlines() if "leak" not in l and "Debug" not in l)
        print("\n".join(l for l in out.splitlines() if not l.startswith(("FAILED", "RESULT"))))
        failed = re.findall(r"FAILED (\S+) CROWD (\S*)", out)
        if not failed:
            return 0
        crowd = {n for _, c in failed for n in c.split(",") if n} - ripped
        if rnd == args.rip or not crowd:
            print(f"finish_route: still unrouted after {rnd} rip-up round(s): {', '.join(f for f, _ in failed)}")
            return 1
        first = [f for f, _ in failed] + first
        ripped |= crowd
        print(f"  rip-up round {rnd + 1}: removing the routing of {', '.join(sorted(crowd))}")
        subprocess.run(me + ["--rip-nets", ",".join(sorted(crowd))], capture_output=True, text=True)
    return 1


if __name__ == "__main__":
    sys.exit(main())
