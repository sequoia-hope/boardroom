//! The whole-board plan: before any copper, which gaps between obstacles each connection passes,
//! in what order, on which layer (docs/one-system-spec.md; the reasons are in UltraRoute's
//! `from-zero.md`). A prototype: connections are laid one at a time, shortest first. One with
//! no way has the others in its way lifted and laid again after it; failing that it takes
//! vias. Each layer's triangulation holds only what is there: a via is an obstacle once a
//! connection has put it in, and the layers are then triangulated again round it.
//!
//! A plan is made empty ([`start`]) and added to ([`add`]), so that what a net's pour leaves
//! apart can be planned once the other nets are drawn: such a connection may end at a via
//! standing in the pour ([`Want::into`]). A connection whose wire could not be drawn is laid
//! again narrower, or at its narrowest barred from the gap it stuck in ([`again`]), the rest of
//! the plan left as it is. A connection ends on its pad or on any pad already joined to it,
//! on the bare board ([`Plan::joined`]) or by the strands laid, and is as wide as its net's class asks unless it has no way at that
//! width ([`wide`]). A part whose pads stand nearer each other than vias may can have its way
//! out settled for the whole part before anything else is laid ([`escape`]); without that, no
//! via of another net is put in the way straight out of such a pad ([`Spots::shuts`]). A
//! track keeps from a via what its hole asks as well as what its copper does
//! ([`space::Obstacle::hole`]).

pub mod carry;
pub mod draw;
pub mod space;
pub mod strands;
pub mod ways;

use std::sync::Arc;

use board::{Board, ItemId, Kind};
use serde::Serialize;

pub use space::{End, Space, Spots};
pub use strands::{Sheet, Strand};
use ways::Stop;

/// A connection the plan has no strand for, and why.
#[derive(Debug, Clone, Serialize)]
pub struct Left {
    /// Its number among the plan's [`Plan::wants`].
    pub connection: usize,
    pub net: u32,
    pub from: ItemId,
    pub to: ItemId,
    pub why: String,
}

/// A via the plan put in: its number among the plan's vias, and where it is.
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
    /// Layers a pour covers at least half of, unless every layer is: inner ones are closed to
    /// strands (vias pass), outer ones cost more.
    pub planes: Vec<usize>,
    /// The plane layers closed to strands because nothing on the board says what may cross
    /// them (owner, 2026-11-16: an inner layer one net mostly pours is solid unless stated
    /// otherwise). A plane layer with a rule area on it that bars some nets says it, and is
    /// open, at a plane's cost, to the nets its areas let through.
    pub closed: Vec<usize>,
    pub vias: Vec<Via>,
    pub left: Vec<Left>,
    /// Every connection asked for, in the order asked: a strand's `connection` is its number
    /// here.
    pub wants: Vec<Want>,
    /// For each pad that other pads are joined to on the bare board, by overlapping it or by
    /// copper that stays: all of them, itself among them. A connection to one may end on any.
    #[serde(skip)]
    pub joined: Arc<Joined>,
    /// The pads a hole was asked for that have none: no place in the pad for one, or the plan
    /// could not take it ([`hole`]). They are not asked about again.
    #[serde(skip)]
    pub refused: Vec<ItemId>,
}

/// The pads each pad is joined to before anything is laid.
pub type Joined = std::collections::HashMap<ItemId, Vec<ItemId>>;

/// [`Plan::joined`] for a board.
fn joined(board: &Board) -> Joined {
    let clusters = board::connectivity_without_pours(board).cluster;
    let mut by: std::collections::HashMap<usize, Vec<ItemId>> = std::collections::HashMap::new();
    for (id, item) in board.items() {
        if matches!(item.kind, Kind::Pad { .. })
            && let Some(&c) = clusters.get(&id)
        {
            by.entry(c).or_default().push(id);
        }
    }
    let mut out = Joined::new();
    for mut pads in by.into_values().filter(|pads| pads.len() > 1) {
        pads.sort();
        for &pad in &pads {
            out.insert(pad, pads.clone());
        }
    }
    out
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
    let bounds = space::bounds(board);
    let of = bounds.first().map_or(0.0, |o| area(o)) - bounds.iter().skip(1).map(|h| area(h)).sum::<f64>();
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

/// A net's track width, mm: its class's, and no less than the rules let a track of it be.
pub fn width(board: &Board, net: u32) -> f64 {
    let class = board.rules.class_of(board.net_name(net));
    (class.map_or(board.rules.min_track_width, |c| c.track_width.max(board.rules.min_track_width)) * 1e3).max(least_width(board, net))
}

/// The least a track of `net` may be wide, mm: the board's least, or what a custom rule asks
/// of the net (a supply rail's width for its current).
fn least_width(board: &Board, net: u32) -> f64 {
    (board.rules.min_track_width * 1e3).max(board.min_track_width_of(net) as f64 / 1e6)
}

/// The track width a connection is laid at, mm: its net's class's, unless it has been made
/// narrower.
pub fn wide(board: &Board, want: &Want) -> f64 {
    want.narrow.unwrap_or_else(|| width(board, want.net))
}

/// A track is never laid narrower than this share of what its net's class asks for.
const NARROWEST: f64 = 0.25;

/// Makes a connection narrower, if it may be: half as wide, and no less than the least the
/// board's rules let a track of its net be, nor than [`NARROWEST`] of its class's width (a
/// board may give no least at all). A class's width is what its tracks are drawn at by default, and no rule:
/// the designers lay narrower where that is what fits (KiwiSDR's antenna board asks for 1 mm,
/// and 261 of its 420 pieces of track are 0.25 mm).
fn narrower(board: &Board, want: &mut Want) -> bool {
    let now = wide(board, want);
    let least = least_width(board, want.net).max(NARROWEST * width(board, want.net));
    if want.stub || now <= least + 1e-9 {
        return false;
    }
    want.narrow = Some((now / 2.0).max(least));
    true
}

/// What a strand on an inner plane layer costs, as a multiple of its length: nothing is laid
/// there. A plane is the designer's; a track across it cuts it (owner, 2026-11-16: both
/// parallelogram boards had their In1 ground plane cut when this was 4). Vias still pass through.
const ON_A_PLANE: f64 = f64::INFINITY;
/// What a strand on a plane layer that is open costs: a poured outer side (its pads must get
/// out) or an inner plane whose rule areas say what may cross it.
const ON_A_POURED_SIDE: f64 = 4.0;
/// What a via costs, as length, mm: one is worth a detour this long.
const VIA: f64 = 8.0;

/// What a via into the net's pour costs, as length, mm: less than a via between two tracks,
/// for it ends the connection where it stands.
const DROP: f64 = 3.0;

/// A connection to plan: a net between two pads.
#[derive(Debug, Clone, Serialize)]
pub struct Want {
    pub net: u32,
    pub from: ItemId,
    pub to: ItemId,
    /// The straight line between its ends, mm.
    pub apart: f64,
    /// Spots that stand in the net's pour, in a piece of it that `to` is joined to: a via at
    /// one is as good an end as `to`.
    pub into: Vec<usize>,
    /// Gaps its way may not cross: those a wire for it could not be drawn through.
    pub avoid: Vec<space::GapKey>,
    /// A stub: a pad's way to a via beside it at one of the spots of `into`, where it ends
    /// (`to` is the pad again). Other connections of the pad may start from that via.
    pub stub: bool,
    /// The track width it is laid at, mm, where that is less than its net's class asks for:
    /// it had no way at that width, or its wire could not be drawn.
    pub narrow: Option<f64>,
}

impl Want {
    /// A connection for an airline.
    pub fn of(line: &board::Airline) -> Want {
        Want { net: line.net, from: line.from_item, to: line.to_item, apart: line.from.distance(line.to) / 1e6, into: Vec::new(), avoid: Vec::new(), stub: false, narrow: None }
    }
}

/// The radius and drill radius of a via of `net`, nm: its class's (net 0: the default class's),
/// no less than the board's least, and with no less copper round its hole than the board's
/// least ring (a class's 0.45 mm via drilled 0.3 mm, on a board whose least ring is 0.1 mm,
/// is 0.5 mm across). A power class's via is bigger than a signal's.
pub fn via_size(board: &Board, net: u32) -> (i64, i64) {
    let class = board.rules.class_of(board.net_name(net)).or_else(|| board.rules.class_of(""));
    let drill = class.as_ref().map_or(0.0, |c| c.via_drill).max(board.rules.min_via_drill);
    let d = class.as_ref().map_or(0.0, |c| c.via_diameter).max(board.rules.min_via_diameter).max(drill + 2.0 * board.rules.min_via_annular_width);
    (board::nm(d) / 2, board::nm(drill) / 2)
}

/// A via of `net` at `at`, through every layer.
pub(crate) fn via_item(board: &Board, at: space::P, net: u32) -> board::Item {
    let (r, drill) = via_size(board, net);
    let at = board::Point::new((at[0] * 1e6).round() as i64, (at[1] * 1e6).round() as i64);
    board::Item { kind: Kind::Via, net, layers: (1u64 << board.layers.len()) - 1, shape: board::Shape::Capsule { a: at, b: at, r }, hole: Some((at, drill)), locked: false, span: None }
}

/// Places a via could stand: a grid over the board, at a pitch that lets a track pass between
/// two neighbours, and the point farthest from the corners of each free triangle of the bare
/// layers; each kept only where the exact checker lets a via of no net stand on the bare board.
/// Whether via `id`, of no net, on `board` could stand there for some net: it breaks no rule but
/// those a custom rule makes by net (`disallow via` in a rule area that lets some nets' vias
/// through, the RP2350's ring). Those are asked again with the real net ([`Spots::alone`]) when
/// a via is put in; tested as net 0 they took every spot in the area from every net.
fn for_some_net(board: &Board, id: ItemId) -> bool {
    board::check_item(board, id).iter().all(|v| (v.rule == board::Rule::Keepout && v.b.is_none() && v.required == 0) || v.rule == board::Rule::KeptJoined)
}

pub fn via_spots(board: &Board, tight: bool) -> Spots {
    let (r, drill) = via_size(board, 0);
    let centre = board::Point::new(0, 0);
    // What a track keeps from a via's copper: its clearance, or more where the via's ring is
    // narrower than the rules keep copper from a hole.
    let hole = space::hole_keep(board, &board::Shape::Capsule { a: centre, b: centre, r }, (centre, centre, drill));
    let keep = board.clearance(0, 0).max(board::nm(hole / 1e3));
    // Two vias as near as they may stand: copper from copper, copper from the other's hole,
    // hole from hole.
    let apart = (2 * r + board.clearance(0, 0)).max(r + drill + board::nm(board.rules.min_hole_clearance)).max(2 * drill + board::nm(board.rules.min_hole_to_hole));
    // (A board whose outline does not close has the box round it for one.)
    let bounds = space::bounds(board);
    let outer = bounds.first();
    let box_ = outer.map_or(board::BBox::of([board::Point::new(0, 0)]), |o| board::BBox::of(o.iter().copied()));
    // A little over what a track between two vias needs: a via stands in the triangulation as
    // a triangle about its middle, and the gap between two is that much short of their pitch.
    let pitch = 2 * r + 2 * keep + board::nm(board.rules.min_track_width.max(width(board, 0) / 1e3)) + (8.0 * space::VIA_CORNER * 1e6) as i64;
    // For fan-outs: as near each other as two vias may stand.
    let pitch = if tight { apart + (8.0 * space::VIA_CORNER * 1e6) as i64 } else { pitch };
    let mut scratch = board.clone();
    let mut at = Vec::new();
    let mut y = box_.min.y + pitch / 2;
    while y < box_.max.y {
        let mut x = box_.min.x + pitch / 2;
        while x < box_.max.x {
            let p = board::Point::new(x, y);
            let id = scratch.insert(via_item(board, space::mm(p), 0));
            if outer.is_none_or(|o| board::inside(p, o)) && for_some_net(&scratch, id) {
                at.push(space::mm(p));
            }
            scratch.remove(id);
            x += pitch;
        }
        y += pitch;
    }
    // And where the room is: for each free triangle of each bare layer, the point as far from
    // all three of its corners as can be (the middle of the circle through them), unless a spot
    // is already near. Between the pads of a small crowded board the grid may have no point
    // where a via fits, and the corners are what a via has to keep from.
    let none = Arc::new(Spots::default());
    for l in 0..board.layers.len() {
        let bare = Space::new(board, l, &[], &none);
        for (t, tri) in bare.tris.iter().enumerate() {
            if !bare.free(t) {
                continue;
            }
            let [a, b, c] = tri.v.map(|v| bare.verts[v]);
            let d = 2.0 * (a[0] * (b[1] - c[1]) + b[0] * (c[1] - a[1]) + c[0] * (a[1] - b[1]));
            if d.abs() < 1e-12 {
                continue;
            }
            let sq = |p: space::P| p[0] * p[0] + p[1] * p[1];
            let p = [(sq(a) * (b[1] - c[1]) + sq(b) * (c[1] - a[1]) + sq(c) * (a[1] - b[1])) / d, (sq(a) * (c[0] - b[0]) + sq(b) * (a[0] - c[0]) + sq(c) * (b[0] - a[0])) / d];
            if at.iter().any(|&q| space::dist(p, q) < NEAR) {
                continue;
            }
            let q = board::Point::new((p[0] * 1e6).round() as i64, (p[1] * 1e6).round() as i64);
            let id = scratch.insert(via_item(board, space::mm(q), 0));
            if outer.is_none_or(|o| board::inside(q, o)) && for_some_net(&scratch, id) {
                at.push(space::mm(q));
            }
            scratch.remove(id);
        }
    }
    let apart = apart as f64 / 1e6;
    // Not for fan-outs: there each pad's via has its own place, beside its neighbours'.
    let ways_out = if tight { Vec::new() } else { ways_out(board, apart, pitch as f64 / 1e6) };
    let kept = Spots::kept_of(board, &at);
    Spots { at, radius: r as f64 / 1e6, apart, hole, ways_out, out_keep: (r + keep) as f64 / 1e6 + width(board, 0) / 2.0, beside: (2 * r + board.max_rule_reach()) as f64 / 1e6 + 0.01, checked: Default::default(), bars: space::bars(board), kept, left_out: Default::default(), spared: Vec::new(), holes: Vec::new(), wanted: Vec::new(), middles: space::middles(board), openings: space::openings(board) }
}

/// The way straight out of each pad that stands nearer another pad of its part than two vias
/// may stand (`apart`), and is longer than it is wide: from its end away from the part's
/// middle, `far` mm on along its length, with its net.
fn ways_out(board: &Board, apart: f64, far: f64) -> Vec<(space::P, space::P, u32)> {
    let mid = |id: ItemId| board.bbox(id).map(|b| [(b.min.x + b.max.x) as f64 / 2e6, (b.min.y + b.max.y) as f64 / 2e6]);
    let pads: Vec<(ItemId, usize, u64, space::P, u32)> = board
        .items()
        .filter_map(|(id, item)| match item.kind {
            Kind::Pad { footprint, .. } if item.layers.count_ones() == 1 => Some((id, footprint, item.layers, mid(id)?, item.net)),
            _ => None,
        })
        .collect();
    let mut parts: std::collections::HashMap<(usize, u64), Vec<usize>> = std::collections::HashMap::new();
    for (k, p) in pads.iter().enumerate() {
        parts.entry((p.1, p.2)).or_default().push(k);
    }
    let mut out = Vec::new();
    for of in parts.values() {
        let n = of.len() as f64;
        let middle = of.iter().fold([0.0, 0.0], |c, &k| [c[0] + pads[k].3[0] / n, c[1] + pads[k].3[1] / n]);
        for &k in of {
            let (id, _, _, at, net) = pads[k];
            if net == 0 || !of.iter().any(|&j| j != k && space::dist(pads[j].3, at) < apart) {
                continue;
            }
            // Its length: along its longest side, where it is half as long again as it is wide.
            let Some(item) = board.item(id) else { continue };
            let poly = space::outline(&item.shape);
            let side = |i: usize| [poly[(i + 1) % poly.len()][0] - poly[i][0], poly[(i + 1) % poly.len()][1] - poly[i][1]];
            let Some(long) = (0..poly.len()).map(side).max_by(|a, b| a[0].hypot(a[1]).total_cmp(&b[0].hypot(b[1]))) else { continue };
            let l = long[0].hypot(long[1]);
            if l < 1e-9 {
                continue;
            }
            let long = [long[0] / l, long[1] / l];
            let span = |d: [f64; 2]| {
                let on = poly.iter().map(|q| q[0] * d[0] + q[1] * d[1]);
                on.clone().fold(f64::MIN, f64::max) - on.fold(f64::MAX, f64::min)
            };
            let (len, wide) = (span(long), span([-long[1], long[0]]));
            if len < 1.5 * wide {
                continue;
            }
            let mut dir = long;
            if dir[0] * (at[0] - middle[0]) + dir[1] * (at[1] - middle[1]) < 0.0 {
                dir = [-dir[0], -dir[1]];
            }
            let tip = [at[0] + dir[0] * len / 2.0, at[1] + dir[1] * len / 2.0];
            out.push((tip, [tip[0] + dir[0] * far, tip[1] + dir[1] * far], net));
        }
    }
    out
}

/// A spot in a triangle is left out when another spot is this near, mm.
const NEAR: f64 = 0.3;

/// The vias in the sheets: all but the holes in pads ([`Spots::holes`]) that no strand ends at,
/// which stand ready and are not drilled.
fn vias_of(sheets: &[Sheet]) -> Vec<Via> {
    let Some(first) = sheets.first() else { return Vec::new() };
    let holes = first.space.spots.holes.len();
    let ended_at = |site: usize| sheets.iter().any(|s| s.laid().any(|k| [s.strands[k].from, s.strands[k].to].contains(&End::Site(site))));
    first.space.vias().into_iter().enumerate().filter(|&(site, _)| site >= holes || ended_at(site)).map(|(site, (at, net))| Via { net, site, at }).collect()
}

/// Whether the exact checker passes every via in the sheets, all on the board together: a spot
/// was only ever tried alone, on the bare board, and two vias can together cut more of a pad's
/// thermal spokes than one may.
fn stand(board: &Board, sheets: &[Sheet]) -> bool {
    let mut with = board.clone();
    let ids: Vec<ItemId> = vias_of(sheets).iter().map(|v| with.insert(via_item(board, v.at, v.net))).collect();
    ids.into_iter().all(|id| board::check_item(&with, id).is_empty())
}

/// The sheets with a way in them: none if they do not hold it. A way on one layer is laid as it
/// was found. A way with vias has its vias put into every layer's triangulation, and each of
/// its legs is read off against the new triangles with the strands that were there, as a line
/// between the strands it was found between.
fn put(board: &Board, sheets: &[Sheet], connection: usize, want: &Want, way: &ways::Way) -> Option<Vec<Sheet>> {
    let w = wide(board, want);
    let mut spots: Vec<usize> = Vec::new();
    for stop in way.legs.iter().flat_map(|l| [l.from, l.to]) {
        if let Stop::Spot(c) = stop
            && !spots.contains(&c)
        {
            spots.push(c);
        }
    }
    if spots.is_empty() {
        let mut out = sheets.to_vec();
        for leg in &way.legs {
            let (Stop::End(from), Stop::End(to)) = (leg.from, leg.to) else { return None };
            out[leg.layer].lay(want.net, connection, w, from, to, &leg.steps);
        }
        return way.legs.iter().all(|l| out[l.layer].sound(board)).then_some(out);
    }
    let first = sheets.first()?;
    let known = first.space.spots.clone();
    let mut vias = first.space.vias();
    let before = vias.len();
    vias.extend(spots.iter().map(|&c| (known.at[c], want.net)));
    let end = |stop: Stop| match stop {
        Stop::End(e) => e,
        Stop::Spot(c) => End::Site(before + spots.iter().position(|&x| x == c).unwrap_or(0)),
    };
    // Every crossing the way makes on each layer, in the way's order: where two are in one
    // place of one gap, the later is the nearer the gap's first corner, as when one is laid.
    let all: Vec<Vec<(usize, usize)>> = (0..sheets.len()).map(|l| way.legs.iter().filter(|leg| leg.layer == l).flat_map(|leg| leg.steps.iter().copied()).collect()).collect();
    let mut seen = vec![0; sheets.len()];
    let mut fresh: Vec<Vec<carry::Trace>> = vec![Vec::new(); sheets.len()];
    for leg in &way.legs {
        let sheet = &sheets[leg.layer];
        let places = sheet.places(board);
        let mut pts = Vec::new();
        if let Stop::Spot(c) = leg.from {
            pts.push(known.at[c]);
        }
        for &(e, slot) in &leg.steps {
            let on = &sheet.on[e];
            let lo = if slot > 0 { places[on[slot - 1].0][on[slot - 1].1] } else { 0.0 };
            let hi = if slot < on.len() { places[on[slot].0][on[slot].1] } else { 1.0 };
            let here = &all[leg.layer];
            let same = here.iter().filter(|&&x| x == (e, slot)).count();
            let later = here[seen[leg.layer] + 1..].iter().filter(|&&x| x == (e, slot)).count();
            seen[leg.layer] += 1;
            pts.push(sheet.point(e, lo + (hi - lo) * (later + 1) as f64 / (same + 1) as f64));
        }
        if let Stop::Spot(c) = leg.to {
            pts.push(known.at[c]);
        }
        let side = |step: Option<&(usize, usize)>| {
            let [a, b] = sheet.space.edges[step?.0].v.map(|v| sheet.space.verts[v]);
            Some(carry::Anchor::Side(a, b))
        };
        let anchor = |stop: Stop, step: Option<&(usize, usize)>| match end(stop) {
            End::Site(k) if matches!(stop, Stop::Spot(_)) => Some(carry::Anchor::Via(k)),
            _ => side(step),
        };
        fresh[leg.layer].push(carry::Trace {
            strand: Strand { net: want.net, connection, from: end(leg.from), to: end(leg.to), width: w, crossings: Vec::new() },
            pts,
            start: anchor(leg.from, leg.steps.first())?,
            end: anchor(leg.to, leg.steps.last())?,
        });
    }
    // A strand a via stands just past is led round the far side of it: out to one side of the
    // via, past it, and back in on the other side.
    let mut pushes = way.pushes.clone();
    pushes.sort_by_key(|p| std::cmp::Reverse(p.3));
    let layers: Vec<(usize, &Sheet, Vec<carry::Trace>)> = sheets.iter().enumerate().zip(fresh).map(|((l, sheet), fresh)| (l, sheet, fresh)).collect();
    let out: Vec<Sheet> = side_by_side(layers, |(l, sheet, fresh)| {
            let mut traces = carry::traces(board, sheet, &Some)?;
            for &(spot, _, strand, k) in pushes.iter().filter(|p| p.1 == l) {
                let trace = traces.get_mut(sheet.laid().position(|s| s == strand)?)?;
                let (a, b, p) = (*trace.pts.get(k)?, *trace.pts.get(k + 1)?, known.at[spot]);
                let from = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
                let d = space::dist(from, p);
                if d < 1e-9 {
                    return None;
                }
                let by = PAST * space::VIA_CORNER;
                let out = [(p[0] - from[0]) / d * by, (p[1] - from[1]) / d * by];
                let mut side = [-out[1], out[0]];
                if side[0] * (a[0] - from[0]) + side[1] * (a[1] - from[1]) < 0.0 {
                    side = [-side[0], -side[1]];
                }
                trace.pts.splice(k + 1..k + 1, [[p[0] + side[0], p[1] + side[1]], [p[0] + out[0], p[1] + out[1]], [p[0] - side[0], p[1] - side[1]]]);
            }
            traces.extend(fresh);
            carry::sheet(board, sheet.space.layer, &vias, &known, &traces)
        })
        .into_iter()
        .collect::<Option<_>>()?;
    stand(board, &out).then_some(out)
}

/// A strand led round a via passes this many times [`space::VIA_CORNER`] from its middle:
/// past the via's corners, and short of where a spot is too near a strand to be beside it.
const PAST: f64 = 2.5;

/// The vias a connection's strands end in, by their numbers in the sheets.
fn sites_of(sheets: &[Sheet], connection: usize) -> Vec<usize> {
    let mut sites: Vec<usize> = sheets
        .iter()
        .flat_map(|s| s.laid().map(move |k| &s.strands[k]))
        .filter(|s| s.connection == connection)
        .flat_map(|s| [s.from, s.to])
        .filter_map(|e| if let End::Site(k) = e { Some(k) } else { None })
        .collect();
    sites.sort_unstable();
    sites.dedup();
    sites
}

/// Where a connection may start and where it may end: each pad, the pads joined to it before
/// anything is laid or by the strands laid since (a pad walled in by its neighbours, with a
/// short strand to the pad beside it that faces open board, is reached by way of that one, as
/// a designer wires it), the via the stub of any of them ends in, and the hole in any of them
/// ([`Spots::holes`]). A stub starts at its pad and has no end but the via it puts in.
fn ends(sheets: &[Sheet], wants: &[Want], joined: &Joined, connection: usize) -> (Vec<End>, Vec<End>) {
    let want = &wants[connection];
    if want.stub {
        return (vec![End::Item(want.from)], Vec::new());
    }
    let holes = sheets.first().map_or(&[][..], |s| s.space.spots.holes.as_slice());
    // What the strands laid join, of this net: the two ends of each, a strand of the connection
    // itself aside. They are read as they lie, not as their connections were asked for: one
    // asked for between two pads may have been laid to a third.
    let mut links: std::collections::HashMap<End, Vec<End>> = std::collections::HashMap::new();
    for strand in sheets.iter().flat_map(|s| s.laid().map(move |k| &s.strands[k])).filter(|s| s.net == want.net && s.connection != connection) {
        links.entry(strand.from).or_default().push(strand.to);
        links.entry(strand.to).or_default().push(strand.from);
    }
    let reach = |pad: ItemId| -> Vec<ItemId> {
        let mut pads = vec![pad];
        let mut seen: std::collections::HashSet<End> = std::collections::HashSet::from([End::Item(pad)]);
        let mut todo = vec![End::Item(pad)];
        while let Some(end) = todo.pop() {
            let mut next: Vec<End> = links.get(&end).cloned().unwrap_or_default();
            match end {
                End::Item(p) => {
                    next.extend(joined.get(&p).into_iter().flatten().map(|&q| End::Item(q)));
                    next.extend(holes.iter().enumerate().filter(|(_, h)| h.0 == p).map(|(site, _)| End::Site(site)));
                }
                End::Site(site) => next.extend(holes.get(site).map(|h| End::Item(h.0))),
            }
            for n in next {
                if seen.insert(n) {
                    if let End::Item(p) = n {
                        pads.push(p);
                    }
                    todo.push(n);
                }
            }
        }
        pads
    };
    let (from, mut to) = (reach(want.from), reach(want.to));
    // Its two pads joined already, by connections laid to pads other than their own: it joins
    // them to what is still apart, the pads of the net's other connections that they do not
    // reach.
    if from.contains(&want.to) {
        let mut apart: Vec<ItemId> = wants.iter().filter(|w| w.net == want.net && !w.stub).flat_map(|w| [w.from, w.to]).filter(|p| !from.contains(p)).collect();
        apart.sort();
        apart.dedup();
        if !apart.is_empty() {
            to = apart;
        }
    }
    let cluster = |pads: Vec<ItemId>| {
        let stubs = wants.iter().enumerate().filter(|(_, w)| w.stub && pads.contains(&w.from)).flat_map(|(s, _)| sites_of(sheets, s)).map(End::Site).collect::<Vec<End>>();
        let holed = holes.iter().enumerate().filter(|(_, h)| pads.contains(&h.0)).map(|(site, _)| End::Site(site)).collect::<Vec<End>>();
        pads.into_iter().map(End::Item).chain(stubs).chain(holed).collect::<Vec<End>>()
    };
    (cluster(from), cluster(to))
}

/// How many times a way whose vias the sheets do not hold is looked for again without them.
const AGAIN: usize = 6;

/// Lays a connection along the cheapest way there is for it: none with no way, or with none
/// the sheets hold. A way with vias that they do not hold is looked for again with those
/// spots left out: the search tries a via against the corners of the triangle it would stand
/// in, and what it does to the gaps round it is only known once it is in.
fn lay(board: &Board, sheets: &[Sheet], terms: &ways::Terms, wants: &[Want], joined: &Joined, connection: usize) -> Option<(f64, Vec<Sheet>)> {
    let want = &wants[connection];
    let (from, to) = ends(sheets, wants, joined, connection);
    let mut ban: Vec<usize> = terms.ban.to_vec();
    for _ in 0..AGAIN {
        let way = ways::find(board, sheets, &ways::Terms { ban: &ban, into: &want.into, avoid: &want.avoid, ..*terms }, want.net, wide(board, want), &from, &to)?;
        if let Some(out) = put(board, sheets, connection, want, &way) {
            return Some((way.cost, out));
        }
        let before = ban.len();
        ban.extend(way.legs.iter().flat_map(|l| [l.from, l.to]).filter_map(|s| if let Stop::Spot(c) = s { Some(c) } else { None }));
        if ban.len() == before {
            return None;
        }
    }
    None
}

/// A way on one layer is taken without a thought for vias when it is no longer than this many
/// times the straight line between its ends, plus [`SLACK`] mm.
const DIRECT: f64 = 2.0;
const SLACK: f64 = 4.0;

/// The sheets with every strand of a connection lifted and the vias it used taken out: none if
/// the strands that stay cannot be read off against the triangles without those vias.
fn lift(board: &Board, sheets: &[Sheet], connection: usize) -> Option<Vec<Sheet>> {
    let mut out = sheets.to_vec();
    let mut gone = Vec::new();
    for sheet in out.iter_mut() {
        for k in sheet.laid().collect::<Vec<_>>() {
            if sheet.strands[k].connection == connection {
                for end in [sheet.strands[k].from, sheet.strands[k].to] {
                    if let End::Site(site) = end
                        && !gone.contains(&site)
                    {
                        gone.push(site);
                    }
                }
                sheet.lift(k);
            }
        }
    }
    // A via another connection's strand still ends in stays: a stub's, while it is used. So
    // does a hole in a pad, the first of the vias, which is the pad's and no connection's.
    let holes = out.first().map_or(0, |s| s.space.spots.holes.len());
    gone.retain(|&site| site >= holes && !out.iter().any(|s| s.laid().any(|k| [s.strands[k].from, s.strands[k].to].contains(&End::Site(site)))));
    if gone.is_empty() {
        return Some(out);
    }
    let first = out.first()?;
    let number = |k: usize| (!gone.contains(&k)).then(|| k - gone.iter().filter(|&&g| g < k).count());
    let vias: Vec<(space::P, u32)> = first.space.vias().into_iter().enumerate().filter(|(k, _)| !gone.contains(k)).map(|(_, v)| v).collect();
    let spots = first.space.spots.clone();
    side_by_side(out.iter().collect(), |sheet| carry::sheet(board, sheet.space.layer, &vias, &spots, &carry::traces(board, sheet, &number)?)).into_iter().collect()
}

/// A layer with fewer edges than this is not worth a thread of its own.
const WORTH_A_THREAD: usize = 2000;

/// `f` of each layer, the layers side by side on a thread each where they are big enough to
/// be worth it: every layer is triangulated again when a via goes in or comes out, and one
/// layer's work asks nothing of another's. The answers are in the layers' order.
fn side_by_side<T: Send + Layer, R: Send>(layers: Vec<T>, f: impl Fn(T) -> R + Sync) -> Vec<R> {
    if layers.len() < 2 || layers.iter().all(|l| l.edges() < WORTH_A_THREAD) {
        return layers.into_iter().map(f).collect();
    }
    let f = &f;
    std::thread::scope(|scope| {
        let threads: Vec<_> = layers.into_iter().map(|layer| scope.spawn(move || f(layer))).collect();
        threads.into_iter().map(|t| t.join().unwrap_or_else(|panic| std::panic::resume_unwind(panic))).collect()
    })
}

/// What [`side_by_side`] is given for a layer: how many edges its triangles have.
trait Layer {
    fn edges(&self) -> usize;
}

impl Layer for &Sheet {
    fn edges(&self) -> usize {
        self.space.edges.len()
    }
}

impl Layer for (usize, &Sheet, Vec<carry::Trace>) {
    fn edges(&self) -> usize {
        self.1.space.edges.len()
    }
}

/// Whether another connection's strand ends in a via of connection `c`: for a stub, whether
/// it is used.
fn used(sheets: &[Sheet], c: usize) -> bool {
    let sites = sites_of(sheets, c);
    sheets.iter().any(|s| s.laid().any(|k| s.strands[k].connection != c && [s.strands[k].from, s.strands[k].to].iter().any(|e| matches!(e, End::Site(x) if sites.contains(x)))))
}

/// What going across a strand is counted as, as length, mm: the connection it belongs to has
/// to be laid again another way, round or by vias.
const CROSS: f64 = 12.0;
/// A way is worth lifting others for when it is this much shorter than the best there is
/// without, mm.
const WORTH: f64 = 2.0;
/// How many different sets of connections are lifted for one that has no way.
const OTHERS: usize = 4;
/// How many times a way across others is looked for again without the spots of one that goes
/// across nothing and that the sheets do not hold.
const ACROSS_AGAIN: usize = 24;
/// How deep lifting goes: a connection lifted to let one through may itself have others lifted
/// to be laid again, this many times over.
const DEEP: usize = 2;

/// A wire in 0/45/90 is at most this many times as long as its strand pulled tight: a line
/// half-way between two of its eight directions, and a little for its bends.
const DRAWN: f64 = 1.1;

/// What the planner is told of the board's layers, and each connection with the length a way
/// for it on one layer may be and still be taken without a thought for vias.
struct Job<'a> {
    board: &'a Board,
    dear: &'a [f64],
    wants: &'a [Want],
    joined: &'a Joined,
}

impl Job<'_> {
    fn terms(&self, via: bool, cross: bool) -> ways::Terms<'_> {
        ways::Terms { dear: self.dear, via: via.then_some(VIA), cross: cross.then_some(CROSS), ban: &[], keep: &[], into: &[], drop: DROP, avoid: &[] }
    }

    /// Lays connection `c` the best way there is with the sheets as they are. A way on one
    /// layer that is nearly direct is taken. A long way round is weighed against the way with
    /// vias, each via counted as [`VIA`] of length.
    fn best(&self, sheets: &[Sheet], c: usize) -> Option<(f64, Vec<Sheet>)> {
        let want = &self.wants[c];
        let with_vias = self.terms(true, false);
        if want.stub {
            return lay(self.board, sheets, &with_vias, self.wants, self.joined, c);
        }
        // A way that takes its net past the length a rule holds it to is no way.
        let within = |laid: Option<(f64, Vec<Sheet>)>| laid.filter(|l| self.within(&l.1, want.net));
        match within(lay(self.board, sheets, &self.terms(false, false), self.wants, self.joined, c)) {
            // With a pour to end in, a via into it is always weighed against the way without.
            Some(on_one) if want.into.is_empty() && on_one.0 <= DIRECT * want.apart + SLACK => Some(on_one),
            on_one => [on_one, within(lay(self.board, sheets, &with_vias, self.wants, self.joined, c))].into_iter().flatten().min_by(|a, b| a.0.total_cmp(&b.0)),
        }
    }

    /// Whether `net`'s strands in `sheets` are no longer than a custom rule lets the net's
    /// copper be (`length (max …)`: a flash chip's lines, a crystal's), as KiCad counts it:
    /// its tracks and the height of its vias. A strand is a rubber band, and its wire in
    /// 0/45/90 is longer: [`DRAWN`] times as long at most. A net so held is laid first, while
    /// its straight way is open; without this, lifted to let another through, it was laid
    /// again any way round (parallelogram p2-132-usb6's flash select, 9 mm from pad to pad
    /// and held to 25: laid 27 to 38 mm long, 2026-11-24).
    fn within(&self, sheets: &[Sheet], net: u32) -> bool {
        let Some((_, max)) = self.board.net_maximum("length", net) else { return true };
        let tracks: f64 = sheets.iter().map(|s| s.laid().filter(|&k| s.strands[k].net == net).map(|k| s.sketch(self.board, k).windows(2).map(|w| space::dist(w[0], w[1])).sum::<f64>()).sum::<f64>()).sum();
        let vias = sheets.first().map_or(0, |s| s.space.vias().iter().filter(|v| v.1 == net).count());
        tracks * DRAWN + vias as f64 * self.board.via_height as f64 / 1e6 <= max * 1e3
    }

    /// What a plan costs: its strands' lengths, each on a layer as dear as that layer is, and
    /// its vias.
    fn cost(&self, sheets: &[Sheet]) -> f64 {
        // A closed layer (dear without end) holds nothing, and nothing times its cost is nothing.
        sheets.iter().zip(self.dear).map(|(s, dear)| match s.length(self.board) { len if len > 0.0 => dear * len, _ => 0.0 }).sum::<f64>() + VIA * sheets.first().map_or(0, |s| s.space.sites.len()) as f64
    }

    /// What the cheapest way for `c` would cost if it might go across other connections'
    /// strands, and which it would go across: none of those in `keep`.
    ///
    /// With `stuck`, `c` is known to have no way the sheets hold ([`Job::best`] found none): a
    /// way that goes across nothing is then one whose vias the sheets do not hold, and it is
    /// looked for again without those spots, as [`lay`] does, until one goes across something
    /// or there is none.
    fn across(&self, sheets: &[Sheet], c: usize, keep: &[usize], stuck: bool) -> Option<(f64, Vec<usize>)> {
        let want = &self.wants[c];
        // A stub that another connection starts from stays where it is.
        let keep: Vec<usize> = keep.iter().copied().chain((0..self.wants.len()).filter(|&s| self.wants[s].stub && used(sheets, s))).collect();
        let (from, to) = ends(sheets, self.wants, self.joined, c);
        let mut ban: Vec<usize> = Vec::new();
        for _ in 0..ACROSS_AGAIN {
            let way = ways::find(self.board, sheets, &ways::Terms { keep: &keep, ban: &ban, into: &want.into, avoid: &want.avoid, ..self.terms(true, true) }, want.net, wide(self.board, want), &from, &to)?;
            if !stuck || !way.over.is_empty() {
                return Some((way.cost, way.over));
            }
            let before = ban.len();
            ban.extend(way.legs.iter().flat_map(|l| [l.from, l.to]).filter_map(|s| if let Stop::Spot(c) = s { Some(c) } else { None }));
            if ban.len() == before {
                return None;
            }
        }
        None
    }

    /// Lays `c` by lifting the connections in its way and laying each of those again after
    /// it, shortest first: the sheets with all of them, if all then have a way. One of those
    /// that then has none may have others lifted for it in turn, `deep` times over.
    fn through(&self, sheets: &[Sheet], c: usize, deep: usize) -> Option<Vec<Sheet>> {
        // The connections that were lifted and could not all be laid again are left where
        // they are the next time, and another way across is looked for: a few times.
        let mut keep: Vec<usize> = Vec::new();
        for _ in 0..OTHERS {
            let (_, mut over) = self.across(sheets, c, &keep, false)?;
            if over.is_empty() {
                if let Some(best) = self.best(sheets, c) {
                    return Some(best.1);
                }
                over = self.across(sheets, c, &keep, true)?.1;
                if over.is_empty() {
                    return None;
                }
            }
            over.sort();
            if let Some(laid) = self.lifting(sheets, c, &over, deep) {
                return Some(laid);
            }
            keep.extend(over);
        }
        None
    }

    /// The sheets with `over` lifted, `c` laid, and each of `over` laid again.
    fn lifting(&self, sheets: &[Sheet], c: usize, over: &[usize], deep: usize) -> Option<Vec<Sheet>> {
        let mut tried = sheets.to_vec();
        for &o in over {
            tried = lift(self.board, &tried, o)?;
        }
        tried = self.best(&tried, c)?.1;
        for &o in over {
            tried = match self.best(&tried, o) {
                Some((_, laid)) => laid,
                None if deep > 0 => self.through(&tried, o, deep - 1)?,
                None => return None,
            };
        }
        Some(tried)
    }
}

/// Plans every open connection of a stripped board. A pour joins nothing here: tracks laid
/// across it can cut a pad off from the rest of it, so a pour's net is planned like any other
/// (`wire::route` leaves such a net to its pour, and plans what the pour leaves apart).
pub fn plan(board: &Board) -> Plan {
    let mut plan = start(board);
    add(board, &mut plan, wants(&board::connectivity_without_pours(board).airlines));
    plan
}

/// Airlines as connections to plan, shortest first.
pub fn wants(airlines: &[board::Airline]) -> Vec<Want> {
    let mut wants: Vec<Want> = airlines.iter().map(Want::of).collect();
    // PLAN_JITTER=SEED (for measuring only): the order shaken a little, each length taken as
    // up to 3% more or less than it is, to see how far a result turns on the order alone.
    if let Some(seed) = std::env::var("PLAN_JITTER").ok().and_then(|s| s.parse::<u64>().ok()) {
        let shake = |w: &Want| {
            let mut h = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ ((w.apart * 1e6) as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9) ^ (w.net as u64).wrapping_mul(0x94D0_49BB_1331_11EB);
            h ^= h >> 31;
            h = h.wrapping_mul(0xD6E8_FEB8_6659_FD93);
            h ^= h >> 29;
            w.apart * (1.0 + 0.03 * ((h % 2001) as f64 / 1000.0 - 1.0))
        };
        wants.sort_by(|a, b| shake(a).total_cmp(&shake(b)));
        return wants;
    }
    wants.sort_by(|a, b| a.apart.total_cmp(&b.apart));
    wants
}

/// A plan with nothing in it yet: the board's layers cut into triangles.
pub fn start(board: &Board) -> Plan {
    start_from(board, via_spots(board, false))
}

/// How far from its pad a stub's via may stand, mm.
const FAN_REACH: f64 = 4.0;

/// How many triangles the room round a pad is followed through before it is taken not to be
/// shut in: a pad shut in stands in a pocket, a few triangles between its neighbours.
const SHUT_IN: usize = 300;
/// How many times spokes are given up and the layers cut into triangles again.
const SPARINGS: usize = 3;

/// The pins with a hole whose spokes shut a pad in, each with the layer to spare them on
/// ([`Spots::spared`]). A pad is shut in when a strand of its net, at the narrowest it may be
/// and with nothing else laid, reaches no other pad of the net from it on any layer, through
/// any spot a via could stand at. The spokes spared are those at the gaps too narrow for it
/// round the room it does reach, on layers other than the one the pin keeps them on: the one
/// where it has the most, an inner layer before an outer one (wires are fewest there), the
/// first of those.
fn shut_in_by_spokes(board: &Board, sheets: &[Sheet], spots: &Spots) -> Vec<(ItemId, usize)> {
    // Each pin's spokes on each layer, and how many it needs.
    let mut on_each: std::collections::HashMap<ItemId, (usize, Vec<usize>)> = std::collections::HashMap::new();
    for (_, item) in board.items() {
        if let Kind::Spoke { pad, need, .. } = item.kind {
            let count = on_each.entry(pad).or_insert_with(|| (need as usize, vec![0; board.layers.len()]));
            for (l, n) in count.1.iter_mut().enumerate() {
                *n += item.on(l) as usize;
            }
        }
    }
    let last = board.layers.len().saturating_sub(1);
    let kept_on = |pad: ItemId| -> Option<usize> {
        let (need, count) = on_each.get(&pad)?;
        (0..count.len()).filter(|&l| count[l] >= *need).max_by_key(|&l| (count[l], l != 0 && l != last, std::cmp::Reverse(l)))
    };
    // The pin a corner's obstacle is a spoke of, if it may be spared on that layer.
    let spare = |l: usize, v: usize| -> Option<ItemId> {
        let space = &sheets[l].space;
        let o = &space.obstacles[space.vert_of[v]?];
        let Kind::Spoke { pad, .. } = board.item(o.item?)?.kind else { return None };
        (o.what == space::What::Copper && kept_on(pad).is_some_and(|k| k != l)).then_some(pad)
    };
    if !(0..sheets.len()).any(|l| (0..sheets[l].space.verts.len()).any(|v| spare(l, v).is_some())) {
        return Vec::new();
    }
    // On each layer, the free triangles with a corner on each obstacle.
    let around: Vec<std::collections::HashMap<usize, Vec<usize>>> = sheets
        .iter()
        .map(|sheet| {
            let space = &sheet.space;
            let mut around: std::collections::HashMap<usize, Vec<usize>> = std::collections::HashMap::new();
            for t in (0..space.tris.len()).filter(|&t| space.free(t)) {
                for o in space.tris[t].v.iter().filter_map(|&v| space.vert_of[v]) {
                    if space.obstacles[o].what == space::What::Pad && around.get(&o).is_none_or(|a| a.last() != Some(&t)) {
                        around.entry(o).or_default().push(t);
                    }
                }
            }
            around
        })
        .collect();
    let mut out: Vec<(ItemId, usize)> = Vec::new();
    // The pads to ask it of: those of nets with another pad to reach and no pour to join them.
    let mut pads: Vec<(ItemId, u32)> = board.items().filter(|(_, i)| matches!(i.kind, Kind::Pad { .. }) && i.net != 0 && !board.zone_areas.iter().any(|z| z.net == i.net)).map(|(id, i)| (id, i.net)).collect();
    let mut of_net: std::collections::HashMap<u32, usize> = std::collections::HashMap::new();
    for &(_, net) in &pads {
        *of_net.entry(net).or_default() += 1;
    }
    pads.retain(|(_, net)| of_net[net] > 1);
    pads.sort();
    for (pad, net) in pads {
        let wide = least_width(board, net).max(NARROWEST * width(board, net));
        let mut seen: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
        let mut todo: Vec<(usize, usize)> = Vec::new();
        for (l, around) in around.iter().enumerate() {
            let Some(&o) = sheets[l].space.ends.get(&End::Item(pad)) else { continue };
            for &t in around.get(&o).map_or(&[][..], |a| a.as_slice()) {
                seen.insert((l, t));
                todo.push((l, t));
            }
        }
        let mut short: Vec<(usize, usize)> = Vec::new();
        let (mut steps, mut joined) = (0, todo.is_empty());
        while let Some((l, t)) = todo.pop() {
            let space = &sheets[l].space;
            steps += 1;
            // Another pad of the net, or more room than a pad shut in has.
            let other = |v: usize| space.vert_of[v].is_some_and(|k| space.obstacles[k].what == crate::space::What::Pad && space.obstacles[k].net == net && space.obstacles[k].item != Some(pad));
            if steps > SHUT_IN || space.tris[t].v.iter().any(|&v| other(v)) {
                joined = true;
                break;
            }
            for &k in &space.tri_spots[t] {
                for (m, sheet) in sheets.iter().enumerate() {
                    if let Some(u) = sheet.space.spot_tri[k]
                        && seen.insert((m, u))
                    {
                        todo.push((m, u));
                    }
                }
            }
            for &e in &space.tris[t].e {
                let Some(next) = space.edges[e].t.iter().flatten().copied().find(|&y| y != t) else { continue };
                if !space.free(next) || seen.contains(&(l, next)) || space.barred(next, net) {
                    continue;
                }
                if space.keep(board, space.edges[e].v[0], net) + space.keep(board, space.edges[e].v[1], net) + wide > space.width_for(e, net) + strands::EPS {
                    short.push((l, e));
                    continue;
                }
                seen.insert((l, next));
                todo.push((l, next));
            }
        }
        if joined {
            continue;
        }
        for (l, e) in short {
            let space = &sheets[l].space;
            let [a, b] = space.edges[e].v;
            for (v, w) in [(a, b), (b, a)] {
                // Not the gap between a spoke and its own pin, or two spokes of one pin:
                // nothing passes there with the spoke or without.
                let own = |pin: ItemId| spare(l, w) == Some(pin) || space.vert_of[w].is_some_and(|k| space.obstacles[k].item == Some(pin));
                if let Some(pin) = spare(l, v)
                    && !own(pin)
                    && !out.contains(&(pin, l))
                {
                    out.push((pin, l));
                }
            }
        }
    }
    out.retain(|x| !spots.spared.contains(x));
    out
}

/// How many places across a pad, each way, a hole in it is tried at.
const HOLE_TRIES: i64 = 5;

/// Whether a pad could hold a hole: it is on one layer, has none, its net may have vias, a
/// via of its net is no wider than it either way, and nothing is soldered to it. A hole in a
/// pad that takes solder draws the solder down it unless the board is made with its holes
/// filled and capped: where designers stand a via on such a pad of a board that is not, it
/// is at the pad's end or corner, and none of the gate tier's puts one in a ball of a grid
/// array (PocketBone's U2, 2026-11-21: every ball leaves by a short track to a via between the
/// balls). Such a pad's connection is left open, and said to be. On a board that is made so
/// ([`Board::filled_vias`]) a soldered pad holds one as a bare one does, and so does a pad
/// narrower than the via, a fine-pitch part's pin, if it is as wide as the via's hole (the
/// owner's parallelogram board as the old router left it, 2026-11-24: 152 of its 724 vias
/// stand in soldered pads, 29 of them in pins 0.2 mm wide).
fn holds_a_hole(board: &Board, pad: ItemId) -> bool {
    let (Some(item), Some(b)) = (board.item(pad), board.bbox(pad)) else { return false };
    let (r, drill) = via_size(board, item.net);
    let d = 2 * if board.filled_vias { drill } else { r };
    matches!(item.kind, Kind::Pad { .. }) && (board.filled_vias || !board.soldered.contains(&pad)) && item.net != 0 && item.layers.count_ones() == 1 && item.hole.is_none() && !board.no_vias(item.net) && b.max.x - b.min.x >= d && b.max.y - b.min.y >= d
}

/// A hole for each of `wanted` that can hold one: a via of the pad's net wholly inside the
/// pad's copper, at its middle or as near it as the exact checker lets one stand beside the
/// plan's vias (`standing`) and the holes already found. What a designer does with a grid of bare pads too close for a track
/// between them (the 120-channel test board of the gate tier: a via on each pad and its track
/// on the other side). On a board whose vias are filled and capped the via's hole is wholly
/// inside the pad's copper and its ring may reach past the pad's sides and its end (the old
/// router's stand anywhere along a pin, to its very end): the pad is then that much wider to
/// everything on its own layer ([`space::Space::new`]).
pub fn holes(board: &Board, wanted: &[ItemId], standing: &[(space::P, u32)]) -> Vec<(ItemId, space::P, u32)> {
    let mut scratch = board.clone();
    for &(at, net) in standing {
        scratch.insert(via_item(board, at, net));
    }
    let mut out = Vec::new();
    let mut pads: Vec<(ItemId, u32)> = wanted.iter().filter(|&&pad| holds_a_hole(board, pad)).filter_map(|&pad| Some((pad, board.item(pad)?.net))).collect();
    pads.sort();
    pads.dedup();
    for (pad, net) in pads {
        let Some(item) = board.item(pad) else { continue };
        let (Some(b), copper) = (board.bbox(pad), space::outline(&item.shape)) else { continue };
        let (r, drill) = via_size(board, net);
        // What of the via must lie inside the pad: all of it, or its hole.
        let within = if board.filled_vias { drill } else { r };
        let (cx, cy, hx, hy) = ((b.min.x + b.max.x) / 2, (b.min.y + b.max.y) / 2, (b.max.x - b.min.x) / 2 - within, (b.max.y - b.min.y) / 2 - within);
        if hx < 0 || hy < 0 || copper.len() < 3 {
            continue;
        }
        // The middle first, then outwards.
        let mut places: Vec<(i64, i64)> = (-HOLE_TRIES..=HOLE_TRIES).flat_map(|i| (-HOLE_TRIES..=HOLE_TRIES).map(move |j| (i, j))).filter(|&(i, j)| (hx > 0 || i == 0) && (hy > 0 || j == 0)).collect();
        places.sort_by_key(|&(i, j)| (i * i + j * j, i, j));
        let rim = |at: space::P| (0..16).all(|k| {
            let a = std::f64::consts::TAU * k as f64 / 16.0;
            space::inside([at[0] + within as f64 / 1e6 * a.cos(), at[1] + within as f64 / 1e6 * a.sin()], &copper)
        });
        for (i, j) in places {
            let at = [(cx + hx * i / HOLE_TRIES) as f64 / 1e6, (cy + hy * j / HOLE_TRIES) as f64 / 1e6];
            if !rim(at) {
                continue;
            }
            let id = scratch.insert(via_item(board, at, net));
            if board::check_item(&scratch, id).is_empty() {
                out.push((pad, at, net));
                break;
            }
            scratch.remove(id);
        }
    }
    out
}

/// A plan with nothing in it yet, with `spots` for its vias.
pub fn start_from(board: &Board, spots: Spots) -> Plan {
    // The layers cut into triangles; and again without the spokes that shut a pad in, for as
    // long as giving some up opens a way to more.
    let mut spots = Arc::new(spots);
    let mut made = 0;
    let sheets = loop {
        let sheets: Vec<Sheet> = (0..board.layers.len()).map(|l| Sheet::new(Space::new(board, l, &[], &spots))).collect();
        let more = if made == SPARINGS { Vec::new() } else { shut_in_by_spokes(board, &sheets, &spots) };
        if more.is_empty() {
            break sheets;
        }
        let mut with = (*spots).clone();
        with.spared.extend(more);
        spots = Arc::new(with);
        made += 1;
    };
    let poured = poured(board);
    // On two layers a poured side is still routed on: the designers do (159 of the gate
    // tier's 253 two-layer boards have a side at least half poured).
    let planes: Vec<usize> = if poured.len() <= 2 || poured.iter().all(|s| *s >= board::PLANE_SHARE) { Vec::new() } else { (0..poured.len()).filter(|&l| poured[l] >= board::PLANE_SHARE).collect() };
    let stated = |l: usize| spots.bars.iter().any(|b| !b.nets[l].is_empty());
    let closed = planes.iter().copied().filter(|&l| l != 0 && l + 1 != board.layers.len() && !stated(l)).collect();
    Plan { sheets, planes, closed, vias: Vec::new(), left: Vec::new(), wants: Vec::new(), joined: Arc::new(joined(board)), refused: Vec::new() }
}

/// The pads of the connections the plan found no way for that could hold a hole and have not
/// been tried with one ([`Spots::wanted`]).
pub fn wanting_holes(board: &Board, plan: &Plan) -> Vec<ItemId> {
    let Some(first) = plan.sheets.first() else { return Vec::new() };
    let spots = &first.space.spots;
    let mut pads: Vec<ItemId> = plan.left.iter().flat_map(|l| [l.from, l.to]).filter(|&pad| holds_a_hole(board, pad) && !spots.wanted.contains(&pad) && !plan.refused.contains(&pad) && !spots.holes.iter().any(|h| h.0 == pad)).collect();
    pads.sort();
    pads.dedup();
    pads
}

/// Gives a hole to each pad that could hold one of the connections the plan found no way for,
/// and lays those connections again: a pad shut in by its neighbours, or by their strands once
/// those have taken the way round, leaves by its own hole as a designer has it do. A strand
/// that lies where a hole goes on another layer is lifted and laid again with them, and so is
/// one the pad leaves too little room once it is the wider for its via. Where the plan cannot
/// take all the holes together it takes them one by one, and a pad whose hole it cannot take
/// at all is not asked about again ([`Plan::refused`]). False if no pad was given one.
pub fn hole(board: &Board, plan: &mut Plan) -> bool {
    let pads = wanting_holes(board, plan);
    let Some(first) = plan.sheets.first() else { return false };
    let standing = first.space.vias();
    let found = holes(board, &pads, &standing);
    if found.is_empty() {
        return false;
    }
    if holed(board, plan, &pads, &found) {
        return true;
    }
    let mut any = false;
    for h in &found {
        if holed(board, plan, &[h.0], std::slice::from_ref(h)) {
            any = true;
        } else {
            plan.refused.push(h.0);
        }
    }
    // The pads no hole was found for are not looked at again either.
    let Some(first) = plan.sheets.first() else { return any };
    let spots = &first.space.spots;
    plan.refused.extend(pads.iter().copied().filter(|p| !spots.wanted.contains(p) && !found.iter().any(|h| h.0 == *p)));
    any
}

/// How many times the strands a hole's pad leaves too little room are lifted and the holes
/// tried again ([`holed`]).
const HOLE_LIFTS: usize = 4;

/// Puts the holes `found` into the plan, for `pads` (those asked about, to be marked as tried),
/// and lays again what was in their way and the connections that have one now. False, with
/// the plan as it was, if the strands that stay cannot be carried onto the layers with the
/// holes in them.
fn holed(board: &Board, plan: &mut Plan, pads: &[ItemId], found: &[(ItemId, space::P, u32)]) -> bool {
    let Some(first) = plan.sheets.first() else { return false };
    let spots = first.space.spots.clone();
    // The strands in a hole's way, on any layer: nearer its middle than two vias may stand.
    let mut which: Vec<usize> = Vec::new();
    for sheet in &plan.sheets {
        let Some(traces) = carry::traces(board, sheet, &Some) else { return false };
        for t in &traces {
            if found.iter().any(|h| t.pts.windows(2).any(|s| space::point_segment(h.1, s[0], s[1]) < spots.apart)) {
                which.push(t.strand.connection);
            }
        }
    }
    // A stub in the way goes for good, as one that cannot be drawn does ([`relay`]): what
    // started from its via is laid again from the pad, which may have a hole now.
    let from_stubs = |which: &mut Vec<usize>| {
        for s in which.clone() {
            if plan.wants[s].stub {
                let sites = sites_of(&plan.sheets, s);
                which.extend(plan.sheets.iter().flat_map(|sh| sh.laid().map(move |k| &sh.strands[k])).filter(|st| [st.from, st.to].iter().any(|e| matches!(e, End::Site(x) if sites.contains(x)))).map(|st| st.connection));
            }
        }
    };
    let (old, new) = (spots.holes.len(), found.len());
    let mut with = (*spots).clone();
    with.holes.extend(found.iter().copied());
    with.wanted.extend(pads.iter().copied());
    let with = Arc::new(with);
    let number = |k: usize| Some(if k < old { k } else { k + new });
    let mut taken = None;
    for _ in 0..HOLE_LIFTS {
        from_stubs(&mut which);
        which.sort_unstable();
        which.dedup();
        let mut sheets = plan.sheets.clone();
        for &c in &which {
            let Some(lifted) = lift(board, &sheets, c) else { return false };
            sheets = lifted;
        }
        // The holes are the first of the vias: those there already, the new ones, then the rest.
        let Some(first) = sheets.first() else { return false };
        let mut vias = first.space.vias();
        vias.splice(old..old, found.iter().map(|h| (h.1, h.2)));
        let made = side_by_side(sheets.iter().collect(), |sheet| carry::sheet_or_full(board, sheet.space.layer, &vias, &with, &carry::traces(board, sheet, &number).ok_or(Vec::new())?));
        // A pad with a via wider than itself is wider on its own layer, and so is one whose
        // via is near its edge, for the hole's sake: the strands in the gaps that are now
        // too full are lifted too.
        let full: Vec<usize> = made.iter().filter_map(|m| m.as_ref().err()).flatten().copied().collect();
        if made.iter().all(|m| m.is_ok()) {
            taken = Some(made.into_iter().filter_map(Result::ok).collect::<Vec<Sheet>>());
            break;
        }
        if full.is_empty() || made.iter().any(|m| m.as_ref().is_err_and(|f| f.is_empty())) {
            return false;
        }
        which.extend(full);
    }
    let Some(taken) = taken else { return false };
    plan.sheets = taken;
    // Laid again: what was lifted, and of the connections with no way those with a hole now.
    // (Not every connection with no way: on a board with many, looking again for each costs
    // as much as the plan did.)
    let has_hole = |pad: ItemId| with.holes.iter().any(|h| h.0 == pad);
    which.extend(plan.left.iter().filter(|l| has_hole(l.from) || has_hole(l.to)).map(|l| l.connection));
    which.retain(|&c| !plan.wants[c].stub);
    which.sort_by(|&a, &b| plan.wants[a].apart.total_cmp(&plan.wants[b].apart).then(a.cmp(&b)));
    which.dedup();
    plan.left.retain(|l| !which.contains(&l.connection));
    place(board, plan, &which);
    true
}

/// How far off the line through a row's pads one of them may stand, mm.
const IN_LINE: f64 = 0.05;
/// How much nearer the part, or farther from it, a row of vias is tried where not all of it
/// fits at the distance its fan asks for, mm.
const OUT_STEPS: [f64; 11] = [-0.1, -0.2, -0.3, -0.45, -0.6, -0.8, 0.15, 0.3, 0.5, 0.8, 1.1];
/// How much farther than its pad's tip a tight fan's via is tried, mm, nearest first.
const TIGHT_STEPS: [f64; 6] = [0.0, 0.05, 0.1, 0.2, 0.3, 0.45];

/// Where a tight fan's via has no place at its pad's tip: how far apart the places tried are,
/// mm, how many of them out from the tip, and how many to each side of the pad's line (a
/// quarter of a millimetre: farther aside it stands in front of the next pad).
const NEAR_STEP: f64 = 0.05;
const NEAR_OUT: i64 = 24;
const NEAR_ASIDE: i64 = 5;

/// How a fine-pitch part's rows of vias stand ([`escape`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fan {
    /// A row of vias a via's pitch apart, a little way out from the pads, spread wider than
    /// the row of pads where the pads stand nearer than that.
    Spread,
    /// Each via straight out from its own pad at the pad's tip, every other one a little
    /// farther out; spread where that leaves fewer of a row's vias a place.
    Tight,
}

/// The places for the vias of one row of a part's pads: the pads that have a connection, in
/// their order along the row, each with where its via may stand (none where no via may).
#[allow(clippy::too_many_arguments)]
fn slots(board: &Board, apart: f64, radius: f64, hole: f64, row: &[(ItemId, space::P)], middle: space::P, scratch: &mut Board, fan: Fan) -> Vec<(ItemId, Option<space::P>)> {
    let (first, last) = (row[0].1, row[row.len() - 1].1);
    let len = space::dist(first, last).max(1e-9);
    let along = [(last[0] - first[0]) / len, (last[1] - first[1]) / len];
    let t: Vec<f64> = row.iter().map(|p| (p.1[0] - first[0]) * along[0] + (p.1[1] - first[1]) * along[1]).collect();
    // As near its own pad as each can be with all of them a via's pitch apart: with `k` pitches
    // taken off the `k`th, the places must not come down, and the nearest such to the pads'
    // own are their means over each run that would (pool adjacent violators).
    let pitch = apart + 8.0 * space::VIA_CORNER;
    let mut runs: Vec<(f64, usize)> = Vec::new();
    for (k, &at) in t.iter().enumerate() {
        runs.push((at - k as f64 * pitch, 1));
        while runs.len() >= 2 && runs[runs.len() - 2].0 > runs[runs.len() - 1].0 {
            let (b, a) = (runs.pop().unwrap_or((0.0, 0)), runs.pop().unwrap_or((0.0, 0)));
            runs.push(((a.0 * a.1 as f64 + b.0 * b.1 as f64) / (a.1 + b.1) as f64, a.1 + b.1));
        }
    }
    let places: Vec<f64> = runs.iter().flat_map(|&(y, n)| std::iter::repeat_n(y, n)).enumerate().map(|(k, y)| y + k as f64 * pitch).collect();
    let most = places.iter().zip(&t).map(|(s, t)| (s - t).abs()).fold(0.0, f64::max);
    // Out from the row far enough for the farthest stub to come across at 45 degrees and
    // straighten before its via.
    let clearance = (board.clearance(0, 0) as f64 / 1e6).max(hole);
    let out = most + radius + clearance + width(board, 0);
    let reach = |pad: ItemId, dir: space::P| {
        let Some(item) = board.item(pad) else { return 0.0 };
        let b = item.shape.bbox();
        let c = [(b.min.x + b.max.x) as f64 / 2e6, (b.min.y + b.max.y) as f64 / 2e6];
        let far = |p: board::Point| (p.x as f64 / 1e6 - c[0]) * dir[0] + (p.y as f64 / 1e6 - c[1]) * dir[1];
        match &item.shape {
            board::Shape::Polygon(points) => points.iter().map(|p| far(*p)).fold(0.0, f64::max),
            board::Shape::Capsule { a, b, r } => far(*a).max(far(*b)) + *r as f64 / 1e6,
        }
    };
    let bounds = space::bounds(board);
    let stand = |scratch: &mut Board, dir: space::P| -> Vec<Option<space::P>> {
        let tip = row.iter().map(|p| reach(p.0, dir)).fold(0.0, f64::max);
        let mut at = |out: f64| -> Vec<Option<space::P>> {
            places
                .iter()
                .map(|&s| {
                    let p = [first[0] + along[0] * s + dir[0] * (tip + out), first[1] + along[1] * s + dir[1] * (tip + out)];
                    let q = board::Point::new((p[0] * 1e6).round() as i64, (p[1] * 1e6).round() as i64);
                    let id = scratch.insert(via_item(board, p, 0));
                    let fits = bounds.first().is_none_or(|o| board::inside(q, o)) && board::check_item(scratch, id).is_empty();
                    scratch.remove(id);
                    fits.then_some(p)
                })
                .collect()
        };
        // Where something stands in the row's way: a little nearer the part, or farther, as
        // most of its vias fit. Never so near that a via could not clear the pads.
        let least = radius + clearance;
        let mut best = at(out);
        for step in OUT_STEPS {
            if best.iter().all(|p| p.is_some()) {
                break;
            }
            let tried = at((out + step).max(least));
            if tried.iter().flatten().count() > best.iter().flatten().count() {
                best = tried;
            }
        }
        best
    };
    // Away from the part's middle; for a part that is one row, the side with more room.
    let normal = [-along[1], along[0]];
    let side = (first[0] - middle[0]) * normal[0] + (first[1] - middle[1]) * normal[1];
    // Or tight, as the designers fan a fine-pitch chip out: each via straight out from its own
    // pad, its copper at the pad's tip (a via may touch its own pad), every other one a little
    // farther out so that neighbours keep a via's pitch apart. The ground beyond the row is
    // then left to the wires.
    let tight = |scratch: &mut Board, dir: space::P| -> Vec<Option<space::P>> {
        let stagger = |k: usize| -> f64 {
            let gap = |i: usize, j: usize| (t[i] - t[j]).abs();
            let near = [k.checked_sub(1).map(|j| gap(k, j)), (k + 1 < t.len()).then(|| gap(k, k + 1))].into_iter().flatten().fold(f64::MAX, f64::min);
            if near >= pitch { 0.0 } else { (pitch * pitch - near * near).sqrt() }
        };
        let mut best: Vec<Option<space::P>> = vec![None; row.len()];
        // Every other via stands in one line: they must be a via's pitch apart in it.
        if t.windows(3).any(|w| w[2] - w[0] < pitch) {
            return best;
        }
        let far: Vec<f64> = (0..row.len()).map(|k| if k % 2 == 1 { stagger(k) } else { 0.0 }).collect();
        for extra in TIGHT_STEPS {
            let tried: Vec<Option<space::P>> = row
                .iter()
                .enumerate()
                .map(|(k, &(pad, centre))| {
                    let out = reach(pad, dir) + radius + extra + far[k];
                    let p = [centre[0] + dir[0] * out, centre[1] + dir[1] * out];
                    let q = board::Point::new((p[0] * 1e6).round() as i64, (p[1] * 1e6).round() as i64);
                    let net = board.item(pad).map_or(0, |i| i.net);
                    let id = scratch.insert(via_item(board, p, net));
                    let fits = bounds.first().is_none_or(|o| board::inside(q, o)) && board::check_item(scratch, id).is_empty();
                    scratch.remove(id);
                    fits.then_some(p)
                })
                .collect();
            // Each pad keeps the nearest place that fits.
            for (b, t) in best.iter_mut().zip(tried) {
                if b.is_none() {
                    *b = t;
                }
            }
            if best.iter().all(|p| p.is_some()) {
                break;
            }
        }
        // A pad with no place at its tip (a pad on another layer stands opposite it): the
        // place nearest the tip that the checker passes beside the row's other vias, a
        // little to one side of the pad's line or farther out. Without one its stub went to
        // any spot within reach, across the front of its neighbours.
        let stood: Vec<ItemId> = row.iter().zip(&best).filter_map(|(&(pad, _), at)| Some(scratch.insert(via_item(board, (*at)?, board.item(pad).map_or(0, |i| i.net))))).collect();
        let mut more: Vec<ItemId> = Vec::new();
        let along = [-dir[1], dir[0]];
        let mut tries: Vec<(f64, f64)> = (0..=NEAR_OUT).flat_map(|i| (-NEAR_ASIDE..=NEAR_ASIDE).map(move |j| (i as f64 * NEAR_STEP, j as f64 * NEAR_STEP))).collect();
        tries.sort_by(|a, b| (a.0 + 2.0 * a.1.abs()).total_cmp(&(b.0 + 2.0 * b.1.abs())).then(a.1.total_cmp(&b.1)));
        for (k, &(pad, centre)) in row.iter().enumerate() {
            if best[k].is_some() {
                continue;
            }
            let net = board.item(pad).map_or(0, |i| i.net);
            let tip = reach(pad, dir) + radius;
            for &(out, aside) in &tries {
                let p = [centre[0] + dir[0] * (tip + out) + along[0] * aside, centre[1] + dir[1] * (tip + out) + along[1] * aside];
                let q = board::Point::new((p[0] * 1e6).round() as i64, (p[1] * 1e6).round() as i64);
                let id = scratch.insert(via_item(board, p, net));
                if bounds.first().is_none_or(|o| board::inside(q, o)) && board::check_item(scratch, id).is_empty() {
                    best[k] = Some(p);
                    more.push(id);
                    break;
                }
                scratch.remove(id);
            }
        }
        for id in stood.into_iter().chain(more) {
            scratch.remove(id);
        }
        best
    };
    let out_from = |scratch: &mut Board, dir: space::P| {
        let spread = stand(scratch, dir);
        if fan == Fan::Spread {
            return spread;
        }
        let close = tight(scratch, dir);
        if close.iter().flatten().count() >= spread.iter().flatten().count() { close } else { spread }
    };
    let stood = if side.abs() > IN_LINE {
        out_from(scratch, if side > 0.0 { normal } else { [-normal[0], -normal[1]] })
    } else {
        let (a, b) = (out_from(scratch, normal), out_from(scratch, [-normal[0], -normal[1]]));
        if a.iter().flatten().count() >= b.iter().flatten().count() { a } else { b }
    };
    row.iter().map(|p| p.0).zip(stood).collect()
}

/// The way out of each part whose pads stand nearer each other than two vias may, settled for
/// the whole part: the spots a plan's vias may stand at, and a stub for each such pad that
/// `wants` start or end at.
///
/// The pads of such a part that stand in a row have their vias in a row too, a little way
/// out from the part: one place for each pad that has a connection, in the pads' order, a
/// via's pitch apart and each as near straight out from its own pad as that lets it be, so
/// that the stubs fan out and none crosses another. No other via may stand between the two
/// rows. A stub that is not used is taken out again ([`tidy`]), and its pad's wire then has
/// the place its via would have had to pass through. A pad that stands in no row, or whose
/// own place cannot be reached, has its via wherever one fits within [`FAN_REACH`].
///
/// With [`Fan::Tight`] a row's vias stand at its pads' tips instead, as designers fan out a
/// QFN of 0.4 mm pitch under 0.3 mm vias, leaving the ground beyond the row to the wires
/// (parallelogram p2-132-usb6: 34 connections open with the spread rows, 21 with these).
pub fn escape(board: &Board, wants: &[Want], fan: Fan) -> (Spots, Vec<Want>) {
    let mut spots = via_spots(board, true);
    let mid = |id: ItemId| board.bbox(id).map(|b| [(b.min.x + b.max.x) as f64 / 2e6, (b.min.y + b.max.y) as f64 / 2e6]);
    let part = |id: ItemId| match board.item(id) {
        Some(board::Item { kind: Kind::Pad { footprint, .. }, layers, .. }) if layers.count_ones() == 1 => Some((*footprint, *layers)),
        _ => None,
    };
    let pads: Vec<(ItemId, (usize, u64), space::P)> = board.items().filter_map(|(id, _)| Some((id, part(id)?, mid(id)?))).collect();
    let mut ends: Vec<(ItemId, u32)> = wants.iter().filter(|w| !w.stub).flat_map(|w| [(w.from, w.net), (w.to, w.net)]).collect();
    ends.sort_unstable();
    ends.dedup();
    let mut parts: Vec<(usize, u64)> = pads.iter().map(|p| p.1).collect();
    parts.sort_unstable();
    parts.dedup();
    let mut scratch = board.clone();
    // Each pad's own place, and the ground between each row and its places.
    let mut own: std::collections::HashMap<ItemId, space::P> = std::collections::HashMap::new();
    let mut fans: Vec<Vec<space::P>> = Vec::new();
    for of in parts {
        let mine: Vec<(ItemId, space::P)> = pads.iter().filter(|p| p.1 == of).map(|p| (p.0, p.2)).collect();
        let n = mine.len() as f64;
        let middle = mine.iter().fold([0.0, 0.0], |c, p| [c[0] + p.1[0] / n, c[1] + p.1[1] / n]);
        // The pads that stand near each other, in the groups they make.
        let mut group: Vec<usize> = (0..mine.len()).collect();
        for i in 0..mine.len() {
            for j in 0..i {
                if space::dist(mine[i].1, mine[j].1) < spots.apart {
                    let (a, b) = (group[i], group[j]);
                    group.iter_mut().filter(|g| **g == a).for_each(|g| *g = b);
                }
            }
        }
        let mut groups: Vec<usize> = group.clone();
        groups.sort_unstable();
        groups.dedup();
        for g in groups {
            let mut row: Vec<(ItemId, space::P)> = (0..mine.len()).filter(|&i| group[i] == g).map(|i| mine[i]).collect();
            if row.len() < 2 {
                continue;
            }
            // In a row: all on the line through the two farthest apart.
            row.sort_by(|a, b| a.1[0].total_cmp(&b.1[0]).then(a.1[1].total_cmp(&b.1[1])));
            let (a, b) = (row[0].1, row[row.len() - 1].1);
            let len = space::dist(a, b).max(1e-9);
            let off = |p: space::P| ((p[0] - a[0]) * (b[1] - a[1]) - (p[1] - a[1]) * (b[0] - a[0])).abs() / len;
            if row.iter().any(|p| off(p.1) > IN_LINE) {
                continue;
            }
            let along = |p: space::P| (p[0] - a[0]) * (b[0] - a[0]) + (p[1] - a[1]) * (b[1] - a[1]);
            row.sort_by(|x, y| along(x.1).total_cmp(&along(y.1)));
            let wanted: Vec<(ItemId, space::P)> = row.iter().copied().filter(|p| ends.iter().any(|e| e.0 == p.0)).collect();
            if wanted.is_empty() {
                continue;
            }
            let stood = slots(board, spots.apart, spots.radius, spots.hole, &wanted, middle, &mut scratch, fan);
            let places: Vec<space::P> = stood.iter().filter_map(|s| s.1).collect();
            if let (Some(&first), Some(&last)) = (places.first(), places.last()) {
                fans.push(vec![a, b, last, first]);
            }
            own.extend(stood.into_iter().filter_map(|(pad, at)| Some((pad, at?))));
        }
    }
    // No other via among a row's stubs, nor nearer one of its places than two vias may be.
    let apart = spots.apart;
    let near_fan = |p: space::P| {
        fans.iter().any(|f| space::inside(p, f) || (0..f.len()).any(|i| space::point_segment(p, f[i], f[(i + 1) % f.len()]) < apart))
    };
    spots.at.retain(|&p| !near_fan(p));
    let others = spots.at.len();
    let mut stubs: Vec<((usize, u64), f64, Want)> = Vec::new();
    for (pad, net) in ends {
        let (Some(of), Some(at)) = (part(pad), mid(pad)) else { continue };
        let beside = pads.iter().filter(|p| p.1 == of && p.0 != pad).map(|p| space::dist(p.2, at)).fold(f64::MAX, f64::min);
        if beside >= spots.apart {
            continue;
        }
        let n = pads.iter().filter(|p| p.1 == of).count() as f64;
        let centre = pads.iter().filter(|p| p.1 == of).fold([0.0, 0.0], |c, p| [c[0] + p.2[0] / n, c[1] + p.2[1] / n]);
        let turn = (at[1] - centre[1]).atan2(at[0] - centre[0]);
        // Its own place first; the others within reach are for a pad that has none, or whose
        // own turns out not to be reached.
        let mut into: Vec<usize> = (0..others).filter(|&c| space::dist(spots.at[c], at) <= FAN_REACH).collect();
        if let Some(&place) = own.get(&pad) {
            spots.at.push(place);
            into.insert(0, spots.at.len() - 1);
        }
        stubs.push((of, turn, Want { net, from: pad, to: pad, apart: 0.0, into, avoid: Vec::new(), stub: true, narrow: None }));
    }
    stubs.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
    (spots, stubs.into_iter().map(|s| s.2).collect())
}

/// Takes out every stub that no connection starts from: a via with a track to it and nothing
/// beyond.
pub fn tidy(board: &Board, plan: &mut Plan) {
    for s in (0..plan.wants.len()).filter(|&s| plan.wants[s].stub) {
        if !used(&plan.sheets, s)
            && let Some(lifted) = lift(board, &plan.sheets, s)
        {
            plan.sheets = lifted;
        }
    }
    plan.vias = vias_of(&plan.sheets);
}

/// Plans more connections into a plan, in the order given. Those already in it may be lifted
/// and laid again to let one through.
pub fn add(board: &Board, plan: &mut Plan, more: Vec<Want>) {
    let first = plan.wants.len();
    plan.wants.extend(more);
    // A net a custom rule holds to a length (a crystal's, a flash clock's) is laid first, while
    // its straight way is open; the rest shortest first, as they come.
    let mut bounded: std::collections::HashMap<u32, bool> = std::collections::HashMap::new();
    let mut order: Vec<usize> = (first..plan.wants.len()).collect();
    order.sort_by_key(|&c| !*bounded.entry(plan.wants[c].net).or_insert_with(|| board.net_maximum("length", plan.wants[c].net).is_some()));
    place(board, plan, &order);
}

/// Lays again the connections a wire could not be drawn for (each the connection's number and
/// the gap its wire stuck in), and with them every connection the plan has no way for: the
/// rest of the plan stays as it is, but for what is lifted to let these through. A connection
/// that may be narrower is made so and may go the same way; one that is at its narrowest is
/// barred from the gap. Says whether any was made narrower.
pub fn again(board: &Board, plan: &mut Plan, stuck: &[(usize, space::GapKey)]) -> bool {
    let mut narrowed = false;
    for &(c, gap) in stuck {
        // Narrower first, the same way; at its narrowest, another way.
        if narrower(board, &mut plan.wants[c]) {
            narrowed = true;
        } else if !plan.wants[c].avoid.contains(&gap) {
            plan.wants[c].avoid.push(gap);
        }
    }
    relay(board, plan, stuck.iter().map(|s| s.0).collect());
    narrowed
}

/// Lays again connections whose wires are where they should not be, each barred from a gap
/// it went through, as wide as it was; and with them every connection the plan has no way
/// for.
pub fn bar(board: &Board, plan: &mut Plan, out_of: &[(usize, space::GapKey)]) {
    for &(c, gap) in out_of {
        if !plan.wants[c].avoid.contains(&gap) {
            plan.wants[c].avoid.push(gap);
        }
    }
    relay(board, plan, out_of.iter().map(|s| s.0).collect());
}

/// Takes connections out of the plan, with the vias they used: left unlaid, for [`renew`].
pub fn lift_out(board: &Board, plan: &mut Plan, which: &[usize]) {
    for &c in which {
        if let Some(lifted) = lift(board, &plan.sheets, c) {
            plan.sheets = lifted;
        }
    }
    plan.vias = vias_of(&plan.sheets);
}

/// Lays connections again with new ends: each the connection's number and what it is to be
/// now, its ends and the spots it may end at. What it was barred from and how narrow it had
/// been made stay. For a connection that ended at a via in its net's pour, where the pour
/// there no longer reaches what it was to join.
pub fn renew(board: &Board, plan: &mut Plan, renewed: Vec<(usize, Want)>) {
    let which = renewed.iter().map(|r| r.0).collect();
    for (c, want) in renewed {
        let old = &mut plan.wants[c];
        (old.from, old.to, old.apart, old.into) = (want.from, want.to, want.apart, want.into);
    }
    relay(board, plan, which);
}

/// Lifts the connections numbered `which` and lays them again, and every connection the
/// plan has no way for with them.
fn relay(board: &Board, plan: &mut Plan, mut which: Vec<usize>) {
    which.extend(plan.left.iter().map(|l| l.connection));
    // A stub that could not be drawn goes for good, and what started from its via is laid
    // again from the pad.
    for s in which.clone() {
        if plan.wants[s].stub {
            let sites = sites_of(&plan.sheets, s);
            which.extend(plan.sheets.iter().flat_map(|sh| sh.laid().map(move |k| &sh.strands[k])).filter(|st| [st.from, st.to].iter().any(|e| matches!(e, End::Site(x) if sites.contains(x)))).map(|st| st.connection));
        }
    }
    which.sort_by(|&a, &b| plan.wants[a].apart.total_cmp(&plan.wants[b].apart).then(a.cmp(&b)));
    which.dedup();
    for &c in &which {
        if let Some(lifted) = lift(board, &plan.sheets, c) {
            plan.sheets = lifted;
        }
    }
    which.retain(|&c| !plan.wants[c].stub);
    plan.left.retain(|l| !which.contains(&l.connection));
    place(board, plan, &which);
}

/// Lays the connections numbered `which`, none of which is in the sheets, in that order.
fn place(board: &Board, plan: &mut Plan, which: &[usize]) {
    let mut sheets = std::mem::take(&mut plan.sheets);
    let dear: Vec<f64> = (0..sheets.len())
        .map(|l| match (plan.planes.contains(&l), plan.closed.contains(&l)) {
            (false, _) => 1.0,
            (true, false) => ON_A_POURED_SIDE,
            (true, true) => ON_A_PLANE,
        })
        .collect();
    // (Tried 2026-11-12: one search for every connection that weighs a via against the detour
    // it saves put 19 vias on Shift-in 32, where the designer has none. And vias only as the
    // last thing tried made a connection take any way round, however long.)
    let joined = plan.joined.clone();
    for &c in which {
        // At its class's width; with no way at that, narrower, for as long as it may be.
        loop {
            let job = Job { board, dear: &dear, wants: &plan.wants, joined: &joined };
            let want = &plan.wants[c];
            let best = job.best(&sheets, c).map(|b| b.1);
            // A stub with no way is done without: its pad's connections start from the pad.
            if want.stub {
                if let Some(laid) = best {
                    sheets = laid;
                }
                break;
            }
            // With no way: the connections in its way are lifted for it. With a way that is
            // longer by [`WORTH`] than one across other connections would be: those are lifted,
            // all are laid again, and the plan that costs less of the two is kept.
            let across = job.across(&sheets, c, &[], best.is_none()).filter(|(_, over)| !over.is_empty());
            let lifted = match (&best, across) {
                (None, Some(_)) => job.through(&sheets, c, DEEP),
                (Some(laid), Some((cost, over))) => {
                    let now = job.cost(laid) - job.cost(&sheets);
                    (cost - CROSS * over.len() as f64 + WORTH < now).then(|| job.through(&sheets, c, DEEP)).flatten().filter(|lifted| job.cost(lifted) < job.cost(laid))
                }
                _ => None,
            };
            if let Some(laid) = lifted.or(best) {
                sheets = laid;
                break;
            }
            if narrower(board, &mut plan.wants[c]) {
                continue;
            }
            let want = &plan.wants[c];
            let pads = board.item(want.from).zip(board.item(want.to)).is_some_and(|(f, t)| matches!(f.kind, Kind::Pad { .. }) && matches!(t.kind, Kind::Pad { .. }));
            let why = if pads { "no way through the room left, with vias or without" } else { "an end is not a pad" };
            plan.left.push(Left { connection: c, net: want.net, from: want.from, to: want.to, why: why.to_string() });
            break;
        }
    }
    plan.vias = vias_of(&sheets);
    plan.sheets = sheets;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Of(usize, usize);

    impl Layer for Of {
        fn edges(&self) -> usize {
            self.1
        }
    }

    /// Layers worked on side by side come back in their order, small ones (one after
    /// another) and big ones (a thread each) alike.
    #[test]
    fn layers_side_by_side_come_back_in_their_order() {
        for edges in [10, WORTH_A_THREAD] {
            let layers: Vec<Of> = (0..8).map(|l| Of(l, edges)).collect();
            let out = side_by_side(layers, |layer| {
                // The later layers are done first.
                std::thread::sleep(std::time::Duration::from_millis(8 - layer.0 as u64));
                layer.0 * 10
            });
            assert_eq!(out, (0..8).map(|l| l * 10).collect::<Vec<_>>());
        }
    }
}
