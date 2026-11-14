//! A board whose edge is not drawn as a closed loop still has room to route in: the least
//! convex shape round its edge's lines and everything on it.

use board::Board;

/// Two pads of net A either side of a locked track of net B, on a board whose edge is drawn
/// as `edge`.
fn board(edge: &str) -> Board {
    let text = format!(
        "(kicad_pcb (version 20240108) (generator \"pcbnew\") (general (thickness 1.6))
         (layers (0 \"F.Cu\" signal) (31 \"B.Cu\" signal) (44 \"Edge.Cuts\" user))
         (net 0 \"\") (net 1 \"A\") (net 2 \"B\")
         {edge}
         (footprint \"P\" (layer \"F.Cu\") (at 5 10) (pad \"1\" smd rect (at 0 0) (size 1 1) (layers \"F.Cu\") (net 1 \"A\")))
         (footprint \"P\" (layer \"F.Cu\") (at 15 10) (pad \"1\" smd rect (at 0 0) (size 1 1) (layers \"F.Cu\") (net 1 \"A\")))
         (segment (start 10 5) (end 10 15) (width 0.25) (layer \"F.Cu\") (net 2) (locked yes) (uuid \"5e000000-0000-4000-8000-000000000001\")))"
    );
    let rules = r#"{"board":{"design_settings":{"rules":{"min_clearance":0.2,"min_copper_edge_clearance":0.3,"min_track_width":0.2}}},
        "net_settings":{"classes":[{"name":"Default","clearance":0.2,"track_width":0.25,"via_diameter":0.8,"via_drill":0.4}]}}"#;
    let (pcb, copper) = kicad::parse_board(&text).unwrap();
    Board::new(&pcb, &copper, kicad::Rules::parse(rules).unwrap())
}

fn line(a: (f64, f64), b: (f64, f64)) -> String {
    format!("(gr_line (start {} {}) (end {} {}) (layer \"Edge.Cuts\") (width 0.1))", a.0, a.1, b.0, b.1)
}

/// The box round a board's bounds, mm: its least and greatest corners.
fn box_of(board: &Board) -> [f64; 4] {
    let bounds = plan::space::bounds(board);
    assert_eq!(bounds.len(), 1);
    let (xs, ys): (Vec<f64>, Vec<f64>) = bounds[0].iter().map(|p| (p.x as f64 / 1e6, p.y as f64 / 1e6)).unzip();
    let (least, most) = (|v: &[f64]| v.iter().copied().fold(f64::MAX, f64::min), |v: &[f64]| v.iter().copied().fold(f64::MIN, f64::max));
    [least(&xs), least(&ys), most(&xs), most(&ys)]
}

#[test]
fn a_board_whose_edge_does_not_close_is_routed_in_the_box_round_it() {
    // Three sides of a 20 mm square, and the fourth in two pieces that overlap and so are no
    // loop (LadyBugShield's edge).
    let edge = [line((0.0, 0.0), (0.0, 20.0)), line((0.0, 20.0), (1.0, 20.0)), line((0.5, 20.0), (20.0, 20.0)), line((20.0, 20.0), (20.0, 0.0)), line((20.0, 0.0), (0.0, 0.0))].concat();
    let board = board(&edge);
    assert!(board.outline.is_empty(), "the edge is no closed loop");
    assert_eq!(box_of(&board), [0.0, 0.0, 20.0, 20.0]);
    // The wire goes round the end of the track between its pads, or under it: outside the
    // pads and the track there would be no room for either.
    let (plan, drawing) = wire::route(&board);
    assert!(plan.left.is_empty(), "{:?}", plan.left);
    assert!(drawing.undrawn.is_empty());
    assert_eq!(wire::open(&board, &plan, &drawing), 0);
}

#[test]
fn a_board_with_no_edge_drawn_reaches_a_little_past_what_is_on_it() {
    let board = board("");
    // The pads' outer sides are at 4.5 and 15.5, the track's ends at 5 and 15 less its half
    // width; 2 mm more all round.
    let b = box_of(&board);
    assert!((b[0] - 2.5).abs() < 1e-6 && (b[2] - 17.5).abs() < 1e-6 && (b[1] - 2.875).abs() < 1e-6 && (b[3] - 17.125).abs() < 1e-6, "{b:?}");
    let (plan, drawing) = wire::route(&board);
    assert!(plan.left.is_empty() && drawing.undrawn.is_empty());
    assert_eq!(wire::open(&board, &plan, &drawing), 0);
}

#[test]
fn a_closed_edge_is_the_bounds() {
    let edge = "(gr_rect (start 0 0) (end 20 20) (layer \"Edge.Cuts\") (width 0.1))";
    assert_eq!(box_of(&board(edge)), [0.0, 0.0, 20.0, 20.0]);
    assert_eq!(plan::space::bounds(&board(edge)), board(edge).outline);
}

#[test]
fn an_edge_that_does_not_close_bounds_the_board_by_its_own_lines() {
    // A board with a corner cut off, its edge drawn with a gap in it: the bounds follow the
    // cut, not the box round it, so that no wire is planned where the checker will refuse it.
    let edge = [line((0.0, 0.0), (14.0, 0.0)), line((14.0, 0.0), (20.0, 6.0)), line((20.0, 6.0), (20.0, 20.0)), line((20.0, 20.0), (0.0, 20.0)), line((0.0, 19.0), (0.0, 0.0))].concat();
    let board = board(&edge);
    assert!(board.outline.is_empty());
    let bounds = plan::space::bounds(&board);
    let mut corners: Vec<(f64, f64)> = bounds[0].iter().map(|p| (p.x as f64 / 1e6, p.y as f64 / 1e6)).collect();
    corners.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(corners, [(0.0, 0.0), (0.0, 20.0), (14.0, 0.0), (20.0, 6.0), (20.0, 20.0)]);
}
