//! A net a custom rule allows no vias (`via_count (max 0)`: the pendant boards' crystal and
//! MCU-side USB nets) is never given one by the plan. (The search does not try one; were it
//! to, the checker would refuse it where it stands.)

use board::Board;

/// A 30 by 20 mm board of two layers. Net A's pads stand at (3, 10) and (27, 10) on the front;
/// between them a pad of net B runs the board's height on the front, so the only way for A is
/// under it on the back, by two vias. `dru` is the board's custom rules.
fn board(dru: &str) -> Board {
    board_classed(dru, "")
}

/// The same, with `assign` (KiCad's `netclass_assignments` body) putting nets in a POWER class
/// whose vias are 1.2 mm (the default's are 0.8).
fn board_classed(dru: &str, assign: &str) -> Board {
    let pad = |x: f64, h: f64, n: u32, net: &str| format!("(footprint \"P\" (layer \"F.Cu\") (at {x} 10) (pad \"1\" smd rect (at 0 0) (size 1 {h}) (layers \"F.Cu\") (net {n} \"{net}\")))");
    let text = format!(
        "(kicad_pcb (version 20240108) (generator \"pcbnew\") (general (thickness 1.6))
         (layers (0 \"F.Cu\" signal) (31 \"B.Cu\" signal) (44 \"Edge.Cuts\" user))
         (net 0 \"\") (net 1 \"A\") (net 2 \"B\")
         (gr_rect (start 0 0) (end 30 20) (layer \"Edge.Cuts\") (width 0.1))
         {} {} {})",
        pad(3.0, 1.0, 1, "A"),
        pad(27.0, 1.0, 1, "A"),
        pad(15.0, 19.0, 2, "B"),
    );
    let rules = r#"{"board":{"design_settings":{"rules":{"min_clearance":0.2,"min_copper_edge_clearance":0.3,"min_track_width":0.25}}},
        "net_settings":{"classes":[{"name":"Default","clearance":0.2,"track_width":0.25,"via_diameter":0.8,"via_drill":0.4},
            {"name":"POWER","clearance":0.2,"track_width":0.25,"via_diameter":1.2,"via_drill":0.6}],
            "netclass_assignments":ASSIGN}}"#.replace("ASSIGN", if assign.is_empty() { "{}" } else { assign });
    let (pcb, copper) = kicad::parse_board(&text).unwrap();
    let mut rules = kicad::Rules::parse(&rules).unwrap();
    if !dru.is_empty() {
        rules.apply_custom(dru, "test.kicad_dru");
    }
    Board::new(&pcb, &copper, rules)
}

fn plan(board: &Board) -> plan::Plan {
    let mut plan = plan::start(board);
    plan::add(board, &mut plan, plan::wants(&board::connectivity_without_pours(board).airlines));
    plan
}

#[test]
fn a_net_may_go_under_the_wall_by_vias() {
    let board = board("");
    let plan = plan(&board);
    assert!(plan.left.is_empty(), "left: {:?}", plan.left);
    assert_eq!(plan.vias.len(), 2);
}

#[test]
fn a_net_allowed_no_vias_is_given_none() {
    let board = board("(version 1)\n(rule \"A flat\" (condition \"A.NetName == 'A'\") (constraint via_count (max 0)))");
    assert!(board.no_vias(1));
    let plan = plan(&board);
    assert!(plan.vias.is_empty(), "vias: {:?}", plan.vias);
    assert_eq!(plan.left.len(), 1, "A has no way without one");
}

#[test]
fn a_via_and_wires_left_joining_nothing_are_taken_out() {
    // A goes under the wall by two vias. Were its last wire (front, from the second via to the
    // pad) not drawn, the second via would join the back alone, the back wire would lead to
    // it only, and so on back to the first pad: all of it is taken out, and nothing else.
    let board = board("");
    let plan = plan(&board);
    let mut drawing = wire::draw(&board, &plan);
    assert_eq!(plan.vias.len(), 2);
    assert!(drawing.undrawn.is_empty());
    let whole = wire::loose::loose(&board, &plan, &drawing);
    assert_eq!((whole.0.vias.len(), whole.1.wires.len()), (2, drawing.wires.len()), "a whole connection loses nothing");
    let last = drawing.wires.iter().position(|w| w.layer == 0 && w.pts.iter().any(|p| p[0] > 26.0)).expect("a front wire to the pad at x 27");
    drawing.wires.remove(last);
    let (plan, drawing) = wire::loose::loose(&board, &plan, &drawing);
    assert!(plan.vias.is_empty(), "vias: {:?}", plan.vias);
    assert!(drawing.wires.is_empty(), "wires: {:?}", drawing.wires.iter().map(|w| (w.layer, &w.pts)).collect::<Vec<_>>());
}

#[test]
fn a_via_has_the_boards_least_ring_round_its_hole() {
    // A class whose via leaves 0.075 mm round its hole (0.45 mm drilled 0.3 mm: old KiCad
    // files' own), on a board whose least ring is 0.1 mm: KiCad faults every such via
    // (annular_width), so the via is 0.5 mm across.
    let text = "(kicad_pcb (version 20240108) (generator \"pcbnew\") (general (thickness 1.6))
         (layers (0 \"F.Cu\" signal) (31 \"B.Cu\" signal) (44 \"Edge.Cuts\" user))
         (net 0 \"\") (net 1 \"A\")
         (gr_rect (start 0 0) (end 30 20) (layer \"Edge.Cuts\") (width 0.1)))";
    let rules = |ring: &str| {
        let rules = r#"{"board":{"design_settings":{"rules":{"min_clearance":0.2,"min_track_width":0.25,"min_via_diameter":0.45,"min_through_hole_diameter":0.3,"min_via_annular_width":RING}}},
            "net_settings":{"classes":[{"name":"Default","clearance":0.2,"track_width":0.25,"via_diameter":0.45,"via_drill":0.3}]}}"#.replace("RING", ring);
        let (pcb, copper) = kicad::parse_board(text).unwrap();
        Board::new(&pcb, &copper, kicad::Rules::parse(&rules).unwrap())
    };
    assert_eq!(plan::via_size(&rules("0.1"), 1), (250_000, 150_000));
    // A ring the class's via has already leaves it as it is.
    assert_eq!(plan::via_size(&rules("0.05"), 1), (225_000, 150_000));
}

#[test]
fn a_via_is_its_nets_class_size() {
    // A in the POWER class: its two vias are that class's, 1.2 mm, not the default's 0.8 mm.
    let board = board_classed("", r#"{"A":"POWER"}"#);
    assert_eq!(plan::via_size(&board, 1), (600_000, 300_000));
    assert_eq!(plan::via_size(&board, 0), (400_000, 200_000));
    let plan = plan(&board);
    assert_eq!(plan.vias.len(), 2);
    let drawing = wire::draw(&board, &plan);
    let (routed, _) = wire::onto(&board, &plan, &drawing);
    let sizes: Vec<i64> = routed.items().filter(|(_, i)| i.kind == board::Kind::Via).map(|(_, i)| match i.shape {
        board::Shape::Capsule { r, .. } => r,
        _ => 0,
    }).collect();
    assert_eq!(sizes, vec![600_000, 600_000]);
    assert!(board::check(&routed).is_empty(), "{:?}", board::check(&routed));
}
