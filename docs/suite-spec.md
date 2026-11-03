# Boardroom and Waffle Iron: an electromechanical suite — spec (draft)

Status: draft, 2026-11-03. Written from the owner's direction for four sibling projects
(Waffle Iron, Boardroom, Circuit Composer, UltraRoute) to read and push back on. Nothing here is
built yet except what "What exists today" lists. Choices the owner has not confirmed are marked
**Recommended**, and every open one is under **Open questions**.

Builds on [`platform-spec.md`](platform-spec.md), [`shell-spec.md`](shell-spec.md) and
[`demo-spec.md`](demo-spec.md), and on Waffle Iron's `specs/waffle_v4_document_model.md` and
`specs/kicad_board_link.md`.

## The idea in one paragraph

Mechanical CAD and board design become one suite. A Waffle Iron project and a Boardroom project
stay **linked for as long as the product is being developed**. Either side can change at any
time, the other side can see that it is behind and what changed, and the person chooses to
update or to stay pinned on an older version. The board outline can be designed in Waffle Iron
or in Circuit Composer and synced in either direction. There is no start or finish to this
workflow, and neither app is the permanent owner of the board's shape.

## The owner's direction

1. **A continuous lifecycle, not a handoff.** Board and CAD designs are always being updated.
2. **The outline syncs both ways.** Design it in Waffle Iron or Composer and sync it either way.
3. **Link, stale, update or pin.** A Waffle Iron file or project is linked to a Boardroom project.
   If either is updated the other reports it, and can sync to the latest or stay pinned.
4. **Version tracking is a suite feature.** It was missing from Boardroom's spec, and should be
   considered for Waffle Iron as well.
5. **DXF carries the shared geometry,** with Waffle Iron's exporter extended to put the necessary
   information into it.
6. **First the same browser tab, later a relay.** Waffle Iron starts as a micro-app under Boardroom
   in one tab, sharing browser storage. A connector or relay on a Cloudflare Worker comes later.
7. **Normalize the newer projects toward Waffle Iron's toolkit,** including plain upgrades where
   they help.

## What exists today

| Piece | Where | State |
|---|---|---|
| Board import into CAD: outline as a derived solid, one instance per footprint, a connector on each mounting hole | Waffle Iron `specs/kicad_board_link.md` | Landed, one-way. Writing back is a stated non-goal |
| Re-sync that keeps the user's work, reconciling parts by footprint UUID | same, rows R1–R6 | Landed |
| Links pinned to a commit or following a branch or tag, with the loaded commit and a content hash | Waffle Iron `crates/file-format/src/sources.rs`, v4 spec §2.4 | Landed |
| "Update to tip" and "Pin" as explicit actions | Waffle Iron Sources panel, v4 spec §7.3 | Landed |
| Host adapters: `resolve_ref`, `fetch_blob`, `put_file` | Waffle Iron v4 spec §7.2 | Landed for GitHub, GitLab, Gitea. Local git is listed as a future desktop host |
| `Derived` provenance: read-only, replaced by a re-sync | Waffle Iron `feature-engine` | Landed |
| One-view DXF writer, R12, `LINE`, `ARC`, `CIRCLE`, `POLYLINE`, layers | Waffle Iron `crates/kernel-v2/src/dxf_export.rs` | Landed |
| DXF reader: `LWPOLYLINE`, `CIRCLE`, `LINE` only | Composer `crates/shape/src/import.rs` | Landed. Cannot read Waffle Iron's `ARC` or `POLYLINE`, ignores layers, flattens arcs, drops open chains |
| Apps embedded on one origin | Boardroom `/apps/<name>/`, `docs/shell-spec.md` "One origin" | Landed for Composer and Studio |
| Browser workspace with per-file revisions and a history | `docs/demo-spec.md` | Specified, one workspace per tab |
| Gates going stale when judged inputs change | `docs/shell-spec.md` "Gates tab" | Specified. The server reads a stale reason; computing one is not confirmed |
| Git ahead/behind per project | Boardroom `app/server/src/project.rs` | Landed |
| `.kicad_pcb` reader and writer | UltraRoute `core/crates/kicad-pcb`, used by Composer through a git pin | Landed. Waffle Iron has its own smaller reader with a shared origin |

## Principles

1. **Share results, keep recipes private.** A Waffle Iron constraint means nothing to Composer, and
   Composer's generators mean nothing to Waffle Iron. Only resolved geometry crosses between apps.
2. **Each project owns its own history.** Nothing writes to a file that two projects share. Each
   project publishes its own interface and follows the other's.
3. **A version is a git commit, a release is a git tag.** The suite does not invent a version
   system. Where there is no git yet (stage 1), content-addressed revisions stand in with the same
   interface and map onto git later.
4. **Stale means the interface changed.** A change the other side does not depend on is never
   reported as work to do.
5. **Sync is explicit (Recommended).** A half-finished sketch must not move a routed board. Every
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

In a Waffle Iron document it is a new source kind, `BoardInterface`, in the existing `sources`
table. Adding a kind and the `Workspace` locator below is part of a format version bump.

### Link states

| State | Meaning | Shown as |
|---|---|---|
| **Current** | Latest version, no interface differences | Quiet |
| **New revision, no interface change** | The other project committed something this side does not depend on | Quiet. The loaded commit advances on the next check |
| **Update available** | Following a ref, and the interface changed | Notice with a diff and its consequences |
| **Pinned, behind** | Deliberate. "Pinned at rev B, latest is D, two holes moved" | Information, never a warning |
| **Diverged** | Both sides changed since the last sync | Boardroom inbox item |
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

Two interface hashes are needed: **board shape** (below), and **mechanical placement** for what
Waffle Iron reads from a board: footprint UUID, side, position, rotation and 3D model.

## The interface files

```
interface/<board>/board-shape.dxf     outline, cutouts, holes, keepouts, with IDs
interface/<board>/board-shape.json    revision, parent, author, thickness, geometry hash
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
Open questions). Millimetres, board coordinates, KiCad's frame (Y down), so board files need no
conversion. Waffle Iron converts at its boundary as `kicad_board_link.md` §2.3 already specifies.

| Layer | Entities | Meaning |
|---|---|---|
| `BOARD_OUTLINE` | `LINE`, `ARC`, `CIRCLE` | One closed outer loop |
| `BOARD_CUTOUT` | `LINE`, `ARC`, `CIRCLE` | Closed inner loops |
| `BOARD_HOLE` | `CIRCLE` | Drilled holes |
| `BOARD_KEEPOUT` | `LINE`, `ARC`, `CIRCLE` | Closed keepout loops |

Extended entity data under the application name `BOARD_INTERFACE` attaches, per entity:

- `id`: a stable ID. Without one, a moved hole looks like one deleted and one added, which would
  break the mates attached to it. Composer's `Hole` has no ID today and needs one.
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
  "revision": "<geometry hash>",
  "parent": "<revision this was based on, or null>",
  "author": {"app": "waffle", "source": "<commit or workspace revision>", "at": "2026-11-03T12:00:00Z"},
  "thickness_mm": 1.6,
  "geometry_hash": "<hash>",
  "approximate": false
}
```

### The geometry hash

Computed from parsed geometry, never from file bytes, so re-saving a file is not a design change:

1. Convert every coordinate to whole nanometres, KiCad's own unit. Float metres would make a round
   trip change the hash when nothing changed.
2. Normalize each loop's direction and start point, and sort loops and elements, so entity order in
   the file does not matter.
3. Hash that canonical form with IDs and attributes, plus thickness.

Whether arc endpoints survive centre, radius and angle to the nanometre is unproven. The round-trip
test below settles it. If they do not, endpoints are also carried in extended data.

### Comparing versions

Each side remembers the base: the revision it last synced with.

| Mine vs base | Theirs vs base | Result |
|---|---|---|
| Same | Same | In sync |
| Changed | Same | Offer to publish |
| Same | Changed | Offer to pull |
| Changed | Changed | Diverged, unless the changes merge |

Merging is per element: if one side moves a hole and the other adds a keepout, both apply
automatically. The outline itself is all-or-nothing, because merging two edited outlines can produce
an invalid shape. Publishing fails if the base has moved, which also protects against two apps
writing at once.

Inside a Boardroom project there are two editors, Composer and KiCad. Someone will edit `Edge.Cuts`
directly at some point. That appears as a third changed copy that can be imported, and a later
regeneration must not overwrite it. This follows Composer's hash rule already in
`platform-spec.md`'s seam.

### `board-interface`, the shared library

A small Rust crate in its own repository, in the pattern of `board-draw`: no dependencies on any
app, clean under WASM, MIT. It reads and writes the DXF dialect, computes the hash, and does the
comparison and the diff, so Waffle Iron and Composer cannot drift apart in how they interpret it.

## Pulling over a recipe

If the bolt earring was shaped with sliders in Composer and Waffle Iron publishes a new outline, the
generator cannot reproduce it. Waffle Iron has the same problem in reverse with a constrained sketch.
Both apps behave the same way:

1. The incoming outline becomes read-only **synced geometry**.
2. The old recipe is kept, set aside, with a clear warning.
3. Editing is an explicit **take over**, which starts a new recipe from the synced shape.

Waffle Iron has much of this. `Derived` features are read-only, and a re-sync regenerates them in
place so sketches on the board's faces still resolve (R1). Publishing and taking over are new.
Updating dimensions while keeping constraints when only sizes changed is a later improvement.

Composer's resolved outline is polygons, so a Waffle Iron arc would come back as many short lines
and the two outlines would never compare equal. Composer must carry arcs through and use polygons
only for its own algorithms.

## What crosses, and in which direction

| Data | Edited in | Sync |
|---|---|---|
| Outline, cutouts, holes, thickness, height limits | Waffle Iron, Composer, KiCad | Both ways |
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
- Named design releases, if they do not already exist (to confirm).
- Linked projects must be saved in a versioned store. Browser-only storage is still the default.

## Where versions live: three stages

All three use the same link records and the same adapter interface (`resolve_ref`, `fetch_blob`,
`put_file`). Only the backend changes.

| Stage | Versions live in | What it gives |
|---|---|---|
| **1. Same tab** | IndexedDB workspace on Pages; Boardroom's server for local projects | Linked projects, staleness and pinning, no accounts |
| **2. Git** | GitHub, or a local repo through Boardroom's server | Durable history, releases, work across machines |
| **3. Worker relay** | Still git. The relay carries notices only | Live updates across devices and people |

### Stage 1: same tab

Waffle Iron runs as a micro-app under Boardroom. Pages already gives one origin
(`sequoia-hope.github.io/waffle-iron/`, `…/boardroom/`), and locally the `/apps/` proxy does.

- Add `waffle` to `APPS` in `app/server/src/apps.rs`, built with `BASE_PATH=/apps/waffle`.
- Waffle Iron gets a Boardroom workspace **storage provider**, behind its existing provider
  interface.
- Storage is IndexedDB, not `localStorage` (`demo-spec.md`: about 5 MB, strings only, and it blocks).
- Change notices use the existing `BroadcastChannel('boardroom')` messages.
- Workspace revisions use git-style blob hashes, so they map onto real git at stage 2.
- A new `Workspace` locator, because Waffle Iron treats `Local` as not shareable and its KiCad link
  refuses it.

Things stage 1 must handle:

- **Lasting workspaces.** `demo-spec.md` gives each tab its own. A permanent link needs a named
  project that survives across tabs.
- **Data safety.** Browser storage can be cleared or evicted and stays on one device, and clearing
  the origin's site data wipes every app at once. Project export and import come early, with a "not
  backed up" warning and `navigator.storage.persist()`.
- **Separate addresses mean separate stores.** Pages, `localhost` through Boardroom, and Waffle
  Iron's own port do not see each other's data. Existing Waffle Iron documents need importing.

### Stage 2: git

Waffle Iron's adapters work against hosted git, so it cannot see an unpushed Boardroom commit.
Boardroom's server implements the same three calls for local repositories, which gives Waffle Iron
local versions without a desktop build. Waffle Iron's existing hosted adapters cover everything
that is pushed.

### Stage 3: Cloudflare Worker relay

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
   Fix Waffle Iron's large-file bug, found by that test, before relying on linked boards.
2. **Toolchain and CI** for the three newer projects. Low risk, and everything below relies on it.
3. **`board-interface` and the round trip.** See the acceptance test.
4. **Composer** reads and writes the interface: `ARC`, `POLYLINE`, layers, arcs carried through, hole
   IDs, a DXF writer.
5. **Waffle Iron** publishes and pulls: a sketch-level writer beside the existing one, a DXF reader if
   it has none, `BoardInterface` sources.
6. **Links and versions in Boardroom:** `design/links.toml`, link states, the Versions view, gates
   recording revisions.
7. **Embedded for real:** `/apps/waffle/`, the workspace storage provider, the `Workspace` locator.
8. **Git backend,** then connector positions, shared UI package and shared `kicad-pcb`.
9. **Worker relay.**

### Acceptance test for the data model

No UI. easy2350's outline goes from KiCad to the DXF, through Composer, through Waffle Iron and back,
and the geometry hash must be identical at the end. easy2350's rounded corners exercise the arcs that
Composer cannot read today. If this holds, the model works.

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
with the raw media type. GitHub's reply carries no `Vary` header that the browser honours here, so the
browser's HTTP cache returns the JSON again, and the parser receives that instead of the board.

Measured on the deployed build: with the default cache the second request returned 1,303 bytes of JSON;
with `cache: 'no-store'` it returned the 1,294,055-byte board. The likely fix is passing `cache:
'no-store'` on that request. Git links on the local setup passed only because Playwright's request
interception turns the HTTP cache off, so that pass is not evidence that local works.

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

1. **Sync explicit?** Recommended yes. Automatic sync would feel more seamless, but an edit in one app
   could then invalidate a routed board without anyone choosing that. (Owner)
2. **Boardroom as the suite's main window?** Recommended yes, with Waffle Iron fully usable alone.
   Boardroom's phases and gates would then have to allow mechanical-only projects. (Owner)
3. **Separate repositories as the default?** Recommended, with one-repo products supported. (Owner)
4. **DXF R12 and extended data.** R12 is believed to support extended entity data, which needs its
   application name in the `TABLES` section that Waffle Iron does not write today. To confirm against
   a real parser and against the CAD tools people will re-save with. (Waffle Iron)
5. **Arc precision through DXF,** to the nanometre. (Round-trip test)
6. **Does Waffle Iron need a DXF reader?** Only an exporter was found. (Waffle Iron)
7. **Which Composer outline wins inside a project** when a person edits `Edge.Cuts` and then Composer
   regenerates? (Composer)
8. **Mechanical-placement hash:** exactly which fields does Waffle Iron depend on? (Waffle Iron)
