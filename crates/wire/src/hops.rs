//! A net that steps up to one layer and down again for nothing. Every connection is planned
//! alone, so where several of one net meet, each may come up through a via of its own to meet
//! the others on another layer: a little island of track joining nothing but those vias, and
//! the vias going down again to the one layer they all leave on (p2-132-usb6's SW2_4: four
//! vias 0.43 mm apart, joined on the front, each leaving on In2). A designer joins them on the
//! layer they leave on and has no via there at all.
//!
//! [`hops`] finds such an island: a net's wires on one layer joining two or more of the plan's
//! vias, every one of which joins copper on one other layer, the same for all. Where the
//! island touches no pad, pour or copper that was on the board already, it goes, and the vias
//! are joined on that other layer by short pieces at 0, 45 or 90 degrees where they are not
//! joined already; vias left joining one layer go ([`loose`]). Where it does touch such
//! copper (p2-132-usb6's SW2_4 again: the four vias are a fan from one LED pad), one via is
//! kept, the others go and are joined to it on the other layer, and the island's stubs that
//! led to them are cut back ([`loose`]); of the vias, the one kept is the one that leaves least
//! track. Where a via goes and one wire on the other layer ends at it, the join may start
//! from that wire's last corner instead, its last piece taken off, so the track does not run
//! out to where the via stood and back. Either is kept only where the checker finds nothing against the new pieces and no
//! connection is left open and no pad short of spokes that was not before; otherwise the
//! island stays as it was.

use std::collections::{BTreeSet, HashMap};

use board::{Board, ItemId};
use plan::space::P;
use plan::Plan;

use crate::{bare_starved, onto, open, starved, Drawing, Wire};

/// A net's island on one layer: its wires, the vias they join, the other layer those join,
/// and whether it touches copper the router did not lay.
struct Island {
    /// The other layer: where the vias are joined.
    layer: usize,
    net: u32,
    wires: Vec<usize>,
    vias: Vec<usize>,
    fixed: bool,
}

/// `plan` and `drawing` with the islands described in the module taken out where that holds,
/// and how many vias went with them.
pub fn hops(board: &Board, plan: Plan, drawing: Drawing) -> (Plan, Drawing, usize) {
    let (mut plan, mut drawing) = (plan, drawing);
    let mut gone = 0;
    // Islands tried and left, by their vias' places: they are not tried again.
    let mut left: BTreeSet<Vec<(i64, i64)>> = BTreeSet::new();
    // What is open and short of spokes now, and the pads the bare board leaves short already:
    // worked out once there is an island to try. Each is a filling of the pours.
    let mut was: Option<(usize, usize)> = None;
    let mut before: Option<Vec<(ItemId, usize)>> = None;
    'again: loop {
        for island in islands(board, &plan, &drawing) {
            let key: Vec<(i64, i64)> = island.vias.iter().map(|&v| key_of(plan.vias[v].at)).collect();
            if left.contains(&key) {
                continue;
            }
            // Where the island touches fixed copper, each via in turn is the one kept: the
            // way that leaves fewest vias, then least track, is tried.
            let keeps: Vec<Option<usize>> = if island.fixed { island.vias.iter().map(|&v| Some(v)).collect() } else { vec![None] };
            let length = |d: &Drawing| -> f64 { d.wires.iter().flat_map(|w| w.pts.windows(2)).map(|s| (s[1][0] - s[0][0]).hypot(s[1][1] - s[0][1])).sum() };
            let best = keeps.into_iter().filter_map(|keep| rejoin(board, &plan, &drawing, &island, keep)).min_by(|x, y| x.0.vias.len().cmp(&y.0.vias.len()).then(length(&x.1).total_cmp(&length(&y.1))));
            if let Some((p, d, open_now)) = best {
                let bare = before.get_or_insert_with(|| bare_starved(board));
                let before = *was.get_or_insert_with(|| (open(board, &plan, &drawing), starved(board, &plan, &drawing, bare).len()));
                // The joining ends with the pours filled round what it leaves, and says what is
                // open then.
                let now = (open_now, starved(board, &p, &d, bare).len());
                if p.vias.len() < plan.vias.len() && now.0 <= before.0 && now.1 <= before.1 {
                    gone += plan.vias.len() - p.vias.len();
                    (plan, drawing, was) = (p, d, Some(now));
                    continue 'again;
                }
            }
            left.insert(key);
        }
        return (plan, drawing, gone);
    }
}

fn key_of(p: P) -> (i64, i64) {
    ((p[0] * 1e6).round() as i64, (p[1] * 1e6).round() as i64)
}

/// The islands of the drawing (see the module), each with its vias in the plan's order.
fn islands(board: &Board, plan: &Plan, drawing: &Drawing) -> Vec<Island> {
    let (routed, new) = onto(board, plan, drawing);
    let pieces: usize = drawing.wires.iter().map(|w| w.pts.len().saturating_sub(1)).sum();
    let mut wire_of: HashMap<ItemId, usize> = HashMap::new();
    let mut ids_of: Vec<Vec<ItemId>> = Vec::new();
    let mut ids = new.iter();
    for (w, wire) in drawing.wires.iter().enumerate() {
        let mine: Vec<ItemId> = ids.by_ref().take(wire.pts.len().saturating_sub(1)).copied().collect();
        for &id in &mine {
            wire_of.insert(id, w);
        }
        ids_of.push(mine);
    }
    let via_ids = &new[pieces.min(new.len())..];
    let via_ids = &via_ids[..plan.vias.len().min(via_ids.len())];
    let via_of: HashMap<ItemId, usize> = via_ids.iter().enumerate().map(|(v, &id)| (id, v)).collect();
    let mut poured: Vec<u32> = board.zone_areas.iter().map(|z| z.net).collect();
    poured.sort_unstable();
    // What joins `id` on `layer`: other copper of its net there that the board takes as joined.
    let joins = |id: ItemId, layer: usize| -> Vec<ItemId> {
        let Some(item) = routed.item(id) else { return Vec::new() };
        let mut near = routed.near(layer, item.shape.bbox());
        near.sort_unstable();
        near.dedup();
        near.into_iter().filter(|&o| o != id && routed.item(o).is_some_and(|it| it.net == item.net && it.on(layer) && it.is_copper()) && board::joined(&routed, id, o)).collect()
    };
    let mut seen = vec![false; drawing.wires.len()];
    let mut out = Vec::new();
    for first in 0..drawing.wires.len() {
        let (layer, net) = (drawing.wires[first].layer, drawing.wires[first].net);
        if seen[first] || net == 0 || poured.binary_search(&net).is_ok() {
            continue;
        }
        seen[first] = true;
        let (mut wires, mut vias, mut fixed) = (vec![first], BTreeSet::new(), false);
        let mut k = 0;
        while k < wires.len() {
            for &id in &ids_of[wires[k]] {
                for o in joins(id, layer) {
                    if let Some(&w) = wire_of.get(&o) {
                        if !seen[w] {
                            seen[w] = true;
                            wires.push(w);
                        }
                    } else if let Some(&v) = via_of.get(&o) {
                        vias.insert(v);
                    } else {
                        fixed = true;
                    }
                }
            }
            k += 1;
        }
        if vias.len() < 2 {
            continue;
        }
        // Each via's other layers: one, and the same for all.
        let mut other: Option<usize> = None;
        let mut one = true;
        for &v in &vias {
            let l: Vec<usize> = (0..board.layers.len()).filter(|&l| l != layer && !joins(via_ids[v], l).is_empty()).collect();
            if l.len() != 1 || other.is_some_and(|o| o != l[0]) {
                one = false;
                break;
            }
            other = Some(l[0]);
        }
        if !one {
            continue;
        }
        wires.sort_unstable();
        out.push(Island { layer: other.unwrap_or(layer), net, wires, vias: vias.into_iter().collect(), fixed });
    }
    out
}

/// A wire cut back by its last piece where it ends at a via's place that goes: the wire and
/// that place.
type Cut = Option<(usize, P)>;

/// The ways from `a` to `b` at 0, 45 or 90 degrees with one bend at most: straight where it
/// can be, else the slant first or the straight first.
fn ways(a: P, b: P) -> Vec<Vec<P>> {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let tiny = 1e-6;
    if dx.abs() < tiny || dy.abs() < tiny || (dx.abs() - dy.abs()).abs() < tiny {
        return vec![vec![a, b]];
    }
    let m = dx.abs().min(dy.abs());
    let slant = [m * dx.signum(), m * dy.signum()];
    vec![vec![a, [a[0] + slant[0], a[1] + slant[1]], b], vec![a, [b[0] - slant[0], b[1] - slant[1]], b]]
}

/// Whether a join through `pts` would leave either of its ends at less than a right angle to
/// a wire of the net on its layer that leaves the same point: a sharp wedge of copper where
/// the via stood, which no designer draws (gb-hardware's NT1, 2026-11-25). Where an end is
/// one of `corners`, a place no via stands at any more, so that the wire and the join make one
/// track bent there, a right angle is sharp too where the join bends again: the track would
/// turn a right angle and turn back, a spike out of its way (saiboard_8x8, 2026-11-26).
fn sharp(drawing: &Drawing, island: &Island, pts: &[P], corners: &[P]) -> bool {
    let n = pts.len();
    let leaving = |at: P, next: P| [next[0] - at[0], next[1] - at[1]];
    [(pts[0], leaving(pts[0], pts[1])), (pts[n - 1], leaving(pts[n - 1], pts[n - 2]))].into_iter().any(|(at, dir)| {
        let corner = n > 2 && corners.iter().any(|c| (c[0] - at[0]).hypot(c[1] - at[1]) < 1e-3);
        let most = if corner { -0.5 } else { 1e-6 };
        let near = |p: P| (p[0] - at[0]).hypot(p[1] - at[1]) < 1e-3;
        // Which way a wire leaves its end: toward its first point clear of the end, since a
        // wire drawn as planned can end in a piece far shorter than it is wide (gb-hardware's
        // NT1 ends in one 0.0005 mm long, at no angle that means anything).
        let away = |pts: Vec<P>| -> Option<[f64; 2]> {
            let end = *pts.first()?;
            pts.iter().find(|p| (p[0] - end[0]).hypot(p[1] - end[1]) > 1e-2).map(|&p| leaving(end, p))
        };
        drawing.wires.iter().filter(|w| w.net == island.net && w.layer == island.layer && w.pts.len() >= 2).any(|w| {
            let m = w.pts.len();
            let other = if near(w.pts[0]) {
                away(w.pts.clone())
            } else if near(w.pts[m - 1]) {
                away(w.pts.iter().rev().copied().collect())
            } else {
                None
            };
            let Some(other) = other else { return false };
            let cos = (dir[0] * other[0] + dir[1] * other[1]) / (dir[0].hypot(dir[1]) * other[0].hypot(other[1]));
            cos > most
        })
    })
}

/// The plan and drawing with `island`'s vias joined on its other layer: with the island taken
/// out where `keep` is none, or with every via of it but `keep` taken out; the vias and stubs
/// left joining nothing gone, and how many connections are then open. None where a piece the
/// joining needs cannot be laid.
fn rejoin(board: &Board, plan: &Plan, drawing: &Drawing, island: &Island, keep: Option<usize>) -> Option<(Plan, Drawing, usize)> {
    let mut d = drawing.clone();
    let template = island.wires.first().map(|&w| drawing.wires[w].clone())?;
    let mut p = plan.clone();
    if keep.is_some() {
        let mut k = 0;
        p.vias.retain(|_| {
            k += 1;
            Some(k - 1) == keep || island.vias.binary_search(&(k - 1)).is_err()
        });
    } else {
        let mut k = 0;
        d.wires.retain(|_| {
            k += 1;
            island.wires.binary_search(&(k - 1)).is_err()
        });
    }
    // The width of the net's wires at the vias on the other layer, or the island's.
    let near_via = |w: &Wire| island.vias.iter().any(|&v| w.pts.iter().any(|p| (p[0] - plan.vias[v].at[0]).hypot(p[1] - plan.vias[v].at[1]) < 1e-3));
    let width = d.wires.iter().filter(|w| w.net == island.net && w.layer == island.layer && near_via(w)).map(|w| w.width).fold(None, |m: Option<f64>, x| Some(m.map_or(x, |m| m.max(x)))).unwrap_or(template.width);
    // Which of the vias' places the net's copper on the other layer joins already, without
    // what was taken out: by the copper there that each lies in.
    let (routed, _) = onto(board, &p, &d);
    let cluster = board::net_connectivity(&routed, island.net, false).cluster;
    let mut group: Vec<usize> = island
        .vias
        .iter()
        .map(|&v| {
            let at = board::Point::new((plan.vias[v].at[0] * 1e6).round() as i64, (plan.vias[v].at[1] * 1e6).round() as i64);
            let mut near = routed.near(island.layer, board::BBox::of([at]));
            near.sort_unstable();
            near.into_iter().filter(|&o| routed.item(o).is_some_and(|it| it.net == island.net && it.on(island.layer) && it.shape.contains(at))).find_map(|o| cluster.get(&o).copied()).unwrap_or(usize::MAX - v)
        })
        .collect();
    // The pairs of vias nearest first, each joined where the two are not joined yet.
    let mut pairs: Vec<(f64, usize, usize)> = Vec::new();
    for i in 0..island.vias.len() {
        for j in i + 1..island.vias.len() {
            let (a, b) = (plan.vias[island.vias[i]].at, plan.vias[island.vias[j]].at);
            pairs.push(((a[0] - b[0]).hypot(a[1] - b[1]), i, j));
        }
    }
    pairs.sort_by(|x, y| x.0.total_cmp(&y.0).then((x.1, x.2).cmp(&(y.1, y.2))));
    let near = |a: P, b: P| (a[0] - b[0]).hypot(a[1] - b[1]) < 1e-3;
    let length = |pts: &[P]| pts.windows(2).map(|s| (s[1][0] - s[0][0]).hypot(s[1][1] - s[0][1])).sum::<f64>();
    let gone: Vec<P> = island.vias.iter().filter(|&&v| Some(v) != keep).map(|&v| plan.vias[v].at).collect();
    for (_, i, j) in pairs {
        if group[i] == group[j] {
            continue;
        }
        // Where a join may start at each via's place: the place itself, or, where the via goes
        // and one wire of the net on the layer ends there and nothing else of the net is there,
        // that wire's last corner, its last piece taken off: so the wire does not run on past
        // where the join leaves it and come back (saiboard_8x8's GND, 2026-11-26).
        // Each is the place the join starts, the wire cut back to it, and the length cut off.
        let ends = |d: &Drawing, v: usize| -> Vec<(P, Cut, f64)> {
            let at = plan.vias[v].at;
            let mut out = vec![(at, None, 0.0)];
            if Some(v) == keep {
                return out;
            }
            let on = |w: &Wire| w.net == island.net && w.layer == island.layer && w.pts.windows(2).any(|s| plan::space::point_segment(at, s[0], s[1]) < 1e-3);
            let mut there = d.wires.iter().enumerate().filter(|(_, w)| on(w));
            let (Some((k, w)), None) = (there.next(), there.next()) else { return out };
            let m = w.pts.len();
            let nm = board::Point::new((at[0] * 1e6).round() as i64, (at[1] * 1e6).round() as i64);
            let fixed = board.near(island.layer, board::BBox::of([nm])).into_iter().any(|o| board.item(o).is_some_and(|it| it.net == island.net && !matches!(it.kind, board::Kind::Via) && it.shape.contains(nm)));
            if m >= 3 && !fixed && (near(w.pts[0], at) || near(w.pts[m - 1], at)) {
                let corner = if near(w.pts[0], at) { w.pts[1] } else { w.pts[m - 2] };
                out.push((corner, Some((k, at)), plan::space::dist(corner, at)));
            }
            out
        };
        // Each way from each end of the one to each of the other, least track first.
        let mut tries: Vec<(f64, Vec<P>, [Cut; 2])> = Vec::new();
        for (a, wa, ca) in ends(&d, island.vias[i]) {
            for (b, wb, cb) in ends(&d, island.vias[j]) {
                for pts in ways(a, b) {
                    tries.push((length(&pts) - ca - cb, pts, [wa, wb]));
                }
            }
        }
        tries.sort_by(|x, y| x.0.total_cmp(&y.0));
        let laid = tries.into_iter().find(|(_, pts, cuts)| {
            let was = d.clone();
            let mut corners = gone.clone();
            for &(k, at) in cuts.iter().flatten() {
                let w = &mut d.wires[k];
                if near(w.pts[0], at) {
                    w.pts.remove(0);
                    corners.push(w.pts[0]);
                } else {
                    w.pts.pop();
                    corners.push(*w.pts.last().unwrap());
                }
            }
            if sharp(&d, island, pts, &corners) {
                d = was;
                return false;
            }
            d.wires.push(Wire { layer: island.layer, width, pts: pts.clone(), as_planned: false, ..template.clone() });
            let (routed, new) = onto(board, &p, &d);
            let first: usize = d.wires[..d.wires.len() - 1].iter().map(|w| w.pts.len().saturating_sub(1)).sum();
            // A pour is filled again round new copper, as `route` has it: KiCad's refill is the
            // judge of that.
            let clean = new[first..first + pts.len() - 1].iter().all(|&id| board::check_item(&routed, id).iter().all(|v| [Some(v.a), v.b].into_iter().flatten().any(|o| routed.item(o).is_some_and(|i| i.refilled()))));
            if !clean {
                d = was;
            }
            clean
        });
        laid?;
        let (from, to) = (group[j], group[i]);
        group.iter_mut().filter(|g| **g == from).for_each(|g| *g = to);
    }
    Some(crate::loose::loose_counting(board, &p, &d))
}
