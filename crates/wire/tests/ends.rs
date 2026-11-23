//! What a connection may end on and how wide it is: any pad already joined to its pad, on the
//! bare board or by a strand laid before, a pad that stands where another of its net does, and
//! a track narrower than its class asks for where that is what fits.

use board::Board;

/// A 20 by 20 mm board of two layers with `parts` on it, its tracks `width` mm by default and
/// no narrower than `least` mm.
fn board_with(parts: &str, width: f64, least: f64) -> Board {
    let text = format!(
        "(kicad_pcb (version 20240108) (generator \"pcbnew\") (general (thickness 1.6))
         (layers (0 \"F.Cu\" signal) (31 \"B.Cu\" signal) (44 \"Edge.Cuts\" user))
         (net 0 \"\") (net 1 \"A\") (net 2 \"B\") (net 3 \"C\")
         (gr_rect (start 0 0) (end 20 20) (layer \"Edge.Cuts\") (width 0.1))
         {parts})"
    );
    let rules = format!(
        r#"{{"board":{{"design_settings":{{"rules":{{"min_clearance":0.2,"min_copper_edge_clearance":0.3,"min_track_width":{least}}}}}}},
            "net_settings":{{"classes":[{{"name":"Default","clearance":0.2,"track_width":{width},"via_diameter":0.8,"via_drill":0.4}}]}}}}"#
    );
    let (pcb, copper) = kicad::parse_board(&text).unwrap();
    Board::new(&pcb, &copper, kicad::Rules::parse(&rules).unwrap())
}

/// [`board_with`], no track narrower than 0.2 mm.
fn board(parts: &str, width: f64) -> Board {
    board_with(parts, width, 0.2)
}

fn pad(x: f64, y: f64, w: f64, h: f64, net: &str) -> String {
    let n = ["", "A", "B", "C"].iter().position(|&k| k == net).unwrap();
    format!("(footprint \"P\" (layer \"F.Cu\") (at {x} {y}) (pad \"1\" smd rect (at 0 0) (size {w} {h}) (layers \"F.Cu\") (net {n} \"{net}\")))")
}

fn pin(x: f64, y: f64, net: &str) -> String {
    let n = ["", "A", "B", "C"].iter().position(|&k| k == net).unwrap();
    format!("(footprint \"P\" (layer \"F.Cu\") (at {x} {y}) (pad \"1\" thru_hole circle (at 0 0) (size 1.6 1.6) (drill 0.8) (layers \"*.Cu\" \"*.Mask\") (net {n} \"{net}\")))")
}

/// A square pin 2 mm across with a hole in it, on both layers.
fn post(x: f64, y: f64, net: &str) -> String {
    let n = ["", "A", "B", "C"].iter().position(|&k| k == net).unwrap();
    format!("(footprint \"P\" (layer \"F.Cu\") (at {x} {y}) (pad \"1\" thru_hole rect (at 0 0) (size 2 2) (drill 0.8) (layers \"*.Cu\" \"*.Mask\") (net {n} \"{net}\")))")
}

fn whole(board: &Board) -> (plan::Plan, wire::Drawing) {
    let (plan, drawing) = wire::route(board);
    assert!(plan.left.is_empty(), "left: {:?}", plan.left);
    assert!(drawing.undrawn.is_empty(), "not drawn: {:?}", drawing.undrawn.iter().map(|u| &u.why).collect::<Vec<_>>());
    assert_eq!(wire::open(board, &plan, &drawing), 0);
    (plan, drawing)
}

/// A pin made of two long thin pads with a wider one laid across their ends: the wide one is
/// all but covered by the other two, and what is left of its edge is the slot between them,
/// narrower than a track. The connection is to the wide pad, and is made to one of the thin
/// ones (a pogo-pin adapter's footprint, AVR-ISP_pogo-plug_1.27mm).
#[test]
fn a_pad_under_others_of_its_net_is_reached_by_one_of_them() {
    let pin_of_three = [pad(9.72, 5.0, 0.4, 3.0, "A"), pad(10.0, 5.996, 0.95, 1.0, "A"), pad(10.28, 5.0, 0.4, 3.0, "A")].concat();
    let board = board(&[pin_of_three, pad(10.0, 12.0, 1.5, 1.5, "A")].concat(), 0.25);
    let (_, drawing) = whole(&board);
    assert_eq!(drawing.wires.len(), 1);
}

/// A round pad on the end of a long one of its net, as a footprint that makes a slotted hole
/// of a row of round ones has them: the round one's corner is the long one's, so one of its
/// own sides runs between the other's corner and its own, and a track still ends there (a
/// barrel jack's pins, domotics_power-supply).
#[test]
fn a_pad_on_another_of_its_net_is_one_end_with_it() {
    let long = "(footprint \"P\" (layer \"F.Cu\") (at 5 10) (pad \"1\" smd oval (at 0 0) (size 4.2 2) (layers \"F.Cu\") (net 1 \"A\")))";
    let board = board(&[long.to_string(), pin(6.1, 10.0, "A"), pin(15.0, 10.0, "A")].concat(), 0.25);
    let (_, drawing) = whole(&board);
    assert_eq!(drawing.wires.len(), 1);
}

/// A pin 0.4 mm wide between two neighbours of other nets 0.65 mm away: a track 1 mm wide,
/// its class's width, cannot end on it and keep clear of them, and one half as wide can.
#[test]
fn a_track_is_narrower_than_its_class_asks_where_that_is_what_fits() {
    let row = [pad(9.35, 5.0, 0.4, 1.5, "B"), pad(10.0, 5.0, 0.4, 1.5, "A"), pad(10.65, 5.0, 0.4, 1.5, "C")].concat();
    let board = board(&[row, pad(10.0, 12.0, 1.5, 1.5, "A")].concat(), 1.0);
    let (plan, drawing) = whole(&board);
    assert_eq!(drawing.wires.len(), 1);
    assert!(drawing.wires[0].width < 1.0 && drawing.wires[0].width >= 0.2, "{} mm", drawing.wires[0].width);
    assert!(plan.wants.iter().all(|w| w.narrow.is_some()));
}

/// Where its class's width fits, that is the width.
#[test]
fn a_track_that_fits_keeps_its_class_width() {
    let board = board(&[pad(5.0, 10.0, 1.5, 1.5, "A"), pad(15.0, 10.0, 1.5, 1.5, "A")].concat(), 1.0);
    let (plan, drawing) = whole(&board);
    assert_eq!(drawing.wires[0].width, 1.0);
    assert!(plan.wants.iter().all(|w| w.narrow.is_none()));
}

/// A board whose rules give no least width: a connection whose wire cannot be drawn is made
/// narrower twice, to a quarter of what its class asks for, and after that is sent another
/// way instead.
#[test]
fn a_track_is_never_less_than_a_quarter_of_its_class_width() {
    let board = board_with(&[pad(5.0, 10.0, 1.5, 1.5, "A"), pad(15.0, 10.0, 1.5, 1.5, "A")].concat(), 1.0, 0.0);
    let mut plan = plan::plan(&board);
    assert_eq!(plan.wants.len(), 1);
    let gap = ((0, 0), (1, 1));
    let narrowed: Vec<bool> = (0..4).map(|_| plan::again(&board, &mut plan, &[(0, gap)])).collect();
    assert_eq!(narrowed, [true, true, false, false]);
    assert_eq!(plan.wants[0].narrow, Some(0.25));
    assert_eq!(plan.wants[0].avoid, [gap]);
}

/// A rule area over the top of a long pad, its middle included, where no track may be: the
/// wire stops where it comes onto the pad, which joins it as well as the middle would (a
/// pogo-pin plug's pins under their mask openings, a pad on the board's edge).
#[test]
fn a_wire_that_may_not_reach_its_pads_middle_stops_where_it_comes_onto_the_pad() {
    let long = pad(10.0, 10.0, 1.0, 6.0, "A");
    let area = "(zone (net 0) (net_name \"\") (layers \"F.Cu\") (hatch edge 0.5)
                  (keepout (tracks not_allowed) (vias not_allowed) (pads allowed) (copperpour allowed) (footprints allowed))
                  (polygon (pts (xy 8 6) (xy 12 6) (xy 12 11) (xy 8 11))))";
    let board = board(&[long.clone(), pad(10.0, 17.0, 1.0, 1.0, "A"), area.to_string()].concat(), 0.25);
    let (_, drawing) = whole(&board);
    assert_eq!(drawing.wires.len(), 1);
    let ends = [drawing.wires[0].pts[0], *drawing.wires[0].pts.last().unwrap()];
    let top = ends.iter().map(|p| p[1]).fold(f64::MAX, f64::min);
    assert!((top - 12.95).abs() < 1e-3, "just onto the pad's lower end, at 13: {ends:?}");
    // With no such area it goes to the middle.
    let board = self::board(&[long, pad(10.0, 17.0, 1.0, 1.0, "A")].concat(), 0.25);
    let (_, drawing) = whole(&board);
    let top = drawing.wires[0].pts.iter().map(|p| p[1]).fold(f64::MAX, f64::min);
    assert!((top - 10.0).abs() < 1e-6, "{:?}", drawing.wires[0].pts);
}

/// A wall of other nets' pads across the board with a door 0.75 mm wide, a pad on one side and two
/// of its net on the other: the door has room for one track, not for two side by side. The
/// two connections go through it as one track and part beyond it, as a designer branches a
/// track (a connector's pin that goes to two places, CAL430FR's P2).
#[test]
fn two_connections_of_one_net_go_through_a_narrow_gap_as_one_track() {
    let wall = [pad(10.0, 4.8125, 0.6, 9.625, "B"), pad(10.0, 15.1875, 0.6, 9.625, "C")].concat();
    let board = board(&[wall, pad(5.0, 10.0, 1.0, 1.0, "A"), pad(13.0, 2.0, 1.0, 1.0, "A"), pad(13.0, 18.0, 1.0, 1.0, "A")].concat(), 0.25);
    let (plan, drawing) = whole(&board);
    assert_eq!(drawing.wires.len(), 2);
    assert!(plan.vias.is_empty(), "neither goes under the wall");
    assert!(plan.wants.iter().all(|w| w.narrow.is_none()), "both at their class's width");
    // What the two share through the door is drawn once ([`wire::spare`]): one wire goes
    // through, the other starts on it beyond the wall (whose far side is at 10.3).
    let through = |w: &wire::Wire| w.pts.windows(2).any(|s| (s[0][0] - 10.0) * (s[1][0] - 10.0) <= 0.0);
    assert_eq!(drawing.wires.iter().filter(|w| through(w)).count(), 1, "{:?}", drawing.wires);
    for wire in drawing.wires.iter().filter(|w| !through(w)) {
        assert!(wire.pts.iter().all(|p| p[0] > 10.3), "beyond the wall: {:?}", wire.pts);
    }
    assert!(!drawing.spare.is_empty());
    assert!(board::connectivity_without_pours(&wire::onto(&board, &plan, &drawing).0).airlines.is_empty(), "all joined");
}

/// Nine posts in a square, 0.3 mm apart, where a track with its clearance needs 0.6: the middle
/// one, of net A, is shut in on both layers. The post east of it is of net A too, and the
/// net's third pad stands across the board, nearer the middle post than the east one, so its
/// connection is asked for to the middle one. It is made to the east post, which the middle
/// one is joined to by a strand across the slot between them (an adapter board's rows of
/// holes behind its socket's pads, Youyue-858D-plus-MCU-adapter: the designer wires to the
/// socket pad).
#[test]
fn a_pad_shut_in_is_reached_by_way_of_a_pad_it_is_wired_to() {
    let mut parts = Vec::new();
    for x in [7.7, 10.0, 12.3] {
        for y in [7.7, 10.0, 12.3] {
            parts.push(post(x, y, if y == 10.0 && x >= 10.0 { "A" } else { "" }));
        }
    }
    parts.push(pad(3.0, 16.0, 1.5, 1.5, "A"));
    let board = board(&parts.concat(), 0.2);
    let (plan, drawing) = whole(&board);
    assert_eq!(wire::amiss(&board, &plan, &drawing), (0, 0));
}

/// A castellated pin as a Pico-shaped board has along its edges: a long pad that hangs over the
/// board's edge, its middle (and its half-hole) just outside the board, and a round pad of
/// the same number inside.
fn castellated(y: f64, net: &str) -> String {
    let n = ["", "A", "B", "C"].iter().position(|&k| k == net).unwrap();
    format!(
        "(footprint \"P\" (layer \"F.Cu\") (at 0 {y})
           (pad \"1\" thru_hole rect (at -0.05 0) (size 3.5 1.7) (drill 1) (layers \"*.Cu\" \"*.Mask\") (net {n} \"{net}\"))
           (pad \"1\" thru_hole oval (at 1.65 0) (size 1.7 1.7) (drill 1) (layers \"*.Cu\" \"*.Mask\") (net {n} \"{net}\")))"
    )
}

/// A board 20 mm square with EnvOpenPico's rules: copper may come within 0.025 mm of the edge.
fn edge_board(parts: &str) -> Board {
    let text = format!(
        "(kicad_pcb (version 20240108) (generator \"pcbnew\") (general (thickness 1.6))
         (layers (0 \"F.Cu\" signal) (31 \"B.Cu\" signal) (44 \"Edge.Cuts\" user))
         (net 0 \"\") (net 1 \"A\") (net 2 \"B\") (net 3 \"C\")
         (gr_rect (start 0 0) (end 20 20) (layer \"Edge.Cuts\") (width 0.1))
         {parts})"
    );
    let rules = r#"{"board":{"design_settings":{"rules":{"min_clearance":0.15,"min_copper_edge_clearance":0.025,"min_track_width":0.15}}},
        "net_settings":{"classes":[{"name":"Default","clearance":0.15,"track_width":0.15,"via_diameter":0.4,"via_drill":0.2}]}}"#;
    let (pcb, copper) = kicad::parse_board(&text).unwrap();
    Board::new(&pcb, &copper, kicad::Rules::parse(rules).unwrap())
}

/// Three castellated pins down the board's left edge, each wired to a pin of a chip, in order
/// and crossed over, and to pads above and below the column. A track cannot end at the middle
/// of such a pin, which is off the board: it ends at the middle of the part of the pin that is
/// on it. Every strand of the first plan is drawn, and in 0/45/90 (EnvOpenPico, test set,
/// 2026-11-22: 17 of its first plan's 88 strands were not, for starting past the edge; laying
/// them again and again took the board from 11 vias to 113 and left 10 connections open).
#[test]
fn a_wire_to_a_pad_that_hangs_over_the_boards_edge_ends_on_the_part_that_is_on_the_board() {
    for targets in [
        [pad(8.0, 9.4, 0.2, 0.9, "A"), pad(8.4, 9.4, 0.2, 0.9, "B"), pad(8.8, 9.4, 0.2, 0.9, "C")],
        [pad(8.8, 9.4, 0.2, 0.9, "A"), pad(8.4, 9.4, 0.2, 0.9, "B"), pad(8.0, 9.4, 0.2, 0.9, "C")],
        [pad(3.0, 3.0, 1.0, 1.0, "A"), pad(4.5, 3.0, 1.0, 1.0, "B"), pad(6.0, 3.0, 1.0, 1.0, "C")],
        [pad(8.0, 16.0, 0.2, 0.9, "A"), pad(8.4, 16.0, 0.2, 0.9, "B"), pad(8.8, 16.0, 0.2, 0.9, "C")],
    ] {
        let parts = [castellated(7.46, "A"), castellated(10.0, "B"), castellated(12.54, "C"), targets.concat()].concat();
        let board = edge_board(&parts);
        let plan = plan::plan(&board);
        assert!(plan.left.is_empty(), "{:?}", plan.left);
        let drawing = wire::draw(&board, &plan);
        assert!(drawing.undrawn.is_empty(), "{:?}", drawing.undrawn.iter().map(|u| &u.why).collect::<Vec<_>>());
        for w in &drawing.wires {
            assert!(!w.as_planned, "{:?}", w.pts);
            assert!(w.pts.iter().all(|p| p[0] > 0.1), "on the board: {:?}", w.pts);
        }
    }
}
