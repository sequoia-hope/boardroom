//! Finding a way for one connection through the room left on every layer at once: along a layer
//! from gap to gap, and from layer to layer by a via at one of the spots where a via could
//! stand. No spot is in the triangulation. A via may be put where the way has come to the part
//! of a triangle its spot is in, as the strands there lie, and goes on from that spot's part of
//! its triangle on another layer. A spot that lies just past one strand from there will do
//! too: the strand is then led round the far side of the new via. No way turns more than
//! half-way round one corner.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use board::Board;

use crate::space::{dist, End, Map, P};
use crate::strands::{Region, Sheet};

/// Where a leg ends: on something that is there, or at a via the way puts in at a spot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stop {
    End(End),
    Spot(usize),
}

/// One layer's piece of a way: the edges crossed and the slot taken in each, between two ends.
/// A leg from one new via to another in the same part of a triangle crosses nothing.
#[derive(Debug, Clone)]
pub struct Leg {
    pub layer: usize,
    pub from: Stop,
    pub to: Stop,
    pub steps: Vec<(usize, usize)>,
}

#[derive(Debug, Clone)]
pub struct Way {
    pub legs: Vec<Leg>,
    pub cost: f64,
    /// The connections whose strands the way goes across, where it was let: they must be
    /// lifted before a way like it can be laid.
    pub over: Vec<usize>,
    /// Strands a via of the way stands just past, to be led round it: the spot, the layer,
    /// the strand, and the crossing its line past the spot starts from.
    pub pushes: Vec<(usize, usize, usize, usize)>,
}

/// A place in the search. A crossing: the layer, the edge, the slot, the triangle entered. A
/// via about to be left: the layer it is left on, [`VIA`], the spot, nothing.
type Key = (usize, usize, usize, usize);

/// What a bend in a way costs, as length, mm. A way that winds between pads is hard to draw
/// and is not what anyone would lay: with this it is weighed against a via, or against
/// lifting what is in the way, as more than its length.
const TURN: f64 = 0.75;
/// What starting or ending at a hole in a pad ([`crate::Spots::holes`]) costs, as length, mm:
/// three vias. A hole is where solder goes, and a designer drills one only where the pad has no
/// other way; with this a via beside the pad is taken wherever one will do.
pub const HOLE: f64 = 24.0;
/// In a key's last place: the crossing is the last, onto the end.
const GOAL: usize = usize::MAX;
/// In a key's second place: the key is a via.
const VIA: usize = usize::MAX - 1;

struct Seen {
    cost: f64,
    prev: Option<Key>,
    at: P,
    /// The last bend: its place and the cost there, and the crossing it is (none when it is the
    /// end the leg started from).
    bend_at: P,
    bend_cost: f64,
    bend: Option<Key>,
    /// Whether this is the first crossing of a leg.
    starts: bool,
    /// The connection whose strand was stepped over to come here, in the gap just crossed.
    over: Option<usize>,
    /// What the way is turning about, and how far it has turned about it, radians.
    about: Option<(usize, f64)>,
    /// For a via: the strand it stands just past on the layer it was come to on, and the
    /// crossing that strand's line past it starts from.
    push: Option<(usize, usize, usize)>,
    /// The end the way left from, on its first crossing, or came to, on its last.
    end: Option<End>,
    /// The connections whose strands fill the gap just crossed, or the corner just turned,
    /// so that this one fits there only with them out of it.
    shoved: Vec<usize>,
}

#[derive(PartialEq)]
struct Open {
    /// The cost so far and the least it can still cost: the straight distance to the goal.
    order: f64,
    cost: f64,
    key: Key,
}
impl Eq for Open {}
impl Ord for Open {
    fn cmp(&self, o: &Self) -> Ordering {
        o.order.total_cmp(&self.order).then(o.key.cmp(&self.key))
    }
}
impl PartialOrd for Open {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

/// What the search is told about the board's layers and vias.
#[derive(Clone, Copy)]
pub struct Terms<'a> {
    /// What a strand's length costs on each layer, as a multiple.
    pub dear: &'a [f64],
    /// What a via costs, as length, mm; `None` for no vias.
    pub via: Option<f64>,
    /// What going across another net's strand costs, as length, mm; `None` for never. A way
    /// that does cannot be laid: it tells which connections are in the way, and what it would
    /// be worth to have them out of it.
    pub cross: Option<f64>,
    /// Spots no via is to be put at.
    pub ban: &'a [usize],
    /// Connections whose strands the way may not go across, whatever `cross` says.
    pub keep: &'a [usize],
    /// Spots that stand in the net's pour, in a piece of it the way's far end is joined to: a
    /// via at one ends the way, and costs `drop`. Only where vias are let.
    pub into: &'a [usize],
    pub drop: f64,
    /// Gaps the way may not cross: those a wire for this connection could not be drawn
    /// through.
    pub avoid: &'a [crate::space::GapKey],
}

/// Where a via of `net` at spot `c` would stand on each layer, the triangle and the part of
/// it: none if on some layer it has no triangle, is too near a strand to be on one side of it,
/// or leaves the strands round it too little room; or if the exact checker faults it on
/// `with`, the board with the plan's vias on it. A spot was kept for a via of no net alone on
/// the bare board; a via with a net may not stand there beside the plan's others, which can
/// together cut more of a pad's spokes to its pour than may be cut.
fn standing(board: &Board, sheets: &[Sheet], with: &mut Option<Board>, c: usize, net: u32) -> Option<Vec<(usize, Region)>> {
    // What is known for good comes first: it costs nothing to ask again.
    let beside = alone(board, sheets, c, net)?;
    let places: Vec<(usize, Region)> = sheets.iter().map(|s| standing_on(board, s, c, net)).collect::<Option<_>>()?;
    (!beside || together(board, sheets, with, c, net)).then_some(places)
}

/// Whether a via of `net` may stand at spot `c` for all that is there before the plan: it
/// shuts no other net's pin in, and the exact checker passes it alone on the bare board. None
/// if not; else whether the plan's other vias have a say in it ([`Spots::alone`], or one of
/// them stands near).
fn alone(board: &Board, sheets: &[Sheet], c: usize, net: u32) -> Option<bool> {
    let spots = &sheets.first()?.space.spots;
    if spots.shuts(spots.at[c], net) {
        return None;
    }
    let space = &sheets.first()?.space;
    match spots.alone(board, c, net, &|| crate::via_item(board, spots.at[c], net)) {
        (false, _) => None,
        // And a via of the plan that stands near has a say.
        (true, beside) => Some(beside || space.sites.iter().any(|s| dist(space.obstacles[s.obstacle].centre, spots.at[c]) < spots.beside)),
    }
}

/// Whether the exact checker lets a via of `net` stand at spot `c` beside the plan's others.
fn together(board: &Board, sheets: &[Sheet], with: &mut Option<Board>, c: usize, net: u32) -> bool {
    let Some(first) = sheets.first() else { return false };
    let with = with.get_or_insert_with(|| {
        let mut with = board.clone();
        for (at, of) in first.space.vias() {
            with.insert(crate::via_item(board, at, of));
        }
        with
    });
    let id = with.insert(crate::via_item(board, first.space.spots.at[c], net));
    let fits = board::check_item(with, id).is_empty();
    with.remove(id);
    fits
}

/// Where a via of `net` at spot `c` would stand on one layer, as the strands lie: the triangle
/// and the part of it.
fn standing_on(board: &Board, s: &Sheet, c: usize, net: u32) -> Option<(usize, Region)> {
    s.stood(board, c, net)
}

/// Whether a via of `net` at spot `c` may stand in `region` of triangle `t` on layer `l`
/// though the spot lies just past one strand from there: the strand, and the crossing its
/// line past the spot starts from. A via is new: which side of it a strand that is already
/// there passes is not yet said, and the strand can be led round the far side of it. So a pin
/// that a strand passes in front of can still drop a via in front of itself.
#[allow(clippy::too_many_arguments)]
fn standing_past(board: &Board, sheets: &[Sheet], with: &mut Option<Board>, c: usize, net: u32, l: usize, t: usize, region: Region) -> Option<(usize, usize)> {
    let sheet = &sheets[l];
    if sheet.space.spot_tri[c] != Some(t) {
        return None;
    }
    let p = sheet.space.spots.at[c];
    let (strand, k) = sheet.chord_between(t, region, sheet.region_at(board, t, p)?)?;
    if sheet.strands[strand].net == net || !sheet.via_fits(board, t, region, p, net) {
        return None;
    }
    let beside = alone(board, sheets, c, net)?;
    (sheets.iter().enumerate().all(|(m, s)| m == l || standing_on(board, s, c, net).is_some()) && (!beside || together(board, sheets, with, c, net))).then_some((strand, k))
}

/// The cheapest way for a strand of `net` from `from` to `to`. A way's cost is its length pulled
/// tight: straight from its last bend for as long as the straight line stays inside every gap
/// crossed since.
pub fn find(board: &Board, sheets: &[Sheet], terms: &Terms, net: u32, width: f64, from: &[End], to: &[End]) -> Option<Way> {
    // A net a custom rule allows no vias (`via_count (max 0)`: a crystal's, a USB pair's) takes
    // none, nor a via down into its pour.
    let flat;
    let terms = if board.no_vias(net) && terms.via.is_some() {
        flat = Terms { via: None, into: &[], ..*terms };
        &flat
    } else {
        terms
    };
    let me = (net, width);
    let middle = |e: End| sheets.iter().find_map(|s| s.space.ends.get(&e).map(|&o| s.space.obstacles[o].centre));
    let goals: Vec<(End, P)> = to.iter().map(|&e| Some((e, middle(e)?))).collect::<Option<_>>()?;
    // What the way heads for: the first of its ends, or, with none (a way that ends at a via
    // it puts in), where it starts.
    let goal = goals.first().map(|g| g.1).or_else(|| from.iter().find_map(|&e| middle(e)))?;
    // The least there is still to go from a place: to the nearest of its ends.
    let left = |p: P| goals.iter().map(|g| dist(p, g.1)).fold(f64::MAX, f64::min).min(if goals.is_empty() { 0.0 } else { f64::MAX });
    let spots = sheets.first()?.space.spots.clone();
    let holed = |e: End| if matches!(e, End::Site(k) if k < spots.holes.len()) { HOLE } else { 0.0 };
    let mut best: Map<Key, Seen> = Map::default();
    let mut heap = BinaryHeap::new();
    let mut stands: Map<usize, Option<Vec<(usize, Region)>>> = Map::default();
    // For a spot, a layer, a triangle and a part of it: the strand a via there would push.
    type Past = Map<(usize, usize, usize, Region), Option<(usize, usize)>>;
    let mut pasts: Past = Map::default();
    // The board with the plan's vias on it, made when the first spot is asked about.
    let mut with: Option<Board> = None;
    // For a layer and a spot: the part of its triangle the spot is in, as the strands lie.
    let mut regions: Map<(usize, usize), Option<Region>> = Map::default();
    // Whether spot `c`, which is in triangle `t` on layer `l`, is in `region` of it, and is
    // not barred: where a via would stand if one may.
    let sits = |sheet: &Sheet, regions: &mut Map<(usize, usize), Option<Region>>, l: usize, t: usize, region: Region, c: usize| {
        !terms.ban.contains(&c) && *regions.entry((l, c)).or_insert_with(|| sheet.region_at(board, t, spots.at[c])) == Some(region)
    };
    // With a pour to end in, the far end's middle is no bound on what is left to go.
    let barred = |sheet: &Sheet, e: usize| {
        !terms.avoid.is_empty() && {
            let [a, b] = sheet.space.edges[e].v.map(|v| sheet.space.verts[v]);
            terms.avoid.contains(&crate::space::gap_key(a, b))
        }
    };
    let cheapest = if terms.into.is_empty() || terms.via.is_none() { terms.dear.iter().copied().fold(f64::MAX, f64::min) } else { 0.0 };
    // The first crossings of the leg that leaves `from`, on each layer it is on.
    for (l, sheet, src) in sheets.iter().enumerate().flat_map(|(l, sheet)| from.iter().map(move |&src| (l, sheet, src))) {
        // A closed layer (a plane) is never laid on.
        if !terms.dear[l].is_finite() {
            continue;
        }
        let Some(&o) = sheet.space.ends.get(&src) else { continue };
        let start = sheet.space.obstacles[o].centre;
        for (e, edge) in sheet.space.edges.iter().enumerate() {
            let (Some(x), Some(y)) = (edge.t[0], edge.t[1]) else { continue };
            for (inner, outer) in [(x, y), (y, x)] {
                if sheet.space.tris[inner].inside == Some(o) && sheet.space.free(outer) && !sheet.space.barred(outer, net) {
                    for slot in 0..=sheet.on[e].len() {
                        let at = sheet.crossing(board, e, slot, me, start, goal);
                        let c = holed(src) + terms.dear[l] * dist(start, at);
                        let key = (l, e, slot, outer);
                        if best.get(&key).is_none_or(|b| c < b.cost) {
                            best.insert(key, Seen { cost: c, prev: None, at, bend_at: start, bend_cost: 0.0, bend: None, starts: true, over: None, about: None, push: None, end: Some(src), shoved: Vec::new() });
                            heap.push(Open { order: c + cheapest * left(at), cost: c, key });
                        }
                    }
                }
            }
        }
    }
    // A via at spot `c`, come to on layer `l` at `cost`: it may be left on any other layer.
    // Whether a via may stand there at all (`sure`) is asked when the search comes to leave
    // by it, not here: most of the spots a search passes it never comes back to, and the
    // answer is the dear part of a search. But where a via that pushes a strand round itself
    // and one that does not come to the same spot, which of the two the spot is given to
    // turns on whether the one that does not may stand, and that is asked at once.
    let arrive = |c: usize, l: usize, cost: f64, prev: Key, via: f64, push: Option<(usize, usize, usize)>, best: &mut Map<Key, Seen>, heap: &mut BinaryHeap<Open>, sure: &mut dyn FnMut(usize) -> bool| {
        let p = spots.at[c];
        if terms.into.contains(&c) {
            let (key, cost) = ((l, VIA, c, GOAL), cost + terms.drop);
            if best.get(&key).is_none_or(|b| cost < b.cost) {
                best.insert(key, Seen { cost, prev: Some(prev), at: p, bend_at: p, bend_cost: cost, bend: None, starts: false, over: None, about: None, push, end: None, shoved: Vec::new() });
                heap.push(Open { order: cost, cost, key });
            }
        }
        for m in (0..sheets.len()).filter(|&m| m != l) {
            let (key, cost) = ((m, VIA, c, 0), cost + via + spots.dearer(board, c, net));
            let take = match best.get(&key) {
                None => true,
                Some(there) => {
                    let cheaper = cost < there.cost && !through(best, prev, key);
                    match (push.is_some(), there.push.is_some()) {
                        // Alike: the cheaper.
                        (false, false) | (true, true) => cheaper,
                        // This pushes and the one there does not: the cheaper, or this one
                        // if the one there may not stand, and so is not there.
                        (true, false) => cheaper || !sure(c),
                        // This does not push and the one there does: the cheaper, if it
                        // may stand.
                        (false, true) => cheaper && sure(c),
                    }
                }
            };
            if take {
                best.insert(key, Seen { cost, prev: Some(prev), at: p, bend_at: p, bend_cost: cost, bend: None, starts: false, over: None, about: None, push, end: None, shoved: Vec::new() });
                heap.push(Open { order: cost + cheapest * left(p), cost, key });
            }
        }
    };
    while let Some(Open { cost, key, .. }) = heap.pop() {
        // (A place given since to another way: to a cheaper one, or, a via's, to one that
        // may stand where the cheaper may not.)
        if best.get(&key).is_some_and(|b| b.cost != cost) {
            continue;
        }
        let (l, edge, slot, into) = key;
        if into == GOAL {
            let (legs, over, pushes) = legs(&best, key)?;
            return Some(Way { legs, cost, over, pushes });
        }
        let sheet = &sheets[l];
        let mut found: Vec<(Key, Seen)> = Vec::new();
        if edge == VIA {
            // Out of the via at this spot, on this layer: by a side of the part of its
            // triangle it stands in, or straight to another spot in that part.
            let Some(via) = terms.via else { continue };
            if !terms.dear[l].is_finite() {
                continue;
            }
            let p = spots.at[slot];
            // Whether a via may stand here at all, asked now that the search has come to
            // leave by it (one that pushes a strand was asked as it was come to).
            if best[&key].push.is_none() && stands.entry(slot).or_insert_with(|| if terms.ban.contains(&slot) { None } else { standing(board, sheets, &mut with, slot, net) }).is_none() {
                continue;
            }
            let Some((t, region)) = standing_on(board, sheet, slot, net) else { continue };
            // A rule area that bars the net's tracks here: a via may stand in it, no track leave it.
            if sheet.space.barred(t, net) {
                continue;
            }
            for (e, s) in sheet.doors(t, region) {
                let Some(next) = sheet.space.edges[e].t.iter().flatten().copied().find(|&x| x != t) else { continue };
                let end = goals.iter().find(|g| sheet.space.ends.get(&g.0).is_some_and(|&o| sheet.space.tris[next].inside == Some(o))).copied();
                let ends = end.is_some();
                if !ends && (!sheet.space.free(next) || sheet.space.barred(next, net) || barred(sheet, e) || !sheet.fits(board, e, Some((s, net, width)))) {
                    continue;
                }
                let there = sheet.crossing(board, e, s, me, p, goal);
                let c = cost + terms.dear[l] * dist(p, there);
                let (to_key, c) = match end { Some(g) => ((l, e, s, GOAL), c + terms.dear[l] * dist(there, g.1) + holed(g.0)), None => ((l, e, s, next), c) };
                if best.get(&to_key).is_none_or(|b| c < b.cost && !through(&best, key, to_key)) {
                    found.push((to_key, Seen { cost: c, prev: Some(key), at: there, bend_at: p, bend_cost: cost, bend: None, starts: true, over: None, about: None, push: None, end: end.map(|g| g.0), shoved: Vec::new() }));
                }
            }
            for &c in &sheet.space.tri_spots[t] {
                if c != slot && sits(sheet, &mut regions, l, t, region, c) && (!terms.into.contains(&c) || stands.entry(c).or_insert_with(|| standing(board, sheets, &mut with, c, net)).is_some()) {
                    arrive(c, l, cost + terms.dear[l] * dist(p, spots.at[c]), key, via, None, &mut best, &mut heap, &mut |c| stands.entry(c).or_insert_with(|| if terms.ban.contains(&c) { None } else { standing(board, sheets, &mut with, c, net) }).is_some());
                }
            }
        } else {
            let seen = &best[&key];
            let (here, bend, bend_at, bend_cost) = (seen.at, seen.bend, seen.bend_at, seen.bend_cost);
            // A via at a spot in the part of this triangle the way has come into.
            if let Some(via) = terms.via {
                let region = sheet.region(into, edge, slot);
                for &c in &sheet.space.tri_spots[into] {
                    if sits(sheet, &mut regions, l, into, region, c) {
                        // A via that ends the way where it stands is asked about at once.
                        if !terms.into.contains(&c) || stands.entry(c).or_insert_with(|| standing(board, sheets, &mut with, c, net)).is_some() {
                            arrive(c, l, cost + terms.dear[l] * dist(here, spots.at[c]), key, via, None, &mut best, &mut heap, &mut |c| stands.entry(c).or_insert_with(|| if terms.ban.contains(&c) { None } else { standing(board, sheets, &mut with, c, net) }).is_some());
                        }
                    } else if !terms.ban.contains(&c)
                        && let Some((strand, k)) = *pasts.entry((c, l, into, region)).or_insert_with(|| standing_past(board, sheets, &mut with, c, net, l, into, region))
                    {
                        arrive(c, l, cost + terms.dear[l] * dist(here, spots.at[c]), key, via, Some((l, strand, k)), &mut best, &mut heap, &mut |c| stands.entry(c).or_insert_with(|| if terms.ban.contains(&c) { None } else { standing(board, sheets, &mut with, c, net) }).is_some());
                    }
                }
            }
            // Over the strand on either side of it in the gap it has just crossed.
            if let Some(cross) = terms.cross {
                for (slot2, k) in [(slot + 1, Some(slot)), (slot.wrapping_sub(1), slot.checked_sub(1))] {
                    let Some(&(other, _)) = k.and_then(|k| sheet.on[edge].get(k)) else { continue };
                    let other = &sheet.strands[other];
                    let (to_key, c) = ((l, edge, slot2, into), cost + cross);
                    if other.net != net && !terms.keep.contains(&other.connection) && best.get(&to_key).is_none_or(|b| c < b.cost && !through(&best, key, to_key)) {
                        found.push((to_key, Seen { cost: c, prev: Some(key), at: here, bend_at: here, bend_cost: c, bend: Some(key), starts: false, over: Some(other.connection), about: best[&key].about, push: None, end: None, shoved: Vec::new() }));
                    }
                }
            }
            // Whether a point can be seen from the last bend through every crossing since.
            let seen_from_bend = |p: P| {
                let mut at = key;
                loop {
                    if Some(at) == bend {
                        return true;
                    }
                    if !sheet.sees(board, at.1, at.2, me, bend_at, p) {
                        return false;
                    }
                    match (best[&at].starts, best[&at].prev) {
                        (false, Some(prev)) => at = prev,
                        _ => return true,
                    }
                }
            };
            for (e, s, corner) in sheet.ways_on(into, edge, slot) {
                let Some(next) = sheet.space.edges[e].t.iter().flatten().copied().find(|&x| x != into) else { continue };
                let end = goals.iter().find(|g| sheet.space.ends.get(&g.0).is_some_and(|&o| sheet.space.tris[next].inside == Some(o))).copied();
                let ends = end.is_some();
                if !ends && (!sheet.space.free(next) || sheet.space.barred(next, net) || barred(sheet, e)) {
                    continue;
                }
                // With others' strands in the way of it at a corner or in a gap that would
                // hold it alone: across them all, where going across is let.
                let mut shoved: Vec<usize> = Vec::new();
                if !sheet.throat(board, into, corner, net, width) {
                    match terms.cross.and_then(|_| sheet.throat_alone(board, into, corner, net, width, terms.keep)) {
                        Some(out) => shoved.extend(out),
                        None => continue,
                    }
                }
                // No way turns more than half-way round one corner (or one round thing): that
                // is a way back to where it came from, and no wire can be drawn along it.
                let about = {
                    let v = sheet.space.tris[into].v[corner];
                    let what = match sheet.space.vert_of[v] {
                        Some(o) if sheet.space.obstacles[o].reach > 0.0 => usize::MAX - o,
                        _ => v,
                    };
                    let at = sheet.space.verts[v];
                    let far = |x: usize| sheet.space.edges[x].v.iter().map(|&w| sheet.space.verts[w]).find(|&w| dist(w, at) > 1e-9).unwrap_or(at);
                    let (a, b) = (far(edge), far(e));
                    let (a, b) = ([a[0] - at[0], a[1] - at[1]], [b[0] - at[0], b[1] - at[1]]);
                    let turn = (a[0] * b[1] - a[1] * b[0]).atan2(a[0] * b[0] + a[1] * b[1]).abs();
                    (what, best[&key].about.filter(|x| x.0 == what).map_or(0.0, |x| x.1) + turn)
                };
                if about.1 > std::f64::consts::PI + 1e-6 {
                    continue;
                }
                if !ends && !sheet.fits(board, e, Some((s, net, width))) {
                    match terms.cross.and_then(|_| sheet.fits_alone(board, e, net, width, terms.keep)) {
                        Some(out) => shoved.extend(out.into_iter().filter(|c| !shoved.contains(c)).collect::<Vec<_>>()),
                        None => continue,
                    }
                }
                // Each of them costs a crossing, once for as long as the way runs on beside it.
                let dearer = terms.cross.unwrap_or(0.0) * shoved.iter().filter(|c| !best[&key].shoved.contains(c)).count() as f64;
                // Straight from the last bend if that can be; else this crossing is a bend.
                let straight = sheet.crossing(board, e, s, me, bend_at, goal);
                let (there, c, bend, bend_at, bend_cost) = if seen_from_bend(straight) {
                    (straight, bend_cost + terms.dear[l] * dist(bend_at, straight), bend, bend_at, bend_cost)
                } else {
                    let there = sheet.crossing(board, e, s, me, here, goal);
                    (there, cost + TURN + terms.dear[l] * dist(here, there), Some(key), here, cost + TURN)
                };
                // The end's own side is the last crossing; then the way ends at its middle.
                let (c, bend_cost) = (c + dearer, bend_cost + dearer);
                let (to_key, c) = match end { Some(g) => ((l, e, s, GOAL), c + terms.dear[l] * dist(there, g.1) + holed(g.0)), None => ((l, e, s, next), c) };
                if best.get(&to_key).is_none_or(|b| c < b.cost && !through(&best, key, to_key)) {
                    found.push((to_key, Seen { cost: c, prev: Some(key), at: there, bend_at, bend_cost, bend, starts: false, over: None, about: Some(about), push: None, end: end.map(|g| g.0), shoved }));
                }
            }
        }
        for (k, seen) in found {
            // A way that has reached the goal has nothing left to go.
            let to_go = if k.3 == GOAL { 0.0 } else { cheapest * left(seen.at) };
            heap.push(Open { order: seen.cost + to_go, cost: seen.cost, key: k });
            best.insert(k, seen);
        }
    }
    None
}

/// Whether the way that ends at `from` has made the crossing `key`. A way is told by following
/// `prev` from its last crossing, so a crossing must never be given a way that already goes
/// through it: that would be a loop with no start. It can be offered one, because a way's cost
/// is its length pulled tight, which can be less at a crossing than at the one before it, and
/// so a way that goes on round a corner and back can come out cheaper than the way it began on.
fn through(best: &Map<Key, Seen>, from: Key, key: Key) -> bool {
    let mut at = Some(from);
    while let Some(k) = at {
        if k == key {
            return true;
        }
        at = best[&k].prev;
    }
    false
}

/// The way that ends at `last`, as legs, a new leg at every via, and the connections it goes
/// across.
#[allow(clippy::type_complexity)]
fn legs(best: &Map<Key, Seen>, last: Key) -> Option<(Vec<Leg>, Vec<usize>, Vec<(usize, usize, usize, usize)>)> {
    // Each leg's start is filled in when the way has been followed back to it.
    let from = End::Site(usize::MAX);
    let mut legs: Vec<Leg> = Vec::new();
    let mut over: Vec<usize> = Vec::new();
    let mut pushes: Vec<(usize, usize, usize, usize)> = Vec::new();
    // A way that ends at a via in the pour: its last leg comes to that via.
    let dropped = last.1 == VIA && last.3 == GOAL;
    let mut leg = Leg { layer: last.0, from: Stop::End(from), to: if dropped { Stop::Spot(last.2) } else { Stop::End(best[&last].end?) }, steps: Vec::new() };
    let mut first = None;
    if dropped && let Some((l, s, k)) = best[&last].push {
        pushes.push((last.2, l, s, k));
    }
    let mut at = if dropped { best[&last].prev } else { Some(last) };
    let mut steps = 0;
    while let Some(k) = at {
        steps += 1;
        assert!(steps <= best.len(), "a way goes through one crossing twice");
        let prev = best[&k].prev;
        if prev.is_none() {
            first = best[&k].end;
        }
        if k.1 == VIA {
            if let Some((l, s, c)) = best[&k].push
                && !pushes.contains(&(k.2, l, s, c))
            {
                pushes.push((k.2, l, s, c));
            }
            // The leg that leaves this via is whole; the one before it ends here, on the
            // layer the via was come to on.
            leg.from = Stop::Spot(k.2);
            leg.steps.reverse();
            let layer = prev.map_or(k.0, |p| p.0);
            legs.push(std::mem::replace(&mut leg, Leg { layer, from: Stop::End(from), to: Stop::Spot(k.2), steps: Vec::new() }));
        } else if let Some(c) = best[&k].over {
            // A step over a strand is no crossing of a gap.
            if !over.contains(&c) {
                over.push(c);
            }
        } else {
            for &c in &best[&k].shoved {
                if !over.contains(&c) {
                    over.push(c);
                }
            }
            leg.steps.push((k.1, k.2));
        }
        at = prev;
    }
    leg.steps.reverse();
    leg.from = Stop::End(first?);
    legs.push(leg);
    legs.reverse();
    Some((legs, over, pushes))
}
