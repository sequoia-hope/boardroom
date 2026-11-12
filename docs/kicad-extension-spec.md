# Boardroom for KiCad: a standalone KiCad extension — spec (draft)

Status: draft, 2026-11-07. Written from the owner's question "how would we package Boardroom
into a KiCad extension?" and their answer that it is **for KiCad users who don't have Boardroom**,
accepting that this is more work. Nothing here is built. Choices the owner has not confirmed are
marked **Recommended**, and every open one is under **Open questions**. UltraRoute and Circuit
Composer have not reviewed their requests ("Requests to other projects").

Builds on [`shell-spec.md`](shell-spec.md) (the server, the bus, jobs and the lock) and
[`platform-spec.md`](platform-spec.md) (the file seam between the apps).

## The idea in one paragraph

A KiCad user installs one package from KiCad's Plugin and Content Manager and gets Boardroom:
the server, the web app, UltraRoute and its Studio, and Composer, built for their platform, with
nothing else to install and no Rust, Node or git needed. A small **bridge plugin** puts
Boardroom's buttons on KiCad's toolbars, starts Boardroom on a port the OS picks, opens the
KiCad project in it, routes the open board with UltraRoute and lands the result **in the open
board as one undoable edit**, and joins Boardroom's bus so selection travels both ways. Boardroom
still runs as its own local web app in a browser window; it does not move inside KiCad.

## The owner's direction

1. Package Boardroom as a KiCad extension.
2. It is for KiCad users who don't have Boardroom: the package carries everything. More work is
   accepted.

## Why a bridge and not a port

- KiCad extensions are processes that KiCad launches and that talk to it over a socket (the IPC
  API, KiCad 9 and later). Nothing requires a plugin's UI to live inside KiCad, and Boardroom's
  UI is a web page with a server behind it. Rebuilding it in wxWidgets would be a second app.
- The real gain is the seam. Today Boardroom and UltraRoute edit `.kicad_pcb` files on disk, and
  the shell spec has to refuse a job whose write scope includes a board KiCad has open
  (shell-spec.md, "The job holds the project while it runs"). Through the IPC API a route lands
  in KiCad's live copy of the board, and the file changes only when the person saves.

## What KiCad provides (checked on this machine, KiCad 9.0.8)

From `/usr/share/kicad/schemas/api.v1.schema.json` and `pcm.v1.schema.json`:

- **A plugin** is a folder with a `plugin.json`: `identifier`, `name`, `description`, `runtime`
  and `actions`.
  - `runtime.type` is `python` or `exec`. A Python plugin gets a virtualenv that KiCad builds
    from `requirements.txt`, using the interpreter set in Preferences.
  - Each action has an `entrypoint`, an optional toolbar button (`show-button`), light and dark
    PNG icons, and `scopes`: `pcb`, `schematic`, `footprint`, `symbol`, `project_manager`.
- **A package** is a zip for the Plugin and Content Manager. Its metadata has `type: plugin` and a
  list of versions. Each version has a `download_url`, `download_sha256`, `download_size`,
  `install_size`, `platforms` (`windows`, `macos`, `linux`), `runtime` (`swig` or `ipc`),
  `kicad_version`, `kicad_version_max` and `keep_on_update` (files kept across updates).
  Packages come from a repository: KiCad's official one, or any repository JSON a person adds by
  URL.
- **The API server** must be on (Preferences → Plugins). On this machine it is
  (`kicad_common.json`: `api.enable_server: true`), but a new user's KiCad may have it off.
- **The client library** is `kicad-python` (imported as `kipy`), installed into the plugin's
  virtualenv.
- **The old SWIG `ActionPlugin`** (`pcbnew` module) is deprecated. Nothing here uses it.

### To verify in M0

These are believed true and decide the design, so M0 checks each one before anything else is
built:

1. `kipy` can name the open board's file and project directory.
2. `kipy` can read the selection, set it, create and delete tracks, arcs and vias in one commit
   that KiCad's undo reverts in one step, refill zones, and save.
3. A plugin process may keep running after its action returns, for as long as KiCad does.
4. Whether the API can notify selection changes or only be polled.
5. KiCad's item IDs match the `uuid` fields in the `.kicad_pcb` file.
6. What the API covers in the schematic editor in KiCad 9, and what KiCad 10 adds.
7. **Binaries in a package survive installation:** the Plugin and Content Manager keeps the
   executable bit on Linux and macOS, and macOS's quarantine doesn't stop a binary the plugin
   starts. If either fails, the plugin sets the bit itself and the macOS build is signed and
   notarized (open question 4).
8. **How large a package may be** in KiCad's official repository, and whether it accepts native
   binaries at all.
9. **A plugin can find KiCad's own `kicad-cli`** (for the DRC that judges every route), on all
   three platforms.
10. **Turning the API on:** if a new user's API server is off, what a package can do about it
    besides telling them where the setting is.

## What exists today

- Boardroom's server and web app (`app/`): `boardroom serve --port N`, `boardroom open`,
  `boardroom selection`. The port comes from `--port`, `$PORT` or `proj port`, else the server
  fails.
- The bus (`/api/bus`) with `select {project, board, refs[], nets[]}` and `focus` events, and
  `/api/selection`.
- Projects come from `~/.config/boardroom/projects.toml`, added by `POST /api/projects`.
- **Boardroom expects its own layout:** KiCad projects in `hw/<board>/<variant>/` and the design
  record in `design/status.json` (`project.rs`). An ordinary KiCad project (`foo/foo.kicad_pro`)
  matches neither.
- **What the server needs from this machine:**
  - `proj`, to find Studio's and Composer's ports (`apps.rs`).
  - `ccomp`, from `$CCOMP`, `config.toml` or `PATH` (`problem.rs`).
  - `git` (history, push and publish state).
  - `zenity` (the folder picker).
  - `claude` (terminals, through `portable-pty`).
  - Chromium (the app window); the GTK window is Linux-only.
- **Routing:** the skill's `autoroute.py` runs UltraRoute (built from `vendor/ultraroute` by
  `scripts/route.sh`, which needs cargo), judges each seed with `kicad-cli pcb drc`, and writes
  the best result over the board. UltraRoute's CLI writes a whole board (`--output`) and a report
  (`--report`); it has no file listing just what it changed.
- **Studio** is `studio-server --root DIR --port N`, a Rust binary, with its page in
  `studio/web/dist`. **Composer's app** is a static build (`app/dist`), served today by
  `python3 -m http.server`.
- All three repos are MIT-licensed.
- Release sizes on Linux today:

  | Part | Size |
  |---|---|
  | `boardroom` | 8.4 MB |
  | its web build | 3.3 MB |
  | `ultraroute` | 6.4 MB |
  | `studio-server` | 1.5 MB |
  | Studio's page | 3.3 MB |
  | Composer's app | 4.3 MB |
  | `ccomp` | 12 MB |

  That's about 40 MB before compression.

## The package

- **Name:** "Boardroom for KiCad". **Recommended** identifier: `io.github.sequoia-hope.boardroom`.
- **One zip per platform:** `linux-x86_64`, `macos-arm64`, `macos-x86_64`, `windows-x86_64`.
  Each is a separate version entry with its own `platforms` and sha256; `runtime: ipc` and
  `kicad_version: 9.0`.
- **Contents:**

  ```
  plugins/
    plugin.json              the actions (below)
    requirements.txt         kicad-python
    bridge/                  the bridge (Python)
    bin/<platform>/          boardroom, ultraroute, studio-server, ccomp
    web/boardroom/           app/web/dist
    web/studio/              ultraroute's studio/web/dist
    web/composer/            circuit-composer's app/dist
    skill/                   the routing scripts (autoroute.py and what it imports)
    icons/
  resources/icon.png
  metadata.json
  ```

- **Not included:**
  - **The AI side.** The embedded terminals, AI jobs and the `pcb-design` skill need Claude Code,
    which a KiCad user may not have. They appear when `claude` is on the PATH and are hidden
    otherwise.
  - **The GTK window.** Boardroom opens in the system browser.
- **Built by CI, not by hand (Recommended):** a GitHub Actions workflow in this repository builds
  every binary and web page for the four platforms from the pinned submodules, assembles the
  zips with `kicad/package.sh`, attaches them to a GitHub release, and updates the repository
  JSON published with the Pages demo (demo-spec.md). A person adds that one URL in the Plugin and
  Content Manager.
- **Where the source lives (Recommended):** `kicad/` in this repository:
  - `kicad/plugin/`: the bridge, `plugin.json`, icons.
  - `kicad/package.sh`: assembles one platform's zip and prints its sha256.
  - `kicad/repository/`: the repository JSON's template.
  - `.github/workflows/kicad-package.yml`: the CI workflow above.

  Composer would become a pinned submodule like UltraRoute, so a release names one commit of
  each.

## A standalone Boardroom

The bundled server runs where none of this machine's tools exist. It needs a **standalone mode**,
switched on by `--standalone` (the bridge passes it), that changes only what it must:

| Today | Standalone |
|---|---|
| Port from `--port`, `$PORT` or `proj port` | `--port 0`: the OS picks a free port, and the server writes it to its runtime file (below) |
| Studio and Composer reached through `proj port` | Composer's app served as a static build (`AppKind::Static`, like Waffle Iron). Studio started by Boardroom as a child process (`studio-server --port 0`) and proxied as now |
| `ccomp` from `$CCOMP`, `config.toml` or `PATH` | The bundled `ccomp`, passed by path |
| `scripts/route.sh` builds UltraRoute with cargo | The bundled `ultraroute`, through `$ULTRAROUTE`, which `route.sh` already honours. `autoroute.py` runs from the package's `skill/` with the bridge's Python |
| `git` required | Optional. Without it, history, push and publish are hidden, and the rest works |
| `zenity` folder picker | Not needed: KiCad names the project |
| Terminals and jobs through `claude` | Hidden without `claude` on the PATH |
| `~/.config/boardroom/` | The platform's config directory (`dirs::config_dir()`/`boardroom`), so it also works on macOS and Windows |
| Chromium app window or GTK | The system browser |

The runtime file is `<config>/server.json`, holding `{port, pid, version, api}`. It is written
when the server is listening and removed on exit; a file whose `pid` is dead counts as absent.

A port the OS chose is not a hardcoded port and can't collide, so standalone mode keeps the
"never hardcode a port" rule. On this machine, without `--standalone`, nothing changes: `proj`
still owns the port.

### Ordinary KiCad projects

A KiCad user's project is `foo/foo.kicad_pro` with no `hw/` and no `design/`. Boardroom must
open it as it is:

- **A plain project** is any folder whose `.kicad_pro` is at its root. Boardroom shows it as one
  board with one variant. It doesn't require `design/` or `status.json`; the gates, phases and
  needs panes show "not tracked" rather than errors.
- **Nothing is written into the person's project** until they ask for something that needs it.
  Routing records go to `route/` beside the board, as now, and only when they route; the
  plugin's dialog says so the first time.
- **"Track this project"** offers to add `design/` and `status.json` for people who want gates.
  It never moves their KiCad files into `hw/`.

## Actions

| Action | Scopes | What it does |
|---|---|---|
| **Open in Boardroom** | `pcb`, `schematic`, `project_manager` | Starts Boardroom if needed, adds the KiCad project if it isn't listed, opens it in the browser |
| **Route with UltraRoute** | `pcb` | Routes the open board and lands the result as one undoable edit |
| **Route selected nets** | `pcb` | The same, limited to the nets of the selected items; the rest of the board is locked |
| **Show in Boardroom** | `pcb` | Sends KiCad's selection to the bus and brings Boardroom forward (`focus`) |

The schematic scope gets only "Open in Boardroom" until M0 item 6 says the schematic API can do
more.

## The bridge

One long-running plugin process per KiCad instance, started by the first action and ended when
KiCad's socket closes (M0 item 3; if KiCad ends it after each action, cross-probing falls back to
the "Show in Boardroom" button and a "Show from Boardroom" one):

- **Starting Boardroom:** if `server.json` names no live server, it starts the bundled
  `boardroom serve --standalone --port 0` and waits for the file. A server it starts exits 10
  minutes after its last bridge and browser tab disconnect. If the server can't start, the bridge
  shows a KiCad dialog with the server's error.
- **Version handshake:** `server.json`'s `api` is an integer. A bridge refuses a server with an
  unknown `api` (an older Boardroom left running) and offers to restart it.
- **KiCad → bus:** when KiCad's selection changes (an event if the API has one, otherwise a poll
  every 250 ms that sends only on change), it maps the selection to references (footprints) and
  net names (tracks, vias, pads, zones) and sends `select`.
- **Bus → KiCad:** on a `select` from another window it selects those footprints and the nets'
  copper in KiCad, and ignores its own echoes.
- **The lock:** while a Boardroom job holds the project, it refuses "Route with UltraRoute" and
  says which job holds it.

"Each app keeps working with the bus absent" still holds: KiCad is unaffected if Boardroom fails.

## Routing into the open board

1. **Snapshot.** The plugin asks to save the board; if the person declines, it stops, because a
   route of a stale file would mislead. It records every footprint's position.
2. **Route aside.** It calls `POST /api/projects/{id}/route` (new) with the board path and, for
   "selected nets", the net names. Boardroom runs `autoroute.py` as a job with a new
   `--output FILE`, so the result goes to `route/kicad-<job>.kicad_pcb` and the board file is not
   written. The job keeps today's governance, seeds, KiCad judging (with KiCad's own `kicad-cli`,
   M0 item 9) and `route/autoroute.json` record. Progress shows in Boardroom and as a status line
   in the plugin's dialog, with a "Watch in Studio" link.
3. **The changes.** These are the tracks, arcs and vias added, and the unlocked ones removed
   (`--strip`), each with net, layer, width and KiCad ID. They come from UltraRoute's change file
   (below); until that exists, the server diffs the two files by `uuid` with the `kicad-pcb`
   crate from `vendor/ultraroute`.
4. **Check nothing moved.** The plugin compares footprint positions with the snapshot. If
   anything moved in KiCad while the route ran, it names what moved and offers to route again; it
   doesn't apply.
5. **Apply as one commit:** delete the removed items, create the added ones, refill zones, push
   the commit as "UltraRoute: +N tracks, +M vias, −K removed (seed S, O open)". One Ctrl+Z
   takes it all back.
6. **Saving is the person's.** The board shows as modified, as after any hand edit.

The summary quotes the job's KiCad DRC result (signal open, plane open, errors added), not the
router's count. If M0 item 2 fails, routing falls back to "save, route, reload": KiCad reloads
the routed file, with no single undo.

## Requests to other projects

- **UltraRoute:**
  - **A change file:** `ultraroute route --changes FILE`, a JSON of the copper added (net, layer
    or layers, width, drill, points in nanometres, locked flag) and the IDs removed. Until then,
    Boardroom diffs by `uuid`.
  - **`studio-server --port 0`:** bind a port the OS picks and report it (on stdout or in a file
    named by a flag), so Boardroom can start it as a child.
  - **Release builds for macOS (arm64, x86_64) and Windows** of `ultraroute` and
    `studio-server`, with a CI check that they build. The GPU feature stays off.
- **Circuit Composer:**
  - **`ccomp` for macOS and Windows**, with the same CI check.
  - **The app runs from any base path,** since Boardroom serves it at `/apps/composer/` from a
    folder rather than proxying its server.
  - Become a pinned submodule of Boardroom (`vendor/circuit-composer`).
- **pcbview:** none known. Boardroom's Board tab already builds its viewer into `web/dist`.

## Platforms

- **Linux first:** built and tested on this machine.
- **macOS and Windows** follow once their binaries build in CI and M0 items 7 and 9 pass. Known
  work:
  - Signing and notarizing the macOS binaries (open question 4).
  - Windows paths and process handling in the server (the pty code is already `portable-pty`).
  - Paths and the Python launcher in the bridge.
  - A Windows machine or CI runner to test on, since none is on the LAN.

## Testing

- **Unit tests** (Python, no KiCad): selection to references and nets, the change file to `kipy`
  objects, the moved-footprint check, and reading `server.json` (stale pid, unknown `api`).
- **Server tests** (Rust):
  - Standalone mode: `--port 0` and `server.json`, Composer served statically, Studio started as
    a child and proxied.
  - A plain KiCad project opens with nothing written into it.
  - `/route` with `--output` leaves the board file untouched.
  - The diff by `uuid` on `vendor/ultraroute/core/fixtures/parallelogram-p1-66.kicad_pcb`.
- **A clean-machine test:** the package installed into a fresh KiCad 9 in a container or VM with
  no Rust, Node, git, `proj` or Boardroom. "Open in Boardroom" works, and routing works.
- **End to end on this machine**, with KiCad 9.0.8 and the package from a local repository JSON
  (`file://`):
  - A plain KiCad project (a copy of an earring): "Open in Boardroom" opens it, and nothing is
    written into its folder.
  - easy2350: selecting U1 in KiCad highlights U1 in Boardroom's viewer, and the reverse.
  - parallelogram-p1-66 in a scratch project with its tracks stripped: "Route with UltraRoute"
    lands a route that KiCad's DRC finds complete (0 open, as Studio gets), one Ctrl+Z removes it
    all, and the file on disk is unchanged until saved.
  - Moving a footprint while the route runs: the apply is refused and names the footprint.
  - Killing Boardroom mid-session: each button says what happened and offers to restart it, and
    KiCad keeps working.
- Heavy steps (the builds, the routes, the browser suite) go through `~/.claude/bin/reserve`.

## Milestones

- **M0: verify.** A throwaway plugin answers the ten questions under "To verify in M0", and the
  answers are written here.
- **M1: standalone server.** `--standalone`: `--port 0`, `server.json`, Composer static, Studio
  as a child, bundled `ccomp` and `ultraroute`, optional `git`, no `zenity`, platform config dir,
  system browser. Plain KiCad projects. Studio's `--port 0` from UltraRoute.
- **M2: Linux package.** The bridge with "Open in Boardroom", `package.sh`, the repository JSON,
  the CI workflow for Linux. Installs from the Plugin and Content Manager here, and passes the
  clean-machine test.
- **M3: selection.** Selection both ways; "Show in Boardroom".
- **M4: routing into the board.** `/route`, `autoroute.py --output`, the diff, the moved-part
  check, the single commit, "Route selected nets".
- **M5: macOS and Windows.** CI builds from UltraRoute and Composer, signing, testing.
- **M6: UltraRoute's change file** replaces the diff.
- **Then:** KiCad's official repository (open question 2), schematic actions as the API allows.

## Open questions

1. **What a KiCad user sees first.** Boardroom's home is a project list with gates and phases
   made for the `pcb-design` workflow. **Recommended:** opened from KiCad, Boardroom goes
   straight to that project's Board tab and keeps the list one click away.
2. **Which repository?** **Recommended:** self-hosted on GitHub Pages first, and KiCad's official
   repository once all three platforms work and M0 item 8 says it takes a package like this.
3. **The AI side for KiCad users.** Hidden without `claude` (above). Should the package say what
   it would add and how to get it, or stay silent?
4. **macOS signing** needs an Apple Developer account (paid). Without it, macOS users have to
   approve each binary by hand. Get one, or ship Linux and Windows first?
5. **The name** "Boardroom for KiCad", and the identifier.
6. **Saving before a route.** The plugin asks to save first. Routing KiCad's live, unsaved board
   needs the board exported through the API, which may not exist in KiCad 9.
7. **Telemetry and updates.** **Recommended:** none beyond the Plugin and Content Manager's own
   update check.
