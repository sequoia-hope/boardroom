//! What a connection may end on and how wide it is: any pad already joined to its pad, a pad
//! that stands where another of its net does, and a track narrower than its class asks for
//! where that is what fits.

use board::Board;

/// A 20 by 20 mm board of two layers with `parts` on it, its tracks `width` mm by default and
/// no narrower than `least` mm.
fn board_with(parts: &str, width: f64, least: f64) -> Board {
    let text = format!(
        "(kicad_pcb (version 20240108) (generator \"pcbnew\") (general (thickness 1.6))
         (layers (0 \"F.Cu\" signal) (31 \"B.Cu\" signal) (44 \"Edge.Cuts\" user))
         (net 0 \"\") (net 1 \"A\") (net 2 \"B\") (net 3 \"C\")
         (gr_rect (start 0 0) (end 20 20) (layer \"Edge.Cuts\") (width 0.1))
         {parts})"
    );
    let rules = format!(
        r#"{{"board":{{"design_settings":{{"rules":{{"min_clearance":0.2,"min_copper_edge_clearance":0.3,"min_track_width":{least}}}}}}},
            "net_settings":{{"classes":[{{"name":"Default","clearance":0.2,"track_width":{width},"via_diameter":0.8,"via_drill":0.4}}]}}}}"#
    );
    let (pcb, copper) = kicad::parse_board(&text).unwrap();
    Board::new(&pcb, &copper, kicad::Rules::parse(&rules).unwrap())
}

/// [`board_with`], no track narrower than 0.2 mm.
fn board(parts: &str, width: f64) -> Board {
    board_with(parts, width, 0.2)
}

fn pad(x: f64, y: f64, w: f64, h: f64, net: &str) -> String {
    let n = ["", "A", "B", "C"].iter().position(|&k| k == net).unwrap();
    format!("(footprint \"P\" (layer \"F.Cu\") (at {x} {y}) (pad \"1\" smd rect (at 0 0) (size {w} {h}) (layers \"F.Cu\") (net {n} \"{net}\")))")
}

fn pin(x: f64, y: f64, net: &str) -> String {
    let n = ["", "A", "B", "C"].iter().position(|&k| k == net).unwrap();
    format!("(footprint \"P\" (layer \"F.Cu\") (at {x} {y}) (pad \"1\" thru_hole circle (at 0 0) (size 1.6 1.6) (drill 0.8) (layers \"*.Cu\" \"*.Mask\") (net {n} \"{net}\")))")
}

fn whole(board: &Board) -> (plan::Plan, wire::Drawing) {
    let (plan, drawing) = wire::route(board);
    assert!(plan.left.is_empty(), "left: {:?}", plan.left);
    assert!(drawing.undrawn.is_empty(), "not drawn: {:?}", drawing.undrawn.iter().map(|u| &u.why).collect::<Vec<_>>());
    assert_eq!(wire::open(board, &plan, &drawing), 0);
    (plan, drawing)
}

/// A pin made of two long thin pads with a wider one laid across their ends: the wide one is
/// all but covered by the other two, and what is left of its edge is the slot between them,
/// narrower than a track. The connection is to the wide pad, and is made to one of the thin
/// ones (a pogo-pin adapter's footprint, AVR-ISP_pogo-plug_1.27mm).
#[test]
fn a_pad_under_others_of_its_net_is_reached_by_one_of_them() {
    let pin_of_three = [pad(9.72, 5.0, 0.4, 3.0, "A"), pad(10.0, 5.996, 0.95, 1.0, "A"), pad(10.28, 5.0, 0.4, 3.0, "A")].concat();
    let board = board(&[pin_of_three, pad(10.0, 12.0, 1.5, 1.5, "A")].concat(), 0.25);
    let (_, drawing) = whole(&board);
    assert_eq!(drawing.wires.len(), 1);
}

/// A round pad on the end of a long one of its net, as a footprint that makes a slotted hole
/// of a row of round ones has them: the round one's corner is the long one's, so one of its
/// own sides runs between the other's corner and its own, and a track still ends there (a
/// barrel jack's pins, domotics_power-supply).
#[test]
fn a_pad_on_another_of_its_net_is_one_end_with_it() {
    let long = "(footprint \"P\" (layer \"F.Cu\") (at 5 10) (pad \"1\" smd oval (at 0 0) (size 4.2 2) (layers \"F.Cu\") (net 1 \"A\")))";
    let board = board(&[long.to_string(), pin(6.1, 10.0, "A"), pin(15.0, 10.0, "A")].concat(), 0.25);
    let (_, drawing) = whole(&board);
    assert_eq!(drawing.wires.len(), 1);
}

/// A pin 0.4 mm wide between two neighbours of other nets 0.65 mm away: a track 1 mm wide,
/// its class's width, cannot end on it and keep clear of them, and one half as wide can.
#[test]
fn a_track_is_narrower_than_its_class_asks_where_that_is_what_fits() {
    let row = [pad(9.35, 5.0, 0.4, 1.5, "B"), pad(10.0, 5.0, 0.4, 1.5, "A"), pad(10.65, 5.0, 0.4, 1.5, "C")].concat();
    let board = board(&[row, pad(10.0, 12.0, 1.5, 1.5, "A")].concat(), 1.0);
    let (plan, drawing) = whole(&board);
    assert_eq!(drawing.wires.len(), 1);
    assert!(drawing.wires[0].width < 1.0 && drawing.wires[0].width >= 0.2, "{} mm", drawing.wires[0].width);
    assert!(plan.wants.iter().all(|w| w.narrow.is_some()));
}

/// Where its class's width fits, that is the width.
#[test]
fn a_track_that_fits_keeps_its_class_width() {
    let board = board(&[pad(5.0, 10.0, 1.5, 1.5, "A"), pad(15.0, 10.0, 1.5, 1.5, "A")].concat(), 1.0);
    let (plan, drawing) = whole(&board);
    assert_eq!(drawing.wires[0].width, 1.0);
    assert!(plan.wants.iter().all(|w| w.narrow.is_none()));
}

/// A board whose rules give no least width: a connection whose wire cannot be drawn is made
/// narrower twice, to a quarter of what its class asks for, and after that is sent another
/// way instead.
#[test]
fn a_track_is_never_less_than_a_quarter_of_its_class_width() {
    let board = board_with(&[pad(5.0, 10.0, 1.5, 1.5, "A"), pad(15.0, 10.0, 1.5, 1.5, "A")].concat(), 1.0, 0.0);
    let mut plan = plan::plan(&board);
    assert_eq!(plan.wants.len(), 1);
    let gap = ((0, 0), (1, 1));
    let narrowed: Vec<bool> = (0..4).map(|_| plan::again(&board, &mut plan, &[(0, gap)])).collect();
    assert_eq!(narrowed, [true, true, false, false]);
    assert_eq!(plan.wants[0].narrow, Some(0.25));
    assert_eq!(plan.wants[0].avoid, [gap]);
}
