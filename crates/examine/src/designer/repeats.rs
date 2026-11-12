//! Repeats: parts of one footprint whose own nets are routed away from them the same way, as
//! when a designer draws one cell and stamps it.
//!
//! Each part's routing is put in the part's own frame (its position and rotation undone) and
//! marked on a coarse grid; two parts repeat when nearly every mark of each has a mark of the
//! other beside it.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use board::pcb::geom::{self, P};
use board::pcb::Board;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Repeat {
    pub footprint: String,
    pub back: bool,
    pub pads: usize,
    /// References of the parts routed alike.
    pub members: Vec<String>,
    /// Parts with this footprint on this side.
    pub of: usize,
    /// Mean, over the members, of each one's likeness to the member it is most like, 0 to 1.
    pub likeness: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Repeats {
    pub repeats: Vec<Repeat>,
    /// Parts with at least `PADS` pads, and how many of them are in a repeat.
    pub parts: usize,
    pub repeated: usize,
}

/// Parts with fewer pads are not looked at: two-pad parts are routed alike by accident.
pub const PADS: usize = 3;
pub const GRID_MM: f64 = 0.25;
/// Two parts repeat at this likeness.
pub const ALIKE: f64 = 0.8;
/// A part with less routing than this many marks has nothing to compare.
pub const MARKS: usize = 12;

type Marks = HashSet<(usize, i32, i32)>;

/// The part's own nets' tracks and vias near it, in its frame.
fn marks(board: &Board, reference: &str) -> Marks {
    let part = &board.parts[reference];
    let nets: BTreeSet<&str> = part
        .pads
        .iter()
        .map(|&i| board.pads[i].net.as_str())
        .filter(|n| !n.is_empty())
        .collect();
    let extent = part
        .pads
        .iter()
        .map(|&i| geom::dist(board.pads[i].centre, part.at))
        .fold(0.0, f64::max);
    let radius = extent + (extent / 2.0).max(1.5);
    let local = |p: P| geom::rotate(geom::sub(p, part.at), -part.rotation);
    let cell = |p: P| {
        (
            (p[0] / GRID_MM).round() as i32,
            (p[1] / GRID_MM).round() as i32,
        )
    };
    let mut out = Marks::new();
    for t in board
        .tracks
        .iter()
        .filter(|t| nets.contains(t.net.as_str()))
    {
        if geom::point_seg_dist(part.at, t.a, t.b) > radius {
            continue;
        }
        let layer = board.layers.iter().position(|l| *l == t.layer).unwrap_or(0);
        let n = ((geom::dist(t.a, t.b) / (GRID_MM / 2.0)).ceil() as usize).max(1);
        for k in 0..=n {
            let p = geom::add(t.a, geom::scale(geom::sub(t.b, t.a), k as f64 / n as f64));
            if geom::dist(p, part.at) <= radius {
                let (x, y) = cell(local(p));
                out.insert((layer, x, y));
            }
        }
    }
    for v in board
        .vias
        .iter()
        .filter(|v| nets.contains(v.net.as_str()) && geom::dist(v.at, part.at) <= radius)
    {
        let (x, y) = cell(local(v.at));
        out.insert((usize::MAX, x, y));
    }
    out
}

fn covered(a: &Marks, b: &Marks) -> f64 {
    let hit = a
        .iter()
        .filter(|(l, x, y)| (-1..=1).any(|dx| (-1..=1).any(|dy| b.contains(&(*l, x + dx, y + dy)))))
        .count();
    hit as f64 / a.len().max(1) as f64
}

pub fn likeness(a: &Marks, b: &Marks) -> f64 {
    covered(a, b).min(covered(b, a))
}

pub fn repeats(board: &Board) -> Repeats {
    let mut kinds: BTreeMap<(&str, bool), Vec<&str>> = BTreeMap::new();
    let mut parts = 0;
    for p in board.parts.values().filter(|p| p.pads.len() >= PADS) {
        parts += 1;
        kinds
            .entry((p.footprint.as_str(), p.back))
            .or_default()
            .push(p.reference.as_str());
    }
    let mut out = Vec::new();
    for ((footprint, back), refs) in kinds {
        if refs.len() < 2 {
            continue;
        }
        // Parts alike are joined, and so are parts each alike to a third: a row of cells whose
        // ends differ a little from its middle is still one repeat.
        let routed: Vec<(&str, Marks)> = refs
            .iter()
            .map(|r| (*r, marks(board, r)))
            .filter(|(_, m)| m.len() >= MARKS)
            .collect();
        let mut set: Vec<usize> = (0..routed.len()).collect();
        fn find(set: &mut [usize], mut i: usize) -> usize {
            while set[i] != i {
                set[i] = set[set[i]];
                i = set[i];
            }
            i
        }
        let mut best = vec![0.0f64; routed.len()];
        for i in 0..routed.len() {
            for j in i + 1..routed.len() {
                let l = likeness(&routed[i].1, &routed[j].1);
                if l >= ALIKE {
                    let (a, b) = (find(&mut set, i), find(&mut set, j));
                    set[a.max(b)] = a.min(b);
                    best[i] = best[i].max(l);
                    best[j] = best[j].max(l);
                }
            }
        }
        let mut clusters: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for i in 0..routed.len() {
            clusters.entry(find(&mut set, i)).or_default().push(i);
        }
        for members in clusters.into_values().filter(|m| m.len() >= 2) {
            out.push(Repeat {
                footprint: footprint.to_string(),
                back,
                pads: board.parts[routed[members[0]].0].pads.len(),
                likeness: members.iter().map(|&i| best[i]).sum::<f64>() / members.len() as f64,
                of: refs.len(),
                members: members
                    .into_iter()
                    .map(|i| routed[i].0.to_string())
                    .collect(),
            });
        }
    }
    out.sort_by_key(|b| std::cmp::Reverse(b.members.len() * b.pads));
    let repeated = out.iter().map(|r| r.members.len()).sum();
    Repeats {
        repeats: out,
        parts,
        repeated,
    }
}
