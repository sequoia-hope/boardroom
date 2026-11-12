//! Bundles: two or more nets drawn side by side.
//!
//! Two track segments on one layer run side by side when they are parallel, close, and overlap
//! along their length. Segments joined that way form a straight run; runs that at least two
//! nets pass between at a bend are one bundle.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use board::pcb::geom::{self, P};
use board::pcb::Board;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Bundle {
    pub layer: String,
    pub nets: Vec<String>,
    /// Track length in the bundle, mm, and that over its nets: about how far it runs.
    pub track_mm: f64,
    pub length_mm: f64,
    pub centre: P,
    /// Indices into the board's tracks.
    #[serde(skip)]
    pub tracks: Vec<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Bundles {
    pub bundles: Vec<Bundle>,
    /// Share of the board's track length that is in a bundle.
    pub bundled: f64,
    /// Track length, mm, by how many nets the bundle has.
    pub by_nets: BTreeMap<usize, f64>,
}

/// Parallel within this many degrees.
pub const PARALLEL_DEG: f64 = 4.0;
/// Packed: copper to copper no more than this many of the narrower track's widths, or `GAP_MM`,
/// overlapping along their length by `OVERLAP_MM` and by twice their spacing.
pub const GAP_WIDTHS: f64 = 2.5;
pub const GAP_MM: f64 = 0.4;
pub const OVERLAP_MM: f64 = 1.0;
/// At a pitch: further apart than packed (a bus drawn at its connector's pitch), up to
/// `PITCH_MM` between centres, overlapping by `PITCH_OVERLAP` times the spacing, with no other
/// track between the two.
pub const PITCH_MM: f64 = 2.6;
pub const PITCH_OVERLAP: f64 = 4.0;
/// A bundle shorter than this is not reported.
pub const SHORTEST_MM: f64 = 2.0;

struct Sets(Vec<usize>);
impl Sets {
    fn find(&mut self, mut i: usize) -> usize {
        while self.0[i] != i {
            self.0[i] = self.0[self.0[i]];
            i = self.0[i];
        }
        i
    }
    fn join(&mut self, a: usize, b: usize) {
        let (a, b) = (self.find(a), self.find(b));
        self.0[a] = b;
    }
}

enum Beside {
    No,
    Packed,
    /// At a pitch if nothing lies on this line, midway between the two.
    Pitch(P, P),
}

fn beside(board: &Board, i: usize, j: usize) -> Beside {
    let (a, b) = (&board.tracks[i], &board.tracks[j]);
    let (la, lb) = (geom::dist(a.a, a.b), geom::dist(b.a, b.b));
    let (da, db) = (
        geom::scale(geom::sub(a.b, a.a), 1.0 / la),
        geom::scale(geom::sub(b.b, b.a), 1.0 / lb),
    );
    if geom::cross(da, db).abs() > PARALLEL_DEG.to_radians().sin() {
        return Beside::No;
    }
    let (s0, s1) = (
        geom::dot(geom::sub(b.a, a.a), da),
        geom::dot(geom::sub(b.b, a.a), da),
    );
    let (u0, u1) = (s0.min(s1).max(0.0), s0.max(s1).min(la));
    let overlap = u1 - u0;
    let side = (geom::cross(da, geom::sub(b.a, a.a)) + geom::cross(da, geom::sub(b.b, a.a))) / 2.0;
    let spacing = side.abs();
    let gap = spacing - (a.width + b.width) / 2.0;
    if gap <= (GAP_WIDTHS * a.width.min(b.width)).max(GAP_MM)
        && overlap >= OVERLAP_MM.max(2.0 * spacing)
    {
        return Beside::Packed;
    }
    if spacing <= PITCH_MM && overlap >= OVERLAP_MM.max(PITCH_OVERLAP * spacing) {
        let normal = [-da[1] * side / 2.0, da[0] * side / 2.0];
        let on = |u: f64| geom::add(geom::add(a.a, geom::scale(da, u)), normal);
        return Beside::Pitch(on(u0 + overlap * 0.1), on(u1 - overlap * 0.1));
    }
    Beside::No
}

pub fn bundles(board: &Board) -> Bundles {
    let n = board.tracks.len();
    let live: Vec<usize> = (0..n)
        .filter(|&i| geom::dist(board.tracks[i].a, board.tracks[i].b) > 1e-6)
        .collect();
    // Candidates from a grid per layer.
    const GRID: f64 = 2.0;
    let reach = PITCH_MM + 0.2;
    let mut grid: HashMap<(&str, i64, i64), Vec<usize>> = HashMap::new();
    let span = |i: usize, pad: f64| {
        let t = &board.tracks[i];
        let lo = [t.a[0].min(t.b[0]) - pad, t.a[1].min(t.b[1]) - pad];
        let hi = [t.a[0].max(t.b[0]) + pad, t.a[1].max(t.b[1]) + pad];
        (
            (lo[0] / GRID).floor() as i64,
            (lo[1] / GRID).floor() as i64,
            (hi[0] / GRID).floor() as i64,
            (hi[1] / GRID).floor() as i64,
        )
    };
    for &i in &live {
        let (x0, y0, x1, y1) = span(i, 0.0);
        for x in x0..=x1 {
            for y in y0..=y1 {
                grid.entry((board.tracks[i].layer.as_str(), x, y))
                    .or_default()
                    .push(i);
            }
        }
    }
    let mut runs = Sets((0..n).collect());
    let mut paired = vec![false; n];
    for &i in &live {
        let layer = board.tracks[i].layer.as_str();
        let (x0, y0, x1, y1) = span(i, reach);
        let mut near: Vec<usize> = (x0..=x1)
            .flat_map(|x| (y0..=y1).map(move |y| (x, y)))
            .flat_map(|(x, y)| grid.get(&(layer, x, y)).into_iter().flatten().copied())
            .collect();
        near.sort_unstable();
        near.dedup();
        for &j in near
            .iter()
            .filter(|&&j| j > i && board.tracks[i].net != board.tracks[j].net)
        {
            let together = match beside(board, i, j) {
                Beside::No => false,
                Beside::Packed => true,
                Beside::Pitch(p, q) => !near.iter().any(|&k| {
                    k != i
                        && k != j
                        && geom::seg_seg_dist(p, q, board.tracks[k].a, board.tracks[k].b)
                            < board.tracks[k].width / 2.0 + 0.01
                }),
            };
            if together {
                runs.join(i, j);
                paired[i] = true;
                paired[j] = true;
            }
        }
    }
    // Runs that two or more nets pass between at a shared end point are one bundle.
    let key = |p: P| ((p[0] * 1e3).round() as i64, (p[1] * 1e3).round() as i64);
    // (layer, net, point) -> the paired tracks ending there.
    type End<'a> = (&'a str, &'a str, (i64, i64));
    let mut ends: HashMap<End, Vec<usize>> = HashMap::new();
    for &i in live.iter().filter(|&&i| paired[i]) {
        let t = &board.tracks[i];
        for p in [t.a, t.b] {
            ends.entry((t.layer.as_str(), t.net.as_str(), key(p)))
                .or_default()
                .push(i);
        }
    }
    let mut links: BTreeMap<(usize, usize), BTreeSet<&str>> = BTreeMap::new();
    for ((_, net, _), at) in &ends {
        for &i in at {
            for &j in at {
                let (a, b) = (runs.find(i), runs.find(j));
                if a < b {
                    links.entry((a, b)).or_default().insert(net);
                }
            }
        }
    }
    let mut groups = Sets((0..n).collect());
    for i in 0..n {
        let r = runs.find(i);
        groups.join(i, r);
    }
    for ((a, b), nets) in &links {
        if nets.len() >= 2 {
            groups.join(*a, *b);
        }
    }
    let mut members: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for &i in live.iter().filter(|&&i| paired[i]) {
        members.entry(groups.find(i)).or_default().push(i);
    }
    let total: f64 = board.tracks.iter().map(|t| geom::dist(t.a, t.b)).sum();
    let mut out = Vec::new();
    for tracks in members.into_values() {
        let nets: BTreeSet<&str> = tracks
            .iter()
            .map(|&i| board.tracks[i].net.as_str())
            .collect();
        let track_mm: f64 = tracks
            .iter()
            .map(|&i| geom::dist(board.tracks[i].a, board.tracks[i].b))
            .sum();
        let length_mm = track_mm / nets.len() as f64;
        if nets.len() < 2 || length_mm < SHORTEST_MM {
            continue;
        }
        let mut c = [0.0, 0.0];
        for &i in &tracks {
            let t = &board.tracks[i];
            let l = geom::dist(t.a, t.b);
            c = geom::add(c, geom::scale(geom::add(t.a, t.b), l / 2.0 / track_mm));
        }
        out.push(Bundle {
            layer: board.tracks[tracks[0]].layer.clone(),
            nets: nets.into_iter().map(String::from).collect(),
            track_mm,
            length_mm,
            centre: c,
            tracks,
        });
    }
    out.sort_by(|a, b| b.track_mm.total_cmp(&a.track_mm));
    let mut by_nets = BTreeMap::new();
    for b in &out {
        *by_nets.entry(b.nets.len()).or_default() += b.track_mm;
    }
    let bundled = if total > 0.0 {
        out.iter().map(|b| b.track_mm).sum::<f64>() / total
    } else {
        0.0
    };
    Bundles {
        bundles: out,
        bundled,
        by_nets,
    }
}
