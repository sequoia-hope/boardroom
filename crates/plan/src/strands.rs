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
    /// Where the strands lie ([`Sheet::places`]), worked out when first asked for.
    #[serde(skip)]
    placed: std::sync::OnceLock<Vec<Vec<f64>>>,
}

/// A part of a free triangle, as its strands divide it: each strand cuts a corner off, so the
/// parts are the bands between the strands round each corner and what is left in the middle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Region {
    /// What touches all three sides.
    Middle,
    /// Round corner `corner` of the triangle (0 to 2): past `band` of its strands, short of
    /// the next.
    Band { corner: usize, band: usize },
}

/// A spot this near where a strand lies, mm, is on neither side of it.
const BESIDE: f64 = 5.0 * crate::space::VIA_CORNER;

/// Lengths this much over still fit: sums of rule values are not exact in floating point.
pub const EPS: f64 = 1e-6;

/// How far round a spot the strands are looked at before a via is put there, mm
/// ([`Sheet::via_clear`]).
const ROUND_A_VIA: f64 = 3.0;


impl Sheet {
    pub fn new(space: Space) -> Sheet {
        let on = vec![Vec::new(); space.edges.len()];
        Sheet { space, strands: Vec::new(), on, placed: Default::default() }
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
                d += last.map_or(self.space.keep(board, corner, n), |l| Self::clear(board, l, n)) + w;
                last = Some(n);
            }
            d + last.map_or(self.space.keep(board, corner, me.0), |l| Self::clear(board, l, me.0)) + me.1 / 2.0
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
    /// net and width `added` at a slot.
    pub fn need(&self, board: &Board, edge: usize, added: Option<(usize, u32, f64)>) -> f64 {
        let e = &self.space.edges[edge];
        let mut nets = self.nets_on(edge);
        if let Some((slot, net, width)) = added {
            nets.insert(slot, (net, width));
        }
        let (Some(first), Some(last)) = (nets.first(), nets.last()) else { return 0.0 };
        let between: f64 = nets.windows(2).map(|w| Self::clear(board, w[0].0, w[1].0)).sum();
        self.space.keep(board, e.v[0], first.0) + self.space.keep(board, e.v[1], last.0) + between + nets.iter().map(|n| n.1).sum::<f64>()
    }

    /// How full an edge is: what its strands need over its length (0 with none).
    pub fn fill(&self, board: &Board, edge: usize) -> f64 {
        let len = self.space.width(edge);
        if len > 0.0 { self.need(board, edge, None) / len } else { 0.0 }
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
        let need = self.space.keep(board, tri.v[c], net) + self.space.keep(board, wall.v[0], net).max(self.space.keep(board, wall.v[1], net)) + width + there;
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
        self.placed = Default::default();
        id
    }

    /// Takes a strand out of every gap it crosses. Its number stays, with no crossings.
    pub fn lift(&mut self, strand: usize) {
        for e in std::mem::take(&mut self.strands[strand].crossings) {
            self.on[e].retain(|x| x.0 != strand);
        }
        self.placed = Default::default();
    }

    /// The strands that are laid.
    pub fn laid(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.strands.len()).filter(|&k| !self.strands[k].crossings.is_empty())
    }

    /// How far along the gap of its crossing `k` the middle of strand `s` may lie, as shares
    /// of the gap from its first corner: from each corner it keeps that corner's distance and
    /// the room of the strands between.
    pub fn room(&self, board: &Board, s: usize, k: usize) -> Option<(f64, f64)> {
        let strand = &self.strands[s];
        let e = *strand.crossings.get(k)?;
        let at = self.on[e].iter().position(|&x| x == (s, k))?;
        let nets = self.nets_on(e);
        Some(self.span(board, e, (strand.net, strand.width), &nets[..at], &nets[at + 1..]))
    }

    /// Whether no gap holds more than its room, and the strands in every free triangle are
    /// chords that each cut one corner off: no side holds more than the other two together,
    /// and the three hold an even number.
    pub fn sound(&self, board: &Board) -> bool {
        let chords = self.space.tris.iter().enumerate().all(|(t, tri)| {
            let l = tri.e.map(|e| self.on[e].len());
            !self.space.free(t) || ((l[0] + l[1] + l[2]) % 2 == 0 && l[0] <= l[1] + l[2] && l[1] <= l[0] + l[2] && l[2] <= l[0] + l[1])
        });
        chords && (0..self.space.edges.len()).all(|e| self.on[e].is_empty() || !self.space.gap(e) || self.need(board, e, None) <= self.space.width(e) + EPS)
    }

    /// The point of `edge` a share `u` of the way from its first corner to its second.
    pub fn point(&self, edge: usize, u: f64) -> P {
        let e = &self.space.edges[edge];
        lerp(self.space.verts[e.v[0]], self.space.verts[e.v[1]], u)
    }

    /// Where the strands lie: for each strand and each of its crossings, how far along the gap
    /// it crosses, as a share from the gap's first corner. Each strand is pulled tight
    /// ([`Sheet::sketch`]); where that leaves two in one gap out of their order, or nearer
    /// than they may be, they are moved apart. This is where a strand is taken to be when a
    /// via is put beside it, and the line it is followed along when the layer is triangulated
    /// again: lines that keep their order in every gap never cross.
    pub fn places(&self, board: &Board) -> &Vec<Vec<f64>> {
        self.placed.get_or_init(|| {
            let mut at: Vec<Vec<f64>> = self.strands.iter().map(|s| vec![0.5; s.crossings.len()]).collect();
            let mut tight = vec![false; self.strands.len()];
            for s in self.laid() {
                let c = &self.strands[s].crossings;
                let pts = self.sketch(board, s);
                if pts.len() != c.len() + 2 {
                    continue;
                }
                tight[s] = true;
                for (k, &e) in c.iter().enumerate() {
                    let edge = &self.space.edges[e];
                    let (a, b) = (self.space.verts[edge.v[0]], self.space.verts[edge.v[1]]);
                    let l2 = (b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2);
                    at[s][k] = if l2 > 0.0 { ((pts[k + 1][0] - a[0]) * (b[0] - a[0]) + (pts[k + 1][1] - a[1]) * (b[1] - a[1])) / l2 } else { 0.5 };
                }
            }
            for (e, on) in self.on.iter().enumerate() {
                let n = on.len();
                if n == 0 {
                    continue;
                }
                let len = self.space.edges[e].len.max(1e-12);
                // What two neighbours keep between their middles, as a share of the gap.
                let apart = |i: usize| {
                    let (a, b) = (&self.strands[on[i - 1].0], &self.strands[on[i].0]);
                    ((a.width / 2.0 + Self::clear(board, a.net, b.net) + b.width / 2.0) / len).max(1e-9)
                };
                let mut y: Vec<f64> = on.iter().map(|&(s, k)| at[s][k]).collect();
                for i in 1..n {
                    y[i] = y[i].max(y[i - 1] + apart(i));
                }
                for i in (0..n - 1).rev() {
                    y[i] = y[i].min(y[i + 1] - apart(i + 1));
                }
                // With no room for that, or a strand that could not be pulled tight: evenly.
                let fine = on.iter().all(|&(s, _)| tight[s]) && y[0] > 0.0 && y[n - 1] < 1.0 && y.windows(2).all(|w| w[0] < w[1]);
                for (i, &(s, k)) in on.iter().enumerate() {
                    at[s][k] = if fine { y[i] } else { (i + 1) as f64 / (n + 1) as f64 };
                }
            }
            at
        })
    }

    /// How long the laid strands are in all, as they lie, mm.
    pub fn length(&self, board: &Board) -> f64 {
        let places = self.places(board);
        let end = |e: End| self.space.ends.get(&e).map(|&o| self.space.obstacles[o].centre);
        self.laid()
            .map(|s| {
                let strand = &self.strands[s];
                let pts: Vec<P> = end(strand.from).into_iter().chain(strand.crossings.iter().enumerate().map(|(k, &e)| self.point(e, places[s][k]))).chain(end(strand.to)).collect();
                pts.windows(2).map(|w| dist(w[0], w[1])).sum::<f64>()
            })
            .sum()
    }

    /// The part of triangle `t` a strand is in that has come into it through `edge` at `slot`.
    pub fn region(&self, t: usize, edge: usize, slot: usize) -> Region {
        let tri = &self.space.tris[t];
        let i = tri.e.iter().position(|&e| e == edge).unwrap();
        let (j, k) = ((i + 1) % 3, (i + 2) % 3);
        let n = self.corners(t);
        let p = self.counted_from(t, i, j, slot);
        if p < n[j] {
            Region::Band { corner: j, band: p }
        } else if p > n[j] {
            Region::Band { corner: k, band: self.on[edge].len() - p }
        } else {
            Region::Middle
        }
    }

    /// The sides of triangle `t` a part of it touches, each with the slot there a strand
    /// leaving the part by that side would take.
    pub fn doors(&self, t: usize, region: Region) -> Vec<(usize, usize)> {
        let tri = &self.space.tris[t];
        match region {
            Region::Band { corner, band } => [(corner + 1) % 3, (corner + 2) % 3].iter().map(|&side| (tri.e[side], self.counted_from(t, side, corner, band))).collect(),
            Region::Middle => {
                let n = self.corners(t);
                (0..3).map(|side| (tri.e[side], self.counted_from(t, side, (side + 1) % 3, n[(side + 1) % 3]))).collect()
            }
        }
    }

    /// The one strand between two parts of triangle `t`, and the crossing of its that its
    /// line through the triangle starts from: none unless the parts lie side by side.
    pub fn chord_between(&self, t: usize, a: Region, b: Region) -> Option<(usize, usize)> {
        let n = self.corners(t);
        let band = |r: Region, c: usize| match r {
            Region::Band { corner, band } if corner == c => Some(band),
            Region::Middle => Some(n[c]),
            _ => None,
        };
        (0..3).find_map(|c| {
            let (x, y) = (band(a, c)?, band(b, c)?);
            if x.abs_diff(y) != 1 {
                return None;
            }
            let [(s, k), (s2, k2)] = self.chord(t, c, x.min(y));
            (s == s2 && k.abs_diff(k2) == 1).then_some((s, k.min(k2)))
        })
    }

    /// The strand that is `q` strands out from corner `c` of triangle `t`, and its crossing of
    /// each of the two sides that meet there.
    fn chord(&self, t: usize, c: usize, q: usize) -> [(usize, usize); 2] {
        let tri = &self.space.tris[t];
        [(c + 1) % 3, (c + 2) % 3].map(|side| {
            let on = &self.on[tri.e[side]];
            on[if self.space.edges[tri.e[side]].v[0] == tri.v[c] { q } else { on.len() - 1 - q }]
        })
    }

    /// The part of triangle `t` the point `p` is in, as the strands lie ([`Sheet::places`]):
    /// none when it is too near a strand to be on either side of it.
    pub fn region_at(&self, board: &Board, t: usize, p: P) -> Option<Region> {
        let tri = &self.space.tris[t];
        let n = self.corners(t);
        let places = self.places(board);
        let mut found = Region::Middle;
        let mut bands = 0;
        for (c, &round) in n.iter().enumerate() {
            let x = self.space.verts[tri.v[c]];
            // The strands round this corner that the point is beyond.
            let mut beyond = round;
            for q in 0..round {
                let [(s, k), (s2, k2)] = self.chord(t, c, q);
                let (a, b) = (self.point(self.strands[s].crossings[k], places[s][k]), self.point(self.strands[s2].crossings[k2], places[s2][k2]));
                let l = dist(a, b);
                if l < 1e-12 {
                    return None;
                }
                let side = |z: P| ((b[0] - a[0]) * (z[1] - a[1]) - (b[1] - a[1]) * (z[0] - a[0])) / l;
                if side(p).abs() < BESIDE {
                    return None;
                }
                if (side(p) > 0.0) == (side(x) > 0.0) {
                    beyond = q;
                    break;
                }
            }
            if beyond < round {
                bands += 1;
                found = Region::Band { corner: c, band: beyond };
            }
        }
        (bands <= 1).then_some(found)
    }

    /// Whether a via of `net` at `p`, in a part of triangle `t`, leaves the strands that pass
    /// between it and each corner of the triangle their room: those are the strands round
    /// that corner the via is beyond, and, where the via is in a band round another corner,
    /// the strands of that corner that pass beyond the via.
    pub fn via_fits(&self, board: &Board, t: usize, region: Region, p: P, net: u32) -> bool {
        let tri = &self.space.tris[t];
        let n = self.corners(t);
        let of = |c: usize, q: usize| {
            let s = &self.strands[self.chord(t, c, q)[0].0];
            (s.net, s.width)
        };
        (0..3).all(|x| {
            let inner = match region {
                Region::Band { corner, band } if corner == x => band,
                _ => n[x],
            };
            let mut nets: Vec<(u32, f64)> = (0..inner).map(|q| of(x, q)).collect();
            if let Region::Band { corner, band } = region
                && corner != x
            {
                nets.extend((band..n[corner]).rev().map(|q| of(corner, q)));
            }
            let (Some(first), Some(last)) = (nets.first(), nets.last()) else { return true };
            let between: f64 = nets.windows(2).map(|w| Self::clear(board, w[0].0, w[1].0)).sum();
            let via = if last.0 == net && net != 0 { 0.0 } else { board.clearance(last.0, net) as f64 / 1e6 + self.space.spots.radius };
            let need = self.space.keep(board, tri.v[x], first.0) + between + nets.iter().map(|w| w.1).sum::<f64>() + via;
            need <= dist(self.space.verts[tri.v[x]], p) + EPS
        })
    }

    /// Whether a via of `net` at `p`, in triangle `t`, leaves their room to the strands that
    /// pass between it and every corner within [`ROUND_A_VIA`] of it that it can see, as the
    /// strands lie ([`Sheet::places`]). Once the via is in, the triangles round it are others,
    /// and it has a gap to corners that were none of its own triangle's: what [`Sheet::via_fits`]
    /// asks of those three is asked here of the rest.
    pub fn via_clear(&self, board: &Board, t: usize, p: P, net: u32) -> bool {
        let places = self.places(board);
        let v = &self.space.verts;
        // The free triangles in reach of the spot, from its own outwards: their strands as
        // lines, their corners, and the sides that are walls.
        let mut seen = vec![t];
        let mut chords: Vec<(P, P, usize)> = Vec::new();
        let mut walls: Vec<(P, P)> = Vec::new();
        let mut corners: Vec<usize> = Vec::new();
        let mut i = 0;
        while i < seen.len() {
            let x = seen[i];
            i += 1;
            let tri = &self.space.tris[x];
            let n = self.corners(x);
            for (c, &round) in n.iter().enumerate() {
                for q in 0..round {
                    let [(s, k), (s2, k2)] = self.chord(x, c, q);
                    chords.push((self.point(self.strands[s].crossings[k], places[s][k]), self.point(self.strands[s2].crossings[k2], places[s2][k2]), s));
                }
                if !corners.contains(&tri.v[c]) && !self.space.tris[t].v.contains(&tri.v[c]) {
                    corners.push(tri.v[c]);
                }
            }
            for &e in &tri.e {
                let edge = &self.space.edges[e];
                if point_segment(p, v[edge.v[0]], v[edge.v[1]]) > ROUND_A_VIA {
                    continue;
                }
                match edge.t.iter().flatten().copied().find(|&y| y != x) {
                    Some(y) if self.space.free(y) => {
                        if !seen.contains(&y) {
                            seen.push(y);
                        }
                    }
                    _ => walls.push((v[edge.v[0]], v[edge.v[1]])),
                }
            }
        }
        if chords.is_empty() {
            return true;
        }
        // Where the line from the spot to `x` crosses the one from `a` to `b`: how far along
        // the first, if it is short of `x` and within the second.
        let cross = |x: P, a: P, b: P| {
            let (d, r) = ([x[0] - p[0], x[1] - p[1]], [b[0] - a[0], b[1] - a[1]]);
            let den = d[0] * r[1] - d[1] * r[0];
            if den.abs() < 1e-18 {
                return None;
            }
            let w = [a[0] - p[0], a[1] - p[1]];
            let (s, u) = ((w[0] * r[1] - w[1] * r[0]) / den, (w[0] * d[1] - w[1] * d[0]) / den);
            (s > 1e-9 && s < 1.0 - 1e-9 && (0.0..1.0).contains(&u)).then_some(s)
        };
        corners.iter().all(|&corner| {
            let x = v[corner];
            let d = dist(p, x);
            if d > ROUND_A_VIA || walls.iter().any(|&(a, b)| cross(x, a, b).is_some()) {
                return true;
            }
            // The strands between, from the corner outwards: one that crosses the line twice
            // is not between the two.
            let mut between: Vec<(f64, usize, usize)> = Vec::new();
            for &(a, b, s) in &chords {
                if let Some(at) = cross(x, a, b) {
                    match between.iter_mut().find(|k| k.1 == s) {
                        Some(k) => *k = (k.0.max(at), s, k.2 + 1),
                        None => between.push((at, s, 1)),
                    }
                }
            }
            between.retain(|k| k.2 % 2 == 1);
            between.sort_by(|a, b| b.0.total_cmp(&a.0));
            let nets: Vec<(u32, f64)> = between.iter().map(|k| (self.strands[k.1].net, self.strands[k.1].width)).collect();
            let (Some(first), Some(last)) = (nets.first(), nets.last()) else { return true };
            let gaps: f64 = nets.windows(2).map(|w| Self::clear(board, w[0].0, w[1].0)).sum();
            let via = if last.0 == net && net != 0 { 0.0 } else { board.clearance(last.0, net) as f64 / 1e6 + self.space.spots.radius };
            self.space.keep(board, corner, first.0) + gaps + nets.iter().map(|w| w.1).sum::<f64>() + via <= d + EPS
        })
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
