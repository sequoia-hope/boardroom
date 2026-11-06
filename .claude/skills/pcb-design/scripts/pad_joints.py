#!/usr/bin/env python3
"""pad_joints.py -- every SMD pad joined to its copper by at least a track's width.

    pad_joints.py hw/main/a            check the board, exit 1 on a thin joint
    pad_joints.py hw/main/a --json f   also write the findings as JSON

A track that ends just outside a pad touches it only where its round end-cap
crosses the pad edge, and a track that ends just inside a pad corner (or grazes
it at an angle) can cross the pad outline over a chord narrower than the fab's
minimum track width. KiCad's DRC misses both when connectivity is satisfied
(review-layout 2026-11-01 A2: J1.B7, U1.4, U1.8 and U5.6 at 0.058-0.087 mm;
round-pendant: 462 corner-straddling joints under 0.09 mm).

Method:
  * Each SMD pad is tested on its actual outer copper layer (`B.Cu` when
    `pad.IsOnLayer(pcbnew.B_Cu)`, else `F.Cu`; `pad.GetLayer()` returns `F.Cu`
    even on flipped footprints).
  * Each same-net track segment `AB` of width `w` on that layer is modelled as
    its exact convex capsule `{x : dist(x, AB) <= w/2}` (the width-`w` slab on
    `AB` unioned with the two radius-`w/2` end-cap disks) and intersected
    analytically with every edge of `pad.GetEffectivePolygon(layer)`.
  * Contiguous covered segments around each polygon loop are chained into
    connected boundary arcs where copper crosses the pad outline. If the
    capsule covers the whole loop or wraps more than 180 deg around the pad,
    the joint has the track's full width `w`; otherwise its width is
    `min(w, chord)`, where `chord` is the straight-line distance between the
    outline points entering and leaving the capsule (sub-nanometre exact, so a
    0.09 mm neck is cleanly separated from a 0.10 mm minimum). Segments lying
    strictly inside the pad cross no outline edge and are ignored so an inner
    stub cannot mask a thin exit neck.

A pad passes when any track crossing reaches the fab minimum track width
(checks.toml [fab] min_track_mm), a same-net zone fill covers the pad's centre,
or a same-net via ring overlaps the pad by at least the minimum. Otherwise, if
tracks touch the pad only through narrower crossings, it fails. Run at gate 7
after routing, beside DRC.
"""
import argparse
import json
import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import pcbnew  # noqa: E402

EPS_MM = 1e-5  # 10 nm slack for integer-nanometre polygon vertex rounding


def board_path(arg: str) -> Path:
    p = Path(arg)
    if p.is_dir():
        pcbs = sorted(p.glob("*.kicad_pcb"))
        if len(pcbs) != 1:
            sys.exit(f"pad_joints: expected one .kicad_pcb in {p}, found {len(pcbs)}")
        return pcbs[0]
    return p.with_suffix(".kicad_pcb")


def min_track(pcb: Path) -> float:
    try:
        from kicad_files import load_checks, repo_root
        return float(load_checks(repo_root(pcb.parent))["fab"].get("min_track_mm", 0.09))
    except Exception:
        return 0.09


def _pt_seg_dist_sq(p: tuple[float, float], a: tuple[float, float], b: tuple[float, float]) -> float:
    vx, vy = b[0] - a[0], b[1] - a[1]
    wx, wy = p[0] - a[0], p[1] - a[1]
    vv = vx * vx + vy * vy
    t = 0.0 if vv == 0.0 else max(0.0, min(1.0, (wx * vx + wy * vy) / vv))
    dx, dy = wx - t * vx, wy - t * vy
    return dx * dx + dy * dy


def _edge_capsule_interval(
    p0: tuple[float, float], p1: tuple[float, float],
    a: tuple[float, float], b: tuple[float, float], r: float,
) -> tuple[float, float] | None:
    """Parameter interval [t0, t1] in [0, 1] where edge p0->p1 lies inside capsule(a, b, r)."""
    dx, dy = p1[0] - p0[0], p1[1] - p0[1]
    dd = dx * dx + dy * dy
    rr = r * r
    if dd == 0.0:
        return (0.0, 1.0) if _pt_seg_dist_sq(p0, a, b) <= rr else None

    ivs: list[tuple[float, float]] = []
    for cx, cy in (a, b):
        fx, fy = p0[0] - cx, p0[1] - cy
        hb = fx * dx + fy * dy
        disc = hb * hb - dd * (fx * fx + fy * fy - rr)
        if disc >= 0.0:
            sq = math.sqrt(disc)
            lo = max(0.0, (-hb - sq) / dd)
            hi = min(1.0, (-hb + sq) / dd)
            if lo <= hi:
                ivs.append((lo, hi))

    vx, vy = b[0] - a[0], b[1] - a[1]
    seg_len = math.hypot(vx, vy)
    if seg_len > 1e-12:
        ux, uy = vx / seg_len, vy / seg_len
        nx, ny = -uy, ux
        fx, fy = p0[0] - a[0], p0[1] - a[1]
        s0, ds = fx * ux + fy * uy, dx * ux + dy * uy
        h0, dh = fx * nx + fy * ny, dx * nx + dy * ny
        lo, hi = 0.0, 1.0
        for v0, dv, mn, mx in ((s0, ds, 0.0, seg_len), (h0, dh, -r, r)):
            if abs(dv) <= 1e-15:
                if v0 < mn or v0 > mx:
                    lo, hi = 1.0, 0.0
                    break
            else:
                t1, t2 = (mn - v0) / dv, (mx - v0) / dv
                if t1 > t2:
                    t1, t2 = t2, t1
                lo, hi = max(lo, t1), min(hi, t2)
                if lo > hi:
                    break
        if lo <= hi:
            ivs.append((lo, hi))

    if not ivs:
        return None
    return min(iv[0] for iv in ivs), max(iv[1] for iv in ivs)


def _net_turn(arc: list[tuple[float, float]]) -> float:
    dirs: list[tuple[float, float]] = []
    for i in range(len(arc) - 1):
        ux, uy = arc[i + 1][0] - arc[i][0], arc[i + 1][1] - arc[i][1]
        ln = math.hypot(ux, uy)
        if ln > 1e-12:
            dirs.append((ux / ln, uy / ln))
    turn = 0.0
    for i in range(len(dirs) - 1):
        dot = max(-1.0, min(1.0, dirs[i][0] * dirs[i + 1][0] + dirs[i][1] * dirs[i + 1][1]))
        cross = dirs[i][0] * dirs[i + 1][1] - dirs[i][1] * dirs[i + 1][0]
        turn += math.atan2(cross, dot)
    return abs(turn)


def track_pad_joint_mm(poly, a_nm, b_nm, w_nm: int) -> float | None:
    """Width (mm) of the widest crossing of capsule(a, b, w/2) across poly's outline, or None."""
    w = w_nm * 1e-6
    r = w * 0.5
    a = (a_nm.x * 1e-6, a_nm.y * 1e-6)
    b = (b_nm.x * 1e-6, b_nm.y * 1e-6)
    rr = r * r + 1e-18
    best = 0.0
    for oi in range(poly.OutlineCount()):
        outline = poly.Outline(oi)
        n = outline.PointCount()
        if n < 3:
            continue
        pts = [(outline.CPoint(i).x * 1e-6, outline.CPoint(i).y * 1e-6) for i in range(n)]
        v_in = [_pt_seg_dist_sq(pt, a, b) <= rr for pt in pts]
        ivs: list[tuple[float, float] | None] = []
        for i in range(n):
            iv = _edge_capsule_interval(pts[i], pts[(i + 1) % n], a, b, r)
            if iv is not None:
                t0 = 0.0 if v_in[i] else iv[0]
                t1 = 1.0 if v_in[(i + 1) % n] else iv[1]
                iv = (t0, t1)
            ivs.append(iv)
        if not any(iv is not None for iv in ivs):
            continue
        conn = [
            ivs[i] is not None and ivs[(i + 1) % n] is not None
            and (v_in[(i + 1) % n] or (ivs[i][1] >= 1.0 - 1e-9 and ivs[(i + 1) % n][0] <= 1e-9))
            for i in range(n)
        ]
        if all(conn):
            return w
        used = [False] * n
        for start in range(n):
            if ivs[start] is None or conn[(start - 1) % n] or used[start]:
                continue
            arc: list[tuple[float, float]] = []
            idx = start
            while True:
                used[idx] = True
                t0, t1 = ivs[idx]  # type: ignore[misc]
                p0, p1 = pts[idx], pts[(idx + 1) % n]
                q0 = (p0[0] + t0 * (p1[0] - p0[0]), p0[1] + t0 * (p1[1] - p0[1]))
                q1 = (p0[0] + t1 * (p1[0] - p0[0]), p0[1] + t1 * (p1[1] - p0[1]))
                if not arc:
                    arc.append(q0)
                arc.append(q1)
                if conn[idx]:
                    idx = (idx + 1) % n
                    if idx == start:
                        break
                else:
                    break
            if _net_turn(arc) > math.pi:
                return w
            chord = math.hypot(arc[-1][0] - arc[0][0], arc[-1][1] - arc[0][1])
            if chord > best:
                best = min(w, chord)
    return best if best > 1e-6 else None


def _track_segments(t):
    if t.Type() != pcbnew.PCB_ARC_T:
        return ((t.GetStart(), t.GetEnd()),)
    ctr = t.GetCenter()
    st = t.GetStart()
    ang = t.GetAngle().AsRadians()
    r = math.hypot(st.x - ctr.x, st.y - ctr.y)
    a0 = math.atan2(st.y - ctr.y, st.x - ctr.x)
    steps = max(8, int(math.ceil(abs(ang) / (math.pi / 16))))
    pts = [st]
    for k in range(1, steps):
        ak = a0 + ang * (k / steps)
        pts.append(pcbnew.VECTOR2I(round(ctr.x + r * math.cos(ak)), round(ctr.y + r * math.sin(ak))))
    pts.append(t.GetEnd())
    return tuple((pts[i], pts[i + 1]) for i in range(len(pts) - 1))


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("project")
    ap.add_argument("--json")
    args = ap.parse_args()
    pcb = board_path(args.project)
    floor = min_track(pcb)
    b = pcbnew.LoadBoard(str(pcb))
    mm = pcbnew.ToMM
    tracks = [t for t in b.GetTracks() if t.Type() != pcbnew.PCB_VIA_T]
    vias = [t for t in b.GetTracks() if t.Type() == pcbnew.PCB_VIA_T]
    zones = [z for z in b.Zones() if not z.GetIsRuleArea()]
    bad, checked = [], 0
    for fp in b.GetFootprints():
        for pad in fp.Pads():
            net = pad.GetNetname()
            if pad.GetAttribute() != pcbnew.PAD_ATTRIB_SMD or not net or net.startswith("unconnected"):
                continue
            layer = pcbnew.B_Cu if pad.IsOnLayer(pcbnew.B_Cu) else pcbnew.F_Cu
            poly = pad.GetEffectivePolygon(layer)
            bb = poly.BBox()
            bx0, by0 = bb.GetX(), bb.GetY()
            bx1, by1 = bx0 + bb.GetWidth(), by0 + bb.GetHeight()
            c = pad.GetPosition()
            sound, joints = False, []
            for t in tracks:
                if t.GetNetname() != net or t.GetLayer() != layer:
                    continue
                w = t.GetWidth()
                r_nm = w // 2 + 1000
                for a, e in _track_segments(t):
                    if (max(a.x, e.x) + r_nm < bx0 or min(a.x, e.x) - r_nm > bx1
                            or max(a.y, e.y) + r_nm < by0 or min(a.y, e.y) - r_nm > by1):
                        continue
                    j = track_pad_joint_mm(poly, a, e, w)
                    if j is not None:
                        joints.append(j)
                        if j >= floor - EPS_MM:
                            sound = True
                            break
                if sound:
                    break
            if not sound:
                sound = any(z.GetNetname() == net and z.IsOnLayer(layer)
                            and z.GetFilledPolysList(layer).Contains(c) for z in zones)
            if not sound:
                for v in vias:
                    if v.GetNetname() == net and v.IsOnLayer(layer):
                        d = math.sqrt(poly.SquaredDistance(v.GetPosition()))
                        if v.GetWidth(layer) / 2 - d >= pcbnew.FromMM(floor - EPS_MM):
                            sound = True
                            break
            checked += 1
            if not sound and joints and max(joints) < floor - EPS_MM:
                bad.append({"pad": f"{fp.GetReference()}.{pad.GetNumber()}", "net": net,
                            "neck_mm": round(max(joints), 3),
                            "at": [round(mm(c.x), 3), round(mm(c.y), 3)]})
    print(f"pad_joints: {pcb.name}: {checked} SMD pads, joints under {floor} mm:")
    for f in bad:
        print(f"  {f['pad']:8s} {f['net']:16s} neck {f['neck_mm']:.3f} mm at ({f['at'][0]}, {f['at'][1]})")
    print(f"\n  {'FAIL' if bad else 'PASS'}: {len(bad)} thin pad joints")
    if args.json:
        Path(args.json).write_text(json.dumps(bad, indent=1))
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
