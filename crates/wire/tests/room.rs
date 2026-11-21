//! What the plan keeps round vias and pads: a small via's hole, the corners a simpler outline
//! leaves out, a pad's side that leans over the room beside it, and the way out of a pin
//! that stands between two others.

use std::sync::Arc;

use board::Board;
use plan::space::{Space, Spots, VIA_CORNER};

/// A 20 by 20 mm board of two layers with `parts` on it: tracks 0.1 mm wide and 0.1 mm
/// apart, vias 0.3 mm across with a 0.15 mm hole (a ring of 0.075 mm, the least the board
/// allows), and copper 0.2 mm from any hole.
fn board(parts: &str) -> Board {
    let text = format!(
        "(kicad_pcb (version 20240108) (generator \"pcbnew\") (general (thickness 1.6))
         (layers (0 \"F.Cu\" signal) (31 \"B.Cu\" signal) (44 \"Edge.Cuts\" user))
         (net 0 \"\") (net 1 \"A\") (net 2 \"B\") (net 3 \"C\")
         (gr_rect (start 0 0) (end 20 20) (layer \"Edge.Cuts\") (width 0.1))
         {parts})"
    );
    let rules = r#"{"board":{"design_settings":{"rules":{"min_clearance":0.1,"min_copper_edge_clearance":0.3,"min_track_width":0.1,"min_hole_clearance":0.2,"min_hole_to_hole":0.25,"min_via_diameter":0.2,"min_through_hole_diameter":0.15,"min_via_annular_width":0.075}}},
            "net_settings":{"classes":[{"name":"Default","clearance":0.1,"track_width":0.1,"via_diameter":0.3,"via_drill":0.15}]}}"#;
    let (pcb, copper) = kicad::parse_board(&text).unwrap();
    Board::new(&pcb, &copper, kicad::Rules::parse(rules).unwrap())
}

fn pad(x: f64, y: f64, w: f64, h: f64, shape: &str, net: &str) -> String {
    let n = ["", "A", "B", "C"].iter().position(|&k| k == net).unwrap();
    format!("(pad \"{net}\" smd {shape} (at {x} {y}) (size {w} {h}) (layers \"F.Cu\") (net {n} \"{net}\"))")
}

fn part(pads: &[String]) -> String {
    format!("(footprint \"P\" (layer \"F.Cu\") (at 0 0) {})", pads.concat())
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

/// A via 0.3 mm across with a 0.15 mm hole has 0.075 mm of copper round the hole. A track
/// 0.1 mm from that copper is 0.175 mm from the hole, and the rules ask 0.2: the plan keeps
/// a track of another net 0.125 mm from the via's copper, and two vias 0.425 mm apart (the
/// copper of one from the hole of the other), not 0.4.
#[test]
fn a_track_keeps_the_rules_distance_from_a_small_vias_hole() {
    let board = board("");
    let spots = plan::via_spots(&board, false);
    assert!(near(spots.hole, 0.125), "{}", spots.hole);
    assert!(near(spots.apart, 0.425), "{}", spots.apart);
    // A via of the plan, of net A, as an obstacle: what a track of B keeps from its middle.
    let space = Space::new(&board, 0, &[([10.0, 10.0], 1)], &Arc::new(spots));
    let corner = (0..space.verts.len()).find(|&v| space.vert_of[v] == Some(space.sites[0].obstacle)).unwrap();
    assert!(near(space.keep(&board, corner, 2), 0.125 + 0.15 + VIA_CORNER), "{}", space.keep(&board, corner, 2));
    assert!(near(space.keep(&board, corner, 1), 0.0));
    // Spots stand far enough apart for a track between two vias.
    let spots = &space.spots;
    let nearest = spots.at.iter().enumerate().flat_map(|(i, a)| spots.at[..i].iter().map(move |b| plan::space::dist(*a, *b))).fold(f64::MAX, f64::min);
    assert!(nearest >= 0.3 + 2.0 * 0.125 + 0.1, "{nearest}");
}

/// A pad with round corners is held as a simpler outline, and its round corners lie outside
/// that: a track keeps its clearance from where the copper is.
#[test]
fn a_track_keeps_clear_of_the_corners_a_simpler_outline_leaves_out() {
    let round = "(footprint \"P\" (layer \"F.Cu\") (at 10 10) (pad \"1\" smd roundrect (at 0 0) (size 0.6 1.2) (layers \"F.Cu\") (roundrect_rratio 0.25) (net 1 \"A\")))";
    let board = board(round);
    let space = Space::new(&board, 0, &[], &Arc::new(Spots::default()));
    let o = space.obstacles.iter().position(|o| o.net == 1).unwrap();
    let pad = &space.obstacles[o];
    assert!(pad.past > 0.0 && pad.past <= 0.0101, "{} with {} corners", pad.past, pad.poly.len());
    let corner = (0..space.verts.len()).find(|&v| space.vert_of[v] == Some(o)).unwrap();
    assert!(near(space.keep(&board, corner, 2), 0.1 + pad.past));
}

/// A via 0.3 mm above the long side of a pad, 0.5 mm in from the pad's corner: what passes
/// between the two passes between the via and the side, 0.3 mm, not through the 0.58 mm
/// from the via to the corner.
#[test]
fn the_room_between_a_corner_and_a_via_is_no_more_than_its_pads_side_leaves() {
    let board = board(&part(&[pad(10.0, 10.0, 3.0, 1.0, "rect", "A")]));
    let space = Space::new(&board, 0, &[], &Arc::new(Spots::default()));
    // The pad's corner at (11.5, 9.5), and a place above its top side.
    let corner = (0..space.verts.len()).find(|&v| near(space.verts[v][0], 11.5) && near(space.verts[v][1], 9.5)).unwrap();
    assert!(near(space.room_to(corner, [11.0, 9.2]), 0.3), "{}", space.room_to(corner, [11.0, 9.2]));
    // Out past the corner nothing leans over: the room is the distance to the corner.
    assert!(near(space.room_to(corner, [11.8, 9.1]), 0.5), "{}", space.room_to(corner, [11.8, 9.1]));
}

/// Three pins 0.4 mm apart: nearer than two vias may stand. The way straight out of each is
/// kept free of other nets' vias, for the middle one has no other; its own via may stand in
/// it.
#[test]
fn no_via_of_another_net_stands_in_the_way_out_of_a_fine_pitch_pin() {
    let row = part(&[pad(9.6, 10.0, 0.2, 0.8, "rect", "A"), pad(10.0, 10.0, 0.2, 0.8, "rect", "B"), pad(10.4, 10.0, 0.2, 0.8, "rect", "C"), pad(10.0, 13.0, 3.0, 0.5, "rect", "")]);
    let board = board(&row);
    let spots = plan::via_spots(&board, false);
    assert_eq!(spots.ways_out.len(), 3, "{:?}", spots.ways_out);
    // The row's pads point away from the part's middle, which lies below them: up.
    let b = spots.ways_out.iter().find(|w| w.2 == 2).unwrap();
    assert!(near(b.0[0], 10.0) && near(b.0[1], 9.6) && b.1[1] < 9.2, "{b:?}");
    assert!(spots.shuts([10.1, 9.3], 1));
    assert!(!spots.shuts([10.0, 9.3], 2));
    assert!(!spots.shuts([10.0, 8.0], 1));
    // Not where each pin's via has its own place, beside its neighbours'.
    assert!(plan::via_spots(&board, true).ways_out.is_empty());
}

/// What the checker says of a via alone on the bare board is asked once for a place and a
/// net, and is the same the next time: on another net's pad it may not stand, in the open it
/// may, and with no thermal spoke near the plan's other vias have no say in it.
#[test]
fn the_checkers_answer_for_a_via_alone_is_kept() {
    let board = board(&part(&[pad(5.0, 5.0, 1.0, 1.0, "rect", "A")]));
    let spots = Spots { at: vec![[10.0, 10.0], [5.2, 5.0]], ..plan::via_spots(&board, false) };
    let asked = std::cell::Cell::new(0);
    let via = |at: [f64; 2], net: u32| {
        asked.set(asked.get() + 1);
        let c = board::Point::new((at[0] * 1e6) as i64, (at[1] * 1e6) as i64);
        board::Item { kind: board::Kind::Via, net, layers: 0b11, shape: board::Shape::Capsule { a: c, b: c, r: 150_000 }, hole: Some((c, 75_000)), locked: false, span: None }
    };
    assert_eq!(spots.alone(&board, 0, 2, &|| via([10.0, 10.0], 2)), (true, false));
    assert!(!spots.alone(&board, 1, 2, &|| via([5.2, 5.0], 2)).0);
    assert_eq!(asked.get(), 2);
    assert_eq!(spots.alone(&board, 0, 2, &|| via([10.0, 10.0], 2)), (true, false));
    assert!(!spots.alone(&board, 1, 2, &|| via([5.2, 5.0], 2)).0);
    assert_eq!(asked.get(), 2, "asked again of a place it knew");
}
