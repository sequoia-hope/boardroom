# U5 — Winbond W25Q128JVSIQ 128 Mbit QSPI flash (RP2350 boot flash)

Datasheet: datasheets/w25q128jv.pdf (Winbond W25Q128JV, Revision J, 2024-04-25). Read: all of it.
Page numbers below are PDF pages; Winbond prints a number one lower.
MPN: W25Q128JVSIQ   Supplier: C97521 W25Q128JVSIQ (Winbond, SOIC-8-208mil): basic, JLC stock 64503, lifecycle normal, checked 2026-10-23
Footprint: `Package_SO:SOIC-8_5.3x5.3mm_P1.27mm`. MPN decode (ordering, p.75): W25Q 128J V,
**S = 8-pin SOIC 208-mil**, **I = Industrial −40…+85 °C**, **Q = green package with QE = 1 fixed
in Status Register-2**; top marking 25Q128JVSQ (p.76). Package (p.68): body D 5.18–5.38, E1
5.13–5.33, lead span H 7.70–8.10, foot L 0.50–0.80, lead b 0.35–0.48, pitch 1.27. KiCad pads
1.625 × 0.65 at ±3.5875: they reach 4.40 mm from centre (toe at H/2 = 3.85–4.05 → 0.35–0.55 mm toe
fillet), start 2.775 mm from centre (foot heel at H/2 − L = 3.05–3.55 → heel covered), 0.65 wide
for a 0.35–0.48 lead. Winbond gives no land pattern. OK.

Symbol: `Memory_Flash:W25Q128JVS` (extends W25Q32JVSS): 1 ~CS, 2 DO/IO1, 3 ~WP/IO2, 4 GND,
5 DI/IO0, 6 CLK, 7 ~HOLD/~RESET/IO3, 8 VCC. Datasheet Fig. 1a and pin table (p.6): identical.

Also read: `rp2350-datasheet.pdf` (Release 8, build 2026-10-01) §5.2 boot flow (PDF p.339–340),
§5.2.7 flash boot and Table 463 (p.344–345), §5.8 UART boot (p.379), Table 1675 QSPI pins
(p.1334), Table 1683 digital IO (p.1337–1338); `hardware-design-with-rp2350.pdf` (Release 3) §3
flash memory (PDF p.11–13) and Appendix A schematic (p.21).

Operating conditions: VCC = 3V3 = 3.198–3.402 V (ldo-3v3.md linear worst case, junction-temperature corner); 0–40 °C; QSPI
from RP2350A pins 55–60, same 3V3 rail on QSPI_IOVDD (pin 54).

## Absolute max vs our worst case

| Parameter | Rating (page) | Our worst case | Margin |
|---|---|---|---|
| VCC abs max | −0.6 … 4.6 V (p.61) | 3.402 V | 1.2 V |
| VCC operating, FR = 133 MHz | 3.0–3.6 V (p.61); 2.7–3.0 V allows only 104 MHz | 3.198–3.402 V | OK, full speed grade (≥ 0.198 V margin) |
| Pin voltage | −0.6 … VCC + 0.4 V; < 20 ns transients −2.0 … VCC + 2.0 V (p.61) | 0–3.3 V from the RP2350 on the same rail | OK |
| TA (I grade) | −40 … +85 °C (p.61) | 0–40 °C ambient, board warmer (ldo-3v3.md) | OK |
| ESD HBM | ±2000 V (p.61) | Only QSPI_SS is finger-reachable, via the BOOTSEL button (intent: don't care, as on the Pico) | Accepted by intent |
| Supply current | read 15/18/20 mA max at 50/80/104 MHz quad; program/erase 25 mA max (p.63) | intent budget 25 mA max | OK |

## Required external parts (from datasheets)

- **Decoupling on VCC.** The Winbond datasheet makes no recommendation. The vendor's minimal board
  uses one 100 nF 25 V X7R 0402 at VCC (Appendix A, p.21, C2) → C_FLASH 100 nF 16 V X7R 0402
  (C1525); 16 V vs 3.3 V is 4.8× and X7R at 21 % of rating loses little.
- **No /WP or /HOLD pull-ups.** With QE = 1, "/WP pin becomes IO2 and the /HOLD pin becomes IO3"
  (p.10); for the Q suffix QE = 1 is "factory fixed" and "/HOLD function is disabled" (p.17; p.75
  note 5). The pins are data lines only and are driven by the RP2350 or the flash in every mode the
  bootrom and SDK use. The RP2350 also holds QSPI_SD2 and QSPI_SD3 with pull-ups in reset (Table
  1675, p.1334). The vendor schematic wires pins 3 and 7 straight to QSPI_SD2/SD3 (p.21).
- **No CS pull-up, and no footprint for one** (reasoning below; decisions.md).
- **No series 0 Ω (vendor R10).** R10 exists so an RP2354 (internal flash) can move U3 to GPIO0 as a
  second chip select (guide p.12, p.21 note "If using RP2354, remove R10"). We fit an RP2350A only,
  so QSPI_SS runs straight from U1 pin 60 to U5 pin 1.

## Calculations

### Does /CS need an external pull-up?

The flash's requirement: "/CS must track the VCC supply level at power-up and power-down ... If
needed a pull-up resister on the /CS pin can be used" (p.10; again p.13 and Fig. 58b, p.62), and
"After power-up, /CS must transition from high to low before a new instruction will be accepted"
(p.10). VCC(min) to /CS low ≥ 20 µs (tVSL, p.62).

What we have without one:
1. QSPI_SS resets to a pull-up (Table 1675, p.1334), RPU 32–86 kΩ at 3.3 V (Table 1683, p.1337),
   powered from QSPI_IOVDD = 3V3, the same net as the flash's VCC. Once the pads are live the pin
   tracks VCC.
2. The window the vendor worries about ("a short period of time during switch-on where the state of
   the QSPI_SS pin cannot be guaranteed", guide p.11) is before the RP2350's pads come out of
   isolation; the bootrom only "remove[s] isolation from QSPI pads" in Prepare for Image Boot
   (p.340).
3. During that window the flash cannot be corrupted: below VWI (1.0–2.0 V) it is held in reset and
   ignores everything; above VWI program/erase/write-status are ignored for tPUW ≥ 5 ms (p.13, p.62),
   and WEL is cleared at power-up so a write needs an explicit 06h first (p.13). A spurious select
   can at most start a read.
4. Before its first flash read the bootrom drives CSn = 1 and clocks, then CSn = 0 with a fixed
   sequence (guide p.13, "Issue XIP exit sequence to chip select 0", p.340). That supplies the
   "high to low" transition the flash wants before its first real instruction, long after tVSL.
5. The vendor fits R1 (10 kΩ) DNF: "we have found that with this particular flash device, the
   external pull-up is unnecessary" (guide p.11). Same part number as ours.

**Decision: no pull-up and no DNF footprint** (logged in `decisions.md` by the lead after the
gate-1 review, finding #4). This audit first proposed a DNF 10 kΩ footprint, as the vendor keeps,
for a possible flash substitution; there is no room for it between the header rows, and the
reasons above do not depend on it: QSPI_SS resets with its own pull-up (Table 1675, p.1334). A
flash substitution would have to repeat this check (guide p.11: other parts may need the
pull-up).

### QSPI_SS shared with the BOOTSEL button (vendor R6 1 kΩ, owned by `user-io.md`)

- Button pressed, RP2350 sampling (pin as input with its pull-up): 3.3 V × 1 kΩ / (1 kΩ + 32 kΩ)
  = **0.10 V worst case** (strongest pull-up, Table 1683) < VIL 0.8 V (Table 1683). BOOTSEL reads.
  The flash sees /CS low too, but with no clock it does nothing (instructions are clocked in on CLK,
  p.10).
- Button pressed while the RP2350 drives SS high (running code, or firmware that ignores the
  button): 3.3 V / 1 kΩ = 3.3 mA. The pad's VOH ≥ 2.62 V at its 2/4/8/12 mA rating (Table 1683)
  is ≥ the flash's VIH = 0.7 × VCC = 2.24–2.38 V (p.63) only if the pad's drive setting is ≥ 4 mA
  (assumption: the core audit confirms the QSPI pad drive default). This is why the guide says R6
  "allows the QSPI_SS pin to safely over-drive" the button (p.11).
- UART boot is "QSPI CSn low and QSPI SD1 high" (p.379). With the button pressed and no clock,
  the flash's DO (SD1) stays high-impedance (DO only drives during a read's output phase,
  p.10, p.29), and SD1 resets with a pull-down (Table 1675, "default pull-down selects USB boot",
  p.343). So BOOTSEL gives USB boot, as wanted.

### RP2350 boot compatibility

- The bootrom scans flash with EBh quad, BBh dual, 0Bh and 03h at SCK divisors 3, 6, 12, 24, quad
  first (Table 463, p.344). W25Q128JV supports all four (instruction table p.23–24; 03h p.29, 0Bh
  p.30, BBh p.33, EBh p.34). Because QE = 1 is fixed on the IQ part (p.17, p.75), the very first
  attempt (EBh quad, divisor 3) is valid; no status-register write is ever needed for quad mode.
- 03h is limited to 50 MHz (fR, p.65), all other reads to 133 MHz at VCC ≥ 3.0 V (p.65). The SDK's
  default QSPI clock on RP2350 is clk_sys / 2 = 75 MHz at 150 MHz (from memory of the pico-sdk
  default PICO_FLASH_SPI_CLKDIV = 2; not in our datasheets) — inside 133 MHz with margin.
- Bootrom programming needs 02h page program, 05h read status, 06h write enable, 20h 4 kB sector
  erase (guide p.12–13): all present (instruction table p.23).
- Capacity: 24-bit addressing reaches exactly 16 MB, which matches a 128 Mbit part with no 4-byte
  address mode needed; the guide calls 128 Mbit the largest RP2350 supports on its own (p.11).
- Continuous-read ("XIP") mode: the JV-IQ datasheet tells the host to send M7–M0 = Fxh with EBh
  (Fig. 24a, p.34), i.e. it documents no continuous mode for this ordering code; the bootrom's fixed
  exit sequence (guide p.13) is sent anyway, so a warm reset (RUN or watchdog) without a power cycle
  still finds the flash answering commands.

### Logic levels and drive

- RP2350 → flash: RP2350 VOH ≥ 2.62 V, VOL ≤ 0.5 V at rated drive (Table 1683); flash VIH 0.7 VCC
  ≤ 2.38 V, VIL 0.3 VCC ≥ 0.96 V (p.63). OK.
- Flash → RP2350: VOH ≥ VCC − 0.2 V, VOL ≤ 0.2 V at 100 µA (p.63); RP2350 VIH 2.0 V, VIL 0.8 V
  (Table 1683). OK.
- Output drive: the IQ part ships with DRV = 25 % (p.18, note 2). Winbond's AC timings are measured
  into 30 pF (p.64), tCLQV 6 ns max (p.65). Our load is a short trace (< 20 mm, a few pF) plus one
  RP2350 pad, so the 25 % driver is comfortably fast at 75 MHz (13.3 ns period). Keep the traces
  short; do not hang test points or header pins on QSPI nets.

## Fine print / gotchas

- **QE = 1 is fixed** on "IQ" (p.17, p.75): /WP and /HOLD functions are unavailable. Hardware
  write protection via /WP + SRP therefore does not exist on this board; software protection only.
- **"IM"/"JM" parts have QE = 0** and /HOLD active (p.17). A substitution to an "M" ordering code
  would need pull-ups on IO2/IO3 for single/dual modes (the RP2350's reset pull-ups cover SD2/SD3
  only until software reconfigures the pads) and a QE write for quad. Pin the MPN.
- **Pin 7 can be /RESET** on some packages; on SOIC-8 the /HOLD-or-/RESET choice is the HOLD/RST
  bit (Status Register-3, p.18), and with QE = 1 the pin is IO3 regardless (p.6 note 2).
- **Write-status time 10–15 ms, sector erase 45–400 ms, chip erase 40–200 s** (p.66): UF2
  programming of a 16 MB image takes a while; not a hardware issue.
- **VCC during read must stay within ±10 % of the programming voltage** (p.61 note 1); one rail,
  so automatically true.
- **Datasheet decoupling silence**: the 100 nF comes from the vendor design, not Winbond.
- **Guide placement note**: R1/R6 (and R9/R10) "should be placed close to the flash chip, so we
  avoid additional lengths of copper tracks" (p.12) — applies to R_BOOTSEL.

## Schematic connections

| Part / pin | Net | Note |
|---|---|---|
| U5 1 (/CS) | QSPI_SS | U1 pin 60; also R_BOOTSEL (user-io.md); no pull-up |
| U5 2 (DO/IO1) | QSPI_SD1 | U1 pin 59 |
| U5 3 (/WP/IO2) | QSPI_SD2 | U1 pin 58; no pull-up |
| U5 4 (GND) | GND | |
| U5 5 (DI/IO0) | QSPI_SD0 | U1 pin 57 |
| U5 6 (CLK) | QSPI_SCLK | U1 pin 56 |
| U5 7 (/HOLD/IO3) | QSPI_SD3 | U1 pin 55; no pull-up |
| U5 8 (VCC) | 3V3 | |

| Passive | Value | Voltage / power | Dielectric / tol | LCSC | Footprint | Connects |
|---|---|---|---|---|---|---|
| C_FLASH | 100 nF | 16 V | X7R ±10 % (CL05B104KO5NNNC) | C1525 (basic) | `Capacitor_SMD:C_0402_1005Metric` | 3V3 – GND at U5 pin 8 |

## Layout guidance

From the guides:
- QSPI pins "wired directly to the flash, using short connections to maintain the signal integrity,
  and to also reduce crosstalk in surrounding circuits" (guide p.11).
- Pull-up and BOOTSEL resistors close to the flash (guide p.12).
- architecture.md: "short and away from VREG_LX".

Ours:
- U5 next to U1's QSPI corner (pins 55–60), all six nets ≤ 20 mm and roughly equal (± 5 mm); one
  layer where possible, over unbroken L2 GND; no vias in QSPI_SCLK if avoidable.
- Keep QSPI_SCLK away from XIN/XOUT and from VREG_LX/L1 (the guide's regulator-layout warnings
  apply to everything near L1).
- C_FLASH within 1 mm of pin 8, GND via at its pad.
- R_BOOTSEL at U5 pin 1. R_BOOTSEL isolates the button trace from
  QSPI_SS, so the button itself may sit at the board edge.
- No test points on QSPI nets (they load the 25 % driver). QSPI_SS is reachable via the button.

## Open questions

1. Confirm the RP2350 QSPI pad drive default (core audit) for the button-held case (≥ 4 mA wanted).
2. ~~DNF CS pull-up footprint~~ Resolved: dropped for space, logged in decisions.md (review #4).
3. SDK default QSPI clock (75 MHz) is from memory; check the pico-sdk version used at bring-up.

## Findings

| Rank | Finding | Action |
|---|---|---|
| NOTE | W25Q128JVSIQ is the vendor's own part; QE = 1 fixed makes the bootrom's first (EBh quad) attempt succeed; /WP and /HOLD need nothing. | — |
| NOTE | No CS pull-up needed: the RP2350's reset pull-up on QSPI_SS is on the same rail, the flash ignores writes for 5 ms after VWI, and the bootrom provides the high-to-low transition; the vendor found it unnecessary for this part. No footprint either (no space; decisions.md). | A flash substitution must repeat the check. |
| NOTE | Vendor R10 (0 Ω) omitted: only for RP2354 secondary-flash use. | — |
| NOTE | Footprint `SOIC-8_5.3x5.3mm_P1.27mm` matches package code S (208 mil, p.68). Symbol pins match p.6. | — |
| NOTE | Do not accept "IM"/"JM" substitutes (QE = 0, /HOLD active). | Pin the MPN in the BOM. |
