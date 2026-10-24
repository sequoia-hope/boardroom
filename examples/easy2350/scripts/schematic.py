#!/usr/bin/env python3
"""schematic.py -- draw the easy2350 schematic.

    python3 scripts/symbols.py && python3 scripts/schematic.py

This script is the schematic's source: edit it, re-run it, then check with
`kicad-cli sch erc` and the exported PDF. One A3 sheet, blocks in signal-flow
order: USB-C and the 3V3 regulator on the left, the RP2350 in the middle with
its supply and regulator above it, flash and crystal on its left, the GPIO
headers on the right, and the small blocks (buttons, SWD, status LED, RGB
LEDs) along the bottom. Every value comes from a part audit in design/parts/.
"""
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT.parents[1] / ".claude/skills/pcb-design/scripts"))
from kicad_write import Schematic, SymbolLib  # noqa: E402

HW = ROOT / "hw/easy2350/a"
SCH = HW / "easy2350.kicad_sch"

C0402 = "Capacitor_SMD:C_0402_1005Metric"
C0402S = "easy2350:C_0402_1005Metric_small_pads"
C0805 = "Capacitor_SMD:C_0805_2012Metric"
R0402 = "Resistor_SMD:R_0402_1005Metric"

# value -> (LCSC, MPN, manufacturer). Sourcing in design/parts/*.md.
PARTS = {
    "100n":  ("C1525", "CL05B104KO5NNNC", "Samsung Electro-Mechanics"),
    "4.7u":  ("C23733", "CL05A475MP5NRNC", "Samsung Electro-Mechanics"),
    "10u":   ("C15850", "CL21A106KAYNNNE", "Samsung Electro-Mechanics"),
    "15p":   ("C1548", "0402CG150J500NT", "FH"),
    "1k":    ("C11702", "0402WGF1001TCE", "UNI-ROYAL"),
    "27R":   ("C25100", "0402WGF270JTCE", "UNI-ROYAL"),
    "33R":   ("C25105", "0402WGF330JTCE", "UNI-ROYAL"),
    "5.1k":  ("C25905", "0402WGF5101TCE", "UNI-ROYAL"),
    "470R":  ("C25117", "0402WGF4700TCE", "UNI-ROYAL"),
    "100R":  ("C25076", "0402WGF1000TCE", "UNI-ROYAL"),
    "1u 0603": ("C15849", "CL10A105KB8NNNC", "Samsung Electro-Mechanics"),
    "2.2u 0603": ("C23630", "CL10A225KO8NNNC", "Samsung Electro-Mechanics"),
    "4.7k":  ("C25900", "0402WGF4701TCE", "UNI-ROYAL"),
    "100k":  ("C25741", "0402WGF1003TCE", "UNI-ROYAL"),
    "4.7u 0603": ("C19666", "CL10A475KO8NNNC", "Samsung Electro-Mechanics"),
}


def fields(value):
    lcsc, mpn, mfr = PARTS[value]
    return {"LCSC": lcsc, "MPN": mpn, "Manufacturer": mfr}


class Sheet(Schematic):
    """Schematic plus the few drawing idioms this sheet repeats."""

    def cap(self, ref, value, top, fp=C0402, label=None, text_left=False, decouples=None):
        """A decoupler hanging from `top` with its ground below it. `decouples`
        names the pin it serves ("U1.11"); placement checks read the field."""
        x, y = top
        dx = -2.54 if text_left else 2.54
        j = "right" if text_left else "left"
        f = fields(value) | ({"Decouples": decouples} if decouples else {})
        c = self.symbol("Device:C", ref, label or value, (x, y + 3.81), footprint=fp,
                        fields=f, ref_at=(x + dx, y + 2.54), value_at=(x + dx, y + 5.08),
                        justify=j)
        self.power("power:GND", (x, y + 7.62))
        return c

    def res_v(self, ref, value, top, text_left=False, to_gnd=False):
        x, y = top
        dx = -2.54 if text_left else 2.54
        j = "right" if text_left else "left"
        r = self.symbol("Device:R", ref, value, (x, y + 3.81), footprint=R0402, fields=fields(value),
                        ref_at=(x + dx, y + 2.54), value_at=(x + dx, y + 5.08), justify=j)
        if to_gnd:
            self.power("power:GND", (x, y + 7.62))
        return r

    def res_h(self, ref, value, left, text_below=False):
        """A resistor lying in a horizontal wire, pin 1 at `left`."""
        x, y = left
        ty = (y + 2.54, y + 5.08) if text_below else (y - 5.08, y - 2.54)
        return self.symbol("Device:R", ref, value, (x + 3.81, y), rot=90, footprint=R0402,
                           fields=fields(value), ref_at=(x + 3.81, ty[0]), value_at=(x + 3.81, ty[1]),
                           justify="")


def build() -> Sheet:
    s = Sheet("easy2350", title="easy2350", rev="A", date="2026-10-23",
              company="github.com/sequoia-hope/easy_pcb", paper="A3",
              comments=["RP2350A dev board, 21 x 51 mm", "CERN-OHL-P-2.0", "Author: Sequoia Alexander",
                        "Drawn by scripts/schematic.py"],
              libs=SymbolLib([HW / "lib/easy2350.kicad_sym"]))

    # ------------------------------------------------------------ RP2350A
    U1x, U1y = 233.68, 147.32
    u1 = s.symbol("easy2350:RP2350A", "U1", "RP2350A", (U1x, U1y),
                  footprint="easy2350:RP2350-QFN-60-1EP_7x7_P0.4mm_EP3.4x3.4mm_ThermalVias",
                  fields={"LCSC": "C42411118", "MPN": "RP2350A", "Manufacturer": "Raspberry Pi"},
                  ref_at=(U1x + 31.75, U1y - 44.45), value_at=(U1x + 31.75, U1y - 41.91))
    top = 101.6                       # y of every top pin's end
    rail = 81.28                      # the 3V3 rail above the chip

    # 3V3 users: verticals from each pin up to the rail.
    for name in ("USB_OTP_VDD", "ADC_AVDD", "QSPI_IOVDD", "IOVDD"):
        p = u1.pin(name)
        s.wire(p, (p[0], rail))
    vin = u1.pin("VREG_VIN")
    s.wire(vin, (vin[0], rail))
    s.wire((124.46, rail), (vin[0], rail))   # continues the LDO output

    # CIN at VREG_VIN, between IOVDD and VREG_VIN.
    s.wire((vin[0], 86.36), (vin[0] - 6.35, 86.36))
    s.cap("C14", "4.7u", (vin[0] - 6.35, 86.36), fp=C0402S, decouples="U1.49")

    # VREG_AVDD: 33 R from the rail, 4.7 u to ground at the pin.
    av = u1.pin("VREG_AVDD")
    s.res_v("R3", "33R", (av[0], rail))
    s.wire((av[0], rail + 7.62), av)
    s.wire((av[0], 99.06), (av[0] - 7.62, 99.06))
    s.cap("C13", "4.7u", (av[0] - 7.62, 99.06), text_left=True, decouples="U1.46")
    s.label("VREG_AVDD", (av[0], 97.79), rot=90)
    s.power("power:PWR_FLAG", (av[0] - 7.62, 99.06), value="PWR_FLAG")

    # IOVDD, QSPI_IOVDD, ADC_AVDD, USB_OTP_VDD decouplers: one per pin, on the rail.
    # C9 serves ADC_AVDD 44 and IOVDD 45 together, as the vendor's C17 does: a
    # separate cap at pin 44 blocked GPIO29's escape (decisions.md, phase 7).
    serves = [("C4", "1"), ("C5", "11"), ("C6", "20"), ("C7", "30"), ("C8", "38"), ("C11", "53"),
              ("C9", "44")]                   # the unstacked pins (53, 44) nearest the chip
    for i, (ref, pin) in enumerate(serves):
        s.cap(ref, "100n", (137.16 + 8.89 * i, rail), decouples=f"U1.{pin}")
    s.text("100 nF at IOVDD pins 1, 11, 20, 30, 38 (C4-C8),\n"
           "ADC_AVDD 44 and IOVDD 45 (C9, shared as in the vendor\n"
           "layout), USB_OTP_VDD 53 and QSPI_IOVDD 54 (C11). The\nDecouples field on each names its pin.",
           (129.54, 63.5))

    # Core regulator: VREG_LX -> L1 -> 1V1 -> VREG_FB and DVDD.
    lx, fb, dv = u1.pin("VREG_LX"), u1.pin("VREG_FB"), u1.pin("DVDD")
    y_lx, y_fb, x1 = 71.12, 88.9, dv[0] + 5.08
    s.wire(lx, (lx[0], y_lx))
    l1 = s.symbol("Device:L", "L1", "3.3u", ((lx[0] + x1) / 2, y_lx), rot=270,
                  footprint="easy2350:L_pol_2016",
                  fields={"LCSC": "C42411119", "MPN": "AOTA-B201610S3R3-101-T", "Manufacturer": "Abracon"},
                  ref_at=((lx[0] + x1) / 2, y_lx - 5.08), value_at=((lx[0] + x1) / 2, y_lx - 2.54),
                  justify="")
    s.wire((lx[0], y_lx), l1.pin("2"))     # pad 2 is LX; pad 1 (the dot) is 1V1
    s.wire(l1.pin("1"), (x1, y_lx))
    s.label("VREG_LX", (lx[0], 96.52), rot=90)
    s.wire(fb, (fb[0], y_fb), (x1, y_fb), (x1, y_lx))
    s.wire(dv, (dv[0], y_fb))
    s.wire((x1, y_lx), (x1 + 50.8, y_lx))
    s.power("power:+1V1", (x1 + 50.8, y_lx))
    s.power("power:PWR_FLAG", (x1 + 43.18, y_lx), value="PWR_FLAG")
    tp3 = s.symbol("Connector:TestPoint", "TP3", "1V1", (x1 + 50.8, y_lx + 7.62), rot=180,
                   footprint="TestPoint:TestPoint_Pad_D1.0mm", in_bom=False,
                   fields={"Manufacturer": "", "MPN": "", "LCSC": ""},
                   ref_at=(x1 + 53.34, y_lx + 10.16), value_at=(x1 + 53.34, y_lx + 12.7))
    s.wire((x1 + 50.8, y_lx), tp3.pin("1"))
    s.cap("C15", "4.7u", (x1 + 7.62, y_lx), fp=C0402S, decouples="U1.50")
    s.cap("C16", "4.7u", (x1 + 17.78, y_lx), decouples="U1.23")
    for x, ref, pin in ((x1 + 27.94, "C17", "6"), (x1 + 38.1, "C18", "39")):
        s.cap(ref, "100n", (x, y_lx), decouples=f"U1.{pin}")
    s.text("C15 COUT at L1; C16 at DVDD pin 23;\nC17, C18 at DVDD pins 6, 39", (x1 + 5.08, y_lx - 7.62))
    s.text("Regulator: C14 CIN and C15 COUT (0402 small pads), L1's\npolarity and copper all follow chips/rp2350.md", (lx[0] - 48.26, 60.96))

    # Ground pins.
    for name in ("VREG_PGND", "GND"):
        p = u1.pin(name)
        s.power("power:GND", p)
    s.text("VREG_PGND joins GND at one point", (U1x - 15.24, 200.66))

    # GPIO: a short wire and a label each; the headers are on the right.
    for i in range(30):
        name = f"GPIO{i}" if i < 26 else f"GPIO{i}/ADC{i - 26}"
        p = u1.pin(name)
        s.wire(p, (p[0] + 5.08, p[1]))
        s.label(f"GPIO{i}", (p[0] + 5.08, p[1]))

    # RUN: a label to the RESET button and header pin 30.
    p = u1.pin("RUN")
    s.wire(p, (p[0] - 7.62, p[1]))
    s.label("RUN", (p[0] - 7.62, p[1]), rot=180)
    # SWD: straight to J4, as on the vendor board (the 100 R series pair the
    # core audit suggested took the GPIO escape channel; decisions.md).
    for name in ("SWCLK", "SWDIO"):
        p = u1.pin(name)
        s.wire(p, (p[0] - 7.62, p[1]))
        s.label(name, (p[0] - 7.62, p[1]), rot=180)

    # USB: 27 R at the chip, staggered so their text has room.
    dm, dp = u1.pin("USB_DM"), u1.pin("USB_DP")
    r4 = s.res_h("R4", "27R", (dm[0] - 26.67, dm[1]))
    s.wire(dm, r4.pin("2"))
    s.label("USB_DM", r4.pin("1"), rot=180)
    r5 = s.res_h("R5", "27R", (dp[0] - 26.67, dp[1]), text_below=True)
    s.wire(dp, r5.pin("2"))
    s.label("USB_DP", r5.pin("1"), rot=180)
    s.text("27R at the chip", (dm[0] - 55.88, dm[1] - 2.54))
    s.label("USB_DM_MCU", (dm[0] - 5.08, dm[1]), rot=180)
    s.label("USB_DP_MCU", (dp[0] - 5.08, dp[1]), rot=180)

    # Flash, mirrored so its pins face the chip; QSPI wires run straight across.
    ss = u1.pin("~{QSPI_SS}")
    fx, fy = 157.48, ss[1] + 7.62
    u5 = s.symbol("Memory_Flash:W25Q128JVS", "U5", "W25Q128JVSIQ", (fx, fy), mirror="y",
                  footprint="Package_SO:SOIC-8_5.3x5.3mm_P1.27mm",
                  fields={"LCSC": "C97521", "MPN": "W25Q128JVSIQ", "Manufacturer": "Winbond"},
                  ref_at=(fx - 8.89, fy + 13.97), value_at=(fx - 8.89, fy + 16.51), justify="right")
    for fpin, upin, lab in (("1", "~{QSPI_SS}", "QSPI_SS"), ("6", "QSPI_SCLK", "QSPI_SCLK"),
                            ("5", "QSPI_SD0", "QSPI_SD0"), ("2", "QSPI_SD1", "QSPI_SD1"),
                            ("3", "QSPI_SD2", "QSPI_SD2"), ("7", "QSPI_SD3", "QSPI_SD3")):
        a, b = u5.pin(fpin), u1.pin(upin)
        assert abs(a[1] - b[1]) < 1e-6, (fpin, a, b)
        s.wire(a, b)
        s.label(lab, (a[0] + 7.62, a[1]))
    vcc, gnd = u5.pin("8"), u5.pin("4")
    s.power("power:+3V3", vcc)
    s.wire(vcc, (vcc[0] - 12.7, vcc[1]))
    s.cap("C22", "100n", (vcc[0] - 12.7, vcc[1]), text_left=True, decouples="U5.8")
    s.power("power:GND", gnd)

    # Crystal: XIN straight to pin 1, XOUT through 1 k to pin 3, 15 p loads.
    xin, xout = u1.pin("XIN"), u1.pin("XOUT")
    xa, xb = 172.72, 180.34            # crystal pins 1 and 3
    yc = (xin[1] + xout[1]) / 2
    y1 = s.symbol("Device:Crystal_GND24", "Y1", "12MHz", ((xa + xb) / 2, yc),
                  footprint="Crystal:Crystal_SMD_3225-4Pin_3.2x2.5mm",
                  fields={"LCSC": "C20625731", "MPN": "ABM8-272-T3", "Manufacturer": "Abracon"},
                  ref_at=((xa + xb) / 2, xin[1] - 5.08), value_at=((xa + xb) / 2, xin[1] - 2.54), justify="")
    p1, p3 = y1.pin("1"), y1.pin("3")
    s.wire(xin, (p1[0], xin[1]), p1)
    s.label("XIN", (187.96, xin[1]))
    r6 = s.res_h("R6", "1k", (xout[0] - 12.7, xout[1]))
    s.wire(xout, r6.pin("2"))
    s.label("XOUT", r6.pin("2"))
    s.wire(r6.pin("1"), (p3[0], xout[1]), p3)
    s.label("XOUT_R", (p3[0] + 2.54, xout[1]))
    s.power("power:GND", y1.pin("2"))
    s.wire((p1[0], xin[1]), (p1[0] - 7.62, xin[1]))
    s.cap("C20", "15p", (p1[0] - 7.62, xin[1]), text_left=True)
    s.cap("C21", "15p", (p3[0], xout[1]))

    s.box((128.27, 55.88), (320.04, 205.74), "RP2350A, FLASH, CRYSTAL")

    usb_power(s)
    rgb_leds(s)
    headers(s)
    controls(s)
    return s


def usb_power(s: Sheet):
    """USB-C receptacle, ESD array, CC pull-downs, and the 3V3 LDO."""
    vbus = 81.28                      # same y as the 3V3 rail: the LDO output runs straight in
    j1 = s.symbol("Connector:USB_C_Receptacle_USB2.0_16P", "J1", "USB-C", (35.56, 96.52),
                  footprint="Connector_USB:USB_C_Receptacle_HRO_TYPE-C-31-M-12",
                  fields={"LCSC": "C165948", "MPN": "TYPE-C-31-M-12", "Manufacturer": "Korean Hroparts Elec"},
                  ref_at=(25.4, 73.66), value_at=(25.4, 76.2))
    s.wire(j1.pin("A4"), (96.52, vbus))
    s.power("power:VBUS", (55.88, vbus))
    s.power("power:PWR_FLAG", (68.58, vbus), value="PWR_FLAG")
    # Either plug orientation: tie A6/B6 and A7/B7 at the connector.
    s.wire(j1.pin("A7"), (53.34, j1.pin("A7")[1]), (53.34, j1.pin("B7")[1]))
    s.wire(j1.pin("A6"), (53.34, j1.pin("A6")[1]), (53.34, j1.pin("B6")[1]), j1.pin("B6"))
    for sbu in ("A8", "B8"):
        s.no_connect(j1.pin(sbu))
    g, sh = j1.pin("A1"), j1.pin("S1")
    s.wire(sh, (sh[0], sh[1] + 2.54), (g[0], sh[1] + 2.54))
    s.wire(g, (g[0], g[1] + 2.54))
    s.power("power:GND", (g[0], g[1] + 2.54))
    s.power("power:PWR_FLAG", (g[0] + 10.16, g[1] + 2.54), rot=180, value="PWR_FLAG")
    s.wire((g[0], g[1] + 2.54), (g[0] + 10.16, g[1] + 2.54))
    s.text("Shield straight to GND", (15.24, g[1] + 8.89))

    # CC: 5.1 k Rd to ground on each, below the data lines (the wires cross them).
    for pin, net, x in (("A5", "CC1", 55.88), ("B5", "CC2", 63.5)):
        p = j1.pin(pin)
        s.wire(p, (x, p[1]), (x, 106.68))
        s.label(net, (p[0] + 1.27, p[1]))
        s.res_v("R1" if net == "CC1" else "R2", "5.1k", (x, 106.68), to_gnd=True)
    s.text("Rd: USB-C sink", (58.42, 127.0))

    # ESD array in line: D- through I/O1 (1 -> 6), D+ through I/O2 (3 -> 4).
    u2 = s.symbol("Power_Protection:USBLC6-2SC6", "U2", "USBLC6-2SC6", (81.28, 96.52),
                  fields={"LCSC": "C7519", "MPN": "USBLC6-2SC6", "Manufacturer": "STMicroelectronics"},
                  ref_at=(85.09, 105.41), value_at=(85.09, 107.95))
    s.wire(j1.pin("B7"), u2.pin("1"))
    s.wire(j1.pin("A6"), u2.pin("3"))
    s.wire(u2.pin("5"), (81.28, vbus))
    s.label("USB_DM_C", (66.04, j1.pin("B7")[1]))
    s.label("USB_DP_C", (66.04, j1.pin("A6")[1]))
    s.power("power:GND", u2.pin("2"))
    for pin, net in (("6", "USB_DM"), ("4", "USB_DP")):
        p = u2.pin(pin)
        s.wire(p, (p[0] + 2.54, p[1]))
        s.label(net, (p[0] + 2.54, p[1]))
    s.cap("C1", "100n", (86.36, vbus), decouples="U2.5")
    s.box((12.7, 63.5), (96.52, 132.08), "USB-C, ESD")

    # 3V3 LDO.
    u3 = s.symbol("easy2350:AP7361C-33FGE", "U3", "AP7361C-33FGE", (114.3, vbus + 2.54),
                  fields={"LCSC": "C151007", "MPN": "AP7361C-33FGE-7", "Manufacturer": "Diodes Incorporated"},
                  ref_at=(100.33, vbus - 10.16), value_at=(100.33, vbus - 7.62))
    s.wire((96.52, vbus), u3.pin("8"))
    s.wire(u3.pin("8"), u3.pin("5"))          # EN tied to IN (ldo-3v3.md)
    s.power("power:GND", u3.pin("4"))
    s.cap("C2", "2.2u 0603", (100.33, vbus), fp="Capacitor_SMD:C_0603_1608Metric", label="2.2u",
          decouples="U3.8")
    s.wire(u3.pin("1"), (124.46, vbus))
    s.cap("C3", "10u", (121.92, vbus), fp=C0805, decouples="U3.1")
    s.power("power:+3V3", (121.92, vbus))
    s.text("277 mA max, 0.62 W at\n5.5 V in: EP to GND vias", (100.33, 101.6))
    s.box((97.79, 63.5), (127.0, 109.22), "3V3 LDO")


def rgb_leds(s: Sheet):
    """VBUS -> D2 -> VLED; U4 shifts GPIO23 up to VLED logic; four pixels in a chain."""
    rail = 226.06
    s.power("power:VBUS", (22.86, rail))
    d2 = s.symbol("Device:D", "D2", "SM4007PL", (30.48, rail), rot=180,
                  footprint="easy2350:D_SOD-123F_MDD",
                  fields={"LCSC": "C64898", "MPN": "SM4007PL", "Manufacturer": "MDD"},
                  ref_at=(30.48, rail - 5.08), value_at=(30.48, rail - 2.54), justify="")
    s.wire((22.86, rail), d2.pin("2"))
    s.wire(d2.pin("1"), (205.74, rail))
    s.power("power:PWR_FLAG", (116.84, rail), value="PWR_FLAG")
    s.power("power:VDD", (73.66, rail), value="VLED")
    s.res_v("R11", "4.7k", (40.64, rail), to_gnd=True)
    # 5.1 V clamp: bounds VLED after a hot-plug ring (review1-io #1, rgb-leds.md).
    zx = 50.8
    d7 = s.symbol("Device:D_Zener", "D7", "BZT52B5V1", (zx, rail + 3.81), rot=270,
                  footprint="Diode_SMD:D_SOD-123",
                  fields={"LCSC": "C19077393", "MPN": "BZT52B5V1", "Manufacturer": "R+O"},
                  ref_at=(zx + 2.54, rail + 2.54), value_at=(zx + 2.54, rail + 5.08), ref_rot=90)
    k, a = d7.pin("1"), d7.pin("2")
    assert k[1] < a[1], "zener cathode must be up, on VLED"
    s.wire((zx, rail), k)
    s.power("power:GND", a)
    s.cap("C23", "4.7u 0603", (66.04, rail), fp="Capacitor_SMD:C_0603_1608Metric", label="4.7u")
    s.text("VLED 3.5-5.1 V: the rectifier's drop, held up by the\n4.7k bleed, keeps the pixels "
           "inside 3.3-5.3 V;\nD7 clamps a hot-plug ring (parts/rgb-leds.md)", (17.78, 262.89))

    # Level shifter: 3.3 V GPIO in, VLED logic out. OE tied low.
    ux, uy = 93.98, 243.84
    u4 = s.symbol("easy2350:SN74LV1T125", "U4", "SN74LV1T125DBVR", (ux, uy),
                  footprint="Package_TO_SOT_SMD:SOT-23-5",
                  fields={"LCSC": "C473338", "MPN": "SN74LV1T125DBVR", "Manufacturer": "Texas Instruments"},
                  ref_at=(ux + 2.54, uy - 7.62), value_at=(ux + 2.54, uy + 7.62))
    s.wire(u4.pin("5"), (u4.pin("5")[0], rail))
    s.cap("C24", "100n", (ux - 10.16, rail), text_left=True, decouples="U4.5")
    s.power("power:GND", u4.pin("3"))
    s.power("power:GND", u4.pin("1"))
    a = u4.pin("2")
    s.wire((a[0] - 15.24, a[1]), a)
    s.label("GPIO23", (a[0] - 15.24, a[1]), rot=180)
    s.res_v("R12", "100k", (a[0] - 5.08, a[1]), text_left=True, to_gnd=True)
    y = u4.pin("4")
    r10 = s.res_h("R10", "33R", (y[0], y[1]))

    # Pixels: DIN on the left, DOUT on the right, each with its 100 nF.
    prev = r10.pin("2")
    for i, (ref, cref) in enumerate((("D3", "C25"), ("D4", "C26"), ("D5", "C27"), ("D6", "C28"))):
        px = 129.54 + 22.86 * i
        d = s.symbol("LED:WS2812B-2020", ref, "WS2812B-2020-V6", (px, uy),
                     fields={"LCSC": "C52917434", "MPN": "WS2812B-2020-V6", "Manufacturer": "Worldsemi"},
                     datasheet="https://www.lcsc.com/product-detail/C52917434.html",
                     ref_at=(px + 2.54, uy + 8.89), value_at=(px - 7.62, uy + 13.97))
        s.wire(prev, d.pin("3"))
        s.label("LED_DIN" if i == 0 else f"PIX{i}{i + 1}", (prev[0] + 1.27, uy))
        s.wire(d.pin("4"), (px, rail))
        s.power("power:GND", d.pin("2"))
        s.cap(cref, "100n", (px + 7.62, rail), decouples=f"{ref}.4")
        prev = d.pin("1")
    s.no_connect(prev)
    s.box((12.7, 213.36), (210.82, 271.78), "RGB LEDS: VLED RAIL, LEVEL SHIFTER, 4 PIXELS")


# Board pin map: parts/user-io.md. J2 is the left edge (board pins 1-20, top to
# bottom), J3 the right edge (board pins 21-40, bottom to top), USB-C at the top.
J2_MAP = ["GPIO0", "GPIO1", "GND", "GPIO2", "GPIO3", "GPIO4", "GPIO5", "GND", "GPIO6", "GPIO7",
          "GPIO8", "GPIO9", "GND", "GPIO10", "GPIO11", "GPIO12", "GPIO13", "GND", "GPIO14", "GPIO15"]
J3_MAP = ["GPIO16", "GPIO17", "GND", "GPIO18", "GPIO19", "GPIO20", "GPIO21", "GND", "GPIO22", "RUN",
          "GPIO23", "GPIO24", "GPIO25", "GPIO26", "GPIO27", "+3V3", "GPIO28", "GND", "GPIO29", "VBUS"]


def headers(s: Sheet):
    """Drawn as the board is seen from the top: J2 down the left edge, J3 up the right."""
    hdr = {"Manufacturer": "", "MPN": "", "LCSC": ""}
    j2 = s.symbol("Connector_Generic:Conn_01x20", "J2", "GPIO left", (350.52, 147.32),
                  footprint="Connector_PinHeader_2.54mm:PinHeader_1x20_P2.54mm_Vertical", in_bom=False,
                  fields=hdr, ref_at=(347.98, 119.38), value_at=(347.98, 176.53), justify="")
    j3 = s.symbol("Connector_Generic:Conn_01x20", "J3", "GPIO right", (375.92, 149.86), rot=180,
                  footprint="Connector_PinHeader_2.54mm:PinHeader_1x20_P2.54mm_Vertical", in_bom=False,
                  fields=hdr, ref_at=(378.46, 119.38), value_at=(378.46, 176.53), justify="", ref_rot=0)
    # Signals get labels; each header's grounds join on a bus outside the labels.
    for j, pmap, side in ((j2, J2_MAP, -1), (j3, J3_MAP, 1)):
        bus = j.pin("1")[0] + side * 20.32
        grounds = []
        for n, net in enumerate(pmap, 1):
            p = j.pin(str(n))
            if net == "GND":
                s.wire(p, (bus, p[1]))
                grounds.append(p[1])
            else:
                s.wire(p, (p[0] + side * 7.62, p[1]))
                s.label(net, (p[0] + side * 7.62, p[1]), rot=0 if side > 0 else 180)
        low = max(grounds) + 5.08
        s.wire((bus, min(grounds)), (bus, low))
        s.power("power:GND", (bus, low))
    s.text("USB-C end", (352.425, 116.84))
    s.text("Pin map: parts/user-io.md. Not Pico\npin-compatible: pins 31-39 differ.\n"
           "Headers are not assembled.", (330.2, 182.88))
    s.box((322.58, 109.22), (408.94, 195.58), "GPIO HEADERS (TOP VIEW)")


def controls(s: Sheet):
    """Buttons on the left of the title block; SWD and the status LED under the
    headers. VBUS, 3V3 and GND test points are header pins 40, 36 and 38."""
    y = 226.06
    # BOOTSEL: pulls QSPI_SS low through 1 k while the chip boots.
    s.label("QSPI_SS", (220.98, y), rot=180)
    r7 = s.res_h("R7", "1k", (220.98, y))
    s.symbol("Switch:SW_Push", "SW1", "BOOTSEL", (236.22, y + 5.08), rot=90,
             footprint="Button_Switch_SMD:SW_SPST_TS-1088-xR020",
             fields={"LCSC": "C720477", "MPN": "TS-1088-AR02016", "Manufacturer": "XUNPU"},
             ref_at=(242.57, y + 3.81), value_at=(242.57, y + 6.35))
    s.wire(r7.pin("2"), (236.22, y))
    s.power("power:GND", (236.22, y + 10.16))
    # RESET: pulls RUN low through 1 k.
    s.label("RUN", (259.08, y), rot=180)
    r8 = s.res_h("R8", "1k", (259.08, y))
    s.symbol("Switch:SW_Push", "SW2", "RESET", (274.32, y + 5.08), rot=90,
             footprint="Button_Switch_SMD:SW_SPST_TS-1088-xR020",
             fields={"LCSC": "C720477", "MPN": "TS-1088-AR02016", "Manufacturer": "XUNPU"},
             ref_at=(280.67, y + 3.81), value_at=(280.67, y + 6.35))
    s.wire(r8.pin("2"), (274.32, y))
    s.power("power:GND", (274.32, y + 10.16))
    s.box((213.36, 213.36), (298.45, 246.38), "BUTTONS")

    # SWD header, pin order as the Raspberry Pi Debug Probe's 3-pin cable.
    y = 213.36
    j4 = s.symbol("Connector_Generic:Conn_01x03", "J4", "SWD", (345.44, y),
                  footprint="Connector_PinHeader_2.54mm:PinHeader_1x03_P2.54mm_Vertical", in_bom=False,
                  fields={"Manufacturer": "", "MPN": "", "LCSC": ""},
                  ref_at=(347.98, y - 1.27), value_at=(347.98, y + 1.27))
    for n, net in (("1", "SWCLK"), ("2", "GND"), ("3", "SWDIO")):
        p = j4.pin(n)
        s.wire(p, (p[0] - 7.62, p[1]))
        s.label(net, (p[0] - 7.62, p[1]), rot=180)
    # Status LED on GPIO25, as on the Pico.
    y = 233.68
    s.label("GPIO25", (335.28, y), rot=180)
    r9 = s.res_h("R9", "1k", (335.28, y))
    d1 = s.symbol("Device:LED", "D1", "RED", (350.52, y), rot=180,
                  footprint="LED_SMD:LED_0603_1608Metric",
                  fields={"LCSC": "C2286", "MPN": "KT-0603R", "Manufacturer": "Hubei KENTO Elec"},
                  ref_at=(350.52, y - 5.08), value_at=(350.52, y - 2.54), justify="")
    s.wire(r9.pin("2"), d1.pin("2"))
    k = d1.pin("1")
    s.wire(k, (k[0] + 2.54, k[1]), (k[0] + 2.54, k[1] + 2.54))
    s.power("power:GND", (k[0] + 2.54, k[1] + 2.54))
    s.box((322.58, 200.66), (408.94, 246.38), "SWD, STATUS LED")


if __name__ == "__main__":
    build().save(SCH)
    print(f"wrote {SCH}")
