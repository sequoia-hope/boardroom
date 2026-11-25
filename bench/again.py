#!/usr/bin/env python3
"""One build of the new router on each board of a set several times over, and which boards came
out differently: a run of `boardroom route` is to give the same board, byte for byte, every time
it is asked the same thing. Run it after a change to how the router's threads work together, or
when two runs of one build are seen to differ.

    again.py SET [--bin PATH] [--times N] [--run NAME] [--cap SECONDS] [--only id,...|@FILE] [--route-args WORDS]

SET is as for tier.py (`clean`, a tier, `look`, `ours`). --bin is the build (default: the release
build); it is routed --times times (default 3) on every board, all through one pool, a third of
$RESERVED_CORES at once (book them with `reserve --here`), so the runs of a board fall under
different loads. Each run is under tier.py's memory cap and is stopped after --cap seconds; a
board stopped there or failed in any of its runs is counted apart: it says nothing either way.

Writes LAB/again/NAME/ID/K/ (the router's files of run K) and prints each board whose runs
differ in `routed.kicad_pcb` or `wires.json`, with what each run came to, and how many boards
were the same. Exits 1 if any differ.
"""
import argparse
import concurrent.futures
import datetime
import hashlib
import json
import os
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import paths
import prototype
import tier

LAB = paths.lab()
FILES = ("routed.kicad_pcb", "wires.json")


def route(binary, board, out, cap, env, more):
    """One run of one board: its counts and what its files hash to, or why there are none."""
    out.mkdir(parents=True, exist_ok=True)
    for old in ("report.json", *FILES):
        (out / old).unlink(missing_ok=True)
    cmd = ["systemd-run", "--user", "--scope", "-q", "-p", f"MemoryMax={tier.MEMORY}", "-p", "MemorySwapMax=0", "timeout", str(cap), str(binary), "route", str(board), "--out", str(out), *more]
    r = subprocess.run(cmd, capture_output=True, text=True, env=env)
    (out / "route.log").write_text(r.stderr)
    if r.returncode == 124:
        return {"stopped": cap}
    if r.returncode != 0 or not all((out / f).exists() for f in ("report.json", *FILES)):
        return {"failed": (r.stderr.strip() or f"exit {r.returncode}")[-200:]}
    c = json.loads((out / "report.json").read_text())
    return {"open": c["open"], "left": c["left"], "undrawn": c["undrawn"], "strands": c["strands"], "seconds": round(c["seconds"]["draw"], 1),
            **{f: hashlib.sha256((out / f).read_bytes()).hexdigest()[:12] for f in FILES}}


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("set", help="clean, a tier (smoke, focus, guard, gate), look, or ours")
    ap.add_argument("--bin", default=str(paths.REPO / "target" / "release" / "boardroom"), help="the build to run")
    ap.add_argument("--times", type=int, default=3, help="how many times each board is routed")
    ap.add_argument("--run", help="the run's name under LAB/again/ (default: the set and today's date)")
    ap.add_argument("--cap", type=int, default=400, help="seconds a run may take")
    ap.add_argument("--only", help="these boards of the set only: ids with commas between, or @FILE with one id a line")
    ap.add_argument("--route-args", default="", metavar="WORDS", help="more words for `boardroom route`, the same for every run")
    a = ap.parse_args()
    if not Path(a.bin).is_file():
        sys.exit(f"again: --bin must be a built boardroom: {a.bin}")
    if a.times < 2:
        sys.exit("again: --times must be 2 or more")
    only = None
    if a.only:
        only = {line.strip() for line in Path(a.only[1:]).read_text().splitlines() if line.strip()} if a.only.startswith("@") else set(a.only.split(","))
    todo = tier.boards(a.set, only)
    out = LAB / "again" / (a.run or f"{a.set}-{datetime.date.today().isoformat()}")
    env = dict(os.environ)
    if prototype.TEXT.exists():
        env.setdefault("ULTRAROUTE_TEXT_HELPER", str(prototype.TEXT))

    def one(job):
        bid, board, k = job
        return bid, k, route(Path(a.bin).resolve(), board, out / bid / str(k), a.cap, env, a.route_args.split())

    runs = {}
    workers = max(1, int(os.environ.get("RESERVED_CORES", "3")) // 3)
    with concurrent.futures.ThreadPoolExecutor(workers) as pool:
        # Run by run, not board by board: a board's runs are apart in time, under other loads.
        for bid, k, row in pool.map(one, [(bid, board, k) for k in range(a.times) for bid, board in todo]):
            runs.setdefault(bid, {})[k] = row
    same, differ, unknown = [], [], []
    for bid, _ in todo:
        got = [runs[bid][k] for k in range(a.times)]
        if any("open" not in g for g in got):
            unknown.append(bid)
        elif len({tuple(g[f] for f in FILES) for g in got}) == 1:
            same.append(bid)
        else:
            differ.append(bid)
            print(f"differs: {bid}")
            for k, g in enumerate(got):
                print(f"    run {k}: {g['open']} open, {g['left']} left, {g['undrawn']} undrawn, {g['strands']} strands, {g['seconds']} s, board {g[FILES[0]]}")
    print(f"{a.set}: {len(todo)} boards, each routed {a.times} times by {a.bin}; {out}")
    print(f"the same every time: {len(same)}; differing: {len(differ)}" + (f"; stopped or failed in some run, so not told: {len(unknown)} ({', '.join(unknown[:8])}{' ...' if len(unknown) > 8 else ''})" if unknown else ""))
    sys.exit(1 if differ else 0)


if __name__ == "__main__":
    main()
