# Boardroom

This repository holds the Boardroom app (`app/`), its design specs (`docs/`), the
upstream `pcb-design` skill (`.claude/skills/pcb-design/`), the shared board
tooling (`scripts/{doctor,view,route}.sh` and pinned `vendor/{pcbview,ultraroute}`
submodules), the board-project skeleton (`template/`, instantiated by
`scripts/new-board.sh`), and the worked example (`examples/easy2350/`). The root
itself is no longer a board project (`template/CLAUDE.md` is the board-repo version).

- **App (`app/`):** build with `app/build.sh` and serve with `app/serve.sh --port N`
  (never hardcode a port).
- **Making a board repo:** run `scripts/new-board.sh <dir> [--name <title>] [--github <owner>/<repo> [--private]]`,
  which copies `template/`, the upstream skill, `scripts/{doctor,view,route}.sh`, and
  the pinned `vendor/pcbview` and `vendor/ultraroute` submodules.
- **Board work** happens in board repos or in `examples/easy2350/`. Load the
  `pcb-design` skill in `.claude/skills/pcb-design/` for any board work and follow its
  phases and gates. Start every board session by reading `.claude/skills/pcb-design/LESSONS.md`
  and the project's `design/status.json`, and resume from the last passed gate in
  `design/gates/`.
- In a board project (and `examples/easy2350/`), KiCad projects live in
  `hw/<board>/<variant>/`, and the static project page is `index.html` (status from
  `design/status.json` plus the embedded viewer; served in board repos with
  `./serve.sh --port N`).
- The viewer is pcbview, pinned at `vendor/pcbview`. Build the example's viewer with
  `scripts/view.sh build examples/easy2350/pcbview.toml` (or `scripts/view.sh build pcbview.toml`
  in a board repo). Never edit files under `vendor/` or `viewer/`. Viewer gaps
  go to the board's `design/pcbview-requests.md` and to `sequoia-hope/pcbview`.
- `README.md` is the suite's map for agents (projects, repos, pages, board-repo layout).
  Keep it true when the suite changes.
- Update `design/status.json` and rebuild the viewer at every gate during board work.
- When the user corrects you, add the rule to `.claude/skills/pcb-design/LESSONS.md`.
