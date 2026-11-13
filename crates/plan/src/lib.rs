//! The whole-board plan: before any copper, which gaps between obstacles each connection passes,
//! in what order, on which layer (docs/one-system-spec.md; the reasons are in UltraRoute's
//! `from-zero.md`). A prototype: connections are laid one at a time, shortest first. One with
//! no way has one other lifted out of its way and laid again after it; failing that it takes
//! vias, at sites on a grid that is part of every layer's triangulation.

pub mod draw;
pub mod settle;
pub mod space;
pub mod strands;
pub mod ways;

use board::{Board, ItemId, Kind};
use serde::Serialize;

pub use settle::{settle, Settled};
pub use space::{End, Space};
pub use strands::{Sheet, Strand};

/// A connection the plan has no strand for, and why.
#[derive(Debug, Clone, Serialize)]
pub struct Left {
    pub net: u32,
    pub from: ItemId,
    pub to: ItemId,
    pub why: String,
}

/// A via the plan put in: the site it took.
#[derive(Debug, Clone, Serialize)]
pub struct Via {
    pub net: u32,
    pub site: usize,
    pub at: space::P,
}

#[derive(Debug, Clone, Serialize)]
pub struct Plan {
    /// One sheet per copper layer, in the board's order.
    pub sheets: Vec<Sheet>,
    /// Layers a pour covers at least half of, unless every layer is: strands there cost more.
    pub planes: Vec<usize>,
    pub vias: Vec<Via>,
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
/// What a via costs, as length, mm: one is worth a detour this long.
const VIA: f64 = 8.0;

/// A connection to plan: a net between two pads.
#[derive(Debug, Clone, Copy)]
struct Want {
    net: u32,
    from: ItemId,
    to: ItemId,
}

/// The default via's radius and drill radius, nm.
pub fn via_size(board: &Board) -> (i64, i64) {
    let class = board.rules.class_of("");
    let d = class.as_ref().map_or(0.0, |c| c.via_diameter).max(board.rules.min_via_diameter);
    let drill = class.as_ref().map_or(0.0, |c| c.via_drill).max(board.rules.min_via_drill);
    (board::nm(d) / 2, board::nm(drill) / 2)
}

/// Places a via could stand, as small polygons that hold one: a grid over the board, at a
/// pitch that lets a track pass between two neighbours, each kept only where the exact checker
/// lets a via of no net stand on the bare board.
pub fn via_sites(board: &Board) -> Vec<Vec<space::P>> {
    let (r, drill) = via_size(board);
    let all = (1u64 << board.layers.len()) - 1;
    // A board whose outline does not close: the box round everything on it.
    let outer = board.outline.first();
    let box_ = match outer {
        Some(o) => board::BBox::of(o.iter().copied()),
        None => board.items().filter_map(|(id, _)| board.bbox(id)).reduce(|a, b| a.union(b)).unwrap_or(board::BBox::of([board::Point::new(0, 0)])),
    };
    let pitch = 2 * r + 2 * board.clearance(0, 0) + board::nm(board.rules.min_track_width.max(width(board, 0) / 1e3));
    let mut scratch = board.clone();
    let mut sites = Vec::new();
    let mut y = box_.min.y + pitch / 2;
    while y < box_.max.y {
        let mut x = box_.min.x + pitch / 2;
        while x < box_.max.x {
            let at = board::Point::new(x, y);
            let shape = board::Shape::Capsule { a: at, b: at, r };
            let id = scratch.insert(board::Item { kind: Kind::Via, net: 0, layers: all, shape: shape.clone(), hole: Some((at, drill)), locked: false, span: None });
            if outer.is_none_or(|o| board::inside(at, o)) && board::check_item(&scratch, id).is_empty() {
                sites.push(space::outline(&shape));
            }
            scratch.remove(id);
            x += pitch;
        }
        y += pitch;
    }
    sites
}

/// Lays a connection along the cheapest way there is for it, on sheets the caller throws away
/// when this fails: false with no way, or with one whose vias do not fit together.
fn lay(board: &Board, sheets: &mut [Sheet], terms: &ways::Terms, connection: usize, want: Want) -> bool {
    let w = width(board, want.net);
    let Some(way) = ways::find(board, sheets, terms, want.net, w, End::Item(want.from), End::Item(want.to)) else { return false };
    place(board, sheets, connection, want, &way);
    // The caller lays on sheets it will throw away if this fails.
    holds(board, sheets, &way)
}

/// Puts a way that was found on the sheets as they are into them.
fn place(board: &Board, sheets: &mut [Sheet], connection: usize, want: Want, way: &ways::Way) {
    let w = width(board, want.net);
    for leg in &way.legs {
        for end in [leg.from, leg.to] {
            if let End::Site(k) = end {
                for sheet in sheets.iter_mut() {
                    sheet.space.take_site(k, Some(want.net));
                }
            }
        }
    }
    for leg in &way.legs {
        sheets[leg.layer].lay(want.net, connection, w, leg.from, leg.to, &leg.steps);
    }
}

/// Whether the sheets, with a way placed in them, still hold their strands: every gap the way
/// crosses (the search tries one crossing at a time, and a way that crosses a gap twice can
/// fill it past its room) and every gap round a via site it takes (each site is tried alone,
/// and two taken side by side narrow the gap between them from both ends).
fn holds(board: &Board, sheets: &[Sheet], way: &ways::Way) -> bool {
    let fits = |s: &Sheet, e: usize| {
        let [a, b] = s.space.edges[e].v.map(|v| s.space.vert_of[v]);
        // A side of an obstacle, or of a via site, is no gap.
        (a.is_some() && a == b) || s.need(board, e, None, None) <= s.space.edges[e].len + strands::EPS
    };
    let sites = way.legs.iter().flat_map(|l| [l.from, l.to]).filter_map(|e| if let End::Site(k) = e { Some(k) } else { None });
    way.legs.iter().all(|l| l.steps.iter().all(|&(e, _)| fits(&sheets[l.layer], e))) && sites.into_iter().all(|k| sheets.iter().all(|s| s.space.sites[k].around.iter().all(|&e| fits(s, e))))
}

/// Puts the vias of a way on `with`, a board that holds the vias placed before it, and says
/// whether the exact checker passes each: a site was only ever tried alone, on the bare board,
/// and two vias can together cut more of a pad's thermal spokes than one may.
fn stand(with: &mut Board, sheets: &[Sheet], way: &ways::Way, net: u32) -> bool {
    let (r, drill) = via_size(with);
    let all = (1u64 << with.layers.len()) - 1;
    let mut sites: Vec<usize> = way.legs.iter().flat_map(|l| [l.from, l.to]).filter_map(|e| if let End::Site(k) = e { Some(k) } else { None }).collect();
    sites.sort();
    sites.dedup();
    let Some(sheet) = sheets.first() else { return true };
    let mut fine = true;
    for k in sites {
        let c = sheet.space.obstacles[sheet.space.sites[k].obstacle].centre;
        let at = board::Point::new((c[0] * 1e6).round() as i64, (c[1] * 1e6).round() as i64);
        let id = with.insert(board::Item { kind: Kind::Via, net, layers: all, shape: board::Shape::Capsule { a: at, b: at, r }, hole: Some((at, drill)), locked: false, span: None });
        fine &= board::check_item(with, id).is_empty();
    }
    fine
}

/// A way on one layer is taken without a thought for vias when it is no longer than this many
/// times the straight line between its ends, plus [`SLACK`] mm.
const DIRECT: f64 = 2.0;
const SLACK: f64 = 4.0;

/// Lifts every strand of a connection, and gives back the via sites it alone used.
fn lift(sheets: &mut [Sheet], connection: usize) {
    let mut sites = Vec::new();
    for sheet in sheets.iter_mut() {
        for k in sheet.laid().collect::<Vec<_>>() {
            if sheet.strands[k].connection == connection {
                sites.extend([sheet.strands[k].from, sheet.strands[k].to]);
                sheet.lift(k);
            }
        }
    }
    for end in sites {
        let End::Site(k) = end else { continue };
        let used = sheets.iter().any(|s| s.laid().any(|x| s.strands[x].from == end || s.strands[x].to == end));
        if !used {
            for sheet in sheets.iter_mut() {
                sheet.space.take_site(k, None);
            }
        }
    }
}

/// How many laid connections are lifted, one at a time, to let through a connection that found
/// no way: a count, so the work is bounded whatever the board.
const LIFTS: usize = 30;

/// Lays `want` by lifting one connection that is in its way and laying that one again after
/// it. The sheets are changed only if both then fit.
fn lay_by_lifting(board: &Board, sheets: &mut Vec<Sheet>, terms: &ways::Terms, wants: &[Want], connection: usize) -> bool {
    let want = wants[connection];
    let middle = |sheets: &[Sheet], item: ItemId| sheets.iter().find_map(|s| s.space.ends.get(&End::Item(item)).map(|&o| s.space.obstacles[o].centre));
    let (Some(a), Some(b)) = (middle(sheets, want.from), middle(sheets, want.to)) else { return false };
    // Nearest first: by how far the connection's ends are from this one's.
    let mut laid: Vec<usize> = sheets.iter().flat_map(|s| s.laid().map(|k| s.strands[k].connection)).collect();
    laid.sort();
    laid.dedup();
    let mut near: Vec<(f64, usize)> = laid
        .iter()
        .filter_map(|&c| {
            let (p, q) = (middle(sheets, wants[c].from)?, middle(sheets, wants[c].to)?);
            Some(((space::dist(a, p) + space::dist(b, q)).min(space::dist(a, q) + space::dist(b, p)), c))
        })
        .collect();
    near.sort_by(|x, y| x.0.total_cmp(&y.0));
    for &(_, c) in near.iter().take(LIFTS) {
        let mut tried = sheets.clone();
        lift(&mut tried, c);
        if lay(board, &mut tried, terms, connection, want) && lay(board, &mut tried, terms, c, wants[c]) {
            *sheets = tried;
            return true;
        }
    }
    false
}

/// Plans every open connection of a stripped board.
pub fn plan(board: &Board) -> Plan {
    let sites = via_sites(board);
    let mut sheets: Vec<Sheet> = (0..board.layers.len()).map(|l| Sheet::new(Space::new(board, l, &sites))).collect();
    let poured = poured(board);
    // On two layers a poured side is still routed on: the designers do (159 of the gate
    // tier's 253 two-layer boards have a side at least half poured).
    let planes: Vec<usize> = if poured.len() <= 2 || poured.iter().all(|s| *s >= 0.5) { Vec::new() } else { (0..poured.len()).filter(|&l| poured[l] >= 0.5).collect() };
    let dear: Vec<f64> = (0..sheets.len()).map(|l| if planes.contains(&l) { ON_A_PLANE } else { 1.0 }).collect();
    // (Tried 2026-11-12: one search for every connection that weighs a via against the detour
    // it saves put 19 vias on Shift-in 32, where the designer has none. And vias only as the
    // last thing tried made a connection take any way round, however long.)
    let (flat, with_vias) = (ways::Terms { dear: &dear, via: None }, ways::Terms { dear: &dear, via: Some(VIA) });
    // A pour joins nothing here: tracks laid across it can cut a pad off from the rest of it,
    // so a pour's net is planned like any other and the pour fills in round what is drawn.
    let mut airlines = board::connectivity_without_pours(board).airlines;
    airlines.sort_by(|a, b| a.from.distance(a.to).total_cmp(&b.from.distance(b.to)));
    let wants: Vec<Want> = airlines.iter().map(|a| Want { net: a.net, from: a.from_item, to: a.to_item }).collect();
    let mut left = Vec::new();
    // The board with the vias placed so far on it.
    let mut with = board.clone();
    for (c, (&want, line)) in wants.iter().zip(&airlines).enumerate() {
        // A way on one layer that is nearly direct is taken. A long way round is weighed
        // against the way with vias, each via counted as [`VIA`] of length.
        let w = width(board, want.net);
        let (from, to) = (End::Item(want.from), End::Item(want.to));
        let direct = DIRECT * line.from.distance(line.to) / 1e6 + SLACK;
        // A way is one that still holds once it is in the sheets.
        let sound = |way: &ways::Way, with: &Board| {
            let mut tried = sheets.clone();
            place(board, &mut tried, c, want, way);
            holds(board, &tried, way) && stand(&mut with.clone(), &tried, way, want.net)
        };
        let on_one = ways::find(board, &sheets, &flat, want.net, w, from, to).filter(|way| sound(way, &with));
        let best = match on_one {
            Some(way) if way.cost <= direct => Some(way),
            on_one => {
                let through = ways::find(board, &sheets, &with_vias, want.net, w, from, to).filter(|way| sound(way, &with));
                [on_one, through].into_iter().flatten().min_by(|a, b| a.cost.total_cmp(&b.cost))
            }
        };
        if let Some(way) = best {
            place(board, &mut sheets, c, want, &way);
            stand(&mut with, &sheets, &way, want.net);
            continue;
        }
        if lay_by_lifting(board, &mut sheets, &flat, &wants, c) || lay_by_lifting(board, &mut sheets, &with_vias, &wants, c) {
            continue;
        }
        let pads = board.item(want.from).zip(board.item(want.to)).is_some_and(|(f, t)| matches!(f.kind, Kind::Pad { .. }) && matches!(t.kind, Kind::Pad { .. }));
        let why = if pads { "no way through the room left, with vias or without" } else { "an end is not a pad" };
        left.push(Left { net: want.net, from: want.from, to: want.to, why: why.to_string() });
    }
    let mut vias = Vec::new();
    if let Some(first) = sheets.first() {
        for (k, site) in first.space.sites.iter().enumerate() {
            let o = &first.space.obstacles[site.obstacle];
            if o.what == space::What::Pad {
                vias.push(Via { net: o.net, site: k, at: o.centre });
            }
        }
    }
    Plan { sheets, planes, vias, left }
}
