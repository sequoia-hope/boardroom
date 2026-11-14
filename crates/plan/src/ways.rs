//! Finding a way for one connection through the room left on every layer at once: along a layer
//! from gap to gap, and from layer to layer by a via at one of the spots where a via could
//! stand. No spot is in the triangulation. A via may be put where the way has come to the part
//! of a triangle its spot is in, as the strands there lie, and goes on from that spot's part of
//! its triangle on another layer.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use board::Board;

use crate::space::{dist, End, P};
use crate::strands::{Region, Sheet, EPS};

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
}

/// A place in the search. A crossing: the layer, the edge, the slot, the triangle entered. A
/// via about to be left: the layer it is left on, [`VIA`], the spot, nothing.
type Key = (usize, usize, usize, usize);

/// What a bend in a way costs, as length, mm. A way that winds between pads is hard to draw
/// and is not what anyone would lay: with this it is weighed against a via, or against
/// lifting what is in the way, as more than its length.
const TURN: f64 = 0.75;
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
    let places: Vec<(usize, Region)> = standing_on(board, sheets, c, net)?;
    let first = sheets.first()?;
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
    fits.then_some(places)
}

fn standing_on(board: &Board, sheets: &[Sheet], c: usize, net: u32) -> Option<Vec<(usize, Region)>> {
    sheets
        .iter()
        .map(|s| {
            let t = s.space.spot_tri[c]?;
            let p = s.space.spots.at[c];
            let region = s.region_at(board, t, p)?;
            s.via_fits(board, t, region, p, net).then_some((t, region))
        })
        .collect()
}

/// The cheapest way for a strand of `net` from `from` to `to`. A way's cost is its length pulled
/// tight: straight from its last bend for as long as the straight line stays inside every gap
/// crossed since.
pub fn find(board: &Board, sheets: &[Sheet], terms: &Terms, net: u32, width: f64, from: End, to: End) -> Option<Way> {
    let me = (net, width);
    let middle = |e: End| sheets.iter().find_map(|s| s.space.ends.get(&e).map(|&o| s.space.obstacles[o].centre));
    let goal = middle(to)?;
    let spots = sheets.first()?.space.spots.clone();
    let mut best: HashMap<Key, Seen> = HashMap::new();
    let mut heap = BinaryHeap::new();
    let mut stands: HashMap<usize, Option<Vec<(usize, Region)>>> = HashMap::new();
    // The board with the plan's vias on it, made when the first spot is asked about.
    let mut with: Option<Board> = None;
    // With a pour to end in, the far end's middle is no bound on what is left to go.
    let barred = |sheet: &Sheet, e: usize| {
        !terms.avoid.is_empty() && {
            let [a, b] = sheet.space.edges[e].v.map(|v| sheet.space.verts[v]);
            terms.avoid.contains(&crate::space::gap_key(a, b))
        }
    };
    let cheapest = if terms.into.is_empty() || terms.via.is_none() { terms.dear.iter().copied().fold(f64::MAX, f64::min) } else { 0.0 };
    // The first crossings of the leg that leaves `from`, on each layer it is on.
    for (l, sheet) in sheets.iter().enumerate() {
        let Some(&o) = sheet.space.ends.get(&from) else { continue };
        let start = sheet.space.obstacles[o].centre;
        for (e, edge) in sheet.space.edges.iter().enumerate() {
            let (Some(x), Some(y)) = (edge.t[0], edge.t[1]) else { continue };
            for (inner, outer) in [(x, y), (y, x)] {
                if sheet.space.tris[inner].inside == Some(o) && sheet.space.free(outer) {
                    for slot in 0..=sheet.on[e].len() {
                        let at = sheet.crossing(board, e, slot, me, start, goal);
                        let c = terms.dear[l] * dist(start, at);
                        let key = (l, e, slot, outer);
                        if best.get(&key).is_none_or(|b| c < b.cost) {
                            best.insert(key, Seen { cost: c, prev: None, at, bend_at: start, bend_cost: 0.0, bend: None, starts: true, over: None });
                            heap.push(Open { order: c + cheapest * dist(at, goal), cost: c, key });
                        }
                    }
                }
            }
        }
    }
    // A via at spot `c`, come to on layer `l` at `cost`: it may be left on any other layer.
    let arrive = |c: usize, l: usize, cost: f64, prev: Key, via: f64, best: &mut HashMap<Key, Seen>, heap: &mut BinaryHeap<Open>| {
        let p = spots.at[c];
        if terms.into.contains(&c) {
            let (key, cost) = ((l, VIA, c, GOAL), cost + terms.drop);
            if best.get(&key).is_none_or(|b| cost < b.cost) {
                best.insert(key, Seen { cost, prev: Some(prev), at: p, bend_at: p, bend_cost: cost, bend: None, starts: false, over: None });
                heap.push(Open { order: cost, cost, key });
            }
        }
        for m in (0..sheets.len()).filter(|&m| m != l) {
            let (key, cost) = ((m, VIA, c, 0), cost + via);
            if best.get(&key).is_none_or(|b| cost < b.cost && !through(best, prev, key)) {
                best.insert(key, Seen { cost, prev: Some(prev), at: p, bend_at: p, bend_cost: cost, bend: None, starts: false, over: None });
                heap.push(Open { order: cost + cheapest * dist(p, goal), cost, key });
            }
        }
    };
    while let Some(Open { cost, key, .. }) = heap.pop() {
        if best.get(&key).is_some_and(|b| b.cost < cost) {
            continue;
        }
        let (l, edge, slot, into) = key;
        if into == GOAL {
            let (legs, over) = legs(&best, key, from, to);
            return Some(Way { legs, cost, over });
        }
        let sheet = &sheets[l];
        let mut found: Vec<(Key, Seen)> = Vec::new();
        if edge == VIA {
            // Out of the via at this spot, on this layer: by a side of the part of its
            // triangle it stands in, or straight to another spot in that part.
            let Some(via) = terms.via else { continue };
            let p = spots.at[slot];
            let Some((t, region)) = stands.entry(slot).or_insert_with(|| if terms.ban.contains(&slot) { None } else { standing(board, sheets, &mut with, slot, net) }).as_ref().map(|s| s[l]) else { continue };
            for (e, s) in sheet.doors(t, region) {
                let Some(next) = sheet.space.edges[e].t.iter().flatten().copied().find(|&x| x != t) else { continue };
                let ends = sheet.space.ends.get(&to).is_some_and(|&o| sheet.space.tris[next].inside == Some(o));
                if !ends && (!sheet.space.free(next) || barred(sheet, e) || sheet.need(board, e, Some((s, net, width))) > sheet.space.width(e) + EPS) {
                    continue;
                }
                let there = sheet.crossing(board, e, s, me, p, goal);
                let c = cost + terms.dear[l] * dist(p, there);
                let (to_key, c) = if ends { ((l, e, s, GOAL), c + terms.dear[l] * dist(there, goal)) } else { ((l, e, s, next), c) };
                if best.get(&to_key).is_none_or(|b| c < b.cost && !through(&best, key, to_key)) {
                    found.push((to_key, Seen { cost: c, prev: Some(key), at: there, bend_at: p, bend_cost: cost, bend: None, starts: true, over: None }));
                }
            }
            for &c in &sheet.space.tri_spots[t] {
                if c != slot && stands.entry(c).or_insert_with(|| if terms.ban.contains(&c) { None } else { standing(board, sheets, &mut with, c, net) }).as_ref().is_some_and(|s| s[l] == (t, region)) {
                    arrive(c, l, cost + terms.dear[l] * dist(p, spots.at[c]), key, via, &mut best, &mut heap);
                }
            }
        } else {
            let seen = &best[&key];
            let (here, bend, bend_at, bend_cost) = (seen.at, seen.bend, seen.bend_at, seen.bend_cost);
            // A via at a spot in the part of this triangle the way has come into.
            if let Some(via) = terms.via {
                let region = sheet.region(into, edge, slot);
                for &c in &sheet.space.tri_spots[into] {
                    if stands.entry(c).or_insert_with(|| if terms.ban.contains(&c) { None } else { standing(board, sheets, &mut with, c, net) }).as_ref().is_some_and(|s| s[l] == (into, region)) {
                        arrive(c, l, cost + terms.dear[l] * dist(here, spots.at[c]), key, via, &mut best, &mut heap);
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
                        found.push((to_key, Seen { cost: c, prev: Some(key), at: here, bend_at: here, bend_cost: c, bend: Some(key), starts: false, over: Some(other.connection) }));
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
                let ends = sheet.space.ends.get(&to).is_some_and(|&o| sheet.space.tris[next].inside == Some(o));
                if !ends && (!sheet.space.free(next) || barred(sheet, e)) {
                    continue;
                }
                if !sheet.throat(board, into, corner, net, width) {
                    continue;
                }
                if !ends && sheet.need(board, e, Some((s, net, width))) > sheet.space.width(e) + EPS {
                    continue;
                }
                // Straight from the last bend if that can be; else this crossing is a bend.
                let straight = sheet.crossing(board, e, s, me, bend_at, goal);
                let (there, c, bend, bend_at, bend_cost) = if seen_from_bend(straight) {
                    (straight, bend_cost + terms.dear[l] * dist(bend_at, straight), bend, bend_at, bend_cost)
                } else {
                    let there = sheet.crossing(board, e, s, me, here, goal);
                    (there, cost + TURN + terms.dear[l] * dist(here, there), Some(key), here, cost + TURN)
                };
                // The end's own side is the last crossing; then the way ends at its middle.
                let (to_key, c) = if ends { ((l, e, s, GOAL), c + terms.dear[l] * dist(there, goal)) } else { ((l, e, s, next), c) };
                if best.get(&to_key).is_none_or(|b| c < b.cost && !through(&best, key, to_key)) {
                    found.push((to_key, Seen { cost: c, prev: Some(key), at: there, bend_at, bend_cost, bend, starts: false, over: None }));
                }
            }
        }
        for (k, seen) in found {
            // A way that has reached the goal has nothing left to go.
            let to_go = if k.3 == GOAL { 0.0 } else { cheapest * dist(seen.at, goal) };
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
fn through(best: &HashMap<Key, Seen>, from: Key, key: Key) -> bool {
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
fn legs(best: &HashMap<Key, Seen>, last: Key, from: End, to: End) -> (Vec<Leg>, Vec<usize>) {
    let mut legs: Vec<Leg> = Vec::new();
    let mut over: Vec<usize> = Vec::new();
    // A way that ends at a via in the pour: its last leg comes to that via.
    let dropped = last.1 == VIA && last.3 == GOAL;
    let mut leg = Leg { layer: last.0, from: Stop::End(from), to: if dropped { Stop::Spot(last.2) } else { Stop::End(to) }, steps: Vec::new() };
    let mut at = if dropped { best[&last].prev } else { Some(last) };
    let mut steps = 0;
    while let Some(k) = at {
        steps += 1;
        assert!(steps <= best.len(), "a way goes through one crossing twice");
        let prev = best[&k].prev;
        if k.1 == VIA {
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
            leg.steps.push((k.1, k.2));
        }
        at = prev;
    }
    leg.steps.reverse();
    legs.push(leg);
    legs.reverse();
    (legs, over)
}
