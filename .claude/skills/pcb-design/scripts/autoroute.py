#!/usr/bin/env python3
"""autoroute.py -- Freerouting, headless and governed, several settings tried in turn.

    autoroute.py hw/main/a --planes In1.Cu          L2 is a plane: route nothing on it
    autoroute.py hw/main/a --planes In1.Cu,In2.Cu --variants 8 --passes 100

What it does (references/routing.md):

  1. exports the board to Specctra DSN (KiCad's own exporter); locked tracks
     and vias go out as fixed wiring and come back untouched, zones go out as
     planes
  2. marks every --planes layer as a power layer, so Freerouting routes
     nothing on it but can drop vias into it
  3. tries up to N variants of Freerouting's autoroute settings (via cost,
     layer directions, cost of running against them, rip-up cost) one after
     another, stopping at the first that leaves no signal unrouted. Its
     default layer costs left a 4-layer board's bottom layer nearly empty
     while signals failed for room; trying other settings is what got the
     example board routed
  4. keeps the variant with the fewest unrouted signal connections (plane and
     zone nets don't count: Freerouting reports pads it reaches through zones
     as unrouted), then most-routed overall, and imports its session
  5. writes route/autoroute.json with every variant's result

Every run is governed (routing.md, "Running the autorouter without hurting
the host"): one at a time, `nice -n 15`, a systemd scope with a memory
ceiling when systemd-run is available, a JVM heap cap, a wall-clock timeout
per run, and one thread unless --threads says otherwise (at most half the
cores).

Unlocked tracks already on the board are removed first. (Starting Freerouting
from an earlier result instead made the example board worse, 6 -> 14
unrouted, and KiCad's session import replaces the routing anyway.) Zones that should
not be obstacles (full-board pours on routing layers) must not exist yet: add
them after routing, then fill. Rule areas (keepouts) are honoured.

Needs Java and the Freerouting jar ($FREEROUTING_JAR, or
~/.local/share/freerouting/freerouting-*.jar).
"""
from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from kicad_files import find_project  # noqa: E402

# (via cost, F.Cu dir, In dir, B.Cu dir, against-direction cost, rip-up cost)
VARIANTS = [
    (20, "vertical", "horizontal", "vertical", 1.5, 100),
    (50, "vertical", "horizontal", "vertical", 1.5, 100),
    (30, "horizontal", "vertical", "horizontal", 1.2, 100),
    (50, "horizontal", "vertical", "vertical", 2.0, 100),
    (20, "horizontal", "vertical", "vertical", 1.2, 300),
    (30, "vertical", "vertical", "vertical", 1.0, 50),
    (80, "vertical", "horizontal", "vertical", 2.0, 200),
    (30, "vertical", "horizontal", "horizontal", 1.2, 100),
    (10, "vertical", "horizontal", "vertical", 1.5, 100),
    (20, "horizontal", "vertical", "vertical", 1.5, 50),
    (40, "vertical", "horizontal", "vertical", 1.3, 300),
    (20, "vertical", "horizontal", "horizontal", 2.5, 100),
    (60, "horizontal", "vertical", "horizontal", 1.5, 150),
    (15, "horizontal", "horizontal", "vertical", 1.2, 100),
    (25, "vertical", "vertical", "horizontal", 1.8, 80),
    (35, "horizontal", "vertical", "vertical", 1.0, 250),
]


def jar() -> Path:
    if os.environ.get("FREEROUTING_JAR"):
        return Path(os.environ["FREEROUTING_JAR"])
    found = sorted((Path.home() / ".local/share/freerouting").glob("freerouting-*.jar"))
    if not found:
        sys.exit("Freerouting jar not found: set FREEROUTING_JAR")
    return found[-1]


def pcbnew_step(code: str, *args: str):
    """Run a short pcbnew snippet in its own process (one board per process)."""
    r = subprocess.run([sys.executable, "-c", code, *args], capture_output=True, text=True)
    if r.returncode:
        sys.exit(r.stderr[-3000:])
    return r.stdout


EXPORT = """
import sys, pcbnew
b = pcbnew.LoadBoard(sys.argv[1])
old = [t for t in list(b.GetTracks()) if not t.IsLocked()]
for t in old:
    b.Remove(t)
ok = pcbnew.ExportSpecctraDSN(b, sys.argv[2])
pcbnew.SaveBoard(sys.argv[1], b)
print("EXPORTED", len(old), ok)
"""

IMPORT = """
import sys, pcbnew
b = pcbnew.LoadBoard(sys.argv[1])
if not pcbnew.ImportSpecctraSES(b, sys.argv[2]):
    sys.exit("import failed")
ds = b.GetDesignSettings()
narrow = vias = 0
for t in list(b.GetTracks()):
    if t.IsLocked():
        continue
    if t.Type() == pcbnew.PCB_VIA_T:
        # KiCad takes an imported via's drill from the net class, not the padstack
        # Freerouting chose, which can leave a thin ring. Freerouting placed the
        # via for its pad size, so shrink the drill (growing the pad would break
        # the clearance it routed to).
        if (t.GetWidth(pcbnew.F_Cu) - t.GetDrillValue()) // 2 < ds.m_ViasMinAnnularWidth:
            t.SetDrill(max(ds.m_MinThroughDrill, t.GetWidth(pcbnew.F_Cu) - 2 * ds.m_ViasMinAnnularWidth))
            vias += 1
    elif t.GetWidth() < ds.m_TrackMinWidth:
        # Freerouting necks tracks down where they meet fine-pitch pads.
        t.SetWidth(ds.m_TrackMinWidth)
        narrow += 1
pcbnew.SaveBoard(sys.argv[1], b)
print("FIXED", narrow, vias)
"""


def settings(layers: list[str], planes: set[str], v) -> str:
    via, f_dir, in_dir, b_dir, against, ripup = v
    rules = []
    for name in layers:
        d = f_dir if name == "F.Cu" else b_dir if name == "B.Cu" else in_dir
        rules.append(f"      (layer_rule {name} (active {'off' if name in planes else 'on'}) "
                     f"(preferred_direction {d}) (preferred_direction_trace_costs 1.0) "
                     f"(against_preferred_direction_trace_costs {against}))\n")
    return ("    (autoroute_settings\n      (fanout on) (autoroute on) (postroute on) (vias on)"
            f" (via_costs {via}) (plane_via_costs 5) (start_ripup_costs {ripup}) (start_pass_no 1)\n"
            + "".join(rules) + "    )\n")


def unrouted(log: str, plane_nets: set[str]) -> tuple[int, int]:
    """(signal connections, all connections) Freerouting left unrouted."""
    block = log.split("could not be routed", 1)[1] if "could not be routed" in log else ""
    sig = tot = 0
    for m in re.finditer(r"Net '([^']*)' \((\d+) unrouted", block):
        n = int(m.group(2))
        tot += n
        if m.group(1) not in plane_nets:
            sig += n
    return sig, tot


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("project", type=Path)
    ap.add_argument("--planes", default="", help="comma-separated plane layers (no routing), e.g. In1.Cu")
    ap.add_argument("--variants", type=int, default=len(VARIANTS))
    ap.add_argument("--passes", type=int, default=100)
    ap.add_argument("--threads", type=int, default=1, help="Freerouting threads (capped at half the cores)")
    ap.add_argument("--heap", type=int, default=4, help="Java heap, GB")
    ap.add_argument("--timeout", type=int, default=600, help="seconds per Freerouting run")
    ap.add_argument("--clearance-margin", type=float, default=0.01,
                    help="mm added to every clearance Freerouting routes to: it lands a few microns "
                         "under the rule, which DRC then reports (0.094 against 0.1)")
    args = ap.parse_args()

    threads = max(1, min(args.threads, (os.cpu_count() or 2) // 2))
    _, _, pcb = find_project(args.project)
    out = pcb.parent / "route"
    out.mkdir(exist_ok=True)
    dsn = out / f"{pcb.stem}.dsn"
    m = re.search(r"EXPORTED (\d+) (\w+)", pcbnew_step(EXPORT, str(pcb), str(dsn)))
    if not m or m.group(2) != "True":
        sys.exit("Specctra export failed")
    removed = m.group(1)
    text = dsn.read_text()
    layers = re.findall(r"\(layer (\S+)\n\s+\(type \w+\)", text)
    planes = {p for p in args.planes.split(",") if p}
    for p in planes:
        text, n = re.subn(rf"(\(layer {re.escape(p)}\n\s+\(type )signal\)", r"\1power)", text)
        if not n:
            sys.exit(f"{p} is not a layer in the DSN ({layers})")
    plane_nets = set(re.findall(r"\(plane (\S+) \(polygon", text))
    if args.clearance_margin and "(unit um)" in text:
        add = args.clearance_margin * 1000
        text = re.sub(r"\(clearance ([\d.]+)(\)| \(type (?!smd_smd)\w+\)\))",
                      lambda m: f"(clearance {float(m.group(1)) + add:g}{m.group(2)}", text)

    def run(i):
        v = VARIANTS[i % len(VARIANTS)]
        d = out / f"v{i}.dsn"
        d.write_text(text.replace("    (boundary", settings(layers, planes, v) + "    (boundary", 1))
        ses = out / f"v{i}.ses"
        ses.unlink(missing_ok=True)
        java = ["java", f"-Xmx{args.heap}g", "-jar", str(jar()), "-de", str(d), "-do", str(ses),
                "-mp", str(args.passes), "-mt", str(threads)]
        gov = ["nice", "-n", "15", "timeout", str(args.timeout)]
        if shutil.which("systemd-run"):
            gov = ["systemd-run", "--user", "--scope", "--quiet", "-p", f"MemoryMax={args.heap + 2}G",
                   "-p", f"CPUQuota={threads * 100}%"] + gov
        r = subprocess.run(gov + java, capture_output=True, text=True)
        log = r.stdout + r.stderr
        (out / f"v{i}.log").write_text(log)
        sig, tot = unrouted(log, plane_nets) if ses.exists() else (10**6, 10**6)
        return {"variant": i, "settings": dict(zip(
            ["via_cost", "F.Cu", "inner", "B.Cu", "against_cost", "ripup_cost"], v)),
            "signal_unrouted": sig, "all_unrouted": tot, "ses": str(ses)}

    print(f"autoroute: {pcb.name}: removed {removed} unlocked tracks; planes {sorted(planes) or 'none'}; "
          f"{threads} thread(s), nice 15, {args.timeout} s per run", flush=True)
    results = []
    for i in range(min(args.variants, len(VARIANTS))):
        res = run(i)
        results.append(res)
        print(f"  v{i}: {res['signal_unrouted']} signal / {res['all_unrouted']} total unrouted "
              f"(Freerouting's count)", flush=True)
        if res["signal_unrouted"] == 0:
            break
    best = min(results, key=lambda r: (r["signal_unrouted"], r["all_unrouted"]))
    fixed = re.search(r"FIXED (\d+) (\d+)", pcbnew_step(IMPORT, str(pcb), best["ses"]))
    (out / "autoroute.json").write_text(json.dumps({"best": best, "variants": results}, indent=2) + "\n")
    print(f"  imported v{best['variant']} ({best['signal_unrouted']} signal connections left)")
    if fixed:
        print(f"  widened {fixed.group(1)} necked-down tracks to the board minimum; "
              f"{fixed.group(2)} via drills reduced to keep the minimum ring")
    print("  run DRC after the pours are added and filled: Freerouting can't see zone connections")
    return 0 if best["signal_unrouted"] == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
