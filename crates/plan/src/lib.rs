//! The whole-board plan: before any copper, which gaps between obstacles each connection passes,
//! in what order, on which layer (docs/one-system-spec.md; the reasons are in UltraRoute's
//! `from-zero.md`). A prototype: strands are laid one at a time, shortest connection first, each
//! on one layer; there are no vias and nothing is taken up again yet.

pub mod draw;
pub mod space;
pub mod strands;

use board::{Board, ItemId, Kind};
use serde::Serialize;

pub use space::Space;
pub use strands::{Sheet, Strand};

/// A connection the plan has no strand for, and why.
#[derive(Debug, Clone, Serialize)]
pub struct Left {
    pub net: u32,
    pub from: ItemId,
    pub to: ItemId,
    pub why: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Plan {
    /// One sheet per copper layer, in the board's order.
    pub sheets: Vec<Sheet>,
    /// Layers a pour covers at least half of: strands go there only when they fit nowhere else.
    pub planes: Vec<usize>,
    pub left: Vec<Left>,
}

/// Takes every track and via that is not locked off the board: what is left is what a router
/// is given.
pub fn strip(board: &mut Board) {
    let gone: Vec<ItemId> = board.items().filter(|(_, i)| matches!(i.kind, Kind::Track | Kind::Via) && !i.locked).map(|(id, _)| id).collect();
    for id in gone {
        board.remove(id);
    }
}

/// The share of the board's area each layer's pours cover.
pub fn poured(board: &Board) -> Vec<f64> {
    let area = |p: &[board::Point]| (0..p.len()).map(|i| p[i].x as f64 * p[(i + 1) % p.len()].y as f64 - p[(i + 1) % p.len()].x as f64 * p[i].y as f64).sum::<f64>().abs() / 2.0;
    let of = board.outline.first().map_or(0.0, |o| area(o)) - board.outline.iter().skip(1).map(|h| area(h)).sum::<f64>();
    (0..board.layers.len())
        .map(|l| {
            let a: f64 = board.items().filter(|(_, i)| matches!(i.kind, Kind::Pour { .. }) && i.on(l) && !i.drawn()).map(|(_, i)| match &i.shape {
                board::Shape::Polygon(p) => area(p),
                _ => 0.0,
            }).sum();
            if of > 0.0 { a / of } else { 0.0 }
        })
        .collect()
}

/// A net's track width, mm: its class's.
pub fn width(board: &Board, net: u32) -> f64 {
    let class = board.rules.class_of(board.net_name(net));
    class.map_or(board.rules.min_track_width, |c| c.track_width.max(board.rules.min_track_width)) * 1e3
}

/// What a strand on a plane layer costs, as a multiple of its length.
const ON_A_PLANE: f64 = 4.0;

/// A connection to plan: a net between two pads.
#[derive(Debug, Clone, Copy)]
struct Want {
    net: u32,
    from: ItemId,
    to: ItemId,
}

/// Lays a strand for `want` on the layer where it is cheapest, if it fits on any.
fn lay(board: &Board, sheets: &mut [Sheet], planes: &[usize], want: Want) -> bool {
    let w = width(board, want.net);
    let ways = sheets.iter().enumerate().filter_map(|(l, s)| {
        let way = s.find(board, want.net, w, want.from, want.to)?;
        Some((way.cost * if planes.contains(&l) { ON_A_PLANE } else { 1.0 }, l, way))
    });
    match ways.min_by(|x, y| x.0.total_cmp(&y.0)) {
        Some((_, l, way)) => {
            sheets[l].lay(want.net, w, want.from, want.to, &way);
            true
        }
        None => false,
    }
}

/// How many laid strands are lifted, one at a time, to let a connection through that found no
/// way: a count, so the work is bounded whatever the board.
const LIFTS: usize = 40;

/// Lays `want` by lifting one strand that is in its way and laying that one again after it.
/// The sheets are changed only if both then fit.
fn lay_by_lifting(board: &Board, sheets: &mut Vec<Sheet>, planes: &[usize], want: Want) -> bool {
    let middle = |sheets: &[Sheet], item: ItemId| sheets.iter().find_map(|s| s.space.of_item.get(&item).map(|&o| s.space.obstacles[o].centre));
    let (Some(a), Some(b)) = (middle(sheets, want.from), middle(sheets, want.to)) else { return false };
    // Nearest first: by how far the strand's ends are from this connection's.
    let mut near: Vec<(f64, usize, usize)> = Vec::new();
    for (l, sheet) in sheets.iter().enumerate() {
        for k in sheet.laid() {
            let s = &sheet.strands[k];
            let (Some(p), Some(q)) = (middle(sheets, s.from), middle(sheets, s.to)) else { continue };
            let d = (space::dist(a, p) + space::dist(b, q)).min(space::dist(a, q) + space::dist(b, p));
            near.push((d, l, k));
        }
    }
    near.sort_by(|x, y| x.0.total_cmp(&y.0));
    for &(_, l, k) in near.iter().take(LIFTS) {
        let mut tried = sheets.clone();
        let lifted = Want { net: tried[l].strands[k].net, from: tried[l].strands[k].from, to: tried[l].strands[k].to };
        tried[l].lift(k);
        if lay(board, &mut tried, planes, want) && lay(board, &mut tried, planes, lifted) {
            *sheets = tried;
            return true;
        }
    }
    false
}

/// Plans every open connection of a stripped board.
pub fn plan(board: &Board) -> Plan {
    let mut sheets: Vec<Sheet> = (0..board.layers.len()).map(|l| Sheet::new(Space::new(board, l))).collect();
    let planes: Vec<usize> = poured(board).iter().enumerate().filter(|(_, s)| **s >= 0.5).map(|(l, _)| l).collect();
    let mut airlines = board::connectivity(board).airlines;
    airlines.sort_by(|a, b| a.from.distance(a.to).total_cmp(&b.from.distance(b.to)));
    let mut left = Vec::new();
    for a in &airlines {
        let want = Want { net: a.net, from: a.from_item, to: a.to_item };
        if lay(board, &mut sheets, &planes, want) || lay_by_lifting(board, &mut sheets, &planes, want) {
            continue;
        }
        let (f, t) = (board.item(a.from_item), board.item(a.to_item));
        let shared = f.zip(t).is_some_and(|(f, t)| f.layers & t.layers != 0);
        let pads = f.zip(t).is_some_and(|(f, t)| matches!(f.kind, Kind::Pad { .. }) && matches!(t.kind, Kind::Pad { .. }));
        let why = if !pads { "an end is not a pad" } else if !shared { "its pads share no layer: needs a via" } else { "no way through the room left on a layer both pads are on" };
        left.push(Left { net: a.net, from: a.from_item, to: a.to_item, why: why.to_string() });
    }
    Plan { sheets, planes, left }
}
