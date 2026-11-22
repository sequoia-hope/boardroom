//! A track that leaves its own pad past something standing close beside that pad's side: the
//! gap between the pad and the thing is no measure of the room it has, for it comes out of the
//! pad and is not passing between the two.

use board::Board;

const RULES: &str = r#"{"board":{"design_settings":{"rules":{"min_clearance":0.2,"min_copper_edge_clearance":0.3,"min_track_width":0.2,"min_via_diameter":0.8,"min_through_hole_diameter":0.4}}},
    "net_settings":{"classes":[{"name":"Default","clearance":0.2,"track_width":0.2,"via_diameter":0.8,"via_drill":0.4}]}}"#;

/// A pad of `size` on the front that a part is soldered to (no hole is put in it).
fn pad(reference: &str, (x, y): (f64, f64), (w, h): (f64, f64), n: usize, net: &str) -> String {
    format!("(footprint \"P\" (layer \"F.Cu\") (at {x} {y}) (property \"Reference\" \"{reference}\") (pad \"1\" smd rect (at 0 0) (size {w} {h}) (layers \"F.Cu\" \"F.Paste\" \"F.Mask\") (net {n} \"{net}\")))")
}

fn board(parts: &[String]) -> Board {
    let text = format!(
        "(kicad_pcb (version 20240108) (generator \"pcbnew\") (general (thickness 1.6))
         (layers (0 \"F.Cu\" signal) (31 \"B.Cu\" signal) (44 \"Edge.Cuts\" user))
         (net 0 \"\") (net 1 \"A\")
         (gr_rect (start 0 0) (end 20 20) (layer \"Edge.Cuts\") (width 0.1))
         {})",
        parts.concat()
    );
    let (pcb, copper) = kicad::parse_board(&text).unwrap();
    Board::new(&pcb, &copper, kicad::Rules::parse(RULES).unwrap())
}

/// Net A's pad 2 mm square, fenced on three sides by pads 0.3 mm from it (a track with its
/// clearance needs 0.4), and on the fourth two small pads as near its side, 0.8 mm apart: room
/// between them for a track straight out of the side, as a designer draws it. The pad's other
/// pad is across the board. With `apart` less, there is no room between the two either.
fn fenced(apart: f64) -> Board {
    let mut parts = vec![pad("A1", (6.0, 10.0), (2.0, 2.0), 1, "A"), pad("A2", (14.0, 10.0), (2.0, 2.0), 1, "A")];
    for (k, at) in [(3.7, 7.7), (6.0, 7.7), (3.7, 10.0), (3.7, 12.3), (6.0, 12.3)].into_iter().enumerate() {
        parts.push(pad(&format!("F{k}"), at, (2.0, 2.0), 0, ""));
    }
    // The two beside the open side reach up and down past the fence's corners.
    let half = apart / 2.0;
    for (k, y) in [10.0 - half - 1.0, 10.0 + half + 1.0].into_iter().enumerate() {
        parts.push(pad(&format!("H{k}"), (7.6, y), (0.6, 2.0), 0, ""));
    }
    board(&parts)
}

#[test]
fn a_track_leaves_its_pad_past_what_stands_close_beside_the_pads_side() {
    let board = fenced(0.8);
    let (plan, drawing) = wire::route(&board);
    assert!(plan.left.is_empty() && drawing.undrawn.is_empty(), "{:?} {:?}", plan.left, drawing.undrawn);
    assert_eq!(wire::open(&board, &plan, &drawing), 0);
    assert_eq!(wire::amiss(&board, &plan, &drawing), (0, 0));
    assert!(plan.vias.is_empty(), "{:?}", plan.vias);
}

#[test]
fn but_not_where_there_is_no_room_between_them() {
    let board = fenced(0.5);
    let (plan, drawing) = wire::route(&board);
    assert_eq!(wire::open(&board, &plan, &drawing), 1);
}
