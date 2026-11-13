//! One wire: the path with fewest bends, in the eight directions of a frame, from one end to
//! the other through the gaps its strand crosses, in their order.
//!
//! A straight piece is a direction and an offset, so all the pieces that could come next are a
//! direction and a range of offsets, and where they can be inside the triangle they are in is a
//! convex region. The search goes from such a region to the next: on through the next gap, which
//! narrows the range to the lines that keep their distance from what is on each side of it, or
//! round a bend of 45 degrees anywhere in the region, which starts a new range. Breadth first by
//! bends, so the first ways to reach the far end have the fewest; of those the shortest is taken.

use std::collections::{BinaryHeap, HashMap};

use crate::convex::{add, clip, contains, dot, extent, left, len, scale, strip, sub, sweep, within, P, TOL};

/// A point a wire must pass on one side of, no nearer than `r`. A line that passes it so is
/// free of it. Any other line may still carry a piece of the wire, one that stops short of the
/// point: a corner at the far end of a long gap holds nothing from a wire that turns before it
/// gets there. With no `r` the point is a place on the gap itself, and every line through the
/// gap passes it on its side.
#[derive(Debug, Clone, Copy)]
pub struct Con {
    pub p: P,
    pub r: f64,
    /// On the line's left as it runs (its offset is the greater).
    pub left: bool,
    /// Along the gap, from where the wire crosses towards the point.
    pub toward: P,
}

/// Where a bend may be: `n·p <= c`.
#[derive(Debug, Clone, Copy)]
pub struct Half {
    pub n: P,
    pub c: f64,
}

/// A gap the wire crosses: its two corners, the way across, and what a line through it keeps
/// its distance from.
#[derive(Debug, Clone)]
pub struct Gap {
    pub ends: [P; 2],
    pub forward: P,
    pub cons: Vec<Con>,
}

/// A wire's whole course: `regions[m]` is where it may be after crossing `m` gaps, and
/// `halves[m]` where a bend there may be.
#[derive(Debug, Clone)]
pub struct Course {
    pub a: P,
    pub b: P,
    pub regions: Vec<Vec<P>>,
    pub gaps: Vec<Gap>,
    pub halves: Vec<Vec<Half>>,
    /// What every line keeps its distance from while the wire is still by its first end, and
    /// once it is by its last: the corners of what stands round each end.
    pub starts: Vec<Con>,
    pub ends: Vec<Con>,
    /// No piece is shorter than this where it need not be, mm.
    pub least: f64,
    /// The frame: how far the eight directions are turned from the board's axes, radians.
    pub frame: f64,
}

#[derive(Debug, Clone)]
pub struct Path {
    /// The corners, end to end.
    pub pts: Vec<P>,
    /// The piece between each two gaps: where it crossed the one, its bends, where it crosses the
    /// next (the ends themselves at the first and last).
    pub stages: Vec<Vec<P>>,
    pub length: f64,
}

struct State {
    /// Gaps crossed.
    m: usize,
    d: usize,
    bends: u32,
    /// Where this piece may be in the region it is in.
    w: Vec<P>,
    /// Its offsets.
    i: (f64, f64),
    /// Where the piece may begin: the bend before it, or the wire's first end.
    start: Vec<P>,
    /// What the piece must stop short of, wherever it runs on.
    ahead: Vec<Half>,
    /// No gap crossed since the bend it began at.
    fresh: bool,
    turned: bool,
    parent: Option<usize>,
}

/// No wire has more bends than this.
const BENDS: u32 = 14;
/// How many different ranges are kept for one gap count and direction.
const KEPT: usize = 8;
/// How many ways to the far end are compared for length.
const ENDS: usize = 12;
/// How many pieces are tried in all.
const TRIED: usize = 60_000;

fn allowed<'a>(cons: impl Iterator<Item = &'a Con>, d: P) -> (f64, f64) {
    let n = left(d);
    let (mut lo, mut hi) = (f64::MIN, f64::MAX);
    for c in cons {
        let o = dot(n, c.p);
        if c.left {
            hi = hi.min(o - c.r);
        } else {
            lo = lo.max(o + c.r);
        }
    }
    (lo, hi)
}

/// One way for the lines of a direction to pass what a gap holds them from: their offsets, what
/// a piece on them stops short of, and what it begins beyond.
type Way = ((f64, f64), Vec<Half>, Vec<Half>);

/// The ways lines of direction `d` with offsets `i` may pass `cons`: the lines free of them
/// all, and all the lines that pass the places on the gap, as pieces that stop short of (or
/// begin beyond) each point they are not free of. Each as its offsets, what the piece stops
/// short of, and what it begins beyond.
fn admit(cons: &[Con], d: P, i: (f64, f64)) -> Vec<Way> {
    let Some(placed) = meet(i, allowed(cons.iter().filter(|c| c.r <= 0.0), d)) else { return Vec::new() };
    let mut out = Vec::new();
    let free = meet(placed, allowed(cons.iter().filter(|c| c.r > 0.0), d));
    if let Some(free) = free {
        out.push((free, Vec::new(), Vec::new()));
    }
    if free != Some(placed) {
        let (mut ahead, mut behind) = (Vec::new(), Vec::new());
        for c in cons.iter().filter(|c| c.r > 0.0) {
            if meet(placed, allowed(std::iter::once(c), d)) == Some(placed) {
                continue;
            }
            if dot(d, c.toward) > 0.0 {
                ahead.push(Half { n: d, c: dot(d, c.p) - c.r });
            } else {
                behind.push(Half { n: scale(d, -1.0), c: -dot(d, c.p) - c.r });
            }
        }
        out.push((placed, ahead, behind));
    }
    out
}

fn cut(mut region: Vec<P>, halves: &[Half]) -> Vec<P> {
    for h in halves {
        region = clip(&region, h.n, h.c);
    }
    region
}

fn meet(a: (f64, f64), b: (f64, f64)) -> Option<(f64, f64)> {
    let (lo, hi) = (a.0.max(b.0), a.1.min(b.1));
    if lo <= hi {
        Some((lo, hi))
    } else if lo <= hi + TOL {
        Some(((lo + hi) / 2.0, (lo + hi) / 2.0))
    } else {
        None
    }
}

/// Where a search that found no path got to: how many gaps it crossed, and where the pieces
/// that got that far could be.
#[derive(Debug, Clone)]
pub struct Stuck {
    pub reached: usize,
    pub reach: Vec<Vec<P>>,
}

/// The path, or how far the search got before nothing went on.
pub fn fewest_bends(course: &Course) -> Result<Path, Stuck> {
    let dirs: Vec<P> = (0..8).map(|k| {
        let t = course.frame + std::f64::consts::FRAC_PI_4 * k as f64;
        [t.cos(), t.sin()]
    }).collect();
    let last = course.gaps.len();
    let mut states: Vec<State> = Vec::new();
    let mut open: BinaryHeap<std::cmp::Reverse<(u32, usize)>> = BinaryHeap::new();
    let mut kept: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
    let mut put = |states: &mut Vec<State>, open: &mut BinaryHeap<std::cmp::Reverse<(u32, usize)>>, s: State| {
        if s.w.is_empty() {
            return;
        }
        let there = kept.entry((s.m, s.d)).or_default();
        if there.len() >= KEPT || there.iter().any(|&k| states[k].fresh <= s.fresh && within(&s.w, &states[k].w)) {
            return;
        }
        there.push(states.len());
        open.push(std::cmp::Reverse((s.bends, states.len())));
        states.push(s);
    };
    // What a piece in the region after `m` gaps keeps from, beyond what its gaps say.
    let by_end = |m: usize| -> Vec<Con> {
        let mut cons = Vec::new();
        if m == 0 {
            cons.extend(course.starts.iter().copied());
        }
        if m == last {
            cons.extend(course.ends.iter().copied());
        }
        cons
    };
    // A piece that begins in `start`, in the region after `m` gaps.
    let begin = |m: usize, d: usize, bends: u32, start: &[P], parent: Option<usize>| -> Vec<State> {
        let dir = dirs[d];
        admit(&by_end(m), dir, extent(start, left(dir)))
            .into_iter()
            .filter_map(|(i, ahead, behind)| {
                let start = cut(cut(strip(start, dir, i), &behind), &ahead);
                if start.is_empty() {
                    return None;
                }
                let i = extent(&start, left(dir));
                let w = cut(intersect(&sweep(&start, dir), &course.regions[m]), &ahead);
                Some(State { m, d, bends, w, i, start, ahead, fresh: parent.is_some(), turned: parent.is_some(), parent })
            })
            .collect()
    };
    for d in 0..8 {
        for s in begin(0, d, 0, &[course.a], None) {
            put(&mut states, &mut open, s);
        }
    }
    let mut ends: Vec<usize> = Vec::new();
    let mut best: Option<u32> = None;
    let mut tried = 0;
    let mut reached = 0;
    while let Some(std::cmp::Reverse((bends, k))) = open.pop() {
        if best.is_some_and(|b| bends > b) || tried >= TRIED {
            break;
        }
        tried += 1;
        let (m, d) = (states[k].m, states[k].d);
        reached = reached.max(m);
        let dir = dirs[d];
        if m == last {
            if contains(&states[k].w, course.b, 1e-6) {
                best = Some(bends);
                ends.push(k);
                if ends.len() >= ENDS {
                    break;
                }
                continue;
            }
        } else {
            let gap = &course.gaps[m];
            if dot(dir, gap.forward) > 1e-9 {
                let mut cons = gap.cons.clone();
                cons.extend(by_end(m + 1));
                for (i, ahead, behind) in admit(&cons, dir, states[k].i) {
                    let start = cut(strip(&states[k].start, dir, i), &behind);
                    if start.is_empty() {
                        continue;
                    }
                    let i = extent(&start, left(dir));
                    let mut all = states[k].ahead.clone();
                    all.extend(ahead);
                    let w = cut(strip(&course.regions[m + 1], dir, i), &all);
                    put(&mut states, &mut open, State { m: m + 1, d, bends, w, i, start, ahead: all, fresh: false, turned: false, parent: Some(k) });
                }
            }
        }
        if bends >= BENDS {
            continue;
        }
        // Where the bend may be: in this piece's reach, clear of what a bend here keeps from; a
        // piece that began in this region and ends in it crosses no gap, so it keeps its
        // distance here from what the gaps on both sides would have held it from.
        let mut at = states[k].w.clone();
        if states[k].fresh {
            let mut i = Some(states[k].i);
            for g in [m.checked_sub(1), (m < last).then_some(m)].into_iter().flatten() {
                i = i.and_then(|i| meet(i, allowed(course.gaps[g].cons.iter(), dir)));
            }
            at = i.map_or(Vec::new(), |i| strip(&at, dir, i));
        }
        for h in &course.halves[m] {
            at = clip(&at, h.n, h.c);
        }
        if at.is_empty() {
            continue;
        }
        for turn in [1, 7] {
            for s in begin(m, (d + turn) % 8, bends + 1, &at, Some(k)) {
                put(&mut states, &mut open, s);
            }
        }
    }
    ends.iter().filter_map(|&k| trace(course, &dirs, &states, k)).min_by(|a, b| a.length.total_cmp(&b.length)).ok_or_else(|| Stuck { reached, reach: states.iter().filter(|s| s.m == reached).map(|s| s.w.clone()).collect() })
}

/// What two convex regions share.
fn intersect(a: &[P], b: &[P]) -> Vec<P> {
    if b.len() < 3 {
        return if b.iter().all(|&p| contains(a, p, 1e-6)) { b.to_vec() } else { Vec::new() };
    }
    let turn: f64 = (0..b.len()).map(|i| crate::convex::cross(b[i], b[(i + 1) % b.len()])).sum();
    let mut out = a.to_vec();
    for i in 0..b.len() {
        let (p, q) = (b[i], b[(i + 1) % b.len()]);
        // Inside is on the left of each side of an anticlockwise polygon.
        let mut n = left(sub(q, p));
        let l = len(n);
        if l <= 0.0 {
            continue;
        }
        n = scale(n, if turn >= 0.0 { -1.0 / l } else { 1.0 / l });
        out = clip(&out, n, dot(n, p));
    }
    out
}

/// The way that ends in state `k`, as points: from the far end back, each piece's line is known,
/// and its bend is put as late as it may be.
fn trace(course: &Course, dirs: &[P], states: &[State], k: usize) -> Option<Path> {
    enum Event {
        Bend(P),
        Cross(P),
    }
    let mut back: Vec<Event> = Vec::new();
    let mut at = course.b;
    let mut k = k;
    loop {
        let dir = dirs[states[k].d];
        let n = left(dir);
        let o = dot(n, at);
        // Back along this piece to where it began, noting the gaps it crossed. Where it may
        // begin is what its last state knows: each gap it crossed may have narrowed that.
        let begins = &states[k].start;
        let mut first = k;
        while !states[first].turned {
            let Some(parent) = states[first].parent else { break };
            let gap = &course.gaps[states[first].m - 1];
            let (u, v) = (gap.ends[0], gap.ends[1]);
            let across = dot(n, sub(v, u));
            if across.abs() < 1e-12 {
                return None;
            }
            back.push(Event::Cross(add(u, scale(sub(v, u), (o - dot(n, u)) / across))));
            first = parent;
        }
        if !states[first].turned {
            break;
        }
        let on = clip(&strip(begins, dir, (o, o)), dir, dot(dir, at));
        if on.is_empty() {
            return None;
        }
        // As late as it may be: wires that run together then turn together, each as near the
        // next as it is allowed.
        let (early, late) = extent(&on, dir);
        let late = late.min(dot(dir, at) - course.least).max(early);
        let bend = add(scale(dir, late), scale(n, o));
        back.push(Event::Bend(bend));
        at = bend;
        k = states[first].parent?;
    }
    let mut pts = vec![course.a];
    let mut stages = vec![vec![course.a]];
    for e in back.iter().rev() {
        match *e {
            Event::Bend(p) => {
                if len(sub(p, *pts.last().unwrap())) > 1e-6 {
                    pts.push(p);
                }
                stages.last_mut().unwrap().push(p);
            }
            Event::Cross(p) => {
                stages.last_mut().unwrap().push(p);
                stages.push(vec![p]);
            }
        }
    }
    if len(sub(course.b, *pts.last().unwrap())) > 1e-6 {
        pts.push(course.b);
    }
    stages.last_mut().unwrap().push(course.b);
    let length = pts.windows(2).map(|w| len(sub(w[1], w[0]))).sum();
    Some(Path { pts, stages, length })
}
