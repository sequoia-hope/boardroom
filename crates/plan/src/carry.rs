//! Carrying the strands onto new triangles. A via is an obstacle only once a connection has put
//! one in, so each time one goes in or comes out every layer is triangulated again, and every
//! strand is followed as a line through the old triangles and read off against the new ones.
//! The lines are where the strands lie ([`Sheet::places`]): they keep their order in every gap,
//! so no two cross, and a new via stands clear of them on the side of each it was found on.

use std::collections::HashMap;
use std::sync::Arc;

use board::Board;

use crate::space::{End, Space, Spots, P};
use crate::strands::{Sheet, Strand};

/// How a line starts or ends.
#[derive(Debug, Clone, Copy)]
pub enum Anchor {
    /// On a side of an obstacle that is in both triangulations: the side's two corners.
    Side(P, P),
    /// In the middle of a via of the plan, by its number in the new triangulation.
    Via(usize),
}

/// A strand to be read against new triangles: what it is (its ends as they are numbered in the
/// new triangulation, its crossings not yet known) and the line it runs along.
#[derive(Debug, Clone)]
pub struct Trace {
    pub strand: Strand,
    pub pts: Vec<P>,
    pub start: Anchor,
    pub end: Anchor,
}

/// Each edge by the places of its two corners, to a nanometre, the lesser first.
type Sides = HashMap<((i64, i64), (i64, i64)), usize>;

fn key(p: P) -> (i64, i64) {
    ((p[0] * 1e6).round() as i64, (p[1] * 1e6).round() as i64)
}

/// Where the segment from `p` to `q` crosses the one from `a` to `b`: how far along each.
fn crossing(p: P, q: P, a: P, b: P) -> Option<(f64, f64)> {
    let (d, r) = ([q[0] - p[0], q[1] - p[1]], [b[0] - a[0], b[1] - a[1]]);
    let den = d[0] * r[1] - d[1] * r[0];
    if den.abs() < 1e-18 {
        return None;
    }
    let w = [a[0] - p[0], a[1] - p[1]];
    let s = (w[0] * r[1] - w[1] * r[0]) / den;
    let u = (w[0] * d[1] - w[1] * d[0]) / den;
    (0.0..=1.0).contains(&u).then_some((s, u))
}

/// Two crossings this near along a segment (as a share of it) are one place.
const NEAR: f64 = 1e-7;

/// The gaps of `space` a line crosses, from where it starts to where it ends, each with how
/// far along the gap (from its first corner) it crosses.
fn read(space: &Space, sides: &Sides, trace: &Trace) -> Option<Vec<(usize, f64)>> {
    let pts = &trace.pts;
    let side = |a: P, b: P| {
        let (a, b) = (key(a), key(b));
        sides.get(&(a.min(b), a.max(b))).copied()
    };
    let along = |e: usize, p: P| {
        let [a, b] = space.edges[e].v.map(|v| space.verts[v]);
        let l2 = (b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2);
        if l2 > 0.0 { ((p[0] - a[0]) * (b[0] - a[0]) + (p[1] - a[1]) * (b[1] - a[1])) / l2 } else { 0.5 }
    };
    let inside = |k: usize| space.sites.get(k).map(|s| s.obstacle);
    let mut out = Vec::new();
    let mut came = None;
    let mut t = match trace.start {
        Anchor::Side(a, b) => {
            let first = side(a, b)?;
            out.push((first, along(first, *pts.first()?)));
            came = Some(first);
            space.edges[first].t.iter().flatten().copied().find(|&t| space.free(t))?
        }
        Anchor::Via(k) => {
            let o = inside(k)?;
            (0..space.tris.len()).find(|&t| space.tris[t].inside == Some(o))?
        }
    };
    let last = match trace.end {
        Anchor::Side(a, b) => Some(side(a, b)?),
        Anchor::Via(_) => None,
    };
    let goal = match trace.end {
        Anchor::Via(k) => Some(inside(k)?),
        Anchor::Side(..) => None,
    };
    if pts.len() == 1 && matches!(trace.start, Anchor::Side(..)) && last == came {
        return Some(out);
    }
    let mut arrived = false;
    'walk: for (i, w) in pts.windows(2).enumerate() {
        let (p, q) = (w[0], w[1]);
        let ending = i + 2 == pts.len();
        // How far along this segment the walk has come. A crossing at the very end of a
        // segment is left to the next, which finds it at its start.
        let mut from = 0.0;
        let until = if ending { 1.0 + NEAR } else { 1.0 - NEAR };
        // No line crosses more gaps than there are.
        for _ in 0..space.edges.len() {
            let next = space.tris[t]
                .e
                .iter()
                .filter_map(|&e| {
                    let [a, b] = space.edges[e].v.map(|v| space.verts[v]);
                    let after = if Some(e) == came { from + NEAR } else { from - NEAR };
                    crossing(p, q, a, b).filter(|&(s, _)| s > after && s <= until).map(|(s, u)| (s, u, e))
                })
                .min_by(|x, y| x.0.total_cmp(&y.0));
            let Some((s, u, e)) = next else { break };
            if ending && Some(e) == last {
                break;
            }
            out.push((e, u));
            t = space.edges[e].t.iter().flatten().copied().find(|&x| x != t)?;
            if !space.free(t) {
                // Into the via it ends in, on the last piece of its line: there it ends.
                if ending && goal.is_some() && space.tris[t].inside == goal {
                    arrived = true;
                    break 'walk;
                }
                return None;
            }
            came = Some(e);
            from = s;
        }
    }
    match last {
        Some(last) => {
            if !space.tris[t].e.contains(&last) {
                return None;
            }
            out.push((last, along(last, pts[pts.len() - 1])));
        }
        None if !arrived => return None,
        None => {}
    }
    // In and straight out again through one gap is no crossing of it.
    let mut kept: Vec<(usize, f64)> = Vec::new();
    for x in out {
        if kept.len() > 1 && kept.last().is_some_and(|l| l.0 == x.0) {
            kept.pop();
        } else {
            kept.push(x);
        }
    }
    Some(kept)
}

/// The laid strands of a sheet as lines, their ends numbered by `number` (which gives a via of
/// the old triangulation its number in the new): none if a strand ends on a via that is going.
pub fn traces(board: &Board, sheet: &Sheet, number: &dyn Fn(usize) -> Option<usize>) -> Option<Vec<Trace>> {
    let places = sheet.places(board);
    let end = |e: End| match e {
        End::Site(k) => number(k).map(End::Site),
        item => Some(item),
    };
    let side = |e: usize| {
        let [a, b] = sheet.space.edges[e].v.map(|v| sheet.space.verts[v]);
        Anchor::Side(a, b)
    };
    sheet
        .laid()
        .map(|s| {
            let strand = &sheet.strands[s];
            let c = &strand.crossings;
            Some(Trace {
                strand: Strand { net: strand.net, connection: strand.connection, from: end(strand.from)?, to: end(strand.to)?, width: strand.width, crossings: Vec::new() },
                pts: c.iter().enumerate().map(|(k, &e)| sheet.point(e, places[s][k])).collect(),
                start: side(*c.first()?),
                end: side(*c.last()?),
            })
        })
        .collect()
}

/// One layer triangulated with `vias`, and the strands of `traces` read off against it: none
/// if a strand cannot be read, or if what was read is not strands that never cross.
pub fn sheet(board: &Board, layer: usize, vias: &[(P, u32)], spots: &Arc<Spots>, traces: &[Trace]) -> Option<Sheet> {
    let space = Space::new(board, layer, vias, spots);
    let sides: Sides = space
        .edges
        .iter()
        .enumerate()
        .map(|(e, edge)| {
            let [a, b] = edge.v.map(|v| key(space.verts[v]));
            ((a.min(b), a.max(b)), e)
        })
        .collect();
    let mut sheet = Sheet::new(space);
    let mut places: Vec<Vec<(f64, usize, usize)>> = vec![Vec::new(); sheet.space.edges.len()];
    for trace in traces {
        let read = read(&sheet.space, &sides, trace)?;
        let id = sheet.strands.len();
        for (k, &(e, u)) in read.iter().enumerate() {
            places[e].push((u, id, k));
        }
        sheet.strands.push(Strand { crossings: read.iter().map(|x| x.0).collect(), ..trace.strand.clone() });
    }
    for (e, mut there) in places.into_iter().enumerate() {
        there.sort_by(|a, b| a.0.total_cmp(&b.0));
        sheet.on[e] = there.into_iter().map(|(_, s, k)| (s, k)).collect();
    }
    sheet.sound(board).then_some(sheet)
}
