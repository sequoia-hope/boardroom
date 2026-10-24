# Gate 3: net classification

Status: **passed** (2026-10-23)

`design/nets.csv`: every named net with class, voltage, current, impedance,
length limit and a reason. Checked with `nets_check.py --rules` against the
KiCad net classes written by `scripts/project.py`:

```
nets_check: /home/sequoia/Software/easy_pcb/examples/easy2350/hw/easy2350/a
  61 named nets in easy2350.kicad_sch, 61 rows in nets.csv

  derived (mm; 1 oz outer / 0.5 oz inner, dT 10 C, 0.3 mm via = 0.80 A, x1.5 safety, clearance IPC-2221 B2)
    net                class           V     I   Ipk   w_out   w_in vias   clr  kicad class
    VBUS               POWER         5.5  0.50  0.75   0.090  0.115    2  0.10  POWER
    +3V3               POWER         3.6  0.28  0.40   0.090  0.090    1  0.10  POWER
    +1V1               POWER         1.3  0.20  0.40   0.090  0.090    1  0.10  POWER
    VLED               POWER         5.3  0.16  0.20   0.090  0.090    1  0.10  POWER
    GND                POWER         0.0  0.50  1.00   0.090  0.115    2  0.10  POWER
    VREG_LX            SWITCH        3.6  0.20  0.40   0.090  0.090    1  0.10  SWITCH
    VREG_AVDD          POWER         3.6  0.01  0.02   0.090  0.090    1  0.10  POWER
    USB_DP_C           DIFF_HS       3.6     -     -       -      -    1  0.10  DIFF_HS
    USB_DM_C           DIFF_HS       3.6     -     -       -      -    1  0.10  DIFF_HS
    USB_DP             DIFF_HS       3.6     -     -       -      -    1  0.10  DIFF_HS
    USB_DM             DIFF_HS       3.6     -     -       -      -    1  0.10  DIFF_HS
    USB_DP_MCU         DIFF_HS       3.6     -     -       -      -    1  0.10  DIFF_HS
    USB_DM_MCU         DIFF_HS       3.6     -     -       -      -    1  0.10  DIFF_HS
    QSPI_SCLK          SE_HS         3.6     -     -       -      -    1  0.10  SE_HS
    QSPI_SD0           SE_HS         3.6     -     -       -      -    1  0.10  SE_HS
    QSPI_SD1           SE_HS         3.6     -     -       -      -    1  0.10  SE_HS
    QSPI_SD2           SE_HS         3.6     -     -       -      -    1  0.10  SE_HS
    QSPI_SD3           SE_HS         3.6     -     -       -      -    1  0.10  SE_HS
    QSPI_SS            SE_HS         3.6     -     -       -      -    1  0.10  SE_HS
    XIN                CLOCK         3.6     -     -       -      -    1  0.10  CLOCK
    XOUT               CLOCK         3.6     -     -       -      -    1  0.10  CLOCK
    XOUT_R             CLOCK         3.6     -     -       -      -    1  0.10  CLOCK
    CC1                SIGNAL        5.5     -     -       -      -    1  0.10  SIGNAL
    CC2                SIGNAL        5.5     -     -       -      -    1  0.10  SIGNAL
    RUN                SIGNAL        3.6     -     -       -      -    1  0.10  SIGNAL
    SWCLK              SIGNAL        3.6     -     -       -      -    1  0.10  SIGNAL
    SWDIO              SIGNAL        3.6     -     -       -      -    1  0.10  SIGNAL
    LED_DIN            SIGNAL        5.3     -     -       -      -    1  0.10  SIGNAL
    PIX12              SIGNAL        5.3     -     -       -      -    1  0.10  SIGNAL
    PIX23              SIGNAL        5.3     -     -       -      -    1  0.10  SIGNAL
    PIX34              SIGNAL        5.3     -     -       -      -    1  0.10  SIGNAL

  PASS: 0 errors, 0 warnings
```

(The 30 GPIO rows, all SIGNAL / 3.6 V, are omitted above.)
