# Boardroom

Boardroom is becoming one program with many crates: Projects, Compose and Route as modes of
one app, with the router rewritten here. **Read `docs/one-system-spec.md` before any work on
the code**: it has the layout, the dependency rules, the order of work and where each crate
comes from. Nothing has to keep working during the migration, but `main` builds at every commit
and each step's commit message says what is broken.

This repository holds the Cargo workspace (`crates/`: `project`, a board project on disk;
`server`, the HTTP server; `boardroom`, the one binary, whose commands are functions in
`cli.rs`; `kicad`, the only reader and writer of KiCad files; `board`, the board model with the checker;
`intent`, `recognise`, `library`, `interfaces` and `examine` (the judge, from Composer's verifier);
`plan`, the new router's first stage, a prototype: each connection as a strand through the gaps between obstacles, before any copper;
`wire`, its second: each strand drawn alone as 0/45/90 pieces held by the corners it passes, kept only if the checker passes it, and the board planned again with what drawing showed (`wire::route`);
`scene`, plain drawing data and its mesh; `render`, a board as a scene, with the WebGL renderer in
`crates/render/js`; `wasm`, the browser module, built by `web/scripts/wasm.sh`; `gtk`, the native
window; `gpl/kicad-text`, the GPL text program that is run and never linked; `crates/boardroom/tests/layers.rs` enforces the spec's
dependency rules, so a new crate must be added to a group there), the web app (`web/`, with the shared store in `web/store/`), the
viewer (`view/`, pcbview, run by `scripts/view.sh`), Composer's chip packages and research library (`packages/`, `library/`), the design specs (`docs/`), the upstream
`pcb-design` skill (`.claude/skills/pcb-design/`), the board scripts
(`scripts/{doctor,view,route}.sh`), the board-project skeleton (`template/`, instantiated by
`scripts/new-board.sh`), and the worked example (`examples/easy2350/`). The root itself is not
a board project (`template/CLAUDE.md` is the board-repo version).

- **Build and serve:** `./build.sh` (book cores with `reserve`) and `./serve.sh --port N` (never
  hardcode a port). The toolchain is pinned in `rust-toolchain.toml`. `cargo build` builds the
  server; the window is `cargo build -p boardroom-gtk`.
- **Checks:** `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`
  (booked with `reserve`), and in `web/` `npm run test:store`, `test:stale` and `test:board`
  (after `scripts/wasm.sh` and `npm run build:pages`). `BOARD_SHOT=out.png npm run test:board`
  saves a picture of the Board tab: look at it after changing what is drawn.
- **There is no router here yet.** `scripts/route.sh` needs `$ULTRAROUTE` (a built binary of the
  old router) until the new one exists. The old router's code is never brought in or built here.
- **Making a board repo:** run `scripts/new-board.sh <dir> [--name <title>] [--github <owner>/<repo> [--private]]`,
  which copies `template/`, the upstream skill and `scripts/{doctor,view,route}.sh`, and adds
  `vendor/pcbview` and `vendor/ultraroute` submodules at the last pins of the old repositories
  (board repos stay on those until `boardroom new` replaces the script).
- **Board work** happens in board repos or in `examples/easy2350/`. Load the
  `pcb-design` skill in `.claude/skills/pcb-design/` for any board work and follow its
  phases and gates. Start every board session by reading `.claude/skills/pcb-design/LESSONS.md`
  and the project's `design/status.json`, and resume from the last passed gate in
  `design/gates/`.
- In a board project (and `examples/easy2350/`), KiCad projects live in
  `hw/<board>/<variant>/`, and the static project page is `index.html` (status from
  `design/status.json` plus the embedded viewer; served in board repos with
  `./serve.sh --port N`).
- The viewer is pcbview, in `view/` here (in a board repo, pinned at `vendor/pcbview`). Build the
  example's viewer with `scripts/view.sh build examples/easy2350/pcbview.toml` (or
  `scripts/view.sh build pcbview.toml` in a board repo). Never edit generated files under
  `viewer/`, or `vendor/` in a board repo. `view/` is Boardroom's own code now and is edited here.
- `README.md` is the suite's map for agents (projects, repos, pages, board-repo layout).
  Keep it true when the suite changes.
- Update `design/status.json` and rebuild the viewer at every gate during board work.
- When the user corrects you, add the rule to `.claude/skills/pcb-design/LESSONS.md`.
