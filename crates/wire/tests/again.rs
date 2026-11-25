//! A board routed again comes out the same: nothing of a result may turn on the order a hash
//! map happens to give its entries in, which is another in every run.

use board::Board;

/// A 20 by 24 mm board of four layers, ground poured over all of In1 and power over all of
/// In2, with three surface-mount pads of each on the front: the power pads stand as the ground
/// pads do, 3 mm lower, so each connection of the one net is as long as one of the other.
fn two_planes() -> Board {
    let zone = |net: u32, name: &str, layer: &str| {
        format!(
            "(zone (net {net}) (net_name \"{name}\") (layer \"{layer}\") (hatch edge 0.5) (connect_pads (clearance 0.2)) (min_thickness 0.25)
               (fill yes (thermal_gap 0.5) (thermal_bridge_width 0.5))
               (polygon (pts (xy 0.5 0.5) (xy 19.5 0.5) (xy 19.5 23.5) (xy 0.5 23.5)))
               (filled_polygon (layer \"{layer}\") (pts (xy 0.5 0.5) (xy 19.5 0.5) (xy 19.5 23.5) (xy 0.5 23.5))))"
        )
    };
    let pad = |x: f64, y: f64, net: u32, name: &str| format!("(footprint \"P\" (layer \"F.Cu\") (at {x} {y}) (pad \"1\" smd rect (at 0 0) (size 1 0.6) (layers \"F.Cu\") (net {net} \"{name}\")))");
    let pads: String = [(4.0, 4.0), (16.0, 4.0), (10.0, 16.0)].iter().map(|&(x, y)| pad(x, y, 1, "GND") + &pad(x, y + 3.0, 2, "VCC")).collect();
    let text = format!(
        "(kicad_pcb (version 20240108) (generator \"pcbnew\") (general (thickness 1.6))
         (layers (0 \"F.Cu\" signal) (1 \"In1.Cu\" signal) (2 \"In2.Cu\" signal) (31 \"B.Cu\" signal) (44 \"Edge.Cuts\" user))
         (net 0 \"\") (net 1 \"GND\") (net 2 \"VCC\")
         (gr_rect (start 0 0) (end 20 24) (layer \"Edge.Cuts\") (width 0.1))
         {} {} {pads})",
        zone(1, "GND", "In1.Cu"),
        zone(2, "VCC", "In2.Cu")
    );
    let rules = r#"{"board":{"design_settings":{"rules":{"min_clearance":0.2,"min_copper_edge_clearance":0.3,"min_track_width":0.25}}},
        "net_settings":{"classes":[{"name":"Default","clearance":0.2,"track_width":0.25,"via_diameter":0.8,"via_drill":0.4}]}}"#;
    let (pcb, copper) = kicad::parse_board(&text).unwrap();
    Board::new(&pcb, &copper, kicad::Rules::parse(rules).unwrap())
}

/// What a routing came to, as text: every via and every wire, in the order they were made.
fn copper(plan: &plan::Plan, drawing: &wire::Drawing) -> String {
    let vias: Vec<String> = plan.vias.iter().map(|v| format!("via {} {:?}", v.net, v.at)).collect();
    let wires: Vec<String> = drawing.wires.iter().map(|w| format!("wire {} {} {:?}", w.net, w.layer, w.pts)).collect();
    [vias, wires].concat().join("\n")
}

/// Two nets each with a plane nothing reaches yet: each pad goes down to its plane by a via of
/// its own, and the nets' connections, as long as each other's, are laid in the nets' order
/// every time.
#[test]
fn two_nets_with_planes_nothing_reaches_are_routed_the_same_every_time() {
    let board = two_planes();
    let (plan, drawing) = wire::route(&board);
    assert_eq!(wire::open(&board, &plan, &drawing), 0, "left {:?}, not drawn {:?}", plan.left, drawing.undrawn);
    assert_eq!(plan.vias.len(), 6, "a via for each pad");
    let first = copper(&plan, &drawing);
    for again in 1..8 {
        let (plan, drawing) = wire::route(&board);
        assert_eq!(copper(&plan, &drawing), first, "routed again ({again}), the board came out another way");
    }
}
