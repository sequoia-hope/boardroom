//! Copper as a drawing of the plan (docs/one-system-spec.md): each strand of [`plan::Plan`] as
//! the track with fewest bends, in 0/45/90, that crosses the strand's gaps in their order and
//! keeps its side of every obstacle and of the strands beside it. A prototype.
//!
//! Wires are drawn one at a time. In each gap a wire keeps, from the corner on each side, the
//! room of the strands between it and that corner that are not drawn yet; from one that is
//! drawn it keeps its distance from the copper itself. A strand with no such path is left
//! undrawn and reported: nothing is mended afterwards.

pub mod convex;
pub mod draw;
pub mod path;

use board::{Board, Item, ItemId, Kind, Shape};
use plan::space::{What, P};
use plan::{End, Plan, Sheet};
use serde::Serialize;

use convex::{add, cross, dot, hull, left, len, scale, sub};
use path::{Con, Course, Gap, Half};

/// A strand as copper: one track through `pts`.
#[derive(Debug, Clone, Serialize)]
pub struct Wire {
    pub layer: usize,
    pub strand: usize,
    pub net: u32,
    /// mm.
    pub width: f64,
    pub pts: Vec<P>,
}

/// A strand the drawing found no path for.
#[derive(Debug, Clone, Serialize)]
pub struct Undrawn {
    pub layer: usize,
    pub strand: usize,
    pub net: u32,
    pub why: String,
    /// The path the checker refused, when one was found.
    pub refused: Vec<P>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Drawing {
    pub wires: Vec<Wire>,
    pub undrawn: Vec<Undrawn>,
}

/// A drawn strand as the wires after it see it.
struct Done {
    /// The piece between each two of the gaps it was drawn through (see [`path::Path::stages`]).
    stages: Vec<Vec<P>>,
    /// The triangle after each of its crossings but the last.
    tris: Vec<Option<usize>>,
    /// The crossings it was drawn through: those after `lead` up to `last`. The ones before and
    /// after are round its own ends, where it is held to no side of its own pad's corners.
    lead: usize,
    last: usize,
}

/// What is next to a wire on one side of a gap.
enum Beside {
    /// The gap's corner, with the room the undrawn strands between need.
    Corner { v: usize, between: f64, net: u32 },
    /// A drawn strand, at its crossing `k` of the gap, and the distance to keep between the two
    /// centre lines.
    Wire { strand: usize, k: usize, r: f64 },
}

fn nm(p: P) -> board::Point {
    board::Point::new((p[0] * 1e6).round() as i64, (p[1] * 1e6).round() as i64)
}

fn clear(board: &Board, a: u32, b: u32) -> f64 {
    if a == b { 0.0 } else { board.clearance(a, b) as f64 / 1e6 }
}

/// The triangle two edges are both sides of.
fn shared(sheet: &Sheet, e: usize, f: usize) -> Option<usize> {
    sheet.space.edges[e].t.iter().flatten().copied().find(|t| sheet.space.edges[f].t.contains(&Some(*t)))
}

fn corners(sheet: &Sheet, t: usize) -> Vec<P> {
    sheet.space.tris[t].v.iter().map(|&v| sheet.space.verts[v]).collect()
}

/// Whether a corner is an obstacle to copper of `net`: not a via site no via took, not copper
/// of the net itself.
fn solid(sheet: &Sheet, v: usize, net: u32) -> bool {
    sheet.space.vert_of[v].is_some_and(|o| {
        let o = &sheet.space.obstacles[o];
        match o.what {
            What::Site => false,
            What::Pad | What::Copper => !(o.net == net && net != 0),
            _ => true,
        }
    })
}

/// Whether an edge is a wall on the far side from triangle `t`: no strand may be beyond it.
fn wall(sheet: &Sheet, e: usize, t: usize) -> bool {
    !sheet.space.edges[e].t.iter().flatten().any(|&x| x != t && sheet.space.free(x))
}

struct Drawer<'a> {
    board: &'a Board,
    sheet: &'a Sheet,
    done: Vec<Option<Done>>,
}

impl Drawer<'_> {
    /// What is beside strand `s` in gap `e`, towards the gap's first corner or its second.
    fn beside(&self, s: usize, k: usize, e: usize, first: bool) -> Beside {
        let sheet = self.sheet;
        let on = &sheet.on[e];
        let at = on.iter().position(|&x| x == (s, k)).unwrap();
        let me = &sheet.strands[s];
        let (mut between, mut net) = (0.0, me.net);
        let order: Vec<usize> = if first { (0..at).rev().collect() } else { (at + 1..on.len()).collect() };
        for i in order {
            let (o, ok) = on[i];
            let other = &sheet.strands[o];
            if let Some(d) = &self.done[o] {
                // By its own end it was drawn free of this gap: the corner beyond is what counts.
                if ok <= d.lead || ok > d.last {
                    continue;
                }
                let r = if other.net == me.net && between == 0.0 { 0.0 } else { me.width / 2.0 + between + clear(self.board, net, other.net) + other.width / 2.0 };
                return Beside::Wire { strand: o, k: ok, r };
            }
            between += clear(self.board, net, other.net) + other.width;
            net = other.net;
        }
        Beside::Corner { v: sheet.space.edges[e].v[if first { 0 } else { 1 }], between, net }
    }

    /// How far the middle of strand `s` keeps from a corner with `between` of undrawn strands
    /// before it, the last of them of `net`.
    fn kept_from(&self, s: usize, v: usize, between: f64, net: u32) -> f64 {
        let me = &self.sheet.strands[s];
        if solid(self.sheet, v, net) {
            me.width / 2.0 + between + self.sheet.space.keep(self.board, v, net, None)
        } else if between > 0.0 {
            me.width / 2.0 + between
        } else {
            0.0
        }
    }

    /// The pieces of drawn strand `o` in triangle `t`, next to its crossing `k`: the points,
    /// and whether they begin at that crossing (or end at it).
    fn pieces(&self, o: usize, k: usize, t: usize) -> Option<(&[P], bool)> {
        let d = self.done[o].as_ref()?;
        if k <= d.lead || k > d.last {
            return None;
        }
        // Crossing k ends one piece and begins the next.
        if d.tris[k] == Some(t) {
            Some((&d.stages[k - d.lead], true))
        } else if d.tris[k - 1] == Some(t) {
            Some((&d.stages[k - d.lead - 1], false))
        } else {
            None
        }
    }

    /// What a line of strand `s` through gap `e` (its crossing `k`) keeps its distance from.
    /// `own` is the obstacle the wire ends on, when this gap is beside it: its corner is none.
    fn cons(&self, s: usize, k: usize, e: usize, forward: P, own: Option<usize>) -> Vec<Con> {
        let sheet = self.sheet;
        let edge = &sheet.space.edges[e];
        let ends = edge.v.map(|v| sheet.space.verts[v]);
        let mid = scale(add(ends[0], ends[1]), 0.5);
        let mut cons = Vec::new();
        for first in [true, false] {
            let (near, far) = if first { (ends[0], ends[1]) } else { (ends[1], ends[0]) };
            if own.is_some() && sheet.space.vert_of[edge.v[if first { 0 } else { 1 }]] == own {
                continue;
            }
            let side = dot(left(forward), sub(near, mid)) > 0.0;
            let along = scale(sub(far, near), 1.0 / edge.len.max(1e-12));
            let toward = scale(along, -1.0);
            match self.beside(s, k, e, first) {
                Beside::Corner { v, between, net } => {
                    let r = self.kept_from(s, v, between, net);
                    cons.push(Con { p: near, r, left: side, toward });
                    // The wire crosses the gap itself no nearer the corner than that.
                    if own.is_none() && r > 0.0 {
                        cons.push(Con { p: add(near, scale(along, r.min(edge.len * 0.999))), r: 0.0, left: side, toward });
                    }
                    if !solid(sheet, v, net) {
                        continue;
                    }
                    // An obstacle's side that leaves this corner at a sharp angle to the gap is
                    // nearer than the corner to a wire crossing a little way along.
                    for t in edge.t.iter().flatten() {
                        for x in sheet.space.tris[*t].e {
                            let other = &sheet.space.edges[x];
                            if x == e || !other.v.contains(&v) || !wall(sheet, x, *t) {
                                continue;
                            }
                            let to = sheet.space.verts[if other.v[0] == v { other.v[1] } else { other.v[0] }];
                            let w = scale(sub(to, near), 1.0 / other.len.max(1e-12));
                            let (c, sn) = (dot(along, w), cross(along, w).abs());
                            if c > 0.0 && sn > 1e-9 {
                                // Only as far as the side reaches: past its end the nearest of
                                // it is its far corner, which the next gap keeps clear of.
                                let d = (r / sn).min(other.len / c).min(edge.len * 0.999);
                                if own.is_none() {
                                    cons.push(Con { p: add(near, scale(along, d)), r: 0.0, left: side, toward });
                                }
                            }
                        }
                    }
                }
                Beside::Wire { strand, k: ok, r } => {
                    for t in edge.t.iter().flatten() {
                        let Some((pts, begins)) = self.pieces(strand, ok, *t) else { continue };
                        let (x, next) = if begins { (pts[0], pts[1]) } else { (pts[pts.len() - 1], pts[pts.len() - 2]) };
                        cons.push(Con { p: x, r, left: side, toward });
                        for &p in &pts[1..pts.len() - 1] {
                            cons.push(Con { p, r, left: side, toward });
                        }
                        // The wire crosses the gap itself no nearer the other than that: where
                        // the other leaves the gap at a slant its side is nearer than its
                        // crossing point, but only as far as that piece of it runs.
                        if r > 0.0 && own.is_none() {
                            let g = sub(next, x);
                            let l = len(g).max(1e-12);
                            let (cs, sn) = (dot(g, along) / l, (cross(g, along) / l).abs());
                            let d = if cs > 1e-9 && sn > 1e-9 { (r / sn).min(l / cs).max(r) } else { r };
                            cons.push(Con { p: add(x, scale(along, d.min(edge.len))), r: 0.0, left: side, toward });
                        }
                    }
                }
            }
        }
        cons
    }

    /// Where a bend of strand `s` in triangle `t` may be, as far as gap `e` (its crossing `k`)
    /// says: on its own side of each drawn strand beside it there, and off the triangle's wall.
    fn halves(&self, s: usize, k: usize, e: usize, t: usize, out: &mut Vec<Half>) {
        let sheet = self.sheet;
        let edge = &sheet.space.edges[e];
        let me = &sheet.strands[s];
        for first in [true, false] {
            // The gap's corner on this wire's own side of what is beside it.
            let mine = sheet.space.verts[edge.v[if first { 1 } else { 0 }]];
            match self.beside(s, k, e, first) {
                Beside::Wire { strand, k: ok, r } => {
                    let Some((pts, begins)) = self.pieces(strand, ok, t) else { continue };
                    let (a, b) = if begins { (pts[0], pts[1]) } else { (pts[pts.len() - 2], pts[pts.len() - 1]) };
                    let sign = if cross(sub(b, a), sub(mine, a)) >= 0.0 { 1.0 } else { -1.0 };
                    for w in pts.windows(2) {
                        let l = len(sub(w[1], w[0]));
                        if l < 1e-9 {
                            continue;
                        }
                        // sign * nl·(p - a) >= r
                        let nl = scale(left(sub(w[1], w[0])), 1.0 / l);
                        let n = scale(nl, -sign);
                        out.push(Half { n, c: dot(n, w[0]) - r });
                    }
                }
                Beside::Corner { v, between, net } => {
                    // The triangle's third side, where it is a wall and this corner is on it.
                    for x in sheet.space.tris[t].e {
                        let other = &sheet.space.edges[x];
                        if x == e || !other.v.contains(&v) || !wall(sheet, x, t) || !solid(sheet, v, net) {
                            continue;
                        }
                        let (p, q) = (sheet.space.verts[other.v[0]], sheet.space.verts[other.v[1]]);
                        let nl = scale(left(sub(q, p)), 1.0 / other.len.max(1e-12));
                        let sign = if dot(nl, sub(mine, p)) >= 0.0 { 1.0 } else { -1.0 };
                        let r = me.width / 2.0 + between + sheet.space.keep(self.board, v, net, None);
                        let n = scale(nl, -sign);
                        out.push(Half { n, c: dot(n, p) - r });
                    }
                }
            }
        }
    }

    /// The course of strand `s`: its ends, the gaps between, and where it may be and bend.
    fn course(&self, s: usize) -> Option<(Course, Done)> {
        let sheet = self.sheet;
        let strand = &sheet.strands[s];
        let c = &strand.crossings;
        let end = |e: End| sheet.space.ends.get(&e).copied();
        let (from, to) = (end(strand.from)?, end(strand.to)?);
        // The gaps between the ends' own sides, and the triangle after each crossing.
        let gaps = c.len().saturating_sub(2);
        let mut tris: Vec<Option<usize>> = (0..c.len().saturating_sub(1)).map(|i| shared(sheet, c[i], c[i + 1])).collect();
        if tris.is_empty() {
            tris.push(None);
        }
        // A gap with a corner on the pad the wire ends on is not one the wire is held to: which
        // side of its own pad's corner it passes is all the same. Those round each end go, and
        // the wire is free in the triangles they joined.
        let beside = |k: usize, o: usize| sheet.space.edges[c[k]].v.iter().any(|&v| sheet.space.vert_of[v] == Some(o));
        let lead = (1..=gaps).take_while(|&k| beside(k, from)).count();
        let trail = (lead + 1..=gaps).rev().take_while(|&k| beside(k, to)).count();
        let last = gaps - trail;
        let n = last - lead + 1;
        let stage = |m: usize| m.saturating_sub(lead).min(n - 1);
        // Where the wire may be between two gaps: the triangle between them, and the free one
        // beyond its third side, since a wire that leaves by a side and comes back by it has
        // gone round nothing. By its own pad, all the triangles that touch the pad.
        let mut regions: Vec<Vec<P>> = vec![Vec::new(); n];
        for (m, t) in tris.iter().enumerate() {
            let Some(t) = *t else { continue };
            regions[stage(m)].extend(corners(sheet, t));
            for x in sheet.space.tris[t].e {
                if c.get(m) == Some(&x) || c.get(m + 1) == Some(&x) {
                    continue;
                }
                if let Some(beyond) = sheet.space.edges[x].t.iter().flatten().copied().find(|&y| y != t && sheet.space.free(y)) {
                    regions[stage(m)].extend(corners(sheet, beyond));
                }
            }
        }
        for (o, at) in [(from, 0), (to, n - 1)] {
            for (t, tri) in sheet.space.tris.iter().enumerate() {
                if sheet.space.free(t) && tri.v.iter().any(|&v| sheet.space.vert_of[v] == Some(o)) {
                    regions[at].extend(corners(sheet, t));
                }
            }
        }
        regions[0].extend(sheet.space.obstacles[from].poly.iter().copied());
        regions[n - 1].extend(sheet.space.obstacles[to].poly.iter().copied());
        let mut regions: Vec<Vec<P>> = regions.iter().map(|r| hull(r)).collect();
        // Never past the line of the gap before, nor of the gap after.
        for k in lead + 1..=last {
            let edge = &sheet.space.edges[c[k]];
            let ends = edge.v.map(|v| sheet.space.verts[v]);
            let Some(into) = tris[k] else { continue };
            let Some(apex) = sheet.space.tris[into].v.iter().copied().find(|v| !edge.v.contains(v)) else { continue };
            let mut forward = left(sub(ends[1], ends[0]));
            if dot(forward, sub(sheet.space.verts[apex], ends[0])) < 0.0 {
                forward = scale(forward, -1.0);
            }
            let g = k - lead;
            regions[g - 1] = convex::clip(&regions[g - 1], forward, dot(forward, ends[0]));
            regions[g] = convex::clip(&regions[g], scale(forward, -1.0), -dot(forward, ends[0]));
        }
        let (a, b) = (sheet.space.obstacles[from].centre, sheet.space.obstacles[to].centre);
        let mut course = Course { a, b, regions, gaps: Vec::new(), halves: vec![Vec::new(); n], starts: Vec::new(), ends: Vec::new(), least: 2.0 * strand.width, frame: 0.0 };
        for k in 1..=gaps {
            let e = c[k];
            let edge = &sheet.space.edges[e];
            let ends = edge.v.map(|v| sheet.space.verts[v]);
            // Across the gap, into the triangle after it.
            let into = tris[k]?;
            let apex = sheet.space.tris[into].v.iter().copied().find(|v| !edge.v.contains(v))?;
            let mut forward = left(sub(ends[1], ends[0]));
            if dot(forward, sub(sheet.space.verts[apex], ends[0])) < 0.0 {
                forward = scale(forward, -1.0);
            }
            let forward = scale(forward, 1.0 / len(forward).max(1e-12));
            if k <= lead {
                course.starts.extend(self.cons(s, k, e, forward, Some(from)));
            } else if k > last {
                course.ends.extend(self.cons(s, k, e, forward, Some(to)));
            } else {
                course.gaps.push(Gap { ends, forward, cons: self.cons(s, k, e, forward, None) });
            }
            for (m, t) in [(k - 1, tris[k - 1]), (k, tris[k])] {
                if let Some(t) = t {
                    self.halves(s, k, e, t, &mut course.halves[stage(m)]);
                }
            }
        }
        Some((course, Done { stages: Vec::new(), tris, lead, last }))
    }
}

/// Draws every strand of the plan, layer by layer, in the order the plan laid them.
pub fn draw(board: &Board, plan: &Plan) -> Drawing {
    draw_watching(board, plan, None).0
}

/// [`draw`], and a picture of the course of one strand (a layer and a strand's number) as it
/// was when its turn came: its gaps in order, what each holds it from, and how far it got.
pub fn draw_watching(board: &Board, plan: &Plan, watch: Option<(usize, usize)>) -> (Drawing, Option<String>) {
    let mut drawing = Drawing::default();
    let mut watched = None;
    // The board as it fills: every wire is put to the exact checker as it is drawn, and one it
    // faults is not kept.
    let mut work = board.clone();
    let (r, drill) = plan::via_size(board);
    let all = (1u64 << board.layers.len()) - 1;
    for v in &plan.vias {
        let c = nm(v.at);
        work.insert(Item { kind: Kind::Via, net: v.net, layers: all, shape: Shape::Capsule { a: c, b: c, r }, hole: Some((c, drill)), locked: false, span: None });
    }
    for (layer, sheet) in plan.sheets.iter().enumerate() {
        let mut drawer = Drawer { board, sheet, done: (0..sheet.strands.len()).map(|_| None).collect() };
        for s in sheet.laid() {
            let strand = &sheet.strands[s];
            let Some((course, mut done)) = drawer.course(s) else {
                drawing.undrawn.push(Undrawn { layer, strand: s, net: strand.net, why: "its gaps do not follow one another".to_string(), refused: Vec::new() });
                continue;
            };
            let found = path::fewest_bends(&course);
            if watch == Some((layer, s)) {
                watched = Some(draw::course(sheet, &course, &found));
            }
            match found {
                Ok(path) => {
                    let ids: Vec<ItemId> = path
                        .pts
                        .windows(2)
                        .map(|seg| {
                            let shape = Shape::Capsule { a: nm(seg[0]), b: nm(seg[1]), r: (strand.width * 1e6 / 2.0).round() as i64 };
                            work.insert(Item { kind: Kind::Track, net: strand.net, layers: 1 << layer, shape, hole: None, locked: false, span: None })
                        })
                        .collect();
                    let fault = ids.iter().flat_map(|&id| board::check_item(&work, id)).next();
                    if let Some(fault) = fault {
                        for id in ids {
                            work.remove(id);
                        }
                        drawing.undrawn.push(Undrawn { layer, strand: s, net: strand.net, why: format!("the path found breaks a rule ({:?})", fault.rule), refused: path.pts });
                        continue;
                    }
                    drawing.wires.push(Wire { layer, strand: s, net: strand.net, width: strand.width, pts: path.pts });
                    done.stages = path.stages;
                    drawer.done[s] = Some(done);
                }
                Err(stuck) => {
                    let reached = stuck.reached;
                    let why = if reached == course.gaps.len() { "no way to end on the pad".to_string() } else { format!("no path in 0/45/90 past gap {} of {}", reached + 1, course.gaps.len()) };
                    drawing.undrawn.push(Undrawn { layer, strand: s, net: strand.net, why, refused: Vec::new() });
                }
            }
        }
    }
    (drawing, watched)
}

/// The board with the drawing's copper on it: the wires as tracks and the plan's vias, and
/// which items those are.
pub fn onto(board: &Board, plan: &Plan, drawing: &Drawing) -> (Board, Vec<ItemId>) {
    let mut out = board.clone();
    let mut new = Vec::new();
    let at = |p: P| board::Point::new((p[0] * 1e6).round() as i64, (p[1] * 1e6).round() as i64);
    for w in &drawing.wires {
        for seg in w.pts.windows(2) {
            let shape = Shape::Capsule { a: at(seg[0]), b: at(seg[1]), r: (w.width * 1e6 / 2.0).round() as i64 };
            new.push(out.insert(Item { kind: Kind::Track, net: w.net, layers: 1 << w.layer, shape, hole: None, locked: false, span: None }));
        }
    }
    let (r, drill) = plan::via_size(board);
    let all = (1u64 << board.layers.len()) - 1;
    for v in &plan.vias {
        let c = at(v.at);
        new.push(out.insert(Item { kind: Kind::Via, net: v.net, layers: all, shape: Shape::Capsule { a: c, b: c, r }, hole: Some((c, drill)), locked: false, span: None }));
    }
    (out, new)
}

/// The board file's text with its unlocked tracks and vias taken out and the drawing put in.
pub fn write(text: &str, original: &Board, plan: &Plan, drawing: &Drawing) -> String {
    let mut edit = kicad::write::Edit { uuid_seed: Some(1), ..Default::default() };
    edit.remove = original.items().filter(|(_, i)| matches!(i.kind, Kind::Track | Kind::Via) && !i.locked).filter_map(|(_, i)| i.span).collect();
    let m = |p: P| [p[0] / 1e3, p[1] / 1e3];
    for w in &drawing.wires {
        for seg in w.pts.windows(2) {
            edit.tracks.push(kicad::write::NewTrack { start: m(seg[0]), end: m(seg[1]), width: w.width / 1e3, layer: original.layers[w.layer].clone(), net: original.file_net(w.net), locked: false });
        }
    }
    let (r, drill) = plan::via_size(original);
    let (Some(top), Some(bottom)) = (original.layers.first(), original.layers.last()) else { return kicad::write::apply(text, &edit) };
    for v in &plan.vias {
        edit.vias.push(kicad::write::NewVia { at: m(v.at), size: 2.0 * r as f64 / 1e9, drill: 2.0 * drill as f64 / 1e9, layers: [top.clone(), bottom.clone()], net: original.file_net(v.net), locked: false });
    }
    kicad::write::apply(text, &edit)
}
