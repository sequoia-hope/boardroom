# Boardroom

Boardroom is becoming one program with many crates: Projects, Compose and Route as modes of
one app, with the router rewritten here. **`governance/ENGINEERING_CONSTITUTION.md` is the law
for how work is done here and overrides this file: read it at the start of every session.** Its
core: look at the board (the designer's beside ours, every layer that matters) before settling
on a cause, before claiming a success, and before asking the owner anything. **Read `docs/one-system-spec.md` before any work on
the code**: it has the layout, the dependency rules, the order of work and where each crate
comes from. Nothing has to keep working during the migration, but `main` builds at every commit
and each step's commit message says what is broken.

This repository holds the Cargo workspace (`crates/`: `project`, a board project on disk;
`server`, the HTTP server; `boardroom`, the one binary, whose commands are functions in
`cli.rs`; `kicad`, the only reader and writer of KiCad files; `board`, the board model with the checker;
`intent`, `recognise`, `library`, `interfaces` and `examine` (the judge, from Composer's verifier);
`plan`, the new router's first stage, a prototype: each connection as a strand through the gaps between obstacles, before any copper, two of one net beside each other as one track, a net a custom rule holds to a length laid first and never longer than that, and a connection with no way given one by moving the strands that shut it in;
`wire`, its second: each strand drawn alone as 0/45/90 pieces held by the corners it passes, kept only if the checker passes it (wires drawn earlier that are in its way drawn again round it where they can be), a connection that cannot be drawn laid again narrower and then another way, a connection the plan finds no way for, at first or after others are laid again, given a hole in its pad where the pad is bare (no solder paste on it), or the board is made with its vias filled and capped (six copper layers or more, as JLCPCB makes them unasked, or `--filled-vias yes`), and can hold one (a via inside the pad, its ring past the sides of a pin narrower than itself where vias are filled, its track on another layer) and laid again, a poured net left to its pour (and a connection the pours leave open again laid again; one whose net has an inner plane nothing reaches yet goes down to that plane by a via of its own; two pours of a net still apart are joined by a via where one lies over the other), a pad the wires leave too few thermal spokes given one drawn by hand, or the pour its spokes end in a via to the net's pour on another layer, or those wires laid another way, and a board routed two more ways beside the first, with each fine-pitch part's way out settled first, a row of vias for each row of its pads (spread along the row, or at the pads' tips staggered as a designer places them, a pad whose tip has a pad of the other side opposite it given the nearest place beside), the one leaving least open kept, and a way well behind the others once its pours are joined dropped before its end (`wire::route`, on three threads and one more for each layer);
`route`, the router's top level: a board file in, routed copper, pictures and a report out (`boardroom route`), the checker's verdict on a board (`boardroom check`), and each pour's pieces and necks (`boardroom pours`);
`packages`, `packages/` as the source of boards' blocks: a layout's copper and rule areas round its chip (`areas.json`) placed on a board as zones and `.kicad_dru` rules, compared with the board's copy and accepted or declined (`boardroom blocks`, pins in `<board>.blocks.json`), and a pour it keeps joined checked section by section;
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
- **The new router is `boardroom route <board.kicad_pcb> [--out DIR]`** (default `route/<board>/`
  in the board's project; it keeps each pour the board's pinned blocks keep joined whole, from
  `packages/` or `--packages DIR`; `--filled-vias yes|no` says whether the board is ordered with
  its vias filled and capped, so that one may stand in a soldered pad, where its layer count
  does not: the report counts those vias and notes that the board must be ordered so; while it
  runs it keeps `partial/` in its output, a `trace.log` of its stages and the board so far under
  a new number at least every ten minutes or `--save-every SECONDS`, so a run stopped or killed
  has left what it had, and a run that ends leaves `trace.log`); `boardroom check <board.kicad_pcb>` gives our checker's faults and open
  connections. It is a prototype: judge it with `bench/tier.py clean` (KiCad's DRC beside ours, on
  the boards whose designer's own finished board passes that DRC: the owner's test set since
  2026-11-22, and no design rule is relaxed to get a board through), and after a run of it
  rebuild the gallery the owner reviews (`bench/gallery.py`, pages in the lab).
  Board repos' `scripts/route.sh` still runs the old router (`$ULTRAROUTE`, a built binary); the
  old router's code is never brought in or built here.
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
