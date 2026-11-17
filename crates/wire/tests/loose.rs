//! Copper taken out for joining nothing ([`wire::loose`]) is only ever copper KiCad would not
//! count as joined.

use board::Board;

/// A 24 by 20 mm board of one layer with net A's pads at (3, 10) and (21, 10).
fn board() -> Board {
    let pad = |x: f64| format!("(footprint \"P\" (layer \"F.Cu\") (at {x} 10) (pad \"1\" smd rect (at 0 0) (size 1 1) (layers \"F.Cu\") (net 1 \"A\")))");
    let text = format!(
        "(kicad_pcb (version 20240108) (generator \"pcbnew\") (general (thickness 1.6))
         (layers (0 \"F.Cu\" signal) (44 \"Edge.Cuts\" user))
         (net 0 \"\") (net 1 \"A\")
         (gr_rect (start 0 0) (end 24 20) (layer \"Edge.Cuts\") (width 0.1))
         {} {})",
        pad(3.0),
        pad(21.0)
    );
    let rules = r#"{"board":{"design_settings":{"rules":{"min_clearance":0.2,"min_copper_edge_clearance":0.3,"min_track_width":0.25}}},
        "net_settings":{"classes":[{"name":"Default","clearance":0.2,"track_width":1.0,"via_diameter":0.8,"via_drill":0.4}]}}"#;
    let (pcb, copper) = kicad::parse_board(&text).unwrap();
    Board::new(&pcb, &copper, kicad::Rules::parse(rules).unwrap())
}

fn wire(pts: Vec<[f64; 2]>) -> wire::Wire {
    wire::Wire { layer: 0, strand: 0, net: 1, width: 1.0, pts, as_planned: false }
}

#[test]
fn two_wide_wires_that_overlap_are_joined_though_neither_end_lies_in_the_other() {
    // The second wire ends 0.9 mm from the first's middle line: outside its 0.5 mm half width,
    // but the two 1 mm tracks overlap, and KiCad joins them (KiwiSDR's antenna board lost a
    // connection when this was taken for a free end).
    let board = board();
    let plan = plan::start(&board);
    let drawing = wire::Drawing { wires: vec![wire(vec![[3.0, 10.0], [12.0, 10.0]]), wire(vec![[21.0, 10.0], [10.0, 10.9]])], ..Default::default() };
    let (_, kept) = wire::loose::loose(&board, &plan, &drawing);
    assert_eq!(kept.wires.len(), 2);
}

#[test]
fn a_wire_with_an_end_on_nothing_is_taken_out() {
    let board = board();
    let plan = plan::start(&board);
    let drawing = wire::Drawing { wires: vec![wire(vec![[3.0, 10.0], [12.0, 10.0]]), wire(vec![[21.0, 10.0], [14.0, 10.0]])], ..Default::default() };
    let (_, kept) = wire::loose::loose(&board, &plan, &drawing);
    assert!(kept.wires.is_empty(), "{:?}", kept.wires.iter().map(|w| &w.pts).collect::<Vec<_>>());
}
