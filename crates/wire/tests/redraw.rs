//! Another router's copper drawn again by this one: the way kept, the copper in 0/45/90.

use board::{Board, ItemId, Kind, Shape};
use wire::redraw::redraw;

/// A 20 by 20 mm board of two layers: pads 0.6 mm square on the front, each `(x, y, net)`
/// with nets 1 "A" and 2 "B", and `copper`, the tracks and vias on it as the file has them.
/// Tracks 0.1 mm wide and vias 0.3 mm across by the net class, 0.1 mm clearance.
fn board(pads: &[(f64, f64, u32)], copper: &str) -> Board {
    let name = |net: u32| if net == 1 { "A" } else { "B" };
    let pads: String = pads.iter().map(|p| format!("(pad \"1\" smd rect (at {} {}) (size 0.6 0.6) (layers \"F.Cu\") (net {} \"{}\"))", p.0, p.1, p.2, name(p.2))).collect();
    let text = format!(
        "(kicad_pcb (version 20240108) (generator \"pcbnew\") (general (thickness 1.6))
         (layers (0 \"F.Cu\" signal) (31 \"B.Cu\" signal) (44 \"Edge.Cuts\" user))
         (net 0 \"\") (net 1 \"A\") (net 2 \"B\")
         (gr_rect (start 0 0) (end 20 20) (layer \"Edge.Cuts\") (width 0.1))
         (footprint \"P\" (layer \"F.Cu\") (at 0 0) {pads})
         {copper})"
    );
    let rules = r#"{"board":{"design_settings":{"rules":{"min_clearance":0.1,"min_copper_edge_clearance":0.3,"min_track_width":0.1,"min_hole_clearance":0.2,"min_hole_to_hole":0.25,"min_via_diameter":0.2,"min_through_hole_diameter":0.15}}},
            "net_settings":{"classes":[{"name":"Default","clearance":0.1,"track_width":0.1,"via_diameter":0.3,"via_drill":0.15}]}}"#;
    let (pcb, copper) = kicad::parse_board(&text).unwrap();
    Board::new(&pcb, &copper, kicad::Rules::parse(rules).unwrap())
}

fn track(a: [f64; 2], b: [f64; 2], layer: &str, net: u32) -> String {
    format!("(segment (start {} {}) (end {} {}) (width 0.1) (layer \"{layer}\") (net {net}))", a[0], a[1], b[0], b[1])
}

fn via(at: [f64; 2], size: f64, drill: f64, net: u32) -> String {
    format!("(via (at {} {}) (size {size}) (drill {drill}) (layers \"F.Cu\" \"B.Cu\") (net {net}))", at[0], at[1])
}

/// The tracks and vias on a board, in the file's order.
fn copper(board: &Board) -> Vec<ItemId> {
    board.items().filter(|(_, i)| matches!(i.kind, Kind::Track | Kind::Via)).map(|(id, _)| id).collect()
}

fn open(board: &Board) -> usize {
    board::connectivity_without_pours(board).airlines.len()
}

/// Whether every piece of every wire lies along 0, 45 or 90 degrees.
fn square(drawing: &wire::Drawing) -> bool {
    drawing.wires.iter().flat_map(|w| w.pts.windows(2)).all(|s| {
        let (dx, dy) = ((s[1][0] - s[0][0]).abs(), (s[1][1] - s[0][1]).abs());
        dx < 1e-6 || dy < 1e-6 || (dx - dy).abs() < 1e-6
    })
}

/// A track laid at odd angles from one pad to another, round a pad of another net, is drawn
/// again in 0/45/90 on the same side of that pad, and still joins the two.
#[test]
fn a_track_at_odd_angles_is_drawn_again_square_the_same_way_round() {
    let laid = [track([4.0, 10.0], [9.7, 8.3], "F.Cu", 1), track([9.7, 8.3], [16.0, 10.0], "F.Cu", 1)].concat();
    let board = board(&[(4.0, 10.0, 1), (16.0, 10.0, 1), (10.0, 10.0, 2)], &laid);
    assert_eq!(open(&board), 0);
    let again = redraw(&board, &copper(&board));
    assert!(again.as_laid.is_empty(), "{:?}", again.as_laid);
    assert_eq!((again.lifted.len(), again.drawing.wires.len()), (2, 1));
    assert!(square(&again.drawing), "{:?}", again.drawing.wires);
    assert_eq!(open(&wire::onto(&again.base, &again.plan, &again.drawing).0), 0);
    // Above the pad in the middle (a smaller y), as it was laid: not below it.
    let wire = &again.drawing.wires[0];
    assert!(wire.pts.iter().all(|p| p[1] < 10.0 + 1e-6), "{:?}", wire.pts);
    assert!(wire.pts.iter().any(|p| p[1] < 9.7), "{:?}", wire.pts);
}

/// Tracks that meet in a T, the foot of it in the middle of the other track, are read as two
/// paths that share a stem, and drawn again they join all three pads.
#[test]
fn tracks_that_meet_in_a_t_join_all_three_pads_drawn_again() {
    let laid = [track([4.0, 10.0], [16.0, 10.5], "F.Cu", 1), track([10.0, 10.25], [10.4, 15.0], "F.Cu", 1)].concat();
    let board = board(&[(4.0, 10.0, 1), (16.0, 10.5, 1), (10.4, 15.0, 1)], &laid);
    assert_eq!(open(&board), 0);
    let again = redraw(&board, &copper(&board));
    assert!(again.as_laid.is_empty(), "{:?}", again.as_laid);
    assert_eq!(again.drawing.wires.len(), 2);
    assert!(square(&again.drawing), "{:?}", again.drawing.wires);
    assert_eq!(open(&wire::onto(&again.base, &again.plan, &again.drawing).0), 0);
}

/// A way that changes layer keeps its via where it was, at our size, and its track on each
/// side is drawn again: a track that runs on through the via's middle with no corner there
/// ends at the via.
#[test]
fn a_way_through_a_via_keeps_the_via_and_is_drawn_again_on_both_layers() {
    let laid = [track([4.0, 10.0], [9.0, 11.3], "F.Cu", 1), via([9.0, 11.3], 0.5, 0.15, 1), track([7.0, 11.9], [9.0, 11.3], "B.Cu", 1), track([9.0, 11.3], [15.0, 9.5], "B.Cu", 1), via([15.0, 9.5], 0.5, 0.15, 1), track([15.0, 9.5], [16.0, 10.0], "F.Cu", 1)].concat();
    let board = board(&[(4.0, 10.0, 1), (16.0, 10.0, 1)], &laid);
    assert_eq!(open(&board), 0);
    let again = redraw(&board, &copper(&board));
    assert!(again.as_laid.is_empty(), "{:?}", again.as_laid);
    assert_eq!(again.plan.vias.len(), 2);
    assert!(again.plan.vias.iter().any(|v| (v.at[0] - 9.0).abs() < 1e-6 && (v.at[1] - 11.3).abs() < 1e-6), "{:?}", again.plan.vias);
    assert!(square(&again.drawing), "{:?}", again.drawing.wires);
    let after = wire::onto(&again.base, &again.plan, &again.drawing).0;
    assert_eq!(open(&after), 0);
    // The vias are ours now: the net class's size, not 0.5 mm across.
    let ours = plan::via_size(&board, 1).0;
    assert!(ours < 250_000);
    assert!(after.items().filter(|(_, i)| matches!(i.kind, Kind::Via)).all(|(_, i)| matches!(i.shape, Shape::Capsule { r, .. } if r == ours)));
    // The piece of track that ended on nothing is gone: nothing reaches where it began.
    assert!(!again.drawing.wires.iter().any(|w| w.pts.iter().any(|p| p[0] < 8.0 && w.layer == 1)), "{:?}", again.drawing.wires);
}

/// Only the copper asked for is drawn again. A new track that ends on a track that stays is
/// drawn to where it ended on it; and where two end on one track at two places, each is.
#[test]
fn a_track_that_ends_on_copper_that_stays_is_drawn_to_where_it_ended() {
    let laid = [track([4.0, 10.0], [16.0, 10.0], "F.Cu", 1), track([6.0, 10.0], [6.9, 15.0], "F.Cu", 1), track([14.0, 10.0], [13.2, 15.0], "F.Cu", 1)].concat();
    let board = board(&[(4.0, 10.0, 1), (16.0, 10.0, 1), (6.9, 15.0, 1), (13.2, 15.0, 1)], &laid);
    assert_eq!(open(&board), 0);
    let new = copper(&board)[1..].to_vec();
    let again = redraw(&board, &new);
    assert!(again.as_laid.is_empty(), "{:?}", again.as_laid);
    assert_eq!((again.lifted.len(), again.drawing.wires.len()), (2, 2));
    assert!(square(&again.drawing), "{:?}", again.drawing.wires);
    assert_eq!(open(&wire::onto(&again.base, &again.plan, &again.drawing).0), 0);
    // One wire has an end at 6, 10 and the other at 14, 10.
    for x in [6.0, 14.0] {
        let ends_there = |w: &wire::Wire| [w.pts[0], w.pts[w.pts.len() - 1]].iter().any(|p| (p[0] - x).abs() < 0.01 && (p[1] - 10.0).abs() < 0.01);
        assert!(again.drawing.wires.iter().any(ends_there), "no wire ends at {x}, 10: {:?}", again.drawing.wires);
    }
}

/// A via that cannot stand at our size where the other router's stood (theirs was smaller,
/// and a pad of another net is near) stays as it was laid, with why, and the tracks to it are
/// drawn again and end on it.
#[test]
fn a_via_that_does_not_stand_at_our_size_stays_and_its_tracks_are_drawn_to_it() {
    // A via 0.22 mm across with a 0.1 mm hole, 0.56 mm from the middle of the pad of net B:
    // its hole is 0.21 mm from that pad, and ours, 0.15 mm, would be 0.185 mm of the 0.2 asked.
    let laid = [track([4.0, 10.0], [9.0, 10.56], "F.Cu", 1), via([9.0, 10.56], 0.22, 0.1, 1), track([9.0, 10.56], [16.0, 10.0], "B.Cu", 1), via([16.0, 10.0], 0.22, 0.1, 1)].concat();
    let board = board(&[(4.0, 10.0, 1), (16.0, 10.0, 1), (9.0, 10.0, 2)], &laid);
    assert_eq!(open(&board), 0);
    let again = redraw(&board, &copper(&board));
    assert_eq!(again.as_laid.len(), 1, "{:?}", again.as_laid);
    assert!(again.as_laid[0].2.contains("a via of our size does not stand"), "{:?}", again.as_laid);
    assert_eq!(again.lifted.len(), 3);
    assert!(square(&again.drawing), "{:?}", again.drawing.wires);
    let after = wire::onto(&again.base, &again.plan, &again.drawing).0;
    assert_eq!(open(&after), 0);
    // No fault that the board as it was laid has not (its vias' holes are smaller than the
    // rules' least).
    assert!(board::check(&after).len() <= board::check(&board).len(), "{:?}", board::check(&after));
}

/// A new track that runs from its pad along a track of its net that stays, and on from that
/// track's end, is drawn again from where it leaves the track that stays.
#[test]
fn a_track_laid_along_copper_that_stays_is_drawn_from_where_it_leaves_it() {
    let laid = [track([4.0, 10.0], [8.0, 10.0], "F.Cu", 1), "(segment (start 4 10) (end 8 10) (width 0.12) (layer \"F.Cu\") (net 1))".to_string(), track([8.0, 10.0], [16.0, 11.3], "F.Cu", 1)].concat();
    let board = board(&[(4.0, 10.0, 1), (16.0, 11.3, 1)], &laid);
    assert_eq!(open(&board), 0);
    let new = copper(&board)[1..].to_vec();
    let again = redraw(&board, &new);
    assert!(again.as_laid.is_empty(), "{:?}", again.as_laid);
    assert_eq!(again.lifted.len(), 2);
    assert!(square(&again.drawing), "{:?}", again.drawing.wires);
    assert_eq!(open(&wire::onto(&again.base, &again.plan, &again.drawing).0), 0);
    // Its one wire starts on the track that stays, not back at the pad.
    assert_eq!(again.drawing.wires.len(), 1);
    let wire = &again.drawing.wires[0];
    assert!(wire.pts.iter().all(|p| p[0] > 7.9), "{:?}", wire.pts);
}
