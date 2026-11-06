#!/usr/bin/env python3
"""Tests for gate_inputs.py (recording and checking phase gate inputs)."""
from __future__ import annotations

import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
SCRIPTS = HERE.parent
GATE_INPUTS = SCRIPTS / "gate_inputs.py"

SAMPLE_PCB = """\
(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers
\t\t(0 "F.Cu" signal)
\t\t(31 "B.Cu" signal)
\t\t(44 "Edge.Cuts" user)
\t)
\t(gr_line
\t\t(start 100 100)
\t\t(end 150 100)
\t\t(stroke (width 0.05) (type default))
\t\t(layer "Edge.Cuts")
\t\t(uuid "11111111-1111-1111-1111-111111111111")
\t)
\t(gr_line
\t\t(start 150 100)
\t\t(end 150 130)
\t\t(stroke (width 0.05) (type default))
\t\t(layer "Edge.Cuts")
\t\t(uuid "22222222-2222-2222-2222-222222222222")
\t)
\t(gr_arc
\t\t(start 150 130)
\t\t(mid 149.707107 130.707107)
\t\t(end 149 131)
\t\t(stroke (width 0.05) (type default))
\t\t(layer "Edge.Cuts")
\t\t(uuid "33333333-3333-3333-3333-333333333333")
\t)
\t(footprint "Resistor_SMD:R_0603_1608Metric"
\t\t(layer "F.Cu")
\t\t(uuid "44444444-4444-4444-4444-444444444444")
\t\t(at 110 110)
\t\t(property "Reference" "R1")
\t)
\t(segment
\t\t(start 110 110)
\t\t(end 120 110)
\t\t(width 0.2)
\t\t(layer "F.Cu")
\t\t(net 1)
\t\t(uuid "55555555-5555-5555-5555-555555555555")
\t)
)
"""


class GateInputsTest(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = Path(tempfile.mkdtemp(prefix="gate-inputs-"))
        self.proj = self.tmp / "proj"
        (self.proj / "design" / "parts").mkdir(parents=True)
        (self.proj / "design" / "gates").mkdir(parents=True)
        (self.proj / "design" / "sim" / "pdn").mkdir(parents=True)
        self.hw = self.proj / "hw" / "demo" / "a"
        (self.hw / "fab").mkdir(parents=True)

        (self.proj / "pcbview.toml").write_text(
            'title = "demo"\n\n'
            '[[boards]]\n'
            'id = "demo-a"\n'
            'name = "demo A"\n'
            'pcb = "hw/demo/a/demo.kicad_pcb"\n'
            'sch = "hw/demo/a/demo.kicad_sch"\n\n'
            '[[boards]]\n'
            'id = "demo-rev0"\n'
            'name = "demo rev0 (frozen)"\n'
            'git = "v0.1"\n'
            'pcb = "hw/demo/rev0/demo.kicad_pcb"\n'
            'sch = "hw/demo/rev0/demo.kicad_sch"\n',
            encoding="utf-8",
        )
        (self.proj / "design" / "intent.md").write_text("# Intent\n5V in, 3.3V 500mA out.\n", encoding="utf-8")
        (self.proj / "design" / "parts" / "ldo.md").write_text("# LDO audit\nAP2112K-3.3\n", encoding="utf-8")
        (self.proj / "design" / "nets.csv").write_text("net,class,reason\nGND,power,return\n", encoding="utf-8")
        (self.proj / "design" / "brief.md").write_text("# Brief\n50x31 mm 2-layer.\n", encoding="utf-8")
        (self.proj / "design" / "stackup.md").write_text("# Stackup\n1.6 mm JLC 2-layer.\n", encoding="utf-8")
        (self.proj / "design" / "sim" / "pdn" / "result.txt").write_text("Z < 0.1 ohm\n", encoding="utf-8")
        (self.hw / "demo.kicad_pro").write_text('{"board": {}}\n', encoding="utf-8")
        (self.hw / "demo.kicad_dru").write_text("(version 1)\n", encoding="utf-8")
        (self.hw / "demo.kicad_sch").write_text("(kicad_sch (version 20231120))\n", encoding="utf-8")
        (self.hw / "demo.kicad_pcb").write_text(SAMPLE_PCB, encoding="utf-8")
        (self.hw / "fab" / "bom.csv").write_text("Comment,Designator\n1k,R1\n", encoding="utf-8")

    def tearDown(self) -> None:
        shutil.rmtree(self.tmp, ignore_errors=True)

    def run_cli(self, *args: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [sys.executable, str(GATE_INPUTS), *args, "--project", str(self.proj)],
            capture_output=True,
            text=True,
        )

    def test_no_records_exits_clean(self) -> None:
        r = self.run_cli("check")
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertIn("no gate input records", r.stdout)

    def test_record_and_check_file_edits_and_downstream(self) -> None:
        for g in range(11):
            r = self.run_cli("record", str(g))
            self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
            rec_file = self.proj / "design" / "gates" / f"phase{g}.inputs.json"
            self.assertTrue(rec_file.is_file())
            data = json.loads(rec_file.read_text(encoding="utf-8"))
            self.assertEqual(data["schema"], 1)
            self.assertEqual(data["gate"], g)
            self.assertTrue(data["judged_at"].endswith("Z"))
            self.assertTrue(data["skill"])

        # Everything clean right after recording.
        r = self.run_cli("check")
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)

        # Edit schematic -> gates 0 and 1 stay clean, gate 2 stales, 3..10 stale downstream.
        (self.hw / "demo.kicad_sch").write_text("(kicad_sch (version 20231120) (paper \"A4\"))\n", encoding="utf-8")
        self.assertEqual(self.run_cli("check", "0").returncode, 0)
        self.assertEqual(self.run_cli("check", "1").returncode, 0)

        r2 = self.run_cli("check", "2")
        self.assertEqual(r2.returncode, 1)
        self.assertIn("hw/demo/a/demo.kicad_sch changed since gate 2", r2.stdout)

        r_all = self.run_cli("check")
        self.assertEqual(r_all.returncode, 1)
        self.assertIn("gate 2 (Schematic): STALE -- hw/demo/a/demo.kicad_sch changed since gate 2", r_all.stdout)
        # Gate 4 did not judge the schematic directly, so in an all-gates check it reports downstream staleness.
        self.assertIn("gate 4 (Mechanical and layout brief): STALE -- gate 2 is stale", r_all.stdout)

        # Adding a new part audit file after gate 1 stales gate 1.
        (self.proj / "design" / "parts" / "buck.md").write_text("# Buck\n", encoding="utf-8")
        r1 = self.run_cli("check", "1")
        self.assertEqual(r1.returncode, 1)
        self.assertIn("design/parts/buck.md added since gate 1", r1.stdout)

    def _write_pcb(self, text: str) -> None:
        (self.hw / "demo.kicad_pcb").write_text(text, encoding="utf-8")

    def _stale(self, gate: int) -> str:
        r = self.run_cli("check", str(gate))
        self.assertIn(r.returncode, (0, 1), r.stderr)
        return r.stdout if r.returncode == 1 else ""

    def test_each_gate_hashes_only_its_part_of_the_board(self) -> None:
        for g in (4, 6, 7, 9):
            self.assertEqual(self.run_cli("record", str(g)).returncode, 0)
        rec6 = json.loads((self.proj / "design" / "gates" / "phase6.inputs.json").read_text(encoding="utf-8"))
        self.assertIn("hw/demo/a/demo.kicad_pcb#placement", rec6["files"])

        # Re-route (track moved, via added) and a new UUID on an outline item:
        # outline and placement are untouched, copper and the whole file changed.
        rerouted = (
            SAMPLE_PCB
            .replace("(end 120 110)", "(end 128.5 114.2)")
            .replace("11111111-1111-1111-1111-111111111111", "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee")
            .rstrip()[:-1]
            + '\t(via (at 120 110) (size 0.6) (drill 0.3) (layers "F.Cu" "B.Cu") (net 1))\n)\n'
        )
        self._write_pcb(rerouted)
        self.assertEqual(self._stale(4), "")
        self.assertEqual(self._stale(6), "")
        self.assertIn("hw/demo/a/demo.kicad_pcb (copper) changed since gate 7", self._stale(7))
        self.assertIn("hw/demo/a/demo.kicad_pcb changed since gate 9", self._stale(9))

        # Silkscreen work and a zone refill don't stale routing.
        for g in (7,):
            self.assertEqual(self.run_cli("record", str(g)).returncode, 0)
        zone = (
            '\t(zone (net 1) (net_name "GND") (layer "B.Cu") (uuid "66666666-6666-6666-6666-666666666666")'
            ' (polygon (pts (xy 100 100) (xy 150 100) (xy 150 130))){fill})\n'
        )
        silk = '\t(gr_text "demo" (at 130 125) (layer "F.SilkS") (uuid "77777777-7777-7777-7777-777777777777"))\n'
        base = rerouted.rstrip()[:-1]
        self._write_pcb(base + zone.format(fill="") + ")\n")
        self.assertEqual(self.run_cli("record", "7").returncode, 0)
        filled = " (filled_polygon (layer \"B.Cu\") (pts (xy 101 101) (xy 149 101) (xy 149 129)))"
        self._write_pcb(base + zone.format(fill=filled) + silk + ")\n")
        self.assertEqual(self._stale(7), "")

        # Moving a footprint stales placement.
        self._write_pcb(rerouted.replace("(at 110 110)", "(at 115 112 90)"))
        self.assertIn("hw/demo/a/demo.kicad_pcb (placement) changed since gate 6", self._stale(6))
        self.assertEqual(self._stale(4), "")

        # Moving an outline item stales gate 4.
        self._write_pcb(SAMPLE_PCB.replace("(end 150 100)", "(end 152 100)"))
        self.assertIn("hw/demo/a/demo.kicad_pcb (Edge.Cuts) changed since gate 4", self._stale(4))

    def test_links_and_own_interface_staleness(self) -> None:
        (self.proj / "design" / "links.toml").write_text(
            '[[links]]\n'
            'id = "enclosure"\n'
            'kind = "waffle"\n'
            'board = "demo"\n'
            'locator = { type = "Git", remote = "../enc", path = "interface/demo/board-shape.json", ref = { type = "Branch", name = "main" } }\n'
            'resolved = { commit = "9f8e7d6c5b4a32109f8e7d6c5b4a32109f8e7d6c", at = "2026-11-01T00:00:00Z" }\n'
            'interface_hash = "sha256:aaaa1111"\n'
            'pin_reason = ""\n',
            encoding="utf-8",
        )
        link_dir = self.proj / ".boardroom" / "links" / "enclosure"
        link_dir.mkdir(parents=True)
        (link_dir / "board-shape.json").write_text(
            json.dumps({
                "schema": 1,
                "board": "demo",
                "revision": "rev:sha256:rev1",
                "geometry_hash": "sha256:aaaa1111",
            }),
            encoding="utf-8",
        )
        own_dir = self.proj / "interface" / "demo"
        own_dir.mkdir(parents=True)
        (own_dir / "board-shape.json").write_text(
            json.dumps({
                "schema": 1,
                "board": "demo",
                "revision": "rev:sha256:own1",
                "geometry_hash": "sha256:bbbb2222",
            }),
            encoding="utf-8",
        )
        vend = self.proj / ".claude" / "skills" / "pcb-design"
        vend.mkdir(parents=True)
        (vend / "VENDORED_FROM").write_text("boardroom@deadbeef1234\n", encoding="utf-8")

        self.assertEqual(self.run_cli("record", "4").returncode, 0)
        self.assertEqual(self.run_cli("record", "10").returncode, 0)

        rec10 = json.loads((self.proj / "design" / "gates" / "phase10.inputs.json").read_text(encoding="utf-8"))
        self.assertEqual(rec10["skill"], "boardroom@deadbeef1234")
        self.assertEqual(
            rec10["links"]["enclosure"],
            {
                "commit": "9f8e7d6c5b4a32109f8e7d6c5b4a32109f8e7d6c",
                "revision": "rev:sha256:rev1",
                "geometry_hash": "sha256:aaaa1111",
            },
        )
        self.assertEqual(
            rec10["own_interface"]["demo"],
            {"revision": "rev:sha256:own1", "geometry_hash": "sha256:bbbb2222"},
        )

        # Advance commit + revision with SAME geometry_hash -> NOT stale.
        (self.proj / "design" / "links.toml").write_text(
            (self.proj / "design" / "links.toml")
            .read_text(encoding="utf-8")
            .replace("9f8e7d6c5b4a32109f8e7d6c5b4a32109f8e7d6c", "a1b2c3d4e5f60718a1b2c3d4e5f60718a1b2c3d4"),
            encoding="utf-8",
        )
        (link_dir / "board-shape.json").write_text(
            json.dumps({
                "schema": 1,
                "board": "demo",
                "revision": "rev:sha256:rev2",
                "geometry_hash": "sha256:aaaa1111",
            }),
            encoding="utf-8",
        )
        self.assertEqual(self.run_cli("check", "4").returncode, 0)
        self.assertEqual(self.run_cli("check", "10").returncode, 0)

        # Now change geometry_hash as well -> stale naming old and new 7-char commits!
        (link_dir / "board-shape.json").write_text(
            json.dumps({
                "schema": 1,
                "board": "demo",
                "revision": "rev:sha256:rev3",
                "geometry_hash": "sha256:cccc3333",
            }),
            encoding="utf-8",
        )
        r4 = self.run_cli("check", "4")
        self.assertEqual(r4.returncode, 1)
        self.assertIn("linked outline enclosure moved to a1b2c3d (was 9f8e7d6)", r4.stdout)

        # Restore link geometry, then change own_interface geometry_hash -> "own outline changed".
        (link_dir / "board-shape.json").write_text(
            json.dumps({
                "schema": 1,
                "board": "demo",
                "revision": "rev:sha256:rev2",
                "geometry_hash": "sha256:aaaa1111",
            }),
            encoding="utf-8",
        )
        (own_dir / "board-shape.json").write_text(
            json.dumps({
                "schema": 1,
                "board": "demo",
                "revision": "rev:sha256:own2",
                "geometry_hash": "sha256:dddd4444",
            }),
            encoding="utf-8",
        )
        r4_own = self.run_cli("check", "4")
        self.assertEqual(r4_own.returncode, 1)
        self.assertIn("own outline changed", r4_own.stdout)


if __name__ == "__main__":
    unittest.main()
