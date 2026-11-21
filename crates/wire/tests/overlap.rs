//! A pad drawn as shapes that overlap, with sides in common: the triangulation the plan's map
//! is made with stops on some such sides, and the map is made without them.

use board::Board;

/// A 30 by 20 mm board of two layers with a part whose two pads are each a rectangle and, over
/// its inner end, a filled outline drawn with a 0.1 mm pen, and a small through pad in that:
/// seven pieces of copper to a pad, their sides running along and ending on one another.
fn board() -> Board {
    let pad = |number: u32, net: &str, x: f64, hole: f64, poly: [f64; 2]| {
        format!(
            "(pad \"{number}\" smd custom (at {x} 0 90) (size 1.3 0.95) (layers \"F.Cu\") (net {number} \"{net}\")
               (options (clearance outline) (anchor rect))
               (primitives (gr_poly (pts (xy {a} 0.1786) (xy {b} 0.1786) (xy {b} -0.177) (xy {a} -0.177)) (width 0.1) (fill yes))))
             (pad \"{number}\" thru_hole circle (at {hole} 0) (size 0.4572 0.4572) (drill 0.3) (layers \"*.Cu\") (net {number} \"{net}\"))",
            a = poly[0],
            b = poly[1],
        )
    };
    let text = format!(
        "(kicad_pcb (version 20240108) (generator \"pcbnew\") (general (thickness 1.6))
         (layers (0 \"F.Cu\" signal) (31 \"B.Cu\" signal) (44 \"Edge.Cuts\" user))
         (net 0 \"\") (net 1 \"A\") (net 2 \"B\")
         (gr_rect (start 110 50) (end 140 70) (layer \"Edge.Cuts\") (width 0.1))
         (footprint \"D\" (layer \"F.Cu\") (at 126.308496 61.37575 90) {} {})
         (footprint \"P\" (layer \"F.Cu\") (at 115 55) (pad \"1\" smd rect (at 0 0) (size 1 1) (layers \"F.Cu\") (net 1 \"A\")))
         (footprint \"P\" (layer \"F.Cu\") (at 135 65) (pad \"1\" smd rect (at 0 0) (size 1 1) (layers \"F.Cu\") (net 2 \"B\"))))",
        pad(1, "A", -1.775, -0.6746, [0.521, 1.0942]),
        pad(2, "B", 1.775, 0.6746, [-0.521, -1.0942]),
    );
    let rules = r#"{"board":{"design_settings":{"rules":{"min_clearance":0.2,"min_copper_edge_clearance":0.3,"min_track_width":0.2}}},
        "net_settings":{"classes":[{"name":"Default","clearance":0.2,"track_width":0.25,"via_diameter":0.8,"via_drill":0.4}]}}"#;
    let (pcb, copper) = kicad::parse_board(&text).unwrap();
    Board::new(&pcb, &copper, kicad::Rules::parse(rules).unwrap())
}

#[test]
fn a_pad_of_overlapping_shapes_with_sides_in_common_is_planned_round() {
    let board = board();
    let plan = plan::plan(&board);
    assert!(plan.left.is_empty(), "left: {:?}", plan.left.iter().map(|l| l.connection).collect::<Vec<_>>());
    assert_eq!(plan.wants.len(), 2);
}
