//! A net a custom rule holds to a length is laid before the rest, while its straight way is
//! open (easy2350's crystal net ran 10.25 mm against its 10 mm rule).

use board::Board;

/// A 30 by 20 mm board of one layer, a wall across it at x 15 with one gap at y 10 that holds
/// one track (0.8 mm: a 0.25 mm track and 0.2 mm each side). Net A runs (3, 5) to (27, 15),
/// net B (13, 6) to (17, 14): both must go through the gap, B is the shorter, and there is no
/// other way for either.
fn board(dru: &str) -> Board {
    let pad = |x: f64, y: f64, w: f64, h: f64, n: u32, net: &str| format!("(footprint \"P\" (layer \"F.Cu\") (at {x} {y}) (pad \"1\" smd rect (at 0 0) (size {w} {h}) (layers \"F.Cu\") (net {n} \"{net}\")))");
    let text = format!(
        "(kicad_pcb (version 20240108) (generator \"pcbnew\") (general (thickness 1.6))
         (layers (0 \"F.Cu\" signal) (44 \"Edge.Cuts\" user))
         (net 0 \"\") (net 1 \"A\") (net 2 \"B\") (net 3 \"C\") (net 4 \"D\")
         (gr_rect (start 0 0) (end 30 20) (layer \"Edge.Cuts\") (width 0.1))
         {} {} {} {} {} {})",
        pad(3.0, 5.0, 1.0, 1.0, 1, "A"),
        pad(27.0, 15.0, 1.0, 1.0, 1, "A"),
        pad(13.0, 6.0, 1.0, 1.0, 2, "B"),
        pad(17.0, 14.0, 1.0, 1.0, 2, "B"),
        pad(15.0, 4.8, 1.0, 9.6, 3, "C"),
        pad(15.0, 15.2, 1.0, 9.6, 4, "D"),
    );
    let rules = r#"{"board":{"design_settings":{"rules":{"min_clearance":0.2,"min_copper_edge_clearance":0.0,"min_track_width":0.25}}},
        "net_settings":{"classes":[{"name":"Default","clearance":0.2,"track_width":0.25,"via_diameter":0.8,"via_drill":0.4}]}}"#;
    let (pcb, copper) = kicad::parse_board(&text).unwrap();
    let mut rules = kicad::Rules::parse(rules).unwrap();
    if !dru.is_empty() {
        rules.apply_custom(dru, "test.kicad_dru");
    }
    Board::new(&pcb, &copper, rules)
}

fn left_nets(board: &Board) -> Vec<u32> {
    let mut plan = plan::start(board);
    plan::add(board, &mut plan, plan::wants(&board::connectivity_without_pours(board).airlines));
    plan.left.iter().map(|l| plan.wants[l.connection].net).collect()
}

#[test]
fn the_shorter_connection_takes_the_gap_when_neither_has_a_length_rule() {
    assert_eq!(left_nets(&board("")), vec![1]);
}

#[test]
fn a_net_held_to_a_length_takes_the_gap_first() {
    let board = board("(version 1)\n(rule \"A short\" (condition \"A.NetName == 'A'\") (constraint length (max 40mm)))");
    assert!(board.net_maximum("length", 1).is_some());
    assert_eq!(left_nets(&board), vec![2]);
}
