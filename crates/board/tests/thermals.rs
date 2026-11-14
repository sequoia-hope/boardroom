//! Room for a pour's thermal spokes, as KiCad 9.0.8's filler and DRC treat them (probed with
//! kicad-cli on boards like these, 2026-10-31): a pad the pour joins with reliefs keeps as many
//! spokes free of other nets' copper as `min_resolved_spokes` asks (2 unless a rule says), and a
//! spoke is lost to a track whose clearance leaves the pour past the gap less than the zone's
//! minimum width deep.

use board::*;

const RULES: &str = r#"{"board":{"design_settings":{"rules":{"min_clearance":0.2,"min_copper_edge_clearance":0.3}}}}"#;

/// Net A's zone over F.Cu, with a 0.5 mm thermal gap and spokes, 0.25 mm minimum width and
/// 0.2 mm clearance, unfilled; `pad` and `extra` inside it.
fn board(pad: &str, connect: &str, extra: &str, custom: &str) -> Board {
    let text = format!(
        "(kicad_pcb (version 20240108) (generator \"pcbnew\") (general (thickness 1.6))
         (layers (0 \"F.Cu\" signal) (31 \"B.Cu\" signal) (44 \"Edge.Cuts\" user))
         (net 0 \"\") (net 1 \"A\") (net 2 \"B\")
         (gr_rect (start 0 0) (end 20 20) (layer \"Edge.Cuts\") (width 0.1))
         (footprint \"P\" (layer \"F.Cu\") (at 10 10) (property \"Reference\" \"U1\") {pad})
         (zone (net 1) (net_name \"A\") (layer \"F.Cu\") (hatch edge 0.5) (connect_pads {connect} (clearance 0.2)) (min_thickness 0.25)
           (fill yes (thermal_gap 0.5) (thermal_bridge_width 0.5))
           (polygon (pts (xy 1 1) (xy 19 1) (xy 19 19) (xy 1 19))))
         {extra})"
    );
    let (pcb, copper) = kicad::parse_board(&text).unwrap();
    let mut rules = kicad::Rules::parse(RULES).unwrap();
    if !custom.is_empty() {
        rules.apply_custom(custom, "test.kicad_dru");
    }
    Board::new(&pcb, &copper, rules)
}

const RECT: &str = "(pad \"1\" smd rect (at 0 0) (size 1 0.6) (layers \"F.Cu\") (net 1 \"A\"))";
const FOUR: &str = "(version 1) (rule spokes (constraint min_resolved_spokes 4))";

/// The far ends of the spokes kept, in millimetres, rounded to the micron.
fn tips(b: &Board) -> Vec<(f64, f64)> {
    let mut out: Vec<(f64, f64)> = b
        .items()
        .filter_map(|(_, it)| match (&it.kind, &it.shape) {
            (Kind::Spoke { .. }, Shape::Polygon(p)) => Some((((p[1].x + p[2].x) / 2) as f64 / 1e6, ((p[1].y + p[2].y) / 2) as f64 / 1e6)),
            _ => None,
        })
        .map(|(x, y)| ((x * 1e3).round() / 1e3, (y * 1e3).round() / 1e3))
        .collect();
    out.sort_by(|a, b| a.partial_cmp(b).unwrap());
    out
}

/// A 0.2 mm track of net B from (x, 2) to (x, 18), laid by the router.
fn track(b: &mut Board, x: f64) -> ItemId {
    b.insert(Item {
        kind: Kind::Track,
        net: 2,
        layers: 1,
        shape: Shape::Capsule { a: Point::new(nm(x * 1e-3), nm(2e-3)), b: Point::new(nm(x * 1e-3), nm(18e-3)), r: nm(0.1e-3) },
        hole: None,
        locked: false,
        span: None,
    })
}

/// How many whole spokes each pad's kept spokes ask for.
fn needs(b: &Board) -> Vec<u8> {
    b.items().filter_map(|(_, it)| match it.kind {
        Kind::Spoke { need, .. } => Some(need),
        _ => None,
    }).collect()
}

#[test]
fn a_relief_pad_keeps_room_for_every_spoke_that_fits_and_needs_two_unless_a_rule_asks_more() {
    // Along the pad's axes, past its edge, the 0.5 mm gap and a 0.25 mm land of pour.
    let four = vec![(8.75, 10.0), (10.0, 8.95), (10.0, 11.05), (11.25, 10.0)];
    assert_eq!(tips(&board(RECT, "", "", "")), four);
    assert_eq!(needs(&board(RECT, "", "", "")), vec![2; 4]);
    assert_eq!(needs(&board(RECT, "", "", FOUR)), vec![4; 4]);
    assert_eq!(needs(&board(RECT, "", "", "(version 1) (rule one (constraint min_resolved_spokes 1))")), vec![1; 4]);
}

#[test]
fn a_solid_or_unjoined_pad_keeps_none() {
    assert!(tips(&board(RECT, "yes", "", "")).is_empty(), "the zone joins pads solidly");
    assert!(tips(&board(RECT, "no", "", "")).is_empty(), "the zone joins no pad");
    assert!(tips(&board(RECT, "thru_hole_only", "", "")).is_empty(), "reliefs only for through-hole pads");
    let solid = "(pad \"1\" smd rect (at 0 0) (size 1 0.6) (layers \"F.Cu\") (net 1 \"A\") (zone_connect 2))";
    assert!(tips(&board(solid, "", "", "")).is_empty(), "the pad's own connection is solid");
    let tht = "(pad \"1\" thru_hole circle (at 0 0) (size 1.6 1.6) (drill 0.8) (layers \"*.Cu\") (net 1 \"A\"))";
    assert_eq!(tips(&board(tht, "thru_hole_only", "", "")).len(), 4);
}

#[test]
fn a_round_pads_spokes_run_at_45_degrees_and_a_rotated_pads_along_its_axes() {
    let round = "(pad \"1\" smd circle (at 0 0) (size 1 1) (layers \"F.Cu\") (net 1 \"A\"))";
    let r = 0.5 + 0.5 + 0.25;
    let d = ((r / 2f64.sqrt()) * 1e3).round() / 1e3;
    assert_eq!(tips(&board(round, "", "", FOUR)), vec![(10.0 - d, 10.0 - d), (10.0 - d, 10.0 + d), (10.0 + d, 10.0 - d), (10.0 + d, 10.0 + d)]);
    let turned = "(pad \"1\" smd rect (at 0 0 90) (size 1 0.6) (layers \"F.Cu\") (net 1 \"A\"))";
    assert_eq!(tips(&board(turned, "", "", FOUR)), vec![(8.95, 10.0), (10.0, 8.75), (10.0, 11.25), (11.05, 10.0)]);
}

#[test]
fn spokes_that_other_copper_or_the_zones_edge_would_cut_are_not_kept() {
    // Net B's pad 0.6 mm right of U1's: the +x spoke has no room.
    let neighbour = "(footprint \"P\" (layer \"F.Cu\") (at 11.3 10) (property \"Reference\" \"U2\")
                       (pad \"1\" smd rect (at 0 0) (size 0.4 0.6) (layers \"F.Cu\") (net 2 \"B\")))";
    assert_eq!(tips(&board(RECT, "", neighbour, FOUR)), vec![(8.75, 10.0), (10.0, 8.95), (10.0, 11.05)]);
    let b = board(RECT, "", neighbour, FOUR);
    assert!(b.warnings.iter().any(|w| w.contains("thermal spokes: 1 of 1")), "{:?}", b.warnings);
    // Two are enough by default: no warning.
    assert!(!board(RECT, "", neighbour, "").warnings.iter().any(|w| w.contains("thermal")));
}

#[test]
fn new_copper_of_another_net_keeps_clear_of_a_spoke_as_kicad_judges_it() {
    let mut b = board(RECT, "", "", FOUR);
    // The +x spoke's land ends at x = 11.25: KiCad keeps the spoke for a 0.2 mm track at
    // x = 11.55 (0.2 mm clearance) and loses it at 11.5.
    let near = track(&mut b, 11.5);
    let hits: Vec<Violation> = check_item(&b, near).into_iter().filter(|v| v.rule == Rule::Clearance).collect();
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert!(matches!(b.item(hits[0].b.unwrap()).unwrap().kind, Kind::Spoke { .. }));
    b.remove(near);
    let clear = track(&mut b, 11.56);
    assert!(check_item(&b, clear).is_empty(), "{:?}", check_item(&b, clear));
}

/// A 0.2 mm track of net B from (2, y) to (18, y).
fn across(b: &mut Board, y: f64) -> ItemId {
    b.insert(Item {
        kind: Kind::Track,
        net: 2,
        layers: 1,
        shape: Shape::Capsule { a: Point::new(nm(2e-3), nm(y * 1e-3)), b: Point::new(nm(18e-3), nm(y * 1e-3)), r: nm(0.1e-3) },
        hole: None,
        locked: false,
        span: None,
    })
}

#[test]
fn other_nets_may_cut_the_spokes_a_pad_can_spare_and_no_more() {
    let mut b = board(RECT, "", "", "");
    let clearance = |b: &Board, id| check_item(b, id).iter().filter(|v| v.rule == Rule::Clearance).count();
    let first = track(&mut b, 11.5);
    assert_eq!(clearance(&b, first), 0, "the +x spoke goes, three are left");
    let second = track(&mut b, 8.5);
    assert_eq!(clearance(&b, second), 0, "the -x spoke goes, two are left");
    let third = across(&mut b, 11.3);
    assert_eq!(clearance(&b, third), 1, "the +y spoke would leave one");
    b.remove(first);
    assert_eq!(clearance(&b, third), 0, "with the +x spoke back, two are left");
}

#[test]
fn a_spoke_joins_nothing() {
    // A pad of net A far outside the zone, and a track of A ending inside U1's +x spoke: the
    // spoke is room, not copper, so the track reaches nothing.
    let other = "(footprint \"P\" (layer \"F.Cu\") (at 10 10) (property \"Reference\" \"U3\")
                   (pad \"2\" smd rect (at 9 9) (size 0.5 0.5) (layers \"F.Cu\") (net 1 \"A\")))";
    let mut b = board(RECT, "", other, FOUR);
    let before = connectivity_without_pours(&b).airlines.len();
    b.insert(Item {
        kind: Kind::Track,
        net: 1,
        layers: 1,
        shape: Shape::Capsule { a: Point::new(nm(11.1e-3), nm(10e-3)), b: Point::new(nm(19e-3), nm(19e-3)), r: nm(0.1e-3) },
        hole: None,
        locked: false,
        span: None,
    });
    assert_eq!(connectivity_without_pours(&b).airlines.len(), before, "U1's pad and U3's are no nearer joined");
}

#[test]
fn a_spoke_that_lands_in_another_pads_relief_is_not_kept() {
    // A second pad of net A 0.8 mm above U1's: the pour leaves its gap round it, so U1's -y
    // spoke (and its +y one toward U1) would land where there is no pour.
    let above = "(footprint \"P\" (layer \"F.Cu\") (at 10 9.2) (property \"Reference\" \"U2\")
                   (pad \"1\" smd rect (at 0 0) (size 1 0.6) (layers \"F.Cu\") (net 1 \"A\")))";
    let tips = tips(&board(RECT, "", above, FOUR));
    assert!(!tips.contains(&(10.0, 8.95)) && !tips.contains(&(10.0, 10.25)), "{tips:?}");
    assert!(tips.contains(&(11.25, 10.0)) && tips.contains(&(10.0, 11.05)), "{tips:?}");
}

#[test]
fn an_unlocked_track_leaves_a_spoke_its_room_and_a_locked_one_does_not() {
    // Net B's track across the +x spoke's gap, as the designer left it: the router may take an
    // unlocked one up (`--strip-all` takes them all once the board is loaded).
    let across = |lock: &str| format!("(segment (start 10.8 5) (end 10.8 15) (width 0.2) (layer \"F.Cu\") (net 2) {lock} (uuid \"5e000000-0000-4000-8000-000000000001\"))");
    assert_eq!(tips(&board(RECT, "", &across(""), FOUR)).len(), 4);
    assert_eq!(tips(&board(RECT, "", &across("(locked yes)"), FOUR)), vec![(8.75, 10.0), (10.0, 8.95), (10.0, 11.05)]);
}

/// A track of net A from (x0, y0) to (x1, y1).
fn own(b: &mut Board, (x0, y0): (f64, f64), (x1, y1): (f64, f64)) -> ItemId {
    b.insert(Item {
        kind: Kind::Track,
        net: 1,
        layers: 1,
        shape: Shape::Capsule { a: Point::new(nm(x0 * 1e-3), nm(y0 * 1e-3)), b: Point::new(nm(x1 * 1e-3), nm(y1 * 1e-3)), r: nm(0.1e-3) },
        hole: None,
        locked: false,
        span: None,
    })
}

#[test]
fn the_projects_minimum_holds_where_no_rule_sets_one() {
    let text = std::fs::read_to_string(env!("CARGO_MANIFEST_DIR").to_string() + "/../../fixtures/parallelogram-p1-66.kicad_pro").unwrap();
    assert!(text.contains("\"min_resolved_spokes\": 1"));
    let (pcb, copper) = kicad::parse_board(&std::fs::read_to_string(env!("CARGO_MANIFEST_DIR").to_string() + "/../../fixtures/parallelogram-p1-66.kicad_pcb").unwrap()).unwrap();
    let b = Board::new(&pcb, &copper, kicad::Rules::parse(&text).unwrap());
    assert!(needs(&b).iter().all(|&n| n == 1), "the parallelogram asks one spoke of each pad");
}

#[test]
fn a_track_of_the_pads_own_net_out_of_it_counts_as_a_spoke_and_a_via_on_it_joins_it() {
    let clearance = |b: &Board, id| check_item(b, id).iter().filter(|v| v.rule == Rule::Clearance).count();
    let mut b = board(RECT, "", "", "");
    track(&mut b, 11.5);
    track(&mut b, 8.5);
    let third = across(&mut b, 11.3);
    assert_eq!(clearance(&b, third), 1, "one spoke left of two");
    own(&mut b, (10.0, 10.0), (10.0, 6.0));
    assert_eq!(clearance(&b, third), 0, "the -y spoke and the pad's own track out of it");
    // All four cut: KiCad leaves a pad no spoke reaches alone; with a via of its net on it, it
    // is joined.
    let mut b = board(RECT, "", "", "");
    track(&mut b, 11.5);
    track(&mut b, 8.5);
    across(&mut b, 11.3);
    let last = across(&mut b, 8.7);
    assert_eq!(clearance(&b, last), 1, "none left, and nothing else joins the pad");
    let at = Point::new(nm(10e-3), nm(10e-3));
    b.insert(Item { kind: Kind::Via, net: 1, layers: 3, shape: Shape::Capsule { a: at, b: at, r: nm(0.2e-3) }, hole: Some((at, nm(0.1e-3))), locked: false, span: None });
    assert_eq!(clearance(&b, last), 0, "a via of its net stands on it");
}

#[test]
fn pads_of_one_number_each_leave_their_own_gap() {
    // Three holes under a regulator's tab, all pad 1, 0.79 mm apart: the pour leaves its gap
    // round each, so the outer two keep only the spokes that point away from the row and the
    // middle one none (TLPHnodeV2's U7, kicad-cli 9.0.8).
    let hole = |y: f64| format!("(pad \"1\" thru_hole circle (at 0 {y}) (size 0.508 0.508) (drill 0.254) (layers \"*.Cu\") (net 1 \"A\"))");
    let row = [hole(-0.7874), hole(0.0), hole(0.7874)].concat();
    let tips = tips(&board(&row, "", "", FOUR));
    assert_eq!(tips.len(), 4, "{tips:?}");
    assert!(tips.iter().all(|&(_, y)| !(9.3..=10.7).contains(&y)), "{tips:?}");
}

#[test]
fn a_spoke_with_room_for_its_middle_alone_is_kept_that_wide() {
    // Net B's pad 0.15 mm above the -x spoke's edge, nearer than the zone's 0.2 mm: KiCad
    // cuts that much off the spoke's side and keeps the rest, which is still as wide as the
    // zone's least width (kitspace's O1, kicad-cli 9.0.8).
    let beside = "(footprint \"P\" (layer \"F.Cu\") (at 9 9.3) (property \"Reference\" \"U2\")
                    (pad \"1\" smd rect (at 0 0) (size 0.5 0.6) (layers \"F.Cu\") (net 2 \"B\")))";
    let b = board(RECT, "", beside, FOUR);
    assert!(tips(&b).contains(&(8.75, 10.0)), "{:?}", tips(&b));
    let widths: Vec<f64> = b
        .items()
        .filter_map(|(_, it)| match (&it.kind, &it.shape) {
            (Kind::Spoke { .. }, Shape::Polygon(p)) if (p[1].x + p[2].x) / 2 < nm(9e-3) => Some(p[0].distance(p[3]) / 1e6),
            _ => None,
        })
        .collect();
    assert_eq!(widths.len(), 1);
    assert!((widths[0] - 0.25).abs() < 1e-3, "{widths:?}");
    // And a track that takes the middle too takes the spoke.
    let mut b = b;
    let cut = track(&mut b, 9.1);
    assert_eq!(check_item(&b, cut).iter().filter(|v| v.rule == Rule::Clearance).count(), 1);
}

/// A track of net B from (x0, y0) to (x1, y1).
fn other(b: &mut Board, (x0, y0): (f64, f64), (x1, y1): (f64, f64)) -> ItemId {
    let id = own(b, (x0, y0), (x1, y1));
    let mut item = b.item(id).unwrap().clone();
    b.remove(id);
    item.net = 2;
    b.insert(item)
}

/// A pad with a hole wider than its spokes, so that each spoke is its own piece of copper.
const HOLED: &str = "(pad \"1\" thru_hole rect (at 0 0) (size 1.6 1.6) (drill 1) (layers \"*.Cu\") (net 1 \"A\"))";
/// The same pad with no hole: its spokes are one piece of copper under it.
const WHOLE: &str = "(pad \"1\" smd rect (at 0 0) (size 1.6 1.6) (layers \"F.Cu\") (net 1 \"A\"))";
/// Another pad of net A in the zone, for the pour to join U1's to.
const SECOND: &str = "(footprint \"P\" (layer \"F.Cu\") (at 4 16) (property \"Reference\" \"U3\")
                        (pad \"1\" smd rect (at 0 0) (size 1 0.6) (layers \"F.Cu\") (net 1 \"A\")))";

/// Net B's tracks round three sides of the pour U1's +x spoke ends in, the pad's own gap the
/// fourth: a piece of pour 0.6 by 1 mm that joins nothing but that spoke.
fn shut_in(b: &mut Board) -> [ItemId; 3] {
    [other(b, (11.2, 9.2), (12.2, 9.2)), other(b, (12.2, 9.2), (12.2, 10.8)), other(b, (12.2, 10.8), (11.2, 10.8))]
}

#[test]
fn a_spoke_into_pour_that_joins_nothing_else_is_no_spoke() {
    let mut b = board(HOLED, "", SECOND, FOUR);
    assert!(Pours::likely(&b).starved(&b).is_empty(), "four spokes into the pour");
    let round = shut_in(&mut b);
    assert!(round.iter().all(|&t| check_item(&b, t).is_empty()), "no spoke is cut");
    let pours = Pours::likely(&b);
    let starved = pours.starved(&b);
    assert_eq!(starved.len(), 1, "three spokes of the four asked for: {starved:?}");
    assert_eq!((starved[0].layer, starved[0].pockets.len()), (0, 1));
    // The tracks round that piece are what it ends at; one far off is not.
    assert!(round.iter().all(|&t| pours.beside(&b, starved[0].pockets[0], t)));
    let far = track(&mut b, 16.0);
    assert!(!pours.beside(&b, starved[0].pockets[0], far));
}

#[test]
fn spokes_under_a_pad_with_no_hole_are_one_piece_of_copper() {
    // The same pour shut in, but the +x spoke is joined to the other three under the pad, and
    // so the pour it ends in to the rest (kicad-cli 9.0.8: OpenBot's U2, whose spokes are
    // wider than its hole, is not starved; R9, whose hole parts them, is).
    let mut b = board(WHOLE, "", SECOND, FOUR);
    shut_in(&mut b);
    assert!(Pours::likely(&b).starved(&b).is_empty());
}

#[test]
fn a_track_of_the_pads_net_into_the_pour_counts_for_a_spoke_unless_it_ends_in_the_pads_hole() {
    let mut b = board(HOLED, "", SECOND, FOUR);
    shut_in(&mut b);
    // Out of the pad into its own +y spoke, inside the hole: KiCad takes the hole out of
    // the pour, so this ends in none.
    own(&mut b, (10.5, 10.0), (10.0, 10.2));
    assert_eq!(Pours::likely(&b).starved(&b).len(), 1);
    own(&mut b, (10.0, 10.0), (7.0, 7.0));
    assert!(Pours::likely(&b).starved(&b).is_empty(), "three spokes and a track into the pour");
}

#[test]
fn a_pad_whose_spokes_all_end_in_pour_that_joins_nothing_is_starved_only_once_a_track_joins_it() {
    // Net B all round U1, 3 mm out: the pour inside joins nothing but the pad. KiCad leaves
    // that to the connectivity check; but a track out of the pad on the other side of the
    // board, ending over that pour, counts as one spoke, and then one is too few (Shift-in
    // 32's P1-8, kicad-cli 9.0.8).
    let mut b = board(HOLED, "", SECOND, "");
    for (from, to) in [((7.0, 7.0), (13.0, 7.0)), ((13.0, 7.0), (13.0, 13.0)), ((13.0, 13.0), (7.0, 13.0)), ((7.0, 13.0), (7.0, 7.0))] {
        other(&mut b, from, to);
    }
    assert!(Pours::likely(&b).starved(&b).is_empty());
    let back = own(&mut b, (10.0, 10.0), (11.8, 11.8));
    let mut item = b.item(back).unwrap().clone();
    b.remove(back);
    item.layers = 2;
    b.insert(item);
    let starved = Pours::likely(&b).starved(&b);
    assert_eq!(starved.len(), 1, "{starved:?}");
    assert_eq!(starved[0].pockets.len(), 1, "all four spokes end in the one piece");
}

#[test]
fn two_pads_whose_spokes_meet_are_joined_by_them() {
    // A second pad of net A below U1's, their ends 1.14 mm apart: no pour between the two
    // gaps, and each pad's spoke towards the other ends inside the other's. KiCad keeps the
    // two as a bridge (Biscay's SOT-23-6 with two ground pins, kicad-cli 9.0.8).
    let below = "(footprint \"P\" (layer \"F.Cu\") (at 10 11.74) (property \"Reference\" \"U2\")
                   (pad \"1\" smd rect (at 0 0) (size 1 0.6) (layers \"F.Cu\") (net 1 \"A\")))";
    let mut b = board(RECT, "", &[below, SECOND].concat(), "");
    // Net B takes every other spoke of the lower pad, and the upper pad's two sideways.
    track(&mut b, 11.5);
    track(&mut b, 8.5);
    across(&mut b, 12.9);
    let open = |b: &Board| connectivity_poured(b, &Pours::of(b)).airlines.iter().filter(|a| a.net == 1).count();
    assert_eq!(open(&b), 0, "the lower pad by the bridge, the upper by its spoke up into the pour");
    // A track of net B between the two cuts the bridge.
    other(&mut b, (9.0, 10.87), (11.0, 10.87));
    assert_eq!(open(&b), 1);
}

#[test]
fn a_bridge_cut_at_one_pads_end_is_cut_for_the_other_pad_too() {
    // The two pads of the test before, one spoke asked of each. Net B takes every spoke of
    // the upper pad but the one that meets the lower pad's.
    let below = "(footprint \"P\" (layer \"F.Cu\") (at 10 11.74) (property \"Reference\" \"U2\")
                   (pad \"1\" smd rect (at 0 0) (size 1 0.6) (layers \"F.Cu\") (net 1 \"A\")))";
    let mut b = board(RECT, "", below, "(version 1) (rule one (constraint min_resolved_spokes 1))");
    track(&mut b, 11.5);
    track(&mut b, 8.5);
    across(&mut b, 8.7);
    // A track that comes within its clearance of the lower pad's half of the bridge alone:
    // the lower pad has its spoke down still, but the upper is left none.
    let beside = other(&mut b, (10.5, 11.12), (11.2, 11.12));
    let hits: Vec<Violation> = check_item(&b, beside).into_iter().filter(|v| v.rule == Rule::Clearance).collect();
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert!(matches!(b.item(hits[0].b.unwrap()).unwrap().kind, Kind::Spoke { .. }));
    // Farther off it cuts neither.
    b.remove(beside);
    let clear = other(&mut b, (10.7, 11.12), (11.2, 11.12));
    assert!(check_item(&b, clear).iter().all(|v| v.rule != Rule::Clearance));
}
