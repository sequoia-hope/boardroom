//! Routing a board whose ground is poured: a pad the pour reaches needs no wire, and a pad on
//! the other side of the board is joined by a short wire to a via standing in the pour.

use board::Board;

const RULES: &str = r#"{"board":{"design_settings":{"rules":{"min_clearance":0.2,"min_copper_edge_clearance":0.3}}}}"#;

/// A 30 by 20 mm board of two layers with GND poured over all of the back, a signal between two
/// pads on the front, and `ground`: footprints with pads of GND.
fn board(ground: &str) -> Board {
    let text = format!(
        "(kicad_pcb (version 20240108) (generator \"pcbnew\") (general (thickness 1.6))
         (layers (0 \"F.Cu\" signal) (31 \"B.Cu\" signal) (44 \"Edge.Cuts\" user))
         (net 0 \"\") (net 1 \"GND\") (net 2 \"SIG\")
         (gr_rect (start 0 0) (end 30 20) (layer \"Edge.Cuts\") (width 0.1))
         (footprint \"P\" (layer \"F.Cu\") (at 10 5) (property \"Reference\" \"R1\") (pad \"1\" smd rect (at 0 0) (size 1 0.6) (layers \"F.Cu\") (net 2 \"SIG\")))
         (footprint \"P\" (layer \"F.Cu\") (at 20 5) (property \"Reference\" \"R2\") (pad \"1\" smd rect (at 0 0) (size 1 0.6) (layers \"F.Cu\") (net 2 \"SIG\")))
         {ground}
         (zone (net 1) (net_name \"GND\") (layer \"B.Cu\") (hatch edge 0.5) (connect_pads yes (clearance 0.2)) (min_thickness 0.25)
           (fill yes (thermal_gap 0.5) (thermal_bridge_width 0.5))
           (polygon (pts (xy 0.5 0.5) (xy 29.5 0.5) (xy 29.5 19.5) (xy 0.5 19.5)))))"
    );
    let (pcb, copper) = kicad::parse_board(&text).unwrap();
    Board::new(&pcb, &copper, kicad::Rules::parse(RULES).unwrap())
}

fn pin(reference: &str, x: f64, y: f64) -> String {
    format!("(footprint \"P\" (layer \"F.Cu\") (at {x} {y}) (property \"Reference\" \"{reference}\") (pad \"1\" thru_hole circle (at 0 0) (size 1.6 1.6) (drill 0.8) (layers \"*.Cu\" \"*.Mask\") (net 1 \"GND\")))")
}

fn smd(reference: &str, x: f64, y: f64) -> String {
    format!("(footprint \"P\" (layer \"F.Cu\") (at {x} {y}) (property \"Reference\" \"{reference}\") (pad \"1\" smd rect (at 0 0) (size 1 0.6) (layers \"F.Cu\") (net 1 \"GND\")))")
}

fn length(w: &wire::Wire) -> f64 {
    w.pts.windows(2).map(|p| (p[1][0] - p[0][0]).hypot(p[1][1] - p[0][1])).sum()
}

#[test]
fn pads_the_pour_reaches_need_no_wire() {
    let board = board(&[pin("J1", 5.0, 12.0), pin("J2", 25.0, 12.0)].concat());
    let (plan, drawing) = wire::route(&board);
    assert!(drawing.undrawn.is_empty() && plan.left.is_empty());
    assert!(drawing.wires.iter().all(|w| board.net_name(w.net) == "SIG"), "ground is left to its pour");
    assert_eq!(drawing.wires.len(), 1);
    assert!(plan.vias.is_empty());
    assert_eq!(wire::open(&board, &plan, &drawing), 0);
}

#[test]
fn a_pad_on_the_other_side_takes_a_via_into_the_pour() {
    let board = board(&[pin("J1", 5.0, 12.0), smd("C1", 25.0, 12.0)].concat());
    let (plan, drawing) = wire::route(&board);
    assert!(drawing.undrawn.is_empty() && plan.left.is_empty());
    assert_eq!(plan.vias.len(), 1, "one via, into the pour");
    assert_eq!(board.net_name(plan.vias[0].net), "GND");
    let ground: Vec<&wire::Wire> = drawing.wires.iter().filter(|w| board.net_name(w.net) == "GND").collect();
    assert_eq!(ground.len(), 1);
    assert!(length(ground[0]) < 5.0, "a short wire to the via, not one across the board: {} mm", length(ground[0]));
    assert_eq!(wire::open(&board, &plan, &drawing), 0);
}
