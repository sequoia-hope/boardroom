//! An opening drawn in the solder mask over one net's copper is no place for another net's
//! track: the plan leads a wire round it.

use board::Board;

/// A pad 1 mm square on the front.
fn pad(x: f64, y: f64, n: usize, net: &str) -> String {
    format!("(footprint \"P\" (layer \"F.Cu\") (at {x} {y}) (pad \"1\" smd rect (at 0 0) (size 1 1) (layers \"F.Cu\" \"F.Mask\") (net {n} \"{net}\")))")
}

/// Net B's two pads either side of an opening in the front's mask that bares net A's pour (a
/// logo over the ground pour, as on EnvOpenPico between its chip and its debug pads: test set,
/// 2026-11-22). The first plan's strand goes round the opening and is drawn; before, it went
/// straight across for the drawing to refuse, and was laid again and again.
#[test]
fn a_wire_goes_round_an_opening_in_the_mask_over_another_nets_copper() {
    let parts = [pad(5.0, 10.0, 2, "B"), pad(15.0, 10.0, 2, "B"), pad(10.0, 17.0, 1, "A")].concat();
    let text = format!(
        "(kicad_pcb (version 20240108) (generator \"pcbnew\") (general (thickness 1.6))
         (layers (0 \"F.Cu\" signal) (31 \"B.Cu\" signal) (38 \"B.Mask\" user) (39 \"F.Mask\" user) (44 \"Edge.Cuts\" user))
         (setup (pad_to_mask_clearance 0))
         (net 0 \"\") (net 1 \"A\") (net 2 \"B\")
         (gr_rect (start 0 0) (end 20 20) (layer \"Edge.Cuts\") (width 0.1))
         (gr_rect (start 8 6) (end 12 14) (layer \"F.Mask\") (fill solid) (width 0))
         {parts}
         (zone (net 1) (net_name \"A\") (layer \"F.Cu\") (hatch edge 0.5) (connect_pads (clearance 0.2)) (min_thickness 0.25)
           (fill yes (thermal_gap 0.5) (thermal_bridge_width 0.5))
           (polygon (pts (xy 0.5 0.5) (xy 19.5 0.5) (xy 19.5 19.5) (xy 0.5 19.5)))))"
    );
    let rules = r#"{"board":{"design_settings":{"rules":{"min_clearance":0.2,"min_copper_edge_clearance":0.3,"min_track_width":0.2}}},
        "net_settings":{"classes":[{"name":"Default","clearance":0.2,"track_width":0.2,"via_diameter":0.8,"via_drill":0.4}]}}"#;
    let (pcb, copper) = kicad::parse_board(&text).unwrap();
    let board = Board::new(&pcb, &copper, kicad::Rules::parse(rules).unwrap());
    assert_eq!(board.mask_areas.len(), 1, "the drawn opening");
    let plan = plan::plan(&board);
    assert!(plan.left.is_empty(), "{:?}", plan.left);
    let drawing = wire::draw(&board, &plan);
    assert!(drawing.undrawn.is_empty(), "{:?}", drawing.undrawn.iter().map(|u| &u.why).collect::<Vec<_>>());
    assert_eq!(drawing.wires.len(), 1);
    // Round the opening on the front, not through it and not by the back.
    assert!(plan.vias.is_empty(), "{:?}", plan.vias);
    let inside = |p: [f64; 2]| p[0] > 8.0 && p[0] < 12.0 && p[1] > 6.0 && p[1] < 14.0;
    assert!(!drawing.wires[0].pts.iter().any(|&p| inside(p)), "{:?}", drawing.wires[0].pts);
}

/// A pin whose footprint draws its own opening in the mask, a filled box with a thin frame
/// round it (a USB-C socket's pins, DRM_Watch_v3, test set). The opening is the pin's own and
/// its wire leaves through it: taken for a wall, the frame shut every pin of that socket in
/// and left 11 connections open on a board that had none.
#[test]
fn a_pads_own_opening_drawn_in_the_mask_is_no_wall_to_its_wire() {
    let bare = |x: f64, y: f64| format!("(footprint \"P\" (layer \"F.Cu\") (at {x} {y}) (pad \"1\" smd rect (at 0 0) (size 1 1) (layers \"F.Cu\") (net 2 \"B\")))");
    let text = format!(
        "(kicad_pcb (version 20240108) (generator \"pcbnew\") (general (thickness 1.6))
         (layers (0 \"F.Cu\" signal) (31 \"B.Cu\" signal) (38 \"B.Mask\" user) (39 \"F.Mask\" user) (44 \"Edge.Cuts\" user))
         (setup (pad_to_mask_clearance 0))
         (net 0 \"\") (net 1 \"A\") (net 2 \"B\")
         (gr_rect (start 0 0) (end 20 20) (layer \"Edge.Cuts\") (width 0.1))
         (gr_rect (start 4.4 9.4) (end 5.6 10.6) (stroke (width 0.014) (type solid)) (fill yes) (layer \"F.Mask\"))
         {} {}
         (zone (net 1) (net_name \"A\") (layer \"F.Cu\") (hatch edge 0.5) (connect_pads (clearance 0.2)) (min_thickness 0.25)
           (fill yes (thermal_gap 0.5) (thermal_bridge_width 0.5))
           (polygon (pts (xy 0.5 0.5) (xy 19.5 0.5) (xy 19.5 19.5) (xy 0.5 19.5)))))",
        bare(5.0, 10.0),
        bare(15.0, 10.0)
    );
    let rules = r#"{"board":{"design_settings":{"rules":{"min_clearance":0.2,"min_copper_edge_clearance":0.3,"min_track_width":0.2}}},
        "net_settings":{"classes":[{"name":"Default","clearance":0.2,"track_width":0.2,"via_diameter":0.8,"via_drill":0.4}]}}"#;
    let (pcb, copper) = kicad::parse_board(&text).unwrap();
    let board = Board::new(&pcb, &copper, kicad::Rules::parse(rules).unwrap());
    assert!(board.mask_areas.iter().all(|a| a.2 == 2), "the opening bares net B's pad: {:?}", board.mask_areas.iter().map(|a| a.2).collect::<Vec<_>>());
    assert!(!board.mask_over_pour.iter().any(|&p| p));
    let plan = plan::plan(&board);
    assert!(plan.left.is_empty(), "{:?}", plan.left);
    let drawing = wire::draw(&board, &plan);
    assert!(drawing.undrawn.is_empty(), "{:?}", drawing.undrawn.iter().map(|u| &u.why).collect::<Vec<_>>());
    assert!(plan.vias.is_empty(), "{:?}", plan.vias);
}
