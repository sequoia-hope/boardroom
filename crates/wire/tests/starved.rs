//! A pad its pour joins by thermal spokes keeps as many as the rules ask once the board is
//! routed: a spoke into pour that the wires have shut in on every side is none.

use board::Board;

const RULES: &str = r#"{"board":{"design_settings":{"rules":{"min_clearance":0.2,"min_copper_edge_clearance":0.3}}}}"#;

fn hole(reference: &str, x: f64, y: f64, n: usize, net: &str) -> String {
    format!("(footprint \"P\" (layer \"F.Cu\") (at {x} {y}) (property \"Reference\" \"{reference}\") (pad \"1\" thru_hole rect (at 0 0) (size 1.6 1.6) (drill 1) (layers \"*.Cu\" \"*.Mask\") (net {n} \"{net}\")))")
}

fn smd(reference: &str, x: f64, y: f64, n: usize, net: &str) -> String {
    format!("(footprint \"P\" (layer \"F.Cu\") (at {x} {y}) (property \"Reference\" \"{reference}\") (pad \"1\" smd rect (at 0 0) (size 1 1.4) (layers \"F.Cu\") (net {n} \"{net}\")))")
}

/// A column of three pins, the middle one ground, beside a column of three pads, with ground
/// poured over the front and joined to its pads by spokes. The pins above and below the
/// ground pin take its spokes that way, so it has two: one west into the open pour, one east
/// into the pour between the columns. X is the top pin and the top pad, Y the bottom ones:
/// their wires run straight across and shut that pour in. Unless `wired`, the top and bottom
/// pads are joined to nothing.
fn board(wired: bool) -> Board {
    let (x, y) = if wired { ((2, "X"), (3, "Y")) } else { ((0, ""), (0, "")) };
    let parts = [
        hole("J1", 10.0, 7.5, 2, "X"),
        hole("J2", 10.0, 10.0, 1, "GND"),
        hole("J3", 10.0, 12.5, 3, "Y"),
        smd("R1", 13.0, 8.2, x.0, x.1),
        smd("R2", 13.0, 10.0, 4, "Z"),
        smd("R3", 13.0, 11.8, y.0, y.1),
        smd("R4", 5.0, 10.0, 1, "GND"),
    ]
    .concat();
    let text = format!(
        "(kicad_pcb (version 20240108) (generator \"pcbnew\") (general (thickness 1.6))
         (layers (0 \"F.Cu\" signal) (31 \"B.Cu\" signal) (44 \"Edge.Cuts\" user))
         (net 0 \"\") (net 1 \"GND\") (net 2 \"X\") (net 3 \"Y\") (net 4 \"Z\")
         (gr_rect (start 0 0) (end 20 20) (layer \"Edge.Cuts\") (width 0.1))
         {parts}
         (zone (net 1) (net_name \"GND\") (layer \"F.Cu\") (hatch edge 0.5) (connect_pads (clearance 0.2)) (min_thickness 0.25)
           (fill yes (thermal_gap 0.5) (thermal_bridge_width 0.5))
           (polygon (pts (xy 0.5 0.5) (xy 19.5 0.5) (xy 19.5 19.5) (xy 0.5 19.5)))))"
    );
    let (pcb, copper) = kicad::parse_board(&text).unwrap();
    Board::new(&pcb, &copper, kicad::Rules::parse(RULES).unwrap())
}

#[test]
fn a_pad_whose_spoke_is_shut_in_is_drawn_one_by_hand() {
    let board = board(true);
    let before = board::Pours::likely(&board).starved(&board);
    assert!(before.is_empty(), "{before:?}");
    let (plan, drawing) = wire::route(&board);
    assert!(plan.left.is_empty() && drawing.undrawn.is_empty());
    assert_eq!(drawing.wires.len(), 2, "X and Y, ground left to its pour");
    assert_eq!(wire::amiss(&board, &plan, &drawing), (0, 0));
    // One track of ground from the pin's middle, on the diagonal where it had no spoke, out
    // past its gap on the side of the open pour.
    assert_eq!(drawing.ties.len(), 1, "{:?}", drawing.ties);
    let tie = &drawing.ties[0];
    assert_eq!((board.net_name(tie.net), tie.layer, tie.pts[0]), ("GND", 0, [10.0, 10.0]));
    assert!(tie.pts[1][0] < 9.2 - 0.5 && (tie.pts[1][1] - 10.0).abs() > 0.8 + 0.5, "{:?}", tie.pts);
    // It is copper of the board written out.
    let (with, new) = wire::onto(&board, &plan, &drawing);
    assert_eq!(new.len(), drawing.wires.iter().map(|w| w.pts.len() - 1).sum::<usize>() + 1);
    assert!(new.iter().all(|&id| board::check_item(&with, id).is_empty()));
}

#[test]
fn a_pad_with_spokes_enough_is_drawn_none() {
    // The same board with the pads of the right column joined to nothing: no wire shuts the
    // pour in, and the pin keeps its two spokes.
    let board = board(false);
    let (plan, drawing) = wire::route(&board);
    assert!(drawing.wires.is_empty() && drawing.ties.is_empty());
    assert_eq!(wire::amiss(&board, &plan, &drawing), (0, 0));
}

#[test]
fn the_wires_that_shut_a_spoke_in_are_found_and_laid_another_way() {
    let board = board(true);
    // X and Y alone, planned and drawn, and nothing done for the pin.
    let lines = board::connectivity_without_pours(&board).airlines;
    let mut plan = plan::start(&board);
    plan::add(&board, &mut plan, plan::wants(&lines).into_iter().filter(|w| board.net_name(w.net) != "GND").collect());
    let drawing = wire::draw(&board, &plan);
    assert_eq!(wire::amiss(&board, &plan, &drawing), (0, 1));
    let (with, _) = wire::onto(&board, &plan, &drawing);
    let short = board::Pours::likely(&with).starved(&with);
    let bars = wire::shutting(&board, &plan, &drawing, &short);
    let mut nets: Vec<&str> = bars.iter().map(|b| board.net_name(plan.wants[b.0].net)).collect();
    nets.sort();
    assert_eq!(nets, ["X", "Y"], "the two wires round the pour the spoke ends in");
    // Barred from the gap it went through, each is laid again as wide as it was, and not
    // through that gap.
    plan::bar(&board, &mut plan, &bars);
    assert!(plan.left.is_empty());
    for &(c, gap) in &bars {
        assert_eq!((plan.wants[c].avoid.as_slice(), plan.wants[c].narrow), ([gap].as_slice(), None));
        let mut laid = 0;
        for sheet in &plan.sheets {
            for strand in sheet.laid().map(|k| &sheet.strands[k]).filter(|st| st.connection == c) {
                laid += 1;
                for &e in &strand.crossings {
                    let ends = sheet.space.edges[e].v.map(|v| sheet.space.verts[v]);
                    assert_ne!(plan::space::gap_key(ends[0], ends[1]), gap);
                }
            }
        }
        assert!(laid > 0);
    }
}
