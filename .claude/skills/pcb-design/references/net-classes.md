# Phase 3: net classification

After the schematic and before placement, classify every named net in `design/nets.csv`. This table is the single source of truth for net classes, trace widths, clearances, via counts, plane assignment, placement priorities and which simulations are needed. Nothing downstream is decided ad hoc.

## Format

```csv
net,class,v_max,i_cont,i_peak,impedance,length_limit,reason
+48V,POWER_HI,60,20,40,,,"Motor bus, 20 A continuous from intent.md"
MOTOR_A,SWITCH_HI,60,20,40,,,"Bridge phase output, fast dV/dt"
SW_BUCK,SWITCH,24,2,3,,,"Buck switch node, keep minimal area"
+3V3,POWER,3.3,0.5,0.8,,,"Logic rail"
ISENSE_A_P,ANALOG_DIFF,0.2,,,,,"Shunt Kelvin sense, pair with ISENSE_A_N"
USB_DP,DIFF_HS,3.3,,,90 diff,,"USB FS/HS"
I2C_SDA,SIGNAL,3.3,,,,,"Slow bus"
```

Unnamed local nets inherit `SIGNAL` unless they connect to a power pin or carry significant current; check them.

## Classes

| Class | Typical examples | Drives |
|-------|------------------|--------|
| POWER_HI | motor bus, battery, > 3 A | pours/planes, wide copper, via arrays, current-calculated widths |
| POWER | logic rails | planes or wide traces, standard via count |
| SWITCH_HI | half-bridge phase nodes | short wide copper, keep away from everything, minimal loop area |
| SWITCH | buck/boost switch nodes | minimal area, no traces underneath on adjacent layer except ground |
| GATE | gate drive | short, paired with return (source/Kelvin), low inductance |
| ANALOG / ANALOG_DIFF | sense, references, ADC inputs | guard from switching, Kelvin connections, differential pairs |
| CLOCK | crystals, oscillators | very short, guarded, no vias if possible |
| DIFF_HS | USB, Ethernet, LVDS, etc. | impedance-controlled pairs, length matching, reference plane continuity |
| SE_HS | fast single-ended (SPI > 20 MHz, QSPI flash) | controlled impedance if long, series termination |
| RF | antennas, RF front end | 50 Ω coplanar, keepouts, sim |
| HV | > 50 V or mains | creepage and clearance per standard |
| SIGNAL | everything else | defaults |

## Derived values (computed by script from the CSV and stackup)

- **Trace width** from i_cont using IPC-2152 with a temperature rise target (default 10 °C), for the actual copper weight and layer (internal layers run hotter).
- **Via count**: per-via current capacity from drill size and plating thickness; count = ceil(i_peak / per-via capacity × safety factor 1.5). Record this per net.
- **Clearance** from v_max per IPC-2221 (and fab minimums).
- **Simulation triggers**: any DIFF_HS, SE_HS over a length threshold, RF, SWITCH_HI near ANALOG, or POWER_HI with tight drop budget.

## Gate checklist (phase 3)

- [ ] Every named net in the schematic is in `nets.csv` (script check, both directions)
- [ ] Every row has a class and reason
- [ ] Currents trace back to `intent.md` or part audits
- [ ] Derived widths and via counts computed and pasted
- [ ] User has glanced at the table (send them the PCB view link)
