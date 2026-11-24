//! A fine-pitch chip's corner with other parts hard against its pins: lines from one end of a
//! 0.4 mm row of pins to a flash chip beside it, a resistor half a millimetre under two of
//! the pins, and the pads of an LED on the other side of the board opposite the pins' tips, so
//! that a via has few places to stand. The corner of parallelogram p2-132-usb6 where its
//! processor meets its flash (the owner's board, 2026-11-24), cut down to the least of it that
//! shows what the router did there.

use board::Board;

/// A pad: its part, its number, its side, where it is, its size, its turn, whether its corners
/// are rounded, its net ("X" for a net of its own).
type Pad = (&'static str, &'static str, char, f64, f64, f64, f64, f64, bool, &'static str);

/// The six flash lines, the resistor, a capacitor and a resistor below, and the LED opposite.
#[rustfmt::skip]
const FLASH: [Pad; 28] = [
    ("D93", "1", 'F', 4.525, 1.625, 0.45, 0.65, 0.0, false, "X"),
    ("D93", "2", 'F', 4.525, 2.575, 0.45, 0.65, 0.0, false, "X"),
    ("D93", "3", 'F', 5.775, 1.625, 0.45, 0.65, 0.0, false, "X"),
    ("D93", "4", 'F', 5.775, 2.575, 0.45, 0.65, 0.0, false, "X"),
    ("R15", "1", 'B', 5.9, 4.11, 0.54, 0.64, 90.0, true, "X"),
    ("R15", "2", 'B', 5.9, 3.09, 0.54, 0.64, 90.0, true, "X"),
    ("C13", "1", 'B', 4.9, 3.12, 0.56, 0.62, 270.0, true, "X"),
    ("C13", "2", 'B', 4.9, 4.08, 0.56, 0.62, 270.0, true, "GND"),
    ("R16", "1", 'B', 4.5407, 2.0963, 0.54, 0.64, 180.0, true, "SS"),
    ("R16", "2", 'B', 3.5207, 2.0963, 0.54, 0.64, 180.0, true, "X"),
    ("U4", "1", 'B', 5.255, 11.3, 0.75, 0.5, 90.0, true, "SS"),
    ("U4", "2", 'B', 3.985, 11.3, 0.75, 0.5, 90.0, true, "SD1"),
    ("U4", "3", 'B', 2.715, 11.3, 0.75, 0.5, 90.0, true, "SD2"),
    ("U4", "4", 'B', 1.445, 11.3, 0.75, 0.5, 90.0, true, "GND"),
    ("U4", "5", 'B', 1.445, 5.9, 0.75, 0.5, 90.0, true, "SD0"),
    ("U4", "6", 'B', 2.715, 5.9, 0.75, 0.5, 90.0, true, "SCLK"),
    ("U4", "7", 'B', 3.985, 5.9, 0.75, 0.5, 90.0, true, "SD3"),
    ("U4", "8", 'B', 5.255, 5.9, 0.75, 0.5, 90.0, true, "X"),
    ("U3", "1", 'B', 3.5625, 0.2, 0.875, 0.2, 0.0, false, "X"),
    ("U3", "52", 'B', 7.4, 0.8375, 0.2, 0.875, 0.0, true, "X"),
    ("U3", "53", 'B', 7.0, 0.8375, 0.2, 0.875, 0.0, true, "X"),
    ("U3", "54", 'B', 6.6, 0.8375, 0.2, 0.875, 0.0, true, "X"),
    ("U3", "55", 'B', 6.2, 0.8375, 0.2, 0.875, 0.0, true, "SD3"),
    ("U3", "56", 'B', 5.8, 0.8375, 0.2, 0.875, 0.0, true, "SCLK"),
    ("U3", "57", 'B', 5.4, 0.8375, 0.2, 0.875, 0.0, true, "SD0"),
    ("U3", "58", 'B', 5.0, 0.8375, 0.2, 0.875, 0.0, true, "SD2"),
    ("U3", "59", 'B', 4.6, 0.8375, 0.2, 0.875, 0.0, true, "SD1"),
    ("U3", "60", 'B', 4.2, 0.8375, 0.2, 0.875, 0.0, false, "SS"),
];

/// The pads as a board of four layers, the second a ground plane: tracks and clearances of
/// 0.1 mm, vias of 0.3 mm with a 0.15 mm hole that keeps 0.2 mm from other copper.
fn corner(pads: &[Pad]) -> Board {
    let mut nets: Vec<String> = vec![String::new(), "GND".to_string()];
    let mut parts: Vec<(&str, char, Vec<String>)> = Vec::new();
    for &(part, number, side, x, y, w, h, turn, round, net) in pads {
        let net = if net == "X" { format!("X{}", nets.len()) } else { net.to_string() };
        if !nets.contains(&net) {
            nets.push(net.clone());
        }
        let n = nets.iter().position(|x| *x == net).unwrap_or(0);
        let (shape, ratio) = if round { ("roundrect", " (roundrect_rratio 0.25)") } else { ("rect", "") };
        let pad = format!("(pad \"{number}\" smd {shape} (at {x} {y} {turn}) (size {w} {h}) (layers \"{side}.Cu\" \"{side}.Mask\" \"{side}.Paste\"){ratio} (net {n} \"{net}\"))");
        match parts.iter_mut().find(|p| p.0 == part && p.1 == side) {
            Some(p) => p.2.push(pad),
            None => parts.push((part, side, vec![pad])),
        }
    }
    let parts: String = parts.iter().map(|(part, side, pads)| format!("(footprint \"{part}\" (layer \"{side}.Cu\") (at 0 0) (property \"Reference\" \"{part}\") {})\n", pads.join(" "))).collect();
    let nets: String = nets.iter().enumerate().map(|(n, net)| format!("(net {n} \"{net}\") ")).collect();
    let text = format!(
        "(kicad_pcb (version 20240108) (generator \"pcbnew\") (general (thickness 1.6))
         (layers (0 \"F.Cu\" signal) (1 \"In1.Cu\" signal) (2 \"In2.Cu\" signal) (31 \"B.Cu\" signal) (36 \"B.Paste\" user) (37 \"F.Paste\" user) (38 \"B.Mask\" user) (39 \"F.Mask\" user) (44 \"Edge.Cuts\" user))
         (setup (pad_to_mask_clearance 0))
         {nets}
         (gr_rect (start 0 0) (end 8 12.9) (layer \"Edge.Cuts\") (width 0.05))
         {parts}
         (zone (net 1) (net_name \"GND\") (layer \"In1.Cu\") (hatch edge 0.5) (connect_pads (clearance 0.1)) (min_thickness 0.1)
           (fill yes (thermal_gap 0.2) (thermal_bridge_width 0.2))
           (polygon (pts (xy 0.1 0.1) (xy 7.9 0.1) (xy 7.9 12.8) (xy 0.1 12.8)))
           (filled_polygon (layer \"In1.Cu\") (pts (xy 0.3 0.3) (xy 7.7 0.3) (xy 7.7 12.6) (xy 0.3 12.6)))))"
    );
    let rules = r#"{"board":{"design_settings":{"rules":{"min_clearance":0.1,"min_copper_edge_clearance":0.2,"min_track_width":0.1,"min_hole_clearance":0.2,"min_hole_to_hole":0.254,"min_via_annular_width":0.05,"min_via_diameter":0.2,"min_through_hole_diameter":0.15}}},
        "net_settings":{"classes":[{"name":"Default","clearance":0.1,"track_width":0.1,"via_diameter":0.3,"via_drill":0.15}]}}"#;
    let (pcb, copper) = kicad::parse_board(&text).unwrap();
    Board::new(&pcb, &copper, kicad::Rules::parse(rules).unwrap())
}

/// The connections of every net that joins two pads or more, but ground: shortest first.
fn lines(board: &Board) -> Vec<plan::Want> {
    plan::wants(&board::connectivity_without_pours(board).airlines).into_iter().filter(|w| board.net_name(w.net) != "GND").collect()
}

/// What the plan left, by net.
fn left(board: &Board, plan: &plan::Plan) -> Vec<String> {
    plan.left.iter().map(|l| board.net_name(l.net).to_string()).collect()
}

/// The first line laid goes down past its neighbours' tips, between the resistor and the
/// capacitor below, and the gap there holds one track. A neighbour then has one way left that
/// the search can find, a via just past that track, and the plan cannot hold a via there: it
/// was left with no way, though moving the first line lets both through. All six are planned.
#[test]
fn a_pin_shut_in_by_its_neighbours_line_has_that_line_moved() {
    let board = corner(&FLASH);
    let mut plan = plan::start(&board);
    plan::add(&board, &mut plan, lines(&board));
    assert_eq!(plan.wants.len(), 7);
    assert!(plan.left.is_empty(), "left: {:?}", left(&board, &plan));
}

/// With each pin's way out settled first, a via at its tip: the LED's pads on the other side
/// stand opposite the tips of four of the pins, and no via may stand there. Each of those has
/// its via at the nearest place there is beside its tip instead; before, its track went
/// wherever a via could stand within reach, across the front of its neighbours, and one of
/// the six was left.
#[test]
fn a_pin_with_no_room_for_a_via_at_its_tip_has_one_as_near_as_may_be() {
    let board = corner(&FLASH);
    let wants = lines(&board);
    let (spots, stubs) = plan::escape(&board, &wants, plan::Fan::Tight);
    let mut plan = plan::start_from(&board, spots);
    plan::add(&board, &mut plan, stubs);
    plan::add(&board, &mut plan, wants);
    assert!(plan.left.is_empty(), "left: {:?}", left(&board, &plan));
}
