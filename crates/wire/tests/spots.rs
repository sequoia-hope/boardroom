//! Where a via may stand: on the grid, and where the corners round about leave most room.

use board::Board;

const RULES: &str = r#"{"board":{"design_settings":{"rules":{"min_clearance":0.2,"min_copper_edge_clearance":0.3,"min_track_width":0.2}}},
    "net_settings":{"classes":[{"name":"Default","clearance":0.2,"track_width":0.25,"via_diameter":0.8,"via_drill":0.4}]}}"#;

/// Four pins of other nets at the corners of a square 2.2 mm across, about (10.2, 10.2): a via
/// fits between them only within 0.15 mm of the square's middle, and the grid's nearest points
/// are 0.7 mm from it, each beside a pin.
fn pocket() -> Board {
    let pin = |x: f64, y: f64| format!("(footprint \"P\" (layer \"F.Cu\") (at {x} {y}) (pad \"1\" thru_hole circle (at 0 0) (size 1.6 1.6) (drill 0.8) (layers \"*.Cu\" \"*.Mask\") (net 1 \"A\")))");
    let text = format!(
        "(kicad_pcb (version 20240108) (generator \"pcbnew\") (general (thickness 1.6))
         (layers (0 \"F.Cu\" signal) (31 \"B.Cu\" signal) (44 \"Edge.Cuts\" user))
         (net 0 \"\") (net 1 \"A\")
         (gr_rect (start 0 0) (end 20 20) (layer \"Edge.Cuts\") (width 0.1))
         {} {} {} {})",
        pin(9.1, 9.1),
        pin(11.3, 9.1),
        pin(9.1, 11.3),
        pin(11.3, 11.3)
    );
    let (pcb, copper) = kicad::parse_board(&text).unwrap();
    Board::new(&pcb, &copper, kicad::Rules::parse(RULES).unwrap())
}

#[test]
fn a_via_may_stand_in_the_middle_of_four_pins_where_the_grid_has_no_point() {
    let board = pocket();
    let spots = plan::via_spots(&board, false);
    let from_middle = |p: &[f64; 2]| (p[0] - 10.2).hypot(p[1] - 10.2);
    let within: Vec<&[f64; 2]> = spots.at.iter().filter(|p| from_middle(p) < 1.0).collect();
    assert!(!within.is_empty(), "no spot between the pins");
    assert!(within.iter().all(|p| from_middle(p) < 0.15), "a spot beside a pin: {within:?}");
}

/// Every spot is a place the checker lets a via stand on the bare board.
#[test]
fn every_spot_passes_the_checker() {
    let board = pocket();
    let (r, drill) = plan::via_size(&board);
    for p in &plan::via_spots(&board, false).at {
        let mut with = board.clone();
        let at = board::Point::new((p[0] * 1e6).round() as i64, (p[1] * 1e6).round() as i64);
        let id = with.insert(board::Item { kind: board::Kind::Via, net: 0, layers: 3, shape: board::Shape::Capsule { a: at, b: at, r }, hole: Some((at, drill)), locked: false, span: None });
        assert!(board::check_item(&with, id).is_empty(), "{p:?}");
    }
}
