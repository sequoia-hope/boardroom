//! Connections to copper that stays on the board.

use plan::{plan, strip};

/// A pad, and a short locked track of its net across the board with no pad or via on it: the
/// connection between the two is planned, ending on the track. (easy2350's "twin VBUS" group,
/// 2026-11-25: two short tracks and a via held by the board, which a stripped board leaves
/// with their far end bare; the two connections to that end were left with no way, on every
/// run, because a strand could end only on a pad or a via.)
#[test]
fn a_connection_may_end_on_a_track_that_stays() {
    let text = "(kicad_pcb (version 20240108) (generator \"pcbnew\") (general (thickness 1.6))
        (layers (0 \"F.Cu\" signal) (31 \"B.Cu\" signal) (44 \"Edge.Cuts\" user))
        (net 0 \"\") (net 1 \"A\")
        (gr_rect (start 0 0) (end 20 20) (layer \"Edge.Cuts\") (width 0.1))
        (footprint \"P\" (layer \"F.Cu\") (at 4 10) (property \"Reference\" \"R1\")
          (pad \"1\" smd rect (at 0 0) (size 1 1) (layers \"F.Cu\") (net 1 \"A\")))
        (segment (start 15 9) (end 15.5 9.8) (width 0.25) (locked yes) (layer \"F.Cu\") (net 1) (uuid \"11111111-1111-1111-1111-111111111111\")))";
    let rules = r#"{"board":{"design_settings":{"rules":{"min_clearance":0.2,"min_track_width":0.2,"min_copper_edge_clearance":0.3}}},
        "net_settings":{"classes":[{"name":"Default","clearance":0.2,"track_width":0.25,"via_diameter":0.6,"via_drill":0.3}]}}"#;
    let (pcb, copper) = kicad::parse_board(text).unwrap();
    let mut board = board::Board::new(&pcb, &copper, kicad::Rules::parse(rules).unwrap());
    strip(&mut board);
    assert_eq!(board::connectivity(&board).airlines.len(), 1, "the pad and the track are one connection to make");
    let plan = plan(&board);
    assert!(plan.left.is_empty(), "left: {:?}", plan.left.iter().map(|l| &l.why).collect::<Vec<_>>());
    assert_eq!(plan.sheets[0].laid().count(), 1);
}
