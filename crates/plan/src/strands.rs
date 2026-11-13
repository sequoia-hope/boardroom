//! Strands: each connection as the gaps it crosses, in order, with its place among the other
//! strands in each gap. Two strands on a layer never cross, so within a triangle every strand is
//! a chord that cuts one corner off, the chords round a corner are nested, and what is left in
//! the middle touches all three sides. A new strand may go from one side to another only inside
//! one such region; that is the whole of the bookkeeping.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use board::{Board, ItemId};
use serde::Serialize;

use crate::space::{dist, lerp, point_segment, Space, P};

#[derive(Debug, Clone, Serialize)]
pub struct Strand {
    pub net: u32,
    pub from: ItemId,
    pub to: ItemId,
    /// Track width, mm.
    pub width: f64,
    /// The edges crossed, from the first pad's side to the last's: both ends are sides of the
    /// pads themselves.
    pub crossings: Vec<usize>,
}

/// One layer's room and the strands laid in it.
#[derive(Debug, Clone, Serialize)]
pub struct Sheet {
    pub space: Space,
    pub strands: Vec<Strand>,
    /// For each edge, the strands that cross it in order from its first corner to its second,
    /// each with which of that strand's crossings this is.
    pub on: Vec<Vec<(usize, usize)>>,
}

/// A way found for one strand: the edges and the slot taken in each (counted from the edge's
/// first corner, among the strands there before this one), and what it costs.
#[derive(Debug, Clone)]
pub struct Way {
    pub steps: Vec<(usize, usize)>,
    pub cost: f64,
}

/// Lengths this much over still fit: sums of rule values are not exact in floating point.
const EPS: f64 = 1e-6;

#[derive(PartialEq)]
struct Open {
    cost: f64,
    edge: usize,
    slot: usize,
    /// The triangle being entered.
    into: usize,
}
impl Eq for Open {}
impl Ord for Open {
    fn cmp(&self, o: &Self) -> Ordering {
        o.cost.total_cmp(&self.cost).then(o.edge.cmp(&self.edge)).then(o.slot.cmp(&self.slot))
    }
}
impl PartialOrd for Open {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

impl Sheet {
    pub fn new(space: Space) -> Sheet {
        let on = vec![Vec::new(); space.edges.len()];
        Sheet { space, strands: Vec::new(), on }
    }

    /// How far along `edge` (0 at its first corner, 1 at its second) the middle of a strand may
    /// lie, given the strands `before` and `after` it there: each end keeps its clearance and
    /// the others their widths. Where they do not fit, the middle of what is left.
    fn span(&self, board: &Board, edge: usize, me: (u32, f64), before: &[(u32, f64)], after: &[(u32, f64)]) -> (f64, f64) {
        let e = &self.space.edges[edge];
        let clear = |a: u32, b: u32| if a == b { 0.0 } else { board.clearance(a, b) as f64 / 1e6 };
        let side = |corner: usize, others: &mut dyn Iterator<Item = &(u32, f64)>| {
            // From the corner inwards: the corner's keep, then each strand and the gap after it.
            let mut d = 0.0;
            let mut last: Option<u32> = None;
            for &(n, w) in others {
                d += last.map_or(self.space.keep(board, corner, n), |l| clear(l, n)) + w;
                last = Some(n);
            }
            d + last.map_or(self.space.keep(board, corner, me.0), |l| clear(l, me.0)) + me.1 / 2.0
        };
        let lo = side(e.v[0], &mut before.iter());
        let hi = e.len - side(e.v[1], &mut after.iter().rev());
        if e.len <= 0.0 {
            (0.5, 0.5)
        } else if lo > hi {
            let m = ((lo + hi) / 2.0 / e.len).clamp(0.0, 1.0);
            (m, m)
        } else {
            (lo / e.len, hi / e.len)
        }
    }

    fn nets_on(&self, edge: usize) -> Vec<(u32, f64)> {
        self.on[edge].iter().map(|&(s, _)| (self.strands[s].net, self.strands[s].width)).collect()
    }

    /// The point of `edge` between `lo` and `hi` along it nearest to where the line from `a` to
    /// `b` crosses it.
    fn nearest(&self, edge: usize, (lo, hi): (f64, f64), a: P, b: P) -> P {
        let e = &self.space.edges[edge];
        let (p, q) = (self.space.verts[e.v[0]], self.space.verts[e.v[1]]);
        let (d, r) = ([b[0] - a[0], b[1] - a[1]], [q[0] - p[0], q[1] - p[1]]);
        let den = r[0] * d[1] - r[1] * d[0];
        let t = if den.abs() > 1e-12 {
            ((a[0] - p[0]) * d[1] - (a[1] - p[1]) * d[0]) / den
        } else {
            let l2 = r[0] * r[0] + r[1] * r[1];
            if l2 > 0.0 { ((a[0] - p[0]) * r[0] + (a[1] - p[1]) * r[1]) / l2 } else { 0.5 }
        };
        lerp(p, q, t.clamp(lo, hi))
    }

    /// Where a new strand in `slot` of `edge` would cross it, coming from `from` and bound for
    /// `goal`: as near the straight line as the strands already there allow.
    pub fn crossing(&self, board: &Board, edge: usize, slot: usize, me: (u32, f64), from: P, goal: P) -> P {
        let nets = self.nets_on(edge);
        self.nearest(edge, self.span(board, edge, me, &nets[..slot], &nets[slot..]), from, goal)
    }

    /// The room `edge` has for copper of `net` between its corners, and what the strands on it
    /// with one of `net` and `width` added at `slot` need.
    pub fn room(&self, board: &Board, edge: usize, slot: usize, net: u32, width: f64) -> (f64, f64) {
        let e = &self.space.edges[edge];
        let mut nets: Vec<(u32, f64)> = self.on[edge].iter().map(|&(s, _)| (self.strands[s].net, self.strands[s].width)).collect();
        nets.insert(slot, (net, width));
        let first = self.space.keep(board, e.v[0], nets[0].0);
        let last = self.space.keep(board, e.v[1], nets[nets.len() - 1].0);
        let between: f64 = nets.windows(2).map(|w| if w[0].0 == w[1].0 { 0.0 } else { board.clearance(w[0].0, w[1].0) as f64 / 1e6 }).sum();
        (e.len, first + last + between + nets.iter().map(|n| n.1).sum::<f64>())
    }

    /// How many strands cut each corner of triangle `t` off.
    fn corners(&self, t: usize) -> [usize; 3] {
        let l = self.space.tris[t].e.map(|e| self.on[e].len());
        [(l[1] + l[2] - l[0]) / 2, (l[0] + l[2] - l[1]) / 2, (l[0] + l[1] - l[2]) / 2]
    }

    /// A slot of side `i` of triangle `t`, given from the edge's first corner, counted instead
    /// from the triangle's corner `from`.
    fn counted_from(&self, t: usize, i: usize, from: usize, slot: usize) -> usize {
        let e = self.space.tris[t].e[i];
        if self.space.edges[e].v[0] == self.space.tris[t].v[from] { slot } else { self.on[e].len() - slot }
    }

    /// Where a strand that came into triangle `t` through `edge` at `slot` may leave it: the
    /// side, the slot there, and the corner it then cuts off.
    fn ways_on(&self, t: usize, edge: usize, slot: usize) -> Vec<(usize, usize, usize)> {
        let tri = &self.space.tris[t];
        let i = tri.e.iter().position(|&e| e == edge).unwrap();
        let (j, k) = ((i + 1) % 3, (i + 2) % 3);
        let n = self.corners(t);
        // Side i runs between corners j and k; p counts from corner j.
        let p = self.counted_from(t, i, j, slot);
        let len = self.on[edge].len();
        // Leaving by side `side` at `q` counted from `corner`.
        let out = |side: usize, corner: usize, q: usize| (tri.e[side], self.counted_from(t, side, corner, q), corner);
        if p < n[j] {
            vec![out(k, j, p)]
        } else if p > n[j] {
            vec![out(j, k, len - p)]
        } else {
            vec![out(k, j, n[j]), out(j, k, n[k])]
        }
    }

    /// Whether one more strand of `net` and `width` fits round corner `c` of triangle `t`, where
    /// the side opposite is a wall: the strands that cut the corner off pass between the two.
    fn throat(&self, board: &Board, t: usize, c: usize, net: u32, width: f64) -> bool {
        let tri = &self.space.tris[t];
        let wall = &self.space.edges[tri.e[c]];
        let beyond = wall.t.iter().flatten().find(|&&x| x != t);
        if beyond.is_some_and(|&x| self.space.free(x)) {
            return true;
        }
        let v = &self.space.verts;
        let room = point_segment(v[tri.v[c]], v[wall.v[0]], v[wall.v[1]]);
        let side = tri.e[(c + 1) % 3];
        let n = self.corners(t)[c];
        let from_first = self.space.edges[side].v[0] == tri.v[c];
        let there: Vec<(u32, f64)> = (0..n)
            .map(|q| {
                let (s, _) = self.on[side][if from_first { q } else { self.on[side].len() - 1 - q }];
                (self.strands[s].net, self.strands[s].width)
            })
            .collect();
        let clear = |a: u32, b: u32| if a == b { 0.0 } else { board.clearance(a, b) as f64 / 1e6 };
        let need = self.space.keep(board, tri.v[c], net)
            + self.space.keep(board, wall.v[0], net).max(self.space.keep(board, wall.v[1], net))
            + width
            + there.iter().map(|&(m, w)| w + clear(m, net)).sum::<f64>();
        need <= room + EPS
    }

    /// Whether the straight line from `a` to `b` crosses `edge` where a strand `me` in `slot`
    /// may be.
    fn sees(&self, board: &Board, edge: usize, slot: usize, me: (u32, f64), a: P, b: P) -> bool {
        let e = &self.space.edges[edge];
        let (p, q) = (self.space.verts[e.v[0]], self.space.verts[e.v[1]]);
        let (d, r) = ([b[0] - a[0], b[1] - a[1]], [q[0] - p[0], q[1] - p[1]]);
        let den = r[0] * d[1] - r[1] * d[0];
        if den.abs() < 1e-12 {
            return false;
        }
        let t = ((a[0] - p[0]) * d[1] - (a[1] - p[1]) * d[0]) / den;
        let nets = self.nets_on(edge);
        let (lo, hi) = self.span(board, edge, me, &nets[..slot], &nets[slot..]);
        t >= lo - 1e-9 && t <= hi + 1e-9
    }

    /// The cheapest way for a strand of `net` from pad `from` to pad `to` through the room left.
    /// A way's cost is its length pulled tight: straight from its last bend for as long as the
    /// straight line stays inside every gap crossed since.
    pub fn find(&self, board: &Board, net: u32, width: f64, from: ItemId, to: ItemId) -> Option<Way> {
        type Key = (usize, usize, usize);
        struct Seen {
            cost: f64,
            prev: Option<Key>,
            at: P,
            /// The last bend: a crossing before this one, or the pad it started from.
            bend: Option<Key>,
        }
        const GOAL: usize = usize::MAX;
        let (a, b) = (*self.space.of_item.get(&from)?, *self.space.of_item.get(&to)?);
        let (start, goal) = (self.space.obstacles[a].centre, self.space.obstacles[b].centre);
        let me = (net, width);
        let mut best: HashMap<Key, Seen> = HashMap::new();
        let mut heap = BinaryHeap::new();
        for (e, edge) in self.space.edges.iter().enumerate() {
            let (Some(x), Some(y)) = (edge.t[0], edge.t[1]) else { continue };
            for (inner, outer) in [(x, y), (y, x)] {
                if self.space.tris[inner].inside == Some(a) && self.space.free(outer) {
                    for slot in 0..=self.on[e].len() {
                        let at = self.crossing(board, e, slot, me, start, goal);
                        let cost = dist(start, at);
                        best.insert((e, slot, outer), Seen { cost, prev: None, at, bend: None });
                        heap.push(Open { cost, edge: e, slot, into: outer });
                    }
                }
            }
        }
        while let Some(Open { cost, edge, slot, into }) = heap.pop() {
            let key = (edge, slot, into);
            if best.get(&key).is_some_and(|b| b.cost < cost) {
                continue;
            }
            if into == GOAL {
                let mut steps = vec![(edge, slot)];
                let mut at = key;
                while let Some(prev) = best[&at].prev {
                    steps.push((prev.0, prev.1));
                    at = prev;
                }
                steps.reverse();
                return Some(Way { steps, cost });
            }
            let (here, bend) = (best[&key].at, best[&key].bend);
            // The last bend's place and cost, and whether a point can be seen from it through
            // every crossing since.
            let (bend_at, bend_cost) = bend.map_or((start, 0.0), |k| (best[&k].at, best[&k].cost));
            let seen_from_bend = |p: P| {
                let mut at = Some(key);
                while at != bend {
                    let k = at.unwrap();
                    if !self.sees(board, k.0, k.1, me, bend_at, p) {
                        return false;
                    }
                    at = best[&k].prev;
                }
                true
            };
            let mut found: Vec<(Key, Seen)> = Vec::new();
            for (e, s, corner) in self.ways_on(into, edge, slot) {
                let Some(next) = self.space.edges[e].t.iter().flatten().copied().find(|&x| x != into) else { continue };
                let ends = self.space.tris[next].inside == Some(b);
                if !ends && !self.space.free(next) {
                    continue;
                }
                if !self.throat(board, into, corner, net, width) {
                    continue;
                }
                if !ends {
                    let (room, need) = self.room(board, e, s, net, width);
                    if need > room + EPS {
                        continue;
                    }
                }
                // Straight from the last bend if that can be; else this crossing is a bend.
                let straight = self.crossing(board, e, s, me, bend_at, goal);
                let (there, c, bend) = if seen_from_bend(straight) {
                    (straight, bend_cost + dist(bend_at, straight), bend)
                } else {
                    let there = self.crossing(board, e, s, me, here, goal);
                    (there, cost + dist(here, there), Some(key))
                };
                // The pad's own side is the last crossing; then the way ends at its middle.
                let (to_key, c) = if ends { ((e, s, GOAL), c + dist(there, goal)) } else { ((e, s, next), c) };
                if best.get(&to_key).is_none_or(|b| c < b.cost) {
                    found.push((to_key, Seen { cost: c, prev: Some(key), at: there, bend }));
                }
            }
            for (k, seen) in found {
                heap.push(Open { cost: seen.cost, edge: k.0, slot: k.1, into: k.2 });
                best.insert(k, seen);
            }
        }
        None
    }

    /// Lays a strand along a way found on this sheet as it is now.
    pub fn lay(&mut self, net: u32, width: f64, from: ItemId, to: ItemId, way: &Way) -> usize {
        let id = self.strands.len();
        // Later slots first, so a second crossing of one edge keeps its place.
        let mut order: Vec<usize> = (0..way.steps.len()).collect();
        order.sort_by_key(|&k| std::cmp::Reverse(way.steps[k]));
        for k in order {
            let (e, s) = way.steps[k];
            self.on[e].insert(s, (id, k));
        }
        self.strands.push(Strand { net, from, to, width, crossings: way.steps.iter().map(|s| s.0).collect() });
        id
    }

    /// Takes a strand out of every gap it crosses. Its number stays, with no crossings.
    pub fn lift(&mut self, strand: usize) {
        for e in std::mem::take(&mut self.strands[strand].crossings) {
            self.on[e].retain(|x| x.0 != strand);
        }
    }

    /// The strands that are laid.
    pub fn laid(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.strands.len()).filter(|&k| !self.strands[k].crossings.is_empty())
    }

    /// The strand as points: its pads' middles and where it crosses each edge, pulled as tight
    /// as the other strands in each gap allow (a rubber band).
    pub fn sketch(&self, board: &Board, strand: usize) -> Vec<P> {
        let s = &self.strands[strand];
        let end = |item: ItemId| self.space.of_item.get(&item).map(|&o| self.space.obstacles[o].centre);
        let (Some(a), Some(b)) = (end(s.from), end(s.to)) else { return Vec::new() };
        let me = (s.net, s.width);
        let spans: Vec<(usize, (f64, f64))> = s
            .crossings
            .iter()
            .enumerate()
            .filter_map(|(k, &e)| {
                let at = self.on[e].iter().position(|&x| x == (strand, k))?;
                let nets = self.nets_on(e);
                Some((e, self.span(board, e, me, &nets[..at], &nets[at + 1..])))
            })
            .collect();
        let mut pts: Vec<P> = std::iter::once(a).chain(spans.iter().map(|&(e, sp)| self.nearest(e, sp, a, b))).chain(std::iter::once(b)).collect();
        for _ in 0..200 {
            let mut moved: f64 = 0.0;
            for i in 1..pts.len() - 1 {
                let (e, sp) = spans[i - 1];
                let p = self.nearest(e, sp, pts[i - 1], pts[i + 1]);
                moved = moved.max(dist(p, pts[i]));
                pts[i] = p;
            }
            if moved < 1e-4 {
                break;
            }
        }
        pts
    }

    /// How full an edge is: what its strands need over its room (0 with none).
    pub fn fill(&self, board: &Board, edge: usize) -> f64 {
        let e = &self.space.edges[edge];
        if self.on[edge].is_empty() || e.len <= 0.0 {
            return 0.0;
        }
        let nets: Vec<(u32, f64)> = self.on[edge].iter().map(|&(s, _)| (self.strands[s].net, self.strands[s].width)).collect();
        let between: f64 = nets.windows(2).map(|w| if w[0].0 == w[1].0 { 0.0 } else { board.clearance(w[0].0, w[1].0) as f64 / 1e6 }).sum();
        let need = self.space.keep(board, e.v[0], nets[0].0) + self.space.keep(board, e.v[1], nets[nets.len() - 1].0) + between + nets.iter().map(|n| n.1).sum::<f64>();
        need / e.len
    }
}
