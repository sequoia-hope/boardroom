# The working demo: Boardroom, Composer and Route on GitHub Pages

Status: **decided** (2026-10-30; the answers are under Decisions). Built so far: milestones
0–4, the Pages workflow from Boardroom through Composer and Route, milestone 5 (the stale bar, and the Board tab
drawing from the `.kicad_pcb`) and milestone 6 (the same Composer workflow on this machine).

## What the person asked for

A fully working demo on GitHub Pages: open a board in Boardroom, rearrange it in Circuit
Composer, route it in Route (UltraRoute Studio), and see the result back in Boardroom. Changes
are kept in the browser, survive a reload, and are passed from one app to the next. The same
workflow on this machine saves to the project on disk instead.

Composer's own board selector goes away wherever Boardroom opened it. Standalone Composer opens
Boardroom's example projects through the same store, not examples of its own.

The aim is closing the gaps that stop the workflow running entirely in the browser.

## What exists today

| | Local (served from this machine) | Pages |
|---|---|---|
| **Boardroom** | Reads projects from disk; watches them; the bus over a WebSocket | A snapshot (`boardroom snapshot`): `design/`, `viewer/`, and (milestone 2) the KiCad files and Composer's placement problems |
| **Composer** | Static files; (milestone 4) opens a project's board through Boardroom's store and saves it back; (milestone 6) Boardroom's server makes its placement problem on read | Built-in example boards (`problem.json` made by `ccomp` at build time). Saving is "Download placement"; (milestone 4) opens a project's board from the store and saves it back |
| **Route** | Opens a project by path; routes on the server; **Save to project** writes the `.kicad_pcb` | `VITE_LOCAL` build: routes in the tab (WASM), opens files from your computer, **downloads** the routed board; (milestone 3) opens a project's board from the store and saves it back |

Today, nothing passes a change from one app to the next: each app only saves to disk or downloads a file.

## The fact the design rests on

`sequoia-hope.github.io/boardroom/`, `…/circuit-composer/` and `…/ultraroute/studio/` are **one
origin**. Every app, embedded or in its own window, sees the same IndexedDB and the same
`BroadcastChannel`. So on Pages the browser can stand in for both the disk and the bus:

- **The disk** becomes a *workspace* in IndexedDB.
- **The bus** becomes a `BroadcastChannel('boardroom')` carrying the same messages
  (`changed`, `select`).

Not `localStorage`. It holds only about 5 MB, only strings, and it blocks the page while
reading. One board file is 1–2 MB. `localStorage` stays what it is now: per-viewer settings
(window mode, theme, last project).

## The workspace

A project in the browser is the snapshot it came from plus the files changed since:

```
workspace/<project id>/
  base        which snapshot (deploy commit), so a newer deploy can be noticed
  files       path → { bytes, rev, app, time, message }   only the changed files
  history     [ { rev, path, app, time, message } ]       the browser's stand-in for git log
```

Reading a file: the changed copy if there is one, else the snapshot's. Writing a file adds a
revision. The history makes these possible:

- **Undo to any revision** and **Reset to the published board**.
- **Download changes**: a zip of the changed files at their project paths, to unpack over a
  clone, so a demo session can become a real commit.
- **Upload a project folder**: later, not in this pass.

**One workspace per tab.** A tab's workspace id lives in `sessionStorage`, so a reload keeps
it and a new tab starts fresh from the published board. Boardroom passes the id to the apps
it embeds or opens (`?ws=…`), so they share it. The workspaces of closed tabs are cleared
after a week; Download changes is the way to keep one.

`navigator.storage.persist()` is requested on the first save, so the browser doesn't evict the
workspace when space runs low. If that's refused, Boardroom shows a note that changes can be
evicted.

## One store interface, two backends

Every app reads and writes project files through one small module, with the same calls in
every mode:

```js
store.read(path)                       // → bytes, rev
store.write(path, bytes, { app, message, base: rev })   // → new rev, or a conflict
store.history(path?)                   // → revisions
store.watch(fn)                        // changed paths, from any app or window
```

- **Server backend** (local): Boardroom's server, `/api/projects/<id>/files/…` for reads and a
  new write endpoint. Git is the history and the file watcher feeds `watch`.
- **Browser backend** (Pages): the IndexedDB workspace, with `BroadcastChannel` for `watch`.

The module lives in this repo (`app/store/store.js`, one file of plain JS, no dependencies).
Composer and Studio copy it at a pinned commit (an npm git dependency would clone this whole
repo, renders and all). Boardroom opens an app with `?store=<root>` (and `&pid=…&ws=…` in the
browser); `fromLocation()` turns that back into the store. Opened without one, an app keeps its
own demo boards as now.

On the server a revision is a hash of the file's bytes, sent as its ETag; a write is
`PUT /api/projects/<id>/files/<path>` with `If-Match` (or `If-None-Match: *` for a new file),
412 when the file moved on, and the bus then carries
`{type: "changed", project, paths, app, message}`. `GET /api/projects/<id>/history[?path=]` is
what isn't committed, then `git log`. Writes stay inside the project and out of `.git`, and,
like every change, come only from this machine.

**Conflicts.** `write` carries the revision it started from. If the file moved on in the
meantime (you routed in one window and moved a part in another), the write is refused. The app
then says so and offers **Reload** (drop mine) or **Save as a copy** (`…-2.kicad_pcb`), never a
silent overwrite. An app with no unsaved edits reloads by itself when `watch` reports its file.

## The handoffs

The board file, `hw/<board>/<variant>/<board>.kicad_pcb`, is the single truth. Every app reads
it and the last step writes it, so KiCad, a download and every view agree.

1. **Boardroom → Composer.** Composer needs a placement problem (parts, nets, outline, locks)
   for the board. On Pages, `boardroom snapshot` writes `compose/<board>/problem.json` with
   `ccomp floorplan --problem`, next to the board. Locally, Composer's server makes it on open.
2. **Composer saves the board.** It writes `compose/<board>/placement.json` and the `.kicad_pcb`
   itself: `Board.write_kicad_pcb` in its WASM core moves, turns and flips each part with
   ultraroute's `kicad_pcb::place`, which does what pcbnew's `Flip`, `SetPosition` and
   `SetOrientation` do, byte for byte, and keeps every other byte of the file. So a KiCad user
   who never routes still gets a board that opens in KiCad, and pcbnew is not needed.
3. **Route routes.** It reads the `.kicad_pcb` Composer saved, routes it, and **Save** writes
   the `.kicad_pcb`, plus the run report to `route/<board>/report.json`.
4. **Boardroom shows the result.** The overview takes routing counts from the report, as it
   does locally. The history shows "Composer: placement", "Route: routed 83/83".

**What goes stale.** Boardroom's Board tab, Renders and Viewer are drawn from pcbview's
generated data and from images made by KiCad, before publishing. After an edit they show the
published board. Short term, those tabs get a "showing the published board; changed in Route at
14:05" bar. Longer term, the Board tab draws straight from the `.kicad_pcb` with `board-draw`,
the renderer Composer and Studio already share.

## Composer when Boardroom opens it

- `?project=…&board=…` hides the board selector, and the project chrome too when embedded. It
  loads that board from the store, and nothing else.
- If Composer has no problem for the board (no `problem.json`, no server), it says so: "Composer
  can't open bike-tracker yet: its placement problem isn't in the snapshot". It does not fall
  back to a demo board, as it does today.
- Opened with no `?project=` (its own Pages site), it is the standalone demo with its selector,
  as now.

## Locally, the same workflow

- Composer gets the server its spec already describes (milestones A and B in its
  `docs/app-spec.md`): it opens a project path and saves `placement.json` and the `.kicad_pcb`
  through the same core as on Pages, so pcbnew isn't needed in the loop.
- Route's **Save to project** goes through the same `write` call, so it gets the conflict check.
- **Other devices** (a phone on the LAN): today Boardroom refuses Studio's socket from anything
  other than this machine, and Studio loads nothing without its socket. That is why Route was
  blank on your phone. Left as it is: the owner cares about the browser-only workflow, not LAN
  devices.

## Milestones

0. **Composer writes KiCad files.** Done 2026-10-30: `kicad_pcb::place` (ultraroute `0e581c4`),
   held byte for byte to pcbnew 9.0.8 on three real boards, every part moved, turned and flipped
   in two rounds, natively and in WASM; `Board.write_kicad_pcb` in Composer's core
   (circuit-composer `2a7abb1`), matching `apply_placement.py` on easy2350. Not yet: drawing a
   new outline (Composer's own shaped boards), and KiCad 10's own flip, if it differs from 9's.
1. **Store.** Done 2026-10-30: `app/store/store.js` with both backends, the server's write and
   history endpoints, and Boardroom's History tab (Restore, Reset, Download changes on Pages;
   uncommitted files and git's log locally). Boardroom hands its store to the apps it opens.
   Tests: `npm run test:store` (the browser backend in Chrome: revisions, conflicts, a tab's
   workspace across reloads and windows, Restore, Reset, the zip, the sweep; and the server
   backend's protocol) and `cargo test` in `app/server` (writes, refusals, history).
2. **Snapshot carries the boards.** Done 2026-10-30: `boardroom snapshot` copies each variant's
   own `hw/<board>/<variant>/*.kicad_{pcb,pro,dru}` (not the routing runs in folders below it) and
   writes `compose/<board>/problem.json` for the board's last variant: the project's own if it
   has one, else `ccomp floorplan --nets-from-board --outline-from-board --problem` on the board
   (`--ccomp PATH` or `$CCOMP`; the snapshot refuses to run without it). `projects.json` lists
   them per project as `compose: [{board, variant, pcb, problem}]`, with `error` instead of
   `problem` for a board ccomp can't read, which doesn't stop the build. The Pages workflow
   clones and builds ccomp, and runs `npm run test:store` too. easy2350 (58 parts, 62 nets) and
   simple-earring (167, 98) both come through, read back through the browser store.
3. **Route reads and writes the store.** Done 2026-10-30 (ultraroute `5c09c33`): opened with a
   browser store and `?board=&variant=`, Studio reads the board's `.kicad_pcb`, `.kicad_pro` and
   `.kicad_dru` from it and routes in the tab. **Save to project** writes the board with the
   revision it read; refused, it offers **Reload** or **Save as a copy** (`<board>-2.kicad_pcb`,
   which leaves the board's report and status alone). With the board it writes
   `route/<board>/report.json` (what a run in the tab knows: its opens; the tab's router makes
   no full report) and the studio's entry in `design/status.json` (opens, routed from the
   board's connections, last route; the published run's findings and router commit dropped). A
   change from another app or window reloads the board when nothing routed is unsaved, and
   warns otherwise. Boardroom's Route button and tab now work on Pages, opening Studio's Pages
   site. Locally, Studio still opens a project by path and saves through its own server. Fixed
   on the way: the browser store refused to make any new file (`32b8bdd`). Checked in Chrome
   with Boardroom's and Studio's Pages builds on one origin: route easy2350 in Boardroom's Route
   tab, save, see it in History, reload and still have it; a Composer save during a run gives
   the conflict, Save as a copy and Reload work.
4. **Composer reads and writes the store.** Done 2026-10-31 (circuit-composer `697163c`).
   Opened with a store and `?board=&variant=` (Boardroom now passes the variant), Composer
   shows no selector. It loads `compose/<board>/problem.json`, and takes the parts' poses and
   the inspector's circuit from the `.kicad_pcb` (`KicadBoard::read` in its core). Read and
   written back unchanged, that board stays byte for byte the same. With no problem it says
   "Composer can't open <board> yet". **Save to project** (Ctrl+S, and in the Board bar when
   embedded) writes the `.kicad_pcb` with the revision it read ("Placement: moved U1"), then
   `compose/<board>/placement.json`. Refused, it offers **Reload** or **Save as a copy**. A
   change from another app reloads it when there are no unsaved moves. Standalone on github.io,
   it lists Boardroom's projects first and opens them through the tab's workspace. Its own
   examples follow, minus easy2350. Composer's workspace tab moved from `?ws=` to `?view=`,
   because `?ws=` is the store's. Checked in Chrome with both Pages builds on one origin: open
   easy2350 from Boardroom's Composer tab, move U1, save, reload and still have it, see it in
   History. A new tab gets the published board. A save from a second window conflicts with
   unsaved moves, and Save as a copy and Reload both work. The saved board differs from the
   published one only in U1's `(at …)` line, and kicad-cli opens it.
5. **Stale bar** on Boardroom's drawn tabs; then the Board tab draws from the `.kicad_pcb`.
   The bar is done 2026-10-31. Board, Viewer, Renders and Overview (its thumbnail is a render)
   show it once a board's `.kicad_pcb` has changed since the tab was drawn. On Pages that is any
   change in the tab's workspace: "Showing the published board; easy2350.kicad_pcb changed in
   Route at 14:05 · Routed easy2350", with a History button. Reset takes it away. Locally it
   compares the board file's time on disk (the server now sends `Last-Modified`) with
   `viewer/site.json`'s, or the thumbnail's, and says how to rebuild. Test: `npm run test:stale`
   (the Pages build in Chrome, also run by the Pages workflow). Checked by hand on this machine
   too: easy2350's board is newer than its viewer data, so Board and Viewer show the bar, and
   Renders and Overview don't.
   The Board tab drawing from the `.kicad_pcb` is done 2026-10-31, the way Studio draws it.
   ultraroute's web module gained `view(pcb, pro, dru)` (ultraroute `ce50b64`): the board's
   lines (header, one frame of everything on it, its poured zones) without routing. Boardroom
   runs it in a worker, feeds the lines to Studio's core, and draws the mesh with board-draw's
   renderer (`d37b1f8`, Studio's pin). `app/web/scripts/draw.sh` builds both WASM modules from
   a sparse clone at the pinned commit. `app/build.sh` and the Pages workflow run it, and the
   2.3 MB loads only when the tab opens. The tab's views are now **Board** (the file through the
   store, no stale bar, redrawn when any app saves it), **Plots** (pcbview's layers, with
   silkscreen and fab, which board-draw doesn't draw yet) and **3D**. A project with boards but
   no pcbview build now gets a Board tab, and a project with several boards gets a picker. File
   names come from the board's `.kicad_pro` (servodrive's is `servodrive_E.kicad_pcb`). Test:
   `npm run test:board` (easy2350's real board in Chrome: counts, picking, a save redraws it).
   Checked locally on easy2350, bike-tracker, servodrive and motherboard (263 parts, 6121
   tracks). Like Studio, it draws open connections as ratsnest lines, including pads that only
   reach a plane through a pour.
6. **Composer's server** for the local workflow. Done 2026-10-31, more simply than planned.
   Since milestone 4, Composer reads and writes a project through Boardroom's store, which on
   this machine *is* a server (`/api/projects/<id>/files/`, with revisions, conflicts and git as
   the history). The one thing it lacked locally was the placement problem, so a separate
   Composer server would only have duplicated the store. Instead, when an app reads
   `compose/<board>/problem.json` and the project has none of its own, Boardroom's server makes
   it from the board's current `.kicad_pcb` with `ccomp floorplan --problem`, as the snapshot
   does (last variant), and serves it without writing it into the project (`problem.rs`). It
   takes about 45 ms, so it is made on every read and always matches the board. ccomp is found
   through `$CCOMP`, then `ccomp = "…"` in `~/.config/boardroom/config.toml`, then the PATH;
   without it the read says how to set it. Checked locally: Composer's tab opens easy2350,
   bike-tracker and the earring. On a throwaway copy of easy2350 a moved part saved as one
   changed `(at …)` line, plus `placement.json`. Composer's own server (its app-spec's
   milestones A and B) stays its own business, for what only it can do (AI fill, Send to
   UltraRoute). Not yet: Composer assumes `hw/<board>/<variant>/<board>.kicad_pcb`, so a board
   named otherwise (servodrive's `servodrive_E.kicad_pcb`) doesn't open; and Route's local
   **Save to project** still goes through Studio's own server, not the store.

Done means: on the Pages site, a phone or laptop can open easy2350, move U1 in Composer, route
in Route, reload the page, and still see the moved, routed board with both steps in the
history. Download changes then gives a `.kicad_pcb` that opens in KiCad.

**Met 2026-10-31**, on the live Pages site in Chrome as a 390×844 touch phone. In
Boardroom's Composer tab U1 was tapped, dragged from (110.5, 126.5) to (112.55, 128.55) and
saved. In the Route tab the board was routed in the tab (9 minutes, 13 connections left open)
and saved. After a reload, History listed the changed files from both apps: Composer's
`placement.json`; Route's board, run report and `design/status.json`. The Board tab drew the
routed board (1098 tracks, 230 vias, against 819 and 191 published). Download changes gave a
zip of those files whose `.kicad_pcb` has U1 at its new place, and `kicad-cli pcb export svg`
opens it. Left over: the router in a tab leaves more open than the published native run did
(13 against 1); that is ultraroute's to close.

## Decisions

The owner's answers to the draft's questions, 2026-10-30:

1. **Phones on the LAN:** no preference; the browser-only workflow matters, not LAN devices.
2. **Who applies a placement:** Composer, writing the `.kicad_pcb` itself in Rust.
3. **Workspaces:** one per tab.
4. **Standalone Composer:** opens Boardroom's example projects.
5. **Bringing your own board:** later.
