//! One wire as a chain of straight pieces, each on a line that is held by something: it passes
//! through one of the wire's ends, or it is as near a corner as the wire may come to it. Bends
//! are where two such lines meet. Of the chains that get from one end to the other, clear of
//! every corner on the side the plan passes it, the shortest is taken, each bend counted as
//! [`BEND`] of length. Chains that cost the same differ only in where they turn, and of those
//! the one is taken whose bends lie furthest one way across the board: the same way for every
//! wire, whichever end it is drawn from, so wires that run together turn together.
//!
//! How near a wire may come to a corner is the corner's own distance and the room of the
//! strands the plan has between the two. So wires that pass a corner together are held by it
//! at even pitch, on parallel lines, whatever order they are drawn in: a bundle comes out
//! nested, and no wire takes the room of one not drawn yet.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use crate::convex::{add, cross, dot, left, len, scale, sub, P};

/// A corner the wire passes: no nearer than `r`, and on the side told.
#[derive(Debug, Clone, Copy)]
pub struct Corner {
    pub at: P,
    pub r: f64,
    /// On the wire's left as it runs.
    pub left: bool,
}

/// A side of an obstacle the wire keeps `r` from.
#[derive(Debug, Clone, Copy)]
pub struct Wall {
    pub a: P,
    pub b: P,
    pub r: f64,
}

/// A gap the wire crosses: its two corners, how far along it (as shares, from the first) the
/// wire's middle may cross, and the way across.
#[derive(Debug, Clone, Copy)]
pub struct Gap {
    pub ends: [P; 2],
    pub lo: f64,
    pub hi: f64,
    pub forward: P,
    /// Where the strand lies in it, in the plan.
    pub mid: P,
}

/// What the wire keeps from between two of its gaps: the corners of both, no nearer than each
/// allows, and the sides of obstacles round the triangle between. Which side of a corner the
/// wire passes is told by where it crosses the gaps.
#[derive(Debug, Clone, Default)]
pub struct Stage {
    pub corners: Vec<Corner>,
    pub walls: Vec<Wall>,
}

/// The room in front of a corner that the strands between it and the wire have: the half of the
/// disc of radius `r` about `at` that faces along the gap (`along`, a unit vector from the
/// corner). The wire keeps out of it wherever it is, not only as it crosses the gap: a wire that
/// leaves one pin of a row and runs under the next pins comes near their corners long before it
/// crosses the gaps that end on them, and the wires of those pins come out between.
#[derive(Debug, Clone, Copy)]
pub struct Front {
    pub at: P,
    pub r: f64,
    pub along: P,
}

/// A wire's course: its ends, the gaps it crosses in order, and what it passes on the way.
#[derive(Debug, Clone, Default)]
pub struct Course {
    pub a: P,
    pub b: P,
    pub gaps: Vec<Gap>,
    /// One more than the gaps: `stages[m]` is what holds the wire once it has crossed `m`.
    pub stages: Vec<Stage>,
    /// What holds the wire wherever it is, each corner on the side told: what it has been told
    /// of since its course was made.
    pub corners: Vec<Corner>,
    /// Corners that hold lines and nothing more: those of the pads the wire ends on, which it
    /// need not keep from, at the distance the strands between leave it.
    pub holds: Vec<Corner>,
    /// What the wire keeps out of wherever it is: the room in front of each corner of its gaps.
    pub fronts: Vec<Front>,
    /// The pieces of wires already drawn that the plan has beside this one, each with the
    /// nearest the middles of the two may be and whether it is on this wire's left: the line
    /// along a piece at that distance is one the wire may take, and a piece on such a line
    /// costs less ([`APART`]), so a wire runs beside its neighbour where it loses nothing by it.
    pub beside: Vec<(P, P, f64, bool)>,
    pub walls: Vec<Wall>,
    /// Where a bend may be: inside one of these triangles. Empty for anywhere.
    pub within: Vec<[P; 3]>,
    /// The directions the wire may leave its first end in, and come to its last end in, as
    /// numbers of eighths of a turn from the frame's first axis: empty for any. A long pad is
    /// left along its length.
    pub out: Vec<usize>,
    pub into: Vec<usize>,
    /// How long the first piece is at least where its direction is told, and the last: enough
    /// to be clear of the pad before the wire turns.
    pub stub: [f64; 2],
    /// No piece between two bends is shorter than this, mm.
    pub least: f64,
    /// How far the directions are turned from the board's axes, radians.
    pub frame: f64,
}

/// Chains already searched for, each by its course: the search finds the same chain for the
/// same course, and a board planned again after a small change has most of its courses as
/// they were.
#[derive(Default)]
pub struct Memo(std::collections::HashMap<String, Result<Path, usize>>);

impl Memo {
    /// [`chain`], searched for only where this course has not been before.
    pub fn chain(&mut self, course: &Course, most: u32) -> Result<Path, usize> {
        self.0.entry(format!("{most} {course:?}")).or_insert_with(|| chain(course, most)).clone()
    }
}

#[derive(Debug, Clone)]
pub struct Path {
    /// The corners, end to end.
    pub pts: Vec<P>,
    pub length: f64,
}

/// Lengths this much short still clear: sums of rule values are not exact in floating point.
const TOL: f64 = 1e-6;
/// How many pieces are tried in all.
const TRIED: usize = 60_000;
/// What a bend costs, as length, mm.
const BEND: f64 = 0.5;
/// What a piece costs more, as a share of its length, when it is not on a line right beside a
/// wire already drawn. People run tracks that go the same way side by side, as near as the
/// rules let them; unless a net is told to keep clear of others, so does this.
const APART: f64 = 0.02;
/// A line is tried for a bend only where the wire has not gone more than this many gaps past
/// the last corner that holds it.
const NEAR: usize = 3;
/// What a bend costs more for each millimetre it lies along [`TILT`]: next to nothing.
const ALONG: f64 = 1e-5;
/// The way across the board that bends are counted along: no direction a wire runs in.
const TILT: P = [1.0, 0.618];

/// The least a wire in the eight directions can be from `p` to `q`.
fn least(p: P, q: P) -> f64 {
    let (dx, dy) = ((q[0] - p[0]).abs(), (q[1] - p[1]).abs());
    dx.max(dy) + (std::f64::consts::SQRT_2 - 1.0) * dx.min(dy)
}

pub fn point_segment(p: P, a: P, b: P) -> f64 {
    let d = sub(b, a);
    let l2 = dot(d, d);
    let t = if l2 > 0.0 { (dot(sub(p, a), d) / l2).clamp(0.0, 1.0) } else { 0.0 };
    len(sub(p, add(a, scale(d, t))))
}

fn segments_cross(a: P, b: P, c: P, d: P) -> bool {
    let side = |p: P, q: P, r: P| cross(sub(q, p), sub(r, p));
    side(a, b, c) * side(a, b, d) < 0.0 && side(c, d, a) * side(c, d, b) < 0.0
}

fn segment_segment(a: P, b: P, c: P, d: P) -> f64 {
    if segments_cross(a, b, c, d) {
        return 0.0;
    }
    point_segment(a, c, d).min(point_segment(b, c, d)).min(point_segment(c, a, b)).min(point_segment(d, a, b))
}

impl Course {
    /// How far a piece may run from `p`, where the wire has crossed `stage` gaps, in direction
    /// `d` (a unit): the stretches it runs through, each as how far along it ends and how many
    /// gaps the wire has crossed in it. It ends where it would come too near a corner or a
    /// wall, pass a corner it has been told of on the wrong side, or cross the next gap where
    /// the wire may not; nowhere, if it starts back over the gap just crossed.
    pub fn reach(&self, p: P, d: P, stage: usize) -> Vec<(f64, usize)> {
        let mut out = Vec::new();
        let (mut m, mut from) = (stage, 0.0);
        // Where a ray from `o` first comes within `r` of a point: none if it never does.
        let disc = |o: P, at: P, r: f64| -> Option<f64> {
            let w = sub(at, o);
            let (along, off) = (dot(w, d), cross(d, w).abs());
            if off >= r - TOL {
                return None;
            }
            let t = along - (r * r - off * off).max(0.0).sqrt();
            // Already inside, or the disc is behind.
            if dot(w, w).sqrt() < r - TOL { Some(0.0) } else if t >= 0.0 { Some(t) } else { None }
        };
        let wall = |o: P, w: &Wall| -> Option<f64> {
            let mut first = [disc(o, w.a, w.r), disc(o, w.b, w.r)].into_iter().flatten().fold(None, |x: Option<f64>, t| Some(x.map_or(t, |x| x.min(t))));
            let e = sub(w.b, w.a);
            let l = len(e);
            if l > 0.0 {
                let n = scale(left(e), 1.0 / l);
                // The two lines `r` off the wall, between its ends.
                for side in [w.r - TOL, -(w.r - TOL)] {
                    let den = dot(d, n);
                    if den.abs() < 1e-12 {
                        continue;
                    }
                    let t = (side - dot(sub(o, w.a), n)) / den;
                    let along = dot(sub(add(o, scale(d, t)), w.a), e) / (l * l);
                    // Coming towards the wall from outside it.
                    if t >= 0.0 && (0.0..=1.0).contains(&along) && den * side < 0.0 {
                        first = Some(first.map_or(t, |x: f64| x.min(t)));
                    }
                }
                if segment_segment(o, o, w.a, w.b) < w.r - TOL {
                    first = Some(0.0);
                }
            }
            first
        };
        // Out of the box the wire may bend in.
        let mut far = f64::MAX;
        if !self.within.is_empty() {
            let (mut lo, mut hi) = ([f64::MAX; 2], [f64::MIN; 2]);
            for q in self.within.iter().flatten() {
                for i in 0..2 {
                    lo[i] = lo[i].min(q[i]);
                    hi[i] = hi[i].max(q[i]);
                }
            }
            for i in 0..2 {
                if d[i] > 1e-12 {
                    far = far.min((hi[i] - p[i]) / d[i]);
                } else if d[i] < -1e-12 {
                    far = far.min((lo[i] - p[i]) / d[i]);
                }
            }
        }
        if let Some(g) = m.checked_sub(1).map(|k| &self.gaps[k])
            && dot(d, g.forward) < 0.0
            && segments_cross(p, add(p, scale(d, 1e-6)), g.ends[0], g.ends[1])
        {
            return out;
        }
        loop {
            let o = add(p, scale(d, from));
            // The first thing this stretch would come too near, or pass on the wrong side.
            let here = &self.stages[m];
            let mut stop = far - from;
            for c in &here.corners {
                if let Some(t) = disc(o, c.at, c.r) {
                    stop = stop.min(t);
                }
            }
            for w in here.walls.iter().chain(&self.walls) {
                if let Some(t) = wall(o, w) {
                    stop = stop.min(t);
                }
            }
            for f in &self.fronts {
                // Where the stretch is within the disc, and where in front of the corner.
                let w = sub(f.at, o);
                let along = dot(w, d);
                let off = dot(w, w) - along * along;
                let r = f.r - TOL;
                if r <= 0.0 || off >= r * r {
                    continue;
                }
                let half = (r * r - off).sqrt();
                let (mut t0, mut t1) = ((along - half).max(0.0), along + half);
                let (h0, hd) = (-dot(w, f.along), dot(d, f.along));
                if hd > 1e-12 {
                    t0 = t0.max(-h0 / hd);
                } else if hd < -1e-12 {
                    t1 = t1.min(-h0 / hd);
                } else if h0 < 0.0 {
                    continue;
                }
                if t0 < t1 - 1e-9 {
                    stop = stop.min(t0);
                }
            }
            for c in &self.corners {
                if let Some(t) = disc(o, c.at, c.r) {
                    stop = stop.min(t);
                }
                let abeam = dot(sub(c.at, o), d);
                if abeam >= 0.0 && (cross(d, sub(c.at, o)) > 0.0) != c.left {
                    stop = stop.min(abeam);
                }
            }
            // The next gap, where the ray crosses it.
            let next = self.gaps.get(m).and_then(|g| {
                let e = sub(g.ends[1], g.ends[0]);
                let den = cross(d, e);
                if den.abs() < 1e-12 {
                    return None;
                }
                let w = sub(g.ends[0], o);
                let (t, u) = (cross(w, e) / den, cross(w, d) / den);
                (t > 1e-9 && (-1e-9..=1.0 + 1e-9).contains(&u)).then_some((t, u, g))
            });
            match next {
                Some((t, u, g)) if t <= stop => {
                    out.push((from + t, m));
                    if dot(d, g.forward) <= 0.0 || u < g.lo - 1e-6 || u > g.hi + 1e-6 {
                        return out;
                    }
                    m += 1;
                    from += t;
                }
                _ => {
                    out.push((from + stop.max(0.0), m));
                    return out;
                }
            }
        }
    }
}

struct State {
    line: usize,
    at: P,
    /// Gaps crossed.
    stage: usize,
    cost: f64,
    bends: u32,
    parent: Option<usize>,
}

struct Open {
    order: f64,
    state: usize,
}
impl PartialEq for Open {
    fn eq(&self, o: &Self) -> bool {
        self.cmp(o) == Ordering::Equal
    }
}
impl Eq for Open {}
impl Ord for Open {
    fn cmp(&self, o: &Self) -> Ordering {
        o.order.total_cmp(&self.order).then(o.state.cmp(&self.state))
    }
}
impl PartialOrd for Open {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

/// The cheapest chain in the eight directions with no more than `most` bends: with no such
/// chain, how many gaps the search got across.
pub fn chain(course: &Course, most: u32) -> Result<Path, usize> {
    const TURNS: usize = 8;
    let dirs: Vec<P> = (0..TURNS).map(|k| {
        let t = course.frame + std::f64::consts::TAU * k as f64 / TURNS as f64;
        [t.cos(), t.sin()]
    }).collect();
    // The lines: each a direction, an offset to its left, the stages whose corners hold it
    // (every stage, for a line through an end or held by what the wire was told of since),
    // and whether it lies right beside a wire already drawn.
    let mut lines: Vec<(usize, f64, usize, usize, bool)> = Vec::new();
    let mut known: HashMap<(usize, i64), usize> = HashMap::new();
    let mut put = |d: usize, o: f64, stage: Option<usize>, beside: bool, lines: &mut Vec<(usize, f64, usize, usize, bool)>| {
        let (lo, hi) = stage.map_or((0, usize::MAX), |s| (s, s));
        match known.get(&(d, (o * 1e7).round() as i64)) {
            Some(&k) => {
                lines[k].2 = lines[k].2.min(lo);
                lines[k].3 = lines[k].3.max(hi);
                lines[k].4 |= beside;
            }
            None => {
                known.insert((d, (o * 1e7).round() as i64), lines.len());
                lines.push((d, o, lo, hi, beside));
            }
        }
    };
    for (d, &dir) in dirs.iter().enumerate() {
        // As near a corner as the wire may come, the corner on its side of the line.
        let held = |c: &Corner| dot(left(dir), c.at) + if c.left { -c.r } else { c.r };
        put(d, dot(left(dir), course.a), None, false, &mut lines);
        put(d, dot(left(dir), course.b), None, false, &mut lines);
        // Through where a piece told its direction may first turn, and last: so a wire in the
        // open, with no corner to hold it, can turn as soon as it is clear of its pad.
        for &k in &course.out {
            put(d, dot(left(dir), add(course.a, scale(dirs[k % TURNS], course.stub[0]))), None, false, &mut lines);
        }
        for &k in &course.into {
            put(d, dot(left(dir), sub(course.b, scale(dirs[k % TURNS], course.stub[1]))), None, false, &mut lines);
        }
        for c in course.corners.iter().chain(&course.holds) {
            put(d, held(c), None, false, &mut lines);
        }
        // Along each piece of a neighbour, in the piece's own direction or against it.
        for &(a, b, r, left) in &course.beside {
            let along = sub(b, a);
            if len(along) > 1e-9 && cross(dir, along).abs() < 1e-6 * len(along) {
                put(d, held(&Corner { at: a, r, left }), None, true, &mut lines);
            }
        }
        for (m, stage) in course.stages.iter().enumerate() {
            for c in &stage.corners {
                put(d, held(c), Some(m), false, &mut lines);
            }
        }
    }
    let through = |k: usize, p: P| (dot(left(dirs[lines[k].0]), p) - lines[k].1).abs() < 1e-7;
    let mut by_dir: Vec<Vec<usize>> = vec![Vec::new(); TURNS];
    for (k, l) in lines.iter().enumerate() {
        by_dir[l.0].push(k);
    }
    // Where the count along the tilt starts: before anything the wire may reach.
    let from = course.stages.iter().flat_map(|s| &s.corners).map(|c| c.at).chain(course.within.iter().flatten().copied()).chain([course.a, course.b]).map(|p| dot(TILT, p)).fold(f64::MAX, f64::min);
    // How far the wire still has to go from a place where it has crossed so many gaps: to
    // where the strand lies in the next gap, and on from gap to gap as the strand lies. The
    // search goes by this, so that it follows the course and does not try every way back.
    let mut tail = vec![0.0; course.gaps.len() + 1];
    for m in (0..course.gaps.len()).rev() {
        tail[m] = tail[m + 1] + least(course.gaps[m].mid, course.gaps.get(m + 1).map_or(course.b, |g| g.mid));
    }
    let to_go = |at: P, stage: usize| course.gaps.get(stage).map_or(least(at, course.b), |g| least(at, g.mid) + tail[stage]);
    let mut states: Vec<State> = Vec::new();
    let mut heap = BinaryHeap::new();
    let mut best: HashMap<(usize, i64, usize), f64> = HashMap::new();
    for (k, l) in lines.iter().enumerate() {
        if through(k, course.a) && (course.out.is_empty() || course.out.contains(&l.0)) {
            heap.push(Open { order: to_go(course.a, 0), state: states.len() });
            states.push(State { line: k, at: course.a, stage: 0, cost: 0.0, bends: 0, parent: None });
        }
    }
    // The cheapest chain found to the far end, and the state its last piece begins at.
    let mut done: Option<(f64, usize)> = None;
    let mut found = false;
    let mut tried = 0;
    while let Some(Open { order, state: k }) = heap.pop() {
        if done.is_some_and(|d| d.0 <= order) || tried >= TRIED {
            break;
        }
        tried += 1;
        let (line, at, stage, cost, bends) = (states[k].line, states[k].at, states[k].stage, states[k].cost, states[k].bends);
        let d = lines[line].0;
        let dir = dirs[d];
        // How far this piece may run, and how many gaps it has crossed by where.
        let reach = course.reach(at, dir, stage);
        let Some(&(far, _)) = reach.last() else { continue };
        let stage_at = |t: f64| reach.iter().find(|r| t <= r.0 + 1e-9).map(|r| r.1);
        // On to the far end, if this line runs through it.
        if through(line, course.b) && (course.into.is_empty() || course.into.contains(&d)) {
            let t = dot(dir, sub(course.b, at));

            let stub = if course.into.is_empty() { -1e-9 } else { course.stub[1] - 1e-9 };
            let whole = cost + t + if lines[line].4 { 0.0 } else { APART * t };
            if t >= stub && t <= far + 1e-9 && stage_at(t) == Some(course.gaps.len()) && done.is_none_or(|d| whole < d.0) {
                // The first chain to the far end was found by following the plan's line, which
                // tells what is still to go only roughly. From here the search is for a
                // cheaper chain, and goes by the least that can still be to go: everything
                // that could not be cheaper is dropped.
                if !found {
                    found = true;
                    let kept: Vec<Open> = heap.drain().map(|o| Open { order: states[o.state].cost + least(states[o.state].at, course.b), state: o.state }).filter(|o| o.order < whole).collect();
                    heap.extend(kept);
                }
                done = Some((whole, k));
            }
        }
        if bends >= most {
            continue;
        }
        let short = if states[k].parent.is_some() {
            course.least
        } else if course.out.is_empty() {
            1e-6
        } else {
            course.stub[0] - 1e-9
        };
        for turn in [1, TURNS - 1] {
            let d2 = (d + turn) % TURNS;
            let (dir2, n2) = (dirs[d2], left(dirs[d2]));
            let across = dot(n2, dir);
            for &k2 in &by_dir[d2] {
                // Not a line held only by what the wire has long passed.
                if lines[k2].3.saturating_add(NEAR) < stage {
                    continue;
                }
                // Where this line meets that one, and how far on along this one that is.
                let t = (lines[k2].1 - dot(n2, at)) / across;
                if t < short || t > far + 1e-9 {
                    continue;
                }
                let Some(stage) = stage_at(t) else { continue };
                if lines[k2].3.saturating_add(NEAR) < stage {
                    continue;
                }
                let meet = add(at, scale(dir, t));
                let apart = if lines[line].4 { 0.0 } else { APART * t };
                let cost = cost + t + apart + BEND + ALONG * (dot(TILT, meet) - from).max(0.0);
                let key = (k2, (dot(dir2, meet) * 1e6).round() as i64, stage);
                if best.get(&key).is_some_and(|&b| b <= cost) {
                    continue;
                }
                best.insert(key, cost);
                heap.push(Open { order: cost + if found { least(meet, course.b) } else { to_go(meet, stage) }, state: states.len() });
                states.push(State { line: k2, at: meet, stage, cost, bends: bends + 1, parent: Some(k) });
            }
        }
    }
    let Some((_, k)) = done else {
        return Err(states.iter().map(|s| s.stage).max().unwrap_or(0));
    };
    let mut pts = vec![course.b];
    let mut at = Some(k);
    while let Some(k) = at {
        if pts.last().is_none_or(|&p| len(sub(states[k].at, p)) > 1e-6) {
            pts.push(states[k].at);
        }
        at = states[k].parent;
    }
    pts.reverse();
    let length = pts.windows(2).map(|w| len(sub(w[1], w[0]))).sum();
    Ok(Path { pts, length })
}
