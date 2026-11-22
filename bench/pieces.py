#!/usr/bin/env python3
"""Where KiCad sees a net in pieces. KiCad's DRC names a pour's own reference point for an
unconnected "Zone - Zone" or "Pad - Zone" pair, which says nothing about where the break is.
This joins a refilled board's pads, vias, tracks and pour islands as KiCad's connectivity does
(a track or via where an end or its middle lies in an island, a pad where its copper touches
one) and prints each net's pieces smallest first, with the pads and the island in each: the
place to point `look.py` at.

    pieces.py BOARD.filled.kicad_pcb [NET]

For a board of a `tier.py` run that is LAB/tier/RUN/TAG/ID/routed.filled.kicad_pcb. An island
with nothing of its net in it is left out: KiCad drops it. Needs KiCad's `pcbnew` module.
"""
import sys, pcbnew
b = pcbnew.LoadBoard(sys.argv[1])
want = sys.argv[2] if len(sys.argv) > 2 else None
conn = b.GetConnectivity()
V = pcbnew.VECTOR2I
def uid(i): return i.m_Uuid.AsString()
nets = {}
for i in list(b.GetPads()) + list(b.GetTracks()):
    if i.GetNetCode() > 0: nets.setdefault(i.GetNetname(), []).append(i)
for net, items in sorted(nets.items()):
    if want and net != want: continue
    parent = {}
    def find(x):
        while parent.setdefault(x, x) != x:
            parent[x] = parent[parent[x]]; x = parent[x]
        return x
    def join(a, c): parent[find(a)] = find(c)
    for i in items:
        find(uid(i))
        for o in list(conn.GetConnectedPads(i)) + list(conn.GetConnectedTracks(i)):
            if o.GetNetCode() == i.GetNetCode(): join(uid(i), uid(o))
    isl = {}
    for zi, z in enumerate(b.Zones()):
        if z.GetNetname() != net: continue
        for layer in z.GetLayerSet().Seq():
            polys = z.GetFilledPolysList(layer)
            for k in range(polys.OutlineCount()):
                one = pcbnew.SHAPE_POLY_SET(); one.AddOutline(polys.Outline(k))
                for h in range(polys.HoleCount(k)): one.AddHole(polys.Hole(k, h))
                key = 'island z%d %s #%d' % (zi, b.GetLayerName(layer), k)
                bb = polys.Outline(k).BBox(); isl[key] = (abs(polys.Outline(k).Area()) / 1e12, bb.Centre().x / 1e6, bb.Centre().y / 1e6)
                find(key)
                for i in items:
                    if not i.IsOnLayer(layer) or not i.GetBoundingBox().Intersects(pcbnew.BOX2I(bb.GetOrigin(), bb.GetSize())): continue
                    c = i.GetClass()
                    if c == 'PAD':
                        hit = one.Contains(i.GetPosition(), 0, 0)
                        if not hit:
                            p = pcbnew.SHAPE_POLY_SET(); i.TransformShapeToPolygon(p, layer, 0, 5000, pcbnew.ERROR_INSIDE)
                            p.BooleanIntersection(one); hit = p.OutlineCount() > 0
                    else:
                        acc = (i.GetWidth() + 1) // 2 if c != 'PCB_VIA' else (i.GetWidth(layer) + 1) // 2
                        pts = [i.GetPosition()] if c == 'PCB_VIA' else [i.GetStart(), i.GetEnd()]
                        hit = any(one.Contains(p, 0, acc) for p in pts)
                    if hit: join(uid(i), key)
    groups = {}
    for i in items: groups.setdefault(find(uid(i)), []).append(i)
    if len(groups) < 2: continue
    def name(i):
        p = i.GetPosition(); c = i.GetClass()
        if c == 'PAD': return 'pad %s.%s %.2f,%.2f' % (i.GetParentFootprint().GetReference(), i.GetNumber(), p.x / 1e6, p.y / 1e6)
        return '%s %.2f,%.2f' % ({'PCB_VIA': 'via', 'PCB_TRACK': 'track', 'PCB_ARC': 'arc'}.get(c, c), p.x / 1e6, p.y / 1e6)
    rows = sorted(groups.items(), key=lambda g: len(g[1]))
    print(net, len(rows), 'pieces')
    for root, g in rows[:-1]:
        mine = [k for k in isl if find(k) == root]
        print('   ', '; '.join(sorted(name(i) for i in g)[:5]), '|', '; '.join('%s %.1f mm2 at %.2f,%.2f' % ((k,) + isl[k]) for k in mine[:3]))
