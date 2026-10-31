#!/usr/bin/env python3
"""board.py -- build the easy2350 board from the schematic, in stages.

    python3 scripts/board.py sync        Update PCB from Schematic (keeps placement)
    python3 scripts/board.py outline     outline, 4 layers, connectors (phase 4)
    python3 scripts/board.py place       every part at its planned position (phase 6)
    python3 scripts/board.py route       planes, then ultraroute via autoroute.py (phase 7)
    python3 scripts/board.py finish      pours, stitching vias, fill (phase 7)

Each stage re-runs the earlier ones it depends on; every number comes from
design/brief.md. Run each stage in its own process (pcbnew can't load two
projects in one).
"""
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT.parents[1] / ".claude/skills/pcb-design/scripts"))
import pcbnew  # noqa: E402
from kicad_write import update_pcb  # noqa: E402

SKILL = ROOT.parents[1] / ".claude/skills/pcb-design/scripts"
HW = ROOT / "hw/easy2350/a"
SCH, PCB = HW / "easy2350.kicad_sch", HW / "easy2350.kicad_pcb"

# Board frame: top-left corner of the outline. USB-C at the top edge, SWD at
# the bottom; J2 down the left edge, J3 up the right (parts/user-io.md).
X0, Y0, W, H, R = 100.0, 100.0, 21.0, 51.0, 1.0
ROW = 1.61          # header pin centre to the long edge (17.78 mm between rows)
EDGE_STRIP = 0.7    # no tracks or vias this close to the long edges (header pads start at 0.76)
PIN1 = 1.37         # first header pin centre to the short edge: 20 pins centred on 51 mm

mm = pcbnew.VECTOR2I_MM


def at(x, y):
    """Board coordinates (mm from the top-left corner) to KiCad page coordinates."""
    return mm(X0 + x, Y0 + y)


def outline(b):
    for d in list(b.GetDrawings()):
        if d.GetLayer() == pcbnew.Edge_Cuts:
            b.Remove(d)

    def seg(a, c):
        s = pcbnew.PCB_SHAPE(b, pcbnew.SHAPE_T_SEGMENT)
        s.SetStart(at(*a))
        s.SetEnd(at(*c))
        s.SetLayer(pcbnew.Edge_Cuts)
        s.SetWidth(pcbnew.FromMM(0.05))
        b.Add(s)

    def arc(centre, start, mid, end):
        s = pcbnew.PCB_SHAPE(b, pcbnew.SHAPE_T_ARC)
        s.SetArcGeometry(at(*start), at(*mid), at(*end))
        s.SetLayer(pcbnew.Edge_Cuts)
        s.SetWidth(pcbnew.FromMM(0.05))
        b.Add(s)

    k = R * (1 - 2 ** -0.5)
    seg((R, 0), (W - R, 0))
    seg((W, R), (W, H - R))
    seg((W - R, H), (R, H))
    seg((0, H - R), (0, R))
    arc((W - R, R), (W - R, 0), (W - k, k), (W, R))
    arc((W - R, H - R), (W, H - R), (W - k, H - k), (W - R, H))
    arc((R, H - R), (R, H), (k, H - k), (0, H - R))
    arc((R, R), (0, R), (k, k), (R, 0))


# The parts that start locked: what the mechanics fix (connectors, test point, buttons), the
# RP2350 with its core regulator, and the LEDs. Circuit Composer's floorplan spec names the same
# set, and a footprint's lock in this file is the lock Composer and ccomp read. Everything else
# may move. No copper is locked (LESSONS.md, [routing]).
LOCKED = {"J1", "J2", "J3", "J4", "U1", "C14", "C15", "L1", "TP3", "SW1", "SW2", "D1", "D3", "D4", "D5", "D6"}


def place_fp(fps, ref, x, y, rot=0):
    fp = fps[ref]
    fp.SetPosition(at(x, y))
    fp.SetOrientationDegrees(rot)
    fp.SetLocked(ref in LOCKED)


def connectors(b):
    fps = {fp.GetReference(): fp for fp in b.GetFootprints()}
    # The GPIO headers mount from the bottom (plastic under the board, pins into
    # a breadboard), so their footprints are on the back.
    # (A flipped footprint turns the other way: rotations are 180 and 0.)
    for ref, x, y, rot in (("J2", ROW, PIN1, 180), ("J3", W - ROW, H - PIN1, 0)):
        fp = fps[ref]
        if not fp.IsFlipped():
            fp.Flip(fp.GetPosition(), pcbnew.FLIP_DIRECTION_LEFT_RIGHT)
        place_fp(fps, ref, x, y, rot)
    # USB-C: KiCad's HRO footprint has its mouth toward +y; the board edge sits
    # at y = +3.19 in footprint coordinates (usb-front-end.md). Turned 180 so the
    # mouth faces out of the top edge.
    place_fp(fps, "J1", W / 2, 3.19, 180)
    # SWD: three pins across the bottom end, between the header rows.
    place_fp(fps, "J4", W / 2 - 2.54, H - PIN1, 90)


def _with_board(fn):
    b = pcbnew.LoadBoard(str(PCB))
    fn(b)
    pcbnew.SaveBoard(str(PCB), b)


def stage_refresh():
    """Reload project-library footprints whose library copy changed (KiCad's
    'Update footprints from library'), keeping position, side, fields and nets."""
    b = pcbnew.LoadBoard(str(PCB))
    lib = HW / "lib/easy2350.pretty"
    old = [fp for fp in list(b.GetFootprints()) if fp.GetFPID().GetLibNickname().wx_str() == "easy2350"]
    saved = []
    for fp in old:
        name = fp.GetFPID().GetLibItemName().wx_str()
        new = pcbnew.FootprintLoad(str(lib), name)
        saved.append((fp, new, fp.GetPosition(), fp.GetOrientation(), fp.IsFlipped(), fp.IsLocked(),
                      {p.GetNumber(): p.GetNet() for p in fp.Pads()},
                      {f.GetName(): (f.GetText(), f.IsVisible(), f.GetPosition(), f.GetLayer())
                       for f in fp.GetFields()}, fp.GetPath(), fp.GetSheetname(), fp.GetSheetfile()))
    for item in saved:
        b.Remove(item[0])
    for fp, new, pos, rot, flipped, locked, nets, fields, path, sheet, sfile in saved:
        new.SetFPID(fp.GetFPID())
        b.Add(new)
        new.SetPosition(pos)
        if flipped:
            new.Flip(pos, pcbnew.FLIP_DIRECTION_LEFT_RIGHT)
        new.SetOrientation(rot)
        for pad in new.Pads():
            if pad.GetNumber() in nets:
                pad.SetNet(nets[pad.GetNumber()])
        for name, (text, vis, at_, layer) in fields.items():
            if not new.HasFieldByName(name):
                new.SetField(name, text)
            f = new.GetFieldByName(name)
            f.SetText(text)
            f.SetVisible(vis)
            f.SetPosition(at_)
        new.SetPath(path)
        new.SetSheetname(sheet)
        new.SetSheetfile(sfile)
        new.SetLocked(locked)
    pcbnew.SaveBoard(str(PCB), b)
    print(f"refresh: {len(saved)} project footprints reloaded")


def stage_sync():
    update_pcb(SCH, PCB, new_parts_at=(X0 + W + 10, Y0 + H / 2))


def stage_outline():
    b = pcbnew.LoadBoard(str(PCB))
    b.SetCopperLayerCount(4)
    connectors(b)
    outline(b)          # after: removing old outline items upsets pcbnew's SWIG proxies
    pcbnew.SaveBoard(str(PCB), b)


# ---------------------------------------------------------------- placement (phase 6)

U1_AT = (W / 2, 26.5)
VENDOR = Path.home() / "pcb/RP-006440-DD-2-RP2350A Minimal Board Kicad archive/RP2350_60QFN_minimal.kicad_pcb"
# Vendor part -> ours, by function (parts/rp2350-core.md). The regulator,
# USB resistors and decouplers keep Raspberry Pi's positions relative to U1.
TRANSPLANT_REFS = {
    "L1": "L1", "C6": "C14", "C7": "C15", "C9": "C13", "R5": "R3",     # regulator
    "R7": "R5", "R8": "R4", "C12": "C11",                                 # USB 27 R, pins 53/54
    "C13": "C4", "C15": "C5", "C18": "C6", "C16": "C7", "C14": "C8", "C17": "C9",   # IOVDD
    "C10": "C16", "C8": "C17", "C11": "C18",                              # DVDD
}
TRANSPLANT_NETS = {"+1V1": "+1V1", "+3V3": "+3V3", "GND": "GND", "/VREG_LX": "/VREG_LX",
                   "/VREG_AVDD": "/VREG_AVDD",
                   # The 4 mm from USB_DP/DM (pins 52/51) to the 27 R: a pair Freerouting
                   # can't see, so it crossed them (phase 7). Raspberry Pi's routing is copied.
                   "Net-(U1-USB_DP)": "/USB_DP_MCU", "Net-(U1-USB_DM)": "/USB_DM_MCU"}
# Anchor frame. The vendor board is wider: its outermost GND vias (x = +-7.7,
# +8.3, +7.6) would land on our header pads, so the box stops at x -7.3 / +7.5
# and the router gives those decouplers their ground vias. It stops at y 7.5,
# short of the vendor crystal, and drops one 3V3 track that leaves the block
# toward the vendor's flash.
TRANSPLANT_BOX = "-7.3,-10,7.5,7.5"
TRANSPLANT_EXCLUDE = "-2.6,6.2,-2.4,6.4"

# Everything else: ref -> (x, y, rotation), board coordinates (brief.md §7).
PLACE = {
    # USB front end and 3V3, under the USB-C
    "U2": (11.2, 10.6, 0), "C1": (11.4, 12.85, 0), "R2": (9.6, 13.6, 90), "R1": (12.9, 13.9, 90),   # CC2 pad is left, CC1 right
    # LDO turned so IN (pin 8) faces up toward VBUS and C2, OUT (pin 1) down toward C3
    "U3": (15.6, 12.6, 90), "C2": (15.1, 9.5, 0), "C3": (15.2, 16.0, 0),
    # Flash at U1's QSPI corner
    "U5": (5.9, 13.2, 90), "C22": (4.2, 7.8, 0), "R7": (6.4, 20.3, 0),
    # Around U1 (not in the vendor layout)
    # R8 and TP3 stay off the diagonal GPIO16 takes from pin 27 in the vendor layout
    # (phase 7); TP3 is a bare pad, so it goes on the bottom (no part to assemble).
    "R8": (14.6, 32.0, 90), "TP3": (U1_AT[0] + 0.2, 33.6, 0),     # under the south 1V1 spoke via
    # Crystal, 3.4 mm closer than the vendor's to free the bottom row
    "Y1": (U1_AT[0], 36.5, 0), "R6": (10.1, 33.4, 90), "C20": (7.4, 37.35, 180), "C21": (13.6, 35.65, 0),
    # Status LED on GPIO25, beside J3 pin 13 (GPIO25's header pin) at the top-right,
    # out of the channel the GPIO16-22 routes need down the right side (phase 7)
    "R9": (16.0, 20.4, 90), "D1": (17.2, 20.4, 90),
    # Left of the crystal: VLED supply (rectifier, bulk, clamp, bleed)
    "D2": (4.0, 32.15, 90), "C23": (6.4, 32.9, 90), "D7": (4.0, 37.05, 90), "R11": (6.0, 38.4, 90),
    # Pixels in a row, turned 180 so each DOUT (bottom-right) faces the next DIN
    # (top-left) across the gap: a 2 mm hop on L1, nothing under a body
    # (rgb-leds.md). Each 100 nF below, VLED toward its pixel's VDD (bottom-left)
    "D3": (4.6, 40.8, 180), "D4": (8.3, 40.8, 180), "D5": (12.7, 40.8, 180), "D6": (16.4, 40.8, 180),
    "C25": (4.6, 42.6, 0), "C26": (8.3, 42.6, 0), "C27": (12.7, 42.6, 0), "C28": (16.4, 42.6, 0),
    # Buttons either side of the level shifter; its passives in the corners by J4
    "SW1": (5.6, 45.6, 0), "SW2": (15.4, 45.6, 0), "U4": (10.5, 45.65, 90),
    "C24": (10.5, 42.6, 0), "R12": (16.6, 50.2, 0),   # C24 between the pixel caps, at U4 pin 5
    "R10": (6.45, 42.6, 180),                          # series R between pixel 1 and 2's caps: no courtyard-clean
                                                       # spot nearer pixel 1's DIN (left) or U4 pin 4 (searched)
}


def band_stubs(b):
    """IOVDD pins 1 and 45 sit at the corners of U1's pad ring, beyond the
    vendor's 3V3 band; the vendor board fed them from a large 3V3 pour we did
    not copy. A 0.15 mm stub from each pad into the band joins them (0.125 mm
    from the next corner pin), and pin 1 runs on along the pin row to its
    decoupler C4, the way the vendor joins pin 11 to its cap (phase 7: both
    were 3V3 islands)."""
    # Read everything first: after a Remove(), pcbnew's proxies go bad.
    fps = {fp.GetReference(): fp for fp in b.GetFootprints()}
    c4 = [p for p in fps["C4"].Pads() if p.GetNetname() == "+3V3"][0]
    tp3_pad = fps["TP3"].Pads()[0]
    c4_at, v33 = c4.GetPosition(), c4.GetNet()
    old = [g for g in list(b.Groups()) if g.GetName() == "band stubs"]
    old_items = [i for g in old for i in g.GetItems()]
    for item in old_items + old:
        b.Remove(item)
    group = pcbnew.PCB_GROUP(b)
    group.SetName("band stubs")
    b.Add(group)
    ux, uy = U1_AT
    paths = [[at(ux - 3.44, uy - 2.8), at(ux - 2.6, uy - 2.8)],                 # pin 1 into the band
             [at(ux + 3.44, uy - 2.8), at(ux + 2.6, uy - 2.8)],                 # pin 45 into the band
             [at(ux - 3.44, uy - 2.8), at(ux - 5.4, uy - 2.8), c4_at],          # pin 1 to C4
             # VREG_VIN (49) and CIN: the vendor fed them from its outer 3V3 pour;
             # here the band does, 0.225 mm clear of pins 48 and 50
             [at(ux + 1.6, uy - 3.44), at(ux + 1.6, uy - 2.6)]]
    for path in paths:
        for a, c in zip(path, path[1:]):
            t = pcbnew.PCB_TRACK(b)
            t.SetStart(a)
            t.SetEnd(c)
            t.SetWidth(pcbnew.FromMM(0.15))
            t.SetLayer(pcbnew.F_Cu)
            t.SetNet(v33)
            t.SetLocked(True)
            b.Add(t)
            group.AddItem(t)
    # TP3 (bottom) hangs off the vendor's south 1V1 spoke via, on B.Cu.
    tp3 = tp3_pad
    t = pcbnew.PCB_TRACK(b)
    t.SetStart(at(ux + 0.2, uy + 5.8))
    t.SetEnd(tp3.GetPosition())
    t.SetWidth(pcbnew.FromMM(0.3))
    t.SetLayer(pcbnew.B_Cu)
    t.SetNet(tp3.GetNet())
    t.SetLocked(True)
    b.Add(t)
    group.AddItem(t)


def critical_routes(b):
    """Short critical connections drawn before the autorouter (routing.md step 3),
    locked, in a group so a re-run replaces them. Each answered a review finding
    (design/gates/review-layout.md, design/sim/si-review.md):
      - XIN on L1 only, round Y1's GND pad 4 into pad 1, with C20 at pad 1;
        XOUT straight to R6, XOUT_R to Y1 pad 3 and C21 (Freerouting took XIN
        through two vias and 6 mm of L4)
      - USB-C: D- (A7/B7) tied through two vias under the receptacle and an L4
        link; D+ (A6/B6) tied by a U on L1 and taken under U2 on L4 to pin 3
        (Freerouting looped D+ 7 mm past the flash clock)
      - USB D+/D- from U2 to the 27 ohm resistors, down L4 side by side
      - the pixel chain, DOUT to the next DIN across each gap on L1 (Freerouting
        ran PIX12 25 mm under U1, and PIX23 45 mm past the crystal)."""
    fps = {fp.GetReference(): fp for fp in b.GetFootprints()}

    def pad(ref, num):
        p = [q for q in fps[ref].Pads() if q.GetNumber() == num][0]
        return p.GetPosition(), p.GetNet()
    nets = {n: pad(r, k)[1] for n, (r, k) in {"XIN": ("Y1", "1"), "XOUT": ("R6", "2"), "XOUT_R": ("Y1", "3"),
                                              "DP": ("J1", "A6"), "DM": ("J1", "A7"),
                                              "USB_DP": ("U2", "4"), "USB_DM": ("U2", "6"),
                                              "PIX12": ("D3", "1"), "PIX23": ("D4", "1"),
                                              "PIX34": ("D5", "1")}.items()}
    old = [g for g in list(b.Groups()) if g.GetName() == "critical"]
    old_items = [i for g in old for i in g.GetItems()]
    for item in old_items + old:
        b.Remove(item)
    group = pcbnew.PCB_GROUP(b)
    group.SetName("critical")
    b.Add(group)

    def track(net, layer, *pts, w=0.15):
        for a, c in zip(pts, pts[1:]):
            tr = pcbnew.PCB_TRACK(b)
            tr.SetStart(at(*a))
            tr.SetEnd(at(*c))
            tr.SetWidth(pcbnew.FromMM(w))
            tr.SetLayer(layer)
            tr.SetNet(nets[net])
            tr.SetLocked(True)
            b.Add(tr)
            group.AddItem(tr)

    def via(net, x, y, d=0.5, drill=0.2):
        v = pcbnew.PCB_VIA(b)
        v.SetPosition(at(x, y))
        v.SetWidth(pcbnew.FromMM(d))
        v.SetDrill(pcbnew.FromMM(drill))
        v.SetNet(nets[net])
        v.SetLocked(True)
        b.Add(v)
        group.AddItem(v)

    F, B = pcbnew.F_Cu, pcbnew.B_Cu
    mmxy = lambda ref, n: tuple(round(pcbnew.ToMM(v) - o, 4) for v, o in zip(
        (pad(ref, n)[0].x, pad(ref, n)[0].y), (X0, Y0)))
    # Crystal
    p21, y1p1, c20 = mmxy("U1", "21"), mmxy("Y1", "1"), mmxy("C20", "1")
    # down past pin 20's 3V3 stub, between C6 and R6, under C6's GND via, round Y1 pad 4
    track("XIN", F, p21, (p21[0], 31.8), (9.4, 32.1), (9.4, 34.3), (9.15, 34.55), (8.55, 34.55),
          (8.3, 34.8), (8.3, y1p1[1]), y1p1)
    track("XIN", F, (8.3, y1p1[1]), c20)
    p22, r6x, r6r = mmxy("U1", "22"), mmxy("R6", "2"), mmxy("R6", "1")
    track("XOUT", F, p22, r6x)
    y1p3, c21 = mmxy("Y1", "3"), mmxy("C21", "1")
    track("XOUT_R", F, r6r, (r6r[0], 34.5), (r6r[0] + 1.15, y1p3[1]), y1p3, c21)
    # USB-C data pairs at the receptacle
    a6, b6, a7, b7, u1, u3 = (mmxy("J1", "A6"), mmxy("J1", "B6"), mmxy("J1", "A7"), mmxy("J1", "B7"),
                              mmxy("U2", "1"), mmxy("U2", "3"))
    top = a7[1] - 0.725                     # pad top edge (pads are 1.45 mm long)
    via("DM", a7[0], top - 0.95)
    via("DM", b7[0], top - 0.95)
    track("DM", F, a7, (a7[0], top - 0.95))
    track("DM", F, b7, (b7[0], top - 0.95))
    track("DM", B, (a7[0], top - 0.95), (b7[0], top - 0.95))
    track("DM", F, b7, (b7[0], u1[1] - 0.35), (u1[0] + 0.66 + 0.2, u1[1]), u1)
    # D+ ties A6/B6 with a U on L1, then drops to L4 under U2 and comes up
    # below pin 3: on L1 it would fill the gap between U5 and U2 that the
    # flash's SD0/SCLK/SD3 need (the first try left QSPI_SD2 unroutable)
    bot = a6[1] + 0.725 + 0.35
    track("DP", F, b6, (b6[0], bot), (a6[0], bot), a6)
    below = u3[1] + 0.7
    via("DP", b6[0], bot)
    via("DP", u3[0], below)
    track("DP", B, (b6[0], bot), (b6[0], below - (u3[0] - b6[0])), (u3[0], below))
    track("DP", F, (u3[0], below), u3)
    # U2 to the 27 ohm resistors (R5 D+, R4 D-): each drops beside U2, runs
    # straight down L4 and comes up beside its resistor, ~10 mm and 2 vias each.
    # Left to Freerouting, D+ once went back up to J1 and down again (29.9 mm).
    u4, u6, r5, r4 = mmxy("U2", "4"), mmxy("U2", "6"), mmxy("R5", "1"), mmxy("R4", "1")
    dp_top, dp_bot = (u4[0] + 0.71, u4[1] + 0.71), (r5[0] + 3.12, r5[1] - 1.43)
    via("USB_DP", *dp_top)
    via("USB_DP", *dp_bot)
    track("USB_DP", F, u4, dp_top, w=0.2)
    track("USB_DP", B, dp_top, (dp_top[0] + 0.16, dp_top[1] + 0.16), (dp_top[0] + 0.16, dp_top[1] + 1.74),
          (dp_bot[0], dp_top[1] + 2.43), dp_bot, w=0.2)
    track("USB_DP", F, dp_bot, (r5[0] + 1.43, dp_bot[1]), r5, w=0.2)
    dm_top, dm_bot = (u6[0] - 1.02, u6[1] + 0.47), (r4[0] + 0.75, r4[1] - 0.89)
    via("USB_DM", *dm_top)
    via("USB_DM", *dm_bot)
    track("USB_DM", F, u6, (u6[0] - 0.55, u6[1]), dm_top, w=0.2)
    track("USB_DM", B, dm_top, (dm_top[0], dm_bot[1] - 0.17), dm_bot, w=0.2)
    track("USB_DM", F, dm_bot, (r4[0], r4[1] - 0.14), r4, w=0.2)
    # The pixel chain: each DOUT (pad 1, bottom-right) straight to the next DIN
    # (pad 3, top-left) across the gap, on L1 (rgb-leds.md: short hops on the top)
    for net, (a, c) in {"PIX12": ("D3", "D4"), "PIX23": ("D4", "D5"), "PIX34": ("D5", "D6")}.items():
        o, i = mmxy(a, "1"), mmxy(c, "3")
        track(net, F, o, (o[0] + 0.4, o[1]), (i[0] - 0.4, i[1]), i, w=0.1)


def stage_place():
    import subprocess
    b = pcbnew.LoadBoard(str(PCB))
    fps = {fp.GetReference(): fp for fp in b.GetFootprints()}
    place_fp(fps, "U1", *U1_AT)
    pcbnew.SaveBoard(str(PCB), b)
    del b
    r = subprocess.run([sys.executable, str(SKILL / "transplant.py"), str(VENDOR), str(PCB),
                        "--anchor", "U1=U1", "--refs", ",".join(f"{a}={b}" for a, b in TRANSPLANT_REFS.items()),
                        "--nets", ",".join(f"{a}={b}" for a, b in TRANSPLANT_NETS.items()),
                        "--box", TRANSPLANT_BOX, "--exclude", TRANSPLANT_EXCLUDE],
                       capture_output=True, text=True)
    print(r.stdout.strip())
    if r.returncode:
        sys.exit(r.stderr)
    b = pcbnew.LoadBoard(str(PCB))
    fps = {fp.GetReference(): fp for fp in b.GetFootprints()}
    # The vendor copy places the regulator's parts as Raspberry Pi's board has them; its copper
    # goes, since the router lays every track (LESSONS.md, [routing]).
    copied = [t for t in b.GetTracks() if t.GetParentGroup() is not None and t.GetParentGroup().GetName() == "transplant"]
    copied += [z for z in b.Zones() if z.GetZoneName().startswith("transplant")]
    for item in copied:
        b.Remove(item)
    for fp in fps.values():
        fp.SetLocked(fp.GetReference() in LOCKED)
    BOTTOM = {"TP3"}
    for ref, (x, y, rot) in PLACE.items():
        if ref in fps:
            if (ref in BOTTOM) != fps[ref].IsFlipped():
                fps[ref].Flip(fps[ref].GetPosition(), pcbnew.FLIP_DIRECTION_LEFT_RIGHT)
            place_fp(fps, ref, x, y, rot)
    missing = sorted(set(fps) - set(PLACE) - set(TRANSPLANT_REFS.values()) - {"U1", "J1", "J2", "J3", "J4"})
    if missing:
        print("  not placed yet:", ", ".join(missing))
    pcbnew.SaveBoard(str(PCB), b)


# ---------------------------------------------------------------- planes (phase 7)

PLANE_TAG = "plane"
SOFT_TAG = "soft-novia"   # ultraroute: vias cost a detour here but are allowed


def stage_zones(pours: bool = True, keepouts: bool = True):
    """Ground on L2 (and filling L1/L4), 3V3 on L3 under the RP2350 and flash,
    and the layer-2 cut-out under VREG_LX (stackup.md, chips/rp2350.md).
    With pours=False only the L2 and L3 planes and the cut-out, for routing:
    the router keeps tracks off the plane layers and joins their nets through
    the fill, and the L1/L4 pours go in after routing (a full-board outer pour
    is copper every other net must route round)."""
    b = pcbnew.LoadBoard(str(PCB))
    zones = list(b.Zones())
    fps = {fp.GetReference(): fp for fp in b.GetFootprints()}

    # Every polygon is built before anything is removed: after a Remove(),
    # pcbnew's SWIG proxies for the remaining zones can return garbage.
    # The regulator corner: L1's body, where no other top copper may run (chips/rp2350.md), grown
    # 0.15 mm. Its copper is the router's now (no locked vendor copy: LESSONS.md, [routing]).
    reg = pcbnew.SHAPE_POLY_SET()
    reg.Append(fps["L1"].GetCourtyard(pcbnew.F_CrtYd))
    reg.Inflate(pcbnew.FromMM(0.15), pcbnew.CORNER_STRATEGY_ROUND_ALL_CORNERS, pcbnew.FromMM(0.01))
    # No L2 copper under the switch node: the VREG_LX pads (L1's and U1's) and whatever VREG_LX
    # copper is on L1 already (after routing), each grown 0.3 mm (a bounding box would swallow
    # the PGND vias).
    cut = pcbnew.SHAPE_POLY_SET()
    for fp in (fps["L1"], fps["U1"]):
        for pad in fp.Pads():
            if pad.GetNetname().endswith("VREG_LX"):
                pad.TransformShapeToPolygon(cut, pcbnew.F_Cu, 0, pcbnew.FromMM(0.005), pcbnew.ERROR_INSIDE)
    for t in b.GetTracks():
        if t.GetNetname().endswith("VREG_LX") and t.GetLayer() == pcbnew.F_Cu:
            t.TransformShapeToPolygon(cut, pcbnew.F_Cu, 0, pcbnew.FromMM(0.005), pcbnew.ERROR_INSIDE)
    cut.Simplify()
    cut.Inflate(pcbnew.FromMM(0.3), pcbnew.CORNER_STRATEGY_ROUND_ALL_CORNERS, pcbnew.FromMM(0.01))
    for z in zones:
        if z.GetZoneName().startswith((PLANE_TAG, SOFT_TAG)):
            b.Remove(z)

    def box(x0, y0, x1, y1):
        poly = pcbnew.SHAPE_POLY_SET()
        poly.NewOutline()
        for x, y in ((x0, y0), (x1, y0), (x1, y1), (x0, y1)):
            v = mm(x, y)
            poly.Append(v.x, v.y)
        return poly

    def rect(name, layer, net, x0, y0, x1, y1, priority=0, keepout=False, minus=None):
        z = pcbnew.ZONE(b)
        z.SetZoneName(f"{PLANE_TAG} {name}")
        z.SetLayer(layer)
        poly = box(x0, y0, x1, y1)
        if minus is not None:
            poly.BooleanSubtract(minus)
            # A zone holds one outline (with holes): keep the largest and drop
            # the slivers a subtraction can leave at the edges.
            while poly.OutlineCount() > 1:
                areas = [abs(poly.COutline(i).Area()) for i in range(poly.OutlineCount())]
                poly.DeletePolygon(areas.index(min(areas)))
        poly.thisown = False                # SetOutline takes ownership of the polygon
        z.SetOutline(poly)
        if keepout:
            z.SetIsRuleArea(True)
            z.SetDoNotAllowCopperPour(True)
            z.SetDoNotAllowTracks(True)
            z.SetDoNotAllowVias(True)
            z.SetDoNotAllowPads(False)
            z.SetDoNotAllowFootprints(False)
        else:
            z.SetNet(b.FindNet(net))
            z.SetAssignedPriority(priority)
            z.SetMinThickness(pcbnew.FromMM(0.2))
            z.SetLocalClearance(pcbnew.FromMM(0.2))
            z.SetThermalReliefGap(pcbnew.FromMM(0.2))
            z.SetThermalReliefSpokeWidth(pcbnew.FromMM(0.3))
        b.Add(z)
        return z

    full = (X0 + 0.3, Y0 + 0.3, X0 + W - 0.3, Y0 + H - 0.3)
    rect("GND L2", pcbnew.In1_Cu, "GND", *full)
    # 3V3 is a whole-board plane on L3, present while routing so the router
    # drops 3V3 vias into it (a pour added after routing left 3V3 islands).
    rect("3V3 L3", pcbnew.In2_Cu, "+3V3", *full)
    if pours:
        rect("GND L1", pcbnew.F_Cu, "GND", *full, minus=reg)
        # 3V3 on L1 around U1, over the ground pour: the vendor joins IOVDD, its
        # band and the decouplers to 3V3 with a top pour like this; without it they
        # were one island the router and stitcher could not reach (phase 7).
        ux, uy = U1_AT
        keep_out = pcbnew.SHAPE_POLY_SET(reg)
        keep_out.BooleanAdd(box(X0 + ux - 3.0, Y0 + uy - 3.0, X0 + ux + 3.0, Y0 + uy + 3.0))   # the pad ring
        rect("3V3 L1 around U1", pcbnew.F_Cu, "+3V3", X0 + ux - 7.2, Y0 + uy - 5.2,
             X0 + ux + 7.2, Y0 + uy + 5.2, priority=1, minus=keep_out)
        rect("GND L4", pcbnew.B_Cu, "GND", *full)
    # The art zone (brief.md §9): no vias under the fixed bottom silk, so the
    # router, the pad vias and the stitching keep off the labels. Tracks and
    # pours are fine under silk.
    # Where the vendor's locked copper already has a via, no rule area: the
    # silk stage nudges that label instead.
    # vias that stay put: the vendor copy's and the pre-routes' (pad vias and the
    # VBUS twins are re-placed every run and keep off the reservations themselves;
    # a hole for one would hold its spot under the label for the next run)
    def lasting(v):
        g = v.GetParentGroup()
        return v.IsLocked() and not (g is not None and (g.GetName().startswith("padvias")
                                                        or g.GetName() == "twin VBUS"))
    locked_vias = [v.GetPosition() for v in b.GetTracks() if v.Type() == pcbnew.PCB_VIA_T and lasting(v)]
    for tx in silk_fixed(b):
        ps = pcbnew.SHAPE_POLY_SET()
        tx.TransformTextToPolySet(ps, 0, pcbnew.FromMM(0.005), pcbnew.ERROR_INSIDE)
        bb = ps.BBox()
        bb.Inflate(pcbnew.FromMM(0.15))
        p = tx.GetPosition()
        # J2 pins 1-11: GPIO0-5 drop to L4 beside these labels to get past
        # the flash. As no-via areas the router left 3-4 connections open on
        # every seed (Freerouting twice, ultraroute 574caac seeds 0-3), so
        # they are soft: a via there costs the router a detour but is allowed
        # (KiCad ignores the area). finish moves any via that still lands
        # under one (via_nudge.py) and the silk stage nudges the text
        soft = tx.GetHorizJustify() == pcbnew.GR_TEXT_H_ALIGN_RIGHT and pcbnew.ToMM(p.y) - Y0 < PIN1 + 2.54 * 10.5
        poly = pcbnew.SHAPE_POLY_SET()
        poly.NewOutline()
        for x, y in ((bb.GetLeft(), bb.GetTop()), (bb.GetRight(), bb.GetTop()),
                     (bb.GetRight(), bb.GetBottom()), (bb.GetLeft(), bb.GetBottom())):
            poly.Append(x, y)
        # a locked via already under the label (the vendor's under RUN, a
        # pre-route's or a pad via): cut a hole round it rather than drop the
        # area, or the router puts more vias there; the silk stage nudges the text
        for v in locked_vias:
            if bb.GetInflated(pcbnew.FromMM(0.3)).Contains(v):
                hole = pcbnew.SHAPE_POLY_SET()
                hole.NewOutline()
                h = pcbnew.FromMM(0.5)
                for x, y in ((v.x - h, v.y - h), (v.x + h, v.y - h), (v.x + h, v.y + h), (v.x - h, v.y + h)):
                    hole.Append(x, y)
                poly.BooleanSubtract(hole)
        z = pcbnew.ZONE(b)
        z.SetZoneName(f"{SOFT_TAG if soft else PLANE_TAG} silk {tx.GetText()}")
        z.SetLayerSet(pcbnew.LSET.AllCuMask())
        z.SetIsRuleArea(True)
        z.SetDoNotAllowCopperPour(False)
        z.SetDoNotAllowTracks(False)
        z.SetDoNotAllowVias(not soft)
        z.SetDoNotAllowPads(False)
        z.SetDoNotAllowFootprints(False)
        poly.thisown = False
        z.SetOutline(poly)
        b.Add(z)
    k = pcbnew.ZONE(b)
    k.SetZoneName(f"{PLANE_TAG} no copper under VREG_LX")
    k.SetLayer(pcbnew.In1_Cu)
    k.SetIsRuleArea(True)
    k.SetDoNotAllowCopperPour(True)
    k.SetDoNotAllowTracks(True)
    k.SetDoNotAllowVias(True)
    k.SetDoNotAllowPads(False)
    k.SetDoNotAllowFootprints(False)
    cut.thisown = False                 # SetOutline takes ownership of the polygon
    k.SetOutline(cut)
    b.Add(k)
    # Nothing under a pixel's centre strip on L1 (rgb-leds.md, WS2812B-2020
    # p.2: the package's metal polarity mark): no tracks, vias or pour.
    for ref in ("D3", "D4", "D5", "D6"):
        c = fps[ref].GetPosition()
        strip = pcbnew.SHAPE_POLY_SET()
        strip.NewOutline()
        for dx, dy in ((-0.45, -1.0), (0.45, -1.0), (0.45, 1.0), (-0.45, 1.0)):
            strip.Append(c.x + pcbnew.FromMM(dx), c.y + pcbnew.FromMM(dy))
        e = pcbnew.ZONE(b)
        e.SetZoneName(f"{PLANE_TAG} pixel centre {ref}")
        e.SetLayer(pcbnew.F_Cu)
        e.SetIsRuleArea(True)
        e.SetDoNotAllowCopperPour(True)
        e.SetDoNotAllowTracks(True)
        e.SetDoNotAllowVias(True)
        e.SetDoNotAllowPads(False)
        e.SetDoNotAllowFootprints(False)
        strip.thisown = False
        e.SetOutline(strip)
        b.Add(e)
    # No tracks or vias within 0.7 mm of the long edges (pours allowed): the
    # header pads start 0.76 mm in, so nothing runs between them and the edge,
    # where JLC's ±0.2 mm outline tolerance and the hand-soldered header joints
    # are (review-release #2: BOOTSEL ran 0.35 mm from the edge, 0.14 mm from
    # J2's pads).
    for x0 in (0.0, W - EDGE_STRIP):
        strip = pcbnew.SHAPE_POLY_SET()
        strip.NewOutline()
        for x, y in ((x0, 0.0), (x0 + EDGE_STRIP, 0.0), (x0 + EDGE_STRIP, H), (x0, H)):
            strip.Append(pcbnew.FromMM(X0 + x), pcbnew.FromMM(Y0 + y))
        e = pcbnew.ZONE(b)
        e.SetZoneName(f"{PLANE_TAG} edge strip {'left' if x0 == 0 else 'right'}")
        e.SetLayerSet(pcbnew.LSET.AllCuMask())
        e.SetIsRuleArea(True)
        e.SetDoNotAllowCopperPour(False)
        e.SetDoNotAllowTracks(True)
        e.SetDoNotAllowVias(True)
        e.SetDoNotAllowPads(False)
        e.SetDoNotAllowFootprints(False)
        strip.thisown = False
        e.SetOutline(strip)
        b.Add(e)
    # The regulator corner (vendor zones outside the pad ring: VREG_LX, the 1V1
    # and 3V3 patches, PGND) is the vendor's copper only. While routing, no
    # other net's tracks (a tracks-only rule area: a no-pour rule area would
    # stop the vendor zones filling too). After routing, the GND pour is cut
    # around it, since PGND must join GND at its two vias alone (chips/rp2350.md).
    if not pours and keepouts:
        k = pcbnew.ZONE(b)
        k.SetZoneName(f"{PLANE_TAG} regulator corner")
        k.SetLayer(pcbnew.F_Cu)
        k.SetIsRuleArea(True)
        k.SetDoNotAllowCopperPour(False)
        k.SetDoNotAllowTracks(True)
        k.SetDoNotAllowVias(False)
        k.SetDoNotAllowPads(False)
        k.SetDoNotAllowFootprints(False)
        reg.thisown = False
        k.SetOutline(reg)
        b.Add(k)
    else:
        pass                            # the GND L1 pour above is cut around it
    if not pours and keepouts:
        # Routing only: nothing new inside U1's pad ring, where the vendor's 1V1
        # ring and 3V3 band live (chips/rp2350.md: no foreign net crosses it).
        # Removed again before the pours, because the vendor's own spoke vias
        # sit inside it.
        ux, uy = U1_AT
        rect("routing keepout U1 ring", pcbnew.F_Cu, None, X0 + ux - 3.0, Y0 + uy - 3.0,
             X0 + ux + 3.0, Y0 + uy + 3.0, keepout=True)
    pcbnew.SaveBoard(str(PCB), b)


def fill():
    b = pcbnew.LoadBoard(str(PCB))
    pcbnew.ZONE_FILLER(b).Fill(b.Zones())
    pcbnew.SaveBoard(str(PCB), b)


def signals_unrouted() -> int:
    """Unconnected signal connections per KiCad DRC (plane nets excluded)."""
    import json
    import shutil
    import subprocess
    import tempfile
    with tempfile.TemporaryDirectory() as tmp:
        out = Path(tmp) / "drc.json"
        subprocess.run(["kicad-cli", "pcb", "drc", "--format", "json", "--severity-error", "-o", str(out),
                        str(PCB)], capture_output=True, text=True)
        items = json.loads(out.read_text()).get("unconnected_items", [])
    return sum(1 for u in items if not any(n in d["description"] for d in u["items"]
                                           for n in ("[GND]", "[+3V3]", "[+1V1]")))


def stage_padvias():
    """Ground vias at every capacitor's ground pad, six round the LDO and one at
    the ESD array; a 3V3 via at every capacitor's 3V3 pad (the router leaves
    plane nets to vias, and C22's had no room after routing), a second at the
    LDO output cap and one at the
    flash's VCC (left to stitch --fix, it was the only spot and sat under the
    GP2 label, which can move instead): locked, before
    routing, so the autorouter works round them (review-layout.md: decoupler
    grounds that reached L2 only through the L1 pour)."""
    import subprocess
    for cmd in ([str(SKILL / "stitch.py"), str(HW), "--net", "GND", "--pads", "C*", "--pads", "U3:6",
                 "--pads", "U2", "--pads", "U4", "--pads", "D3", "--pads", "D4", "--pads", "D5", "--pads", "D6",
                 "--tag", "padvias GND", "--lock"],
                [str(SKILL / "stitch.py"), str(HW), "--net", "+3V3", "--pads", "C*", "--pads", "C3:2", "--pads", "U5", "--pads", "U3", "--wired",
                 "--tag", "padvias 3V3", "--lock"]):
        r = subprocess.run([sys.executable] + cmd, capture_output=True, text=True)
        print("\n".join(l for l in r.stdout.splitlines() if "leak" not in l and "Debug" not in l))
        if r.returncode:
            sys.exit(r.stderr[-2000:])


def stage_route():
    """Autoroute everything not already copper (routing.md) with the skill's
    autoroute.py, which runs ultraroute (vendor/ultraroute) governed, a few
    seeds in turn, judged by KiCad: L2 is the GND plane and L3 the 3V3 plane,
    signals on L1 and L4. Each step runs in its own process (pcbnew loads one
    board per process)."""
    import subprocess

    def run(cmd):
        r = subprocess.run(cmd, capture_output=True, text=True)
        print("\n".join(l for l in r.stdout.splitlines() if "leak" not in l and "Debug" not in l))
        return r.returncode

    # Finish-stage copper from an earlier run (the locked VBUS twins) must not
    # be routed through: the next finish replaces it and cuts the route.
    run([sys.executable, str(SKILL / "stitch.py"), str(HW), "--net", "VBUS", "--tag", "twin VBUS"])
    run([sys.executable, __file__, "unroute"])    # the old route would block pad-via spots
    run([sys.executable, __file__, "zones-for-routing"])
    run([sys.executable, __file__, "fill"])       # the router joins plane nets through the fill
    # A 35 um grid: the default 50 um left the QFN's 0.4 mm pitch escapes short of room.
    run([sys.executable, str(SKILL / "autoroute.py"), str(HW), "--planes", "In1.Cu,In2.Cu",
         "--extra", "--cell 0.035"])
    # The routing keepouts go before the outer pours (they stop the copied zones filling).
    run([sys.executable, __file__, "zones-bare"])
    print(f"route: {signals_unrouted()} signal connections unrouted")


def stage_unroute():
    """Remove every unlocked track and via (an earlier route) before the pad vias
    go in: they must find their spots on the pre-route board, as the router
    clears it anyway (an old route once left C25/C27 without a ground via)."""
    b = pcbnew.LoadBoard(str(PCB))
    gone = [t for t in b.GetTracks() if not t.IsLocked()]
    for t in gone:
        b.Remove(t)
    pcbnew.SaveBoard(str(PCB), b)
    print(f"unroute: removed {len(gone)} unlocked tracks and vias")


def stage_clean():
    """Delete unlocked signal-net vias and track stubs DRC calls dangling:
    router leftovers (one via sat 0.23 mm from its twin, a hole-to-hole error;
    a 0.5 mm GPIO28 stub; vendor copper to a pad this board lacks). A stub's
    removal can leave its via dangling, so stage_finish repeats this until it
    removes nothing (one board load per process: pcbnew crashes loading the
    same board twice). A pass that would open a connection is not saved."""
    import shutil
    import json
    import subprocess
    import tempfile
    with tempfile.TemporaryDirectory() as tmp:
        out = Path(tmp) / "drc.json"
        subprocess.run(["kicad-cli", "pcb", "drc", "--format", "json", "--severity-all", "-o", str(out),
                        str(PCB)], capture_output=True, text=True)
        out_json = out.read_text()
        ids = {i["uuid"] for v in json.loads(out_json)["violations"]
               if v["type"] == "track_dangling"          # a stub carries nothing on any net
               or v["type"] == "via_dangling"            # a power via may be meant to reach a plane
               and not any(n in v["items"][0]["description"] for n in ("[GND]", "[+3V3]", "[+1V1]", "[VBUS]", "[VLED]"))
               for i in v["items"]}
    b = pcbnew.LoadBoard(str(PCB))
    # Locked copper is left alone (a dangling pre-route is a bug to see in DRC),
    # except the vendor copy's: its +3V3 stub by C9 led to a pad this board
    # doesn't have (review-layout #20)
    def vendor(t):
        g = t.GetParentGroup()
        return g is not None and g.GetName() == "transplant"
    gone = [t for t in b.GetTracks() if t.m_Uuid.AsString() in ids and (not t.IsLocked() or vendor(t))]
    # Track ends that meet inside a via can differ by nanometres; the via joined
    # them. Snap them to its centre before it goes, or removing a via that DRC
    # calls dangling (one layer only) cuts the track it sat on (it cut VBUS at U2).
    import math
    for v in [g for g in gone if g.Type() == pcbnew.PCB_VIA_T]:
        c, r = v.GetPosition(), v.GetWidth(pcbnew.F_Cu) // 2
        for t in b.GetTracks():
            if t.Type() == pcbnew.PCB_TRACE_T and t.GetNetCode() == v.GetNetCode() and t not in gone \
                    and not t.IsLocked():             # a vendor joint inside the via stays where it is
                if math.dist((t.GetStart().x, t.GetStart().y), (c.x, c.y)) <= r:
                    t.SetStart(c)
                if math.dist((t.GetEnd().x, t.GetEnd().y), (c.x, c.y)) <= r:
                    t.SetEnd(c)
    for t in gone:
        b.Remove(t)
    if gone:
        pcbnew.ZONE_FILLER(b).Fill(b.Zones())
        # Never trade a warning for an open: DRC a copy, keep it only if no
        # connection was lost (removing "dangling" VBUS copper once cut VBUS).
        before = len(json.loads(out_json).get("unconnected_items", []))
        with tempfile.TemporaryDirectory() as tmp:
            trial = Path(tmp) / PCB.name
            shutil.copy(PCB.with_suffix(".kicad_pro"), trial.with_suffix(".kicad_pro"))
            pcbnew.SaveBoard(str(trial), b)
            rep = Path(tmp) / "after.json"
            subprocess.run(["kicad-cli", "pcb", "drc", "--format", "json", "--severity-error", "-o", str(rep),
                            str(trial)], capture_output=True, text=True)
            after = len(json.loads(rep.read_text()).get("unconnected_items", []))
        if after > before:
            print(f"clean: removing {len(gone)} item(s) would open {after - before} connection(s); left as they are")
            return
        pcbnew.SaveBoard(str(PCB), b)
    print(f"clean: removed {len(gone)} dangling signal via(s) and track(s)")


def stage_finish():
    """After routing: the L1/L4 ground pours, the silkscreen, a second via at
    each VBUS layer change, then stitching
    vias (the skill's stitch.py: a grid that keeps off the silk text, and vias
    for anything the router left on GND or 3V3 without one), then a final fill."""
    import subprocess
    for cmd in ([sys.executable, __file__, "pours"],
                # vias in a reservation added after they were routed move out first
                [sys.executable, str(SKILL / "via_nudge.py"), str(HW)],
                [sys.executable, __file__, "silk"],      # first, so the grid vias keep off the text
                # then any via a label couldn't be nudged clear of (unreserved labels)
                [sys.executable, str(SKILL / "via_nudge.py"), str(HW)],
                # VBUS needs two vias per layer change (nets_check: 0.75 A peak x 1.5 > 0.8 A)
                [sys.executable, str(SKILL / "stitch.py"), str(HW), "--net", "VBUS", "--twin",
                 "--tag", "twin VBUS"],
                [sys.executable, str(SKILL / "stitch.py"), str(HW), "--net", "GND", "--net", "+3V3",
                 "--grid", "1.6", "--fix"],
                [sys.executable, __file__, "fill"],
                *[[sys.executable, __file__, "clean"]] * 3):
        r = subprocess.run(cmd, capture_output=True, text=True)
        print("\n".join(l for l in r.stdout.splitlines() if "leak" not in l and "Debug" not in l))
        if r.returncode:
            sys.exit(r.stderr[-2000:])


# ---------------------------------------------------------------- silkscreen (phase 9)

SILK_H, SILK_W = 0.8, 0.15                 # mm: text height, stroke (checks.toml fab minimums)
SELECTIVE = ("J", "U", "TP", "SW", "D", "Y")   # brief.md §10: connectors, ICs, buttons, LEDs, test point, crystal


def _label_map():
    """Board pin labels for the bottom silk, from the schematic's header map."""
    sys.path.insert(0, str(ROOT / "scripts"))
    from schematic import J2_MAP, J3_MAP
    short = {"+3V3": "3V3"}
    return ([short.get(n, n).replace("GPIO", "GP") for n in J2_MAP],
            [short.get(n, n).replace("GPIO", "GP") for n in J3_MAP])


URL_LINES = ("github.com/", "sequoia-hope/", "easy_pcb")   # the URL, wrapped: 22 mm on one line
URL_X = 6.6                                                   # first column, clear of GP11-13
URL_PITCH = 1.4                                               # descenders (q, p) need 0.15 mm to the next line
URL_Y = (39.5, 39.5, 38.0)                                    # line centres: easy_pcb stops above the PIX12 via


def silk_fixed(b):
    """The silk that has a fixed place: a label beside every header pad, J4's
    pin order, the URL, the identity, limit and warning lines, and the top's
    "5V" and "25". Returned as unplaced PCB_TEXTs; the zones stage keeps vias
    off them (brief.md §9, the art zone) and the silk stage prints them.
    Text placed only after routing found no room on some routes (the VBUS
    warning, "25"), so everything with a place is reserved first."""
    def make(s, x, y, angle=0, justify=0, top=False):
        tx = pcbnew.PCB_TEXT(b)
        tx.SetText(s)
        tx.SetLayer(pcbnew.F_SilkS if top else pcbnew.B_SilkS)
        tx.SetTextSize(pcbnew.VECTOR2I(pcbnew.FromMM(SILK_H), pcbnew.FromMM(SILK_H)))
        tx.SetTextThickness(pcbnew.FromMM(SILK_W))
        tx.SetTextAngleDegrees(angle)
        tx.SetHorizJustify(justify)
        tx.SetMirrored(not top)
        tx.SetPosition(at(x, y))
        return tx
    j2, j3 = _label_map()
    narrow = {"VBUS": 0.65}                    # mm wide: 0.8 mm wide meets J1's shell hole S1
    out = [make(s, ROW + 1.25, PIN1 + 2.54 * n, justify=pcbnew.GR_TEXT_H_ALIGN_RIGHT) for n, s in enumerate(j2)]
    out += [make(s, W - ROW - 1.25, H - PIN1 - 2.54 * n, justify=pcbnew.GR_TEXT_H_ALIGN_LEFT)
            for n, s in enumerate(j3)]
    for tx in out:
        if tx.GetText() in narrow:
            tx.SetTextWidth(pcbnew.FromMM(narrow[tx.GetText()]))
        if tx.GetText() == "RUN":
            # the vendor's 1V1 via (16.3, 26.1) sits under the pin-aligned spot:
            # reserve where the label can actually go, 0.35 mm lower
            tx.SetPosition(tx.GetPosition() + pcbnew.VECTOR2I(0, pcbnew.FromMM(0.35)))
    # Seen from the bottom, J4 pin 1 (SWCLK, square pad) is on the right.
    out.append(make("DIO GND CLK", W / 2, H - PIN1 - 1.75))
    out += [make(s, URL_X + URL_PITCH * i, URL_Y[i], angle=90) for i, s in enumerate(URL_LINES)]
    # Identity, limits and warnings, one line each, where the free-spot search
    # put them on the 2026-10-23 route (no via-free strip is over 13 mm)
    tb = b.GetTitleBlock()
    for s_, x, y, a in ((f"{tb.GetTitle()} rev {tb.GetRevision()}", 11.0, 41.25, 90), (tb.GetDate()[:7], 7.5, 27.0, 90),
                        ("CERN-OHL-P-2.0", 12.0, 19.0, 90), ("3V3 out 100mA", 13.2, 41.5, 90),
                        ("VBUS out 50mA", 9.95, 17.25, 90), ("3.3V logic only", 7.0, 12.5, 90),
                        ("no VBUS in", W / 2, 3.25, 0), ("with USB in", W / 2, 4.65, 0)):
        out.append(make(s_, x, y, angle=a))
    # Top: the USB-C input and the GPIO25 LED
    out += [make("5V", 4.2, 3.75, top=True), make("25", 17.2, 18.15, top=True)]
    # The buttons and SWD in the strip above J4: BOOT as far left and SWD as far
    # right as they go, so they don't read as one phrase (review-layout #14);
    # SWD can't sit over J4's middle (U4's courtyard ends 0.2 mm above J4's pads)
    out += [make("BOOT", 4.0, 48.05, top=True), make("SWD", 7.5, 48.05, top=True),
            make("RST", 16.4, 48.05, top=True)]
    return out


def stage_silk():
    """Silkscreen (brief.md §10, decisions.md): no designators on the copper
    side (the top is too dense; the fab layer carries them for the assembly
    drawing), function labels placed where they fit, footprint silk trimmed
    where it meets pads or the edge, and the bottom's pin labels and identity."""
    b = pcbnew.LoadBoard(str(PCB))
    fps = {fp.GetReference(): fp for fp in b.GetFootprints()}
    drawings = list(b.GetDrawings())
    # this stage's own output from an earlier run: board text and the pin-1 dots
    old_text = [d for d in drawings if d.GetClass() in ("PCB_TEXT", "PCB_SHAPE")
                and d.GetLayer() in (pcbnew.F_SilkS, pcbnew.B_SilkS)]
    tracks = list(b.GetTracks())
    edge = pcbnew.BOX2I(at(0.35, 0.35), pcbnew.VECTOR2L(pcbnew.FromMM(W - 0.7), pcbnew.FromMM(H - 0.7)))

    def court(fp):
        c = fp.GetCourtyard(pcbnew.B_CrtYd if fp.IsFlipped() else pcbnew.F_CrtYd)
        return c.BBox() if c.OutlineCount() else fp.GetBoundingBox(False)

    copper = {False: [], True: []}               # side -> mask openings silk must avoid
    vias = []                                    # (centre, radius): both sides
    for fp in fps.values():
        for pad in fp.Pads():
            for side in (False, True):
                if pad.IsOnLayer(pcbnew.B_Mask if side else pcbnew.F_Mask):
                    bb = pad.GetBoundingBox()
                    bb.Inflate(pcbnew.FromMM(0.05))
                    copper[side].append(bb)
    for tr in tracks:
        grp = tr.GetParentGroup()
        if grp is not None and grp.GetName() == "stitch":
            continue                               # replaced after the silk, keeping off it
        if tr.Type() == pcbnew.PCB_VIA_T:          # tested against the text strokes, not boxes
            vias.append((tr.GetPosition(), tr.GetWidth(pcbnew.F_Cu) // 2 + pcbnew.FromMM(0.12)))
    for fp in fps.values():                      # plated holes the mask covers (thermal vias)
        for pad in fp.Pads():
            if pad.HasHole() and not (pad.IsOnLayer(pcbnew.F_Mask) and pad.IsOnLayer(pcbnew.B_Mask)):
                r = max(pad.GetSize(pcbnew.F_Cu).x, pad.GetSize(pcbnew.F_Cu).y, pad.GetDrillSizeX()) // 2
                vias.append((pad.GetPosition(), r + pcbnew.FromMM(0.12)))
    courts = {False: [], True: []}
    for ref, fp in fps.items():
        if ref in ("J2", "J3", "J4"):           # through-hole headers: the board between pads is bare
            continue
        courts[fp.IsFlipped()].append(court(fp))

    placed = {False: [], True: []}               # side -> silk already there
    # 1. Footprint silk that runs onto pads or past the edge: board-local trim.
    trimmed = []
    for ref, fp in fps.items():
        for item in list(fp.GraphicalItems()):
            if item.GetLayer() not in (pcbnew.F_SilkS, pcbnew.B_SilkS) or item.GetClass() == "PCB_TEXT":
                continue
            side = item.GetLayer() == pcbnew.B_SilkS
            bb = item.GetBoundingBox()
            others = [court(o) for r, o in fps.items() if r != ref and o.IsFlipped() == side]
            # and silk that meets another part's silk (turned parts, e.g. the pixels)
            theirs = [i.GetBoundingBox() for r, o in fps.items() if r != ref and r < ref and o.IsFlipped() == side
                      for i in o.GraphicalItems() if i.GetLayer() == item.GetLayer() and i.GetClass() != "PCB_TEXT"]
            if ref in ("J2", "J3", "J4") \
                    or not edge.Contains(bb.GetOrigin()) or not edge.Contains(bb.GetEnd()) \
                    or any(bb.Intersects(c) for c in copper[side]) \
                    or any(bb.Intersects(c) for c in others) \
                    or any(bb.GetInflated(pcbnew.FromMM(0.15)).Intersects(t) for t in theirs):
                # header outlines go entirely (the pin labels do their job); other
                # silk goes where it meets pads, the edge or a neighbour's body
                trimmed.append((fp, item))
    # 2. Designators off the silk (policy none); values off.
    for fp in fps.values():
        fp.Reference().SetVisible(False)
        fp.Value().SetVisible(False)
    for fp, item in trimmed:
        fp.Remove(item)
    for fp in fps.values():                      # what is left of the footprints' silk
        for item in fp.GraphicalItems():
            if item.GetLayer() in (pcbnew.F_SilkS, pcbnew.B_SilkS) and item.GetClass() != "PCB_TEXT":
                placed[item.GetLayer() == pcbnew.B_SilkS].append(item.GetBoundingBox())
    for d in old_text:
        b.Remove(d)

    def outline(tx):
        ps = pcbnew.SHAPE_POLY_SET()
        tx.TransformTextToPolySet(ps, 0, pcbnew.FromMM(0.005), pcbnew.ERROR_INSIDE)
        return ps

    def tight(tx):
        """The strokes' box (KiCad's text box is ~1.8x the text height)."""
        bb = outline(tx).BBox()
        bb.Inflate(pcbnew.FromMM(0.05))
        return bb

    def clear(tx, side, bx=None, apart=0.16):
        """Strokes clear of part bodies, pads, other silk and vias (silk_check.py
        tests the strokes too). `apart` (mm) from other text: the board's silk
        clearance for fixed labels, more for text placed by search, so two
        unrelated words don't read as one ("2026-10" 0.38 mm from "GP8" did)."""
        ps = outline(tx)
        bx = ps.BBox()
        bx.Inflate(pcbnew.FromMM(0.05))
        if not (edge.Contains(bx.GetOrigin()) and edge.Contains(bx.GetEnd())):
            return False
        if any(bx.Intersects(o) for o in courts[side] + copper[side]):
            return False
        for o in placed[side]:                   # other silk: a box, or (box, strokes) for text
            if isinstance(o, tuple):
                if o[0].GetInflated(pcbnew.FromMM(apart)).Intersects(bx) and ps.Collide(o[1], pcbnew.FromMM(apart)):
                    return False
            elif o.Intersects(bx):
                return False
        return not any(ps.Collide(c, r) for c, r in vias if bx.GetInflated(r).Contains(c))

    missed = []

    def text(s, x, y, side=False, angle=0, justify=0, size=SILK_H, slide=(0,), axis=1, quiet=False, width=None):
        """Board text at (x, y) mm, moved along `axis` by the first `slide` offset
        (mm), or by the first (dx, dy) in `slide`, that keeps it clear of pads,
        vias, part bodies and other text."""
        tx = pcbnew.PCB_TEXT(b)
        tx.SetText(s)
        tx.SetLayer(pcbnew.B_SilkS if side else pcbnew.F_SilkS)
        tx.SetTextSize(pcbnew.VECTOR2I(pcbnew.FromMM(width or size), pcbnew.FromMM(size)))
        tx.SetTextThickness(pcbnew.FromMM(SILK_W))
        tx.SetTextAngleDegrees(angle)
        tx.SetHorizJustify(justify)
        tx.SetMirrored(side)
        for d in slide:
            dx, dy = d if isinstance(d, tuple) else ((d, 0) if axis == 0 else (0, d))
            tx.SetPosition(at(x + dx, y + dy))
            if clear(tx, side):
                break
        else:
            missed.append(s)
            if quiet:
                pass
            else:
                print(f"silk: {s!r} overlaps something at every offset tried")
        b.Add(tx)
        placed[side].append((tight(tx), outline(tx)))
        return tx

    def label(s, ref, side=False):
        """A label beside a part: the nearest free spot around its courtyard."""
        c = court(fps[ref])
        cx, cy = c.GetCenter().x, c.GetCenter().y
        gap = pcbnew.FromMM(0.2)
        for angle in (0, 90):
            tx = text(s, 0, 0, side=side, angle=angle, quiet=True)
            b.Remove(tx)
            placed[side].pop()
            tb = tight(tx)
            tw, th = tb.GetWidth(), tb.GetHeight()
            best = None
            for slide in (0, -1, 1, -2, 2, -3, 3):
                for x, y in ((cx, c.GetTop() - gap - th // 2), (cx, c.GetBottom() + gap + th // 2),
                             (c.GetLeft() - gap - tw // 2, cy), (c.GetRight() + gap + tw // 2, cy)):
                    pos = pcbnew.VECTOR2I(int(x + slide * pcbnew.FromMM(0.3)), int(y))
                    bx = pcbnew.BOX2I(pcbnew.VECTOR2I(pos.x - tw // 2, pos.y - th // 2),
                                      pcbnew.VECTOR2L(tw, th))
                    if not (edge.Contains(bx.GetOrigin()) and edge.Contains(bx.GetEnd())):
                        continue
                    tx.SetPosition(pos)
                    if not clear(tx, side, bx, apart=0.45):
                        continue
                    d = (pos.x - cx) ** 2 + (pos.y - cy) ** 2
                    if best is None or d < best[0]:
                        best = (d, pos)
            if best:
                tx = text(s, 0, 0, side=side, angle=angle, quiet=True)
                tx.SetPosition(best[1])
                placed[side][-1] = (tight(tx), outline(tx))
                return True
        print(f"silk: no room for the {s!r} label at {ref}")
        return False

    def dot(ref, pad, why):
        """A 0.3 mm silk dot beside `pad` of `ref`, on the side away from the
        part's centre, where the trim pass took the footprint's own pin-1 or
        polarity mark (silkscreen-and-art.md: marks must survive trimming)."""
        import math
        fp = fps[ref]
        q = next(x for x in fp.Pads() if x.GetNumber() == pad)
        side = fp.IsFlipped()
        c, mid = q.GetPosition(), fp.GetPosition()
        half = max(q.GetBoundingBox().GetWidth(), q.GetBoundingBox().GetHeight()) // 2
        out = math.atan2(c.y - mid.y, c.x - mid.x)
        r = pcbnew.FromMM(0.15)
        spots = []
        for gap in (0.2, 0.3, 0.45, 0.6, 0.8):
            for k in range(24):
                a = math.radians(15 * k)         # 15 degrees: a QFN's pin-1 corner is on the diagonal
                d = half + pcbnew.FromMM(gap)
                spots.append((math.cos(a - out) * -1 + gap, pcbnew.VECTOR2I(int(c.x + d * math.cos(a)),
                                                                            int(c.y + d * math.sin(a)))))
        others = [court(o) for rr, o in fps.items() if rr != ref and o.IsFlipped() == side
                  and rr not in ("J2", "J3", "J4")]
        rest = [x.GetPosition() for x in fp.Pads() if x.GetNumber() != pad]
        for _, pos in sorted(spots, key=lambda t: t[0]):
            mine = math.dist((pos.x, pos.y), (c.x, c.y))
            if any(math.dist((pos.x, pos.y), (o.x, o.y)) < 1.4 * mine for o in rest):
                continue                         # must point at this pad, not its neighbour
            bx = pcbnew.BOX2I(pcbnew.VECTOR2I(pos.x - r, pos.y - r), pcbnew.VECTOR2L(2 * r, 2 * r))
            if not (edge.Contains(bx.GetOrigin()) and edge.Contains(bx.GetEnd())):
                continue
            hit = bx.GetInflated(pcbnew.FromMM(0.05))
            if any(hit.Intersects(o) for o in copper[side] + others):
                continue
            if any(o.Intersects(bx.GetInflated(pcbnew.FromMM(0.16))) for o in placed[side] if not isinstance(o, tuple)) \
                    or any(o[0].Intersects(bx.GetInflated(pcbnew.FromMM(0.4))) for o in placed[side] if isinstance(o, tuple)):
                continue                         # 0.4 mm from text: a dot by a word reads as part of it
            if any(math.dist((pos.x, pos.y), (v.x, v.y)) < rv + r for v, rv in vias):
                continue
            sh = pcbnew.PCB_SHAPE(b)
            sh.SetShape(pcbnew.SHAPE_T_CIRCLE)
            sh.SetCenter(pos)
            sh.SetEnd(pcbnew.VECTOR2I(pos.x + r // 2, pos.y))
            sh.SetWidth(r)                       # ring of width r round radius r/2: a solid 0.3 mm dot
            sh.SetFilled(True)
            sh.SetLayer(pcbnew.B_SilkS if side else pcbnew.F_SilkS)
            b.Add(sh)
            placed[side].append(bx)
            return
        missed.append(f"{ref} {why}")
        print(f"silk: no room for the {why} dot at {ref}")

    # Function labels (silkscreen-and-art.md): connectors, buttons, LED, test point.
    # Bottom: pin labels beside every header pad, as on a Pico, J4's pin order
    # and the URL, where the zones stage kept vias away; nudged only if needed.
    nudge = [(dx * 0.3, dy) for dx in range(6) for dy in (0, 0.3, -0.3, 0.5, -0.5, 0.7, -0.7)]
    nudge += [(dx * 0.3, dy) for dx in range(6) for dy in (0.9, 1.1)]   # VBUS: J1's shell hole is above it
    for tx in silk_fixed(b):
        p = tx.GetPosition()
        x, y = pcbnew.ToMM(p.x) - X0, pcbnew.ToMM(p.y) - Y0
        sign = -1 if tx.GetHorizJustify() == pcbnew.GR_TEXT_H_ALIGN_LEFT else 1
        side = tx.GetLayer() == pcbnew.B_SilkS
        args = dict(side=side, angle=tx.GetTextAngleDegrees(), justify=tx.GetHorizJustify(), quiet=True)
        t1 = text(tx.GetText(), x, y, slide=[(sign * dx, dy) for dx, dy in nudge],
                  width=pcbnew.ToMM(tx.GetTextWidth()), **args)
        if missed and missed[-1] == tx.GetText():
            # no clear spot (the unreserved J2 labels beside GPIO0-5's vias): try
            # narrower and further, still well inside the 2.54 mm pin pitch
            b.Remove(t1)
            placed[side].pop()
            missed.pop()
            wide = nudge + [(dx * 0.3, dy) for dx in range(6) for dy in (-0.9, -1.1)]
            text(tx.GetText(), x, y, slide=[(sign * dx, dy) for dx, dy in wide], width=0.6,
                 **dict(args, quiet=False))
    label("1V1", "TP3", side=True)             # after the fixed text, which cannot move far
    # The rest goes at the free spot nearest where it would like to be (a 2D
    # search, vertical first). Only the Pico warning is left to search: it was
    # added at the release review, after the reserved spots were chosen.
    groups = [(["not Pico pinout"], (W / 2, 30.0))]   # user-io.md: a Pico carrier puts 5 V on GPIO29
    grid = sorted(((dx * 0.25, dy * 0.25) for dx in range(-24, 25) for dy in range(-100, 101)),
                  key=lambda d: d[0] ** 2 + (d[1] / 3) ** 2)

    def make(s, angle):
        tx = pcbnew.PCB_TEXT(b)
        tx.SetText(s)
        tx.SetLayer(pcbnew.B_SilkS)
        tx.SetTextSize(pcbnew.VECTOR2I(pcbnew.FromMM(SILK_H), pcbnew.FromMM(SILK_H)))
        tx.SetTextThickness(pcbnew.FromMM(SILK_W))
        tx.SetTextAngleDegrees(angle)
        tx.SetMirrored(True)
        return tx

    def fit(group, gx, gy, angle):
        """The group's lines at the free spot nearest (gx, gy): side by side when
        vertical, stacked when horizontal."""
        txs = [make(s, angle) for s in group]
        for dx, dy in grid:
            for j, tx in enumerate(txs):
                tx.SetPosition(at(gx + dx + (1.4 * j if angle else 0), gy + dy + (0 if angle else 1.4 * j)))
            if all(clear(tx, True, apart=0.45) for tx in txs) and not any(
                    tight(a).Intersects(tight(c)) for a in txs for c in txs if a is not c):
                return txs
        return None

    for group, (gx, gy) in groups:
        txs = fit(group, gx, gy, 90) or fit(group, W / 2, gy, 0)
        if txs is None:
            print(f"silk: no room for {group}")
            continue
        for tx in txs:
            b.Add(tx)
            placed[True].append((tight(tx), outline(tx)))
    # Last, so they keep clear of all the text: the marks the trim pass removed,
    # pin 1 of the QFN/DFN/SOT parts and pixels, the diodes' cathodes, L1's
    # 1V1 end (review-layout #17; the vendor board marks it).
    for ref, pad, why in (("U1", "1", "pin 1"), ("U3", "1", "pin 1"), ("U4", "1", "pin 1"),
                          ("D2", "1", "cathode"), ("D7", "1", "cathode"), ("L1", "1", "1V1 end"),
                          *[(f"D{i}", "1", "pin 1") for i in range(3, 7)]):   # the pixels' chamfer line
        dot(ref, pad, why)
    pcbnew.SaveBoard(str(PCB), b)
    print(f"silk: trimmed {len(trimmed)} footprint silk items that met pads or the edge")


STAGES = {"sync": [stage_sync], "refresh": [stage_refresh], "outline": [stage_outline], "place": [stage_place],
          "zones": [stage_zones], "zones-for-routing": [lambda: stage_zones(pours=False)],
          "zones-bare": [lambda: stage_zones(pours=False, keepouts=False)],
          "route": [stage_route], "stubs": [lambda: _with_board(band_stubs)],
          "critical": [lambda: _with_board(critical_routes)],
          "pours": [stage_zones], "padvias": [stage_padvias], "unroute": [stage_unroute], "clean": [stage_clean], "fill": [fill], "finish": [stage_finish], "silk": [stage_silk]}

if __name__ == "__main__":
    if len(sys.argv) != 2 or sys.argv[1] not in STAGES:
        sys.exit(__doc__)
    for fn in STAGES[sys.argv[1]]:
        fn()
    print(f"{sys.argv[1]}: wrote {PCB}")
