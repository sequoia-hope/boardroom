# The old router, read again: what it does and what to share

Written 2026-11-25 at the owner's request: "read the old router code ... make sense of how it
works, and just consider whether any of those techniques could be useful ... to the extent that
we are borrowing code, we should just share it and make a shared library between the old code
and the new code and let them coexist."

The old router is `~/Software/ultraroute` (main at `0736180`). Its MIT core is
`core/crates`: `grid` (24,800 lines), `morph` (10,700), `kicad-pcb` (6,900), `ultraroute` the
driver (6,600), `board` (5,200), `topo` (2,000). `core/crates/kicad-text`, `copperroute/`,
`copperroute-main/` and `freerouting/` are GPL and are not part of this.

How it was read: I read `grid/src/nudge.rs`, `grid/src/push.rs` and the head of
`morph/src/compact.rs` myself. Five readers took the rest in parallel, each with a narrow
brief and told to give file and line for every claim. Where a statement below is a reader's
and not mine it is marked (reader). Nothing was built or run for this document.

## 1. What the old router is

A grid router with repair stages round it. A board is rastered at 50 µm (25 µm for the
owner's pendant). One connection at a time is searched for on the grid, the path is turned
into track, and **every piece laid is put on the board and asked of the exact checker**
(`board::check_item`); a piece that breaks a rule takes its whole route back off. The grid is
deliberately conservative, so it may refuse a legal path and never accepts an illegal one
(reader: `grid/src/grid.rs:1-5`, `route.rs:1074-1079`).

Twenty-four stages run in a fixed order (reader: `ultraroute/src/lib.rs:1248-1274`). The ones
that matter:

1. **Ways out of fine-pitch pins first** (`prefanout.rs`): each such pad gets a stub, a
   dogbone or an escape on a 10 µm grid, depths staggered by pad, each piece checked.
2. **Negotiation** (`negotiate.rs`): every connection is routed with the others' copper
   priced, the price of sharing rising 1.6 times a round for up to 30 rounds; then the routes
   are committed greedily, each through the exact check, and what does not fit is tried again
   against what was laid.
3. **Rip-up** (`ripup.rs`): a blocked connection is searched for with the router's own copper
   passable at a price; its route is laid, the checker names what it hits, those pieces are
   taken off and routed again. **Kept only if fewer connections are open; otherwise the board
   is put back from a snapshot.**
4. **The plough** (`shove.rs`, called shove): for a connection still open, a priced path finds
   where it is pinched; other nets' movable copper within reach of the pinch (0.5 mm, doubling
   to 4 mm on each failure) is taken off; the trapped net and those nets are negotiated
   together with the trapped one favoured. Kept if fewer are open, or, within limits (two per
   net, six in all), if it frees the trapped connection and leaves no more open than before.
   Up to 60 passes, stopping after 8 or 12 that close nothing.
5. **Clean-up** (`cleanup.rs`, `vias.rs`): tracks pulled taut, squared onto 45 degrees,
   doubled and dangling copper dropped, vias that are not needed removed, joints mended.

## 2. Why it closes the last connections

In order of weight. The judgement is mine; the facts under each are the readers'.

**A local search on finished copper that keeps only improvements.** Steps 3 and 4 above are
one idea: snapshot the board, tear up what is near the trouble, route it again with the stuck
connection first, count what is open, keep or put back. It may tear up anything not locked,
its own work included. On p2-132-usb6 (run again 2026-11-24, the lab's
`old/p2-132-usb6-2026-11-24/`): the full route left 10 open; a separate finishing pass on
that board (`--adopt --shove-plough region`, 600 s) went 10, 2, 2, 2, 2, 2, 1, 0 over seven
passes in 345 s (reader: `plough.log:25-31`). Our router has no stage after its copper is
drawn that may take committed work up again.

**The checker is the last word, and says whom to blame.** A violation names the other item.
Rip-up uses that to pick exactly which pieces to lift (reader: `ripup.rs:391-423`).

**The finisher has its own time.** Inside the main run the plough got one pass before its
deadline; alone it needed seven, four of them closing nothing (reader: `s0.log`,
`plough.log`).

**Ways out first.** On the earring this alone took 10 open to none (reader:
`docs/focus-trapped-escapes.md:176-187`).

**Many tries at a walled-in end:** a finer grid in a small window, legs to sixteen free cells
near a pad, a track necked only across the gap it cannot pass (reader: `route.rs:813-957`).

What it is *not*: there is no clever search that finds room. There is no piecemeal search over
free regions, and no energy that guides a route (looked for by two readers and by me; not
found). The dipole plough that pushed copper away step by step was measured against the
others on the earring (one run each, under load, so noisy: 10 open against 6 for the warp
and 11 for doing nothing) and was later deleted (`docs/focus-trapped-escapes.md:55-60`).

## 3. The nudge: what is there and what is not

The owner asked whether we have "the nudge capabilities of the old router where we can nudge
without DRC violations".

- **`grid/src/nudge.rs` moves parts, not vias** (read myself). For an open connection it
  tries a footprint near it 0.25, 0.5 and 1 mm in eight directions. `push.rs` carries the
  copper round the part with it by a smooth field, **rips whatever the flow pushed into
  something it may not touch**, routes that again, and keeps the step only if fewer are open.
  It is off by default. On the earring "nearly every step is illegal and none helps"
  (`ROADMAP.md:137`).
- **`morph/src/compact.rs` is the mover that never breaks a rule** (head read myself, the rest
  a reader's). All copper is one graph of points and segments; a via is one point every track
  ending at it shares, so nothing comes apart when it moves. Each clearance shortfall becomes a
  push shared between the two sides by how free each is to move (a via 0.6, a track corner
  1.0, a point on a pad 0). No point moves more than 35 µm an iteration. After each step the
  whole board is checked: no more violations than at the start, no new open connection, no via
  out of its plane; else the last good state is restored. It never rips and never reroutes.
  **It was only ever used to shrink boards**: nothing in it drives a chosen via toward a
  place. And **it leaves copper at any angle**: its only rule about angles is no acute corner,
  and segment counts about double (`docs/research/compaction-v1-2026-11.md:242`).
- Inside it, `slide_pinched_vias` (reader: `compact.rs:3770-4408`) is the nearest thing to a
  hand's nudge: a via that breaks a rule is slid 0.1 to 1.6 mm along its own net's track, or
  stepped out to one of 5 radii in 12 directions; the via and its tracks are put on the board,
  checked, and the first clean place wins. It moves one via and does not push its neighbours.

So the answer stands: nothing in the old router nudges one via or track at a time, checked,
with neighbours pushed in turn. The pieces of one are there.

## 4. What is worth sharing, and what each costs

Our `board` crate is the old one with additions: geometry, items, insert and remove,
`check_item` and the connectivity functions have the same shapes (reader:
`docs/one-system-spec.md:134`, and a diff of the two). The checker's verdicts have since
changed in places (spokes, kept pours, mask openings), so code that leans on it must be
measured again, not assumed.

| What | Where | For us | Cost |
|---|---|---|---|
| Two vias of a net whose copper overlaps: one deleted if the net stays joined, else bridged | `cleanup.rs` `mend_via_grazes` (595) | The owner's SW2_4 note | Small. Needs four joint helpers the new `board` left behind |
| A via pair replaced by track on one layer; a redundant loop deleted | `vias.rs` `try_replace` (200) | The same, and fewer vias generally | The delete is small; the replace needs a local reroute, so our planner |
| A track end drawn on into the pad or via it stops in, to its middle | `cleanup.rs` `mend_pad_joints` (825), `collapse_via` (919) | The owner's CS1_17 note (done another way on 2026-11-25: a joining piece) | Small |
| Tails past a joint trimmed, doubled track dropped, turn-backs straightened | `cleanup.rs` `trim_overhang`, `drop_duplicates`, `straighten_turn_backs` | The owner's "overlapping traces" note | Small |
| Snapshot, tear up near the pinch, route again stuck one first, keep if fewer open, widen the reach, bounded ties, its own budget | `shove.rs`, `ripup.rs` | The missing finisher | No code to take: it is a discipline, and its routing is the grid's. Built on our planner (begun 2026-11-25) |
| The smallest move that satisfies a set of pushes | `compact.rs` `solve_half_planes_2d` (5900, 96 lines), `blame.rs` `satisfy` (384) | Where a nudged via should go | Small |
| Move one via, drag its tracks, check, restore | `compact.rs` Mode C (reader: about 100 lines) | A checked via nudge on drawn copper | Small; any-angle tracks unless re-squared |
| The whole relaxer | `compact.rs` (about 1,900 of 7,700 lines) | Cascading nudges on drawn copper | Ports cheaply (four timing calls, a renamed crate). **Spoils the look** unless followed by squaring |
| The grid router itself, as a second formula | `grid` | A closer for what ours leaves | Does not port cheaply: about 230 timing and probe calls, the region rules and joint helpers our `board` dropped. It runs today as a built binary on a board of ours (`--adopt`) with nothing ported |

## 5. Recommendation

The new router goes on making the copper: its look is not to be disturbed. The old code's
place is as **operators on copper that is already there**, both routers on one board model.

1. **A shared crate of clean-ups on finished copper**, ported from `cleanup.rs` and
   `vias.rs` onto our `board`, run after `wire` has drawn. It answers the owner's three notes
   on p2 directly, each change checked and undone if anything comes apart.
2. **The finisher, built on our planner**, with the old router's discipline: find the pinch
   on the way the stuck connection would take (not only at its ends), make room there by
   nudging first and by lifting only then, keep only if fewer are open, widen, allow bounded
   ties, give it its own budget. The owner's preference is the nudge: in our plan a track is a
   rubber band, so a via moved a little takes its tracks with it and they are drawn square
   again after, which the old relaxer cannot do. Vias must first be able to stand off the
   0.43 mm grid of spots.
3. **The old router as a closer beside ours**, by its binary first (it adopts a routed board
   as it is), ported into the workspace only if that proves worth it. An experiment on our own
   4-open p2 board was queued on 2026-11-25; its result is in the lab at
   `ours/p2-132-usb6/owner-2026-11-24/old-finisher/`.

What stays out: the GPL crates, behind a process boundary as now; and the relaxer run as it is
over our copper.

## 6. What this changes in our rules

`governance/ENGINEERING_CONSTITUTION.md` R5 says the old router is never brought in or built
here, and `docs/one-system-spec.md` lists `grid`, `ultraroute`, `morph` and `topo` as not
coming. The owner's words above change that. The amendment is proposed in its own commit, as
section 11 asks.
