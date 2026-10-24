# Architecture: easy2350

Phase 1. Each block below is one region of the schematic and one region of the
placement. Part audits are in `parts/`, one file per block.

```mermaid
flowchart LR
  J1["J1 USB-C<br/>CC: 5.1 k Rd"] -->|VBUS| ESD["U2 USBLC6-2SC6<br/>ESD on D+/D−/VBUS"]
  J1 -->|D+ D−| ESD
  ESD -->|VBUS| LDO["U3 AP7361C-33E<br/>LDO 3V3"]
  ESD -->|VBUS| DLED["D2 SM4007PL<br/>VBUS → VLED"]
  ESD -->|D+ D− via 27 Ω| MCU
  LDO -->|3V3| MCU["U1 RP2350A<br/>internal SMPS → 1V1 (L1)"]
  LDO -->|3V3| FLASH["U5 W25Q128JV<br/>16 MB QSPI"]
  XTAL["Y1 12 MHz"] --> MCU
  MCU <-->|QSPI| FLASH
  BTN["SW1 BOOTSEL<br/>SW2 RESET"] --> MCU
  MCU -->|GPIO25| D1["D1 LED"]
  MCU -->|GPIO23| BUF["U4 SN74LV1T125<br/>3.3 V → VLED logic"]
  DLED -->|VLED| BUF
  DLED -->|VLED| PIX["D3–D6 WS2812B-2020-V6<br/>4 RGB pixels"]
  BUF -->|DIN| PIX
  MCU <-->|GPIO0–29, RUN| HDR["J2, J3 1×20 headers<br/>+ 3V3, VBUS, GND"]
  MCU <-->|SWD| J4["J4 1×3 SWD"]
```

## Blocks

| Block | Parts | Audit |
|---|---|---|
| USB front end | J1, CC resistors, U2, 27 Ω series pair | `parts/usb-front-end.md` |
| 3V3 regulator | U3, its input and output capacitors | `parts/ldo-3v3.md` |
| RP2350 core | U1, L1 and the regulator passives, decoupling, Y1 and its load, BOOTSEL/RUN | `parts/rp2350-core.md` |
| Flash | U5 and its decoupler | `parts/flash.md` |
| RGB LEDs | D2, U4, D3–D6 and their decouplers | `parts/rgb-leds.md` |
| User I/O | SW1, SW2, D1 and its resistor, J2–J4, test points | `parts/user-io.md` |

## Signal and power paths

- **VBUS** (4.5–5.5 V at J1) feeds three loads: the LDO, the LED rail through
  D2, and header pin VBUS. The USB 2.0 attach limit caps everything on VBUS at
  10 µF.
- **3V3** feeds the RP2350 (IOVDD, VREG_VIN, VREG_AVDD through its RC,
  ADC_AVDD, USB_OTP_VDD, QSPI_IOVDD), the flash, D1, and header pin 3V3.
- **1V1** is made inside the RP2350 and only goes to its DVDD pins. It never
  leaves the chip's corner of the board.
- **VLED** is VBUS less a Schottky drop. It only feeds U4 and the four pixels.
- **USB D+/D−**: J1 → U2 (in-line) → 27 Ω at U1 → U1.
- **QSPI**: U1 ↔ U5, six lines, short and away from VREG_LX.
- **GPIO**: every GPIO goes straight to the headers. GPIO25 also drives D1;
  GPIO23 also drives U4's input. Either still works as a header GPIO, with the
  LED or pixel chain responding.
