# Boardroom app — spec (draft)

Status: draft, 2026-10-29. Boardroom's half of [`platform-spec.md`](platform-spec.md), the
counterpart of `circuit-composer/docs/app-spec.md`. It says what the Boardroom app shows, what
you do in it, how it holds the suite together (windows, selection, AI jobs), and how it is
built. Questions for the owner are under **Open questions**; answers to Circuit Composer's
questions are under **Answers to circuit-composer**.

**Name:** the suite's main app is **Boardroom** (chosen 2026-10-29), formerly the placeholder
easy_pcb. The repository and folder are still called `easy_pcb` until they are renamed;
`ccomp import easy-pcb` keeps its name for now.

## The paradigm

KiCad's model is a project manager that launches editors. That part is right and we keep it:
Boardroom is the window you open first, it knows your projects, and Circuit Composer and
UltraRoute Studio are the editors it launches.

But KiCad's project manager is a **file browser**: it lists files and you decide what to open.
In an AI-forward tool the person is not doing most of the steps; they are steering, deciding
and signing off. So the Boardroom window is centred on **the state of the design**, not its
files:

1. **Where is it?** The phase pipeline, 0 to 10, with each gate passed, open, failed or *stale*.
2. **What needs me?** Every question and sign-off Claude or an app is waiting on, across all
   projects, answerable in place.
3. **What is Claude doing?** Running and finished AI jobs, each one a reviewable change to the
   project.

Three ideas carry it, and the first two are worth more than any one screen:

- **Every change is a reviewable change set.** Projects are git repositories. An AI job runs
  from a clean snapshot and ends as a diff (files, renders, check results) that the person
  accepts, asks to revise, or throws away. This is how "the AI fills in what you leave out"
  stays trustworthy: nothing it does is invisible or irreversible.
- **One selection across all windows** (KiCad's cross-probing, done everywhere). Select U3 in
  Composer and it lights up in Studio, in the viewer, in the parts list and in the gate
  finding that mentions it.
- **Gates are the spine.** The `pcb-design` phases stay the project's structure. Apps are where
  phases happen; the shell is where they are judged and signed off. Composer already proposes
  this split (phases 1–6 in Composer, 7 in Studio, 0 and 8–10 in the shell).

What we do not build: a schematic symbol editor or a manual layout editor. KiCad stays the place
for hand edits, and the shell has "Open in KiCad" for both.

## Windows

The person asked for desktop-style windows with a single-window mode as well. One build does
both:

| Mode | What you get | Used for |
|---|---|---|
| **Windows** (default) | The shell is one window. Composer, Studio, the viewer and KiCad each open in their own window, one per project and board | Desktop use, several monitors |
| **Single window** | Apps open as tabs inside the shell window (Composer's `&embed=1`, Studio's equivalent) | A laptop, a browser tab, a phone check-in |

Windows mode is the same pages opened as separate app windows. A setting picks the mode, and
any tab in single-window mode can be "popped out" into its own window and back.

### How windows are made

The shell is a **local Rust server plus browser-engine windows**, not a web page you happen to
open:

- `boardroom` starts the shell server (or finds it running) and opens the shell window with
  Chromium's app mode (`--app=<url>`): a window with no tabs or address bar, its own taskbar
  entry and icon. Opening Composer for a project is another `--app` window, which the server
  opens, so it works the same from a button, the CLI (`boardroom open composer <project>`) or
  Claude.
- **Why not Tauri / a webview (yet):** the apps draw with WebGL2, not WebGPU (decided with
  ultraroute, `platform-spec.md`), so a WebKitGTK webview could draw them; that removes the
  main objection. Chromium app windows are still the first step: one engine for windows mode,
  single-window mode and a plain browser tab, devtools, and the cross-origin isolation Studio
  needs (below) behaving the same everywhere. The cost is no native menus and no native file
  dialog in the page; the server covers the dialog (below). The server is the app either way,
  so wrapping it in Tauri later changes only how windows are made.
- **Folder picking.** A browser page can't learn a real path, so "Add project" asks the
  server, which opens the desktop's own folder dialog (xdg-desktop-portal, `zenity` as a
  fallback) and returns the path. A path can also be pasted.

### One origin

The shell server **proxies the apps** under its own address: `/apps/composer/…` to Composer's
server and `/apps/studio/…` to Studio's, WebSockets included. Each app still runs its own
server on its own registered port and still works on its own. Through the shell they share one
origin, which makes iframes, `BroadcastChannel` and one login-free session simple, and means
only the shell's port matters to the person.

> **ultraroute — keep the apps cross-origin isolated.** Studio routes in the tab on workers that
> share memory (`SharedArrayBuffer`), which only works on a cross-origin-isolated page, so
> Studio's server sends `Cross-Origin-Opener-Policy: same-origin` and
> `Cross-Origin-Embedder-Policy: require-corp`. Through the shell the proxy must keep those
> headers on Studio's pages; in single-window mode the shell page must be isolated too and the
> iframe carry `allow="cross-origin-isolated"`. Otherwise Studio silently loses in-tab routing.
> Composer needs the same once its annealer workers share memory.

> **Boardroom:** Agreed, and simplest done everywhere: the shell server sends
> `Cross-Origin-Opener-Policy: same-origin` and `Cross-Origin-Embedder-Policy: require-corp` on
> every page it serves, passes the apps' own headers through the proxy, and gives every app
> iframe `allow="cross-origin-isolated"`. Since the proxy puts everything on one origin, that
> costs nothing for our own pages; pcbview loads nothing from other origins (checked
> 2026-10-29). The rule it imposes: Boardroom pages embed nothing from other sites (no LCSC part
> images or remote datasheet PDFs inline); those are links, or files fetched into the project
> first, as datasheets already are. The selection bus runs over the shell's WebSocket, not
> `window.opener`, so isolated windows lose nothing.

## The shell window

```
┌──────────────┬───────────────────────────────────────────────────────────────┐
│ PROJECTS     │ easy2350 ▸ board easy2350 ▸ variant a          ● 2 jobs  ⚙   │
│              ├───────────────────────────────────────────────────────────────┤
│ ● easy2350   │ [render]  0 1 2 3 4 5 6 7 8 9 10    phase 10 release           │
│   release ⚑2 │           ■ ■ ■ ■ ■ ■ ■ ◩ ◩ ◩ □     doing: re-route waits on…  │
│ ○ rp2350-bat │                                                               │
│   floorplan  │ NEEDS YOU                              CHECKS                 │
│ ○ earrings   │ ⚑ Approve release  [review] [approve]  DRC   0 err  ✓        │
│   circuit ⚑1 │ ⚑ Pick JLC stackup 7628? [yes][no][…]  sync  PASS    ✓        │
│              │                                        stock L1 128  !       │
│ + Add        │ OPEN IN  [Composer] [Studio] [Viewer] [KiCad] [Folder]       │
│ + New        │                                                               │
│              │ Overview │ Requirements │ Parts │ Gates │ Decisions │          │
│ INBOX     ⚑5 │ Renders │ Release │ Claude                                    │
│ JOBS      ●2 │ …tab content…                                                 │
└──────────────┴───────────────────────────────────────────────────────────────┘
```

### Sidebar

- **Projects**: each row shows the name, current phase, and a flag count for what needs you.
  Click selects (the right side shows it); double-click or Enter opens its last app.
  Ordered by recent activity; pinning and a filter box for when there are many.
- **Add** an existing folder; **New** makes a project from the template (below).
- **Inbox**: every "needs you" item across all projects, newest first. This is the view to
  open in the morning.
- **Jobs**: every AI job, running and recent, across projects.

### Project header

Breadcrumb of project ▸ board ▸ variant (a project can hold several boards, each with
variants, as `hw/<board>/<variant>/` and `pcbview.toml` already allow). The variant picker
also creates a variant ("branch this board"), which is how the skill says experiments go.

### Overview tab (the headline)

- A thumbnail (`design/renders/top.png` or the 3D render) and the **phase pipeline**: one cell
  per phase, coloured passed / open / failed / stale / not started, each linking to its gate.
- **Doing**: one line, from `status.json`.
- **Needs you**: items with buttons. A sign-off is approve / reject-with-note; a question has
  its suggested answers plus free text. Answering writes the answer to the project and, if
  Claude asked it, resumes that job with it.
- **Checks**: name, result, pass/warn/fail, each linking to the gate evidence it came from.
- **Per-app status**: Composer's stage, open fields and placement metrics; Studio's opens and
  last route; from their keys in `status.json`.
- **Project facts**: git branch and whether it's dirty, last commit, last update, board size,
  layers, part count, BOM cost against the target if one is set.

### Requirements tab

`design/intent.md` as a form, not a text file: each requirement a row with its number, value,
unit, "don't care", and which phase or check covers it. Phase 0's gate ("every requirement has
a number or an explicit don't care") shows live: rows without one are flagged. Named
**Requirements** in the UI, as Composer suggested, so it isn't confused with Composer's intent.
Editing raw markdown stays available.

### Parts tab

The BOM joined with what the skill knows about each part: the audit file in `design/parts/`,
whether its datasheet is in `design/datasheets/` (open the PDF), LCSC number, stock and
lifecycle (`jlc_parts.py`), price, and which block it belongs to. Missing datasheet or missing
audit is a flag here, since phase 1 can't pass without them. "Refresh stock" is a job.

### Gates tab

The gate files rendered: checklist, evidence per item (command output, renders), reviewer
findings by severity, and how each was answered (fixed, or disputed in `decisions.md`). Actions:

- **Run gate**: starts an AI job that runs the phase's checks and the independent review.
- **Sign off** (phases 0, 4, 10, and the render reviews of 6 and 9): records who and when in
  the gate file.
- **Stale**: when an input a passed gate depended on changes (a part swapped after gate 1,
  the schematic edited after gate 2), the gate is marked stale and so is everything
  downstream, with the change that caused it. To do this, gate files record hashes of the
  inputs they judged (a small addition to the skill's gate format).

### Decisions tab

`decisions.md` as a list: decision, reasoning, alternatives, date, and links to the gate
findings it answers. Filter by phase. Add a decision by hand.

### Renders tab

The renders folder as a gallery, newest first, with before/after for any render an AI job
changed. Opening one full size lets you comment on a spot, which becomes a note an AI job can
pick up ("this silk label overlaps the via here").

### Release tab

Phase 10: the fab outputs (gerbers, drill, BOM, CPL, STEP), the gerber renders, the sync and
rotation checks, and the order checklist from `needs_user`. "Export release" zips and tags.

### Claude tab

The project's **Claude terminal**: a real Claude Code session in the project folder, for
anything the buttons don't cover and for the conversations a project starts with (phase 0 is
a conversation). See **Terminals** below for how it works and how it relates to Composer's.

### The viewer

pcbview stays the viewer, built by `scripts/view.sh` into `viewer/`. It opens in its own window
or a tab, and takes part in the shared selection. Its rebuild after each gate becomes a job the
shell runs, rather than something Claude has to remember. Changes pcbview needs (selection
messages) go to the pcbview project as requests, as the skill already says.

## Making projects

- **New project** copies the template into a folder you choose, `git init`s it, writes
  `status.json` at phase 0, adds it to the project list, and opens the Claude tab with the
  phase-0 prompt ("describe the board").
- The template moves into this repo as `template/`. The repo becomes the app plus the
  template plus the skill, instead of being the template itself.
- **Existing projects open as they are.** A folder with `design/status.json` is a project;
  anything the app needs and finds missing (schema version, input hashes, the skill) it
  adds on first write, not on open.

## The skill: a vendored copy in every project

Decided by the owner, 2026-10-29: **a project is a folder that can live anywhere, and it
carries its own copy of the skill**, but nobody edits those copies by hand. The copies exist so
a project works anywhere, and only the tool changes them, the way Cargo and npm vendor
dependencies.

- **Every project is complete.** `.claude/skills/pcb-design/` holds the skill, its scripts and
  its `LESSONS.md`. A Claude Code session opened in the folder loads it like any installed
  skill and can run the whole project, with or without the app. A project posted to GitHub on
  its own works for whoever clones it, at the skill version it was built with.
- **One upstream copy**: this repo's `.claude/skills/pcb-design/`. The app records where it is
  in `~/.config/boardroom/config.toml` (`skill_upstream = "<path>"`).
- **Updating is one command.** `boardroom skill update` copies the upstream skill into every project on
  the list and commits it in each, with the upstream commit in the message. The shell can run
  it on start or from a button, and shows each project's skill version. Editing the skill
  means editing the upstream copy once and running this.
- **Pinning.** A project mid-design or released can be pinned (`boardroom skill pin <project>`,
  or a checkbox) and is skipped by updates until unpinned. Each gate file records the skill
  version it was judged with; a gate passed under an older skill is shown as such, not as
  stale.
- **Lessons go upstream.** The project's `CLAUDE.md` says: when the person corrects you,
  read `~/.config/boardroom/config.toml`; if `skill_upstream` exists on this machine, add the rule
  to its `LESSONS.md`; otherwise add it to the project's own copy. On the owner's machine every
  lesson lands in one place and the next update spreads it. Lessons written into a project's
  copy (on a machine without the app) are not lost: before overwriting, `skill update` looks
  for lines the upstream copy lacks and offers to move them up.
- **Hand edits to a project's copy** are found the same way (the copy no longer matches any
  upstream commit) and shown before an update would overwrite them.
- **Tools stay with the project too:** the project's `scripts/` and the pinned
  `vendor/pcbview` and `vendor/ultraroute` submodules. A route depends on the exact router
  version, so pinning it per project is a feature.
  > **ultraroute:** Agreed. A route can now be reproduced: the same board, flags and seeds give
  > byte-identical output (`--uuid-seed`). So the pin should name the commit, and the recipe
  > (the flags) should live in the project beside it.

  > **Boardroom:** Agreed: the recipe lives in `route/<board>/` (Studio's directory), and gate 7
  > records the router commit and the recipe it was routed with, so a gate can be re-run
  > byte for byte.
- **Other skills** a project uses (Circuit Composer's) are vendored the same way, each with its
  own upstream path in the config, by the same command.

(Considered and dropped: a string path to one shared copy, which breaks on another machine,
and Claude Code doesn't load an `@/absolute/path` import from outside the project in a
headless `claude -p` run; a symlink, which git stores as a path that dangles elsewhere; a
plugin or submodule, more machinery than needed; plain copies updated by hand, too much work
per update.)

## The project list

`~/.config/boardroom/projects.toml`: one entry per project with its path and a pinned flag.
Not the `proj` registry, which is for servers and ports; a project is a folder, not a server.
Projects may live anywhere. A missing path shows greyed with "locate…".

## `status.json`, version 1

The existing fields stay, with `needs_user` growing from one string to a list so the inbox can
answer items one by one, and a key per app so apps don't collide:

```json
{
  "schema": 1,
  "board": "easy2350",
  "phase": 10,
  "doing": "…",
  "needs_user": [
    {"id": "release-approve", "kind": "signoff", "text": "Approve the release", "gate": 10},
    {"id": "stackup", "kind": "question", "text": "Use JLC04161H-7628?", "options": ["yes", "no"],
     "job": "j-20261029-0412"}
  ],
  "checks": {"DRC": {"result": "0 errors", "ok": true, "gate": 7}},
  "gates": {"7": {"state": "passed", "stale_because": null}},
  "links": [],
  "apps": {
    "composer": {"easy2350": {"stage": "handed_off", "open_fields": 0, "findings": {"error": 0, "warning": 2},
                              "placement": {"wire_mm": 790, "board_mm": [50, 30], "legal": true}, "updated": "…"}},
    "studio":   {"easy2350": {"opens": 0, "drc_ok": true, "findings": {"error": 0, "warning": 1},
                              "last_route": "…", "router": "<commit>", "updated": "…"}}
  },
  "updated": "2026-10-29T14:02:00Z"
}
```

`needs_user` as a plain string is still read (as one question). The shell is the one writer
of `gates`; each app writes only under its own `apps.<name>` key. Writes are
read-modify-write under a lock file, so two apps can't lose each other's update.

## The AI backend

The shell server runs every AI job, for itself and for the apps.

- **A job** is `claude -p` in the project directory with `--output-format stream-json`, its
  session id recorded so it can be resumed (`--resume`) when the person answers its question.
  It uses the person's own Claude Code login: no API keys.
- **Submitting:** `POST /api/jobs` on the shell server, `{project, board, app, skill, prompt,
  request_file, write_scope}`; events (started, text, tool use, question, finished) stream
  over the shell's WebSocket to every window, and the job's whole log is kept under
  `.boardroom/jobs/<id>/` in the project (gitignored).
- **Write scope:** a job declares what it may change (Composer's jobs: `compose/**`). It is
  passed as permission rules, and checked afterwards against the diff; anything outside it is
  shown as a violation and not accepted with the rest.
- **Change sets:** a job starts from a clean tree (see the note below) and the shell notes the
  commit. Afterwards the diff is the change set. **Accept**
  commits it with the job id in the message; **Discard** restores the base; **Revise** resumes
  the job with the person's note. Renders and check results changed by the job are shown as
  before/after.
  > **ultraroute:** A dirty tree as the base mixes the person's unsaved edits into the job's
  > change set, and Discard would throw them away with the job's. Require a clean tree, or have
  > the job commit a snapshot of the dirty one first.

  > **Boardroom:** Right, that was a bug in the plan, and the same bug has a second half: edits
  > made *while* the job runs (Studio saving a route, Composer saving a placement, the person
  > saving in KiCad) would land in the job's diff too. So:
  > - **Everything is scoped to the job's write scope.** "Clean", the lock and the change set
  >   all apply to the paths the job may write (`compose/<board>/**` for a Composer job), not
  >   the whole repo, so uncommitted work elsewhere in the project doesn't block a job, and a
  >   job can run in a repo that is busy with other work (Composer's own `boards/`, below).
  >   Changes outside the scope are flagged as violations, as before.
  > - **Clean start.** A job never starts with uncommitted changes inside its scope. If the tree is dirty, Boardroom shows
  >   what's uncommitted and offers "Commit and run", which commits it as
  >   `Snapshot before job <id>`; the job's base is that commit. (This makes a commit even for
  >   people who'd rather commit themselves; see open question 3.)
  > - **The job holds the project while it runs.** Boardroom takes a write lock on the project
  >   (`.boardroom/lock`, announced on the bus). Composer and Studio keep showing the files live
  >   but hold their saves until the job ends, then save on top of its result. KiCad can't be
  >   told to wait, so Boardroom won't start a job whose write scope includes a board or
  >   schematic KiCad has open (KiCad's `.lck` file beside it), and if a KiCad save lands
  >   mid-job anyway, the change set flags those files instead of mixing them in silently.
  > - Discard then only ever throws away the job's own changes.
  >
  > Running each job in its own git worktree would isolate it completely and allow parallel
  > jobs in one project, but the apps couldn't watch the job work live in the project's files,
  > and each worktree would need its own submodules and router build. Not now.
- **Queue:** one queue per machine. Jobs in different projects run in parallel up to a limit
  (setting, default 2); jobs in one project run one at a time, since they share a working tree.
  Heavy steps inside a job still go through `~/.claude/bin/reserve`, which the project's
  `CLAUDE.md` already requires.
- **Questions mid-job:** a job that needs the person ends with a `needs_user` item tied to
  its session; answering resumes it.

## Terminals

A terminal here means a **real, interactive Claude Code session** (`claude` in a pty), shown
in a panel. It is the conversational side of the suite; AI jobs are the other side: headless,
one task, ending as a change set. Terminals are for working *with* Claude; jobs are for handing
it a task.

### Which apps get one

| App | Terminal? | Why |
|---|---|---|
| **Boardroom** | **Yes: the project terminal** | Project-level talk: describing the board (phase 0), requirements, parts, "why did gate 7 fail", "get this ready to order". Loads `pcb-design`. Starts in the project folder. |
| **Circuit Composer** | **Yes: the composition terminal** | "Add a LiPo charger here", "why this inductor?", "make this net a 2 A power net". Loads the circuit-composer skill, knows the open board and what's selected, and works through `ccomp` commands, so its edits show up on the canvas as they land. |
| **UltraRoute Studio** | **No** | Studio is a conventional tool: a board, rules and a recipe go in; a routed board and a run report come out. Claude uses it from the other terminals and from jobs through its CLI and reads its reports; "why are there opens?" is asked in the project terminal. Studio stays a GUI you drive directly. |

The rule behind the table: **an app gets a terminal when its work is a conversation** (what
the board should be, what the circuit should contain) and not when it's a function (route
this). If that changes for Studio later, it gets one the same way Composer does.

### One mechanism, shown in two places

- **Boardroom's server runs every terminal**, as it runs every job: it owns the ptys, so one
  place lists every Claude session on the machine and the `reserve` and job rules apply to all
  of them. A terminal panel is a component in `suite-ui` that connects to the server
  (`/api/term/<id>` over a WebSocket); Composer shows the same panel in its own window.
- **A terminal outlives its window**, like a tmux session: close Composer and reopen it and
  the composition terminal is still there, scrollback and all. After a restart the session is
  resumed with `claude --resume`.
- **Separate sessions, not one shared conversation.** The project terminal and the
  composition terminal are different Claude sessions: they load different skills and keep
  their context small. They share the files, the selection and the bus, not their chat.
  Boardroom's sidebar lists every open terminal (project, app, board) so none get lost.
- **The project terminal can hand off.** Asked to do composition work, it says so and offers
  to open Composer's terminal on that board (and the reverse for project-level work), rather
  than doing another app's work without its skill.

### Knowing what "this" means

The terminal starts with a short context line (project, board, app). For the live part, which
parts or nets are selected and which workspace is open, Claude runs a command rather than
having text pushed into its prompt: `boardroom selection` prints the current selection from
the bus as JSON. Both skills say: when the person says "this" or "here", run it. Composer adds
its own `ccomp context` for the workspace and the open fields under the selection.

### Terminals, jobs and the lock

A terminal's edits are the person's edits: they land directly, are not change sets, and are
committed when the person commits (Composer still marks its AI-made values `by: "ai"`, so
they can be reviewed in place). The one interaction with jobs: while a job holds a write
scope, a terminal must not write into it. Claude Code can't be paused mid-turn, so each
project's `.claude/settings.json` has a **PreToolUse hook** that refuses an Edit or Write
inside a locked scope and tells Claude why; it can wait for the job or work elsewhere.

### Your own terminal still works

None of this is required. Because every project carries its skill and `CLAUDE.md`, running
`claude` in the project folder from any terminal gives the same agent. The in-app terminals
add the selection, the hand-off, the lock hook and a place to find every session.

## Selection and events across windows

The shell server runs one event bus (WebSocket at `/api/bus`) that every window and app joins:

| Event | Sent by | Meaning |
|---|---|---|
| `select {project, board, refs[], nets[]}` | any | The person selected parts or nets; others highlight them |
| `focus {project, board, app}` | any | Bring that app's window forward (or its tab) |
| `files {project, paths[]}` | shell's watcher | Files changed on disk, from any source |
| `status {project}` | shell | `status.json` changed |
| `job {id, state, …}` | shell | AI job progress |

Selection is by KiCad reference and net name, which every app and the viewer already share.
Each app keeps working with the bus absent.

## Architecture

Same shape as Studio and Composer:

- **`app/server`**: Rust (axum or tungstenite, whichever Studio and Composer settle on). It
  holds the project list, reads and writes project files, watches them, proxies the apps,
  runs the event bus, the job queue, every Claude terminal (Boardroom's and Composer's), and
  opens windows. Port from
  `--port`, then `$PORT`, then `proj port`; fails with none. Registered with `proj` as
  `boardroom`.
- **`app/web`**: Svelte 5 and Vite, like the other two (plain Vite; SvelteKit's page routing
  isn't needed for a handful of tabs). The shell is forms, lists and text and draws no boards
  itself: thumbnails are renders, and the viewer is pcbview. A Rust→WASM core only if shared
  code makes it worth it.
- **Shared with the other apps** (ultraroute's proposal, agreed):
  - **`suite-ui`**, a Svelte package with the design tokens and the panels every app repeats:
    inspector, checks list, jobs list, inbox item, and the selection-bus client. Boardroom
    owns the bus, the jobs and the inbox, so the package starts in this repo
    (`packages/suite-ui`) and the others depend on it; it moves to its own repo if that
    gets awkward.
  - **The `status.json` schema types**, a small Rust crate (`crates/status`) beside it.
  - **The board renderer** is `board-draw` (working name), its own repo, extracted from Studio
    and pinned by Studio and Composer (Composer's `app-spec.md`). The shell doesn't draw
    boards, so Boardroom doesn't depend on it.
- **The CLI comes first**, as in Composer: `boardroom projects`, `boardroom status <project>`,
  `boardroom job <project> "<prompt>"`, `boardroom open composer <project>`, `boardroom signoff
  <project> <gate>`. Every button is one of these, so Claude can drive the shell too.
- **`index.html` and `serve.sh` stay** in projects as the static project page (phone, GitHub
  Pages). The app does not need them.

## Answers to circuit-composer

From `platform-spec.md` and Composer's `app-spec.md`:

- **(a) How do apps submit a job?** `POST /api/jobs` on the shell server, events back on
  `/api/bus` (above). Composer's request file goes in `request_file`, its write scope as
  `compose/**`.
- **(b) Composer standalone and AI work:** agreed, one queue: Composer asks the shell server.
  The server is a small daemon that runs without its window, so "requires the shell" means
  `boardroom serve` is running, not the UI open. Composer can start it if it isn't.
- **(c) Can a job name its skill?** Yes, `skill` in the job; the prompt tells Claude to use
  it. The skill has to be present in the project: it is vendored into
  `.claude/skills/circuit-composer/` by `boardroom skill update`, from your repo as its upstream,
  the same way as `pcb-design`.
- **Net classes:** agreed, once a project has `compose/`, `intent.json` is the source and
  `nets.csv` is generated. The shell shows `nets.csv` read only in that case, with "edit in
  Composer".
- **Naming:** agreed, "Requirements" in the UI.
- **Window default:** agreed, launch opens a window; the shell's project overview shows
  Composer's status, not an embedded canvas (a small canvas there would be cramped and
  duplicate the app).
- **Where Composer's boards go:** decided, they stay in `circuit-composer/boards/` for now, with
  Composer's "compose root" setting. They aren't Boardroom projects (no `design/`), so they
  don't appear in the project list. AI jobs on them still go through Boardroom's backend: a job
  names a folder and a write scope, not only a project, and the scoped clean check and lock
  (above) mean the circuit-composer repo's other uncommitted work doesn't get in the way.
- **Who reviews a Composer job.** Composer does, field by field (accept makes a fill *set*,
  reject makes it *open* again), since only Composer can show a fill with its reasons and
  citations in place. Boardroom's Jobs view shows the job and its diff, and its Accept for a
  Composer job is "Review in Composer", which opens the board there. When the person has gone
  through the fills, Composer tells the backend, and the job's change set is committed with
  the rejected fills already reverted to open.

## Milestones

1. **Server and list.** `boardroom serve` with the project list, reading `status.json` (old and
   v1) for easy2350 and one new project made with New. Shell window with sidebar and Overview. Registered
   with `proj`.
2. **Windows.** Open the viewer, KiCad, Composer and Studio for a project, each in its own app
   window, through the proxy. Single-window mode with tabs.
3. **Jobs.** `POST /api/jobs`, one `claude -p` job streaming into a Jobs view, ending as a change
   set that can be accepted or discarded.
4. **Inbox.** `needs_user` as a list; answer and sign off from the shell; a job resumed by an
   answer.
5. **Gates.** Gates tab, run-gate as a job, staleness from input hashes.
6. **The rest of the tabs** (Requirements, Parts, Decisions, Renders, Release), and the Claude
   terminal (Composer's terminal follows with its canvas).
7. **Selection bus**, with Composer and Studio, and a pcbview request for it.

## Open questions

1. ~~Where projects live~~ Decided: anywhere; a project is a folder. Composer's boards stay
   in `circuit-composer/boards/` for now (Composer's decision, 2026-10-29).
2. **Chromium app windows now, Tauri later?** The proposal is app windows first. With WebGPU
   dropped, Tauri is a real option later; if native menus and dialogs matter on day one, say so.
3. **Change sets as commits.** Accepting a job commits it. Some people would rather commit
   themselves; a setting, or always commit?
4. ~~The skill's copy per project~~ Decided: a copy in every project (see "The skill: a vendored
   copy in every project"). Still open: `~/.claude/skills/pcb-design` is an older, diverged copy, and
   it loads in every session on this machine. Replace it with a symlink to this repo's copy,
   or remove it?
5. **Phone.** Single-window mode in a phone browser for checking status and answering the
   inbox from the couch: worth designing for from milestone 1, or later?
