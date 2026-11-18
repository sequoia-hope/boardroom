//! A pour's pieces after a refill, and where each piece is narrow: what a block's band or ring
//! still is once other nets' vias and tracks have been let through it. Each piece of a zone's
//! fill on a layer, the pads, vias and tracks of its net it joins, and its necks: the narrowest
//! copper between two broad parts of it, where it would come apart first, with the copper of
//! other nets that pinches it there. Good to a raster step, as the fill is.

use crate::fill::{Pours, STEP};
use crate::geom::{BBox, Nm, Point, Shape};
use crate::model::{Board, ItemId, Kind};

/// One piece of a zone's fill on one layer.
#[derive(Clone, Debug, PartialEq)]
pub struct PourPiece {
    /// The zone, as numbered among the file's, its net and the layer.
    pub zone: usize,
    pub net: u32,
    pub layer: usize,
    /// Its number among the pieces of every pour ([`Pours::pieces`]).
    pub piece: usize,
    /// Its copper in square nanometres, and a point at the middle of its broadest part.
    pub area: f64,
    pub at: Point,
    /// The copper of its net it joins.
    pub pads: Vec<ItemId>,
    pub vias: Vec<ItemId>,
    pub tracks: Vec<ItemId>,
    /// Where it is narrow, narrowest first: one for each place it would come apart, so a piece
    /// of three broad parts has two.
    pub necks: Vec<Neck>,
}

/// Where a piece of pour is narrow between two broad parts of it.
#[derive(Clone, Debug, PartialEq)]
pub struct Neck {
    /// The copper's width there.
    pub width: Nm,
    pub at: Point,
    /// The other copper nearest it there, nearest first, with its gap from the neck's middle:
    /// what pinches it.
    pub by: Vec<(ItemId, Nm)>,
    /// Narrower than the zone's minimum width and a step: as like to be dropped by KiCad's
    /// refill as kept, so the piece may be two there.
    pub at_limit: bool,
}

/// The pins of a pour's net within it, and which the copper inside it joins: its fill and its
/// net's own pads, tracks and vias there, not a way round outside it or through another layer.
#[derive(Clone, Debug, PartialEq)]
pub struct PinLinks {
    pub zone: usize,
    pub net: u32,
    pub layer: usize,
    /// The pads of its net on this layer with copper inside the zone's outline.
    pub pins: Vec<ItemId>,
    /// The pins the copper inside joins, a group each, largest first, with each group's
    /// weakest link: where the copper on the widest way between its pins is narrowest (none
    /// for a pin alone).
    pub groups: Vec<(Vec<ItemId>, Option<Neck>)>,
}

/// A section of a pour: the pins of its net that its copper inside would join if only the nets
/// let cut it crossed it, and whether the copper on the board still joins them, by copper at
/// least so wide.
#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    pub layer: usize,
    pub pins: Vec<ItemId>,
    /// The pins as the copper inside joins them, a group each; one when the section is whole.
    pub groups: Vec<Vec<ItemId>>,
    /// The weakest link on the widest way between its pins, when whole and of more than one pin.
    pub weakest: Option<Neck>,
    /// Whole, and no link narrower than the width asked for (to a raster step).
    pub ok: bool,
}

/// Zone `zone`'s sections ([`Section`]): the pour with only the copper of its own net and of
/// `cut_by` crossing it (no other net's tracks or vias) is poured again, and the pins its
/// copper joins then are a section; each is whole if the copper on the board joins its pins
/// too, each link at least `min_width` wide.
pub fn sections(board: &Board, zone: usize, cut_by: &[u32], min_width: Nm) -> Vec<Section> {
    let Some(area) = board.zone_areas.iter().find(|z| z.zone == zone) else { return Vec::new() };
    let mut ideal = board.clone();
    let outline = area.outline.bbox();
    let loose: Vec<ItemId> = board
        .items()
        .filter(|(id, it)| {
            matches!(it.kind, Kind::Track | Kind::Via) && it.net != area.net && !cut_by.contains(&it.net) && board.bbox(*id).is_some_and(|b| b.overlaps(outline))
        })
        .map(|(id, _)| id)
        .collect();
    for id in loose {
        ideal.remove(id);
    }
    let want = Pours::likely(&ideal).links_of(&ideal, zone);
    let have = Pours::likely(board).links_of(board, zone);
    let mut out = Vec::new();
    for w in want {
        let actual = have.iter().find(|h| h.layer == w.layer);
        for (pins, _) in w.groups {
            let mut groups: Vec<Vec<ItemId>> = Vec::new();
            let mut weakest = None;
            for (g, neck) in actual.map(|a| a.groups.as_slice()).unwrap_or_default() {
                let mine: Vec<ItemId> = g.iter().copied().filter(|p| pins.contains(p)).collect();
                if mine.is_empty() {
                    continue;
                }
                if mine.len() == pins.len() {
                    weakest = neck.clone();
                }
                groups.push(mine);
            }
            // A pin the board's copper inside reaches not at all is a group of its own.
            for p in &pins {
                if !groups.iter().any(|g| g.contains(p)) {
                    groups.push(vec![*p]);
                }
            }
            let ok = groups.len() == 1 && weakest.as_ref().is_none_or(|n: &Neck| n.width + STEP / 2 >= min_width);
            out.push(Section { layer: w.layer, pins, groups, weakest, ok });
        }
    }
    out
}

/// Two broad parts are told from a spur by how much broader than the neck between them each
/// is: at least this much on each side (so 2 steps of width).
const BROADER: f64 = 1.0;

impl Pours {
    /// The middles of the raster cells of zone `zone`'s fill on `layer`, each with its piece.
    pub fn cells_of(&self, zone: usize, layer: usize) -> Vec<(Point, usize)> {
        let mut out = Vec::new();
        for part in self.parts.iter().filter(|p| p.zone == zone && p.layer == layer) {
            let r = &part.raster;
            for (c, &k) in part.piece.iter().enumerate() {
                if k != 0 {
                    out.push((r.center(c % r.w, c / r.w), k as usize - 1));
                }
            }
        }
        out
    }

    /// Which pins of zone `zone`'s net within it its copper joins, on each of its layers, and
    /// how narrow the widest way between them is.
    pub fn links_of(&self, board: &Board, zone: usize) -> Vec<PinLinks> {
        let Some(area) = board.zone_areas.iter().find(|z| z.zone == zone) else { return Vec::new() };
        let limit = area.min_thickness + STEP;
        let mut out = Vec::new();
        for part in self.parts.iter().filter(|p| p.zone == zone) {
            let r = &part.raster;
            let mut copper = r.cells.clone();
            let mut pin_at: Vec<u32> = vec![u32::MAX; copper.len()];
            let mut pins = Vec::new();
            let b = BBox::of([r.center(0, 0), r.center(r.w - 1, r.h - 1)]).grow(STEP);
            for id in board.near(part.layer, b) {
                let Some(item) = board.item(id) else { continue };
                if item.net != part.net || !item.on(part.layer) {
                    continue;
                }
                let cells: Vec<usize> = r.cells_in(&item.shape).into_iter().filter(|&c| area.covers(r.center(c % r.w, c / r.w))).collect();
                match item.kind {
                    Kind::Pad { .. } if !cells.is_empty() => {
                        for &c in &cells {
                            copper[c] = true;
                            pin_at[c] = pins.len() as u32;
                        }
                        pins.push(id);
                    }
                    Kind::Track | Kind::Via => cells.iter().for_each(|&c| copper[c] = true),
                    _ => {}
                }
            }
            if pins.is_empty() {
                continue;
            }
            let cells: Vec<usize> = (0..copper.len()).filter(|&c| copper[c]).collect();
            let groups = links(r.w, r.h, &cells, &pin_at, pins.len());
            let point = |c: usize| r.center(c % r.w, c / r.w);
            let mut groups: Vec<(Vec<ItemId>, Option<Neck>)> = groups
                .into_iter()
                .map(|(members, weakest)| {
                    let neck = weakest.map(|(c, half)| {
                        let at = point(c);
                        let width = ((2.0 * half - 1.0) * STEP as f64).max(STEP as f64) as Nm;
                        Neck { width, at, by: pinching(board, part.net, part.layer, at, width), at_limit: width < limit }
                    });
                    (members.into_iter().map(|k| pins[k]).collect(), neck)
                })
                .collect();
            groups.sort_by_key(|(m, _)| std::cmp::Reverse(m.len()));
            out.push(PinLinks { zone, net: part.net, layer: part.layer, pins, groups });
        }
        out
    }

    /// The pieces of zone `zone`'s fill, on each of its layers, with what each joins and where
    /// it is narrow.
    pub fn pieces_of(&self, board: &Board, zone: usize) -> Vec<PourPiece> {
        let mut out = Vec::new();
        let limit = board.zone_areas.iter().find(|z| z.zone == zone).map_or(0, |z| z.min_thickness) + STEP;
        for part in self.parts.iter().filter(|p| p.zone == zone) {
            let r = &part.raster;
            let mut ids: Vec<u32> = part.piece.iter().copied().filter(|&k| k != 0).collect();
            ids.sort_unstable();
            ids.dedup();
            for k in ids {
                let piece = k as usize - 1;
                let cells: Vec<usize> = (0..part.piece.len()).filter(|&c| part.piece[c] == k).collect();
                let (necks, at) = necks(r.w, r.h, &cells);
                let point = |c: usize| r.center(c % r.w, c / r.w);
                let necks = necks
                    .into_iter()
                    .map(|(c, half)| {
                        let at = point(c);
                        let width = ((2.0 * half - 1.0) * STEP as f64).max(STEP as f64) as Nm;
                        Neck { width, at, by: pinching(board, part.net, part.layer, at, width), at_limit: width < limit }
                    })
                    .collect();
                let mut p = PourPiece {
                    zone,
                    net: part.net,
                    layer: part.layer,
                    piece,
                    area: cells.len() as f64 * (STEP as f64).powi(2),
                    at: point(at),
                    pads: Vec::new(),
                    vias: Vec::new(),
                    tracks: Vec::new(),
                    necks,
                };
                let b = BBox::of(cells.iter().map(|&c| point(c))).grow(STEP);
                for id in board.near(part.layer, b) {
                    let Some(item) = board.item(id) else { continue };
                    if item.net != part.net || !self.joined(board, id, part.layer).contains(&piece) {
                        continue;
                    }
                    match item.kind {
                        Kind::Pad { .. } => p.pads.push(id),
                        Kind::Via => p.vias.push(id),
                        Kind::Track => p.tracks.push(id),
                        _ => {}
                    }
                }
                out.push(p);
            }
        }
        out
    }
}

/// The copper not of `net` on `layer` nearest a neck at `at`, within a little more than the
/// nearest's gap: what makes it narrow.
fn pinching(board: &Board, net: u32, layer: usize, at: Point, width: Nm) -> Vec<(ItemId, Nm)> {
    let reach = width + 600_000;
    let dot = Shape::Capsule { a: at, b: at, r: 0 };
    let mut near: Vec<(ItemId, Nm)> = board
        .near(layer, BBox::of([at]).grow(reach))
        .into_iter()
        .filter_map(|id| {
            let item = board.item(id)?;
            let copper = matches!(item.kind, Kind::Pad { .. } | Kind::Via | Kind::Track | Kind::Pour { .. });
            (copper && item.net != net && item.on(layer)).then(|| (id, item.shape.gap(&dot).max(0.0) as Nm))
        })
        .filter(|&(_, gap)| gap <= reach)
        .collect();
    near.sort_by_key(|&(id, gap)| (gap, id));
    let Some(&(_, first)) = near.first() else { return near };
    near.retain(|&(_, gap)| gap <= first + 3 * STEP);
    near.truncate(4);
    near
}

/// The necks of one piece of a `w`×`h` raster, given as its cells: each the cell where two
/// broad parts of it meet and that cell's distance from the nearest cell outside the piece in
/// steps, narrowest first; and the piece's deepest cell.
///
/// Cells are taken deepest first and joined to their taken neighbours; where two groups meet,
/// each deeper than where they meet by [`BROADER`], that is a neck. A spur meets the body about
/// as deep as it is, so it is not one.
fn necks(w: usize, h: usize, cells: &[usize]) -> (Vec<(usize, f64)>, usize) {
    let depth = distances(w, h, cells);
    let mut order: Vec<usize> = cells.to_vec();
    order.sort_by(|a, b| depth[*b].total_cmp(&depth[*a]).then(a.cmp(b)));
    let mut parent: std::collections::HashMap<usize, usize> = std::collections::HashMap::with_capacity(cells.len());
    let mut peak: std::collections::HashMap<usize, f64> = std::collections::HashMap::new();
    fn root(parent: &mut std::collections::HashMap<usize, usize>, mut c: usize) -> usize {
        while parent[&c] != c {
            let up = parent[&parent[&c]];
            parent.insert(c, up);
            c = up;
        }
        c
    }
    let mut out = Vec::new();
    for &c in &order {
        let d = depth[c];
        parent.insert(c, c);
        peak.insert(c, d);
        let (i, j) = (c % w, c / w);
        let mut around = Vec::with_capacity(4);
        if i > 0 {
            around.push(c - 1);
        }
        if i + 1 < w {
            around.push(c + 1);
        }
        if j > 0 {
            around.push(c - w);
        }
        if j + 1 < h {
            around.push(c + w);
        }
        for n in around {
            if !parent.contains_key(&n) {
                continue;
            }
            let (a, b) = (root(&mut parent, c), root(&mut parent, n));
            if a == b {
                continue;
            }
            let (pa, pb) = (peak[&a], peak[&b]);
            if pa.min(pb) >= d + BROADER {
                out.push((c, d));
            }
            parent.insert(b, a);
            peak.insert(a, pa.max(pb));
        }
    }
    out.sort_by(|x, y| x.1.total_cmp(&y.1).then(x.0.cmp(&y.0)));
    (out, order[0])
}

/// The pins the copper `cells` of a `w`×`h` raster joins, a group each, with the group's weakest
/// link: the cell and depth at which its last two parts met, taking cells deepest first. That
/// is the narrowest place on the widest way between its pins.
#[allow(clippy::type_complexity)]
fn links(w: usize, h: usize, cells: &[usize], pin_at: &[u32], pins: usize) -> Vec<(Vec<usize>, Option<(usize, f64)>)> {
    let depth = distances(w, h, cells);
    let mut order: Vec<usize> = cells.to_vec();
    order.sort_by(|a, b| depth[*b].total_cmp(&depth[*a]).then(a.cmp(b)));
    let mut parent: Vec<usize> = (0..w * h).collect();
    let mut taken = vec![false; w * h];
    let mut members: Vec<Vec<usize>> = vec![Vec::new(); w * h];
    let mut weakest: Vec<Option<(usize, f64)>> = vec![None; w * h];
    fn root(parent: &mut [usize], mut c: usize) -> usize {
        while parent[c] != c {
            parent[c] = parent[parent[c]];
            c = parent[c];
        }
        c
    }
    let mut seen = vec![false; pins];
    for &c in &order {
        taken[c] = true;
        let k = pin_at[c];
        if k != u32::MAX && !seen[k as usize] {
            seen[k as usize] = true;
            members[c].push(k as usize);
        }
        let (i, j) = (c % w, c / w);
        let around = [(i > 0).then(|| c - 1), (i + 1 < w).then(|| c + 1), (j > 0).then(|| c - w), (j + 1 < h).then(|| c + w)];
        for n in around.into_iter().flatten() {
            if !taken[n] {
                continue;
            }
            let (a, b) = (root(&mut parent, c), root(&mut parent, n));
            if a == b {
                continue;
            }
            let both = !members[a].is_empty() && !members[b].is_empty();
            parent[b] = a;
            let moved = std::mem::take(&mut members[b]);
            members[a].extend(moved);
            let mut wk = [weakest[a], weakest[b].take()].into_iter().flatten().min_by(|x, y| x.1.total_cmp(&y.1));
            if both {
                wk = Some((c, depth[c]));
            }
            weakest[a] = wk;
        }
    }
    let mut out = Vec::new();
    for c in 0..w * h {
        if taken[c] && parent[c] == c && !members[c].is_empty() {
            let mut m = std::mem::take(&mut members[c]);
            m.sort_unstable();
            out.push((m, weakest[c]));
        }
    }
    out
}

/// Each cell's distance in steps from the nearest cell centre outside the piece (outside the
/// raster counts as outside); exact, by Felzenszwalb and Huttenlocher's two passes.
fn distances(w: usize, h: usize, cells: &[usize]) -> Vec<f64> {
    const FAR: f64 = 1e12;
    let mut f = vec![0.0f64; (w + 2) * (h + 2)];
    let (pw, ph) = (w + 2, h + 2);
    for &c in cells {
        f[(c / w + 1) * pw + c % w + 1] = FAR;
    }
    let mut line = Vec::new();
    for j in 0..ph {
        line.clear();
        line.extend((0..pw).map(|i| f[j * pw + i]));
        let d = pass(&line);
        for i in 0..pw {
            f[j * pw + i] = d[i];
        }
    }
    for i in 0..pw {
        line.clear();
        line.extend((0..ph).map(|j| f[j * pw + i]));
        let d = pass(&line);
        for j in 0..ph {
            f[j * pw + i] = d[j];
        }
    }
    let mut out = vec![0.0; w * h];
    for &c in cells {
        out[c] = f[(c / w + 1) * pw + c % w + 1].sqrt();
    }
    out
}

/// One pass of the squared distance transform along a line.
fn pass(f: &[f64]) -> Vec<f64> {
    let n = f.len();
    let mut d = vec![0.0; n];
    let mut v = vec![0usize; n];
    let mut z = vec![0.0f64; n + 1];
    let mut k = 0;
    z[0] = f64::NEG_INFINITY;
    z[1] = f64::INFINITY;
    let sq = |q: usize| (q * q) as f64;
    let meet = |q: usize, p: usize| ((f[q] + sq(q)) - (f[p] + sq(p))) / (2.0 * q as f64 - 2.0 * p as f64);
    for q in 1..n {
        let mut s = meet(q, v[k]);
        while s <= z[k] {
            k -= 1;
            s = meet(q, v[k]);
        }
        k += 1;
        v[k] = q;
        z[k] = s;
        z[k + 1] = f64::INFINITY;
    }
    k = 0;
    for (q, out) in d.iter_mut().enumerate() {
        while z[k + 1] < q as f64 {
            k += 1;
        }
        let p = v[k];
        *out = (q as f64 - p as f64).powi(2) + f[p];
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(rows: &[&str]) -> (usize, usize, Vec<usize>) {
        let w = rows[0].len();
        let cells = rows.iter().enumerate().flat_map(|(j, r)| r.bytes().enumerate().filter(|(_, b)| *b == b'#').map(move |(i, _)| j * w + i)).collect();
        (w, rows.len(), cells)
    }

    #[test]
    fn distances_are_to_the_nearest_cell_outside() {
        let (w, h, cells) = grid(&["#####", "#####", "#####"]);
        let d = distances(w, h, &cells);
        assert_eq!(d[0], 1.0);
        assert_eq!(d[w + 2], 2.0);
    }

    #[test]
    fn two_blocks_joined_by_a_strip_have_one_neck_and_a_spur_has_none() {
        let rows = [
            "...........................",
            ".#######...........#######.",
            ".#######...........#######.",
            ".#######...........#######.",
            ".###################################",
            ".#######...........#######.",
            ".#######...........#######.",
            ".#######...........#######.",
            "...........................",
        ];
        let rows: Vec<String> = rows.iter().map(|r| format!("{r:.<36}")).collect();
        let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
        let (w, h, cells) = grid(&rows);
        let (necks, _) = necks(w, h, &cells);
        assert_eq!(necks.len(), 1, "{necks:?}");
        let (c, d) = necks[0];
        assert_eq!(c / w, 4);
        assert!((7..=20).contains(&(c % w)), "{c}");
        assert_eq!(d, 1.0);
    }
}
