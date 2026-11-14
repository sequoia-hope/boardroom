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
//! again barred from the gap it stuck in ([`again`]), the rest of the plan left as it is.

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
}

impl Want {
    /// A connection for an airline.
    pub fn of(line: &board::Airline) -> Want {
        Want { net: line.net, from: line.from_item, to: line.to_item, apart: line.from.distance(line.to) / 1e6, into: Vec::new(), avoid: Vec::new() }
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
/// two neighbours, each kept only where the exact checker lets a via of no net stand on the
/// bare board.
pub fn via_spots(board: &Board) -> Spots {
    let (r, _) = via_size(board);
    // A board whose outline does not close: the box round everything on it.
    let outer = board.outline.first();
    let box_ = match outer {
        Some(o) => board::BBox::of(o.iter().copied()),
        None => board.items().filter_map(|(id, _)| board.bbox(id)).reduce(|a, b| a.union(b)).unwrap_or(board::BBox::of([board::Point::new(0, 0)])),
    };
    // A little over what a track between two vias needs: a via stands in the triangulation as
    // a triangle about its middle, and the gap between two is that much short of their pitch.
    let pitch = 2 * r + 2 * board.clearance(0, 0) + board::nm(board.rules.min_track_width.max(width(board, 0) / 1e3)) + (8.0 * space::VIA_CORNER * 1e6) as i64;
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
    Spots { at, radius: r as f64 / 1e6, apart: (2 * r + board.clearance(0, 0)) as f64 / 1e6 }
}

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
    let w = width(board, want.net);
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
    let out: Vec<Sheet> = sheets
        .iter()
        .zip(fresh)
        .map(|(sheet, fresh)| {
            let mut traces = carry::traces(board, sheet, &Some)?;
            traces.extend(fresh);
            carry::sheet(board, sheet.space.layer, &vias, &known, &traces)
        })
        .collect::<Option<_>>()?;
    stand(board, &out).then_some(out)
}

/// How many times a way whose vias the sheets do not hold is looked for again without them.
const AGAIN: usize = 6;

/// Lays a connection along the cheapest way there is for it: none with no way, or with none
/// the sheets hold. A way with vias that they do not hold is looked for again with those
/// spots left out: the search tries a via against the corners of the triangle it would stand
/// in, and what it does to the gaps round it is only known once it is in.
fn lay(board: &Board, sheets: &[Sheet], terms: &ways::Terms, connection: usize, want: &Want) -> Option<(f64, Vec<Sheet>)> {
    let mut ban: Vec<usize> = terms.ban.to_vec();
    for _ in 0..AGAIN {
        let way = ways::find(board, sheets, &ways::Terms { ban: &ban, into: &want.into, avoid: &want.avoid, ..*terms }, want.net, width(board, want.net), End::Item(want.from), End::Item(want.to))?;
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
    if gone.is_empty() {
        return Some(out);
    }
    let first = out.first()?;
    let number = |k: usize| (!gone.contains(&k)).then(|| k - gone.iter().filter(|&&g| g < k).count());
    let vias: Vec<(space::P, u32)> = first.space.vias().into_iter().enumerate().filter(|(k, _)| !gone.contains(k)).map(|(_, v)| v).collect();
    let spots = first.space.spots.clone();
    out.iter().map(|sheet| carry::sheet(board, sheet.space.layer, &vias, &spots, &carry::traces(board, sheet, &number)?)).collect()
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
        match lay(self.board, sheets, &self.terms(false, false), c, want) {
            // With a pour to end in, a via into it is always weighed against the way without.
            Some(on_one) if want.into.is_empty() && on_one.0 <= DIRECT * want.apart + SLACK => Some(on_one),
            on_one => [on_one, lay(self.board, sheets, &self.terms(true, false), c, want)].into_iter().flatten().min_by(|a, b| a.0.total_cmp(&b.0)),
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
        let way = ways::find(self.board, sheets, &ways::Terms { keep, into: &want.into, avoid: &want.avoid, ..self.terms(true, true) }, want.net, width(self.board, want.net), End::Item(want.from), End::Item(want.to))?;
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
    let spots = Arc::new(via_spots(board));
    let sheets: Vec<Sheet> = (0..board.layers.len()).map(|l| Sheet::new(Space::new(board, l, &[], &spots))).collect();
    let poured = poured(board);
    // On two layers a poured side is still routed on: the designers do (159 of the gate
    // tier's 253 two-layer boards have a side at least half poured).
    let planes: Vec<usize> = if poured.len() <= 2 || poured.iter().all(|s| *s >= 0.5) { Vec::new() } else { (0..poured.len()).filter(|&l| poured[l] >= 0.5).collect() };
    Plan { sheets, planes, vias: Vec::new(), left: Vec::new(), wants: Vec::new() }
}

/// Plans more connections into a plan, in the order given. Those already in it may be lifted
/// and laid again to let one through.
pub fn add(board: &Board, plan: &mut Plan, more: Vec<Want>) {
    let first = plan.wants.len();
    plan.wants.extend(more);
    place(board, plan, &(first..plan.wants.len()).collect::<Vec<_>>());
}

/// Lays again the connections a wire could not be drawn for, each barred from the gap its
/// wire stuck in (the connection's number and the gap), and with them every connection the
/// plan has no way for: the rest of the plan stays as it is, but for what is lifted to let
/// these through.
pub fn again(board: &Board, plan: &mut Plan, stuck: &[(usize, space::GapKey)]) {
    let mut which: Vec<usize> = plan.left.iter().map(|l| l.connection).collect();
    for &(c, gap) in stuck {
        if !plan.wants[c].avoid.contains(&gap) {
            plan.wants[c].avoid.push(gap);
        }
        which.push(c);
    }
    which.sort_by(|&a, &b| plan.wants[a].apart.total_cmp(&plan.wants[b].apart).then(a.cmp(&b)));
    which.dedup();
    for &c in &which {
        if let Some(lifted) = lift(board, &plan.sheets, c) {
            plan.sheets = lifted;
        }
    }
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
    let job = Job { board, dear: &dear, wants: &plan.wants };
    for &c in which {
        let want = &plan.wants[c];
        let best = job.best(&sheets, c).map(|b| b.1);
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
            continue;
        }
        let pads = board.item(want.from).zip(board.item(want.to)).is_some_and(|(f, t)| matches!(f.kind, Kind::Pad { .. }) && matches!(t.kind, Kind::Pad { .. }));
        let why = if pads { "no way through the room left, with vias or without" } else { "an end is not a pad" };
        plan.left.push(Left { connection: c, net: want.net, from: want.from, to: want.to, why: why.to_string() });
    }
    plan.vias = vias_of(&sheets);
    plan.sheets = sheets;
}
