# The working demo: Boardroom, Composer and Route on GitHub Pages

Status: **draft for review** (2026-10-30). Nothing here is built yet.

## What the person asked for

A fully working demo on GitHub Pages: open a board in Boardroom, rearrange it in Circuit
Composer, route it in Route (UltraRoute Studio), and see the result back in Boardroom. Changes
are kept in the browser, survive a reload, and are passed from one app to the next. The same
workflow on this machine saves to the project on disk instead.

Composer's own board selector goes away wherever Boardroom opened it. Composer still runs
standalone, but what standalone Composer looks like is not settled (open question 4).

## What exists today

| | Local (served from this machine) | Pages |
|---|---|---|
| **Boardroom** | Reads projects from disk; watches them; the bus over a WebSocket | A read-only snapshot (`boardroom snapshot`): `design/`, `viewer/`. No `hw/`, so no KiCad files |
| **Composer** | Static files (`python3 -m http.server`); no server yet. Ignores `?project=` and shows its built-in copy of the board, or `rp2350-battery` | Built-in example boards (`problem.json` made by `ccomp` at build time). Saving is "Download placement" |
| **Route** | Opens a project by path; routes on the server; **Save to project** writes the `.kicad_pcb` | `VITE_LOCAL` build: routes in the tab (WASM), opens files from your computer, **downloads** the routed board |

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
- **Upload a project folder**: maybe later, not in this pass (open question 5).

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

The module lives in this repo (`app/store/`, plain JS, no dependencies). Composer and Studio
pin it the way they pin `board-draw`. An app opened with `?project=` uses it. Opened without
one, it keeps its own demo boards as now.

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
2. **Composer saves** `compose/<board>/placement.json`: each part's position, rotation and side.
   It does not touch the `.kicad_pcb`, because in the browser it can't. Placing parts in a
   KiCad file is done by pcbnew today (`tools/apply_placement.py`), and pcbnew doesn't run in a
   browser.
3. **Route applies the placement.** On open, if `placement.json` is newer than the board, Route
   moves the footprints to it (its core already moves parts and writes `.kicad_pcb` in the tab).
   It shows "Placement from Composer, 14:02" and routes. **Save** writes the `.kicad_pcb`, plus
   the run report to `route/<board>/report.json`.
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
  `docs/app-spec.md`): it opens a project path and saves `placement.json`. Applying it to KiCad
  is Route's step, as on Pages, so pcbnew isn't needed in the loop.
- Route's **Save to project** goes through the same `write` call, so it gets the conflict check.
- **Other devices** (a phone on the LAN): today Boardroom refuses Studio's socket from anything
  other than this machine, and Studio loads nothing without its socket. That is why Route was
  blank on your phone (open question 1).

## Milestones

1. **Store.** `app/store/` with both backends, plus Boardroom's write endpoint. Boardroom's
   History panel shows the history, and offers Reset and Download changes.
2. **Snapshot carries the boards.** `hw/**/*.kicad_{pcb,pro,dru}` and `compose/<board>/problem.json`.
3. **Route reads and writes the store** (ultraroute): the `?project=` path on Pages, Save on
   Pages, and placement applied from `placement.json`.
4. **Composer reads and writes the store** (circuit-composer): no selector with `?project=`,
   and a load error instead of a demo fallback.
5. **Stale bar** on Boardroom's drawn tabs; then the Board tab draws from the `.kicad_pcb`.
6. **Composer's server** for the local workflow.

Done means: on the Pages site, a phone or laptop can open easy2350, move U1 in Composer, route
in Route, reload the page, and still see the moved, routed board with both steps in the
history. Download changes then gives a `.kicad_pcb` that opens in KiCad.

## Open questions

1. **Phones on the LAN.** Can another device route and save through Boardroom, or only view?
   If it can only view, Route still needs a view-only way to load there, without the socket.
2. **Who applies a placement?** This draft says Route (it can in the browser). The alternative
   is Composer writing the `.kicad_pcb` itself, which needs a KiCad writer in Composer's WASM
   core. That is cleaner for KiCad users who never route, but it is more work. A side change
   (top ↔ bottom) is the hard case either way.
3. **One workspace or one per tab?** This draft keeps one per browser: two windows of the demo
   see the same edits. Fine for one person, but a second "fresh demo" means Reset.
4. **Standalone Composer.** Does it keep its own examples and selector, or open from Boardroom's
   example projects through the same store?
5. **Bringing your own board to the Pages demo** (upload a project folder into the workspace):
   in scope now, or later?
