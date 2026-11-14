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
//! the plan left as it is. A connection ends on its pad or on any pad already joined to it
//! ([`Plan::joined`]), and is as wide as its net's class asks unless it has no way at that
//! width ([`wide`]).

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
    /// Layers a pour covers at least half of, unless every layer is: strands there cost more.
    pub planes: Vec<usize>,
    pub vias: Vec<Via>,
    pub left: Vec<Left>,
    /// Every connection asked for, in the order asked: a strand's `connection` is its number
    /// here.
    pub wants: Vec<Want>,
    /// For each pad that other pads are joined to on the bare board, by overlapping it or by
    /// copper that stays: all of them, itself among them. A connection to one may end on any.
    #[serde(skip)]
    pub joined: Arc<Joined>,
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

/// A net's track width, mm: its class's.
pub fn width(board: &Board, net: u32) -> f64 {
    let class = board.rules.class_of(board.net_name(net));
    class.map_or(board.rules.min_track_width, |c| c.track_width.max(board.rules.min_track_width)) * 1e3
}

/// The track width a connection is laid at, mm: its net's class's, unless it has been made
/// narrower.
pub fn wide(board: &Board, want: &Want) -> f64 {
    want.narrow.unwrap_or_else(|| width(board, want.net))
}

/// A track is never laid narrower than this share of what its net's class asks for.
const NARROWEST: f64 = 0.25;

/// Makes a connection narrower, if it may be: half as wide, and no less than the least the
/// board's rules let a track be, nor than [`NARROWEST`] of its class's width (a board may give
/// no least at all). A class's width is what its tracks are drawn at by default, and no rule:
/// the designers lay narrower where that is what fits (KiwiSDR's antenna board asks for 1 mm,
/// and 261 of its 420 pieces of track are 0.25 mm).
fn narrower(board: &Board, want: &mut Want) -> bool {
    let now = wide(board, want);
    let least = (board.rules.min_track_width * 1e3).max(NARROWEST * width(board, want.net));
    if want.stub || now <= least + 1e-9 {
        return false;
    }
    want.narrow = Some((now / 2.0).max(least));
    true
}

/// What a strand on a plane layer costs, as a multiple of its length.
const ON_A_PLANE: f64 = 4.0;
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

/// The default via's radius and drill radius, nm.
pub fn via_size(board: &Board) -> (i64, i64) {
    let class = board.rules.class_of("");
    let d = class.as_ref().map_or(0.0, |c| c.via_diameter).max(board.rules.min_via_diameter);
    let drill = class.as_ref().map_or(0.0, |c| c.via_drill).max(board.rules.min_via_drill);
    (board::nm(d) / 2, board::nm(drill) / 2)
}

/// A via of `net` at `at`, through every layer.
pub(crate) fn via_item(board: &Board, at: space::P, net: u32) -> board::Item {
    let (r, drill) = via_size(board);
    let at = board::Point::new((at[0] * 1e6).round() as i64, (at[1] * 1e6).round() as i64);
    board::Item { kind: Kind::Via, net, layers: (1u64 << board.layers.len()) - 1, shape: board::Shape::Capsule { a: at, b: at, r }, hole: Some((at, drill)), locked: false, span: None }
}

/// Places a via could stand: a grid over the board, at a pitch that lets a track pass between
/// two neighbours, and the point farthest from the corners of each free triangle of the bare
/// layers; each kept only where the exact checker lets a via of no net stand on the bare board.
pub fn via_spots(board: &Board, tight: bool) -> Spots {
    let (r, _) = via_size(board);
    // (A board whose outline does not close has the box round it for one.)
    let bounds = space::bounds(board);
    let outer = bounds.first();
    let box_ = outer.map_or(board::BBox::of([board::Point::new(0, 0)]), |o| board::BBox::of(o.iter().copied()));
    // A little over what a track between two vias needs: a via stands in the triangulation as
    // a triangle about its middle, and the gap between two is that much short of their pitch.
    let pitch = 2 * r + 2 * board.clearance(0, 0) + board::nm(board.rules.min_track_width.max(width(board, 0) / 1e3)) + (8.0 * space::VIA_CORNER * 1e6) as i64;
    // For fan-outs: as near each other as two vias may stand.
    let pitch = if tight { 2 * r + board.clearance(0, 0) + (8.0 * space::VIA_CORNER * 1e6) as i64 } else { pitch };
    let mut scratch = board.clone();
    let mut at = Vec::new();
    let mut y = box_.min.y + pitch / 2;
    while y < box_.max.y {
        let mut x = box_.min.x + pitch / 2;
        while x < box_.max.x {
            let p = board::Point::new(x, y);
            let id = scratch.insert(via_item(board, space::mm(p), 0));
            if outer.is_none_or(|o| board::inside(p, o)) && board::check_item(&scratch, id).is_empty() {
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
            if outer.is_none_or(|o| board::inside(q, o)) && board::check_item(&scratch, id).is_empty() {
                at.push(space::mm(q));
            }
            scratch.remove(id);
        }
    }
    Spots { at, radius: r as f64 / 1e6, apart: (2 * r + board.clearance(0, 0)) as f64 / 1e6 }
}

/// A spot in a triangle is left out when another spot is this near, mm.
const NEAR: f64 = 0.3;

/// The vias in the sheets.
fn vias_of(sheets: &[Sheet]) -> Vec<Via> {
    sheets.first().map_or(Vec::new(), |s| s.space.vias().into_iter().enumerate().map(|(site, (at, net))| Via { net, site, at }).collect())
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
    let out: Vec<Sheet> = sheets
        .iter()
        .enumerate()
        .zip(fresh)
        .map(|((l, sheet), fresh)| {
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
/// anything is laid, and the via the stub of any of them ends in. A stub starts at its pad and
/// has no end but the via it puts in.
fn ends(sheets: &[Sheet], wants: &[Want], joined: &Joined, connection: usize) -> (Vec<End>, Vec<End>) {
    let want = &wants[connection];
    if want.stub {
        return (vec![End::Item(want.from)], Vec::new());
    }
    let cluster = |pad: ItemId| {
        let others = joined.get(&pad).map_or(&[][..], |pads| pads.as_slice());
        let pads: Vec<ItemId> = std::iter::once(pad).chain(others.iter().copied().filter(|&p| p != pad)).collect();
        let stubs = wants.iter().enumerate().filter(|(_, w)| w.stub && pads.contains(&w.from)).flat_map(|(s, _)| sites_of(sheets, s)).map(End::Site).collect::<Vec<End>>();
        pads.into_iter().map(End::Item).chain(stubs).collect::<Vec<End>>()
    };
    (cluster(want.from), cluster(want.to))
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
    // A via another connection's strand still ends in stays: a stub's, while it is used.
    gone.retain(|&site| !out.iter().any(|s| s.laid().any(|k| [s.strands[k].from, s.strands[k].to].contains(&End::Site(site)))));
    if gone.is_empty() {
        return Some(out);
    }
    let first = out.first()?;
    let number = |k: usize| (!gone.contains(&k)).then(|| k - gone.iter().filter(|&&g| g < k).count());
    let vias: Vec<(space::P, u32)> = first.space.vias().into_iter().enumerate().filter(|(k, _)| !gone.contains(k)).map(|(_, v)| v).collect();
    let spots = first.space.spots.clone();
    out.iter().map(|sheet| carry::sheet(board, sheet.space.layer, &vias, &spots, &carry::traces(board, sheet, &number)?)).collect()
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
/// How deep lifting goes: a connection lifted to let one through may itself have others lifted
/// to be laid again, this many times over.
const DEEP: usize = 2;

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
        match lay(self.board, sheets, &self.terms(false, false), self.wants, self.joined, c) {
            // With a pour to end in, a via into it is always weighed against the way without.
            Some(on_one) if want.into.is_empty() && on_one.0 <= DIRECT * want.apart + SLACK => Some(on_one),
            on_one => [on_one, lay(self.board, sheets, &with_vias, self.wants, self.joined, c)].into_iter().flatten().min_by(|a, b| a.0.total_cmp(&b.0)),
        }
    }

    /// What a plan costs: its strands' lengths, each on a layer as dear as that layer is, and
    /// its vias.
    fn cost(&self, sheets: &[Sheet]) -> f64 {
        sheets.iter().zip(self.dear).map(|(s, dear)| dear * s.length(self.board)).sum::<f64>() + VIA * sheets.first().map_or(0, |s| s.space.sites.len()) as f64
    }

    /// What the cheapest way for `c` would cost if it might go across other connections'
    /// strands, and which it would go across: none of those in `keep`.
    fn across(&self, sheets: &[Sheet], c: usize, keep: &[usize]) -> Option<(f64, Vec<usize>)> {
        let want = &self.wants[c];
        // A stub that another connection starts from stays where it is.
        let keep: Vec<usize> = keep.iter().copied().chain((0..self.wants.len()).filter(|&s| self.wants[s].stub && used(sheets, s))).collect();
        let (from, to) = ends(sheets, self.wants, self.joined, c);
        let way = ways::find(self.board, sheets, &ways::Terms { keep: &keep, into: &want.into, avoid: &want.avoid, ..self.terms(true, true) }, want.net, wide(self.board, want), &from, &to)?;
        Some((way.cost, way.over))
    }

    /// Lays `c` by lifting the connections in its way and laying each of those again after
    /// it, shortest first: the sheets with all of them, if all then have a way. One of those
    /// that then has none may have others lifted for it in turn, `deep` times over.
    fn through(&self, sheets: &[Sheet], c: usize, deep: usize) -> Option<Vec<Sheet>> {
        // The connections that were lifted and could not all be laid again are left where
        // they are the next time, and another way across is looked for: a few times.
        let mut keep: Vec<usize> = Vec::new();
        for _ in 0..OTHERS {
            let (_, mut over) = self.across(sheets, c, &keep)?;
            if over.is_empty() {
                return Some(self.best(sheets, c)?.1);
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
    wants.sort_by(|a, b| a.apart.total_cmp(&b.apart));
    wants
}

/// A plan with nothing in it yet: the board's layers cut into triangles.
pub fn start(board: &Board) -> Plan {
    start_with(board, false)
}

/// [`start`], with the spots for vias as near each other as two vias may stand where `tight`:
/// what a fan-out needs, at the price of more places to try.
pub fn start_with(board: &Board, tight: bool) -> Plan {
    let spots = Arc::new(via_spots(board, tight));
    let sheets: Vec<Sheet> = (0..board.layers.len()).map(|l| Sheet::new(Space::new(board, l, &[], &spots))).collect();
    let poured = poured(board);
    // On two layers a poured side is still routed on: the designers do (159 of the gate
    // tier's 253 two-layer boards have a side at least half poured).
    let planes: Vec<usize> = if poured.len() <= 2 || poured.iter().all(|s| *s >= 0.5) { Vec::new() } else { (0..poured.len()).filter(|&l| poured[l] >= 0.5).collect() };
    Plan { sheets, planes, vias: Vec::new(), left: Vec::new(), wants: Vec::new(), joined: Arc::new(joined(board)) }
}

/// How far from its pad a stub's via may stand, mm.
const FAN_REACH: f64 = 4.0;

/// A stub for each pad that `wants` start or end at and that stands nearer a pad beside it,
/// on its own part, than two vias may stand: such a part's pins cannot each have a via
/// straight in front of them, so where each one's via goes is settled for all of them before
/// anything else is laid, part by part and round each part in order. The designers do the
/// same: a via beside every pin of a fine-pitch chip, and the rest of the way on another
/// layer.
pub fn fan(board: &Board, plan: &Plan, wants: &[Want]) -> Vec<Want> {
    let Some(spots) = plan.sheets.first().map(|s| s.space.spots.clone()) else { return Vec::new() };
    let mid = |id: ItemId| board.bbox(id).map(|b| [(b.min.x + b.max.x) as f64 / 2e6, (b.min.y + b.max.y) as f64 / 2e6]);
    // Pads on one layer only, by part.
    let part = |id: ItemId| match board.item(id) {
        Some(board::Item { kind: Kind::Pad { footprint, .. }, layers, .. }) if layers.count_ones() == 1 => Some((*footprint, *layers)),
        _ => None,
    };
    let pads: Vec<(ItemId, (usize, u64), space::P)> = board.items().filter_map(|(id, _)| Some((id, part(id)?, mid(id)?))).collect();
    let mut ends: Vec<(ItemId, u32)> = wants.iter().filter(|w| !w.stub).flat_map(|w| [(w.from, w.net), (w.to, w.net)]).collect();
    ends.sort_unstable();
    ends.dedup();
    let mut stubs: Vec<((usize, u64), f64, Want)> = Vec::new();
    for (pad, net) in ends {
        let (Some(of), Some(at)) = (part(pad), mid(pad)) else { continue };
        let beside = pads.iter().filter(|p| p.1 == of && p.0 != pad).map(|p| space::dist(p.2, at)).fold(f64::MAX, f64::min);
        if beside >= spots.apart {
            continue;
        }
        // Round the part from its middle: pads beside each other come one after the other.
        let n = pads.iter().filter(|p| p.1 == of).count() as f64;
        let centre = pads.iter().filter(|p| p.1 == of).fold([0.0, 0.0], |c, p| [c[0] + p.2[0] / n, c[1] + p.2[1] / n]);
        let turn = (at[1] - centre[1]).atan2(at[0] - centre[0]);
        let into: Vec<usize> = (0..spots.at.len()).filter(|&c| space::dist(spots.at[c], at) <= FAN_REACH).collect();
        stubs.push((of, turn, Want { net, from: pad, to: pad, apart: 0.0, into, avoid: Vec::new(), stub: true, narrow: None }));
    }
    stubs.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
    stubs.into_iter().map(|s| s.2).collect()
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
    place(board, plan, &(first..plan.wants.len()).collect::<Vec<_>>());
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
    let dear: Vec<f64> = (0..sheets.len()).map(|l| if plan.planes.contains(&l) { ON_A_PLANE } else { 1.0 }).collect();
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
            let across = job.across(&sheets, c, &[]).filter(|(_, over)| !over.is_empty());
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
