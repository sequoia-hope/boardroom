//! Copper of ours that ends on nothing. Where one wire of a connection through a via is drawn
//! and the other is not, the via is left joining copper on one layer (KiCad's `via_dangling`)
//! and the wire to it leads nowhere. [`loose`] takes such vias out, and of a wire with an end
//! that touches nothing of its net the stretch from that end back to where other copper of
//! the net last joins it (all of it, where none does but at its other end), again and again
//! until nothing more goes: what is taken joins nothing, so nothing that was joined comes
//! apart. Other wires do join a wire along its length: a pad's short way to its via, once the
//! wires that left from the via have been cut back to where they meet that way
//! ([`crate::spare`]) and the via has gone for joining one layer only.
//!
//! Joined is as KiCad has it ([`board::joined`]): a via joins a layer where copper of its net
//! there joins it, or its middle lies in its net's pour there; a wire's end touches its net's
//! copper where its end piece joins copper of the net that comes within the piece's width of
//! the end (two wide tracks join where they overlap, though neither's end lies in the other:
//! KiwiSDR's 1 mm tracks), or the piece lies in its net's pour.

use board::{Board, ItemId, Kind, Point, Pours, Shape};
use plan::space::P;
use plan::Plan;

use crate::{onto, Drawing};

/// `plan`'s vias and `drawing`'s wires without those that join nothing (see the module): the
/// plan comes back with fewer vias and is for writing the board, not for planning again.
pub fn loose(board: &Board, plan: &Plan, drawing: &Drawing) -> (Plan, Drawing) {
    let (plan, drawing, _) = loose_counting(board, plan, drawing);
    (plan, drawing)
}

/// [`loose`], and how many connections are open with what it leaves on the board and the pours
/// filled round it ([`crate::open`] of it): the pours it has just filled to find that nothing
/// more is loose are the ones that count asks for, and filling them is most of its cost.
pub(crate) fn loose_counting(board: &Board, plan: &Plan, drawing: &Drawing) -> (Plan, Drawing, usize) {
    let mut plan = plan.clone();
    let mut drawing = drawing.clone();
    loop {
        let (routed, new) = onto(board, &plan, &drawing);
        let pours = Pours::of(&routed);
        // `onto` puts in each wire's pieces in order, then the vias, then the ties and stitches.
        let pieces: usize = drawing.wires.iter().map(|w| w.pts.len().saturating_sub(1)).sum();
        let (wire_ids, rest) = new.split_at(pieces.min(new.len()));
        let via_ids = &rest[..plan.vias.len().min(rest.len())];

        let lone: Vec<bool> = via_ids.iter().map(|&id| layers_joined(&routed, &pours, id) < 2).collect();
        let mut first = 0;
        // What is left of each wire: none where all of it goes.
        let left: Vec<Option<Vec<P>>> = drawing
            .wires
            .iter()
            .map(|w| {
                let n = w.pts.len().saturating_sub(1);
                let ids = &wire_ids[first..first + n];
                first += n;
                let (Some(&a), Some(&b)) = (ids.first(), ids.last()) else { return None };
                let at = |p: P| Point::new((p[0] * 1e6).round() as i64, (p[1] * 1e6).round() as i64);
                if !end_touches(&routed, &pours, ids, b, at(w.pts[w.pts.len() - 1]), w.layer) {
                    cut_back(&routed, ids, &w.pts, w.layer)
                } else if !end_touches(&routed, &pours, ids, a, at(w.pts[0]), w.layer) {
                    let (back, ids): (Vec<P>, Vec<ItemId>) = (w.pts.iter().rev().copied().collect(), ids.iter().rev().copied().collect());
                    cut_back(&routed, &ids, &back, w.layer).map(|pts| pts.into_iter().rev().collect())
                } else {
                    Some(w.pts.clone())
                }
            })
            .collect();
        if !lone.contains(&true) && left.iter().zip(&drawing.wires).all(|(l, w)| l.as_ref() == Some(&w.pts)) {
            let open = board::connectivity_poured(&routed, &pours).airlines.len();
            return (plan, drawing, open);
        }
        let mut k = 0;
        plan.vias.retain(|_| {
            k += 1;
            !lone[k - 1]
        });
        let mut left = left.into_iter();
        drawing.wires.retain_mut(|w| match left.next().flatten() {
            Some(pts) => {
                w.pts = pts;
                true
            }
            None => false,
        });
    }
}

/// A wire through `pts`, its pieces `ids`, whose last end touches nothing: the wire from its
/// first end to where other copper of its net last joins it, or none where nothing joins it
/// but at its first end. Two things join at an anchor of one that lies in the other's copper
/// ([`board::joined`]), so the place is the other's anchor on the piece it joins, or the end
/// of that piece that lies in the other's copper. Copper the wire's first end lies in is what
/// it starts from, not something met along it: a wire whose first corner is still inside the
/// via it leaves would else keep that stump, and the stump the via.
fn cut_back(routed: &Board, ids: &[ItemId], pts: &[P], layer: usize) -> Option<Vec<P>> {
    let nm = |p: P| [p[0] * 1e6, p[1] * 1e6];
    let start = Point::new(nm(pts[0])[0].round() as i64, nm(pts[0])[1].round() as i64);
    let start = Shape::Capsule { a: start, b: start, r: 0 };
    for k in (0..ids.len()).rev() {
        let piece = routed.item(ids[k])?;
        let Shape::Capsule { r, .. } = piece.shape else { return None };
        let (a, b) = (nm(pts[k]), nm(pts[k + 1]));
        let (d, len2) = ([b[0] - a[0], b[1] - a[1]], (b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2));
        // How far along the piece, as a share of it, the join farthest along it is.
        let mut far: Option<f64> = None;
        for o in routed.near(layer, piece.shape.bbox()) {
            let Some(it) = routed.item(o) else { continue };
            if ids.contains(&o) || it.net != piece.net || !it.on(layer) || !it.is_copper() || matches!(it.kind, Kind::Pour { .. }) || it.shape.gap(&start) == 0.0 || !board::joined(routed, ids[k], o) {
                continue;
            }
            // The other's anchors, each with how far round it the anchor reaches.
            let anchors: Vec<(Point, i64)> = match (&it.kind, &it.shape) {
                (Kind::Track, Shape::Capsule { a, b, .. }) => vec![(*a, 0), (*b, 0)],
                (_, Shape::Capsule { a, b, r }) if a == b => vec![(*a, *r)],
                (_, shape) => {
                    let bbox = shape.bbox();
                    vec![(Point::new((bbox.min.x + bbox.max.x) / 2, (bbox.min.y + bbox.max.y) / 2), 0)]
                }
            };
            let mut here: Option<f64> = None;
            for (q, reach) in anchors {
                let q = [q.x as f64, q.y as f64];
                let t = if len2 > 0.0 { (((q[0] - a[0]) * d[0] + (q[1] - a[1]) * d[1]) / len2).clamp(0.0, 1.0) } else { 0.0 };
                let foot = [a[0] + t * d[0], a[1] + t * d[1]];
                if (q[0] - foot[0]).hypot(q[1] - foot[1]) <= (r + reach) as f64 + 1000.0 {
                    here = Some(here.map_or(t, |h: f64| h.max(t)));
                }
            }
            // Joined by an anchor of the piece's own: the end of it that lies in the other.
            let within = |p: [f64; 2]| {
                let p = Point::new(p[0].round() as i64, p[1].round() as i64);
                it.shape.gap(&Shape::Capsule { a: p, b: p, r: 0 }) == 0.0
            };
            let t = here.unwrap_or(if !within(b) && within(a) { 0.0 } else { 1.0 });
            far = Some(far.map_or(t, |f| f.max(t)));
        }
        let Some(t) = far else { continue };
        let mut kept = pts[..=k].to_vec();
        if t * len2.sqrt() > 1000.0 {
            kept.push(if t >= 1.0 { pts[k + 1] } else { [pts[k][0] + t * (pts[k + 1][0] - pts[k][0]), pts[k][1] + t * (pts[k + 1][1] - pts[k][1])] });
        }
        return (kept.len() >= 2).then_some(kept);
    }
    None
}

/// On how many layers via `id` joins copper of its net: copper there that joins it, or its
/// net's pour there.
fn layers_joined(routed: &Board, pours: &Pours, id: ItemId) -> usize {
    let Some(via) = routed.item(id) else { return 0 };
    let bbox = via.shape.bbox();
    (0..routed.layers.len())
        .filter(|&l| via.on(l))
        .filter(|&l| {
            !pours.joined(routed, id, l).is_empty()
                || routed.near(l, bbox).into_iter().any(|o| o != id && routed.item(o).is_some_and(|it| it.net == via.net && it.on(l) && !matches!(it.kind, Kind::Pour { .. })) && board::joined(routed, id, o))
        })
        .count()
}

/// Whether the end `p` of a wire, whose pieces are `own` and whose end piece is `piece`, on
/// `layer`, touches copper of its net that is not the wire itself (see the module).
fn end_touches(routed: &Board, pours: &Pours, own: &[ItemId], piece: ItemId, p: Point, layer: usize) -> bool {
    let Some(item) = routed.item(piece) else { return false };
    let reach = match item.shape {
        Shape::Capsule { r, .. } => r,
        _ => 0,
    };
    let about = Shape::Capsule { a: p, b: p, r: reach };
    routed.near(layer, about.bbox()).into_iter().any(|o| {
        !own.contains(&o)
            && routed.item(o).is_some_and(|it| it.net == item.net && it.on(layer) && it.is_copper() && !matches!(it.kind, Kind::Pour { .. }) && it.shape.gap(&about) == 0.0)
            && board::joined(routed, piece, o)
    }) || !pours.joined(routed, piece, layer).is_empty()
}
