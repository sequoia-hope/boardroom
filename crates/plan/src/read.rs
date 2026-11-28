//! Copper read back as strands (owner, 2026-11-26: "have the new router redraw what the old
//! one closes"). The copper another router laid for a net shows that there is a way: the way
//! is kept and the copper is not. A net's tracks on a layer are cut into paths that each run
//! from one pad or via to another; each path is followed through the triangles of the board
//! as it stands without that copper ([`carry`]) and becomes a strand, for `wire` to draw as
//! it draws any other.
//!
//! Where tracks meet in a T, the paths that share its stem run side by side, as two strands of
//! one net do in any plan: each lies a little to its own side of the copper's middle line,
//! the side it is on when the tracks are walked round from pad to pad, so no two cross.
//!
//! A net is not read where its copper cannot be taken so: a track that ends in
//! the net's pour, a line that passes over a pad of its own net or the room kept for a pad's
//! spoke, a via that does not stand at our size, or lines that cross as they are read. It is
//! named with the reason, for whoever asked to leave its copper as it was laid.
//!
//! Only the copper asked for is read. What else a net has on the board stays, and a path may
//! end on a track of its net that stays as it may on a pad.

use std::collections::{HashMap, HashSet};

use board::{Board, ItemId, Kind, Shape};

use crate::carry::{self, Anchor, Trace};
use crate::space::{dist, mm, End, Space, P};
use crate::strands::{Sheet, Strand};
use crate::{Plan, Want};

/// A straight piece of track on one layer, mm, and the track it is of.
#[derive(Debug, Clone)]
pub struct Piece {
    pub id: ItemId,
    pub net: u32,
    pub layer: usize,
    pub a: P,
    pub b: P,
    pub width: f64,
}

/// A via taken off the board: which it was, its middle, mm, and its net.
#[derive(Debug, Clone)]
pub struct Drill {
    pub id: ItemId,
    pub at: P,
    pub net: u32,
}

/// Takes the tracks and vias `which` off `board`.
pub fn lift(board: &mut Board, which: &[ItemId]) -> (Vec<Piece>, Vec<Drill>) {
    let (mut pieces, mut vias) = (Vec::new(), Vec::new());
    for &id in which {
        if !board.item(id).is_some_and(|i| matches!(i.kind, Kind::Track | Kind::Via) && matches!(i.shape, Shape::Capsule { .. })) {
            continue;
        }
        let Some(item) = board.remove(id) else { continue };
        let Shape::Capsule { a, b, r } = item.shape else { continue };
        if matches!(item.kind, Kind::Via) {
            vias.push(Drill { id, at: mm(a), net: item.net });
            continue;
        }
        for layer in (0..board.layers.len()).filter(|&l| item.on(l)) {
            pieces.push(Piece { id, net: item.net, layer, a: mm(a), b: mm(b), width: 2.0 * r as f64 / 1e6 });
        }
    }
    (pieces, vias)
}

/// Copper that cannot be read: the tracks and vias to leave as they were laid, their net,
/// and why.
#[derive(Debug, Clone)]
pub struct Unread {
    pub items: Vec<ItemId>,
    pub net: u32,
    pub why: String,
}

/// Copper read: the plan, and for each connection of it ([`Plan::wants`]) the tracks its
/// strand was read from.
pub struct Read {
    pub plan: Plan,
    pub from: Vec<Vec<ItemId>>,
}

/// One path of a net's copper on a layer, from one pad or via to another.
#[derive(Debug, Clone)]
struct Path {
    net: u32,
    layer: usize,
    from: End,
    to: End,
    pts: Vec<P>,
    /// The narrowest of its pieces, mm.
    width: f64,
    /// The tracks it runs along.
    of: Vec<ItemId>,
}

/// How far to its own side of the copper's middle line a path lies where it shares tracks
/// with another of its net, mm: more than any tolerance of the reading, less than half the
/// narrowest track a fab makes.
const ASIDE: f64 = 0.01;

/// Two points this near are one place, mm.
const SAME: f64 = 1e-6;

/// A track's end this near the end of another it lies on is joined to that end, mm.
const AT_END: f64 = 1e-3;

fn nm(p: P) -> board::Point {
    board::Point::new((p[0] * 1e6).round() as i64, (p[1] * 1e6).round() as i64)
}

fn key(p: P) -> (i64, i64) {
    ((p[0] * 1e6).round() as i64, (p[1] * 1e6).round() as i64)
}

fn unit(a: P, b: P) -> P {
    let d = dist(a, b).max(1e-12);
    [(b[0] - a[0]) / d, (b[1] - a[1]) / d]
}

/// The paths of one net's tracks on one layer: `ends` says what a place is on, if anything,
/// and `middles` are the middles of the pads and vias a track of the net may end on here.
/// With them, where the net is poured on this layer, the tracks that end on no pad or via: one
/// may be what joins its pad to the pour, and is not ours to take out.
fn paths(net: u32, layer: usize, pieces: &[&Piece], ends: &dyn Fn(P) -> Option<End>, middles: &[P], stays: &[(P, P)], poured: bool) -> (Vec<Path>, Vec<ItemId>) {
    let mut nodes: Vec<P> = Vec::new();
    let mut index: HashMap<(i64, i64), usize> = HashMap::new();
    let mut node = |p: P| {
        *index.entry(key(p)).or_insert_with(|| {
            nodes.push(p);
            nodes.len() - 1
        })
    };
    let segs: Vec<&&Piece> = pieces.iter().filter(|s| key(s.a) != key(s.b)).collect();
    // Where a track's end lies on another track and not at its end, that track is cut there:
    // a T. An end that lies beside the other's middle line, on its copper, is joined to it
    // by a link straight across.
    let mut cuts: Vec<Vec<f64>> = vec![Vec::new(); segs.len()];
    let mut links: Vec<(P, P, f64, ItemId)> = Vec::new();
    for (i, s) in segs.iter().enumerate() {
        for p in [s.a, s.b] {
            for (j, o) in segs.iter().enumerate() {
                if i == j || key(p) == key(o.a) || key(p) == key(o.b) {
                    continue;
                }
                let len = dist(o.a, o.b);
                let d = unit(o.a, o.b);
                let t = (((p[0] - o.a[0]) * d[0] + (p[1] - o.a[1]) * d[1]) / len).clamp(0.0, 1.0);
                let q = [o.a[0] + (o.b[0] - o.a[0]) * t, o.a[1] + (o.b[1] - o.a[1]) * t];
                if dist(p, q) > o.width / 2.0 + SAME {
                    continue;
                }
                let w = s.width.min(o.width);
                if t * len < AT_END {
                    links.push((p, o.a, w, s.id));
                } else if (1.0 - t) * len < AT_END {
                    links.push((p, o.b, w, s.id));
                } else {
                    cuts[j].push(t);
                    links.push((p, q, w, s.id));
                }
            }
        }
    }
    // A track that runs over a pad or via of its net with no corner there is cut where it is
    // nearest that one's middle: it is joined to it there, and is two paths.
    for (j, o) in segs.iter().enumerate() {
        if ends(o.a).is_some() && ends(o.a) == ends(o.b) {
            continue;
        }
        for &c in middles {
            let len = dist(o.a, o.b);
            let d = unit(o.a, o.b);
            let t = (((c[0] - o.a[0]) * d[0] + (c[1] - o.a[1]) * d[1]) / len).clamp(0.0, 1.0);
            let q = [o.a[0] + (o.b[0] - o.a[0]) * t, o.a[1] + (o.b[1] - o.a[1]) * t];
            let on = ends(q);
            if on.is_some() && on != ends(o.a) && on != ends(o.b) {
                cuts[j].push(t);
            }
        }
    }
    // And where it crosses the middle line of a track of its net that stays.
    for (j, o) in segs.iter().enumerate() {
        for &(a, b) in stays {
            if let Some(t) = cross(o.a, o.b, a, b)
                && t * dist(o.a, o.b) > AT_END
                && (1.0 - t) * dist(o.a, o.b) > AT_END
            {
                cuts[j].push(t);
            }
        }
    }
    let mut edges: Vec<(usize, usize, f64, ItemId)> = Vec::new();
    for (j, s) in segs.iter().enumerate() {
        let mut at = cuts[j].clone();
        at.sort_by(|a, b| a.total_cmp(b));
        let mut last = node(s.a);
        for t in at.into_iter().chain([1.0]) {
            let next = node([s.a[0] + (s.b[0] - s.a[0]) * t, s.a[1] + (s.b[1] - s.a[1]) * t]);
            if next != last {
                edges.push((last, next, s.width, s.id));
            }
            last = next;
        }
    }
    for (p, q, w, id) in links {
        let (a, b) = (node(p), node(q));
        if a != b {
            edges.push((a, b, w, id));
        }
    }
    // A tree of them: a piece that closes a ring is left out (the tracks first, so that it is
    // a link where it can be).
    let mut root: Vec<usize> = (0..nodes.len()).collect();
    fn find(root: &mut [usize], mut x: usize) -> usize {
        while root[x] != x {
            root[x] = root[root[x]];
            x = root[x];
        }
        x
    }
    let mut next_to: Vec<Vec<(usize, f64, ItemId)>> = vec![Vec::new(); nodes.len()];
    for (a, b, w, id) in edges {
        let (ra, rb) = (find(&mut root, a), find(&mut root, b));
        if ra == rb {
            continue;
        }
        root[ra] = rb;
        next_to[a].push((b, w, id));
        next_to[b].push((a, w, id));
    }
    let end: Vec<Option<End>> = nodes.iter().map(|&p| ends(p)).collect();
    // A track that ends on nothing joins nothing, unless the net is poured on this layer:
    // then it may be what joins its pad to the pour.
    let mut loose: Vec<ItemId> = Vec::new();
    while let Some(leaf) = (0..nodes.len()).find(|&n| next_to[n].len() == 1 && end[n].is_none()) {
        let (other, _, id) = next_to[leaf][0];
        if poured && !loose.contains(&id) {
            loose.push(id);
        }
        next_to[leaf].clear();
        next_to[other].retain(|x| x.0 != leaf);
    }
    // Walked round from each pad or via: at every fork the branches in turn, counter-clockwise
    // from the way back, so that the ends come in the order they lie round the tracks.
    let mut walked: HashSet<(usize, usize)> = HashSet::new();
    let mut out = Vec::new();
    for start in (0..nodes.len()).filter(|&n| end[n].is_some()) {
        for first in next_to[start].iter().map(|x| x.0).collect::<Vec<_>>() {
            if walked.contains(&(start.min(first), start.max(first))) {
                continue;
            }
            let mut parent: HashMap<usize, usize> = HashMap::new();
            let mut leaves = vec![start];
            let mut stack = vec![(first, start)];
            while let Some((at, from)) = stack.pop() {
                walked.insert((at.min(from), at.max(from)));
                parent.insert(at, from);
                if end[at].is_some() {
                    leaves.push(at);
                    continue;
                }
                let back = unit(nodes[at], nodes[from]);
                let turn = |n: usize| {
                    let d = unit(nodes[at], nodes[n]);
                    let a = (back[0] * d[1] - back[1] * d[0]).atan2(back[0] * d[0] + back[1] * d[1]);
                    if a <= 0.0 { a + 2.0 * std::f64::consts::PI } else { a }
                };
                let mut branches: Vec<usize> = next_to[at].iter().map(|x| x.0).filter(|&n| n != from).collect();
                // The last pushed is the first walked.
                branches.sort_by(|&a, &b| turn(b).total_cmp(&turn(a)));
                stack.extend(branches.into_iter().map(|n| (n, at)));
            }
            let up = |mut n: usize| {
                let mut way = vec![n];
                while n != start {
                    n = parent[&n];
                    way.push(n);
                }
                way
            };
            for pair in leaves.windows(2) {
                let (a, b) = (up(pair[0]), up(pair[1]));
                // Where the two ways to the start first meet.
                let Some((ia, ib)) = a.iter().enumerate().find_map(|(i, n)| b.iter().position(|m| m == n).map(|j| (i, j))) else { continue };
                let way: Vec<usize> = a[..=ia].iter().copied().chain(b[..ib].iter().rev().copied()).collect();
                let (Some(from), Some(to)) = (end[way[0]], end[way[way.len() - 1]]) else { continue };
                // (A way from a pad, via or track back onto the same one is no strand.)
                if from == to || way.len() < 2 {
                    continue;
                }
                let along: Vec<(f64, ItemId)> = way.windows(2).filter_map(|w| next_to[w[0]].iter().find(|x| x.0 == w[1]).map(|x| (x.1, x.2))).collect();
                let width = along.iter().map(|x| x.0).fold(f64::MAX, f64::min);
                let mut of: Vec<ItemId> = along.iter().map(|x| x.1).collect();
                of.dedup();
                let mut pts: Vec<P> = way.iter().map(|&n| nodes[n]).collect();
                if leaves.len() > 2 {
                    // Shared with another path of the net: to the right of its own way.
                    let line = pts.clone();
                    for k in 1..line.len() - 1 {
                        let (d, e) = (unit(line[k - 1], line[k]), unit(line[k], line[k + 1]));
                        let mut m = [d[1] + e[1], -d[0] - e[0]];
                        let mut l = (m[0] * m[0] + m[1] * m[1]).sqrt();
                        if l < 0.2 {
                            (m, l) = ([d[1], -d[0]], 1.0);
                        }
                        let by = ASIDE / (l / 2.0).max(0.5) / l;
                        pts[k] = [line[k][0] + m[0] * by, line[k][1] + m[1] * by];
                    }
                }
                out.push(Path { net, layer, from, to, pts, width, of });
            }
        }
    }
    // A piece left loose is not a path's to take.
    out.retain(|p: &Path| !p.of.iter().any(|id| loose.contains(id)));
    (out, loose)
}

/// Where the segment from `p` to `q` crosses the one from `a` to `b`: how far along the first.
fn cross(p: P, q: P, a: P, b: P) -> Option<f64> {
    let (d, r) = ([q[0] - p[0], q[1] - p[1]], [b[0] - a[0], b[1] - a[1]]);
    let den = d[0] * r[1] - d[1] * r[0];
    if den.abs() < 1e-18 {
        return None;
    }
    let w = [a[0] - p[0], a[1] - p[1]];
    let s = (w[0] * r[1] - w[1] * r[0]) / den;
    let u = (w[0] * d[1] - w[1] * d[0]) / den;
    ((0.0..=1.0).contains(&u) && (0.0..=1.0).contains(&s)).then_some(s)
}

/// A line that starts on `end`, as [`carry`] reads one: from the via's middle, or from where
/// it last crosses a side of the pad (a drawn track leaves its pad by whichever side it
/// likes, and may end short of the pad's middle or past it). The anchor, and the line from
/// there on.
fn anchored(board: &Board, space: &Space, rim: &HashMap<usize, Vec<usize>>, end: End, mut pts: Vec<P>) -> Option<(End, Anchor, Vec<P>)> {
    let o = *space.ends.get(&end)?;
    let track = |o: usize| space.obstacles[o].item.and_then(|id| board.item(id)).is_some_and(|i| matches!(i.kind, Kind::Track));
    // Where it is taken to end: the middle of a pad or via; on a track that stays, the place
    // on its middle line nearest where the line stops.
    let middle = match space.obstacles[o].item.and_then(|id| board.item(id)) {
        Some(board::Item { kind: Kind::Track, shape: Shape::Capsule { a, b, .. }, .. }) => {
            let (a, b) = (mm(*a), mm(*b));
            let len = dist(a, b).max(1e-12);
            let d = unit(a, b);
            // (Inside its ends: the line is to leave by crossing a side of it.)
            let within = (2.0 * crate::space::VIA_CORNER / len).min(0.5);
            let t = (((pts[0][0] - a[0]) * d[0] + (pts[0][1] - a[1]) * d[1]) / len).clamp(within, 1.0 - within);
            [a[0] + (b[0] - a[0]) * t - OFF[0], a[1] + (b[1] - a[1]) * t - OFF[1]]
        }
        _ => space.obstacles[o].centre,
    };
    // A little off it: a via's corners in the triangles lie straight up and at 30 degrees
    // from its middle, a square pad's at 45, and a track that leaves along such a line would
    // run through a corner, where which gap it crosses is anyone's guess.
    let centre = [middle[0] + OFF[0], middle[1] + OFF[1]];
    if let End::Site(k) = end {
        // From the via's middle straight to the first place off its copper: a track that
        // ends on the via short of its middle, or past it, would be led back across it.
        let reach = space.obstacles[o].reach;
        let off = pts.iter().rposition(|&p| dist(p, centre) <= reach).map_or(0, |i| (i + 1).min(pts.len() - 1));
        let mut line = vec![centre];
        line.extend(pts[off..].iter().copied().filter(|&p| dist(p, centre) > SAME));
        return (line.len() > 1).then_some((end, Anchor::Via(k), line));
    }
    if dist(pts[0], centre) > SAME {
        pts.insert(0, centre);
    } else {
        pts[0] = centre;
    }
    // What it may leave from: the thing it ends on, and any track of the net that stays and
    // touches it, or touches one that does (they lie end to end from it, and a track laid along
    // them leaves from the last of them it is on).
    let net = space.obstacles[o].net;
    let mut from = vec![o];
    let shape = |x: usize| space.obstacles[x].item.and_then(|id| board.item(id)).map(|i| &i.shape);
    let line = |x: usize| match shape(x) {
        Some(Shape::Capsule { a, b, .. }) => Some((*a, *b)),
        _ => None,
    };
    let touch = |p: usize, q: usize| line(p).is_some_and(|(a, b)| shape(q).is_some_and(|s| s.contains(a) || s.contains(b)));
    let others: Vec<usize> = (0..space.obstacles.len()).filter(|&x| x != o && space.obstacles[x].net == net && net != 0 && track(x)).collect();
    let mut k = 0;
    while k < from.len() {
        let here = from[k];
        k += 1;
        for &x in &others {
            if !from.contains(&x) && (touch(here, x) || touch(x, here)) {
                from.push(x);
            }
        }
    }
    let mut best: Option<(usize, f64, usize, usize)> = None;
    for (i, w) in pts.windows(2).enumerate() {
        for &x in &from {
            for &e in rim.get(&x).into_iter().flatten() {
                let [a, b] = space.edges[e].v.map(|v| space.verts[v]);
                if let Some(s) = cross(w[0], w[1], a, b)
                    && best.is_none_or(|(bi, bs, _, _)| (i, s) > (bi, bs))
                {
                    best = Some((i, s, e, x));
                }
            }
        }
    }
    let (i, s, e, x) = best?;
    let end = if x == o { end } else { End::Item(space.obstacles[x].item?) };
    let at = [pts[i][0] + s * (pts[i + 1][0] - pts[i][0]), pts[i][1] + s * (pts[i + 1][1] - pts[i][1])];
    let [a, b] = space.edges[e].v.map(|v| space.verts[v]);
    let mut line = vec![at];
    line.extend(pts[i + 1..].iter().copied().filter(|&p| dist(p, at) > SAME));
    Some((end, Anchor::Side(a, b), line))
}

/// How far off the middle of what it ends on a line is taken to end, mm: well inside a via's
/// corners ([`crate::space::VIA_CORNER`]), and along no line a track is drawn on.
const OFF: P = [0.00017, 0.00011];

/// How far from the middle of a via of its own net a line that runs through that middle is led
/// round it, mm: past the corners the via has in the triangles ([`crate::space::VIA_CORNER`]).
const ROUND: f64 = 3.0 * crate::space::VIA_CORNER;

/// `pts` with each corner that lies in the middle of one of `vias` moved off it, to the inside
/// of the turn the line makes there (to its left where it runs straight on): a track on a
/// pad's own layer may run through the hole in that pad, where the plan has a via's corners.
fn round_vias(pts: &mut [P], vias: &[P]) {
    for k in 1..pts.len().saturating_sub(1) {
        let Some(&c) = vias.iter().find(|&&c| dist(c, pts[k]) < ROUND / 2.0) else { continue };
        let (d, e) = (unit(pts[k - 1], c), unit(c, pts[k + 1]));
        let mut b = [e[0] - d[0], e[1] - d[1]];
        let l = (b[0] * b[0] + b[1] * b[1]).sqrt();
        b = if l < 0.05 { [-d[1], d[0]] } else { [b[0] / l, b[1] / l] };
        pts[k] = [c[0] + b[0] * ROUND, c[1] + b[1] * ROUND];
    }
}

/// The copper of `pieces` and `vias` as a plan of `board`, the board without that copper:
/// every path of it a strand, every via a via of the plan (at our size for its net, where the
/// other router's was). Or, where some of it cannot be read so, that copper with why: the
/// plan is of all of it or none, since a via left out is room the others' strands were not
/// read round. Left on the board as it was laid, what cannot be read is copper that stays, and
/// the rest can be read round it.
///
/// A track that stays and has two paths end on it at two places is cut in two between them
/// on `board`: a wire is drawn to the one place its end has in the plan, and each path's wire
/// is to end where its track did.
pub fn read(board: &mut Board, pieces: &[Piece], drills: &[Drill]) -> Result<Read, Vec<Unread>> {
    let unread: std::cell::RefCell<Vec<Unread>> = Default::default();
    let not = |items: &[ItemId], net: u32, why: &str| unread.borrow_mut().push(Unread { items: items.to_vec(), net, why: why.to_string() });
    let (mut all, holes, vias, nets) = {
        let board: &Board = board;
        let pads = |net: u32, layer: Option<usize>| board.items().filter(move |(_, i)| matches!(i.kind, Kind::Pad { .. } | Kind::Via) && i.net == net && layer.is_none_or(|l| i.on(l)));
        // A via in a pad of its own net is a hole in that pad ([`crate::space::Spots::holes`]):
        // the plan's first vias.
        let in_pad = |at: P, net: u32| board.items().find(|(_, i)| matches!(i.kind, Kind::Pad { .. }) && i.net == net && i.hole.is_none() && i.shape.contains(nm(at))).map(|(id, _)| id);
        let (in_pads, others): (Vec<&Drill>, Vec<&Drill>) = drills.iter().partition(|d| in_pad(d.at, d.net).is_some());
        let holes: Vec<(ItemId, P, u32)> = in_pads.iter().filter_map(|d| in_pad(d.at, d.net).map(|pad| (pad, d.at, d.net))).collect();
        let drills: Vec<&Drill> = in_pads.into_iter().chain(others).collect();
        let vias: Vec<(P, u32)> = drills.iter().map(|d| (d.at, d.net)).collect();
        let vias = &vias[..];
        // Our vias where theirs were, all on the board together: each must stand.
        let mut with = board.clone();
        let stood: Vec<ItemId> = vias.iter().map(|&(at, net)| with.insert(crate::via_item(board, at, net))).collect();
        for (&id, d) in stood.iter().zip(&drills) {
            if let Some(fault) = board::check_item(&with, id).first() {
                not(&[d.id], d.net, &format!("a via of our size does not stand where its via was, at {:.2}, {:.2} ({:?}, {:.3} of {:.3} mm)", d.at[0], d.at[1], fault.rule, fault.gap / 1e6, fault.required as f64 / 1e6));
            }
        }
        let mut nets: Vec<u32> = pieces.iter().map(|p| p.net).chain(vias.iter().map(|v| v.1)).collect();
        nets.sort_unstable();
        nets.dedup();
        let mut all: Vec<Path> = Vec::new();
        for &net in &nets {
            let reach = crate::via_size(board, net).0 as f64 / 1e6;
            for layer in 0..board.layers.len() {
                let here: Vec<&Piece> = pieces.iter().filter(|p| p.net == net && p.layer == layer).collect();
                if here.is_empty() {
                    continue;
                }
                // On a pad where there is one on this layer (a hole in it is the pad there) or a
                // via that stays, else on a via of the plan, else on a track of the net that stays.
                let stays = || board.items().filter(move |(_, i)| matches!(i.kind, Kind::Track) && i.net == net && i.on(layer));
                let ends = |p: P| {
                    pads(net, Some(layer))
                        .find(|(_, i)| i.shape.contains(nm(p)))
                        .map(|(id, _)| End::Item(id))
                        .or_else(|| {
                            // (A hole in a pad is that pad on the pad's own layer, where
                            // its ring reaches past the pad too.)
                            let k = vias.iter().position(|&(at, n)| n == net && dist(at, p) <= reach)?;
                            Some(holes.get(k).filter(|h| board.item(h.0).is_some_and(|i| i.on(layer))).map_or(End::Site(k), |h| End::Item(h.0)))
                        })
                        .or_else(|| stays().find(|(_, i)| i.shape.contains(nm(p))).map(|(id, _)| End::Item(id)))
                };
                let poured = board.zone_areas.iter().any(|z| z.net == net && z.layers & (1 << layer) != 0);
                let lines: Vec<(P, P)> = stays()
                    .filter_map(|(_, i)| match i.shape {
                        Shape::Capsule { a, b, .. } => Some((mm(a), mm(b))),
                        Shape::Polygon(_) => None,
                    })
                    .collect();
                let at: Vec<P> = pads(net, Some(layer))
                    .map(|(_, i)| i.shape.bbox())
                    .map(|b| [(b.min.x + b.max.x) as f64 / 2e6, (b.min.y + b.max.y) as f64 / 2e6])
                    .chain(vias.iter().filter(|v| v.1 == net).map(|v| v.0))
                    .chain(lines.iter().flat_map(|l| [l.0, l.1]))
                    .collect();
                let (found, loose) = paths(net, layer, &here, &ends, &at, &lines, poured);
                if !loose.is_empty() {
                    not(&loose, net, "a track of it ends on no pad or via, on a layer its net is poured on");
                }
                all.extend(found);
            }
        }
        (all, holes, vias.to_vec(), nets)
    };
    let vias = &vias[..];
    // Where on a track that stays each path that ends on one ends: its wire is to be drawn to
    // there. A track with several such places is cut between them, each part with its own.
    let mut middles: HashMap<ItemId, P> = HashMap::new();
    let mut places: Vec<(ItemId, Vec<P>)> = Vec::new();
    let onto = |board: &Board, id: ItemId, p: P| match board.item(id) {
        Some(board::Item { kind: Kind::Track, shape: Shape::Capsule { a, b, .. }, .. }) => {
            let (a, b) = (mm(*a), mm(*b));
            let d = unit(a, b);
            let t = (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / dist(a, b).max(1e-12)).clamp(0.0, 1.0);
            Some([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t])
        }
        _ => None,
    };
    for path in &all {
        for (end, p) in [(path.from, path.pts[0]), (path.to, path.pts[path.pts.len() - 1])] {
            let End::Item(id) = end else { continue };
            let Some(q) = onto(board, id, p) else { continue };
            match places.iter_mut().find(|x| x.0 == id) {
                Some(x) if x.1.iter().all(|&o| dist(o, q) > AT_END) => x.1.push(q),
                Some(_) => {}
                None => places.push((id, vec![q])),
            }
        }
    }
    for (id, at) in places {
        if at.len() == 1 {
            middles.insert(id, at[0]);
            continue;
        }
        let Some(item) = board.remove(id) else { continue };
        let Shape::Capsule { a, b, r } = item.shape else { continue };
        let (start, d, len) = (mm(a), unit(mm(a), mm(b)), dist(mm(a), mm(b)));
        let along = |p: P| (p[0] - start[0]) * d[0] + (p[1] - start[1]) * d[1];
        let mut at = at;
        at.sort_by(|x, y| along(*x).total_cmp(&along(*y)));
        let mut from = 0.0;
        for k in 0..at.len() {
            let to = if k + 1 < at.len() { (along(at[k]) + along(at[k + 1])) / 2.0 } else { len };
            let point = |t: f64| nm([start[0] + d[0] * t, start[1] + d[1] * t]);
            let part = board.insert(board::Item { shape: Shape::Capsule { a: if k == 0 { a } else { point(from) }, b: if k + 1 < at.len() { point(to) } else { b }, r }, span: None, ..item.clone() });
            middles.insert(part, at[k]);
            // The paths that ended on the whole track at this place end on this part of it.
            for path in all.iter_mut() {
                let (first, last) = (path.pts[0], path.pts[path.pts.len() - 1]);
                for (end, p) in [(&mut path.from, first), (&mut path.to, last)] {
                    if *end == End::Item(id) && (along(p).clamp(0.0, len) - along(at[k])).abs() <= AT_END {
                        *end = End::Item(part);
                    }
                }
            }
            from = to;
        }
    }
    let board: &Board = board;
    let pads = |net: u32, layer: Option<usize>| board.items().filter(move |(_, i)| matches!(i.kind, Kind::Pad { .. } | Kind::Via) && i.net == net && layer.is_none_or(|l| i.on(l)));
    let mut plan = crate::start(board);
    let Some(spots) = plan.sheets.first().map(|s| s.space.spots.clone()) else { return Ok(Read { plan, from: Vec::new() }) };
    let mut spots = (*spots).clone();
    spots.holes = holes;
    spots.middles.extend(middles);
    let spots = std::sync::Arc::new(spots);
    // Each path a connection of its own: from the pad it starts on to the pad it ends on,
    // or, where it ends in a via, a pad of the net.
    let pad_of = |end: End| match end {
        End::Item(id) => Some(id),
        End::Site(_) => None,
    };
    let mut wants: Vec<Want> = Vec::new();
    let mut from_tracks: Vec<Vec<ItemId>> = Vec::new();
    let mut sheets: Vec<Sheet> = Vec::new();
    for layer in 0..board.layers.len() {
        let space = Space::new(board, layer, vias, &spots);
        let sides = carry::sides(&space);
        // The sides of each thing a strand may end on: an edge with it on one side and free
        // room on the other.
        let mut rim: HashMap<usize, Vec<usize>> = HashMap::new();
        for (e, edge) in space.edges.iter().enumerate() {
            let [Some(a), Some(b)] = edge.t else { continue };
            for (x, y) in [(a, b), (b, a)] {
                if let Some(o) = space.tris[x].inside
                    && space.free(y)
                {
                    rim.entry(o).or_default().push(e);
                }
            }
        }
        let mut lines: Vec<(Strand, Vec<(usize, f64)>)> = Vec::new();
        for path in all.iter().filter(|p| p.layer == layer) {
            let Some(any) = pad_of(path.from).or(pad_of(path.to)).or_else(|| pads(path.net, None).next().map(|x| x.0)) else {
                not(&path.of, path.net, "it has no pad");
                continue;
            };
            let Some((from, start, pts)) = anchored(board, &space, &rim, path.from, path.pts.clone()) else {
                not(&path.of, path.net, &format!("a track of it does not leave what it starts on, at {:.2}, {:.2} on {}", path.pts[0][0], path.pts[0][1], board.layers[layer]));
                continue;
            };
            let back: Vec<P> = pts.into_iter().rev().collect();
            let Some((to, end, back)) = anchored(board, &space, &rim, path.to, back) else {
                let p = path.pts[path.pts.len() - 1];
                not(&path.of, path.net, &format!("a track of it does not reach what it ends on, at {:.2}, {:.2} on {}", p[0], p[1], board.layers[layer]));
                continue;
            };
            // (Along tracks that stay, back onto what it left: it joins nothing more.)
            if from == to {
                continue;
            }
            let mut pts: Vec<P> = back.into_iter().rev().collect();
            let own: Vec<P> = vias.iter().filter(|v| v.1 == path.net).map(|v| v.0).collect();
            round_vias(&mut pts, &own);
            let class = crate::width(board, path.net);
            let width = path.width.min(class).max(crate::least_width(board, path.net));
            let strand = Strand { net: path.net, connection: wants.len(), from, to, width, crossings: Vec::new() };
            let crossed = match carry::read_or(&space, &sides, &Trace { strand: strand.clone(), pts: pts.clone(), start, end }) {
                Ok(crossed) => crossed,
                Err(into) => {
                    let what = match into.map(|o| &space.obstacles[o]) {
                        None => format!("its line cannot be followed from {:.2}, {:.2} to {:.2}, {:.2} on {}", pts[0][0], pts[0][1], pts[pts.len() - 1][0], pts[pts.len() - 1][1], board.layers[layer]),
                        Some(o) => {
                            let kind = match o.item.and_then(|id| board.item(id)).map(|i| &i.kind) {
                                Some(Kind::Pad { .. }) => "a pad",
                                Some(Kind::Via) => "a via",
                                Some(Kind::Track) => "a track",
                                Some(Kind::Spoke { .. }) => "the room kept for a pad's spoke to its pour",
                                Some(Kind::Keepout { .. }) => "a rule area",
                                Some(Kind::Pour { .. }) => "a pour that stays",
                                None if o.what == crate::space::What::Pad => "a via of the plan",
                                None => "something with no item (the board's edge, a hole or an opening in the mask)",
                            };
                            format!("it passes over {kind}{} at {:.2}, {:.2} on {}", if o.net == path.net { " of its own net" } else { "" }, o.centre[0], o.centre[1], board.layers[layer])
                        }
                    };
                    not(&path.of, path.net, &format!("a track of it cannot be read: {what}"));
                    continue;
                }
            };
            wants.push(Want {
                net: path.net,
                from: pad_of(from).unwrap_or(any),
                to: pad_of(to).unwrap_or(any),
                apart: dist(pts[0], pts[pts.len() - 1]),
                into: Vec::new(),
                avoid: Vec::new(),
                stub: false,
                narrow: (width < class - 1e-9).then_some(width),
            });
            from_tracks.push(path.of.clone());
            lines.push((strand, crossed));
        }
        let mut sheet = carry::filled(space.clone(), lines.clone());
        // Sound as far as that no two strands cross. Whether each gap has room for what was
        // read through it is not asked: a gap's room is the plan's estimate, the copper read
        // lay there and passed its checker, and every wire drawn is put to ours.
        let sound = |s: &Sheet| (0..s.space.tris.len()).all(|t| !s.space.free(t) || s.paired(t));
        if !sound(&sheet) {
            // Net by net: one whose strands the sheet does not hold with those before it is
            // left out.
            let mut kept: Vec<(Strand, Vec<(usize, f64)>)> = Vec::new();
            for &net in &nets {
                let mut more = kept.clone();
                more.extend(lines.iter().filter(|l| l.0.net == net).cloned());
                if more.len() == kept.len() {
                    continue;
                }
                if sound(&carry::filled(space.clone(), more.clone())) {
                    kept = more;
                } else {
                    let items: Vec<ItemId> = lines.iter().filter(|l| l.0.net == net).flat_map(|l| from_tracks[l.0.connection].iter().copied()).collect();
                    not(&items, net, &format!("its tracks on {} cross another's as the plan reads them", board.layers[layer]));
                }
            }
            sheet = carry::filled(space, kept);
        }
        sheet.places(board);
        sheets.push(sheet);
    }
    let unread = unread.into_inner();
    if !unread.is_empty() {
        return Err(unread);
    }
    plan.vias = crate::vias_of(&sheets);
    plan.sheets = sheets;
    plan.wants = wants;
    Ok(Read { plan, from: from_tracks })
}
