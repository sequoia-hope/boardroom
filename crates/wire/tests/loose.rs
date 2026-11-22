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

/// A third pad of net A at (12, 4), and a wire from it that ends on the middle of the first
/// wire, which runs on from there to an end on nothing. The first wire is cut back to where
/// the other meets it, not taken out whole: it is what joins the two pads. (Minisumo of the
/// gate tier, 2026-11-21: a chip pad's short way to its via was taken out whole once the via
/// had gone, with the two wires that met it left ending in the air.)
#[test]
fn a_wire_with_an_end_on_nothing_is_cut_back_to_where_another_joins_it() {
    let pad = |x: f64, y: f64| format!("(footprint \"P\" (layer \"F.Cu\") (at {x} {y}) (pad \"1\" smd rect (at 0 0) (size 1 1) (layers \"F.Cu\") (net 1 \"A\")))");
    let text = format!(
        "(kicad_pcb (version 20240108) (generator \"pcbnew\") (general (thickness 1.6))
         (layers (0 \"F.Cu\" signal) (44 \"Edge.Cuts\" user))
         (net 0 \"\") (net 1 \"A\")
         (gr_rect (start 0 0) (end 24 20) (layer \"Edge.Cuts\") (width 0.1))
         {} {})",
        pad(3.0, 10.0),
        pad(12.0, 4.0)
    );
    let rules = r#"{"board":{"design_settings":{"rules":{"min_clearance":0.2,"min_copper_edge_clearance":0.3,"min_track_width":0.25}}},
        "net_settings":{"classes":[{"name":"Default","clearance":0.2,"track_width":1.0,"via_diameter":0.8,"via_drill":0.4}]}}"#;
    let (pcb, copper) = kicad::parse_board(&text).unwrap();
    let board = Board::new(&pcb, &copper, kicad::Rules::parse(rules).unwrap());
    let plan = plan::start(&board);
    let drawing = wire::Drawing { wires: vec![wire(vec![[3.0, 10.0], [15.0, 10.0], [18.0, 13.0]]), wire(vec![[12.0, 4.0], [12.0, 10.0]])], ..Default::default() };
    assert_eq!(wire::open(&board, &plan, &drawing), 0);
    let (plan, kept) = wire::loose::loose(&board, &plan, &drawing);
    assert_eq!(wire::open(&board, &plan, &kept), 0, "the two pads are still joined");
    assert_eq!(kept.wires.len(), 2);
    assert_eq!(kept.wires[0].pts, vec![[3.0, 10.0], [12.0, 10.0]], "cut back to where the other wire meets it");
    assert_eq!(kept.wires[1].pts, vec![[12.0, 4.0], [12.0, 10.0]]);
}

/// A pad on the front with a wire to a via, and on the back a wire from the via whose first
/// corner is still inside the via's copper and whose far end is on nothing. The via's own
/// copper is not something the back wire meets along its way: the wire goes whole, the via
/// with it, and then the front wire that led to it. (Five boards of the gate tier,
/// 2026-11-21, kept a via with a stump inside its ring and the track to it, joining nothing.)
#[test]
fn a_wire_from_a_via_to_nothing_leaves_no_stump_inside_the_via() {
    let text = "(kicad_pcb (version 20240108) (generator \"pcbnew\") (general (thickness 1.6))
         (layers (0 \"F.Cu\" signal) (31 \"B.Cu\" signal) (44 \"Edge.Cuts\" user))
         (net 0 \"\") (net 1 \"A\")
         (gr_rect (start 0 0) (end 24 20) (layer \"Edge.Cuts\") (width 0.1))
         (footprint \"P\" (layer \"F.Cu\") (at 3 10) (pad \"1\" smd rect (at 0 0) (size 1 1) (layers \"F.Cu\") (net 1 \"A\"))))";
    let rules = r#"{"board":{"design_settings":{"rules":{"min_clearance":0.2,"min_copper_edge_clearance":0.3,"min_track_width":0.25}}},
        "net_settings":{"classes":[{"name":"Default","clearance":0.2,"track_width":0.3,"via_diameter":0.8,"via_drill":0.4}]}}"#;
    let (pcb, copper) = kicad::parse_board(text).unwrap();
    let board = Board::new(&pcb, &copper, kicad::Rules::parse(rules).unwrap());
    let mut plan = plan::start(&board);
    plan.vias = vec![plan::Via { net: 1, site: 0, at: [12.0, 10.0] }];
    let thin = |layer: usize, pts: Vec<[f64; 2]>| wire::Wire { layer, strand: 0, net: 1, width: 0.3, pts, as_planned: false };
    let drawing = wire::Drawing { wires: vec![thin(0, vec![[3.0, 10.0], [12.0, 10.0]]), thin(1, vec![[12.0, 10.0], [12.2, 10.0], [18.0, 15.8]])], ..Default::default() };
    let (plan, kept) = wire::loose::loose(&board, &plan, &drawing);
    assert!(kept.wires.is_empty(), "{:?}", kept.wires.iter().map(|w| &w.pts).collect::<Vec<_>>());
    assert!(plan.vias.is_empty(), "{:?}", plan.vias);
}
