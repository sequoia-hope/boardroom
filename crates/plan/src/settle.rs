//! The plan without its scaffolding. The search runs with every via site in each layer's
//! triangulation; the sites no via took are not obstacles, and left in they would decide which
//! side of each a wire passes. Here each layer is triangulated again with only the vias that
//! were placed, and every strand is carried over: followed as a line through the old triangles
//! and read off against the new ones.

use std::collections::HashMap;

use board::Board;

use crate::space::{lerp, End, Space, P};
use crate::strands::{Sheet, Strand, EPS};
use crate::{Plan, Via};

/// A strand as a line: where it crosses each of its gaps, the strands in a gap evenly apart in
/// their order, so no two such lines cross.
fn line(sheet: &Sheet, strand: usize) -> Vec<P> {
    sheet.strands[strand]
        .crossings
        .iter()
        .enumerate()
        .map(|(k, &e)| {
            let on = &sheet.on[e];
            let slot = on.iter().position(|&x| x == (strand, k)).unwrap_or(0);
            let [a, b] = sheet.space.edges[e].v.map(|v| sheet.space.verts[v]);
            lerp(a, b, (slot + 1) as f64 / (on.len() + 1) as f64)
        })
        .collect()
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

/// The gaps of `space` a line crosses, from the side of an obstacle it starts on to the side
/// of the one it ends on, each with how far along the gap (from its first corner) it crosses.
fn read(space: &Space, sides: &Sides, old: &Space, strand: &Strand, pts: &[P]) -> Option<Vec<(usize, f64)>> {
    let side = |e: usize| {
        let [a, b] = old.edges[e].v.map(|v| key(old.verts[v]));
        sides.get(&(a.min(b), a.max(b))).copied()
    };
    let first = side(*strand.crossings.first()?)?;
    let last = side(*strand.crossings.last()?)?;
    let along = |e: usize, p: P| {
        let [a, b] = space.edges[e].v.map(|v| space.verts[v]);
        let l2 = (b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2);
        if l2 > 0.0 { ((p[0] - a[0]) * (b[0] - a[0]) + (p[1] - a[1]) * (b[1] - a[1])) / l2 } else { 0.5 }
    };
    let mut out = vec![(first, along(first, pts[0]))];
    if pts.len() == 1 {
        return Some(out);
    }
    let mut t = space.edges[first].t.iter().flatten().copied().find(|&t| space.free(t))?;
    let mut came = Some(first);
    for (i, w) in pts.windows(2).enumerate() {
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
            if ending && e == last {
                break;
            }
            out.push((e, u));
            t = space.edges[e].t.iter().flatten().copied().find(|&x| x != t)?;
            if !space.free(t) {
                return None;
            }
            came = Some(e);
            from = s;
        }
    }
    if !space.tris[t].e.contains(&last) {
        return None;
    }
    out.push((last, along(last, pts[pts.len() - 1])));
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

/// What became of the plan when its scaffolding went.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct Settled {
    /// Strands that could not be read off against the new triangles: they are lost.
    pub lost: usize,
    /// Gaps that hold more than their room, now that no site divides them.
    pub overfull: usize,
}

/// The plan on triangulations that hold only the vias placed.
pub fn settle(board: &Board, plan: &Plan) -> (Plan, Settled) {
    let Some(top) = plan.sheets.first() else { return (plan.clone(), Settled::default()) };
    // The sites that hold a via, in a new order.
    let mut number: HashMap<usize, usize> = HashMap::new();
    let mut polys = Vec::new();
    let mut vias = Vec::new();
    for v in &plan.vias {
        number.insert(v.site, polys.len());
        vias.push(Via { net: v.net, site: polys.len(), at: v.at });
        polys.push(top.space.obstacles[top.space.sites[v.site].obstacle].poly.clone());
    }
    let end = |e: End| match e {
        End::Site(k) => number.get(&k).map(|&n| End::Site(n)),
        item => Some(item),
    };
    let mut settled = Settled::default();
    let mut sheets = Vec::new();
    for old in &plan.sheets {
        let mut space = Space::new(board, old.space.layer, &polys);
        for v in &vias {
            space.take_site(v.site, Some(v.net));
        }
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
        for s in old.laid() {
            let strand = &old.strands[s];
            let read = read(&sheet.space, &sides, &old.space, strand, &line(old, s));
            let (Some(read), Some(from), Some(to)) = (read, end(strand.from), end(strand.to)) else {
                settled.lost += 1;
                continue;
            };
            let id = sheet.strands.len();
            for (k, &(e, u)) in read.iter().enumerate() {
                places[e].push((u, id, k));
            }
            sheet.strands.push(Strand { net: strand.net, connection: strand.connection, from, to, width: strand.width, crossings: read.iter().map(|x| x.0).collect() });
        }
        for (e, mut there) in places.into_iter().enumerate() {
            there.sort_by(|a, b| a.0.total_cmp(&b.0));
            sheet.on[e] = there.into_iter().map(|(_, s, k)| (s, k)).collect();
        }
        // A side of an obstacle is no gap: a strand that ends on a pad leaves by one.
        let gap = |e: usize| {
            let [a, b] = sheet.space.edges[e].v.map(|v| sheet.space.vert_of[v]);
            a.is_none() || a != b
        };
        settled.overfull += (0..sheet.space.edges.len()).filter(|&e| gap(e) && sheet.need(board, e, None, None) > sheet.space.edges[e].len + EPS).count();
        sheets.push(sheet);
    }
    (Plan { sheets, planes: plan.planes.clone(), vias, left: plan.left.clone() }, settled)
}
