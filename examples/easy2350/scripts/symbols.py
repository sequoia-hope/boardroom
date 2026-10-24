#!/usr/bin/env python3
"""symbols.py -- the project symbol and footprint library for easy2350.

    python3 scripts/symbols.py      writes hw/easy2350/a/lib/ and the lib tables

easy2350:RP2350A is the stock MCU_RaspberryPi:RP2350A with a wider body and
its power pins spread along the top in the order power flows: the 3V3 pins on
the left, then VREG_VIN, VREG_LX, VREG_FB and DVDD on the right. The stock
symbol packs them 2.54 mm apart, which leaves no room to draw a decoupler or
the regulator next to its pin. Pin numbers, names and types are unchanged.

easy2350:SN74LV1T125 is the stock 74xGxx:74LVC1G125 (same pinout) with the
active-low OE moved from the top to the bottom, beside GND, since it is tied
to ground.

Footprints copied from Raspberry Pi's RP2350A minimal board (RP-006440-DD),
as chips/rp2350.md asks: the QFN-60 with its thermal vias, L_pol_2016 (the
polarity-marked inductor) and the 0402 "small pads" capacitor for CIN and
COUT (parts/rp2350-core.md). D_SOD-123F_MDD is KiCad's
D_SOD-123F with the pads MDD's SM4007PL datasheet recommends (1.2 x 1.2 mm at
+-1.6 mm; parts/rgb-leds.md): the stock pads can end short of a long part's toe.
"""
import re
import shutil
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SKILL = ROOT.parents[1] / ".claude/skills/pcb-design/scripts"
sys.path.insert(0, str(SKILL))
from kicad_files import Sym, child, children, head  # noqa: E402
from kicad_write import S, SymbolLib, dump  # noqa: E402

HW = ROOT / "hw/easy2350/a"
LIB = HW / "lib"
VENDOR = Path.home() / "pcb/RP-006440-DD-2-RP2350A Minimal Board Kicad archive/RP2350_60QFN_minimal.pretty"

# Top pins left to right, x in mm from the symbol origin. 3V3 users first, a
# wider gap before VREG_VIN (room for CIN), then the regulator and 1V1 pins.
TOP_X = {"VREG_AVDD": -27.94, "USB_OTP_VDD": -20.32, "ADC_AVDD": -15.24, "QSPI_IOVDD": -10.16,
         "IOVDD": -5.08, "VREG_VIN": 5.08, "VREG_LX": 10.16, "VREG_FB": 15.24, "DVDD": 22.86}
BOTTOM_X = {"VREG_PGND": -5.08, "GND": 5.08}
HALF_W = 30.48  # body half-width; side pins move out to HALF_W + 3.81


def rp2350() -> list:
    sym = SymbolLib().get("MCU_RaspberryPi:RP2350A")
    out = [Sym("symbol"), "RP2350A"]
    for c in sym[2:]:
        if head(c) == "property" and c[1] in ("Reference", "Value"):
            at = child(c, "at")
            at[1] = HALF_W + 1.27
        if head(c) == "symbol":
            c = [c[0], c[1].replace("RP2350A", "RP2350A", 1)] + c[2:]
            for g in c[2:]:
                if head(g) == "rectangle":
                    child(g, "start")[1], child(g, "end")[1] = -HALF_W, HALF_W
                if head(g) != "pin":
                    continue
                at, name = child(g, "at"), child(g, "name")[1]
                if name in TOP_X and float(at[2]) > 0:
                    at[1] = TOP_X[name]
                elif name in BOTTOM_X:
                    at[1] = BOTTOM_X[name]
                elif float(at[1]) < 0:
                    at[1] = -HALF_W - 3.81
                else:
                    at[1] = HALF_W + 3.81
        out.append(c)
    return out


def buffer() -> list:
    sym = SymbolLib().get("74xGxx:74LVC1G125")
    out = [Sym("symbol"), "SN74LV1T125"]
    for c in sym[2:]:
        if head(c) == "property" and c[1] == "Value":
            c = [c[0], c[1], "SN74LV1T125", *c[3:]]
        if head(c) == "property" and c[1] == "Datasheet":
            c = [c[0], c[1], "https://www.ti.com/lit/ds/symlink/sn74lv1t125.pdf", *c[3:]]
        if head(c) == "symbol":
            c = [c[0], c[1].replace("74LVC1G125", "SN74LV1T125"), *c[2:]]
            for g in c[2:]:
                if head(g) == "pin":
                    num = child(g, "number")[1]
                    name = child(g, "name")
                    name[1] = {"1": "~{OE}", "2": "A", "4": "Y"}.get(num, name[1])
                    if num == "1":
                        at = child(g, "at")
                        at[2], at[3] = -10.16, 90
        out.append(c)
    return out


def ldo() -> list:
    """AP7361C-xxFGE (U-DFN3030-8 Type E): IN 8, EN 5 left; OUT 1 right; GND 4
    and the exposed pad 9 at the bottom; NC 2, 3, 6, 7 hidden (parts/ldo-3v3.md)."""
    def pin(kind, num, name, x, y, ang, hide=False):
        p = [Sym("pin"), Sym(kind), Sym("line"), S("at", x, y, ang), S("length", 2.54)]
        if hide:
            p.append(S("hide", Sym("yes")))
        return p + [S("name", name, S("effects", S("font", S("size", 1.27, 1.27)))),
                    S("number", num, S("effects", S("font", S("size", 1.27, 1.27))))]

    def prop(k, v, at, hide=False):
        eff = [Sym("effects"), S("font", S("size", 1.27, 1.27))]
        if hide:
            eff.append(S("hide", Sym("yes")))
        return S("property", k, v, S("at", *at, 0), eff)
    name = "AP7361C-33FGE"
    return [Sym("symbol"), name, S("pin_names", S("offset", 0.254)), S("exclude_from_sim", Sym("no")),
            S("in_bom", Sym("yes")), S("on_board", Sym("yes")),
            prop("Reference", "U", (-5.08, 5.715)), prop("Value", name, (0, 5.715)),
            prop("Footprint", "easy2350:DFN-8-1EP_3x3mm_P0.65mm_EP1.5x2.25mm_ThermalVias", (0, 8.255), True),
            prop("Datasheet", "https://www.diodes.com/assets/Datasheets/AP7361C.pdf", (0, 0), True),
            prop("Description", "1 A LDO, 3.3 V fixed, enable, U-DFN3030-8 Type E", (0, 0), True),
            [Sym("symbol"), f"{name}_0_1",
             S("rectangle", S("start", -5.08, 4.445), S("end", 5.08, -5.08),
               S("stroke", S("width", 0.254), S("type", Sym("default"))), S("fill", S("type", Sym("background"))))],
            [Sym("symbol"), f"{name}_1_1",
             pin("power_in", "8", "IN", -7.62, 2.54, 0), pin("input", "5", "EN", -7.62, 0, 0),
             pin("power_out", "1", "OUT", 7.62, 2.54, 180),
             pin("power_in", "4", "GND", 0, -7.62, 90), pin("passive", "9", "GND", 0, -7.62, 90, hide=True),
             *[pin("no_connect", n, "NC", 5.08, 0, 180, hide=True) for n in ("2", "3", "6", "7")]]]


def main():
    LIB.mkdir(parents=True, exist_ok=True)
    lib = [Sym("kicad_symbol_lib"), S("version", Sym("20241209")), S("generator", "easy2350_symbols"),
           S("generator_version", "9.0"), rp2350(), buffer(), ldo()]
    (LIB / "easy2350.kicad_sym").write_text(dump(lib) + "\n")
    pretty = LIB / "easy2350.pretty"
    pretty.mkdir(exist_ok=True)
    for name in ("L_pol_2016", "C_0402_1005Metric_small_pads",
                 "RP2350-QFN-60-1EP_7x7_P0.4mm_EP3.4x3.4mm_ThermalVias"):
        shutil.copy(VENDOR / f"{name}.kicad_mod", pretty / f"{name}.kicad_mod")
    sod = Path("/usr/share/kicad/footprints/Diode_SMD.pretty/D_SOD-123F.kicad_mod").read_text()
    sod = sod.replace('(footprint "D_SOD-123F"', '(footprint "D_SOD-123F_MDD"', 1)
    sod = sod.replace("(at -1.4 0)\n\t\t(size 1.1 1.1)", "(at -1.6 0)\n\t\t(size 1.2 1.2)")
    sod = sod.replace("(at 1.4 0)\n\t\t(size 1.1 1.1)", "(at 1.6 0)\n\t\t(size 1.2 1.2)")
    assert sod.count("(size 1.2 1.2)") == 2, "D_SOD-123F pads not where expected"
    # Courtyard 0.25 mm beyond the wider pads (review1-io #4).
    for a, b in (("(start -2.2 ", "(start -2.45 "), ("(end -2.2 ", "(end -2.45 "),
                 ("(start 2.2 ", "(start 2.45 "), ("(end 2.2 ", "(end 2.45 ")):
        sod = sod.replace(a, b)
    assert sod.count("2.45 ") == 8, "D_SOD-123F courtyard not where expected"
    (pretty / "D_SOD-123F_MDD.kicad_mod").write_text(sod)
    # AP7361C DFN with thermal vias in the exposed pad (parts/ldo-3v3.md: >= 5 EP
    # vias). Three 0.25 mm vias down the centre line; the paste becomes two strips
    # either side of them (about 47 % of the pad).
    dfn = Path("/usr/share/kicad/footprints/Package_DFN_QFN.pretty/"
               "DFN-8-1EP_3x3mm_P0.65mm_EP1.5x2.25mm.kicad_mod").read_text()
    dfn = dfn.replace('(footprint "DFN-8-1EP_3x3mm_P0.65mm_EP1.5x2.25mm"',
                      '(footprint "DFN-8-1EP_3x3mm_P0.65mm_EP1.5x2.25mm_ThermalVias"', 1)
    paste = re.findall(r'\t\(pad "" smd roundrect\n(?:\t\t.*\n)*?\t\)\n', dfn)
    assert len(paste) == 4, "DFN paste pads not where expected"
    for block in paste:
        dfn = dfn.replace(block, "")
    extra = "".join(
        f'\t(pad "" smd rect (at {x} 0) (size 0.4 2.0) (layers "F.Paste"))\n' for x in (-0.5, 0.5))
    extra += "".join(
        f'\t(pad "9" thru_hole circle (at 0 {y}) (size 0.55 0.55) (drill 0.25) '
        f'(layers "*.Cu") (zone_connect 2))\n' for y in (-0.75, 0, 0.75))
    dfn = dfn.replace('\t(pad "9" smd rect', extra + '\t(pad "9" smd rect', 1)
    (pretty / "DFN-8-1EP_3x3mm_P0.65mm_EP1.5x2.25mm_ThermalVias.kicad_mod").write_text(dfn)
    (HW / "sym-lib-table").write_text(
        '(sym_lib_table\n  (version 7)\n  (lib (name "easy2350")(type "KiCad")'
        '(uri "${KIPRJMOD}/lib/easy2350.kicad_sym")(options "")(descr "easy2350 project symbols"))\n)\n')
    (HW / "fp-lib-table").write_text(
        '(fp_lib_table\n  (version 7)\n  (lib (name "easy2350")(type "KiCad")'
        '(uri "${KIPRJMOD}/lib/easy2350.pretty")(options "")(descr "easy2350 project footprints"))\n)\n')
    print(f"wrote {LIB}")


if __name__ == "__main__":
    main()
