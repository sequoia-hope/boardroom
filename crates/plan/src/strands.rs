//! Strands: each connection as the gaps it crosses, in order, with its place among the other
//! strands in each gap. Two strands on a layer never cross, so within a triangle every strand is
//! a chord that cuts one corner off, the chords round a corner are nested, and what is left in
//! the middle touches all three sides. A new strand may go from one side to another only inside
//! one such region; that is the whole of the bookkeeping.

use board::Board;
use serde::Serialize;

use crate::space::{dist, lerp, point_segment, End, Space, P};

#[derive(Debug, Clone, Serialize)]
pub struct Strand {
    pub net: u32,
    /// The connection this is a piece of: one connection is several strands when it has vias.
    pub connection: usize,
    pub from: End,
    pub to: End,
    /// Track width, mm.
    pub width: f64,
    /// The edges crossed, from the first end's side to the last's: both are sides of the ends
    /// themselves. Empty once the strand is lifted.
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

/// Lengths this much over still fit: sums of rule values are not exact in floating point.
pub const EPS: f64 = 1e-6;

impl Sheet {
    pub fn new(space: Space) -> Sheet {
        let on = vec![Vec::new(); space.edges.len()];
        Sheet { space, strands: Vec::new(), on }
    }

    fn clear(board: &Board, a: u32, b: u32) -> f64 {
        if a == b { 0.0 } else { board.clearance(a, b) as f64 / 1e6 }
    }

    pub fn nets_on(&self, edge: usize) -> Vec<(u32, f64)> {
        self.on[edge].iter().map(|&(s, _)| (self.strands[s].net, self.strands[s].width)).collect()
    }

    /// How far along `edge` (0 at its first corner, 1 at its second) the middle of a strand may
    /// lie, given the strands `before` and `after` it there: each end keeps its clearance and
    /// the others their widths. Where they do not fit, the middle of what is left.
    fn span(&self, board: &Board, edge: usize, me: (u32, f64), before: &[(u32, f64)], after: &[(u32, f64)]) -> (f64, f64) {
        let e = &self.space.edges[edge];
        let side = |corner: usize, others: &mut dyn Iterator<Item = &(u32, f64)>| {
            // From the corner inwards: the corner's keep, then each strand and the gap after it.
            let mut d = 0.0;
            let mut last: Option<u32> = None;
            for &(n, w) in others {
                d += last.map_or(self.space.keep(board, corner, n, None), |l| Self::clear(board, l, n)) + w;
                last = Some(n);
            }
            d + last.map_or(self.space.keep(board, corner, me.0, None), |l| Self::clear(board, l, me.0)) + me.1 / 2.0
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

    /// Where along `edge` the line from `a` to `b` crosses it (0 at its first corner), if the
    /// two are not parallel.
    fn along(&self, edge: usize, a: P, b: P) -> Option<f64> {
        let e = &self.space.edges[edge];
        let (p, q) = (self.space.verts[e.v[0]], self.space.verts[e.v[1]]);
        let (d, r) = ([b[0] - a[0], b[1] - a[1]], [q[0] - p[0], q[1] - p[1]]);
        let den = r[0] * d[1] - r[1] * d[0];
        (den.abs() > 1e-12).then(|| ((a[0] - p[0]) * d[1] - (a[1] - p[1]) * d[0]) / den)
    }

    /// The point of `edge` between `lo` and `hi` along it nearest to where the line from `a` to
    /// `b` crosses it.
    fn nearest(&self, edge: usize, (lo, hi): (f64, f64), a: P, b: P) -> P {
        let e = &self.space.edges[edge];
        let (p, q) = (self.space.verts[e.v[0]], self.space.verts[e.v[1]]);
        let t = self.along(edge, a, b).unwrap_or_else(|| {
            let r = [q[0] - p[0], q[1] - p[1]];
            let l2 = r[0] * r[0] + r[1] * r[1];
            if l2 > 0.0 { ((a[0] - p[0]) * r[0] + (a[1] - p[1]) * r[1]) / l2 } else { 0.5 }
        });
        lerp(p, q, t.clamp(lo, hi))
    }

    /// Where a new strand in `slot` of `edge` would cross it, coming from `from` and bound for
    /// `goal`: as near the straight line as the strands already there allow.
    pub fn crossing(&self, board: &Board, edge: usize, slot: usize, me: (u32, f64), from: P, goal: P) -> P {
        let nets = self.nets_on(edge);
        self.nearest(edge, self.span(board, edge, me, &nets[..slot], &nets[slot..]), from, goal)
    }

    /// Whether the straight line from `a` to `b` crosses `edge` where a new strand `me` in
    /// `slot` may be.
    pub fn sees(&self, board: &Board, edge: usize, slot: usize, me: (u32, f64), a: P, b: P) -> bool {
        let Some(t) = self.along(edge, a, b) else { return false };
        let nets = self.nets_on(edge);
        let (lo, hi) = self.span(board, edge, me, &nets[..slot], &nets[slot..]);
        t >= lo - 1e-9 && t <= hi + 1e-9
    }

    /// What the strands on `edge` need between its corners, edge to edge: with one more of a
    /// net and width `added` at a slot, and with a site taken to hold a `via` of a net.
    pub fn need(&self, board: &Board, edge: usize, added: Option<(usize, u32, f64)>, via: Option<(usize, u32)>) -> f64 {
        let e = &self.space.edges[edge];
        let mut nets = self.nets_on(edge);
        if let Some((slot, net, width)) = added {
            nets.insert(slot, (net, width));
        }
        let (Some(first), Some(last)) = (nets.first(), nets.last()) else { return 0.0 };
        let between: f64 = nets.windows(2).map(|w| Self::clear(board, w[0].0, w[1].0)).sum();
        self.space.keep(board, e.v[0], first.0, via) + self.space.keep(board, e.v[1], last.0, via) + between + nets.iter().map(|n| n.1).sum::<f64>()
    }

    /// How full an edge is: what its strands need over its length (0 with none).
    pub fn fill(&self, board: &Board, edge: usize) -> f64 {
        let len = self.space.edges[edge].len;
        if len > 0.0 { self.need(board, edge, None, None) / len } else { 0.0 }
    }

    /// How many strands cut each corner of triangle `t` off.
    pub fn corners(&self, t: usize) -> [usize; 3] {
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
    pub fn ways_on(&self, t: usize, edge: usize, slot: usize) -> Vec<(usize, usize, usize)> {
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
    pub fn throat(&self, board: &Board, t: usize, c: usize, net: u32, width: f64) -> bool {
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
        let there: f64 = (0..n)
            .map(|q| {
                let (s, _) = self.on[side][if from_first { q } else { self.on[side].len() - 1 - q }];
                self.strands[s].width + Self::clear(board, self.strands[s].net, net)
            })
            .sum();
        let need = self.space.keep(board, tri.v[c], net, None) + self.space.keep(board, wall.v[0], net, None).max(self.space.keep(board, wall.v[1], net, None)) + width + there;
        need <= room + EPS
    }

    /// Lays a strand along `steps` (each an edge and the slot taken in it, counted from the
    /// edge's first corner among the strands there before), found on this sheet as it is now.
    pub fn lay(&mut self, net: u32, connection: usize, width: f64, from: End, to: End, steps: &[(usize, usize)]) -> usize {
        let id = self.strands.len();
        // Later slots first, so a second crossing of one edge keeps its place.
        let mut order: Vec<usize> = (0..steps.len()).collect();
        order.sort_by_key(|&k| std::cmp::Reverse(steps[k]));
        for k in order {
            let (e, s) = steps[k];
            self.on[e].insert(s, (id, k));
        }
        self.strands.push(Strand { net, connection, from, to, width, crossings: steps.iter().map(|s| s.0).collect() });
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

    /// Whether no strand is in or on the edge of a via site.
    pub fn site_empty(&self, site: usize) -> bool {
        self.space.sites[site].edges.iter().all(|&e| self.on[e].is_empty())
    }

    /// Whether the strands that pass a site's corners would still fit with a via of `net` there.
    pub fn site_fits(&self, board: &Board, site: usize, net: u32) -> bool {
        self.space.sites[site].around.iter().all(|&e| self.on[e].is_empty() || self.need(board, e, None, Some((site, net))) <= self.space.edges[e].len + EPS)
    }

    /// The strand as points: its ends' middles and where it crosses each edge, pulled as tight
    /// as the other strands in each gap allow (a rubber band).
    pub fn sketch(&self, board: &Board, strand: usize) -> Vec<P> {
        let s = &self.strands[strand];
        let end = |e: End| self.space.ends.get(&e).map(|&o| self.space.obstacles[o].centre);
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
}
