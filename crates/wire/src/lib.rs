//! Copper as a drawing of the plan (docs/one-system-spec.md): each strand of [`plan::Plan`] as
//! the track with fewest bends, in 0/45/90, that passes every corner beside the strand's gaps
//! on the side the plan passes it. A prototype.
//!
//! Each wire is drawn alone, from the plan and nothing else. From every corner it keeps the
//! corner's own distance and the room of the strands the plan has between the two, whether
//! those are drawn yet or not, and each of its pieces lies as near some corner as that allows
//! ([`chain`]). Each wire is then put to the exact checker; one it faults is not kept, and its
//! connection is laid again another way ([`route`]). Where the first path found for a wire is
//! faulted only against wires drawn before it, those are drawn again round it, if they can
//! be ([`give_way`]).
//!
//! A net with a pour is left to its pour: [`route`] plans and draws the other nets, fills the
//! pours round them ([`board::Pours`]) and plans only what the pours leave apart, and again
//! what they leave apart once that is drawn: a connection that ended at a via in pour which
//! later wires cut off is laid again, the second time all the way to the pad. A pad the
//! wires leave fewer thermal spokes than the rules ask, because they shut in the pour a spoke
//! of it ends in ([`board::Pours::starved`]), is given a spoke drawn by hand ([`Tie`]), or
//! those wires are laid another way ([`shutting`]).
//!
//! Last of all, copper a net has no need of is taken out ([`spare`]): a wire is drawn from pad
//! to pad knowing nothing of the copper its net has already, and where it joins that copper a
//! second time one of the two ways round is spare.

pub mod chain;
pub mod convex;
pub mod draw;
pub mod loose;
pub mod spare;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};

use board::{Board, Item, ItemId, Kind, Shape};
use plan::space::{What, P};
use plan::{End, Plan, Sheet};
use serde::Serialize;

use chain::{Corner, Course, Gap, Stage, Wall};
use convex::{dot, left, sub};

/// A strand as copper: one track through `pts`.
#[derive(Debug, Clone, Serialize)]
pub struct Wire {
    pub layer: usize,
    pub strand: usize,
    pub net: u32,
    /// mm.
    pub width: f64,
    pub pts: Vec<P>,
    /// Drawn as the strand lies in the plan, at whatever angles that is, because no path in
    /// 0/45/90 was found for it.
    pub as_planned: bool,
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
    /// The gap of its course where it went wrong, by its two corners: the one the search for
    /// a path did not get across, or the one nearest what the checker faulted it against.
    pub gap: Option<[P; 2]>,
}

/// A track from a pad's middle straight across the gap its pour leaves round it, into the
/// pour: a spoke drawn by hand, where the pour's own leave the pad too few.
#[derive(Debug, Clone, Serialize)]
pub struct Tie {
    pub layer: usize,
    pub net: u32,
    /// mm.
    pub width: f64,
    pub pts: [P; 2],
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Drawing {
    pub wires: Vec<Wire>,
    pub undrawn: Vec<Undrawn>,
    pub ties: Vec<Tie>,
    /// Lengths of wire taken out again: each was one of two ways between the same two places
    /// of its net's copper, and the other is kept ([`spare`]).
    pub spare: Vec<Wire>,
}

fn nm(p: P) -> board::Point {
    board::Point::new((p[0] * 1e6).round() as i64, (p[1] * 1e6).round() as i64)
}

/// The triangle two edges are both sides of.
fn shared(sheet: &Sheet, e: usize, f: usize) -> Option<usize> {
    sheet.space.edges[e].t.iter().flatten().copied().find(|t| sheet.space.edges[f].t.contains(&Some(*t)))
}

/// The course of strand `s`: its ends, the gaps it crosses with where it may cross each, the
/// corners beside them with how near it may come to each, and the sides of obstacles it runs
/// along.
pub fn course(board: &Board, sheet: &Sheet, s: usize) -> Option<Course> {
    let space = &sheet.space;
    let strand = &sheet.strands[s];
    let c = &strand.crossings;
    let end = |e: End| space.ends.get(&e).copied();
    let (from, to) = (end(strand.from)?, end(strand.to)?);
    let own = |v: usize| space.vert_of[v] == Some(from) || space.vert_of[v] == Some(to);
    let same = |v: usize| space.vert_of[v].is_some_and(|o| space.obstacles[o].net == strand.net && strand.net != 0);
    let tris: Vec<Option<usize>> = (0..c.len().saturating_sub(1)).map(|i| shared(sheet, c[i], c[i + 1])).collect();
    // A gap with a corner on the pad the wire ends on is not one the wire is held to: which
    // side of its own pad's corner it passes is all the same. Those round each end go; the
    // gaps between are crossed in order.
    let gaps = c.len().saturating_sub(2);
    let beside = |k: usize, o: usize| space.edges[c[k]].v.iter().any(|&v| space.vert_of[v] == Some(o));
    let lead = (1..=gaps).take_while(|&k| beside(k, from)).count();
    let trail = (lead + 1..=gaps).rev().take_while(|&k| beside(k, to)).count();
    let last = gaps - trail;
    // The stage a wire is in once it is past crossing `k` of the strand.
    let stage = |k: usize| k.clamp(lead, last) - lead;
    let places = sheet.places(board);
    let mut stages: Vec<Stage> = vec![Stage::default(); last - lead + 1];
    let mut held: Vec<Gap> = Vec::new();
    let mut corners: HashMap<usize, Corner> = HashMap::new();
    let mut holds: HashMap<usize, Corner> = HashMap::new();
    let mut fronts: Vec<chain::Front> = Vec::new();
    for k in 1..=gaps {
        let edge = &space.edges[c[k]];
        let ends = edge.v.map(|v| space.verts[v]);
        let (lo, hi) = sheet.room(board, s, k)?;
        // Across the gap, into the triangle after it.
        let apex = space.tris[tris[k]?].v.iter().copied().find(|v| !edge.v.contains(v))?;
        let mut forward = left(sub(ends[1], ends[0]));
        if dot(forward, sub(space.verts[apex], ends[0])) < 0.0 {
            forward = [-forward[0], -forward[1]];
        }
        if k > lead && k <= last {
            held.push(Gap { ends, lo, hi, forward, mid: sheet.point(c[k], places[s][k]) });
        }
        for (i, r) in [(0, lo * edge.len), (1, (1.0 - hi) * edge.len)] {
            let v = edge.v[i];
            let side = dot(left(forward), sub(ends[i], ends[1 - i])) > 0.0;
            let corner = Corner { at: ends[i], r, left: side };
            // Copper of the wire's own net is nothing to keep from: a corner of its own pad
            // only holds a line, at what the strands between leave.
            if own(v) || (same(v) && r <= strand.width / 2.0 + 1e-9) {
                holds.entry(v).or_insert(corner).r = r.max(holds.get(&v).map_or(0.0, |h| h.r));
                continue;
            }
            corners.entry(v).or_insert(corner).r = r.max(corners.get(&v).map_or(0.0, |h| h.r));
            let along = sub(ends[1 - i], ends[i]);
            fronts.push(chain::Front { at: ends[i], r, along: [along[0] / edge.len.max(1e-12), along[1] / edge.len.max(1e-12)] });
            // Held from it on both sides of the gap.
            for m in [stage(k - 1), stage(k)] {
                stages[m].corners.push(corner);
            }
        }
    }
    // The sides of obstacles round the triangles the strand passes through. The corner just
    // beyond a triangle's third side is on the side of the wire that side's corners are on.
    let near = |v: usize| space.keep(board, v, strand.net) + strand.width / 2.0;
    for (m, t) in tris.iter().enumerate() {
        let Some(t) = *t else { continue };
        for x in space.tris[t].e {
            let edge = &space.edges[x];
            let r = |v: usize| corners.get(&v).map_or(near(v), |c| c.r);
            if !space.gap(x) {
                if !own(edge.v[0]) && !same(edge.v[0]) {
                    // The lesser of its corners' distances: each corner holds the wire to its own.
                    stages[stage(m)].walls.push(Wall { a: space.verts[edge.v[0]], b: space.verts[edge.v[1]], r: r(edge.v[0]).min(r(edge.v[1])) });
                }
                continue;
            }
            if x == c[m] || x == c[m + 1] {
                continue;
            }
            let Some(beyond) = edge.t.iter().flatten().copied().find(|&y| y != t && space.free(y)) else { continue };
            let Some(apex) = space.tris[beyond].v.iter().copied().find(|v| !edge.v.contains(v)) else { continue };
            let sides: Vec<bool> = edge.v.iter().filter_map(|v| corners.get(v).map(|c| c.left)).collect();
            if let Some(&side) = sides.first()
                && sides.iter().all(|&x| x == side)
                && !own(apex)
                && !same(apex)
                && !corners.contains_key(&apex)
            {
                stages[stage(m)].corners.push(Corner { at: space.verts[apex], r: near(apex), left: side });
            }
        }
    }
    let mut walls = Vec::new();
    for o in space.obstacles.iter().filter(|o| o.what == What::Edge) {
        // A little over: a round cut-out is in the outline as chords, each a hair inside the
        // curve the checker measures from.
        let r = board.rules.min_copper_edge_clearance * 1e3 + strand.width / 2.0 + 0.02;
        walls.extend((0..o.poly.len()).map(|i| Wall { a: o.poly[i], b: o.poly[(i + 1) % o.poly.len()], r }));
    }
    // A bend may be anywhere in the box round the course, and a little way out of it: what
    // holds a wire is the corners it passes, not the triangles its strand was planned through.
    let (a, b) = (space.obstacles[from].centre, space.obstacles[to].centre);
    let (mut lo, mut hi) = (a, a);
    for p in corners.values().map(|c| c.at).chain([b]).chain(tris.iter().flatten().flat_map(|&t| space.tris[t].v.map(|v| space.verts[v]))) {
        for i in 0..2 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    let (lo, hi) = ([lo[0] - OUT, lo[1] - OUT], [hi[0] + OUT, hi[1] + OUT]);
    let within = vec![[lo, [hi[0], lo[1]], hi], [lo, hi, [lo[0], hi[1]]]];
    // A pad at least half as long again as it is wide is left, and come to, along its length.
    // (Tried 2026-11-13: by the side the plan has the strand cross, a long side straight out.
    // Every wire of the synthetic buses then left its pad by a hook out of the pad's side.)
    let along = |o: usize| -> Vec<usize> {
        let poly = &space.obstacles[o].poly;
        if poly.len() != 4 || space.obstacles[o].reach > 0.0 {
            return Vec::new();
        }
        let side = |i: usize| sub(poly[(i + 1) % 4], poly[i]);
        let (p, q) = (side(0), side(1));
        let (lp, lq) = (dot(p, p).sqrt(), dot(q, q).sqrt());
        let long = if lp >= 1.5 * lq { p } else if lq >= 1.5 * lp { q } else { return Vec::new() };
        let turn = long[1].atan2(long[0]) / std::f64::consts::FRAC_PI_4;
        if (turn - turn.round()).abs() > 1e-6 {
            return Vec::new();
        }
        let d = turn.round().rem_euclid(8.0) as usize;
        vec![d, (d + 4) % 8]
    };
    // As far as the pad's end, before it turns.
    let stub = |o: usize| {
        let poly = &space.obstacles[o].poly;
        (0..poly.len()).map(|i| dot(sub(poly[(i + 1) % poly.len()], poly[i]), sub(poly[(i + 1) % poly.len()], poly[i])).sqrt()).fold(0.0, f64::max) / 2.0
    };
    // An end of the wire that stands in such room has it for its own.
    fronts.retain(|f| [a, b].iter().all(|&p| dot(sub(p, f.at), sub(p, f.at)).sqrt() >= f.r || dot(sub(p, f.at), f.along) < 0.0));
    Some(Course {
        a,
        b,
        gaps: held,
        stages,
        corners: Vec::new(),
        holds: {
            // In the corners' order, not a hash map's: the lines a wire may take are tried in
            // the order they are given, and two runs must draw the same wire.
            let mut holds: Vec<(usize, Corner)> = holds.into_iter().collect();
            holds.sort_by_key(|h| h.0);
            holds.into_iter().map(|h| h.1).collect()
        },
        fronts,
        beside: Vec::new(),
        walls,
        within,
        out: along(from),
        into: along(to),
        stub: [stub(from), stub(to)],
        least: strand.width / 2.0,
        frame: 0.0,
    })
}

/// Copper that is no obstacle of the plan, for the wire of strand `s` to keep `r` from: its
/// corners and sides, on the side of the wire most of it lies on as the strand lies in the
/// plan. The spoke that joins a pad to its pour is such: wires may cut some of a pad's
/// spokes, and the checker faults the one that cuts too many.
fn learn_shape(board: &Board, sheet: &Sheet, s: usize, shape: &Shape, r: f64, course: &mut Course) -> bool {
    let poly = plan::space::outline(shape);
    if poly.len() < 2 || course.walls.iter().any(|w| w.a == poly[0] && w.b == poly[1]) {
        return false;
    }
    let strand = &sheet.strands[s];
    let places = sheet.places(board);
    let line: Vec<P> = std::iter::once(course.a).chain(strand.crossings.iter().enumerate().map(|(k, &e)| sheet.point(e, places[s][k]))).chain([course.b]).collect();
    let side_of = |at: P| line.windows(2).min_by(|x, y| chain::point_segment(at, x[0], x[1]).total_cmp(&chain::point_segment(at, y[0], y[1]))).is_some_and(|w| convex::cross(sub(w[1], w[0]), sub(at, w[0])) > 0.0);
    let side = poly.iter().filter(|&&p| side_of(p)).count() * 2 > poly.len();
    course.corners.extend(poly.iter().map(|&at| Corner { at, r, left: side }));
    course.walls.extend((0..poly.len()).map(|i| Wall { a: poly[i], b: poly[(i + 1) % poly.len()], r }));
    true
}

/// How a wire already drawn lies to the wire of strand `s`: which side of the new wire it is
/// on, as the plan has the two strands in a gap they both cross (or, with no such gap, the
/// side most of it lies on), and how near the middles of the two may be: the two wires' and
/// the room of the strands the plan has between them. None for a wire of the same net.
fn neighbour(board: &Board, sheet: &Sheet, s: usize, course: &Course, other: &Wire, need: bool) -> Option<(bool, f64)> {
    let space = &sheet.space;
    let (me, it) = (&sheet.strands[s], &sheet.strands[other.strand]);
    if me.net == it.net {
        return None;
    }
    let clear = |a: u32, b: u32| if a == b { 0.0 } else { board.clearance(a, b) as f64 / 1e6 };
    // A gap both cross: which of its corners the other is towards, and what lies between.
    let shared = me.crossings.iter().enumerate().skip(1).find_map(|(k, &e)| {
        let on = &sheet.on[e];
        let mine = on.iter().position(|&x| x == (s, k))?;
        let its = on.iter().position(|&x| x.0 == other.strand)?;
        let (lo, hi) = (mine.min(its), mine.max(its));
        let apart = apart(board, sheet, &on[lo..=hi]);
        let t = shared(sheet, e, *me.crossings.get(k + 1)?)?;
        let edge = &space.edges[e];
        let ends = edge.v.map(|v| space.verts[v]);
        let apex = space.tris[t].v.iter().copied().find(|v| !edge.v.contains(v))?;
        let mut forward = left(sub(ends[1], ends[0]));
        if dot(forward, sub(space.verts[apex], ends[0])) < 0.0 {
            forward = [-forward[0], -forward[1]];
        }
        let towards = if its < mine { 0 } else { 1 };
        Some((dot(left(forward), sub(ends[towards], ends[1 - towards])) > 0.0, apart))
    });
    let (side, apart) = match shared {
        Some(found) => found,
        None if need => return None,
        None => {
            let places = sheet.places(board);
            let line: Vec<P> = std::iter::once(course.a).chain(me.crossings.iter().enumerate().map(|(k, &e)| sheet.point(e, places[s][k]))).chain([course.b]).collect();
            let side_of = |at: P| line.windows(2).min_by(|x, y| chain::point_segment(at, x[0], x[1]).total_cmp(&chain::point_segment(at, y[0], y[1]))).is_some_and(|w| convex::cross(sub(w[1], w[0]), sub(at, w[0])) > 0.0);
            (other.pts.iter().filter(|&&p| side_of(p)).count() * 2 > other.pts.len(), me.width / 2.0 + clear(me.net, it.net) + other.width / 2.0)
        }
    };
    Some((side, apart))
}

/// How far apart the middles of the first and the last of `on` are at least, where those are
/// the strands that lie side by side in a gap from the one to the other: what all of them
/// take ([`Sheet::stack`], in which two of one net beside each other lie on each other as one
/// track) less half the width of each of the two.
fn apart(board: &Board, sheet: &Sheet, on: &[(usize, usize)]) -> f64 {
    let nets: Vec<(u32, f64)> = on.iter().map(|&(s, _)| (sheet.strands[s].net, sheet.strands[s].width)).collect();
    let (Some(first), Some(last)) = (nets.first(), nets.last()) else { return 0.0 };
    (Sheet::stack(board, &nets) - first.1 / 2.0 - last.1 / 2.0).max(0.0)
}

/// Holds the wire of strand `s` off a wire already drawn, in each gap the plan has them both
/// cross: no nearer where the drawn wire crosses it than the two may be with the strands the
/// plan has between them, on the side of it the plan has this one. Each wire is drawn alone,
/// and in a wide gap two drawn where each lies best can leave too little between them for a
/// third the plan has there.
fn keep_off(board: &Board, sheet: &Sheet, s: usize, course: &mut Course, other: &Wire) {
    let space = &sheet.space;
    let (me, it) = (&sheet.strands[s], &sheet.strands[other.strand]);
    if me.net == it.net {
        return;
    }
    let mut m = 0;
    for (k, &e) in me.crossings.iter().enumerate() {
        let edge = &space.edges[e];
        let ends = edge.v.map(|v| space.verts[v]);
        let Some(at) = (m..course.gaps.len()).find(|&x| course.gaps[x].ends == ends) else { continue };
        m = at + 1;
        let on = &sheet.on[e];
        let (Some(mine), Some(its)) = (on.iter().position(|&x| x == (s, k)), on.iter().position(|&x| x.0 == other.strand)) else { continue };
        let (lo, hi) = (mine.min(its), mine.max(its));
        let apart = apart(board, sheet, &on[lo..=hi]);
        // Where the drawn wire crosses the gap, as a share of the gap from its first corner.
        let along = sub(ends[1], ends[0]);
        let crossed = other.pts.windows(2).find_map(|w| {
            let d = sub(w[1], w[0]);
            let den = convex::cross(along, d);
            if den.abs() < 1e-12 {
                return None;
            }
            let t = convex::cross(sub(w[0], ends[0]), d) / den;
            let u = convex::cross(sub(w[0], ends[0]), along) / den;
            ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then_some(t)
        });
        let Some(t) = crossed else { continue };
        let gap = &mut course.gaps[at];
        let by = apart / edge.len.max(1e-12);
        let (new_lo, new_hi) = if its < mine { (gap.lo.max(t + by), gap.hi) } else { (gap.lo, gap.hi.min(t - by)) };
        // (Two that fit with nothing to spare come out a hair over in floating point.)
        if new_lo <= new_hi + 1e-6 {
            (gap.lo, gap.hi) = (new_lo.min(new_hi), new_hi);
        }
    }
}

/// A wire already drawn that the new wire was faulted against, for it to keep from: its
/// corners and pieces, on its side of the new wire, no nearer than the two may be.
fn learn_wire(board: &Board, sheet: &Sheet, s: usize, other: &Wire, course: &mut Course) -> bool {
    if course.walls.iter().any(|w| w.a == other.pts[0] && w.b == other.pts[1]) {
        return false;
    }
    let Some((side, r)) = neighbour(board, sheet, s, course, other, false) else { return false };
    course.corners.extend(other.pts.iter().map(|&at| Corner { at, r, left: side }));
    course.walls.extend(other.pts.windows(2).map(|w| Wall { a: w[0], b: w[1], r }));
    true
}

/// No wire has more bends than this, and one more for every [`GAPS_A_BEND`] gaps it crosses:
/// a wire that winds between the pads of a crowded board for half its length has a bend at
/// most of them.
const BENDS: u32 = 12;
const GAPS_A_BEND: usize = 2;
/// How far out of the box round its course a wire may bend, mm.
const OUT: f64 = 1.0;
/// How many times a wire is drawn again after the checker has faulted it against something its
/// course did not hold it from.
const AGAIN: usize = 4;

/// The corners and sides of an obstacle, for a wire of strand `s` to keep from: each corner on
/// the side of the wire it is on as the strand lies in the plan. Where the course already
/// holds the wire from them and the checker still faulted it, by `short` mm, it is held that
/// much further off: an obstacle's outline here is its shape made simpler, and may lie a
/// little inside it.
fn learn(board: &Board, sheet: &Sheet, s: usize, o: usize, short: f64, course: &mut Course) -> bool {
    let space = &sheet.space;
    let strand = &sheet.strands[s];
    let obstacle = &space.obstacles[o];
    if obstacle.net == strand.net && strand.net != 0 {
        return false;
    }
    let of: Vec<P> = space.verts.iter().enumerate().filter(|&(v, _)| space.vert_of[v] == Some(o)).map(|(_, &p)| p).collect();
    let known = course.stages.iter().flat_map(|x| &x.corners).chain(&course.corners).any(|c| of.contains(&c.at));
    if known && short > 0.0 && short < 0.05 {
        let more = short + 0.002;
        for c in course.stages.iter_mut().flat_map(|x| x.corners.iter_mut()).chain(course.corners.iter_mut()).filter(|c| of.contains(&c.at)) {
            c.r += more;
        }
        for w in course.stages.iter_mut().flat_map(|x| x.walls.iter_mut()).chain(course.walls.iter_mut()).filter(|w| of.contains(&w.a) && of.contains(&w.b)) {
            w.r += more;
        }
        return true;
    }
    // The strand as a line: its ends and where it lies in each gap.
    let places = sheet.places(board);
    let line: Vec<P> = std::iter::once(course.a).chain(strand.crossings.iter().enumerate().map(|(k, &e)| sheet.point(e, places[s][k]))).chain([course.b]).collect();
    let mut new = false;
    let mut r = 0.0f64;
    for (v, &at) in space.verts.iter().enumerate() {
        if space.vert_of[v] != Some(o) || course.corners.iter().any(|c| c.at == at) {
            continue;
        }
        let nearest = line.windows(2).min_by(|x, y| chain::point_segment(at, x[0], x[1]).total_cmp(&chain::point_segment(at, y[0], y[1])));
        let Some(w) = nearest else { continue };
        let keep = space.keep(board, v, strand.net) + strand.width / 2.0;
        r = r.max(keep);
        course.corners.push(Corner { at, r: keep, left: convex::cross(sub(w[1], w[0]), sub(at, w[0])) > 0.0 });
        new = true;
    }
    if new && obstacle.reach == 0.0 {
        course.walls.extend((0..obstacle.poly.len()).map(|i| Wall { a: obstacle.poly[i], b: obstacle.poly[(i + 1) % obstacle.poly.len()], r }));
    }
    new
}

/// Draws every strand of the plan, layer by layer, in the order the plan laid them.
pub fn draw(board: &Board, plan: &Plan) -> Drawing {
    draw_watching(board, plan, None).0
}

/// [`draw`], and a picture of the course of one strand (a layer and a strand's number): what it
/// keeps from, and the wire found.
pub fn draw_watching(board: &Board, plan: &Plan, watch: Option<(usize, usize)>) -> (Drawing, Option<String>) {
    draw_with(board, plan, watch, &mut chain::Memo::default())
}

/// [`draw_watching`], with the chains found for courses drawn before.
fn draw_with(board: &Board, plan: &Plan, watch: Option<(usize, usize)>, memo: &mut chain::Memo) -> (Drawing, Option<String>) {
    let mut drawing = Drawing::default();
    let mut watched = None;
    // The board as it fills: every wire is put to the exact checker as it is drawn, and one it
    // faults is not kept.
    let mut work = board.clone();
    let (r, drill) = plan::via_size(board);
    let all = (1u64 << board.layers.len()) - 1;
    // Which wire of the drawing each track on the board is a piece of.
    let mut tracks: HashMap<ItemId, usize> = HashMap::new();
    let mut vias: HashMap<ItemId, usize> = HashMap::new();
    for v in &plan.vias {
        let c = nm(v.at);
        let id = work.insert(Item { kind: Kind::Via, net: v.net, layers: all, shape: Shape::Capsule { a: c, b: c, r }, hole: Some((c, drill)), locked: false, span: None });
        vias.insert(id, v.site);
    }
    for (layer, sheet) in plan.sheets.iter().enumerate() {
        // The order wires are drawn in: the plan's, but each strand's neighbours in its gaps
        // straight after it, so that a bundle is drawn from one side across and each wire
        // has one beside it to run along.
        let laid: Vec<usize> = sheet.laid().collect();
        let mut next_to: HashMap<usize, Vec<usize>> = HashMap::new();
        for on in &sheet.on {
            for pair in on.windows(2) {
                next_to.entry(pair[0].0).or_default().push(pair[1].0);
                next_to.entry(pair[1].0).or_default().push(pair[0].0);
            }
        }
        let mut order: Vec<usize> = Vec::new();
        let mut queued: std::collections::HashSet<usize> = std::collections::HashSet::new();
        for &first in &laid {
            let mut queue = std::collections::VecDeque::from([first]);
            while let Some(s) = queue.pop_front() {
                if !queued.insert(s) {
                    continue;
                }
                order.push(s);
                let mut beside: Vec<usize> = next_to.get(&s).cloned().unwrap_or_default();
                beside.sort();
                beside.dedup();
                queue.extend(beside.into_iter().filter(|x| !queued.contains(x)));
            }
        }
        let easel = Easel { board, plan, layer, sheet, vias: &vias };
        for s in order {
            match one(&easel, &mut work, &drawing.wires, &tracks, memo, s, None, &[], watch.filter(|w| *w == (layer, s)).map(|_| &mut watched)) {
                Drawn::Wire(wire, ids) => {
                    tracks.extend(ids.iter().map(|&id| (id, drawing.wires.len())));
                    drawing.wires.push(wire);
                }
                Drawn::Not(undrawn, first) => {
                    let gave = first.is_some_and(|(pts, in_the_way)| give_way(&easel, &mut work, &mut drawing, &mut tracks, memo, s, pts, &in_the_way));
                    if !gave {
                        drawing.undrawn.push(undrawn);
                    }
                }
            }
        }
    }
    (drawing, watched)
}

/// What one layer's wires are drawn from.
struct Easel<'a> {
    board: &'a Board,
    plan: &'a Plan,
    layer: usize,
    sheet: &'a Sheet,
    /// The plan's via each via on the board is.
    vias: &'a HashMap<ItemId, usize>,
}

/// What came of drawing one strand.
enum Drawn {
    /// Its wire, and the tracks it is on the board as.
    Wire(Wire, Vec<ItemId>),
    /// No wire. Where the first path found for it was faulted against wires already drawn and
    /// against nothing else, that path and those wires, by their numbers in the drawing.
    Not(Undrawn, Option<(Vec<P>, Vec<usize>)>),
}

/// Draws strand `s` on `work`, the board with `wires` on it (`tracks` tells which wire each
/// track is a piece of): its tracks are left on the board if the checker passes them. The
/// wire numbered `skip` is taken as not there (it is off the board, to be drawn again), and
/// the wire is told of `told` before it is first drawn.
#[allow(clippy::too_many_arguments)]
fn one(easel: &Easel, work: &mut Board, wires: &[Wire], tracks: &HashMap<ItemId, usize>, memo: &mut chain::Memo, s: usize, skip: Option<usize>, told: &[&Wire], watched: Option<&mut Option<String>>) -> Drawn {
    let Easel { board, plan, layer, sheet, vias } = *easel;
    let mut watched = watched;
    let strand = &sheet.strands[s];
    let Some(mut course) = course(board, sheet, s) else {
        return Drawn::Not(Undrawn { layer, strand: s, net: strand.net, why: "its gaps do not follow one another".to_string(), refused: Vec::new(), gap: None }, None);
    };
    // The wires already drawn that the plan has in a gap with this one: their bends
    // hold lines right beside them, for this wire to run along where it can.
    for (_, other) in wires.iter().enumerate().filter(|(w, other)| other.layer == layer && Some(*w) != skip) {
        if let Some((side, r)) = neighbour(board, sheet, s, &course, other, true) {
            course.beside.extend(other.pts.windows(2).map(|w| (w[0], w[1], r, side)));
            keep_off(board, sheet, s, &mut course, other);
        }
    }
    for other in told {
        learn_wire(board, sheet, s, other, &mut course);
    }
    let mut first = None;
    let mut again = 0;
    loop {
        // Along the pads' lengths if that can be; else any way out of them.
        let bends = BENDS + (course.gaps.len() / GAPS_A_BEND) as u32;
        let found = memo.chain(&course, bends).or_else(|_| memo.chain(&Course { out: Vec::new(), into: Vec::new(), ..course.clone() }, bends));
        if let Some(watched) = watched.as_deref_mut() {
            let places = sheet.places(board);
            let line: Vec<P> = std::iter::once(course.a).chain(strand.crossings.iter().enumerate().map(|(k, &e)| sheet.point(e, places[s][k]))).chain([course.b]).collect();
            let drawn: Vec<&Wire> = wires.iter().filter(|w| w.layer == layer).collect();
            *watched = Some(draw::course(sheet, &course, found.as_ref().ok(), &drawn, &line));
        }
        // The gap the search did not get across.
        let stuck = found.as_ref().err().and_then(|&m| course.gaps.get(m).or(course.gaps.last())).map(|g| g.ends);
        // With no path in 0/45/90: the strand as it lies in the plan, from gap to gap.
        // The plan keeps every strand its room in every gap, so that is a wire the
        // checker passes wherever the gaps tell the whole of what is near.
        let as_planned = found.is_err();
        let path = found.unwrap_or_else(|_| {
            let places = sheet.places(board);
            let mut pts = vec![course.a];
            for p in strand.crossings.iter().enumerate().map(|(k, &e)| sheet.point(e, places[s][k])).chain([course.b]) {
                // A point on the line between its neighbours adds nothing.
                while pts.len() >= 2 && chain::point_segment(pts[pts.len() - 1], pts[pts.len() - 2], p) < 1e-6 {
                    pts.pop();
                }
                if pts.last().is_none_or(|&q| convex::len(sub(p, q)) > 1e-6) {
                    pts.push(p);
                }
            }
            chain::Path { length: pts.windows(2).map(|w| convex::len(sub(w[1], w[0]))).sum(), pts }
        });
        let ids = lay(work, &path.pts, strand, layer);
        let faults: Vec<board::Violation> = ids.iter().flat_map(|&id| board::check_item(work, id)).collect();
        if faults.is_empty() {
            return Drawn::Wire(Wire { layer, strand: s, net: strand.net, width: strand.width, pts: path.pts, as_planned }, ids);
        }
        for &id in &ids {
            work.remove(id);
        }
        // What it is faulted for may lie on its own pad, past where it came onto it:
        // the middle of a pad on the board's edge, or of one under a rule area.
        if let Some(short) = short_of_pads(board, plan, strand, layer, &path.pts) {
            let ids = lay(work, &short, strand, layer);
            if ids.iter().all(|&id| board::check_item(work, id).is_empty()) {
                return Drawn::Wire(Wire { layer, strand: s, net: strand.net, width: strand.width, pts: short, as_planned }, ids);
            }
            for &id in &ids {
                work.remove(id);
            }
        }
        // The first path found, where only wires drawn before it are in its way: those may
        // be the ones that should give way ([`give_way`]).
        if again == 0 && !as_planned {
            let others: Vec<Option<usize>> = faults.iter().flat_map(|f| [Some(f.a), f.b]).flatten().filter(|id| !ids.contains(id)).map(|id| tracks.get(&id).copied()).collect();
            if !others.is_empty() && others.iter().all(|w| w.is_some_and(|w| wires[w].layer == layer)) && faults.iter().all(|f| f.b.is_some()) {
                let mut in_the_way: Vec<usize> = others.into_iter().flatten().collect();
                in_the_way.sort_unstable();
                in_the_way.dedup();
                first = Some((path.pts.clone(), in_the_way));
            }
        }
        // What it was faulted against that its course did not hold it from: a pad or a
        // via that is not beside the gaps its strand crosses, or a wire drawn before it
        // that lies where the plan gave it no reason not to. The wire is drawn again
        // told of it.
        let mut learnt = false;
        for other in faults.iter().flat_map(|f| [Some(f.a), f.b]).flatten().filter(|id| !ids.contains(id)) {
            if let Some(&w) = tracks.get(&other) {
                learnt |= wires[w].layer == layer && learn_wire(board, sheet, s, &wires[w], &mut course);
                continue;
            }
            let o = match vias.get(&other) {
                Some(&site) => sheet.space.sites.get(site).map(|x| x.obstacle),
                None => sheet.space.obstacles.iter().position(|x| x.item == Some(other)),
            };
            let short = faults.iter().filter(|f| f.a == other || f.b == Some(other)).map(|f| (f.required as f64 - f.gap) / 1e6).fold(0.0, f64::max);
            match o {
                Some(o) => learnt |= learn(board, sheet, s, o, short, &mut course),
                None => {
                    if let Some(item) = work.item(other).filter(|i| matches!(i.kind, Kind::Spoke { .. }) && i.on(layer)) {
                        let need = faults.iter().filter(|f| f.a == other || f.b == Some(other)).map(|f| f.required).max().unwrap_or(0);
                        learnt |= learn_shape(board, sheet, s, &item.shape, need as f64 / 1e6 + strand.width / 2.0, &mut course);
                    }
                }
            }
        }
        again += 1;
        if !learnt || again > AGAIN || as_planned {
            let why = if as_planned { "no path in 0/45/90, and as planned it breaks a rule" } else { "the path found breaks a rule" };
            // The gap nearest what it was faulted against.
            let other = faults.iter().flat_map(|f| [Some(f.a), f.b]).flatten().find(|id| !ids.contains(id)).and_then(|id| work.bbox(id));
            let near = other.and_then(|b| {
                let at = [(b.min.x + b.max.x) as f64 / 2e6, (b.min.y + b.max.y) as f64 / 2e6];
                course.gaps.iter().min_by(|x, y| chain::point_segment(at, x.ends[0], x.ends[1]).total_cmp(&chain::point_segment(at, y.ends[0], y.ends[1]))).map(|g| g.ends)
            });
            // With no gap held: the one in the middle of its strand.
            let middle = strand.crossings.get(strand.crossings.len() / 2).map(|&e| sheet.space.edges[e].v.map(|v| sheet.space.verts[v]));
            // What it was faulted against, in a word.
            let against = match faults.iter().flat_map(|f| [Some(f.a), f.b]).flatten().find(|id| !ids.contains(id)) {
                Some(id) if tracks.contains_key(&id) => "another wire",
                Some(id) if vias.contains_key(&id) => "a via",
                Some(id) => match work.item(id).map(|i| &i.kind) {
                    Some(Kind::Pad { .. }) => "a pad",
                    Some(Kind::Spoke { .. }) => "a pad's spoke to its pour",
                    Some(Kind::Keepout { .. }) => "a keepout",
                    _ => "copper",
                },
                None if format!("{:?}", faults[0].rule) == "Keepout" => "an area its net is kept out of",
                None => "the board's edge",
            };
            let undrawn = Undrawn { layer, strand: s, net: strand.net, why: format!("{why} ({:?} against {against}, {:.3} of {:.3} mm)", faults[0].rule, faults[0].gap / 1e6, faults[0].required as f64 / 1e6), refused: path.pts, gap: stuck.or(near).or(middle) };
            return Drawn::Not(undrawn, first);
        }
    }
}

/// Draws strand `s` along `pts`, the first path found for it, by having the wires in its way
/// drawn again round it: each wire is drawn alone, and one drawn earlier may lie, within
/// what the plan lets it, where a later one has its only way (a wire that comes onto its pad
/// down the middle of the row, where the wire of the next pad has to leave). Whether that
/// was done: if any of those wires cannot be drawn again, all is left as it was.
#[allow(clippy::too_many_arguments)]
fn give_way(easel: &Easel, work: &mut Board, drawing: &mut Drawing, tracks: &mut HashMap<ItemId, usize>, memo: &mut chain::Memo, s: usize, pts: Vec<P>, in_the_way: &[usize]) -> bool {
    let Easel { layer, sheet, .. } = *easel;
    let strand = &sheet.strands[s];
    // Off the board: the wires in its way.
    for &w in in_the_way {
        let ids: Vec<ItemId> = tracks.iter().filter(|x| *x.1 == w).map(|x| *x.0).collect();
        for id in ids {
            work.remove(id);
            tracks.remove(&id);
        }
    }
    let before: Vec<(usize, Wire)> = in_the_way.iter().map(|&w| (w, drawing.wires[w].clone())).collect();
    let n = drawing.wires.len();
    let ids = lay(work, &pts, strand, layer);
    let mut done = ids.iter().all(|&id| board::check_item(work, id).is_empty());
    if done {
        tracks.extend(ids.iter().map(|&id| (id, n)));
        drawing.wires.push(Wire { layer, strand: s, net: strand.net, width: strand.width, pts, as_planned: false });
        for &w in in_the_way {
            let new = drawing.wires[n].clone();
            match one(easel, work, &drawing.wires, tracks, memo, drawing.wires[w].strand, Some(w), &[&new], None) {
                Drawn::Wire(wire, ids) => {
                    tracks.extend(ids.iter().map(|&id| (id, w)));
                    drawing.wires[w] = wire;
                }
                Drawn::Not(..) => {
                    done = false;
                    break;
                }
            }
        }
    } else {
        for &id in &ids {
            work.remove(id);
        }
    }
    if done {
        return true;
    }
    // As it was: the new wire and what was drawn again come off, the old wires go back.
    let off: Vec<ItemId> = tracks.iter().filter(|x| *x.1 == n || in_the_way.contains(x.1)).map(|x| *x.0).collect();
    for id in off {
        work.remove(id);
        tracks.remove(&id);
    }
    drawing.wires.truncate(n);
    for (w, wire) in before {
        let ids = lay(work, &wire.pts, &sheet.strands[wire.strand], layer);
        tracks.extend(ids.iter().map(|&id| (id, w)));
        drawing.wires[w] = wire;
    }
    false
}

/// The pieces of a wire of `strand` through `pts`, put on the board as tracks.
fn lay(work: &mut Board, pts: &[P], strand: &plan::strands::Strand, layer: usize) -> Vec<ItemId> {
    pts.windows(2)
        .map(|seg| {
            let shape = Shape::Capsule { a: nm(seg[0]), b: nm(seg[1]), r: (strand.width * 1e6 / 2.0).round() as i64 };
            work.insert(Item { kind: Kind::Track, net: strand.net, layers: 1 << layer, shape, hole: None, locked: false, span: None })
        })
        .collect()
}

/// How far onto its pad a wire that stops short of the pad's middle goes, mm: enough for its
/// end to lie on the pad's copper, and no more than half its width.
const ONTO: f64 = 0.05;

/// A wire's path cut short at each end that is a pad: to just past where it first comes onto
/// the copper of that pad, or of a pad joined to it on the bare board. A wire need not reach
/// the middle of its pad to be joined to it. None where that leaves the path as it is.
fn short_of_pads(board: &Board, plan: &Plan, strand: &plan::strands::Strand, layer: usize, pts: &[P]) -> Option<Vec<P>> {
    let copper = |end: End| -> Vec<&Shape> {
        let End::Item(id) = end else { return Vec::new() };
        std::iter::once(id).chain(plan.joined.get(&id).into_iter().flatten().copied()).filter_map(|p| board.item(p)).filter(|i| i.on(layer)).map(|i| &i.shape).collect()
    };
    // The path from its far end towards the pad whose copper is `on`: up to the last point off
    // that copper, and then just onto it.
    let cut = |pts: &[P], on: &[&Shape]| -> Option<Vec<P>> {
        let inside = |p: P| on.iter().any(|s| s.contains(nm(p)));
        let last = (0..pts.len().saturating_sub(1)).rev().find(|&i| !inside(pts[i]))?;
        let (a, b) = (pts[last], pts[last + 1]);
        if !inside(b) {
            return None;
        }
        // Where the piece from `a` to `b` comes onto the copper.
        let (mut lo, mut hi) = (0.0, 1.0);
        for _ in 0..30 {
            let mid = (lo + hi) / 2.0;
            if inside([a[0] + (b[0] - a[0]) * mid, a[1] + (b[1] - a[1]) * mid]) {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        let len = convex::len(sub(b, a));
        let t = (hi + ONTO.min(strand.width / 2.0) / len.max(1e-9)).min(1.0);
        let mut out = pts[..=last].to_vec();
        out.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
        Some(out)
    };
    let (from, to) = (copper(strand.from), copper(strand.to));
    let mut out = pts.to_vec();
    if let Some(shorter) = cut(&out, &to) {
        out = shorter;
    }
    out.reverse();
    if let Some(shorter) = cut(&out, &from) {
        out = shorter;
    }
    out.reverse();
    let same = out.len() == pts.len() && out.iter().zip(pts).all(|(a, b)| convex::len(sub(*a, *b)) < 1e-6);
    (!same && out.len() >= 2).then_some(out)
}

/// How many times the connections that could not be drawn are laid again.
const ROUNDS: usize = 8;
/// How many more rounds there may be in which a wire was made narrower: a track's width is
/// halved at most twice (`plan`'s `NARROWEST`).
const NARROWER: usize = 2;

/// The connections a board's pours leave to make once `drawing` is on it, for the nets in
/// `poured`: the pours are filled round the drawn copper, and what a piece of pour reaches it
/// joins. Each is from pad to pad, and comes with the spots where a via would stand in pour
/// its far end is joined to. And how many connections are open in all, of any net.
fn left_by_pours(board: &Board, plan: &Plan, drawing: &Drawing, poured: &[u32]) -> (Vec<plan::Want>, usize) {
    let (with, _) = onto(board, plan, drawing);
    let pours = board::Pours::of(&with);
    let joined = board::connectivity_poured(&with, &pours);
    let Some(spots) = plan.sheets.first().map(|s| s.space.spots.clone()) else { return (Vec::new(), joined.airlines.len()) };
    let (r, _) = plan::via_size(board);
    // The cluster of copper each piece of pour is part of.
    let mut cluster_of: HashMap<usize, usize> = HashMap::new();
    for (&id, &cluster) in &joined.cluster {
        let Some(item) = with.item(id) else { continue };
        if !poured.contains(&item.net) {
            continue;
        }
        for layer in (0..with.layers.len()).filter(|l| item.on(*l)) {
            for piece in pours.joined(&with, id, layer) {
                cluster_of.insert(piece, cluster);
            }
        }
    }
    // Per net, the spots a via would stand in its pour at, with the cluster that joins it to.
    let mut stands: HashMap<u32, Vec<(usize, usize)>> = HashMap::new();
    for &net in poured {
        let list = stands.entry(net).or_default();
        for (c, p) in spots.at.iter().enumerate() {
            let cluster = (0..with.layers.len()).find_map(|l| pours.piece(net, l, nm(*p), r).and_then(|piece| cluster_of.get(&piece).copied()));
            if let Some(cluster) = cluster {
                list.push((c, cluster));
            }
        }
    }
    // The pads of each cluster of copper, and each pad's middle.
    let middle = |id: ItemId| board.bbox(id).map(|b| board::Point::new((b.min.x + b.max.x) / 2, (b.min.y + b.max.y) / 2));
    let mut pads: HashMap<usize, Vec<(ItemId, board::Point)>> = HashMap::new();
    for (&id, &cluster) in &joined.cluster {
        if board.item(id).is_some_and(|i| matches!(i.kind, Kind::Pad { .. })) {
            pads.entry(cluster).or_default().extend(middle(id).map(|m| (id, m)));
        }
    }
    // (Each cluster's pads in their own order: which is the nearest of two as near is not to
    // turn on the order a hash map gives them in.)
    for of in pads.values_mut() {
        of.sort_by_key(|p| p.0);
    }
    // An end that is a wire or a via (one that stands in pour the wires have since cut off
    // from what it was to join, say) is taken as the pad of its copper nearest the other end.
    let pad_of = |end: ItemId, other: board::Point| -> Option<(ItemId, board::Point)> {
        let of = pads.get(joined.cluster.get(&end)?)?;
        of.iter().find(|p| p.0 == end).or_else(|| of.iter().min_by(|x, y| (x.0 != end, other.distance(x.1)).partial_cmp(&(y.0 != end, other.distance(y.1))).unwrap_or(std::cmp::Ordering::Equal))).copied()
    };
    let mut wants = Vec::new();
    for line in joined.airlines.iter().filter(|a| poured.contains(&a.net)) {
        let (Some(from), Some(to)) = (pad_of(line.from_item, line.to), pad_of(line.to_item, line.from)) else { continue };
        let line = &board::Airline { net: line.net, from: from.1, to: to.1, from_item: from.0, to_item: to.0 };
        let into = |end: ItemId| -> Vec<usize> {
            let cluster = joined.cluster.get(&end);
            stands[&line.net].iter().filter(|(_, k)| Some(k) == cluster).map(|(c, _)| *c).collect()
        };
        let mut want = plan::Want::of(line);
        want.into = into(want.to);
        // From the end that has no pour to the end that has.
        if want.into.is_empty() {
            let back = into(want.from);
            if !back.is_empty() {
                (want.from, want.to, want.into) = (want.to, want.from, back);
            }
        }
        wants.push(want);
    }
    wants.sort_by(|a, b| a.apart.total_cmp(&b.apart));
    (wants, joined.airlines.len())
}

/// How many connections are still open with `drawing` on the board and the pours filled round
/// it.
pub fn open(board: &Board, plan: &Plan, drawing: &Drawing) -> usize {
    let (with, _) = onto(board, plan, drawing);
    board::connectivity_poured(&with, &board::Pours::of(&with)).airlines.len()
}

/// What a round of [`settle`] came to, as one number: the wires drawn, the vias, the wires
/// that stuck with where and along what path, the connections with no way, and what each
/// connection has been told so far (how narrow it is laid, the gaps it is barred from): the
/// next round starts from all of that, and from nothing else.
fn outcome(plan: &Plan, drawing: &Drawing, stuck: &[(usize, plan::space::GapKey)]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let nm = |p: P| ((p[0] * 1e6).round() as i64, (p[1] * 1e6).round() as i64);
    for w in &drawing.wires {
        (w.layer, w.net, (w.width * 1e6).round() as i64, w.pts.iter().map(|&p| nm(p)).collect::<Vec<_>>()).hash(&mut h);
    }
    plan.vias.iter().map(|v| (nm(v.at), v.net)).collect::<Vec<_>>().hash(&mut h);
    stuck.hash(&mut h);
    for u in &drawing.undrawn {
        u.refused.iter().map(|&p| nm(p)).collect::<Vec<_>>().hash(&mut h);
    }
    plan.left.iter().map(|l| l.connection).collect::<Vec<_>>().hash(&mut h);
    for w in &plan.wants {
        (w.narrow.map(f64::to_bits), &w.avoid).hash(&mut h);
    }
    h.finish()
}

/// Draws the plan, and while some strand cannot be drawn, lays its connection again narrower,
/// or at its narrowest barred from the gap its wire stuck in, and draws again: the rest of
/// the plan stays, so what was drawn stays drawn. A gap's length is the most it can hold; what
/// it does hold depends on
/// what stands round it, and drawing is what finds that out. The plan is left as it was
/// when fewest of its connections were open.
fn settle(board: &Board, plan: &mut Plan, memo: &mut chain::Memo, said: &dyn Fn(&str, &Plan, Option<&Drawing>), stop: &AtomicBool) -> Drawing {
    let mut best: Option<(usize, Plan, Drawing)> = None;
    // A round that only made wires narrower is not counted: a few of those.
    let (mut rounds, mut more) = (0, NARROWER);
    // What the round before came to: a round that comes to the same has changed nothing, and
    // the next would be the same again.
    let mut before: Option<u64> = None;
    while rounds < ROUNDS && !stop.load(Ordering::Relaxed) {
        rounds += 1;
        // A stub nothing starts from is not drawn.
        plan::tidy(board, plan);
        let drawing = draw_with(board, plan, None, memo).0;
        said("drawn", plan, Some(&drawing));
        let open = plan.left.len() + drawing.undrawn.len();
        let stuck: Vec<(usize, plan::space::GapKey)> =
            drawing.undrawn.iter().filter_map(|u| Some((plan.sheets[u.layer].strands[u.strand].connection, u.gap.map(|g| plan::space::gap_key(g[0], g[1]))?))).collect();
        let same = before.replace(outcome(plan, &drawing, &stuck)) == before;
        if best.as_ref().is_none_or(|b| open < b.0) {
            best = Some((open, plan.clone(), drawing));
        }
        if open == 0 || stuck.is_empty() || same {
            break;
        }
        if plan::again(board, plan, &stuck) && more > 0 {
            more -= 1;
            rounds -= 1;
        }
        said("laid again", plan, None);
    }
    let (_, kept, drawing) = best.unwrap_or_else(|| (0, plan.clone(), Drawing::default()));
    *plan = kept;
    drawing
}

/// The pads `drawing` leaves with too few spokes to their pour, as KiCad counts them once the
/// pours are filled round it, but for those in `before`, which the bare board has already.
fn starved(board: &Board, plan: &Plan, drawing: &Drawing, before: &[(ItemId, usize)]) -> Vec<board::Starved> {
    let (with, _) = onto(board, plan, drawing);
    board::Pours::likely(&with).starved(&with).into_iter().filter(|s| !before.contains(&(s.pad, s.layer))).collect()
}

/// What is wrong with `drawing` on the board once the pours are filled round it: how many
/// connections are still open, and how many pads are left too few spokes to their pour that
/// had enough on the bare board (KiCad's `starved_thermal`).
pub fn amiss(board: &Board, plan: &Plan, drawing: &Drawing) -> (usize, usize) {
    let before: Vec<(ItemId, usize)> = board::Pours::likely(board).starved(board).iter().map(|s| (s.pad, s.layer)).collect();
    (open(board, plan, drawing), starved(board, plan, drawing, &before).len())
}

/// The wires that shut in the pour a starved pad's spokes end in, each as its connection and
/// the gap of its strand nearest the pad: a spoke into a piece of pour that joins nothing
/// else is no spoke, and it is the wires round that piece that made it one.
pub fn shutting(board: &Board, plan: &Plan, drawing: &Drawing, starved: &[board::Starved]) -> Vec<(usize, plan::space::GapKey)> {
    let (with, new) = onto(board, plan, drawing);
    let pours = board::Pours::likely(&with);
    // The tracks of each wire, in the order `onto` lays them.
    let of_wire = drawing.wires.iter().enumerate().flat_map(|(w, wire)| std::iter::repeat_n(w, wire.pts.len().saturating_sub(1)));
    let mut out = Vec::new();
    for (&id, w) in new.iter().zip(of_wire) {
        let wire = &drawing.wires[w];
        for s in starved.iter().filter(|s| s.layer == wire.layer) {
            let Some(pad) = with.item(s.pad).filter(|p| p.net != wire.net) else { continue };
            if !s.pockets.iter().any(|&piece| pours.beside(&with, piece, id)) {
                continue;
            }
            let sheet = &plan.sheets[wire.layer];
            let strand = &sheet.strands[wire.strand];
            let b = pad.shape.bbox();
            let at = [(b.min.x + b.max.x) as f64 / 2e6, (b.min.y + b.max.y) as f64 / 2e6];
            let ends = |e: usize| sheet.space.edges[e].v.map(|v| sheet.space.verts[v]);
            let nearest = strand.crossings.iter().filter(|&&e| sheet.space.gap(e)).map(|&e| ends(e)).min_by(|x, y| chain::point_segment(at, x[0], x[1]).total_cmp(&chain::point_segment(at, y[0], y[1])));
            if let Some(gap) = nearest {
                let bar = (strand.connection, plan::space::gap_key(gap[0], gap[1]));
                if !out.contains(&bar) {
                    out.push(bar);
                }
            }
        }
    }
    out
}

/// How far `shape` reaches from `c` along `dir`, nm.
fn extent(shape: &Shape, c: board::Point, dir: [f64; 2]) -> f64 {
    let along = |p: board::Point| (p.x - c.x) as f64 * dir[0] + (p.y - c.y) as f64 * dir[1];
    match shape {
        Shape::Polygon(points) => points.iter().map(|p| along(*p)).fold(0.0, f64::max),
        Shape::Capsule { a, b, r } => along(*a).max(along(*b)) + *r as f64,
    }
}

/// A spoke drawn by hand for pad `s` of `with`, the board with the drawing on it and `pours`
/// its pours: a track from the pad's middle, one of the eight ways, across its gap and the
/// zone's least width on into pour that is not shut in, where the pad has no spoke already
/// and the checker passes it. The track is left on `with`.
fn tie(with: &mut Board, pours: &board::Pours, s: &board::Starved) -> Option<Tie> {
    let pad = with.item(s.pad)?.clone();
    let b = pad.shape.bbox();
    let c = board::Point::new((b.min.x + b.max.x) / 2, (b.min.y + b.max.y) / 2);
    let gap = *with.relief_pads.get(&(s.pad, s.zone))? as f64;
    let least = with.zone_areas.iter().find(|z| z.zone == s.zone)?.min_thickness as f64;
    if !pad.shape.contains(c) {
        return None;
    }
    let class = plan::width(with, pad.net);
    let narrow = (class / 2.0).max(with.rules.min_track_width * 1e3).max(with.min_track_width_of(pad.net) as f64 / 1e6);
    for width in [class, narrow] {
        let r = (width * 1e6 / 2.0).round();
        // The diagonals first: a pad's own spokes mostly run along its sides.
        for turn in [1, 3, 5, 7, 0, 2, 4, 6] {
            let (sin, cos) = (turn as f64 * std::f64::consts::FRAC_PI_4).sin_cos();
            let reach = extent(&pad.shape, c, [cos, sin]);
            let at = |d: f64| board::Point::new(c.x + (cos * d).round() as i64, c.y + (sin * d).round() as i64);
            let (ring, far) = (at(reach + gap / 2.0), at(reach + gap + least + r));
            let own = |p: board::Point| {
                with.near(s.layer, board::BBox::of([p])).into_iter().any(|q| with.item(q).is_some_and(|it| matches!(it.kind, Kind::Spoke { pad, .. } if pad == s.pad) && it.on(s.layer) && it.shape.contains(p)))
            };
            let leads_on = pours.piece(pad.net, s.layer, far, r as i64).is_some_and(|piece| !s.pockets.contains(&piece));
            if !leads_on || own(ring) {
                continue;
            }
            let id = with.insert(Item { kind: Kind::Track, net: pad.net, layers: 1 << s.layer, shape: Shape::Capsule { a: c, b: far, r: r as i64 }, hole: None, locked: false, span: None });
            if board::check_item(with, id).is_empty() {
                let mm = |p: board::Point| [p.x as f64 / 1e6, p.y as f64 / 1e6];
                return Some(Tie { layer: s.layer, net: pad.net, width, pts: [mm(c), mm(far)] });
            }
            with.remove(id);
        }
    }
    None
}

/// `drawing` with a spoke drawn by hand for each pad it leaves short of spokes ([`tie`]),
/// and how many pads are short still.
fn tied(board: &Board, plan: &Plan, mut drawing: Drawing, before: &[(ItemId, usize)]) -> (Drawing, usize) {
    for _ in 0..SPOKES {
        let (mut with, _) = onto(board, plan, &drawing);
        let pours = board::Pours::likely(&with);
        let short: Vec<board::Starved> = pours.starved(&with).into_iter().filter(|s| !before.contains(&(s.pad, s.layer))).collect();
        let more: Vec<Tie> = short.iter().filter_map(|s| tie(&mut with, &pours, s)).collect();
        if more.is_empty() {
            return (drawing, short.len());
        }
        drawing.ties.extend(more);
    }
    let short = starved(board, plan, &drawing, before).len();
    (drawing, short)
}

/// Plans a stripped board and draws the plan ([`settle`]).
///
/// A net with a pour is left to its pour: the other nets are planned and drawn first, the
/// pours are filled round them, and only what the pours then leave apart is planned, each such
/// connection to the nearest pad of the other part or to a via standing in its pour. A pad
/// the wires then leave too few spokes, because they shut in the pour a spoke of it ends in,
/// is given one drawn by hand ([`Tie`]); where it has no room for one, the wires that shut the
/// pour in are laid another way, if that leaves no more open than before.
///
/// A board that is not whole that way, and has parts whose pins stand nearer each other than
/// vias may, is routed with those parts' ways out settled first as well ([`plan::escape`]):
/// for each row of such a part's pads a row of vias a little way out, one for each pad in the
/// pads' order, and the connections then from via to via. The one with fewer connections
/// open is kept. The two are worked out side by side, on a thread each, and the second is
/// dropped where the first comes out whole.
pub fn route(board: &Board) -> (Plan, Drawing) {
    let never = AtomicBool::new(false);
    if std::env::var("WIRE_FIRST").is_ok() {
        let (plan, drawing) = route_with(board, false, &never).unwrap_or_else(|| (plan::start(board), Drawing::default()));
        let drawing = without_spare(board, &plan, drawing);
        return loose::loose(board, &plan, &drawing);
    }
    // The two ways side by side, each on a thread of its own: neither asks anything of the
    // other. The second is for a board the first leaves open, and is told to stop, at the
    // end of what it is at, once the first has come out whole: what is kept is as it would
    // be with one after the other.
    let stop = AtomicBool::new(false);
    let (plain, fanned) = std::thread::scope(|scope| {
        let fanned = scope.spawn(|| route_with(board, true, &stop));
        let plain = route_with(board, false, &never);
        let still = plain.as_ref().map(|p| open(board, &p.0, &p.1));
        if still.is_none_or(|open| open == 0) {
            stop.store(true, Ordering::Relaxed);
        }
        (plain.zip(still), fanned.join().unwrap_or_else(|panic| std::panic::resume_unwind(panic)))
    });
    let Some((plain, still)) = plain else { return (plan::start(board), Drawing::default()) };
    let (plan, drawing) = match fanned {
        Some(fanned) if still > 0 && open(board, &fanned.0, &fanned.1) < still => fanned,
        _ => plain,
    };
    let drawing = without_spare(board, &plan, drawing);
    // Vias and wires left joining nothing where a wire beside them could not be drawn.
    loose::loose(board, &plan, &drawing)
}

/// `drawing` with the copper its nets have no need of taken out ([`spare::spare`]), where
/// that leaves no more connections open and no more pads short of spokes: a track that lies
/// in its net's pour can be all that joins a piece of the pour, and a track from a pad into
/// its pour counts as a spoke of the pad. Where it does leave more, the nets with pours are
/// left as they are; and where that does too, all are.
fn without_spare(board: &Board, plan: &Plan, drawing: Drawing) -> Drawing {
    let mut poured: Vec<u32> = board.zone_areas.iter().map(|z| z.net).collect();
    poured.sort_unstable();
    poured.dedup();
    let was = amiss(board, plan, &drawing);
    for leave in [&[][..], &poured[..]] {
        let (less, taken) = spare::spare(board, plan, &drawing, leave);
        if taken == 0 {
            continue;
        }
        let now = amiss(board, plan, &less);
        if now.0 <= was.0 && now.1 <= was.1 {
            return less;
        }
    }
    drawing
}

/// [`route`] one way: with the fine-pitch parts' ways out settled first, or without. None
/// where there is no such part.
fn route_with(board: &Board, fan: bool, stop: &AtomicBool) -> Option<(Plan, Drawing)> {
    let mut poured: Vec<u32> = board.zone_areas.iter().filter(|z| !z.kept && z.net != 0).map(|z| z.net).collect();
    poured.sort_unstable();
    poured.dedup();
    let lines = board::connectivity_without_pours(board).airlines;
    let mut memo = chain::Memo::default();
    let times = std::env::var("WIRE_TIMES").is_ok();
    let clock = std::time::Instant::now();
    let said = |what: &str, plan: &Plan, drawing: Option<&Drawing>| {
        if times {
            eprintln!("{:7.2} s  {}{what}: {} strands, {} vias, {} left{}", clock.elapsed().as_secs_f64(), if fan { "ways out first, " } else { "" }, plan.sheets.iter().map(|s| s.laid().count()).sum::<usize>(), plan.vias.len(), plan.left.len(), drawing.map_or(String::new(), |d| format!(", {} drawn, {} not", d.wires.len(), d.undrawn.len())));
        }
    };
    // Told to stop: at the end of each thing it does, it does no more.
    let stopped = || stop.load(Ordering::Relaxed);
    let wants: Vec<plan::Want> = plan::wants(&lines).into_iter().filter(|w| !poured.contains(&w.net)).collect();
    let mut plan = if fan {
        let (spots, stubs) = plan::escape(board, &wants);
        if stubs.is_empty() {
            return None;
        }
        let mut plan = plan::start_from(board, spots);
        plan::add(board, &mut plan, stubs);
        said("fanned out", &plan, None);
        plan
    } else {
        plan::start(board)
    };
    if stopped() {
        return None;
    }
    plan::add(board, &mut plan, wants);
    said("planned", &plan, None);
    // WIRE_FIRST: the first plan and its drawing, with nothing laid again and no pours: what
    // the plan lays that cannot be drawn.
    if std::env::var("WIRE_FIRST").is_ok() {
        plan::tidy(board, &mut plan);
        let drawing = draw_with(board, &plan, None, &mut memo).0;
        said("drawn", &plan, Some(&drawing));
        return Some((plan, drawing));
    }
    let mut drawing = settle(board, &mut plan, &mut memo, &said, stop);
    if poured.is_empty() {
        return Some((plan, drawing));
    }
    drawing = pour(board, &mut plan, drawing, &poured, &mut memo, &said, stop);
    if stopped() {
        return None;
    }
    let before: Vec<(ItemId, usize)> = board::Pours::likely(board).starved(board).iter().map(|s| (s.pad, s.layer)).collect();
    let (mut drawing, mut short) = tied(board, &plan, drawing, &before);
    for _ in 0..SPOKES {
        if short == 0 {
            break;
        }
        let bars = shutting(board, &plan, &drawing, &starved(board, &plan, &drawing, &before));
        said(&format!("{short} pads short of spokes, {} wires shutting them in", bars.len()), &plan, None);
        if bars.is_empty() {
            break;
        }
        let mut other = plan.clone();
        plan::bar(board, &mut other, &bars);
        let redrawn = settle(board, &mut other, &mut memo, &said, stop);
        let redrawn = pour(board, &mut other, redrawn, &poured, &mut memo, &said, stop);
        if stopped() {
            return None;
        }
        let (redrawn, left) = tied(board, &other, redrawn, &before);
        if (open(board, &other, &redrawn), left) >= (open(board, &plan, &drawing), short) {
            break;
        }
        (plan, drawing, short) = (other, redrawn, left);
    }
    Some((plan, drawing))
}

/// Plans and draws what the pours leave apart, and then what they leave apart once that is
/// drawn: a wire laid to join one pad can cut the pour another was joined by, or the pour a
/// connection laid before it ends in. A round may leave more open than it found: the plan and
/// the drawing are left as they were when fewest connections were open.
fn pour(board: &Board, plan: &mut Plan, mut drawing: Drawing, poured: &[u32], memo: &mut chain::Memo, said: &dyn Fn(&str, &Plan, Option<&Drawing>), stop: &AtomicBool) -> Drawing {
    // The connection the plan has already for a pair of pads.
    let had = |plan: &Plan, w: &plan::Want| plan.wants.iter().position(|x| x.net == w.net && !x.stub && ((x.from, x.to) == (w.from, w.to) || (x.from, x.to) == (w.to, w.from)));
    // How many were found the round before: a round that finds no fewer is the last.
    let mut found = usize::MAX;
    // The connections laid again once already.
    let mut twice: Vec<usize> = Vec::new();
    // The plan and drawing as they were when fewest connections were open: a round may leave
    // more open than it found.
    let mut best: Option<(usize, Plan, Drawing)> = None;
    for _ in 0..POURS {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let (mut wants, open) = left_by_pours(board, plan, &drawing, poured);
        if best.as_ref().is_none_or(|b| open < b.0) {
            best = Some((open, plan.clone(), drawing.clone()));
        }
        // A connection the plan has already, open all the same: it ended at a via in pour
        // that wires laid since have cut off. It is taken out, so that its own copper counts
        // for nothing, and laid again to where the pour reaches now.
        let mut again: Vec<usize> = wants.iter().filter_map(|w| had(plan, w)).collect();
        again.sort_unstable();
        again.dedup();
        if !again.is_empty() {
            plan::lift_out(board, plan, &again);
            drawing = draw_with(board, plan, None, memo).0;
            wants = left_by_pours(board, plan, &drawing, poured).0;
        }
        let (renewed, rest): (Vec<plan::Want>, Vec<plan::Want>) = wants.into_iter().partition(|w| had(plan, w).is_some());
        let mut renewed: Vec<(usize, plan::Want)> = renewed.into_iter().filter_map(|w| had(plan, &w).map(|c| (c, w))).collect();
        renewed.sort_by_key(|r| r.0);
        renewed.dedup_by_key(|r| r.0);
        said(&format!("poured, {open} open: {} left by the pours and {} to lay again", rest.len(), renewed.len()), plan, None);
        if rest.is_empty() && renewed.is_empty() && again.is_empty() {
            // Nothing done since it was counted: it is the best, or as good.
            return match best {
                Some((fewest, kept, drawn)) if fewest < open => {
                    *plan = kept;
                    drawn
                }
                _ => drawing,
            };
        }
        // One laid again before and open again: its wire moves the wires beside it, and those
        // cut the pour it ends in. It goes all the way to the pad.
        for (c, want) in renewed.iter_mut() {
            if twice.contains(c) {
                want.into.clear();
            } else {
                twice.push(*c);
            }
        }
        let stalled = rest.len() + renewed.len() >= found;
        found = rest.len() + renewed.len();
        plan::renew(board, plan, renewed);
        plan::add(board, plan, rest);
        said("planned", plan, None);
        drawing = settle(board, plan, memo, said, stop);
        if stalled {
            break;
        }
    }
    if let Some((fewest, kept, drawn)) = best
        && fewest < open(board, plan, &drawing)
    {
        (*plan, drawing) = (kept, drawn);
    }
    drawing
}

/// How many times at most the pours are filled to see what they leave apart: fewer where a
/// round finds no less to do than the one before.
const POURS: usize = 6;
/// How many times wires that shut a pad's spokes in are laid another way, and how many
/// spokes a pad may be drawn by hand.
const SPOKES: usize = 3;

/// The board with the drawing's copper on it: the wires as tracks, the plan's vias and then
/// the ties, and which items those are.
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
    for t in &drawing.ties {
        let shape = Shape::Capsule { a: at(t.pts[0]), b: at(t.pts[1]), r: (t.width * 1e6 / 2.0).round() as i64 };
        new.push(out.insert(Item { kind: Kind::Track, net: t.net, layers: 1 << t.layer, shape, hole: None, locked: false, span: None }));
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
    for t in &drawing.ties {
        edit.tracks.push(kicad::write::NewTrack { start: m(t.pts[0]), end: m(t.pts[1]), width: t.width / 1e3, layer: original.layers[t.layer].clone(), net: original.file_net(t.net), locked: false });
    }
    let (r, drill) = plan::via_size(original);
    let (Some(top), Some(bottom)) = (original.layers.first(), original.layers.last()) else { return kicad::write::apply(text, &edit) };
    for v in &plan.vias {
        edit.vias.push(kicad::write::NewVia { at: m(v.at), size: 2.0 * r as f64 / 1e9, drill: 2.0 * drill as f64 / 1e9, layers: [top.clone(), bottom.clone()], net: original.file_net(v.net), locked: false });
    }
    kicad::write::apply(text, &edit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan::strands::Strand;

    /// Three strands side by side in a gap, the last two of one net: those two lie on each
    /// other as one track, so the first and the last are as far apart as two tracks are, and
    /// the first two the same.
    #[test]
    fn strands_of_one_net_beside_each_other_take_one_tracks_room_between_drawn_wires() {
        let text = "(kicad_pcb (version 20240108) (generator \"pcbnew\") (general (thickness 1.6))
            (layers (0 \"F.Cu\" signal) (31 \"B.Cu\" signal) (44 \"Edge.Cuts\" user))
            (net 0 \"\") (net 1 \"A\") (net 2 \"B\")
            (gr_rect (start 0 0) (end 20 20) (layer \"Edge.Cuts\") (width 0.1)))";
        let rules = r#"{"board":{"design_settings":{"rules":{"min_clearance":0.2,"min_track_width":0.1}}},
            "net_settings":{"classes":[{"name":"Default","clearance":0.2,"track_width":0.1,"via_diameter":0.8,"via_drill":0.4}]}}"#;
        let (pcb, copper) = kicad::parse_board(text).unwrap();
        let board = Board::new(&pcb, &copper, kicad::Rules::parse(rules).unwrap());
        let mut sheet = Sheet::new(plan::Space::new(&board, 0, &[], &Default::default()));
        let end = End::Site(0);
        for net in [2, 1, 1] {
            sheet.strands.push(Strand { net, connection: 0, from: end, to: end, width: 0.1, crossings: Vec::new() });
        }
        let on = [(0, 0), (1, 0), (2, 0)];
        assert!((apart(&board, &sheet, &on) - 0.3).abs() < 1e-9, "{}", apart(&board, &sheet, &on));
        assert!((apart(&board, &sheet, &on[..2]) - 0.3).abs() < 1e-9);
        assert!(apart(&board, &sheet, &on[1..]).abs() < 1e-9);
    }
}
