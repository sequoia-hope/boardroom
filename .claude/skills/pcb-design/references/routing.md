# Phase 7: routing

Claude is bad at hand-routing by picking coordinates. So route in this order, and let tools do as much as possible.

## Order

1. **Planes and pours.** GND plane (L2 on 4-layer), power planes/pours, and POWER_HI copper as zones, not traces. Set zone priorities and clearances. Pours are drawn as polygons around regions, which is far more reliable than tracing paths.
2. **Via arrays** for every net with a via count from `nets.csv`: stitch power pours between layers at the points where current transfers, with the computed count. Place as arrays (script-generated grids within a zone), not single vias.
3. **Short critical connections by hand, only when short.** Gate drive, crystal, shunt Kelvin sense, switch node: these should be a few mm thanks to placement. Route them directly with straight or single-bend tracks.
4. **Differential and impedance-controlled pairs** with the tool's diff pair router or interactive router; keep on one layer over solid reference; length match per rules.
5. **Autorouter for everything else** (Freerouting for KiCad, Situs/ActiveRoute for Altium) with full net class rules loaded. Lock everything routed in steps 1–4 first so the autorouter can't disturb it. Always run it governed; see "Running the autorouter without hurting the host" below.
6. **Cleanup**: remove autorouter artifacts (acute angles, unnecessary vias, long detours), re-pour zones.
7. **Ground stitching vias** around the board edge and between ground pours, and around RF/sensitive areas.

## Running the autorouter without hurting the host

Autorouters default to taking the whole machine: Freerouting's `-mt` defaults to one fewer than the number of cores, with no memory or time bound. Run it unthrottled on every new project and it pegs the desktop. Every autorouter run is governed. No exceptions, including "just a quick test".

- **One autorouter run at a time per machine.** Never launch routes in parallel to try variants; queue them.
- **Low priority:** `nice -n 15`.
- **Threads:** at most half the cores, e.g. `-mt $(( $(nproc) / 2 ))`. Use `-mt 1` unless you have a measured reason.
- **Memory:** a hard ceiling via a systemd scope, plus a JVM heap cap for Freerouting.
- **Time:** a wall-clock budget (`timeout`). Start with 10 minutes; raise it only if a run times out with progress still being made.
- **Good enough is done:** stop at 0 unrouted and a clean DRC. Don't burn extra passes chasing via counts.

Freerouting:

```bash
systemd-run --user --scope --quiet -p MemoryMax=6G -p CPUQuota=$(( $(nproc) / 2 * 100 ))% \
  nice -n 15 timeout 600 \
  java -Xmx4g -jar freerouting.jar -de board.dsn -do board.ses -mp 100 -mt 1
```

If copperroute is installed, it governs itself by default: niceness 10, half the cores, and a memory ceiling of half the RAM available at start. At that ceiling it stops routing and still writes the session. Give it an explicit budget anyway:

```bash
copperroute route board.kicad_pcb --kicad-project board.kicad_pro -o board.ses \
  --timeout 600 --max-passes 100 --max-memory 4G
```

Its environment overrides are `COPPERROUTE_NICE`, `COPPERROUTE_MAX_MEMORY` and `COPPERROUTE__ROUTER__MAX_THREADS`. Never set them to remove the limits (`--max-memory off`, `--nice 0`) unless the user asks.

If a run hits its time or memory budget, report that, along with the unrouted count. Don't silently retry with bigger limits. Usually the fix is placement, rules or layer count, not more compute.

## Rules of thumb

- Every signal has a continuous reference plane under it. No high-speed or SE_HS trace crosses a plane split. If a layer change is needed, put a ground via next to the signal via.
- Keep the area of high di/dt loops minimal. Return current flows directly under the trace on the plane.
- No traces under switch nodes on the adjacent layer except ground.
- Analog and Kelvin sense lines as tight pairs, away from switching copper.
- Thermal reliefs off for POWER_HI connections, on for hand-soldered pins and big ground pads that need to be solderable.
- Teardrops on (where supported).

## Checks

- DRC: **0 errors, 0 unrouted**. Paste the report.
- Schematic parity: 0 differences.
- Every DRC warning is explained in the gate file, or fixed.
- Current capacity check script: for each POWER/POWER_HI net, the narrowest copper neck along the path and the via count at each layer transition meet `nets.csv` derived values.
- Plane integrity script: report splits and slots in GND plane, and any DIFF_HS/SE_HS trace that crosses one.
- Visual review on per-layer copper renders.

## Gate checklist (phase 7)

- [ ] DRC 0 errors, 0 unrouted (pasted)
- [ ] Parity 0 differences
- [ ] Current capacity check passes
- [ ] Plane integrity check passes
- [ ] Every warning explained
- [ ] Per-layer renders reviewed
- [ ] PCB view regenerated with copper viewer
