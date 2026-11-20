//! An inner layer one net mostly pours is solid (owner, 2026-11-16) unless the board says what
//! may cross it: a rule area on it that bars some nets' tracks says so, and opens it to the
//! rest.

use board::Board;

/// A 20 by 20 mm board of four layers with a ground pour over all of In1, and custom rules
/// `dru` (and the rule area `area`, when given, on In1).
fn board(dru: &str, area: &str) -> Board {
    let text = format!(
        "(kicad_pcb (version 20240108) (generator \"pcbnew\") (general (thickness 1.6))
         (layers (0 \"F.Cu\" signal) (1 \"In1.Cu\" signal) (2 \"In2.Cu\" signal) (31 \"B.Cu\" signal) (44 \"Edge.Cuts\" user))
         (net 0 \"\") (net 1 \"GND\") (net 2 \"A\")
         (gr_rect (start 0 0) (end 20 20) (layer \"Edge.Cuts\") (width 0.1))
         (zone (net 1) (net_name \"GND\") (layer \"In1.Cu\") (hatch edge 0.5) (connect_pads (clearance 0.2)) (min_thickness 0.25)
           (fill yes (thermal_gap 0.5) (thermal_bridge_width 0.5))
           (polygon (pts (xy 0.5 0.5) (xy 19.5 0.5) (xy 19.5 19.5) (xy 0.5 19.5)))
           (filled_polygon (layer \"In1.Cu\") (pts (xy 0.5 0.5) (xy 19.5 0.5) (xy 19.5 19.5) (xy 0.5 19.5))))
         {area})"
    );
    let rules = r#"{"board":{"design_settings":{"rules":{"min_clearance":0.2,"min_copper_edge_clearance":0.3,"min_track_width":0.25}}},
        "net_settings":{"classes":[{"name":"Default","clearance":0.2,"track_width":0.25,"via_diameter":0.8,"via_drill":0.4}]}}"#;
    let (pcb, copper) = kicad::parse_board(&text).unwrap();
    let mut rules = kicad::Rules::parse(rules).unwrap();
    if !dru.is_empty() {
        rules.apply_custom(dru, "test.kicad_dru");
    }
    Board::new(&pcb, &copper, rules)
}

#[test]
fn an_inner_plane_nothing_speaks_for_is_closed() {
    let plan = plan::start(&board("", ""));
    assert_eq!(plan.planes, vec![1]);
    assert_eq!(plan.closed, vec![1]);
}

#[test]
fn an_inner_plane_whose_rule_area_says_who_may_cross_is_open() {
    let area = "(zone (net 0) (net_name \"\") (layer \"In1.Cu\") (name \"In1 crossings\") (hatch edge 0.5)
                  (connect_pads (clearance 0)) (min_thickness 0.25)
                  (keepout (tracks allowed) (vias allowed) (pads allowed) (copperpour allowed) (footprints allowed))
                  (fill (thermal_gap 0.5) (thermal_bridge_width 0.5))
                  (polygon (pts (xy 0 0) (xy 20 0) (xy 20 20) (xy 0 20))))";
    let dru = "(version 1)\n(rule \"In1 only A\" (condition \"A.intersectsArea('In1 crossings') && A.NetName != 'A'\") (constraint disallow track))";
    let plan = plan::start(&board(dru, area));
    assert_eq!(plan.planes, vec![1]);
    assert!(plan.closed.is_empty(), "closed: {:?}", plan.closed);
}

fn length(w: &wire::Wire) -> f64 {
    w.pts.windows(2).map(|p| (p[1][0] - p[0][0]).hypot(p[1][1] - p[0][1])).sum()
}

/// Three surface-mount ground pads on the front, far apart, over a ground plane nothing
/// reaches: no through-hole pad, no via. Each goes down to the plane by a via of its own
/// beside it, as a designer fans a ground pad out, rather than by a track to the next pad.
#[test]
fn pads_over_a_plane_nothing_reaches_each_take_a_via_down_to_it() {
    let pad = |x: f64, y: f64| format!("(footprint \"P\" (layer \"F.Cu\") (at {x} {y}) (pad \"1\" smd rect (at 0 0) (size 1 0.6) (layers \"F.Cu\") (net 1 \"GND\")))");
    let board = board("", &[pad(4.0, 4.0), pad(16.0, 4.0), pad(10.0, 16.0)].concat());
    let (plan, drawing) = wire::route(&board);
    assert!(drawing.undrawn.is_empty() && plan.left.is_empty(), "left {:?}", plan.left);
    assert_eq!(wire::open(&board, &plan, &drawing), 0);
    assert_eq!(plan.vias.len(), 3, "a via for each pad");
    let longest = drawing.wires.iter().map(length).fold(0.0, f64::max);
    assert!(longest < 3.0, "short stubs to the vias, not tracks from pad to pad: the longest is {longest} mm");
    assert!(drawing.wires.iter().all(|w| w.layer == 0), "nothing is drawn on the plane");
}
