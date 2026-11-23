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

/// The same two columns three millimetres further apart, ground poured over the back as well
/// where `back`, and a pad joined to nothing on each diagonal west of the ground pin: where a
/// spoke drawn by hand would go there is no pour, and the pour the wires shut in between the
/// columns has room for a via.
fn fenced(back: bool) -> Board {
    let parts = [
        hole("J1", 10.0, 7.5, 2, "X"),
        hole("J2", 10.0, 10.0, 1, "GND"),
        hole("J3", 10.0, 12.5, 3, "Y"),
        smd("R1", 15.0, 8.2, 2, "X"),
        smd("R2", 15.0, 10.0, 4, "Z"),
        smd("R3", 15.0, 11.8, 3, "Y"),
        smd("R4", 5.0, 10.0, 1, "GND"),
        smd("B1", 8.5, 8.5, 0, ""),
        smd("B2", 8.5, 11.5, 0, ""),
    ]
    .concat();
    let zone = |layer: &str| {
        format!(
            "(zone (net 1) (net_name \"GND\") (layer \"{layer}\") (hatch edge 0.5) (connect_pads (clearance 0.2)) (min_thickness 0.25)
               (fill yes (thermal_gap 0.5) (thermal_bridge_width 0.5))
               (polygon (pts (xy 0.5 0.5) (xy 19.5 0.5) (xy 19.5 19.5) (xy 0.5 19.5))))"
        )
    };
    let text = format!(
        "(kicad_pcb (version 20240108) (generator \"pcbnew\") (general (thickness 1.6))
         (layers (0 \"F.Cu\" signal) (31 \"B.Cu\" signal) (44 \"Edge.Cuts\" user))
         (net 0 \"\") (net 1 \"GND\") (net 2 \"X\") (net 3 \"Y\") (net 4 \"Z\")
         (gr_rect (start 0 0) (end 20 20) (layer \"Edge.Cuts\") (width 0.1))
         {parts}
         {}
         {})",
        zone("F.Cu"),
        if back { zone("B.Cu") } else { String::new() }
    );
    let (pcb, copper) = kicad::parse_board(&text).unwrap();
    Board::new(&pcb, &copper, kicad::Rules::parse(RULES).unwrap())
}

#[test]
fn a_pour_shut_in_with_no_room_for_a_spoke_is_joined_to_the_other_layers_by_a_via() {
    let board = fenced(true);
    let before = board::Pours::likely(&board).starved(&board);
    assert!(before.is_empty(), "{before:?}");
    let (plan, drawing) = wire::route(&board);
    assert!(plan.left.is_empty() && drawing.undrawn.is_empty());
    assert_eq!(drawing.wires.len(), 2, "X and Y, ground left to its pour");
    assert!(drawing.ties.is_empty(), "{:?}", drawing.ties);
    // One via of ground between the columns, between the two wires.
    assert_eq!(drawing.stitches.len(), 1, "{:?}", drawing.stitches);
    let via = &drawing.stitches[0];
    assert_eq!(board.net_name(via.net), "GND");
    assert!(via.at[0] > 11.3 && via.at[0] < 14.3 && via.at[1] > 8.0 && via.at[1] < 12.0, "{:?}", via.at);
    assert_eq!(wire::amiss(&board, &plan, &drawing), (0, 0));
    // It is copper of the board written out.
    let (with, new) = wire::onto(&board, &plan, &drawing);
    assert_eq!(new.len(), drawing.wires.iter().map(|w| w.pts.len() - 1).sum::<usize>() + 1);
    assert!(new.iter().all(|&id| board::check_item(&with, id).is_empty()));
}

#[test]
fn no_via_is_put_where_no_other_layer_has_the_nets_pour() {
    // The same with ground poured on the front alone: a via between the columns would join
    // the pour there to nothing.
    let board = fenced(false);
    let (_, drawing) = wire::route(&board);
    assert!(drawing.stitches.is_empty(), "{:?}", drawing.stitches);
}

/// A pin with a hole, 1 mm square.
fn pin(reference: &str, x: f64, y: f64, n: usize, net: &str) -> String {
    format!("(footprint \"P\" (layer \"F.Cu\") (at {x} {y}) (property \"Reference\" \"{reference}\") (pad \"1\" thru_hole rect (at 0 0) (size 1 1) (drill 0.6) (layers \"*.Cu\" \"*.Mask\") (net {n} \"{net}\")))")
}

#[test]
fn a_wire_runs_over_a_pins_spokes_on_one_side_where_the_other_side_joins_it() {
    // A 1.27 mm header close under the board's edge, ground poured on both sides. X's pin is
    // between two ground pins with a row of other nets' pins below: its one way out is up,
    // and then over a ground pin's spokes, which reach too near the edge to pass beyond. On
    // one side of the board that is allowed: the pin keeps its two spokes on the other
    // (ESP-PROG-Adapter's J3, where the designer does just this on the back).
    let parts = [
        pin("G1", 8.73, 10.0, 1, "GND"),
        pin("S", 10.0, 10.0, 2, "X"),
        pin("G2", 11.27, 10.0, 1, "GND"),
        pin("P", 8.73, 11.27, 3, "P"),
        pin("Q", 10.0, 11.27, 4, "Q"),
        pin("R", 11.27, 11.27, 5, "R"),
        pin("T", 7.0, 13.0, 2, "X"),
        pin("G3", 15.0, 15.0, 1, "GND"),
    ]
    .concat();
    let zone = |layer: &str| {
        format!(
            "(zone (net 1) (net_name \"GND\") (layer \"{layer}\") (hatch edge 0.5) (connect_pads (clearance 0.2)) (min_thickness 0.25)
               (fill yes (thermal_gap 0.5) (thermal_bridge_width 0.5))
               (polygon (pts (xy 0.5 8.5) (xy 19.5 8.5) (xy 19.5 19.5) (xy 0.5 19.5))))"
        )
    };
    let text = format!(
        "(kicad_pcb (version 20240108) (generator \"pcbnew\") (general (thickness 1.6))
         (layers (0 \"F.Cu\" signal) (31 \"B.Cu\" signal) (44 \"Edge.Cuts\" user))
         (net 0 \"\") (net 1 \"GND\") (net 2 \"X\") (net 3 \"P\") (net 4 \"Q\") (net 5 \"R\")
         (gr_rect (start 0 8.3) (end 20 20) (layer \"Edge.Cuts\") (width 0.1))
         {parts}
         {}
         {})",
        zone("F.Cu"),
        zone("B.Cu")
    );
    let (pcb, copper) = kicad::parse_board(&text).unwrap();
    let board = Board::new(&pcb, &copper, kicad::Rules::parse(RULES).unwrap());
    let (plan, drawing) = wire::route(&board);
    assert!(plan.left.is_empty(), "{:?}", plan.left);
    assert!(drawing.undrawn.is_empty(), "{:?}", drawing.undrawn.iter().map(|u| &u.why).collect::<Vec<_>>());
    assert_eq!(wire::open(&board, &plan, &drawing), 0);
    assert_eq!(wire::amiss(&board, &plan, &drawing), (0, 0));
    assert!(drawing.wires.iter().all(|w| w.layer == 1), "X's wire is on the back, where no spoke is kept");
}

/// The ground pin of the three, with a pad of another net hard by its east side as well: on
/// the bare board it has the one spoke west, of the two the rules ask. KiCad's check passes
/// such a pad on the bare board where the stump of a cut spoke still crosses its gap, and
/// fails it once the wires have taken the pour the stump stood on (OpenVNAVI's driver board,
/// gate tier, 2026-11-22: the +5V end pin of each shift register, which its designer draws a
/// track to). It is drawn a spoke by hand all the same.
#[test]
fn a_pad_short_of_spokes_on_the_bare_board_is_drawn_one_by_hand_too() {
    let parts = [hole("J1", 10.0, 7.5, 0, ""), hole("J2", 10.0, 10.0, 1, "GND"), hole("J3", 10.0, 12.5, 0, ""), smd("R2", 12.0, 10.0, 0, ""), smd("R4", 5.0, 10.0, 1, "GND")].concat();
    let text = format!(
        "(kicad_pcb (version 20240108) (generator \"pcbnew\") (general (thickness 1.6))
         (layers (0 \"F.Cu\" signal) (31 \"B.Cu\" signal) (44 \"Edge.Cuts\" user))
         (net 0 \"\") (net 1 \"GND\")
         (gr_rect (start 0 0) (end 20 20) (layer \"Edge.Cuts\") (width 0.1))
         {parts}
         (zone (net 1) (net_name \"GND\") (layer \"F.Cu\") (hatch edge 0.5) (connect_pads (clearance 0.2)) (min_thickness 0.25)
           (fill yes (thermal_gap 0.5) (thermal_bridge_width 0.5))
           (polygon (pts (xy 0.5 0.5) (xy 19.5 0.5) (xy 19.5 19.5) (xy 0.5 19.5)))))"
    );
    let (pcb, copper) = kicad::parse_board(&text).unwrap();
    let board = Board::new(&pcb, &copper, kicad::Rules::parse(RULES).unwrap());
    let before = board::Pours::likely(&board).starved(&board);
    assert_eq!(before.len(), 1, "the pin has one spoke of two: {before:?}");
    let (plan, drawing) = wire::route(&board);
    assert_eq!(wire::amiss(&board, &plan, &drawing), (0, 0));
    assert_eq!(drawing.ties.len(), 1, "{:?}", drawing.ties);
    let tie = &drawing.ties[0];
    assert_eq!(tie.pts[0], [10.0, 10.0], "from the pin's middle");
    assert!(drawing.stitches.is_empty(), "{:?}", drawing.stitches);
}

/// The same pin with a patch that bars pours where each diagonal spoke would end: none of the
/// eight ways has pour at the end of a spoke as long as the pad's own. The spoke is drawn
/// farther, on to the pour past the patch (a USB-C socket's ground pins, sra_dev_board's J1,
/// test set, 2026-11-22: a mounting hole, the next pin and the shield's pad keep the pour a
/// little way off). And none is drawn hard beside the spoke the pin has, though there is pour
/// there to end in.
#[test]
fn a_spoke_drawn_by_hand_reaches_on_for_pour_and_is_not_drawn_beside_another() {
    let patch = |x: f64, y: f64| {
        format!(
            "(zone (net 0) (net_name \"\") (layer \"F.Cu\") (hatch edge 0.5) (connect_pads (clearance 0)) (min_thickness 0.25)
               (keepout (tracks allowed) (vias allowed) (pads allowed) (copperpour not_allowed) (footprints allowed))
               (fill (thermal_gap 0.5) (thermal_bridge_width 0.5))
               (polygon (pts (xy {} {}) (xy {} {}) (xy {} {}) (xy {} {}))))",
            x - 0.3, y - 0.3, x + 0.3, y - 0.3, x + 0.3, y + 0.3, x - 0.3, y + 0.3
        )
    };
    let parts = [hole("J1", 10.0, 7.5, 0, ""), hole("J2", 10.0, 10.0, 1, "GND"), hole("J3", 10.0, 12.5, 0, ""), smd("R2", 12.0, 10.0, 0, ""), smd("R4", 5.0, 10.0, 1, "GND")].concat();
    let patches = [patch(8.6, 8.6), patch(11.4, 8.6), patch(8.6, 11.4), patch(11.4, 11.4)].concat();
    let text = format!(
        "(kicad_pcb (version 20240108) (generator \"pcbnew\") (general (thickness 1.6))
         (layers (0 \"F.Cu\" signal) (31 \"B.Cu\" signal) (44 \"Edge.Cuts\" user))
         (net 0 \"\") (net 1 \"GND\")
         (gr_rect (start 0 0) (end 20 20) (layer \"Edge.Cuts\") (width 0.1))
         {parts}
         (zone (net 1) (net_name \"GND\") (layer \"F.Cu\") (hatch edge 0.5) (connect_pads (clearance 0.2)) (min_thickness 0.25)
           (fill yes (thermal_gap 0.5) (thermal_bridge_width 0.5))
           (polygon (pts (xy 0.5 0.5) (xy 19.5 0.5) (xy 19.5 19.5) (xy 0.5 19.5))))
         {patches})"
    );
    let (pcb, copper) = kicad::parse_board(&text).unwrap();
    let board = Board::new(&pcb, &copper, kicad::Rules::parse(RULES).unwrap());
    assert_eq!(board::Pours::likely(&board).starved(&board).len(), 1);
    let (plan, drawing) = wire::route(&board);
    assert_eq!(wire::amiss(&board, &plan, &drawing), (0, 0));
    assert_eq!(drawing.ties.len(), 1, "{:?}", drawing.ties);
    let [from, to] = drawing.ties[0].pts;
    assert_eq!(from, [10.0, 10.0], "from the pin's middle");
    let (dx, dy) = ((to[0] - from[0]).abs(), (to[1] - from[1]).abs());
    assert!((dx - dy).abs() < 1e-3 && dx > 1.7, "on a diagonal, past the patch: {to:?}");
}

/// A pad `w` by `h` mm on the front.
fn pad(reference: &str, x: f64, y: f64, w: f64, h: f64, n: usize, net: &str) -> String {
    format!("(footprint \"P\" (layer \"F.Cu\") (at {x} {y}) (property \"Reference\" \"{reference}\") (pad \"1\" smd rect (at 0 0) (size {w} {h}) (layers \"F.Cu\") (net {n} \"{net}\")))")
}

/// A two-layer board with ground poured over the front, joined to its pads by spokes, and
/// `parts` on it.
fn poured(parts: &str) -> Board {
    let text = format!(
        "(kicad_pcb (version 20240108) (generator \"pcbnew\") (general (thickness 1.6))
         (layers (0 \"F.Cu\" signal) (31 \"B.Cu\" signal) (44 \"Edge.Cuts\" user))
         (net 0 \"\") (net 1 \"GND\")
         (gr_rect (start 0 0) (end 20 20) (layer \"Edge.Cuts\") (width 0.1))
         {parts}
         (zone (net 1) (net_name \"GND\") (layer \"F.Cu\") (hatch edge 0.5) (connect_pads (clearance 0.2)) (min_thickness 0.25)
           (fill yes (thermal_gap 0.5) (thermal_bridge_width 0.5))
           (polygon (pts (xy 0.5 0.5) (xy 19.5 0.5) (xy 19.5 19.5) (xy 0.5 19.5)))))"
    );
    let (pcb, copper) = kicad::parse_board(&text).unwrap();
    Board::new(&pcb, &copper, kicad::Rules::parse(RULES).unwrap())
}

/// Two ground pads too near each other for pour between them, the lower walled in on its
/// other three sides: its one spoke is the one that meets the upper pad's, and KiCad's check
/// wants two. A track is drawn from its middle to the upper pad's, along the two spokes, as
/// the designer of TheTick draws one between R8 and the pad beside it (test set, 2026-11-22:
/// KiCad counts a track of the pad that ends in the pour, and the upper pad's spoke is pour
/// from its middle out).
#[test]
fn a_pad_joined_by_a_spoke_that_meets_anothers_is_drawn_a_track_to_that_pad() {
    let parts = [
        pad("U1", 10.0, 10.0, 1.0, 0.6, 1, "GND"),
        pad("U2", 10.0, 11.74, 1.0, 0.6, 1, "GND"),
        pad("W1", 8.7, 11.8, 1.0, 2.4, 0, ""),
        pad("W2", 11.3, 11.8, 1.0, 2.4, 0, ""),
        pad("W3", 10.0, 13.1, 3.6, 1.0, 0, ""),
    ]
    .concat();
    let board = poured(&parts);
    let before = board::Pours::likely(&board).starved(&board);
    assert_eq!(before.len(), 1, "the lower pad has the one spoke: {before:?}");
    let (plan, drawing) = wire::route(&board);
    assert_eq!(wire::amiss(&board, &plan, &drawing), (0, 0));
    assert_eq!(drawing.ties.len(), 1, "{:?}", drawing.ties);
    assert_eq!(drawing.ties[0].pts, [[10.0, 11.74], [10.0, 10.0]], "from the lower pad's middle to the upper's");
}

/// The ground pin with one spoke west, its other ways taken: pins above and below, a pad
/// hard by its east side, and a small pad where each diagonal spoke would end. No second way
/// to the pour is to be had, and it is not short for anything the wires did (so no via is put
/// in): a track is drawn beside the spoke it has, into the same pour, which is more copper on
/// the one way there is. hm-mod-rpi-rtc's designer runs a track from C3's pad along its one
/// spoke into the pour (test set, 2026-11-22), and KiCad's check counts it.
#[test]
fn a_pad_with_no_second_way_to_the_pour_is_drawn_a_track_beside_its_spoke() {
    let dot = |x: f64, y: f64| pad("D", x, y, 0.5, 0.5, 0, "");
    let parts = [
        hole("J1", 10.0, 7.5, 0, ""),
        hole("J2", 10.0, 10.0, 1, "GND"),
        hole("J3", 10.0, 12.5, 0, ""),
        smd("R2", 12.0, 10.0, 0, ""),
        smd("R4", 5.0, 10.0, 1, "GND"),
        dot(8.6, 8.6),
        dot(11.4, 8.6),
        dot(8.6, 11.4),
        dot(11.4, 11.4),
    ]
    .concat();
    let board = poured(&parts);
    assert_eq!(board::Pours::likely(&board).starved(&board).len(), 1);
    let (plan, drawing) = wire::route(&board);
    assert_eq!(wire::amiss(&board, &plan, &drawing), (0, 0));
    assert_eq!(drawing.ties.len(), 1, "{:?}", drawing.ties);
    let [from, to] = drawing.ties[0].pts;
    assert_eq!(from, [10.0, 10.0], "from the pin's middle");
    assert!(to[0] < 8.5 && (to[1] - 10.0).abs() < 1.0, "west, beside the pin's spoke: {to:?}");
    assert!(drawing.stitches.is_empty(), "{:?}", drawing.stitches);
}

/// A ground pad shut in a pen of other pads with a scrap of pour beside it, which its one
/// spoke ends in and which joins nothing else; its net reaches it by a wire. KiCad's check
/// counts the one spoke and wants two (TheTick's U2, test set, 2026-11-22). No wire can take
/// the scrap away, so a track of the pad is drawn into it: the scrap then joins two things,
/// and the check has its spoke and its track.
#[test]
fn a_pad_whose_one_spoke_ends_in_a_scrap_of_pour_is_drawn_a_track_into_the_scrap() {
    let parts = [
        pad("U2", 10.0, 10.0, 1.0, 0.6, 1, "GND"),
        pad("W1", 9.0, 8.5, 4.8, 1.0, 0, ""),
        pad("W2", 9.0, 11.5, 4.8, 1.0, 0, ""),
        pad("W3", 6.9, 10.0, 0.6, 2.0, 0, ""),
        pad("W4", 11.1, 10.0, 0.6, 2.0, 0, ""),
        smd("R4", 3.0, 10.0, 1, "GND"),
    ]
    .concat();
    let board = poured(&parts);
    let (plan, drawing) = wire::route(&board);
    assert_eq!(wire::open(&board, &plan, &drawing), 0);
    assert_eq!(drawing.ties.len(), 1, "{:?}", drawing.ties);
    let [from, to] = drawing.ties[0].pts;
    assert_eq!(from, [10.0, 10.0], "from the pad's middle");
    assert!(to[0] > 7.4 && to[0] < 9.0 && to[1] > 9.2 && to[1] < 10.8, "in the scrap: {to:?}");
}
