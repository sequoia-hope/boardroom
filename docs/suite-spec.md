# Boardroom and Waffle Iron: an electromechanical suite — spec (draft)

Status: agreed between the Boardroom and Waffle Iron agents, 2026-11-03, after two rounds of review, with
the owner confirming explicit sync and the split of work. Written from the owner's direction for four
sibling projects (Waffle Iron, Boardroom, Circuit Composer, UltraRoute). Composer and UltraRoute have
not reviewed it yet.
Nothing here is built yet except what "What exists today" lists. Choices the owner has not confirmed
are marked **Recommended**, and every open one is under **Open questions**.

Builds on [`platform-spec.md`](platform-spec.md), [`shell-spec.md`](shell-spec.md) and
[`demo-spec.md`](demo-spec.md), and on Waffle Iron's `specs/waffle_v4_document_model.md` and
`specs/kicad_board_link.md`.

## The idea in one paragraph

Mechanical CAD and board design become one suite. A Waffle Iron project and a Boardroom project
stay **linked for as long as the product is being developed**. Either side can change at any
time, the other side can see that it is behind and what changed, and the person chooses to
update or to stay pinned on an older version. The board outline can be designed in Waffle Iron
or in Circuit Composer. **One side drives it at a time,** the other shows it locked, and the drive
changes hands only when someone explicitly takes it over. There is no start or finish to this
workflow, and neither app is the permanent owner of the board's shape.

It runs as one local power app: Boardroom's native window, with Waffle Iron and Composer inside it,
project files and git on disk, and an embedded terminal that can reach all of them.

**Boardroom is home (the owner's decision):** the program you open first, holding the project list, links,
notices and terminals, with its phases and gates relaxed for mechanical-only projects. Waffle Iron stays
the bigger program, opens from it in its own full window if wanted, and stays fully usable alone.

## The owner's direction

1. **A continuous lifecycle, not a handoff.** Board and CAD designs are always being updated.
2. **The outline can start on either side, with one driver at a time.** The driver stays the driver
   until someone explicitly changes it. The other side shows "Outline controlled by Waffle Iron" with
   an unlock "at your peril", and unlocking warns what is lost (constraints or a generator recipe).
   Editing on both sides and merging is not a goal for now.
3. **Link, stale, update or pin.** A Waffle Iron file or project is linked to a Boardroom project.
   If either is updated the other reports it, and can sync to the latest or stay pinned.
4. **Version tracking is a suite feature.** It was missing from Boardroom's spec, and should be
   considered for Waffle Iron as well.
5. **DXF carries the shared geometry,** with Waffle Iron's exporter extended to put the necessary
   information into it.
6. **First a local native window, later the browser and a relay.** Waffle Iron runs as a micro-app
   under Boardroom in its native window, the way Boardroom already runs here, because cross-app work
   is easier there than on GitHub Pages, where Boardroom's functionality is still partial. Shared
   browser storage and a Cloudflare Worker relay come later.
7. **Normalize the newer projects toward Waffle Iron's toolkit,** including plain upgrades where
   they help.
8. **True arcs in Composer.** Composer's outlines keep real arcs.
9. **Stable IDs are a requirement.** If Waffle Iron lacks any part of them, it gets added.
10. **An embedded terminal** in the app, able to work with Waffle Iron as well as the board tools.

## What exists today

| Piece | Where | State |
|---|---|---|
| Board import into CAD: outline as a derived solid, one instance per footprint, a connector on each mounting hole | Waffle Iron `specs/kicad_board_link.md` | Landed, one-way. Writing back is a stated non-goal |
| Re-sync that keeps the user's work, reconciling by `external_key` (footprint UUID for instances, `"board"` for the board) | same, rows R1–R6 | Landed |
| Links pinned to a commit or following a branch or tag, with the loaded commit and a content hash | Waffle Iron `crates/file-format/src/sources.rs`, v4 spec §2.4 | Landed |
| "Update to tip" and "Pin" as explicit actions | Waffle Iron Sources panel, v4 spec §7.3 | Landed |
| Host adapters: `resolve_ref`, `fetch_blob`, `put_file` | Waffle Iron v4 spec §7.2 | Landed for GitHub, GitLab, Gitea. Local git is listed as a future desktop host |
| `Derived` provenance: read-only, replaced by a re-sync | Waffle Iron `feature-engine` | Landed |
| One-view DXF writer, R12, `LINE`, `ARC`, `CIRCLE`, `POLYLINE`, layers | Waffle Iron `crates/kernel-v2/src/dxf_export.rs` | Landed |
| DXF reader: `LWPOLYLINE`, `CIRCLE`, `LINE` only | Composer `crates/shape/src/import.rs` | Landed. Cannot read Waffle Iron's `ARC` or `POLYLINE`, ignores layers, flattens arcs, drops open chains |
| Apps embedded on one origin | Boardroom `/apps/<name>/`, `docs/shell-spec.md` "One origin" | Landed for Composer and Studio |
| Browser workspace with per-file revisions and a history | `docs/demo-spec.md`, `app/store/store.js` | Store module exists, one workspace per tab |
| Gates going stale when judged inputs change | `docs/shell-spec.md` "Gates tab" | Specified. The server reads a stale reason; computing one is not confirmed |
| Git ahead/behind per project | Boardroom `app/server/src/project.rs` | Landed |
| Boardroom's native window, file API with revisions and conflicts, bus | `app/gtk`, `app/server/src/server.rs` | Landed. The embedded terminal is specified in `shell-spec.md`, not built (no `/api/term` route) |
| Waffle Iron's agent link: an MCP server paired with one tab, whose tools run in that tab's engine | Waffle Iron `relay/`, `specs/waffle_mcp_server.md` | Landed |
| Waffle Iron's native headless host and viewer sync | Waffle Iron `crates/waffle-host`, `specs/waffle_server_mode.md` | Landed in checkpoints. It has no native window |
| `.kicad_pcb` reader and writer | UltraRoute `core/crates/kicad-pcb`, used by Composer through a git pin | Landed. Waffle Iron has its own smaller reader with a shared origin |

## Principles

1. **Share results, keep recipes private.** A Waffle Iron constraint means nothing to Composer, and
   Composer's generators mean nothing to Waffle Iron. Only resolved geometry crosses between apps.
2. **Each project owns its own history.** Nothing writes to a file that two projects share. The
   driver's project publishes the interface and the other follows it. A handover moves who publishes.
3. **A version is a git commit, a release is a git tag.** The suite does not invent a version
   system. Where there is no git (the browser stage), content-addressed revisions stand in with the same
   interface and map onto git later.
4. **Stale means the interface changed.** A change the other side does not depend on is never
   reported as work to do.
5. **Sync is explicit (the owner's decision).** A half-finished sketch must not move a routed board. Every
   sync is a reviewable change.
6. **Files are the seam,** as in `platform-spec.md`, and KiCad files stay the truth for schematic
   and board. Each app stays usable on its own.

## Linked projects

```
Boardroom project                          Waffle Iron project
  interface/<board>/board-shape.*   ──▶      link: follows main, loaded rev 41
  link: pinned at rev 12            ◀──      interface/<board>/board-shape.*
```

The relationship is many-to-many: one enclosure can hold several boards, and one board can go into
several products. So each link names a project and a board, and separate repositories are the
normal case (**Recommended**). A Waffle Iron file can also live inside the Boardroom repository for
a one-board product, through a `Relative` locator. Files linked that way move together unless the
link is pinned to an older commit.

### The link record

Both sides use Waffle Iron's reference model, so a link means the same thing everywhere. A ref is
`Commit` (pinned), `Branch` or `Tag`, with the commit last loaded recorded beside it.

In a Boardroom project, committed in `design/links.toml`:

```toml
[[links]]
id = "enclosure"
kind = "waffle"
board = "easy2350"
locator = { type = "Git", remote = "https://github.com/example/enclosure", path = "interface/easy2350/board-shape.json", ref = { type = "Branch", name = "main" } }
resolved = { commit = "<sha>", at = "2026-11-03T12:00:00Z" }
interface_hash = "<hash>"     # of what was last accepted
pin_reason = ""               # why, when ref is a Commit
```

Boardroom materializes each link's accepted version (`resolved.commit`) byte-for-byte from git at
`<project>/.boardroom/links/<id>/board-shape.{dxf,json}` (`.boardroom/` is gitignored), rewrites it
whenever `resolved.commit` changes, and returns `<project>/.boardroom/links/<id>/board-shape.json`
as `following_path` in `/api/projects/{id}/links`. Circuit Composer reads the accepted shape from
there: `ccomp shape pull --interface <following_path>` (and `shape.json`'s `sync.following`) points
to `following_path` rather than reaching into git itself.

In a Waffle Iron document it is a new source kind, `BoardInterface`, in the existing `sources`
table, rather than an extension of `KicadPcb`, since its content is two files, not one board file.
No format version bump: an older Waffle Iron keeps an unknown kind or locator verbatim and reports it
unresolvable (`sources.rs:182-205`, `:368-381`). `SourceEntry` gains an `interface_hash` beside
`content_hash`, and a remembered base.

### Link states

| State | Meaning | Shown as |
|---|---|---|
| **Current** | Latest version, no interface differences | Quiet |
| **New revision, no interface change** | The other project committed something this side does not depend on | Quiet. The loaded commit advances on the next check |
| **Update available** | Following a ref, and the interface changed | Notice with a diff and its consequences |
| **Pinned, behind** | Deliberate. "Pinned at rev B, latest is D, two holes moved" | Information, never a warning |
| **Locked** | The other side drives this outline | "Outline controlled by Waffle Iron", with unlock "at your peril" |
| **Changed outside the suite** | A third party edited it, such as KiCad's `Edge.Cuts` or a re-save in another CAD program. | Boardroom inbox item: import it? |
| **Competing take overs** | Both sides took over from the same revision before seeing each other (see "Handing over the drive") | Boardroom inbox item: pick one |
| **Unavailable** | The other project cannot be reached | Notice. The last loaded version keeps working |

An update shows what changed before it is accepted, including its consequences: which mates lose
their target (Waffle Iron's `MateTargetGone`), and which gates go stale (Boardroom).

## What counts as stale

Waffle Iron hashes the whole `.kicad_pcb` today (`git_blob_sha1`), so a re-route of the copper would
report the board out of date although nothing mechanical moved. With two designs always changing,
warnings like that soon get ignored. The suite compares only what the other side depends on.

| Change | Result on the other side |
|---|---|
| Copper re-routed | Nothing to do. The new revision is recorded quietly |
| Enclosure rib or colour | Nothing to do |
| Connector or mounting hole moved | Stale, listing what changed |
| Outline or thickness changed | Stale, and the Boardroom gates that depend on it go stale |

Two interface hashes are needed: **board shape** (below), and **mechanical** for what changes geometry
or assembly in Waffle Iron (`kicad_board_link.md:66-143`):

- **Board:** thickness (`general` or the stackup sum).
- **Each footprint:** `uuid` or `tstamp`, layer, `(at x y rot)`, each model's path, `offset`, `scale`
  and `rotate`, and `attr` (`board_only` or `exclude_from_bom` with no model makes no instance).
- **Mounting-hole footprints:** also pad kind, position, drill and size.

Reference, Value, Footprint and Datasheet properties, the title block, nets and the copper layer count
are reported quietly, never as stale. `git_blob_sha1` stays as the file's identity.

### Rules when a `.kicad_pcb` and an interface are both linked

- **Outline and thickness:** the interface drives them, and the `.kicad_pcb` gives only footprints and
  models. A difference over 1 nm is a warning naming which is ahead.
- **Holes:** interface holes are board-level drilled holes only. Footprint holes stay with their
  footprint, so no hole is mated twice.

## The interface files

```
interface/<board>/board-shape.dxf     outline, cutouts, holes, keepouts, with IDs
interface/<board>/board-shape.json    revision, parent, author, driver, thickness, geometry hash
```

Only the shared library below writes in `interface/`, which adds one rule to `platform-spec.md`'s
seam. Publishing is a commit.

**Why two files.** DXF carries geometry well, can be opened in any CAD program, and is the format
Waffle Iron, Composer and KiCad already partly share. But a third-party program can re-save the
file and silently drop its extended data. If the revision history lived there it would vanish at
the moment it was needed. With a manifest beside it, a hash mismatch becomes a useful signal:
"changed outside the suite, import it?"

### `board-shape.dxf`

DXF R12 (`AC1009`), the dialect Waffle Iron already writes (**Recommended**, pending the check under
Open questions). Millimetres, Y up, as DXF expects (**Recommended**): a Y-down file would look
mirrored in any other CAD program, and Composer's reader already negates Y on the way in. A point
`(x, y)` in KiCad's frame is written as `(x, -y)`. Arcs are counter-clockwise in this Y-up frame, as
DXF defines them, so a Y flip must also reverse each arc's direction. Waffle Iron converts at its
boundary as `kicad_board_link.md` §2.3 already specifies.

| Layer | Entities | Meaning |
|---|---|---|
| `BOARD_OUTLINE` | `LINE`, `ARC`, `CIRCLE` | One closed outer loop |
| `BOARD_CUTOUT` | `LINE`, `ARC`, `CIRCLE` | Closed inner loops |
| `BOARD_HOLE` | `CIRCLE` | Drilled holes |
| `BOARD_KEEPOUT` | `LINE`, `ARC`, `CIRCLE` | Closed keepout loops |

Extended entity data under the application name `BOARD_INTERFACE` attaches, per entity:

- `id`: a stable, namespaced ID, **a requirement**. Without one, a moved hole looks like one deleted
  and one added, which would break the mates attached to it. The ID is minted once by the app that
  authors the element, `waffle:<sketch feature uuid>/<entity id>` or `composer:<id>`, and **a pulled
  shape keeps the IDs it arrived with**, through re-publishing and take over. Waffle Iron's sketch
  entity IDs already persist in the document. Two gaps remain: derived sketches renumber from 1 on each
  derive (`kicad.rs:1193-1203`), and Composer's `Hole` has no ID.
- Arcs: start and end points, which the hash uses (below).
- `loop`: which loop a segment belongs to.
- Holes: `plated`, `clearance`.
- Keepouts: `side`, `max_height_mm`.

Arcs stay arcs. Curves with no exact form in this dialect (splines, ellipses) are written as
polylines under a stated tolerance and marked approximate in the manifest.

If extended data is missing on import, elements are re-matched by geometry, and uncertain matches
are put to the person.

### `board-shape.json`

```json
{
  "schema": 1,
  "board": "easy2350",
  "revision": "rev:sha256:<derived, see below>",
  "parent": "<revision this was based on, or null>",
  "author": {"app": "waffle", "source": "<commit or workspace revision>", "at": "2026-11-03T12:00:00Z"},
  "driver": "waffle",
  "last_handover": null,
  "thickness_mm": 1.6,
  "geometry_hash": "<hash>",
  "approximate": false
}
```

`revision` is not the geometry hash. The crate derives it, `revision(manifest)`, by hashing
`geometry_hash`, `driver`, `parent` and `last_handover` together, and both apps must use that
function (or its documented encoding), never their own. Because the parent is part of it, a revision
always differs from its parent: a take over with no edits still gets a new revision, and a publish
that restores an earlier shape does not repeat an old one. Each revision commits to its whole
ancestry, as a git commit does. `geometry_hash` stays geometry only, so "did the shape change" is
still a comparison of geometry hashes. `author` is provenance and is not hashed into either.

Before board-interface 0.2.0 the revision was the geometry hash, so a take over with no edits had
`revision == parent`, and what followed it was misclassified (an edit after it looked like competing
take overs). Files written that way still read and compare, since `compare` treats revisions as
opaque strings.

### The geometry hash

Computed from parsed geometry, never from file bytes, so re-saving a file is not a design change:

1. Convert every coordinate to whole nanometres, KiCad's own unit, in KiCad's frame (Y down). Float
   metres would make a round trip change the hash when nothing changed.
2. Normalize each loop's direction and start point, and sort loops and elements, so entity order in
   the file does not matter.
3. Hash that canonical form with IDs and attributes, plus thickness.

Arc endpoints are hashed from the extended data, not recomputed from angles, where a value could land
on the other side of a rounding boundary. Loops chain through the same endpoints, so chaining needs no
tolerance.

**Publish nothing when nothing moved.** A sketch is solved numerically, so re-solving an unchanged one
can move a point by about 1e-13 m, enough to flip about one coordinate in 10⁴ across a nanometre
boundary. A publish first compares each element with the base within 1 nm, and writes nothing if none
moved.

Output is deterministic: no hash-map iteration order, no exponents in `f64` text, and one exactness
test file that both apps run in CI.

### Comparing versions

Each side remembers the base: the revision it last synced with. Only the driver publishes. A publish
from the other side is **refused, not merged,** and it also fails if the base has moved.

| Theirs vs base | Result on the locked side |
|---|---|
| Same | In sync |
| Changed by the driver | Offer to pull |
| Changed, and the manifest does not match the file | Changed outside the suite, import it? |

Merging holes and keepouts per element could come later if wanted. Outlines never merge.

### Handing over the drive

In separate repositories there is no shared lock, so "refused" can only be enforced by what each side has
seen. The revision chain settles it:

- Each publish names its `parent`. A take over is a publish whose `driver` differs from its parent's,
  with `last_handover` recording who and when. It may change no geometry at all, and it is still a
  publish with its own revision.
- The current shape is the newest revision reachable through parents, whichever repository holds it.
  The old driver sees the new revision through its link, shows the outline locked, and stops publishing.
  Its own file stays in its history and is not rewritten.
- **A take over counts only if its parent is the driver's latest revision.** Taking over from an older
  one would silently drop a change the person never saw, so it is reported like the next case.
- `last_handover.from_revision` records that revision. The new driver may publish again before the old one
  next checks, and then the newest manifest's parent no longer names the old driver's revision.
- Two revisions with the same parent mean competing take overs: both people changed it before either
  saw the other. The suite reports this, changes nothing, and a person picks one, as it does for edits
  outside the suite. So apps cannot silently overwrite each other, though they can still collide.
- **Both sides always hold a link to the other's interface, including the driver.** That link is how a
  driving document learns it no longer drives: on each check it compares the other side's newest manifest
  with what it last published.

In Waffle Iron a `BoardInterface` source holds what it follows, since `sources` holds inputs only. The
publish target and `last_published` live on the Board shape feature.

Inside a Boardroom project there are two editors, Composer and KiCad. Someone will edit `Edge.Cuts`
directly at some point. That appears as a third changed copy that can be imported, and a later
regeneration must not overwrite it. This follows Composer's hash rule already in
`platform-spec.md`'s seam.

### `board-interface`, the shared library

**The neutral record** is what the crate hands to both apps. Coordinates are whole nanometres (`i64`) in
KiCad's frame (Y down), the frame the hash uses:

- Arcs: centre, start, end and a direction flag, as a Waffle Iron sketch `Arc` is stored, so nothing is
  recomputed.
- Circles and holes: centre and **diameter**. A KiCad drill can be an odd number of nanometres, which a
  whole-nanometre radius cannot hold.
- Loops of elements, each with its ID string. Holes with `plated` and `clearance`. Keepouts with `side`
  and height limit.
- The manifest as its own struct: `driver`, `parent`, `last_handover` (who, when, and the revision taken over
  from), thickness, `approximate`.
- An arc's direction flag is `angle_increasing`: true when the sweep from start to end increases
  `atan2(y - cy, x - cx)` in KiCad's Y-down frame, which looks clockwise on screen.

**Functions:** `read(dxf, manifest)` returning a typed error for each kind of malformed input, `write`,
`hash`, `revision(manifest)` and `seal(record)` (geometry hash, then revision, called last before
`write`), `compare(base, theirs)` giving per-ID changes and the take-over classification, and `within(a, b,
1 nm)` for the publish-nothing rule.

**Rules:** no I/O and no clock, so callers pass bytes and times; no `HashMap` iteration in any output; a
version tag to pin; and golden files both projects run in CI: a round board, a rectangle with arc corners,
easy2350, a take over with no edits (`handover`), and one file per typed error.


A small Rust crate in its own repository, in the pattern of `board-draw`: no dependencies on any
app, clean under WASM, MIT. It reads and writes the DXF dialect, computes the hash, and does the
comparison and the diff, so Waffle Iron and Composer cannot drift apart in how they interpret it.
Waffle Iron's use stays out of its kernel stack (`feature-engine` and the WASM bridge), so its kernel
gates are not involved. With no `#[wasm_bindgen]` it has no wasm-bindgen dependency, so the version
difference between the projects does not arise. It reuses `dxf_export.rs`'s formatting rules: nine fixed
decimals, no exponents, `-0.0` written as `0.0`.

## Pulling, locking and taking over

If the bolt earring was shaped with sliders in Composer and Waffle Iron publishes a new outline, the
generator cannot reproduce it. Waffle Iron has the same problem in reverse with a constrained sketch.
Both apps behave the same way:

1. The incoming outline becomes read-only **synced geometry**, shown locked.
2. The old recipe is kept, set aside, with a clear warning.
3. Editing is an explicit **take over**, which warns what is lost, starts a new recipe from the synced
   shape, keeps its IDs, and changes `driver` on the next publish.

**In Waffle Iron,** a dedicated **Board shape** feature selects what is published. It references one
sketch and holds the board name, thickness, and a role per sketch entity: outline, cutout, hole (plated
or not, with clearance) or keepout (side and height limit). Pulled shapes arrive as `Derived`, so a
re-sync regenerates them in place and sketches on the board's faces still resolve (R1), with holes as
connectors keyed by interface ID. Waffle Iron has no DXF reader, so `board-interface` reads the file
into a neutral record and a new step in `feature-engine` derives from it, sharing the outline half of
`derive_board`. Editing the feature is an ordinary undoable edit. Publishing is not: like saving to
git, it writes outside the document. Taking over does not bring constraints across.

**In Composer,** outlines keep **true arcs** (the owner's decision):

- **Synced geometry** is carried as it arrived, arcs intact, with polygons used only for Composer's own
  algorithms. It never enters Clipper2, so it cannot change.
- **Outlines Composer authors** publish true arcs. Clipper2 works on polygons and its rounded corners
  are chords within 0.005 mm, so `Rect`, `Circle`, `Slot` and `corner_radius` must produce arcs as
  segments of the published shape. How far arcs survive the add and subtract booleans has yet to be
  worked out (Open questions). Anything still approximate is marked so in the manifest.

Thickness and keepout height limits are new fields in Composer's `shape.json`.

## What crosses, and in which direction

| Data | Edited in | Sync |
|---|---|---|
| Outline, cutouts, holes, thickness, height limits | Waffle Iron, Composer, KiCad | Either way, one driver at a time |
| Connector positions | Mostly Composer and KiCad, but an enclosure often dictates them | Both ways by footprint UUID, after the outline works |
| Other part placement | Composer, KiCad | To Waffle Iron only, for now |
| Copper and nets | KiCad, UltraRoute | Not shared, beyond the data Waffle Iron already reads |
| Enclosure geometry, the board's position in it | Waffle Iron | Stays in Waffle Iron |

**Footprint UUIDs must be stable.** Waffle Iron reconciles parts by them. easy2350's
`scripts/schematic.py` makes new schematic UUIDs on every run (seen 2026-11-02). Whether footprint
UUIDs change too must be checked for each generator (`shell-spec.md` mentions `--uuid-seed`).

## Version tracking

### Boardroom

New:

- **`design/links.toml`** for links and pins, as above.
- **A Versions view:** history, releases, each link's state, and the semantic diff between versions.
- **Gates record what they judged,** including each linked revision. A pin is then reproducible, and
  updating it marks the right gates stale. This extends `shell-spec.md`'s input hashes.
- **Releases record what they were built against:** "board rev B, built against enclosure rev C".
  A release on a pin behind the latest asks for a reason in `decisions.md`.

Composer's `packages.lock` and the per-project skill pin are this same idea built separately. New
pins follow the one pattern: what was asked for, plus what was actually loaded.

### Waffle Iron

It already has versioned sources. New:

- Publishing an interface, and the interface hashes.
- Named releases need nothing new: a git tag already is one (`waffle_v4_document_model.md:162`).
- Linked projects must be saved in a versioned store. Browser-only storage is still the default for
  Waffle Iron used alone.

## Where it runs: four stages

All stages use the same link records and the same adapter interface (`resolve_ref`, `fetch_blob`,
`put_file`). Only the backend changes.

| Stage | Runs in | Versions live in | What it gives |
|---|---|---|---|
| **1. Local native** | Boardroom's window, on this machine | Files and git in the project, through Boardroom's server | Linked projects, staleness, pinning, real history, the terminal |
| **2. Hosted git** | Same | GitHub and other hosts | Work across machines, and Waffle Iron standalone following a pushed interface |
| **3. Browser** | Pages | IndexedDB workspace | Linked projects with no install |
| **4. Worker relay** | Any | Still git. The relay carries notices only | Live updates across devices and people |

### Stage 1: local native window

Boardroom's server already gives one origin, a file API with revisions and conflicts, git and a bus.
Starting here (owner's direction) means versions are real commits from the first day, and removes what
the browser made hard: per-tab workspaces, storage eviction, separate stores per address, and a
new `Workspace` locator.

- **Normal use is a static build,** `BASE_PATH=/apps/waffle npm run build` from a host checkout at a pinned
  commit (no trailing slash; no Rust needed, as the WASM is committed). Boardroom's server serves that
  directory itself at `/apps/waffle/`, sending unknown paths to `index.html`, as routing is client-side.
  **Development** can proxy to Waffle Iron's dev server instead, published from its container on its
  registered port, with Vite's hot-reload socket passed through.
- **Boardroom's server implements `resolve_ref`, `fetch_blob` and `put_file` for local repositories,**
  so Waffle Iron sees unpushed commits without a desktop build of its own.
- `DocumentStore` grows `put(doc, {base}) → {rev} | {conflict}`, a rev from `get`, and optional `watch`,
  which Waffle Iron wants anyway, so conflicts are never silently overwritten.
- Waffle Iron fixes its root-absolute `/api` clients (`examplesApi.js`, `testCaseApi.js`,
  `assayCaseApi.js`), which would otherwise reach Boardroom's `/api`, and the manifest's `start_url`.
- Waffle Iron stays usable alone, and browser-only stays its default (`waffle_server_mode.md` C1).

Things stage 1 must handle:

- **One origin is one trust boundary.** Waffle Iron keeps git host tokens in `localStorage`
  (`git/tokens.js:8`, `github-auth.js:14`), readable by every app on that origin. Acceptable for our own
  apps, so nothing third-party may ever be served under it.
- **WebKitGTK.** Boardroom's window is WebKitGTK 2.52 on this machine, while the embedding test ran in
  Chrome. Waffle Iron has to be tried there first. Playwright's WebKit is built from the same port, so its
  nightly run is a fair stand-in for the engine: one failure, a drag test. Known traps: mouse coordinates
  truncated to whole pixels, and two clicks in a row are not a double-click. Chromium's app window, which
  Boardroom also supports, is the fallback.
- **What the shell must do, which Playwright cannot show.** Checked against `app/gtk/src/main.rs`:

  | Need | State |
  |---|---|
  | One persistent website-data directory shared by all windows | Done, at the location WebKit already used, and survives a restart |
  | Handle the `download` signal | Done: saves to Downloads, numbered on a clash, with a notice bar |
  | WebGL and hardware acceleration | Done. WebKitGTK reports a fake "Apple GPU", so read the process's mapped libraries to check |
  | No shell shortcuts taking Ctrl+Z, Ctrl+S, Delete or Escape from the page | None registered. Synthetic events reach the page; a real keyboard is still to be checked by hand |
- Sign-in and the relay's WebSocket under COOP and COEP, not yet tested. Waffle Iron does not need
  those headers itself and `require-corp` should not block its CORS traffic.

### The embedded terminal

`shell-spec.md` already has Boardroom's server owning terminals over `/api/term/<id>`, as real Claude
Code sessions. It is not built. Waffle Iron's half mostly exists: its relay is an MCP server paired with
one tab, and its tools (sketching, features, assemblies, measuring, export) run in that tab's engine. A
terminal session given that relay can therefore work on the document on screen, the same way it runs
`ccomp` for Composer.

- **The relay belongs to the session, not to Boardroom.** It serves one agent and one tab, so the project's
  `.mcp.json` declares it and Claude Code starts it, on the host, with `--app-url`, `--allow-origin` set to
  Boardroom's origin, and `--persistent-link` pointing to a secret under a gitignored `.boardroom/`.
- **Pairing:** the agent calls `waffle_connect`, the frame opens the link and the person consents once. The
  persistent link then survives relay restarts, so Boardroom can load the frame at it.
- **The tab connects out to the relay,** so nothing is proxied. COEP should not cover WebSockets, but this is
  untested and on the stage 1 checklist.
- **Viewport tools need a visible tab.** A Waffle Iron tab behind another could have its rendering paused, so
  those calls could hang. Either bring it forward through a `focus` message on the bus, or refuse with a clear
  message. Other tools run in the engine worker and are unaffected.

**One shell.** Waffle Iron does not build a second window. Alone, it opens in the same binary, for example
`boardroom-gtk --app waffle`, with no project chrome. More speed comes later from `waffle-host` running the
engine (a 44-feature model loaded in 168 s natively against 274 s in WASM), with the window as its viewer.

### Tried for real, 2026-11-03

Waffle Iron's static build at `/apps/waffle/` in `boardroom-gtk` on WebKitGTK 2.52: the engine boots in about
1 s with no console errors, on hardware WebGL2 (NVIDIA). A solid modelled through `window.__waffle`, exported as
an STL through the real download path, saved to Waffle Iron's IndexedDB, and reopened after a full restart.
Not tested: a real keyboard, picking precision, camera feel, sign-in and write-back, and the relay's WebSocket.

### Stage 2: hosted git

Waffle Iron's existing hosted adapters cover everything that is pushed.

### Stage 3: browser

Waffle Iron runs under Boardroom on Pages, on the shared origin the embedding test used. It gets a
workspace storage provider and a `Workspace` locator, since `Local` is not shareable. Then:

- **Lasting workspaces.** `demo-spec.md` gives each tab its own. A permanent link needs a named project
  that survives across tabs.
- **Data safety.** Browser storage can be cleared or evicted, and stays on one device. Project export
  and import come early, with a "not backed up" warning and `navigator.storage.persist()`.
- **Separate addresses mean separate stores.** Existing Waffle Iron documents need importing.

### Stage 4: Cloudflare Worker relay

Git stays the source of truth. Two browsers can already sync through GitHub, so the relay adds speed
and reach, not storage: pushing "a new revision exists" to other devices, passing on GitHub webhooks,
sign-in (`worker/github-oauth-proxy` in Waffle Iron is the existing precedent), and live state on a
phone or another machine. Keeping designs out of it avoids a second source of truth with its own
access control and backups. A Worker cannot reach `localhost`, so a local Boardroom connects out to
it. It is separate from Waffle Iron's local agent relay in `relay/`.

## Normalizing the toolkit

Boardroom, Composer and UltraRoute do not move onto Waffle Iron's architecture. All four already use
Rust, WASM and Svelte 5, and Boardroom's local server does a different job from a document editor in
a browser. Repositories stay separate, with pinned revisions. What changes is discipline.

| | Waffle Iron | Boardroom | Composer | UltraRoute |
|---|---|---|---|---|
| Rust tests (rough count of test attributes) | 6,619 | 6 | 129 | 324 |
| CI workflows | 6 | Pages only | Pages only | Pages + tests |
| Git hooks | 2 | 0 | 0 | 0 |
| Rust toolchain pin | 1.97.1 | none | none | 1.95.0 |

**Adopt from Waffle Iron:** exact toolchain pins, hooks, real CI tests (Boardroom first), typed
boundaries (versioned schemas for `status.json` fields and file formats), `Derived`-style provenance
alongside Composer's set/filled/open, the live-app MCP design as the common agent surface, and a
lighter version of `governance/`.

| Item | Now | Change |
|---|---|---|
| Rust | 1.97.1 / 1.95.0 / unpinned | One exact pin everywhere |
| Vite | Waffle Iron 6, others 8 | Waffle Iron to 8, carefully: it needs a newer Svelte plugin too |
| wasm-bindgen | Waffle Iron 0.2.114, others 0.2.129 | Waffle Iron up. Required before sharing WASM crates |
| Playwright | 1.48 to 1.64 | One version |
| three.js | 0.169 / 0.170 | One version |
| Node | 22 on this machine, 20 in Waffle Iron's deploy workflow | Pin 22 with `.nvmrc`. Node 20 is past end of life |
| `board-draw` | Two pinned revisions | One |
| `kicad-pcb` | UltraRoute 6,802 lines read/write; Waffle Iron 2,233 lines read-only; Composer pins all of UltraRoute to get it | A standalone crate, with Waffle Iron moving last because of its golden tests |

## Milestones

1. **Embedding feasibility.** Passed on 2026-11-03, with the limits listed under "Embedding test".
   Waffle Iron's large-file bug, found by that test, is fixed on its `main`.
2. **Toolchain and CI** for the three newer projects. Low risk, and can run alongside the rest.
3. **`board-interface` and the automated round trip.** See "Acceptance tests". This is what proves the
   data model.
4. **Waffle Iron in Boardroom's window:** try it in WebKitGTK, the shell's storage and downloads,
   `/apps/waffle/`, Waffle Iron's fixes, and the local repository adapters.
5. **Waffle Iron** publishes and pulls: the Board shape feature, `BoardInterface` sources, derive from a
   shape, kept IDs.
6. **Composer** reads and writes the interface: true arcs, `ARC`, `POLYLINE`, layers, hole IDs, board
   thickness, keepout height limits, a DXF writer, the locked state and unlock.
7. **The owner's hands-on test.** See "Acceptance tests".
8. **Links and versions in Boardroom:** `design/links.toml`, link states, the Versions view, gates
   recording revisions.
9. **The embedded terminal,** reaching Waffle Iron through its relay.
10. **Hosted git,** then connector positions, shared UI package and shared `kicad-pcb`.
11. **Browser stage,** then the **Worker relay.**

### Who does what, and at what pace

- **Boardroom agent:** `board-interface`, Composer's side, Boardroom's static serving, shell work and local
  repository adapters.
- **Waffle Iron agent:** the `feature-engine` round-trip test, the derive step, the Board shape feature, its
  `/api` and `start_url` fixes, and the `DocumentStore` change.

**Waffle Iron's kernel 1.0 stays first (owner, as reported by its agent).** Its pieces are built alongside by
delegated agents, starting with the small app fixes, then the Board shape feature and derive step once the
record is agreed. Work that needs Waffle Iron sooner is asked for explicitly and goes to the owner.

### Acceptance tests

`board-interface` 0.1.0 passes its side of the first test below: easy2350's golden matches KiCad's own DXF
export in area, perimeter and bounding box, and a reader fuzz of 400,000 mutated files found no panics.
0.2.0 derives `revision` in the crate (see `board-shape.json`), fixing a false conflict after a take over
with no edits ([board-interface#1](https://github.com/sequoia-hope/board-interface/issues/1)).

**Automated, for the data model.** No UI. easy2350's outline goes from KiCad to the DXF, through Composer,
through Waffle Iron and back, and the geometry hash must be identical at the end. easy2350's rounded
corners exercise the arcs that Composer cannot read today. Waffle Iron's part is a Rust test in
`feature-engine`: read the files, derive the `Derived` features, rebuild the board solid through the real
kernel adapter, read the shape back through the Board shape feature, publish, and compare hashes. It also
checks that the solid's volume equals area × thickness, which catches a cutout that came in as an added
region. It covers the Y flip, arc direction under the flip, arcs through a sketch `Arc`, and IDs
surviving a derive.

**By hand, for the whole experience (owner).** In Boardroom's window, draw a circular or rectangular
board outline in Waffle Iron and publish it. It appears in Composer, locked, with "Outline controlled by
Waffle Iron". Change it in Waffle Iron, and Composer reports the update and pulls it. Unlock it in
Composer, see the warning, change it there, and Waffle Iron then shows it locked.

## Embedding test (2026-11-03)

Waffle Iron's deployed build was run in an iframe, in Chrome driven by Playwright, the way Boardroom
embeds its other apps (`allow="cross-origin-isolated"`). No repository was changed. Two setups:

- **Pages:** the deployed Boardroom page and the deployed Waffle Iron, on their real shared origin,
  with no isolation headers (Pages cannot send them).
- **Local:** the live Boardroom page, with Waffle Iron served at `/waffle-iron/` by request
  interception and Boardroom's COOP, COEP and CORP headers added. The frame reported
  `crossOriginIsolated: true`.

Both setups, unless noted:

| Check | Result |
|---|---|
| Waffle Iron's engine boots in the frame | Yes, in 0.8 s on Pages and 3.8 s under interception |
| Same origin as the parent; `localStorage` and `BroadcastChannel` shared both ways | Yes |
| Frame calls GitHub's API and raw hosts under COEP | Yes |
| Sketch and extrude a solid, and it renders | Yes (2 features, 12 triangles) |
| A real mouse click through the frame selects a face | Yes |
| easy2350 (1.29 MB) written to IndexedDB by the parent, read by the frame, imported | Yes (board solid, 58 footprints, `KicadPcb` source) |
| Storage names collide | No: `boardroom-store` and `waffle-iron*` |
| Waffle Iron's "update" and "pin" controls appear on a git-linked board | Yes |

### Bug found, not caused by embedding

**Linking a `.kicad_pcb` over 1 MiB from GitHub fails**, standalone and embedded, with `syntax error
at line 1, column 1: expected '('`. For such files `fetchBlob` in Waffle Iron's
`app/src/lib/storage/git/hosts.js` gets a JSON reply with no content, then requests the same URL again
with the raw media type. GitHub sends `Vary: Accept` but gives both replies the same ETag. The browser
can revalidate its cached JSON with that ETag, get a 304, and return the JSON, so the parser receives
that instead of the board.

Measured on the deployed build: with the default cache the second request returned 1,303 bytes of JSON;
with `cache: 'no-store'` it returned the 1,294,055-byte board. A regression test against a real local
server with GitHub's headers failed the same way and passes with `cache: 'no-store'` on that request.
That fix landed on Waffle Iron's `main` as `592c813f`. Git links on the local
setup passed only because Playwright's request interception turns the HTTP cache off, so that pass is
not evidence that local works.

### Other observations

- The board can look missing after an import into a document that already holds a model far from it:
  the camera stayed on the first model at the origin, and easy2350 sits about 100 mm away. On a fresh
  document the camera frames the board. This looks like a view-fit gap rather than an embedding problem,
  but the first case was only seen embedded.
- The only other errors logged were 404s from Waffle Iron probing for a `.step` and a `.stp` beside the
  board, which is expected.
- A board with 58 footprints gives about as many tabs, truncated to a letter or nothing. That is not
  specific to embedding, but the narrower frame makes it worse.

### Not tested

- The real `/apps/waffle/` proxy with a `BASE_PATH=/apps/waffle` build. Local used Waffle Iron's Pages
  build and interception, so these headers and this origin are representative, but the route is not.
- Keyboard shortcuts in the frame, Waffle Iron's sign-in and write-back under COOP and COEP, its agent
  relay's WebSocket, and saving a document and reopening it across a reload.
- Browsers other than Chrome, and real GPUs (Chrome ran on SwiftShader).

## Open questions

1. **Separate repositories as the default?** Recommended, with one-repo products through a `Relative`
   locator, and Waffle Iron agrees. (Owner)
2. **DXF extended data in other CAD programs.** `ezdxf` keeps it through a read and re-save, with an identical
   record and hash, and files without it are read and matched by geometry. `kicad-cli` cannot re-save a DXF.
   LibreCAD and KiCad's and others' interactive import are untested. (`board-interface`)
3. **Arcs through Composer's booleans.** Which arcs survive add and subtract, and how? (Composer)
4. **Which Composer outline wins inside a project** when a person edits `Edge.Cuts` and then Composer
   regenerates? (Composer)
