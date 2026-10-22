# Board project

This repo is a PCB project run with the `pcb-design` skill in
`.claude/skills/pcb-design/`. Load it for any board work and follow its phases
and gates. Start every session by reading the skill's `LESSONS.md` and
`design/status.json`, and resume from the last passed gate in `design/gates/`.

- KiCad projects live in `hw/<board>/<variant>/`.
- The project page is `index.html` (status from `design/status.json`, plus the
  embedded board viewer). Serve it with `./serve.sh --port N`.
- The viewer is pcbview, pinned at `vendor/pcbview`. Build with
  `scripts/view.sh build pcbview.toml`. Never edit files under `vendor/` or `viewer/`.
- Update `design/status.json` and rebuild the viewer at every gate.
- When the user corrects you, add the rule to `.claude/skills/pcb-design/LESSONS.md`.
