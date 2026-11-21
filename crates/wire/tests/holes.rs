//! A pad with no way out on its own layer leaves by a hole in itself: a via of its net inside
//! its copper, and its track on another layer, as a designer does with a grid of bare pads too
//! close for a track between them.

use board::{Board, Kind};

const RULES: &str = r#"{"board":{"design_settings":{"rules":{"min_clearance":0.2,"min_copper_edge_clearance":0.3,"min_track_width":0.2,"min_via_diameter":0.8,"min_through_hole_diameter":0.4}}},
    "net_settings":{"classes":[{"name":"Default","clearance":0.2,"track_width":0.2,"via_diameter":0.8,"via_drill":0.4}]}}"#;

/// A square pad 2 mm across on the front, bare: nothing is soldered to it.
fn pad(reference: &str, x: f64, y: f64, n: usize, net: &str) -> String {
    pad_on(reference, x, y, n, net, "\"F.Cu\" \"F.Mask\"")
}

/// The same pad on `layers`.
fn pad_on(reference: &str, x: f64, y: f64, n: usize, net: &str, layers: &str) -> String {
    format!("(footprint \"P\" (layer \"F.Cu\") (at {x} {y}) (property \"Reference\" \"{reference}\") (pad \"1\" smd rect (at 0 0) (size 2 2) (layers {layers}) (net {n} \"{net}\")))")
}

fn board(width: f64, parts: &[String]) -> Board {
    let text = format!(
        "(kicad_pcb (version 20240108) (generator \"pcbnew\") (general (thickness 1.6))
         (layers (0 \"F.Cu\" signal) (31 \"B.Cu\" signal) (44 \"Edge.Cuts\" user))
         (net 0 \"\") (net 1 \"A\") (net 2 \"B\") (net 3 \"C\")
         (gr_rect (start 0 0) (end {width} 20) (layer \"Edge.Cuts\") (width 0.1))
         {})",
        parts.concat()
    );
    let (pcb, copper) = kicad::parse_board(&text).unwrap();
    Board::new(&pcb, &copper, kicad::Rules::parse(RULES).unwrap())
}

/// The pad `reference` of the board, and whether `at` lies inside it.
fn within(board: &Board, reference: &str, at: [f64; 2]) -> bool {
    let id = board.items().find(|(_, i)| matches!(i.kind, Kind::Pad { footprint, .. } if board.footprints[footprint].reference == reference)).map(|(id, _)| id).unwrap();
    let b = board.bbox(id).unwrap();
    (b.min.x as f64 / 1e6..=b.max.x as f64 / 1e6).contains(&at[0]) && (b.min.y as f64 / 1e6..=b.max.y as f64 / 1e6).contains(&at[1])
}

/// Nine pads in a square, 0.3 mm apart: a track with its clearance needs 0.6. The middle one,
/// of net A, is shut in; its other pad is across the board.
#[test]
fn a_pad_shut_in_by_its_neighbours_leaves_by_a_hole_in_itself() {
    let mut parts = Vec::new();
    for (i, x) in [7.7, 10.0, 12.3].into_iter().enumerate() {
        for (j, y) in [7.7, 10.0, 12.3].into_iter().enumerate() {
            let middle = (i, j) == (1, 1);
            parts.push(pad(&format!("P{i}{j}"), x, y, if middle { 1 } else { 0 }, if middle { "A" } else { "" }));
        }
    }
    parts.push(pad("Q", 3.0, 16.0, 1, "A"));
    let board = board(20.0, &parts);
    let (plan, drawing) = wire::route(&board);
    assert!(plan.left.is_empty() && drawing.undrawn.is_empty(), "{:?} {:?}", plan.left, drawing.undrawn);
    assert_eq!(wire::open(&board, &plan, &drawing), 0);
    assert_eq!(wire::amiss(&board, &plan, &drawing), (0, 0));
    // A via inside the middle pad and the track on the back; the pad in the open has none in
    // it, its track comes up by a via beside it.
    assert_eq!(plan.vias.iter().filter(|v| within(&board, "P11", v.at)).count(), 1, "{:?}", plan.vias);
    assert!(!plan.vias.iter().any(|v| within(&board, "Q", v.at)), "{:?}", plan.vias);
    assert!(drawing.wires.iter().any(|w| w.layer == 1));
}

/// The same nine pads with solder paste on them: a part is soldered there, and a hole in the
/// pad would take its solder. No hole is made, and the connection is left open.
#[test]
fn a_pad_that_takes_solder_is_given_no_hole() {
    let mut parts = Vec::new();
    for (i, x) in [7.7, 10.0, 12.3].into_iter().enumerate() {
        for (j, y) in [7.7, 10.0, 12.3].into_iter().enumerate() {
            let middle = (i, j) == (1, 1);
            parts.push(pad_on(&format!("P{i}{j}"), x, y, if middle { 1 } else { 0 }, if middle { "A" } else { "" }, "\"F.Cu\" \"F.Paste\" \"F.Mask\""));
        }
    }
    parts.push(pad("Q", 3.0, 16.0, 1, "A"));
    let board = board(20.0, &parts);
    let (plan, drawing) = wire::route(&board);
    assert!(!plan.vias.iter().any(|v| within(&board, "P11", v.at)), "{:?}", plan.vias);
    assert_eq!(wire::open(&board, &plan, &drawing), 1);
}

/// Where no pad is shut in, no hole is made: the same pads with room between them.
#[test]
fn a_pad_with_a_way_out_is_given_no_hole() {
    let mut parts = Vec::new();
    for (i, x) in [6.0, 10.0, 14.0].into_iter().enumerate() {
        for (j, y) in [6.0, 10.0, 14.0].into_iter().enumerate() {
            let middle = (i, j) == (1, 1);
            parts.push(pad(&format!("P{i}{j}"), x, y, if middle { 1 } else { 0 }, if middle { "A" } else { "" }));
        }
    }
    parts.push(pad("Q", 3.0, 18.0, 1, "A"));
    let board = board(20.0, &parts);
    let (plan, drawing) = wire::route(&board);
    assert_eq!(wire::open(&board, &plan, &drawing), 0);
    assert!(plan.vias.is_empty(), "{:?}", plan.vias);
}

/// A column of three pads beside the board's edge, a wall of pads on their other side, the top
/// one hard under the top edge: the strip between the column and the edge holds one track. The
/// bottom pad leaves downwards; of the two above it one takes the strip, and the other, which
/// alone had that way too, is left with none. It is given a hole and the board planned again.
#[test]
fn a_pad_left_with_no_way_once_its_neighbours_are_laid_is_given_a_hole() {
    let mut parts = Vec::new();
    for (k, (y, net)) in [(1.5, "A"), (3.8, "B"), (6.1, "C")].into_iter().enumerate() {
        parts.push(pad(&format!("P{k}"), 17.3, y, k + 1, net));
        parts.push(pad(&format!("W{k}"), 15.0, y, 0, ""));
        parts.push(pad(&format!("Q{k}"), 3.0, 12.0 + 2.5 * k as f64, k + 1, net));
    }
    let board = board(19.2, &parts);
    let (plan, drawing) = wire::route(&board);
    assert!(plan.left.is_empty() && drawing.undrawn.is_empty(), "{:?} {:?}", plan.left, drawing.undrawn);
    assert_eq!(wire::open(&board, &plan, &drawing), 0);
    assert_eq!(wire::amiss(&board, &plan, &drawing), (0, 0));
    // One of the two upper pads has a hole in it; no pad across the board has.
    assert_eq!(plan.vias.iter().filter(|v| within(&board, "P0", v.at) || within(&board, "P1", v.at)).count(), 1, "{:?}", plan.vias);
    assert!(!plan.vias.iter().any(|v| ["Q0", "Q1", "Q2"].iter().any(|q| within(&board, q, v.at))), "{:?}", plan.vias);
}
