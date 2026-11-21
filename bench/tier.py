#!/usr/bin/env python3
"""The new router (`boardroom route`) on a set of boards, one build or several side by side, with
KiCad judging each routed board: the yardstick for a change to `plan` or `wire`.

    tier.py SET [--bin TAG=PATH ...] [--run NAME] [--cap SECONDS] [--only id,...|@FILE] [--against RUN]
                [--no-kicad] [--again] [--rows]

SET is a tier of pcbench_tiers.json (smoke, focus, guard, gate: the designers' boards stripped,
LAB/pcbench/truth/ID/bare.kicad_pcb), or `look` or `ours` (prototype.py's boards). Each --bin is
a build to run, named by its tag; with none it is `head`, the release build
(`cargo build --release -p boardroom -p kicad-text`). To try a change beside the build it
changes, build each into its own target directory and copy the binary to its own name: a build
a run is using must never be built over.

All builds' boards go through one pool, turn about, so load falls on each alike; a board is
routed several ways at once, so the pool is a third of $RESERVED_CORES (book them with
`reserve --here`: the lab is not in git). Each board runs under a memory cap and is stopped
after --cap seconds. With WIRE_FIRST set the router stops at its first plan and drawing: left
plus undrawn there is the steady measure of a planner or drawer change, where whole runs differ
by a few connections from the laying again alone.

Writes LAB/tier/NAME/TAG/ID/ (the router's files and KiCad's reports) and
LAB/tier/NAME/results.json; a board already there is not run again without --again, so a run
cut short is taken up where it stopped, and a second build can be added to a finished run.

Prints, for each build: boards whole and connections open by our count and by KiCad's (whole
there: nothing unconnected and no error the routed copper adds), the same for the boards the
designer routed with nothing open and no error of their own (the others cannot all be made
whole inside the rules: what is open there is a floor, not the router's doing), track length,
vias and time; and for two builds, each board on which they differ.

--against RUN (a tier only) sets a pcbench run of the same designers' boards beside each build,
the frozen old router's as a rule (`pcbench.py run --source truth`), both held to pcbench's
verdict: no more unconnected than the designer's own board has, and no error the routed copper
adds. It names the boards one router finishes and the other does not. The old router's run is
read as it was stored, not run again, so the two had different clocks: say so with the figures.
"""
import argparse
import concurrent.futures
import datetime
import json
import os
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import paths
import pcbench
import prototype

LAB = paths.lab()
MEMORY = "8G"


def boards(name, only):
    """(id, board file) of a set, in the order they are run."""
    if name == "look":
        found = [(bid, LAB / board) for bid, _, board in prototype.BOARDS]
    elif name == "ours":
        found = [(bid, Path(board)) for bid, _, board in prototype.OURS]
    else:
        found = [(bid, LAB / "pcbench" / "truth" / bid / "bare.kicad_pcb") for bid in pcbench.tier_ids(name)]
    if only:
        unknown = only - {bid for bid, _ in found}
        if unknown:
            sys.exit(f"tier: not in {name}: {', '.join(sorted(unknown))}")
        found = [b for b in found if b[0] in only]
    missing = [bid for bid, board in found if not board.exists()]
    if missing and name in ("look", "ours"):
        sys.exit("tier: no such board: " + ", ".join(missing))
    # A tier may name one of our own patched boards (pcbench_variants.json), which has no
    # designer's board to strip: it is left out, and said to be.
    if missing:
        print(f"tier: left out, having no stripped board in LAB/pcbench/truth: {', '.join(missing)}", file=sys.stderr)
    return [b for b in found if b[0] not in missing]


def clean(board):
    """Whether the designer's own board, beside this stripped one, has nothing unconnected and no
    error of its own; None where there is no designer's board to ask (our own boards)."""
    judged = board.parent / "designer.json"
    if not judged.exists():
        return None
    d = json.loads(judged.read_text())
    return d.get("unconnected") == 0 and not d.get("new_errors")


def route(binary, board, out, cap, env, judge):
    """One board routed by one build, and judged: the row kept in results.json."""
    out.mkdir(parents=True, exist_ok=True)
    for old in ("report.json", "routed.kicad_pcb"):
        (out / old).unlink(missing_ok=True)
    cmd = ["systemd-run", "--user", "--scope", "-q", "-p", f"MemoryMax={MEMORY}", "-p", "MemorySwapMax=0", "timeout", str(cap), str(binary), "route", str(board), "--out", str(out)]
    r = subprocess.run(cmd, capture_output=True, text=True, env=env)
    (out / "route.log").write_text(r.stderr)
    if r.returncode == 124:
        return {"stopped": cap}
    report = out / "report.json"
    if r.returncode != 0 or not report.exists():
        return {"failed": (r.stderr.strip() or f"exit {r.returncode}")[-300:]}
    c = json.loads(report.read_text())
    mm, vias = pcbench.lengths(out / "routed.kicad_pcb")
    row = {"open": c["open"], "faults": sum(c["faults"].values()), "left": c["left"], "undrawn": c["undrawn"], "strands": c["strands"],
           "vias": vias, "track_mm": round(mm, 1), "seconds": round(c["seconds"]["draw"], 1)}
    if judge:
        k = prototype.kicad(board, out)
        if isinstance(k, str):
            row["kicad_failed"] = k[-300:]
        else:
            row["kicad_open"], row["kicad_new"] = k[0], k[2]
    return row


def whole(row):
    return "open" in row and row["open"] == 0 and not row["faults"]


def kicad_whole(row):
    return row.get("kicad_open") == 0 and not row.get("kicad_new")


def line(tag, rows, of):
    """One build's totals over `rows` ({id: row}), said of `of` (which boards these are)."""
    done = {b: r for b, r in rows.items() if "open" in r}
    text = f"{tag:12} {of}: {sum(map(whole, done.values()))} of {len(rows)} whole, {sum(r['open'] for r in done.values())} open"
    judged = [r for r in done.values() if "kicad_open" in r]
    if judged:
        new = sum(sum(r["kicad_new"].values()) for r in judged)
        text += f"; KiCad: {sum(map(kicad_whole, judged))} whole, {sum(r['kicad_open'] for r in judged)} unconnected, {new} new errors"
        if len(judged) < len(done):
            text += f" ({len(done) - len(judged)} it could not judge)"
    stopped = sum("stopped" in r for r in rows.values())
    failed = sum("failed" in r for r in rows.values())
    return text + (f"; {stopped} stopped at the cap" if stopped else "") + (f"; {failed} FAILED" if failed else "")


def passes(row, floor):
    """pcbench's verdict on a board of ours: KiCad finds no more unconnected than the designer's
    own board has, and no error the routed copper adds."""
    return floor is not None and "kicad_open" in row and row["kicad_open"] <= floor and not row["kicad_new"]


def against(run, tag, mine, ids):
    """A build's boards beside a stored pcbench run's, both by pcbench's verdict."""
    theirs = pcbench.load(run)[0]
    both = [b for b in ids if b in mine and b in theirs]
    ours = {b: passes(mine[b], pcbench.floor_of("truth", b)) for b in both}
    old = {b: bool(theirs[b].get("success")) for b in both}
    lost = [b for b in both if old[b] and not ours[b]]
    won = [b for b in both if ours[b] and not old[b]]
    print(f"{tag:12} beside {run} on {len(both)} boards: {sum(ours.values())} pass against its {sum(old.values())}; "
          f"{sum(ours[b] and old[b] for b in both)} both, {len(won)} only {tag}, {len(lost)} only {run}, {sum(not ours[b] and not old[b] for b in both)} neither")
    seen = lambda r: "stopped" if "stopped" in r else "failed" if "failed" in r else "not judged" if "kicad_open" not in r else f"{r['kicad_open']} unconnected" + (", new " + ",".join(sorted(r["kicad_new"])) if r["kicad_new"] else "")
    for name, some in ((f"only {run}", lost), (f"only {tag}", won)):
        for b in some:
            print(f"  {name}: {b[:44]:44} {tag} {seen(mine[b])}; {run} {theirs[b].get('unconnected')} unconnected" + (", new " + ",".join(sorted(theirs[b].get("new_errors") or {})) if theirs[b].get("new_errors") else ""))


def told(row):
    """A board's row in a few words."""
    if "stopped" in row:
        return f"stopped at {row['stopped']} s"
    if "failed" in row:
        return "FAILED: " + row["failed"].splitlines()[-1][:80]
    text = f"open {row['open']:3}"
    if row["faults"]:
        text += f" faults {row['faults']}"
    if "kicad_open" in row:
        text += f"  KiCad {row['kicad_open']:3}" + (" +" + ",".join(f"{n} {t}" for t, n in sorted(row["kicad_new"].items())) if row["kicad_new"] else "")
    elif "kicad_failed" in row:
        text += "  KiCad could not judge"
    return text + f"  left {row['left']:3} undrawn {row['undrawn']:3} vias {row['vias']:4} {row['track_mm']:8.1f} mm {row['seconds']:7.1f} s"


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("set", help="a tier (smoke, focus, guard, gate), or look, or ours")
    ap.add_argument("--bin", action="append", default=[], metavar="TAG=PATH", help="a build to run; several are run side by side")
    ap.add_argument("--run", help="the run's name under LAB/tier/ (default: the set and today's date)")
    ap.add_argument("--cap", type=int, default=400, help="seconds a board may take")
    ap.add_argument("--only", help="these boards of the set only: ids with commas between, or @FILE with one id a line")
    ap.add_argument("--against", metavar="RUN", help="a pcbench run of the same designers' boards to set beside each build")
    ap.add_argument("--no-kicad", action="store_true", help="our count only")
    ap.add_argument("--again", action="store_true", help="run boards the run has already")
    ap.add_argument("--rows", action="store_true", help="print every board, not only those the builds differ on")
    a = ap.parse_args()

    builds = {}
    for b in a.bin or [f"head={paths.REPO / 'target' / 'release' / 'boardroom'}"]:
        tag, _, path = b.partition("=")
        if not path or not Path(path).is_file():
            sys.exit(f"tier: --bin TAG=PATH, and PATH must be a built boardroom: {b}")
        builds[tag] = Path(path).resolve()
    only = None
    if a.only:
        # Board ids may have spaces in them: a file of them is easier to pass than a quoted list.
        only = {line.strip() for line in Path(a.only[1:]).read_text().splitlines() if line.strip()} if a.only.startswith("@") else set(a.only.split(","))
    todo = boards(a.set, only)
    if a.against and a.set in ("look", "ours"):
        sys.exit("tier: --against needs a tier: a pcbench run is of the designers' boards")
    first = "WIRE_FIRST" in os.environ
    name = a.run or f"{a.set}-{datetime.date.today().isoformat()}" + ("-first" if first else "")
    out = LAB / "tier" / name
    out.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ)
    if prototype.TEXT.exists():
        env.setdefault("ULTRAROUTE_TEXT_HELPER", str(prototype.TEXT))
    else:
        print("tier: no kicad-text built: copper text is taken as boxes (cargo build --release -p kicad-text)", file=sys.stderr)

    store = out / "results.json"
    results = json.loads(store.read_text()) if store.exists() else {}
    if results.get("first", first) != first:
        sys.exit(f"tier: {name} was run {'with' if results['first'] else 'without'} WIRE_FIRST; give this run another --run name")
    results.update(set=a.set, first=first)
    rows = results.setdefault("rows", {})
    jobs = [(tag, bid, board) for bid, board in todo for tag in builds if a.again or bid not in rows.get(tag, {})]

    def one(job):
        tag, bid, board = job
        return tag, bid, route(builds[tag], board, out / tag / bid, a.cap, env, not (a.no_kicad or first))

    workers = max(1, int(os.environ.get("RESERVED_CORES", "3")) // 3)
    with concurrent.futures.ThreadPoolExecutor(workers) as pool:
        for tag, bid, row in pool.map(one, jobs):
            rows.setdefault(tag, {})[bid] = row
            store.write_text(json.dumps(results, indent=1))
            print(f"{tag:12} {bid[:44]:44} {told(row)}", file=sys.stderr, flush=True)

    ids = [bid for bid, _ in todo]
    fair = [bid for bid, board in todo if clean(board)]
    print(f"{a.set}: {len(ids)} boards" + (f", {len(fair)} of them routed by their designer with nothing open and no error" if fair else "")
          + (", first plan and drawing only (WIRE_FIRST)" if first else "") + f"; {out}")
    for tag in builds:
        mine = {bid: rows[tag][bid] for bid in ids if bid in rows.get(tag, {})}
        print(line(tag, mine, "all"))
        if fair:
            print(line(tag, {b: r for b, r in mine.items() if b in fair}, "designer-clean"))
        done = [r for r in mine.values() if "open" in r]
        print(f"{tag:12} left {sum(r['left'] for r in done)}, undrawn {sum(r['undrawn'] for r in done)}, vias {sum(r['vias'] for r in done)}, "
              f"track {sum(r['track_mm'] for r in done) / 1000:.2f} m, {sum(r['seconds'] for r in done):.0f} s on the {len(done)} boards finished")
        if a.against:
            against(a.against, tag, mine, ids)
    tags = list(builds)
    key = lambda r: (r.get("open"), r.get("faults"), r.get("kicad_open"), r.get("kicad_new"), "stopped" in r, "failed" in r)
    for bid in ids:
        have = [rows.get(tag, {}).get(bid) for tag in tags]
        if None in have:
            continue
        if a.rows or len({json.dumps(key(r), sort_keys=True) for r in have}) > 1:
            for tag, row in zip(tags, have):
                print(f"  {bid[:40]:40} {tag:12} {told(row)}")


if __name__ == "__main__":
    main()
