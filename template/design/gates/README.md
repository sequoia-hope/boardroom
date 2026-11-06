# Gates

One file per phase, `phaseN.md`: the phase's checklist, the evidence for each
item (actual command output, calculations, renders), and the independent
reviewer's findings with their resolutions. A phase is not done until its gate
file exists and has been reviewed.

When gate `N` passes, record the inputs it judged beside `phaseN.md`:

```sh
python3 .claude/skills/pcb-design/scripts/gate_inputs.py record N
```

This writes `phaseN.inputs.json` so Boardroom and `gate_inputs.py check` mark
the gate (and any later passed gates) stale when one of its inputs changes.
