//! Finding a way for one connection through the room left on every layer at once: along a layer
//! from gap to gap, and from layer to layer where a via site is free.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use board::Board;

use crate::space::{dist, End, P};
use crate::strands::{Sheet, EPS};

/// One layer's piece of a way: the edges crossed and the slot taken in each, between two ends.
#[derive(Debug, Clone)]
pub struct Leg {
    pub layer: usize,
    pub from: End,
    pub to: End,
    pub steps: Vec<(usize, usize)>,
}

#[derive(Debug, Clone)]
pub struct Way {
    pub legs: Vec<Leg>,
    pub cost: f64,
}

/// A crossing as a place in the search: the layer, the edge, the slot, the triangle entered.
type Key = (usize, usize, usize, usize);

struct Seen {
    cost: f64,
    prev: Option<Key>,
    at: P,
    /// The last bend: its place and the cost there, and the crossing it is (none when it is the
    /// end the leg started from).
    bend_at: P,
    bend_cost: f64,
    bend: Option<Key>,
    /// The first crossing of a leg, and the end it left.
    starts: Option<End>,
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

const GOAL: usize = usize::MAX;

/// What the search is told about the board's layers and vias.
pub struct Terms<'a> {
    /// What a strand's length costs on each layer, as a multiple.
    pub dear: &'a [f64],
    /// What a via costs, as length, mm; `None` for no vias.
    pub via: Option<f64>,
}

/// The cheapest way for a strand of `net` from `from` to `to`. A way's cost is its length pulled
/// tight: straight from its last bend for as long as the straight line stays inside every gap
/// crossed since.
pub fn find(board: &Board, sheets: &[Sheet], terms: &Terms, net: u32, width: f64, from: End, to: End) -> Option<Way> {
    let me = (net, width);
    let middle = |e: End| sheets.iter().find_map(|s| s.space.ends.get(&e).map(|&o| s.space.obstacles[o].centre));
    let goal = middle(to)?;
    let mut best: HashMap<Key, Seen> = HashMap::new();
    let mut heap = BinaryHeap::new();
    let cheapest = terms.dear.iter().copied().fold(f64::MAX, f64::min);
    // The first crossings of a leg that leaves `end` on layer `l` with `cost` already spent.
    let leave = |l: usize, end: End, cost: f64, prev: Option<Key>, best: &mut HashMap<Key, Seen>, heap: &mut BinaryHeap<Open>| {
        let sheet = &sheets[l];
        let Some(&o) = sheet.space.ends.get(&end) else { return };
        let start = sheet.space.obstacles[o].centre;
        // A site's sides are known; a pad's are found by looking at every edge, once a leg.
        let all: Vec<usize>;
        let sides: &[usize] = match end {
            End::Site(k) => &sheet.space.sites[k].edges,
            End::Item(_) => {
                all = (0..sheet.space.edges.len()).collect();
                &all
            }
        };
        for &e in sides {
            let edge = &sheet.space.edges[e];
            let (Some(x), Some(y)) = (edge.t[0], edge.t[1]) else { continue };
            for (inner, outer) in [(x, y), (y, x)] {
                // A site not yet taken is not `inside` anything: its triangles are told by site.
                let within = sheet.space.tris[inner].inside == Some(o) || matches!(end, End::Site(k) if sheet.space.tri_site[inner] == Some(k));
                let out = sheet.space.free(outer) && !matches!(end, End::Site(k) if sheet.space.tri_site[outer] == Some(k));
                if within && out {
                    for slot in 0..=sheet.on[e].len() {
                        let at = sheet.crossing(board, e, slot, me, start, goal);
                        let c = cost + terms.dear[l] * dist(start, at);
                        let key = (l, e, slot, outer);
                        if best.get(&key).is_none_or(|b| c < b.cost) {
                            best.insert(key, Seen { cost: c, prev, at, bend_at: start, bend_cost: cost, bend: None, starts: Some(end) });
                            heap.push(Open { order: c + cheapest * dist(at, goal), cost: c, key });
                        }
                    }
                }
            }
        }
    };
    for l in 0..sheets.len() {
        leave(l, from, 0.0, None, &mut best, &mut heap);
    }
    while let Some(Open { cost, key, .. }) = heap.pop() {
        if best.get(&key).is_some_and(|b| b.cost < cost) {
            continue;
        }
        let (l, edge, slot, into) = key;
        if into == GOAL {
            return Some(Way { legs: legs(&best, key, to), cost });
        }
        let sheet = &sheets[l];
        let seen = &best[&key];
        let (here, bend, bend_at, bend_cost) = (seen.at, seen.bend, seen.bend_at, seen.bend_cost);
        // Into a via site from outside it: a via here, if the site is free on every layer and
        // the strands round it leave it room, and the way goes on from it on another layer.
        if let (Some(via), Some(site)) = (terms.via, sheet.space.tri_site[into])
            && sheet.space.free(into)
            && sheet.space.edges[edge].t.iter().flatten().any(|&t| sheet.space.tri_site[t] != Some(site))
            && sheets.iter().all(|s| s.site_empty(site) && s.site_fits(board, site, net))
        {
            let centre = sheet.space.obstacles[sheet.space.sites[site].obstacle].centre;
            let at_via = cost + terms.dear[l] * dist(here, centre) + via;
            let mut found: Vec<(Key, Seen)> = Vec::new();
            {
                let mut scratch: HashMap<Key, Seen> = HashMap::new();
                let mut none = BinaryHeap::new();
                for m in (0..sheets.len()).filter(|&m| m != l) {
                    leave(m, End::Site(site), at_via, Some(key), &mut scratch, &mut none);
                }
                found.extend(scratch);
            }
            for (k, s) in found {
                if best.get(&k).is_none_or(|b| s.cost < b.cost) {
                    heap.push(Open { order: s.cost + cheapest * dist(s.at, goal), cost: s.cost, key: k });
                    best.insert(k, s);
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
                    (None, Some(prev)) => at = prev,
                    _ => return true,
                }
            }
        };
        let mut found: Vec<(Key, Seen)> = Vec::new();
        for (e, s, corner) in sheet.ways_on(into, edge, slot) {
            let Some(next) = sheet.space.edges[e].t.iter().flatten().copied().find(|&x| x != into) else { continue };
            let ends = sheet.space.ends.get(&to).is_some_and(|&o| sheet.space.tris[next].inside == Some(o));
            if !ends && !sheet.space.free(next) {
                continue;
            }
            if !sheet.throat(board, into, corner, net, width) {
                continue;
            }
            if !ends && !sheet.space.scaffold(e) && sheet.need(board, e, Some((s, net, width)), None) > sheet.space.edges[e].len + EPS {
                continue;
            }
            // Straight from the last bend if that can be; else this crossing is a bend.
            let straight = sheet.crossing(board, e, s, me, bend_at, goal);
            let (there, c, bend, bend_at, bend_cost) = if seen_from_bend(straight) {
                (straight, bend_cost + terms.dear[l] * dist(bend_at, straight), bend, bend_at, bend_cost)
            } else {
                let there = sheet.crossing(board, e, s, me, here, goal);
                (there, cost + terms.dear[l] * dist(here, there), Some(key), here, cost)
            };
            // The end's own side is the last crossing; then the way ends at its middle.
            let (to_key, c) = if ends { ((l, e, s, GOAL), c + terms.dear[l] * dist(there, goal)) } else { ((l, e, s, next), c) };
            if best.get(&to_key).is_none_or(|b| c < b.cost) {
                found.push((to_key, Seen { cost: c, prev: Some(key), at: there, bend_at, bend_cost, bend, starts: None }));
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

/// The way that ends at `last`, as legs: a new leg wherever a crossing starts one.
fn legs(best: &HashMap<Key, Seen>, last: Key, to: End) -> Vec<Leg> {
    let mut legs: Vec<Leg> = Vec::new();
    let mut leg = Leg { layer: last.0, from: to, to, steps: Vec::new() };
    let mut at = Some(last);
    while let Some(k) = at {
        leg.steps.push((k.1, k.2));
        let seen = &best[&k];
        if let Some(end) = seen.starts {
            leg.from = end;
            leg.steps.reverse();
            legs.push(std::mem::replace(&mut leg, Leg { layer: 0, from: end, to: end, steps: Vec::new() }));
            // The leg before this one ends where this one started, on the layer it came by.
            if let Some(prev) = seen.prev {
                leg.layer = prev.0;
            }
        }
        at = seen.prev;
    }
    legs.reverse();
    legs
}
