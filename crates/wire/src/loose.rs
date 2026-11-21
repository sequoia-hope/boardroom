//! Copper of ours that ends on nothing. Where one wire of a connection through a via is drawn
//! and the other is not, the via is left joining copper on one layer (KiCad's `via_dangling`)
//! and the wire to it leads nowhere. [`loose`] takes such vias out, and wires with an end that
//! touches nothing of their net, again and again until nothing more goes: neither joins
//! anything, so nothing that was joined comes apart.
//!
//! Joined is as KiCad has it ([`board::joined`]): a via joins a layer where copper of its net
//! there joins it, or its middle lies in its net's pour there; a wire's end touches its net's
//! copper where its end piece joins copper of the net that comes within the piece's width of
//! the end (two wide tracks join where they overlap, though neither's end lies in the other:
//! KiwiSDR's 1 mm tracks), or the piece lies in its net's pour.

use board::{Board, ItemId, Kind, Point, Pours, Shape};
use plan::Plan;

use crate::{onto, Drawing};

/// `plan`'s vias and `drawing`'s wires without those that join nothing (see the module): the
/// plan comes back with fewer vias and is for writing the board, not for planning again.
pub fn loose(board: &Board, plan: &Plan, drawing: &Drawing) -> (Plan, Drawing) {
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
        let free: Vec<bool> = drawing
            .wires
            .iter()
            .map(|w| {
                let n = w.pts.len().saturating_sub(1);
                let ids = &wire_ids[first..first + n];
                first += n;
                let (Some(&a), Some(&b)) = (ids.first(), ids.last()) else { return true };
                let at = |p: plan::space::P| Point::new((p[0] * 1e6).round() as i64, (p[1] * 1e6).round() as i64);
                !end_touches(&routed, &pours, ids, a, at(w.pts[0]), w.layer) || !end_touches(&routed, &pours, ids, b, at(w.pts[w.pts.len() - 1]), w.layer)
            })
            .collect();
        if !lone.contains(&true) && !free.contains(&true) {
            return (plan, drawing);
        }
        let mut k = 0;
        plan.vias.retain(|_| {
            k += 1;
            !lone[k - 1]
        });
        let mut k = 0;
        drawing.wires.retain(|_| {
            k += 1;
            !free[k - 1]
        });
    }
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
