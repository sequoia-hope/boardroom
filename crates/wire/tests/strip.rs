//! The strip between a row of pads and a long edge beside it holds what its width holds: the
//! gaps there run from each pad's corner to the far end of the edge, each many times as long
//! as the strip is wide, and the plan must not take their length for their room.

use board::Board;

/// A 20 by 12 mm board of one layer with a row of four pads `strip` mm from its top edge.
/// The first and third are of net A; the second, of net B, reaches down to `strip` mm from
/// the bottom edge: from one A pad to the other the way is past an end of the long pad,
/// through one of the strips. The gaps over the long pad's top run from its corners to the
/// board's far corner, each many times as long as the strip is wide (a chip beside a
/// cut-out: TB6600StepperDriver). Tracks are 0.25 mm, 0.2 mm from other copper and 0.3 mm from the edge: a strip
/// holds one where it is 0.75 mm wide.
fn board(strip: f64) -> Board {
    let pad = |x: f64, y: f64, h: f64, n: u32, net: &str| format!("(footprint \"P\" (layer \"F.Cu\") (at {x} {y}) (pad \"1\" smd rect (at 0 0) (size 1 {h}) (layers \"F.Cu\") (net {n} \"{net}\")))");
    let text = format!(
        "(kicad_pcb (version 20240108) (generator \"pcbnew\") (general (thickness 1.6))
         (layers (0 \"F.Cu\" signal) (44 \"Edge.Cuts\" user))
         (net 0 \"\") (net 1 \"A\") (net 2 \"B\") (net 3 \"C\")
         (gr_rect (start 0 0) (end 20 12) (layer \"Edge.Cuts\") (width 0.1))
         {} {} {} {})",
        pad(3.0, strip + 0.8, 1.6, 1, "A"),
        pad(5.0, 6.0, 12.0 - 2.0 * strip, 2, "B"),
        pad(7.0, strip + 0.8, 1.6, 1, "A"),
        pad(10.0, strip + 0.8, 1.6, 3, "C")
    );
    let rules = r#"{"board":{"design_settings":{"rules":{"min_clearance":0.2,"min_copper_edge_clearance":0.3,"min_track_width":0.25}}},
        "net_settings":{"classes":[{"name":"Default","clearance":0.2,"track_width":0.25,"via_diameter":0.8,"via_drill":0.4}]}}"#;
    let (pcb, copper) = kicad::parse_board(&text).unwrap();
    Board::new(&pcb, &copper, kicad::Rules::parse(rules).unwrap())
}

/// The first plan of the board and its drawing: nothing laid again.
fn first(board: &Board) -> (plan::Plan, wire::Drawing) {
    let mut plan = plan::start(board);
    plan::add(board, &mut plan, plan::wants(&board::connectivity_without_pours(board).airlines));
    let drawing = wire::draw(board, &plan);
    (plan, drawing)
}

#[test]
fn a_strip_too_narrow_for_a_track_is_not_planned_through() {
    // 0.7 mm between each end of the long pad and the edge: 0.05 mm short of a track's room.
    // The plan says there is no way, and lays nothing that cannot be drawn.
    let board = board(0.7);
    let (plan, drawing) = first(&board);
    assert_eq!(plan.left.len(), 1, "left: {:?}", plan.left);
    assert!(drawing.wires.is_empty() && drawing.undrawn.is_empty(), "not drawn: {:?}", drawing.undrawn.iter().map(|u| &u.why).collect::<Vec<_>>());
}

#[test]
fn a_strip_wide_enough_is_gone_through() {
    let board = board(0.8);
    let (plan, drawing) = first(&board);
    assert!(plan.left.is_empty(), "left: {:?}", plan.left);
    assert!(drawing.undrawn.is_empty(), "not drawn: {:?}", drawing.undrawn.iter().map(|u| &u.why).collect::<Vec<_>>());
    assert_eq!(wire::open(&board, &plan, &drawing), 0);
}
