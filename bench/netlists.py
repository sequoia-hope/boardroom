#!/usr/bin/env python3
"""Which corpus boards have a netlist that is theirs?

    netlists.py [--tier T | --only ID,...]

For each board with files fetched by `pcbench_schematics.py fetch` (LAB/pcbench/schematics/ID/),
compares every shipped `.net` with the designer's board (`truth.py prepare`) pad for pad
(`examine::netmatch`, built with `cargo build --release -p examine --example netmatch`) and
keeps the best. Writes LAB/pcbench/netlists.json: per board the netlist chosen, the share of
its nets that are the same pads on the board, and whether that reaches the bar. A netlist that
is not the board's (another revision, another variant) is skipped by whatever reads this.
"""

import argparse
import json
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import paths
import truth

NETMATCH = paths.REPO / "target" / "release" / "examples" / "netmatch"


def one(b):
    lab = paths.lab() / "pcbench"
    board = lab / "truth" / b / "designer.kicad_pcb"
    nets = sorted((lab / "schematics" / b).rglob("*.net")) if (lab / "schematics" / b).is_dir() else []
    row = {"id": b, "netlists": len(nets)}
    if not board.exists():
        return row | {"error": "no designer board (truth.py prepare)"}
    best = None
    for n in nets:
        r = subprocess.run([str(NETMATCH), str(n), str(board)], stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=300)
        if r.returncode != 0:
            row.setdefault("unread", []).append(f"{n.name}: {(r.stderr.strip().splitlines() or ['?'])[-1][:200]}")
            continue
        m = json.loads(r.stdout)
        if best is None or m["share"] > best[1]["share"]:
            best = (n, m)
    if best:
        n, m = best
        row |= {"netlist": str(n.relative_to(lab / "schematics" / b)), "nets": m["nets"], "nets_same": m["nets_same"], "share": round(m["share"], 4),
                "matches": m["matches"], "parts_both": m["parts_both"], "parts_only_on_board": len(m["parts_only_on_board"]),
                "parts_only_in_netlist": len(m["parts_only_in_netlist"]), "apart": m["nets_apart"][:4]}
    return row


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--tier")
    ap.add_argument("--only")
    a = ap.parse_args()
    if not NETMATCH.exists():
        sys.exit(f"netlists: build {NETMATCH.relative_to(paths.REPO)} first: cargo build --release -p examine --example netmatch")
    rows = [one(b) for b in truth.chosen(a)]
    out = paths.lab() / "pcbench" / "netlists.json"
    out.write_text(json.dumps(rows, indent=1))
    have = [r for r in rows if r["netlists"]]
    read = [r for r in have if "share" in r]
    ok = [r for r in read if r["matches"]]
    near = [r for r in read if not r["matches"] and r["share"] >= 0.8]
    print(f"{len(rows)} boards: {len(have)} ship a netlist, {len(read)} could be read, {len(ok)} are the board's "
          f"(at least 95% of nets the same pads), {len(near)} more are close (80% to 95%), {len(read) - len(ok) - len(near)} are not")
    print(f"wrote {out}")


if __name__ == "__main__":
    main()
