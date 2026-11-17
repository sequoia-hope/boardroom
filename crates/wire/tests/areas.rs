//! A rule area that bars some nets' tracks and lets others through (the RP2350's ring and band
//! on the pendant boards: `A.intersectsArea('U3 ring') && A.NetName != '+1V1'`) is a wall for
//! the nets it bars and open ground for the rest, in the plan, not only in the checker.

use board::{Board, Rule};

/// A 30 by 20 mm board of one layer. Net A's pads stand at (3, 10) and (27, 10), net B's at
/// (8, 10) and (22, 10); between them a rule area from x 12 to 18, y 4 to 16, that bars every
/// net but B. B goes straight through it; A must go round, above or below.
fn board() -> Board {
    board_with("(version 1)\n(rule \"ring\" (condition \"A.intersectsArea('ring') && A.NetName != 'B'\") (constraint disallow track))")
}

/// The same board with custom rules `dru`.
fn board_with(dru: &str) -> Board {
    let pad = |x: f64, n: u32, net: &str| format!("(footprint \"P\" (layer \"F.Cu\") (at {x} 10) (pad \"1\" smd rect (at 0 0) (size 1 1) (layers \"F.Cu\") (net {n} \"{net}\")))");
    let area = "(zone (net 0) (net_name \"\") (layer \"F.Cu\") (name \"ring\") (hatch edge 0.5)
                  (connect_pads (clearance 0)) (min_thickness 0.25)
                  (keepout (tracks allowed) (vias allowed) (pads allowed) (copperpour allowed) (footprints allowed))
                  (fill (thermal_gap 0.5) (thermal_bridge_width 0.5))
                  (polygon (pts (xy 12 4) (xy 18 4) (xy 18 16) (xy 12 16))))";
    let text = format!(
        "(kicad_pcb (version 20240108) (generator \"pcbnew\") (general (thickness 1.6))
         (layers (0 \"F.Cu\" signal) (44 \"Edge.Cuts\" user))
         (net 0 \"\") (net 1 \"A\") (net 2 \"B\")
         (gr_rect (start 0 0) (end 30 20) (layer \"Edge.Cuts\") (width 0.1))
         {} {} {} {} {area})",
        pad(3.0, 1, "A"),
        pad(27.0, 1, "A"),
        pad(8.0, 2, "B"),
        pad(22.0, 2, "B"),
    );
    let rules = r#"{"board":{"design_settings":{"rules":{"min_clearance":0.2,"min_copper_edge_clearance":0.3,"min_track_width":0.25}}},
        "net_settings":{"classes":[{"name":"Default","clearance":0.2,"track_width":0.25,"via_diameter":0.8,"via_drill":0.4}]}}"#;
    let (pcb, copper) = kicad::parse_board(&text).unwrap();
    let mut rules = kicad::Rules::parse(rules).unwrap();
    rules.apply_custom(dru, "test.kicad_dru");
    Board::new(&pcb, &copper, rules)
}

#[test]
fn the_board_says_which_nets_the_area_bars() {
    let board = board();
    let ring = board.areas.iter().position(|a| a.0 == "ring").unwrap();
    assert_eq!(board.nets_barred("track", ring, 0), vec![1], "A barred, B let through");
}

#[test]
fn a_net_the_area_bars_is_planned_round_it_and_one_it_spares_through_it() {
    let board = board();
    let mut plan = plan::start(&board);
    plan::add(&board, &mut plan, plan::wants(&board::connectivity_without_pours(&board).airlines));
    assert!(plan.left.is_empty(), "left: {:?}", plan.left);
    // Drawn at the first try: nothing the checker refuses for the area.
    let drawing = wire::draw(&board, &plan);
    assert!(drawing.undrawn.is_empty(), "not drawn: {:?}", drawing.undrawn.iter().map(|u| &u.why).collect::<Vec<_>>());
    let (routed, _) = wire::onto(&board, &plan, &drawing);
    let faults: Vec<_> = board::check(&routed).into_iter().filter(|v| v.rule == Rule::Keepout).collect();
    assert!(faults.is_empty(), "{faults:?}");
    assert_eq!(wire::open(&board, &plan, &drawing), 0);
    // B took the short way, through the area: about 14 mm, not round it.
    let b: f64 = routed.items().filter(|(_, i)| i.net == 2 && matches!(i.kind, board::Kind::Track)).map(|(_, i)| match &i.shape {
        board::Shape::Capsule { a, b, .. } => ((a.x - b.x) as f64).hypot((a.y - b.y) as f64) / 1e6,
        _ => 0.0,
    }).sum();
    assert!(b < 15.0, "B's track is {b:.2} mm");
}

#[test]
fn via_places_in_an_area_that_lets_one_nets_vias_through_are_kept() {
    // Vias of every net but B are barred in the area. Tested as no net, every place in it was
    // dropped for B too.
    let board = board_with("(version 1)\n(rule \"ring vias\" (condition \"A.intersectsArea('ring') && A.NetName != 'B'\") (constraint disallow via))");
    let spots = plan::via_spots(&board, false);
    let inside = spots.at.iter().filter(|p| p[0] > 12.5 && p[0] < 17.5 && p[1] > 4.5 && p[1] < 15.5).count();
    assert!(inside > 10, "{inside} places in the area");
}
