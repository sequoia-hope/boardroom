#!/usr/bin/env python3
"""gate_inputs.py -- record and check the inputs judged by each phase gate.

    python3 gate_inputs.py record N [--project DIR]
    python3 gate_inputs.py check [N] [--project DIR]

`record N` writes `design/gates/phaseN.inputs.json` beside `phaseN.md`.
`check [N]` recomputes the hashes for gate N (or every gate that has an
`.inputs.json` record when N is omitted), prints what changed, and exits 1 if
anything differs (0 otherwise). A gate without a record is never auto-stale.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
import tomllib
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
SKILL_DIR = HERE.parent


# ---------------------------------------------------------------- gate table

@dataclass(frozen=True)
class GateSpec:
    gate: int
    name: str
    design_files: tuple[str, ...] = ()
    design_dirs: tuple[str, ...] = ()     # non-hidden files under each dir (recursive)
    sch: bool = False                     # board .kicad_sch files
    pcb: str | None = None                # None | "edge_cuts" | "placement" | "copper" | "full"
    pro: bool = False                     # board .kicad_pro
    dru: bool = False                     # board .kicad_dru
    fab: bool = False                     # non-hidden files under hw/<board>/<var>/fab/
    interfaces: bool = False              # design/links.toml + interface/<board>/board-shape.json


# Which inputs each gate (0..10) judges. Documented in SKILL.md ("Gate inputs").
#
# Later phases edit the same .kicad_pcb, so a gate hashes only the part of the board
# it judged: gate 4 the outline, gate 6 the placement, gates 7-8 the copper (which
# includes the placement). Routing then never stales placement, and silkscreen never
# stales routing. Only gates 9 and 10 hash the whole file. design/checks.toml is left
# out: later phases add their own sections to it.
GATE_SPECS: dict[int, GateSpec] = {
    0: GateSpec(
        gate=0,
        name="Design intent",
        design_files=("design/intent.md",),
    ),
    1: GateSpec(
        gate=1,
        name="Architecture and parts",
        design_files=("design/intent.md", "design/architecture.md", "design/datasheets/sources.csv"),
        design_dirs=("design/parts",),
    ),
    2: GateSpec(
        gate=2,
        name="Schematic",
        design_dirs=("design/parts",),
        sch=True,
    ),
    3: GateSpec(
        gate=3,
        name="Net classification",
        design_files=("design/nets.csv",),
        sch=True,
    ),
    4: GateSpec(
        gate=4,
        name="Mechanical and layout brief",
        design_files=("design/brief.md",),
        pcb="edge_cuts",
        interfaces=True,
    ),
    5: GateSpec(
        gate=5,
        name="Stackup and rules",
        design_files=("design/stackup.md", "design/nets.csv"),
        pro=True,
        dru=True,
    ),
    6: GateSpec(
        gate=6,
        name="Placement",
        design_files=("design/brief.md",),
        sch=True,
        pcb="placement",
        interfaces=True,
    ),
    7: GateSpec(
        gate=7,
        name="Routing",
        design_files=("design/nets.csv",),
        sch=True,
        pcb="copper",
        pro=True,
        dru=True,
    ),
    8: GateSpec(
        gate=8,
        name="Simulation and SI",
        design_files=("design/stackup.md", "design/nets.csv"),
        design_dirs=("design/sim",),
        pcb="copper",
    ),
    9: GateSpec(
        gate=9,
        name="Silkscreen and art",
        design_files=("design/brief.md",),
        pcb="full",
        pro=True,
    ),
    10: GateSpec(
        gate=10,
        name="Release",
        design_files=("design/intent.md", "design/nets.csv", "design/stackup.md", "design/bringup.md"),
        sch=True,
        pcb="full",
        pro=True,
        dru=True,
        fab=True,
        interfaces=True,
    ),
}


# ------------------------------------- .kicad_pcb aspects (gates 4, 6, 7, 8)
#
# A record keys an aspect hash as `<path>#<aspect>` (e.g.
# `hw/a/b/b.kicad_pcb#placement`); a key without `#` is a sha256 of the bytes.
# Shared verbatim with `app/server/src/gates.rs` (`pcb_aspect_hash_text`):
#
# 1. Tokenize the `.kicad_pcb` UTF-8 text into S-expression lists and string
#    atoms. Whitespace (`' '`, `'\t'`, `'\n'`, `'\r'`) separates tokens outside
#    quotes; `"..."` unescapes `\n` -> LF, `\r` -> CR, `\t` -> TAB, and `\<c>`
#    -> `<c>`; unquoted atoms keep their raw text.
# 2. From the root `(kicad_pcb ...)` list, collect:
#    a) Every direct child list whose head atom starts with `"gr_"` and that
#        sits on `"Edge.Cuts"`, i.e. has a direct child `(layer "Edge.Cuts")`
#       (or `(layers ... "Edge.Cuts" ...)`).
#    b) Inside every direct `(footprint ...)` or `(module ...)` child, for each
#       direct child list whose head starts with `"fp_"` and sits on
#       `"Edge.Cuts"`, emit `["footprint_edge", <at_or_empty>, <fp_item>]`
#       where `<at_or_empty>` is the footprint's first direct `(at ...)` child
#       or `["at"]`. Footprints without `Edge.Cuts` items are ignored.
#    That set is the `edge_cuts` aspect. The `placement` aspect is instead, for
#    every direct `(footprint ...)`/`(module ...)` child, the list
#    `["footprint", <lib id or "">, <reference or "">, <first (at ...) or ["at"]>,
#    <first (layer ...) or ["layer"]>]`; the reference is the value of
#    `(property "Reference" X)` or `(fp_text reference X)`. The `copper` aspect is
#    the placement items plus every direct child whose head is `segment`, `arc`,
#    `via` or `zone`, and every `gr_*` child whose `(layer X)` ends in `.Cu`;
#    inside those, sublists headed `filled_polygon` or `fill_segments` are
#    dropped too (a zone refill doesn't change copper intent).
# 3. Canonicalize each collected item:
#    - Recursively drop sublists whose head atom is `"uuid"` or `"tstamp"`.
#    - Format an atom bare when it is non-empty and every character is in
#      `[A-Za-z0-9_.:+/-]`; otherwise quote it with `"..."` escaping `\`, `"`,
#      `\n`, `\r`, `\t`.
#    - Format a list as `(` + single-space-joined children + `)`.
# 4. Sort the canonicalized item strings by UTF-8 byte / codepoint order, join
#    them with `"\n"`, and return `"sha256:" + sha256(utf8_bytes).hexdigest()`.

_BARE_ATOM = re.compile(r'^[A-Za-z0-9_.:+/-]+\Z')


def _parse_sexpr_text(text: str) -> list | str:
    stack: list[list] = [[]]
    i = 0
    n = len(text)
    while i < n:
        ch = text[i]
        if ch in " \t\n\r":
            i += 1
            continue
        if ch == "(":
            stack.append([])
            i += 1
        elif ch == ")":
            if len(stack) == 1:
                raise ValueError(f"unbalanced ')' at byte/char offset {i}")
            done = stack.pop()
            stack[-1].append(done)
            i += 1
        elif ch == '"':
            i += 1
            buf: list[str] = []
            while i < n:
                c = text[i]
                if c == '"':
                    i += 1
                    break
                if c == "\\":
                    i += 1
                    if i >= n:
                        raise ValueError("unterminated escape in quoted string")
                    esc = text[i]
                    if esc == "n":
                        buf.append("\n")
                    elif esc == "r":
                        buf.append("\r")
                    elif esc == "t":
                        buf.append("\t")
                    else:
                        buf.append(esc)
                    i += 1
                else:
                    buf.append(c)
                    i += 1
            else:
                raise ValueError("unterminated quoted string")
            stack[-1].append("".join(buf))
        else:
            start = i
            while i < n and text[i] not in ' \t\n\r()"':
                i += 1
            stack[-1].append(text[start:i])
    if len(stack) != 1:
        raise ValueError("unbalanced '(' in S-expression")
    if not stack[0]:
        raise ValueError("empty S-expression")
    return stack[0][0]


def _node_head(node) -> str | None:
    if isinstance(node, list) and node and isinstance(node[0], str):
        return node[0]
    return None


def _is_on_edge_cuts(node: list) -> bool:
    for c in node[1:]:
        h = _node_head(c)
        if h == "layer" and len(c) > 1 and isinstance(c[1], str) and c[1] == "Edge.Cuts":
            return True
        if h == "layers":
            for arg in c[1:]:
                if isinstance(arg, str) and arg in ("Edge.Cuts", "*.Cuts"):
                    return True
    return False


def _canon_atom(s: str) -> str:
    if s and _BARE_ATOM.fullmatch(s):
        return s
    out = ['"']
    for ch in s:
        if ch == "\\":
            out.append("\\\\")
        elif ch == '"':
            out.append('\\"')
        elif ch == "\n":
            out.append("\\n")
        elif ch == "\r":
            out.append("\\r")
        elif ch == "\t":
            out.append("\\t")
        else:
            out.append(ch)
    out.append('"')
    return "".join(out)


def _canon_sexpr(node) -> str:
    if isinstance(node, str):
        return _canon_atom(node)
    kept: list[str] = []
    for c in node:
        if isinstance(c, list) and _node_head(c) in ("uuid", "tstamp"):
            continue
        kept.append(_canon_sexpr(c))
    return "(" + " ".join(kept) + ")"


PCB_ASPECTS = ("edge_cuts", "placement", "copper")
ASPECT_WORDS = {"edge_cuts": "Edge.Cuts", "placement": "placement", "copper": "copper"}
_COPPER_HEADS = ("segment", "arc", "via", "zone")
_FILL_HEADS = ("filled_polygon", "fill_segments")


def _first_child(node: list, head: str, default: list) -> list:
    for sub in node[1:]:
        if _node_head(sub) == head:
            return sub
    return default


def _footprint_reference(fp: list) -> str:
    for sub in fp[1:]:
        h = _node_head(sub)
        if h == "property" and len(sub) > 2 and sub[1] == "Reference" and isinstance(sub[2], str):
            return sub[2]
        if h == "fp_text" and len(sub) > 2 and sub[1] == "reference" and isinstance(sub[2], str):
            return sub[2]
    return ""


def _on_copper_layer(node: list) -> bool:
    layer = _first_child(node, "layer", ["layer"])
    return len(layer) > 1 and isinstance(layer[1], str) and layer[1].endswith(".Cu")


def _drop_fills(node):
    if isinstance(node, str):
        return node
    return [_drop_fills(c) for c in node if _node_head(c) not in _FILL_HEADS]


def pcb_aspect_hash_text(text: str, aspect: str) -> str:
    if aspect not in PCB_ASPECTS:
        raise ValueError(f"unknown .kicad_pcb aspect {aspect!r}")
    tree = _parse_sexpr_text(text)
    if not isinstance(tree, list):
        raise ValueError("expected top-level S-expression list")
    items: list[str] = []
    for child in tree[1:]:
        if not isinstance(child, list):
            continue
        h = _node_head(child)
        if aspect == "edge_cuts":
            if h and h.startswith("gr_") and _is_on_edge_cuts(child):
                items.append(_canon_sexpr(child))
            elif h in ("footprint", "module"):
                at_node = _first_child(child, "at", ["at"])
                for sub in child[1:]:
                    sh = _node_head(sub)
                    if isinstance(sub, list) and sh and sh.startswith("fp_") and _is_on_edge_cuts(sub):
                        items.append(_canon_sexpr(["footprint_edge", at_node, sub]))
            continue
        if h in ("footprint", "module"):
            lib_id = child[1] if len(child) > 1 and isinstance(child[1], str) else ""
            items.append(_canon_sexpr([
                "footprint", lib_id, _footprint_reference(child),
                _first_child(child, "at", ["at"]), _first_child(child, "layer", ["layer"]),
            ]))
        elif aspect == "copper" and (h in _COPPER_HEADS or (h and h.startswith("gr_") and _on_copper_layer(child))):
            items.append(_canon_sexpr(_drop_fills(child)))
    items.sort()
    payload = "\n".join(items).encode("utf-8")
    return "sha256:" + hashlib.sha256(payload).hexdigest()


def edge_cuts_hash_text(text: str) -> str:
    return pcb_aspect_hash_text(text, "edge_cuts")


def edge_cuts_hash(pcb_path: Path) -> str:
    return edge_cuts_hash_text(pcb_path.read_text(encoding="utf-8"))


def pcb_aspect_hash(pcb_path: Path, aspect: str) -> str:
    return pcb_aspect_hash_text(pcb_path.read_text(encoding="utf-8"), aspect)


def split_key(key: str) -> tuple[str, str | None]:
    """`path#aspect` -> (path, aspect); a plain path -> (path, None)."""
    path, sep, aspect = key.partition("#")
    return (path, aspect) if sep else (path, None)


def hash_input(root: Path, key: str) -> str:
    path, aspect = split_key(key)
    full = root / path
    return pcb_aspect_hash(full, aspect) if aspect else file_sha256(full)


def file_sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        while chunk := f.read(65536):
            h.update(chunk)
    return "sha256:" + h.hexdigest()


# ------------------------------------------------------- project & board files

@dataclass
class BoardFiles:
    board: str
    variant_dir: Path | None
    pcb: Path | None
    schs: list[Path]
    pro: Path | None
    dru: Path | None


def resolve_project(arg: str | None) -> Path:
    start = Path(arg).resolve() if arg else Path.cwd().resolve()
    for p in [start, *start.parents]:
        if (p / "design").is_dir():
            return p
    return start


def _rel(root: Path, path: Path) -> str:
    return path.resolve().relative_to(root.resolve()).as_posix()


def _board_name_from_rel(rel_path: str, stem_fallback: str) -> str:
    parts = Path(rel_path).parts
    if len(parts) >= 3 and parts[0] == "hw":
        return parts[1]
    return stem_fallback


def _collect_schs(root_sch: Path) -> list[Path]:
    out: set[Path] = set()
    if root_sch.is_file():
        out.add(root_sch.resolve())
    if root_sch.parent.is_dir():
        for p in sorted(root_sch.parent.glob("*.kicad_sch")):
            if p.is_file() and not p.name.startswith("."):
                out.add(p.resolve())
    return sorted(out)


def discover_boards(root: Path) -> list[BoardFiles]:
    boards: list[BoardFiles] = []
    pcbview_toml = root / "pcbview.toml"
    if pcbview_toml.is_file():
        with pcbview_toml.open("rb") as f:
            cfg = tomllib.load(f)
        for entry in cfg.get("boards", []):
            # Frozen fabricated variants read from a git tag (`git = "..."`); skip them.
            if entry.get("git"):
                continue
            pcb_rel = entry.get("pcb")
            sch_rel = entry.get("sch")
            pcb = (root / pcb_rel).resolve() if pcb_rel else None
            sch_main = (root / sch_rel).resolve() if sch_rel else None
            ref_path = pcb or sch_main
            if ref_path is None:
                continue
            stem_path = ref_path.with_suffix("")
            pro_cand = stem_path.with_suffix(".kicad_pro")
            dru_cand = stem_path.with_suffix(".kicad_dru")
            if sch_main is None:
                sch_main = stem_path.with_suffix(".kicad_sch")
            if pcb is None:
                pcb_cand = stem_path.with_suffix(".kicad_pcb")
                if pcb_cand.is_file():
                    pcb = pcb_cand
            rel_str = pcb_rel or sch_rel or _rel(root, ref_path)
            bname = _board_name_from_rel(rel_str, stem_path.name)
            boards.append(BoardFiles(
                board=bname,
                variant_dir=ref_path.parent if ref_path.parent.is_dir() else None,
                pcb=pcb if (pcb and pcb.is_file()) else None,
                schs=_collect_schs(sch_main),
                pro=pro_cand if pro_cand.is_file() else None,
                dru=dru_cand if dru_cand.is_file() else None,
            ))

    if boards:
        return boards

    # Fallback when pcbview.toml has no active [[boards]] (e.g. template or fixture):
    # scan hw/<board>/<variant>/ matching server's project::find_boards / board_pcb.
    hw_dir = root / "hw"
    if not hw_dir.is_dir():
        return boards
    for bdir in sorted(p for p in hw_dir.iterdir() if p.is_dir() and not p.name.startswith(".")):
        for vdir in sorted(p for p in bdir.iterdir() if p.is_dir() and not p.name.startswith(".")):
            pros = sorted(vdir.glob("*.kicad_pro"))
            pcbs = sorted(vdir.glob("*.kicad_pcb"))
            schs_all = sorted(p.resolve() for p in vdir.glob("*.kicad_sch") if p.is_file() and not p.name.startswith("."))
            stem: Path | None = None
            if pros:
                stem = pros[0].with_suffix("")
            elif (vdir / f"{bdir.name}.kicad_pcb").is_file() or (vdir / f"{bdir.name}.kicad_sch").is_file():
                stem = vdir / bdir.name
            elif len(pcbs) == 1:
                stem = pcbs[0].with_suffix("")
            elif len(schs_all) == 1:
                stem = schs_all[0].with_suffix("")
            if stem is None:
                continue
            pcb_cand = stem.with_suffix(".kicad_pcb")
            pro_cand = stem.with_suffix(".kicad_pro")
            dru_cand = stem.with_suffix(".kicad_dru")
            boards.append(BoardFiles(
                board=bdir.name,
                variant_dir=vdir.resolve(),
                pcb=pcb_cand.resolve() if pcb_cand.is_file() else None,
                schs=schs_all,
                pro=pro_cand.resolve() if pro_cand.is_file() else None,
                dru=dru_cand.resolve() if dru_cand.is_file() else None,
            ))
    return boards


def _walk_regular_files(dir_path: Path) -> list[Path]:
    if not dir_path.is_dir():
        return []
    out: list[Path] = []
    for p in sorted(dir_path.rglob("*")):
        if not p.is_file():
            continue
        rel_parts = p.relative_to(dir_path).parts
        if any(part.startswith(".") for part in rel_parts):
            continue
        out.append(p.resolve())
    return out


# --------------------------------------------------- links & own_interface

def read_current_links(root: Path) -> dict[str, dict[str, str]]:
    links_toml = root / "design" / "links.toml"
    if not links_toml.is_file():
        return {}
    with links_toml.open("rb") as f:
        parsed = tomllib.load(f)
    out: dict[str, dict[str, str]] = {}
    for entry in parsed.get("links", []):
        link_id = entry.get("id", "")
        if not link_id:
            continue
        commit = (entry.get("resolved") or {}).get("commit", "")
        fallback_hash = entry.get("interface_hash", "")
        revision = ""
        geometry_hash = fallback_hash
        accepted_json = root / ".boardroom" / "links" / link_id / "board-shape.json"
        if accepted_json.is_file():
            data = json.loads(accepted_json.read_text(encoding="utf-8"))
            revision = str(data.get("revision", ""))
            geometry_hash = str(data.get("geometry_hash") or fallback_hash)
        out[link_id] = {
            "commit": str(commit),
            "revision": revision,
            "geometry_hash": geometry_hash,
        }
    return dict(sorted(out.items()))


def read_current_own_interfaces(root: Path, boards: list[BoardFiles]) -> dict[str, dict[str, str]]:
    names: set[str] = {b.board for b in boards}
    iface_root = root / "interface"
    if iface_root.is_dir():
        for d in iface_root.iterdir():
            if d.is_dir() and not d.name.startswith("."):
                names.add(d.name)
    out: dict[str, dict[str, str]] = {}
    for name in sorted(names):
        manifest = iface_root / name / "board-shape.json"
        if manifest.is_file():
            data = json.loads(manifest.read_text(encoding="utf-8"))
            out[name] = {
                "revision": str(data.get("revision", "")),
                "geometry_hash": str(data.get("geometry_hash", "")),
            }
    return out


def detect_skill_version(root: Path) -> str:
    for candidate in (
        root / ".claude" / "skills" / "pcb-design" / "VENDORED_FROM",
        SKILL_DIR / "VENDORED_FROM",
    ):
        if candidate.is_file():
            lines = [ln.strip() for ln in candidate.read_text(encoding="utf-8").splitlines() if ln.strip()]
            if lines:
                return lines[0]
    for git_args in (
        ["log", "-1", "--format=%H", "--", str(SKILL_DIR)],
        ["rev-parse", "HEAD"],
    ):
        try:
            r = subprocess.run(
                ["git", "-C", str(SKILL_DIR), *git_args],
                capture_output=True, text=True, timeout=5,
            )
            if r.returncode == 0 and r.stdout.strip():
                return r.stdout.strip()
        except (OSError, subprocess.SubprocessError):
            pass
    return "unknown"


# --------------------------------------------------- collect & check gate N

def collect_gate_inputs(root: Path, gate: int) -> tuple[dict[str, str], dict[str, dict[str, str]], dict[str, dict[str, str]]]:
    if gate not in GATE_SPECS:
        raise ValueError(f"unknown gate {gate} (expected 0..10)")
    spec = GATE_SPECS[gate]
    boards = discover_boards(root)

    files: dict[str, str] = {}

    for rel in spec.design_files:
        p = root / rel
        if p.is_file():
            files[_rel(root, p)] = file_sha256(p)

    for rel_dir in spec.design_dirs:
        for p in _walk_regular_files(root / rel_dir):
            files[_rel(root, p)] = file_sha256(p)

    for b in boards:
        if spec.sch:
            for s in b.schs:
                files[_rel(root, s)] = file_sha256(s)
        if spec.pcb in PCB_ASPECTS and b.pcb is not None:
            files[f"{_rel(root, b.pcb)}#{spec.pcb}"] = pcb_aspect_hash(b.pcb, spec.pcb)
        elif spec.pcb == "full" and b.pcb is not None:
            files[_rel(root, b.pcb)] = file_sha256(b.pcb)
        if spec.pro and b.pro is not None:
            files[_rel(root, b.pro)] = file_sha256(b.pro)
        if spec.dru and b.dru is not None:
            files[_rel(root, b.dru)] = file_sha256(b.dru)
        if spec.fab and b.variant_dir is not None:
            for p in _walk_regular_files(b.variant_dir / "fab"):
                files[_rel(root, p)] = file_sha256(p)

    sorted_files = dict(sorted(files.items()))
    links = read_current_links(root) if spec.interfaces else {}
    own_iface = read_current_own_interfaces(root, boards) if spec.interfaces else {}
    return sorted_files, links, own_iface


def _require_non_empty_for_record(root: Path, gate: int, files: dict, links: dict, own_iface: dict) -> None:
    if gate == 0 and "design/intent.md" not in files:
        sys.exit(f"gate 0 requires {root / 'design/intent.md'}")
    if gate == 2 and not any(k.endswith(".kicad_sch") for k in files):
        sys.exit("gate 2 requires at least one .kicad_sch file")
    if gate == 3 and "design/nets.csv" not in files:
        sys.exit(f"gate 3 requires {root / 'design/nets.csv'}")
    if gate in (6, 7, 8, 9, 10) and not any(split_key(k)[0].endswith(".kicad_pcb") for k in files):
        sys.exit(f"gate {gate} requires a .kicad_pcb file")
    if not files and not links and not own_iface:
        sys.exit(f"gate {gate} found no inputs to record under {root}")


def record_gate(root: Path, gate: int) -> Path:
    files, links, own_iface = collect_gate_inputs(root, gate)
    _require_non_empty_for_record(root, gate, files, links, own_iface)
    now = datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")
    doc = {
        "schema": 1,
        "gate": gate,
        "judged_at": now,
        "skill": detect_skill_version(root),
        "files": files,
        "links": links,
        "own_interface": own_iface,
    }
    gates_dir = root / "design" / "gates"
    gates_dir.mkdir(parents=True, exist_ok=True)
    out_path = gates_dir / f"phase{gate}.inputs.json"
    out_path.write_text(json.dumps(doc, indent=2) + "\n", encoding="utf-8")
    return out_path


def _short(sha: str) -> str:
    return sha[:7] if sha else "unknown"


def diff_gate_record(root: Path, record: dict) -> list[str]:
    """Return every difference between `record` and current project state in plain words."""
    gate = int(record["gate"])
    changes: list[str] = []

    rec_files: dict[str, str] = record.get("files") or {}
    for key in sorted(rec_files):
        expected = rec_files[key]
        rel_path, aspect = split_key(key)
        if not (root / rel_path).is_file():
            changes.append(f"{rel_path} removed since gate {gate}")
            continue
        try:
            actual = hash_input(root, key)
        except (ValueError, UnicodeDecodeError):
            actual = ""
        if actual != expected:
            what = f" ({ASPECT_WORDS[aspect]})" if aspect in ASPECT_WORDS else ""
            changes.append(f"{rel_path}{what} changed since gate {gate}")

    # Also detect newly added files in watched directories (e.g. design/parts/, design/sim/).
    if gate in GATE_SPECS:
        cur_files, cur_links, cur_own = collect_gate_inputs(root, gate)
        for rel_path in sorted(cur_files):
            if rel_path not in rec_files:
                changes.append(f"{rel_path} added since gate {gate}")
    else:
        cur_links = read_current_links(root)
        cur_own = read_current_own_interfaces(root, discover_boards(root))

    rec_links: dict[str, dict[str, str]] = record.get("links") or {}
    for link_id in sorted(rec_links):
        expected_link = rec_links[link_id]
        if link_id not in cur_links:
            changes.append(f"linked outline {link_id} was removed since gate {gate}")
            continue
        cur_link = cur_links[link_id]
        # Compare geometry_hash (NOT commit or revision) so a NewRevisionNoInterfaceChange
        # commit advance or no-edit handover does not mark the gate stale.
        if cur_link.get("geometry_hash", "") != expected_link.get("geometry_hash", ""):
            old_c = expected_link.get("commit", "")
            new_c = cur_link.get("commit", "")
            if old_c and new_c and old_c != new_c:
                changes.append(f"linked outline {link_id} moved to {_short(new_c)} (was {_short(old_c)})")
            else:
                changes.append(f"linked outline {link_id} changed since gate {gate}")

    rec_own: dict[str, dict[str, str]] = record.get("own_interface") or {}
    for board in sorted(rec_own):
        expected_own = rec_own[board]
        if board not in cur_own or cur_own[board].get("geometry_hash", "") != expected_own.get("geometry_hash", ""):
            if len(rec_own) > 1:
                changes.append(f"own outline ({board}) changed")
            else:
                changes.append("own outline changed")

    return changes


def cmd_record(root: Path, gate: int) -> int:
    out = record_gate(root, gate)
    doc = json.loads(out.read_text(encoding="utf-8"))
    print(
        f"recorded gate {gate} ({GATE_SPECS[gate].name}) -> {_rel(root, out)} "
        f"({len(doc['files'])} file(s), {len(doc['links'])} link(s), {len(doc['own_interface'])} own interface(s))"
    )
    return 0


def cmd_check(root: Path, gate: int | None) -> int:
    gates_dir = root / "design" / "gates"
    if gate is not None:
        rec_path = gates_dir / f"phase{gate}.inputs.json"
        if not rec_path.is_file():
            print(f"gate {gate}: no record at {rec_path}", file=sys.stderr)
            return 1
        targets = [(gate, rec_path)]
    else:
        targets = []
        for g in sorted(GATE_SPECS):
            p = gates_dir / f"phase{g}.inputs.json"
            if p.is_file():
                targets.append((g, p))
        if not targets:
            print("gate_inputs: no gate input records found")
            return 0

    any_stale = False
    first_stale: int | None = None
    for g, rec_path in targets:
        rec = json.loads(rec_path.read_text(encoding="utf-8"))
        diffs = diff_gate_record(root, rec)
        if diffs:
            any_stale = True
            if first_stale is None:
                first_stale = g
            for d in diffs:
                print(f"gate {g} ({GATE_SPECS[g].name}): STALE -- {d}")
        elif first_stale is not None and gate is None:
            any_stale = True
            print(f"gate {g} ({GATE_SPECS[g].name}): STALE -- gate {first_stale} is stale")
        else:
            print(f"gate {g} ({GATE_SPECS[g].name}): clean")
    return 1 if any_stale else 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Record and check judged inputs for PCB design phase gates.",
    )
    parser.add_argument("--project", default=None, help="Project root directory (default: auto-detect from cwd)")
    sub = parser.add_subparsers(dest="cmd", required=True)

    p_rec = sub.add_parser("record", help="Write design/gates/phaseN.inputs.json")
    p_rec.add_argument("gate", type=int, choices=range(0, 11), metavar="N", help="Gate number (0..10)")
    p_rec.add_argument("--project", default=argparse.SUPPRESS, help="Project root directory")

    p_chk = sub.add_parser("check", help="Recompute hashes and report changes (exit 1 if stale)")
    p_chk.add_argument("gate", nargs="?", type=int, choices=range(0, 11), metavar="N", help="Gate number (0..10); omit to check all recorded gates")
    p_chk.add_argument("--project", default=argparse.SUPPRESS, help="Project root directory")

    ns = parser.parse_args(argv)
    root = resolve_project(ns.project)

    if ns.cmd == "record":
        return cmd_record(root, ns.gate)
    return cmd_check(root, ns.gate)


if __name__ == "__main__":
    raise SystemExit(main())
