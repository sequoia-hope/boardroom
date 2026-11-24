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

/// The same soldered pads on a board made with its vias filled and capped: the hole takes no
/// solder, and the shut-in pad leaves by one as a bare pad does.
#[test]
fn a_pad_that_takes_solder_is_given_a_hole_where_the_vias_are_filled() {
    let mut parts = Vec::new();
    for (i, x) in [7.7, 10.0, 12.3].into_iter().enumerate() {
        for (j, y) in [7.7, 10.0, 12.3].into_iter().enumerate() {
            let middle = (i, j) == (1, 1);
            parts.push(pad_on(&format!("P{i}{j}"), x, y, if middle { 1 } else { 0 }, if middle { "A" } else { "" }, "\"F.Cu\" \"F.Paste\" \"F.Mask\""));
        }
    }
    parts.push(pad("Q", 3.0, 16.0, 1, "A"));
    let mut board = board(20.0, &parts);
    // Two layers: not filled unless said.
    assert!(!board.filled_vias);
    board.filled_vias = true;
    let (plan, drawing) = wire::route(&board);
    assert_eq!(plan.vias.iter().filter(|v| within(&board, "P11", v.at)).count(), 1, "{:?}", plan.vias);
    assert_eq!(wire::open(&board, &plan, &drawing), 0);
    assert_eq!(wire::amiss(&board, &plan, &drawing), (0, 0));
}

/// Nine soldered pins 0.5 mm wide and 2 long, 0.5 apart side by side and 0.3 end to end: no
/// track passes between any two, and a via, 0.8 across, is wider than a pin. Where the vias
/// are filled the middle pin holds one all the same, its hole inside the pin and its ring
/// 0.15 past each side, 0.35 from the pins beside it; where they are not it is left open.
#[test]
fn a_pin_narrower_than_a_via_holds_one_where_the_vias_are_filled() {
    let pin = |reference: &str, x: f64, y: f64, n: usize, net: &str| {
        format!("(footprint \"P\" (layer \"F.Cu\") (at {x} {y}) (property \"Reference\" \"{reference}\") (pad \"1\" smd rect (at 0 0) (size 0.5 2) (layers \"F.Cu\" \"F.Paste\" \"F.Mask\") (net {n} \"{net}\")))")
    };
    let mut parts = Vec::new();
    for (i, x) in [9.0, 10.0, 11.0].into_iter().enumerate() {
        for (j, y) in [7.7, 10.0, 12.3].into_iter().enumerate() {
            let middle = (i, j) == (1, 1);
            parts.push(pin(&format!("P{i}{j}"), x, y, if middle { 1 } else { 0 }, if middle { "A" } else { "" }));
        }
    }
    parts.push(pad("Q", 3.0, 16.0, 1, "A"));
    let mut board = board(20.0, &parts);
    let (plan, drawing) = wire::route(&board);
    assert!(plan.vias.is_empty(), "{:?}", plan.vias);
    assert_eq!(wire::open(&board, &plan, &drawing), 1);
    board.filled_vias = true;
    let (plan, drawing) = wire::route(&board);
    assert_eq!(plan.vias.iter().filter(|v| within(&board, "P11", v.at)).count(), 1, "{:?}", plan.vias);
    assert_eq!(wire::open(&board, &plan, &drawing), 0);
    assert_eq!(wire::amiss(&board, &plan, &drawing), (0, 0));
}

/// On its own layer a pin with a via wider than itself is the two together: the map holds
/// the pin as reaching 0.15 mm farther, the via's ring past its side, and a track of another
/// net keeps the rules' 0.25 from the hole, 0.2 inside that ring, which is 0.05 from the ring.
#[test]
fn a_pin_with_a_via_wider_than_itself_is_wider_in_the_map() {
    let pin = "(footprint \"P\" (layer \"F.Cu\") (at 10 10) (property \"Reference\" \"P\") (pad \"1\" smd rect (at 0 0) (size 0.5 2) (layers \"F.Cu\" \"F.Paste\" \"F.Mask\") (net 1 \"A\")))".to_string();
    let mut board = board(20.0, &[pin]);
    board.filled_vias = true;
    let pad = board.items().find(|(_, i)| matches!(i.kind, Kind::Pad { .. })).map(|(id, _)| id).unwrap();
    let mut spots = plan::via_spots(&board, false);
    spots.holes = plan::holes(&board, &[pad], &[]);
    assert_eq!(spots.holes.len(), 1);
    let vias: Vec<_> = spots.holes.iter().map(|h| (h.1, h.2)).collect();
    let of_pad = |spots: plan::Spots, layer: usize| {
        let space = plan::Space::new(&board, layer, &vias, &std::sync::Arc::new(spots));
        space.obstacles.iter().find(|o| o.item == Some(pad)).map(|o| (o.past, o.hole))
    };
    let (past, hole) = of_pad(spots.clone(), 0).unwrap();
    assert!((past - 0.15).abs() < 1e-6 && (hole - 0.05).abs() < 1e-6, "{past} {hole}");
    // Without the via the pin is its own width.
    spots.holes.clear();
    assert_eq!(of_pad(spots, 0), Some((0.0, 0.0)));
}

/// Two pins side by side, 0.5 mm apart, walled in but for the slot between them, which opens
/// into a channel one track wide: one pin's track takes it and the other pin is left with no
/// way. Its via, wider than the pin, leaves the first pin's track too little room in the slot
/// (0.55 mm wanted of 0.5). That track is lifted for the via and then has no way itself, and
/// its pin is given a via too. (Before, the plan could not take the first via with the track
/// there and gave it up: on the owner's parallelogram board, 2026-11-24, eleven pads' vias
/// were found after the first plan and none was put in.)
#[test]
fn a_via_that_squeezes_a_neighbours_track_has_it_lifted() {
    let part = |reference: &str, x: f64, y: f64, w: f64, h: f64, n: usize, net: &str| {
        format!("(footprint \"P\" (layer \"F.Cu\") (at {x} {y}) (property \"Reference\" \"{reference}\") (pad \"1\" smd rect (at 0 0) (size {w} {h}) (layers \"F.Cu\" \"F.Paste\" \"F.Mask\") (net {n} \"{net}\")))")
    };
    let parts = vec![
        part("PA", 10.5, 10.0, 0.5, 2.0, 1, "A"),
        part("PB", 9.5, 10.0, 0.5, 2.0, 2, "B"),
        // Walls round the pins, too near for a track between: 0.36 mm to the left and to the
        // right (a via's ring reaches 0.15 past a pin's side), 0.3 below and above each pin,
        // the two above 0.7 apart over the slot.
        part("WL", 8.445, 9.85, 0.89, 2.3, 0, ""),
        part("WR", 11.555, 9.85, 0.89, 2.3, 0, ""),
        part("WD", 10.0, 11.75, 4.6, 0.9, 0, ""),
        part("WA", 11.175, 8.25, 1.65, 0.9, 0, ""),
        part("WB", 8.825, 8.25, 1.65, 0.9, 0, ""),
        pad("QA", 16.0, 3.0, 1, "A"),
        pad("QB", 4.0, 3.0, 2, "B"),
    ];
    let mut board = board(20.0, &parts);
    board.filled_vias = true;
    let (plan, drawing) = wire::route(&board);
    assert_eq!(wire::open(&board, &plan, &drawing), 0, "{:?} {:?}", plan.left, drawing.undrawn);
    assert_eq!(wire::amiss(&board, &plan, &drawing), (0, 0));
    assert!(plan.vias.iter().any(|v| within(&board, "PA", v.at)) || plan.vias.iter().any(|v| within(&board, "PB", v.at)), "{:?}", plan.vias);
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

/// Ground poured over the back, and a bare ground pad on the front shut in by its neighbours.
/// A poured net is left to its pours at first, and what they leave open is planned after
/// them: the pad's connection has no way then, and the pad is given its hole then, down into
/// the pour. (Before, pads were given holes after the first plan only, and this one stayed
/// open.)
#[test]
fn a_pad_the_pours_leave_open_with_no_way_is_given_a_hole() {
    let mut parts = Vec::new();
    for (i, x) in [7.7, 10.0, 12.3].into_iter().enumerate() {
        for (j, y) in [7.7, 10.0, 12.3].into_iter().enumerate() {
            let middle = (i, j) == (1, 1);
            parts.push(pad(&format!("P{i}{j}"), x, y, if middle { 1 } else { 0 }, if middle { "A" } else { "" }));
        }
    }
    let text = format!(
        "(kicad_pcb (version 20240108) (generator \"pcbnew\") (general (thickness 1.6))
         (layers (0 \"F.Cu\" signal) (31 \"B.Cu\" signal) (44 \"Edge.Cuts\" user))
         (net 0 \"\") (net 1 \"A\") (net 2 \"B\") (net 3 \"C\")
         (gr_rect (start 0 0) (end 20 20) (layer \"Edge.Cuts\") (width 0.1))
         {}
         (footprint \"P\" (layer \"F.Cu\") (at 3 16) (property \"Reference\" \"J\") (pad \"1\" thru_hole circle (at 0 0) (size 1.6 1.6) (drill 0.8) (layers \"*.Cu\" \"*.Mask\") (net 1 \"A\")))
         (zone (net 1) (net_name \"A\") (layer \"B.Cu\") (hatch edge 0.5) (connect_pads yes (clearance 0.2)) (min_thickness 0.25)
           (fill yes (thermal_gap 0.5) (thermal_bridge_width 0.5))
           (polygon (pts (xy 0.5 0.5) (xy 19.5 0.5) (xy 19.5 19.5) (xy 0.5 19.5)))))",
        parts.concat()
    );
    let (pcb, copper) = kicad::parse_board(&text).unwrap();
    let board = Board::new(&pcb, &copper, kicad::Rules::parse(RULES).unwrap());
    let (plan, drawing) = wire::route(&board);
    assert_eq!(wire::open(&board, &plan, &drawing), 0, "{:?} {:?}", plan.left, drawing.undrawn);
    assert_eq!(plan.vias.iter().filter(|v| within(&board, "P11", v.at)).count(), 1, "{:?}", plan.vias);
}
