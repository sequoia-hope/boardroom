# Boardroom: an AI-forward EDA suite — spec (draft)

Status: draft, 2026-10-29. Written from the owner's description for the three sibling projects
(Boardroom, circuit-composer, ultraroute) to read and push back on. Where this spec guesses, it
says so under **Open questions**.

**Name:** the suite's main app is **Boardroom** (chosen 2026-10-29), formerly the placeholder
easy_pcb. The repository and folder are still called `easy_pcb` until they are renamed;
`ccomp import easy-pcb` keeps its name for now.

## The idea in one paragraph

Boardroom becomes the main window of an EDA tool, the way KiCad's project manager is the window
you open first. It holds the list of board projects, shows each one's headline information, and
launches the apps that do the work. **Circuit Composer** and **UltraRoute Studio** become apps
inside it, as Eeschema and Pcbnew are inside KiCad. The whole suite is AI-forward: anything the
designer leaves undefined, an AI backend fills in, and that backend is the user's own Claude
Code, not an API account.

## The three parts

| Part | Role | Today | Becomes |
|---|---|---|---|
| **Boardroom** | Shell and project manager | A template repo: the `pcb-design` skill, phases and gates, `design/status.json`, a project page with the pcbview viewer | The front end: lists projects, shows headline info, owns the project format and the phase/gate record, launches the apps, runs the AI backend |
| **Circuit Composer** | Composition: schematic, board shape, floorplan, locked copper | `ccomp` CLI: intent schema, packages, composer, floorplanner, constraint compiler. No UI | Its own app (Rust/WASM, Svelte, WebGL2 through a board renderer shared with Studio): a block-diagram canvas whose blocks and nets may be left partly defined for the AI to complete, outline and generator tools, a live floorplanner, locked copper and routing regions. Hands UltraRoute a placed board plus `routing.json` |
| **UltraRoute Studio** | Routing | Studio: live, interactive view of the router at work (Rust server, WASM core, Svelte client) | The routing app: opens a project's board, routes it live, writes the routed board back to the project |

### Boardroom (the shell)

Full spec: [`shell-spec.md`](shell-spec.md), which also answers Circuit Composer's questions
below (job API, standalone AI work, skills, windows). In short: a local Rust server that opens
Chromium app windows (one per app, or one window with tabs), a window centred on the phase
pipeline, an inbox of what needs the person, and AI jobs that end as reviewable change sets.

- **Projects.** A list of Boardroom projects, each with its own path on disk. The user can add,
  open, and select them. Selecting one highlights it and shows its headline info.
- **Headline info** is whatever Boardroom manages for a project: board name, current phase, what
  Claude is doing, what it needs from the user, gates passed, checks, links, a thumbnail. Today
  this is `design/status.json`; that file is the starting point for the schema.
- **Managing that info.** UI to edit what Boardroom owns: design intent (`design/intent.md`),
  decisions, net classes (`design/nets.csv`), parts and datasheets, gates and sign-offs.
- **Launching apps.** "Open in Circuit Composer" / "Open in UltraRoute Studio" for the selected
  project. The shell passes the project path; the app works on files in that project.
- **The viewer.** pcbview stays the board/schematic/3D viewer in the shell.
- **The AI backend** (below) lives here and is shared by the apps.

### Circuit Composer (an app)

Full spec: `circuit-composer/docs/app-spec.md`. In short:

- Takes a project path (and a board name) and reads and writes its files inside that project,
  under `compose/<board>/`. Its existing boards stay under its own `boards/` for now, opened
  in place.
- Five workspaces: **Circuit** (block diagram of package instances and nets, partly defined
  allowed), **Board** (the floorplan, annealed live in the browser, parts dragged and locked),
  **Shape** (outline, holes, keep-outs, parametric generators), **Copper** (generated block
  layouts, pattern generators such as LED ring fields, reserved vias, pours, routing regions)
  and **Rules** (net/block/domain intent with citations). A checks panel links every finding to
  the workspace that fixes it.
- Every field is *set* (by a person), *filled* (by a tool or the AI, with a reason and
  citations) or *open*. Deterministic tools fill first; "Fill with AI" sends the rest to the
  shell's AI backend as a request file, and fills are reviewed and accepted in the app.
- Produces the schematic (`tools/compose_kicad.py`), the floorplan (`ccomp floorplan`), locked
  copper, and the router handoff: a placed board with `(locked yes)` parts and copper, plus
  `.kicad_dru` and `routing.json` (the format agreed with ultraroute in its milestone 3).
- Owns intent, packages, the research library and the constraint compiler, as now.
- Every app action is also a `ccomp` command on a project path, so Claude and the person drive
  the same operations.

### UltraRoute Studio (an app)

- Takes a project path, opens the project's KiCad board, routes it live, and saves the routed
  board back into the project (`hw/<board>/<variant>/`).
- Reads the rules and cost grids Circuit Composer compiles, as it does now.
- Keeps the GPL boundary: the studio runs `copperroute` as a separate process and never links
  it. Embedding the studio in the shell (an iframe or a link) does not change that.

> **ultraroute — the renderer (Composer's open question 5).** Share a renderer rather than adopt
> wgpu. Studio draws boards with a WASM core that builds a mesh and one WebGL2 shader that cuts
> each quad to an anti-aliased capsule or a fill (about 120 lines). That already runs in headless
> Chrome and Playwright, Firefox on Linux and phones, the cases Composer wants its fallback for;
> and Composer's renderer uses no compute shaders, so WebGPU adds nothing to draw with while wgpu
> adds megabytes of WASM. The owner agrees: no WebGPU front end. Proposal, one shared board
> renderer: a Rust crate that builds meshes and does hit testing, a small JS module with the
> shader (WebGL2 now, WebGPU later behind the same interface if it ever pays), text on an overlay
> canvas; 3D through three.js and Threlte, as Waffle Iron does. Studio's renderer becomes that
> package and Composer uses it from milestone A. WebGPU routing is no reason to keep WebGPU
> either: ultraroute's GPU solver is slower than its CPU search and the browser's `--gpu` path
> does not work (ultraroute `REVIEW.md`; its remediation Stage 10). Pages routes on CPU workers.

> **ultraroute — common UI.** Svelte 5 and Vite in all three apps (SvelteKit only if the shell's
> tabs want its page routing), and one shared package for the design tokens and the panels every
> app repeats: inspector, checks list, jobs list, inbox item, and the selection-bus client.
> Otherwise three apps grow three looks.

## The seam: files in the project directory

The apps talk through the project's files, not through each other's internals. Each app can
be run on its own against a project path, and the shell only needs to know how to launch it
and how to read the headline info.

A project directory (today's Boardroom layout, extended):

```
<project>/
  design/status.json        headline info: the shell reads it, every app may update it
  design/intent.md          requirements (Boardroom)
  design/nets.csv           net classes (Boardroom; ccomp imports it)
  design/decisions.md, gates/, parts/, datasheets/
  hw/<board>/<variant>/     KiCad projects: the schematic and board every app reads and writes
  compose/                  Circuit Composer's files: packages.lock, then compose/<board>/
                            (composition, intent, shape, floorplan, placement, locked copper,
                            generators, AI jobs, compiled outputs)
  route/                    UltraRoute Studio's jobs and results for this project (proposed name)
```

Rules for the seam:

1. **KiCad files are the shared truth** for schematic and board. An app that changes the board
   writes a KiCad file the next app can open; nobody depends on another app's private format.
   > **circuit-composer:** Agreed, with one wrinkle: for a composed board the KiCad schematic
   > and the unrouted board are *generated* from `compose/`. Proposal: an app records hashes of
   > the KiCad files it last wrote; before writing again it checks them, and if another app or
   > a person changed the file it reads back what it understands (poses, locks) and shows the
   > difference instead of overwriting. Studio's nudges and locks reach Composer this way.
   > Locks are shared as KiCad's own `(locked yes)`.
2. **Each app owns its own subdirectory** and doesn't write in another's.
3. **`design/status.json` is append-friendly shared state**: an app records what it did and what
   it needs from the user there, so the shell can show it. Its schema is Boardroom's to define,
   with fields the others ask for.
   > **circuit-composer:** Fields we'd ask for, per board: `stage` (composed / floorplanned /
   > handed_off), `open_fields` (count left for the AI or the person), `findings` (errors,
   > warnings), `placement` (wire_mm, board_mm, legal), `updated`. Happy to nest them under an
   > app key (`"composer": {...}`) so apps don't collide.
4. **Ports come from the machine's registry** (`proj`), never hard-coded. Each app is a local
   server that takes `--port` / `$PORT` / `proj port` and fails if it has none.

## The AI backend

- It runs the user's **local Claude Code** (`claude`), using the login it already has. No API
  keys, no credits, nothing to set up.
- Concretely: the shell starts `claude -p` (headless, streaming JSON output) in the project's
  directory, with a prompt for the task (e.g. "fill in the undefined blocks in compose/…").
  Because it runs in the project, it picks up the project's `CLAUDE.md` and skills, so it
  follows the `pcb-design` phases and gates as a human-driven session would.
- The apps ask the shell's backend for AI work rather than each running their own: one queue,
  one place to see what Claude is doing, and heavy jobs still go through the machine's
  `reserve` queue.
- Results come back as file changes in the project plus a status entry; the app re-reads the
  files and shows what changed.

> **circuit-composer:** We'd use it like this: the app writes
> `compose/<board>/ai/<job>.request.json` (open fields, relevant files, constraints) and asks
> the backend to run it; Claude edits only `compose/`, marks every change `by: "ai"`, and writes
> `<job>.result.json`. Questions: (a) how do apps submit a job — an HTTP endpoint on the shell's
> server? (b) when Composer runs without the shell, should it refuse AI work (our proposal: yes,
> one queue) or run `claude -p` itself? (c) can a job name the skill to load (circuit-composer
> rather than pcb-design)?

## Terminals

Interactive Claude Code sessions live in two apps: **Boardroom** (the project terminal, with
`pcb-design`) and **Circuit Composer** (the composition terminal, with its own skill, aware of
the open board and selection). **UltraRoute Studio has none**: it is a conventional tool,
inputs in and outputs out, used by the other two through its CLI and run reports. Boardroom's
server runs all terminals; Composer shows its own in a shared `suite-ui` panel. Details in
`shell-spec.md`, "Terminals".

## What each sibling project needs to do

**Boardroom**
- Project registry (list of project paths) and the shell UI: project list, selection, headline
  panel, launch buttons, embedded pcbview.
- A written schema for `design/status.json` (and a version field).
- The AI backend: run `claude -p` in a project, stream its output to the UI, queue jobs.

**circuit-composer**
- Accept a project path for every command and keep per-project files under the project.
- Build the app (`app/core` Rust→WASM, `app/web` Svelte + the shared WebGL2 board renderer, `app/server` Rust),
  served the registered way, standalone or embedded.
- The "partly defined" model and the AI request/response: drafted in its `docs/app-spec.md`.
- Its own boards (earrings, rp2350-battery) stay under `circuit-composer/boards/` for now
  (owner's call). The app opens them in place as well as Boardroom projects.

**ultraroute**
- Studio: open a board from a project path and save the routed board back to it.
- Report results into `design/status.json` (route done, opens left, DRC result).

> **ultraroute — what Studio will do.**
> 1. Open a board by project path (`/?project=<path>&board=<name>&variant=<v>`) and save the
>    routed board back to `hw/<board>/<variant>/`. Parts moved in Studio are written as KiCad
>    `(at …)` edits and `(locked yes)`, so Composer's `written.json` check sees them.
> 2. A machine-readable run report (`--report <file>.json`). Today the router tells these as
>    stderr lines: open connections, region and length breaches, stages the clock cut short, and
>    forced necks (a wide net necked into a fine-pitch pin, with the KiCad rule that allows it).
>    The report gives them as findings with refs and nets. Studio writes the summary under
>    `apps.studio.<board>` in `status.json`: `opens`, `drc_ok`, `findings`, `last_route`,
>    `router` (commit), `updated`. This lands with ultraroute's remediation Stage 8, where the
>    driver gets structured stats.
> 3. `route/` is fine as Studio's directory: run reports, recipes and replays per board.
>
> Floorplan (open question 1): agreed. Composer places; Studio nudges and locks while routing;
> both through the KiCad board.

## Open questions

1. **Floorplan ownership.** The owner leans to Circuit Composer (it has `ccomp floorplan`);
   ultraroute's roadmap also has placement (physics diffusion, interactive drag-and-lock in
   Studio). Proposal: Circuit Composer makes the floorplan, Studio lets you nudge and lock parts
   while routing, and both write the same KiCad board.
   > **circuit-composer:** Agreed. Composer's floorplan includes the board shape and locked
   > copper (generated block layouts, reserved vias, rails, pours), since those decide where
   > parts can go. Studio's changes come back through the KiCad board (see the note under rule 1).
2. **The line between Boardroom and Circuit Composer.** Proposal above: Boardroom owns
   requirements, project management, phases/gates and release; Circuit Composer owns
   composition, schematic generation, floorplan and constraints. Both sides should confirm.
   > **circuit-composer:** Confirmed. Two seams need a rule: (a) **net classes** — today phase 3
   > writes `design/nets.csv` and `ccomp import easy-pcb` turns it into `intent.json`; once a
   > project has `compose/`, we propose `intent.json` is the source and `nets.csv` is generated
   > from it, so there are never two hand-edited copies. (b) **naming** — `design/intent.md`
   > (requirements) and Composer's `intent.json` (net/block intent) are both "intent"; worth
   > calling the former *requirements* in the shell UI.
3. **One window or several?** Shell embeds the apps (iframes, one browser tab) or launches each
   in its own tab/window. Embedding feels like one program; separate tabs are simpler.
   > **circuit-composer:** Owner's direction: both. Composer will serve one build that runs
   > standalone (`/?project=<path>&board=<name>`, its own window: the default, since a canvas
   > wants the whole screen) or embedded (`&embed=1`: no project chrome, reports selection and
   > status to the parent by `postMessage`). Suggest the shell's launch button opens a window and
   > a smaller embedded view sits in the project panel.
4. **Where the project list lives**: a file in the user's config directory, or entries in the
   `proj` registry.
   > **Boardroom:** Decided by the owner: a project is a folder and can live anywhere; the list
   > is `~/.config/boardroom/projects.toml`. Each project carries its own copy of each skill it
   > uses in `.claude/skills/` (pcb-design from Boardroom, circuit-composer's from its repo), so
   > an agent opened in the folder can run the project on its own, and a project cloned from
   > GitHub works without anything else installed. The copies are vendored:
   > `boardroom skill update` refreshes every project from each skill's upstream copy in one step
   > (pinned projects skipped), and lessons are written upstream. See `shell-spec.md`.
5. **The `pcb-design` skill's phases** were written for a human-plus-Claude terminal flow. How
   they map onto app actions (e.g. phase 2 = Circuit Composer, phase 7 = Studio) needs deciding.
   > **circuit-composer:** Our proposal: phases 1 (blocks and packages), 2 (generated schematic),
   > 3 (net intent), 4 (outline, holes, connector edges), 5 (compiled rules) and 6 (placement)
   > happen in Composer; 7 in Studio; 0 and 8–10 stay in the shell. Gates stay the shell's;
   > Composer writes check output as gate evidence for 2, 3, 5 and 6.

## Suggested first milestones

1. Boardroom shell lists two real projects (easy2350 and a new one) from their paths and shows
   their `status.json`. (Composer's boards stay in its repo for now, so rp2350-battery isn't a
   project yet.)
2. "Open in UltraRoute Studio" opens easy2350's board in Studio; a route saved there shows up
   in the shell's viewer.
3. Circuit Composer runs `compose` + `floorplan` against a project path and writes into it.
   > **circuit-composer:** This is our first step, before the app (its milestones A–F build on
   > it). Its first UI milestone, A, opens a project path and shows an existing floorplan.
4. The AI backend runs one `claude -p` job from the shell on a project and streams it.
5. Circuit Composer's canvas, with one AI-filled block.
