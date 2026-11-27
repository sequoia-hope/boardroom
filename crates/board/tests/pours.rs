//! What a pour joins once it is filled round the copper on the board: the pads it reaches, a
//! pad with thermal reliefs by its spokes, and nothing a track of another net cuts it off from.

use board::*;

const RULES: &str = r#"{"board":{"design_settings":{"rules":{"min_clearance":0.2,"min_copper_edge_clearance":0.3}}}}"#;

/// Net A's zone over F.Cu of a 20 mm board, joining its pads as `connect` says, with `clearance`
/// from other nets, and `parts` on it.
fn board(connect: &str, clearance: f64, parts: &str) -> Board {
    let text = format!(
        "(kicad_pcb (version 20240108) (generator \"pcbnew\") (general (thickness 1.6))
         (layers (0 \"F.Cu\" signal) (31 \"B.Cu\" signal) (44 \"Edge.Cuts\" user))
         (net 0 \"\") (net 1 \"A\") (net 2 \"B\")
         (gr_rect (start 0 0) (end 20 20) (layer \"Edge.Cuts\") (width 0.1))
         {parts}
         (zone (net 1) (net_name \"A\") (layer \"F.Cu\") (hatch edge 0.5) (connect_pads {connect} (clearance {clearance})) (min_thickness 0.25)
           (fill yes (thermal_gap 0.5) (thermal_bridge_width 0.5))
           (polygon (pts (xy 1 1) (xy 19 1) (xy 19 19) (xy 1 19)))))"
    );
    let (pcb, copper) = kicad::parse_board(&text).unwrap();
    Board::new(&pcb, &copper, kicad::Rules::parse(RULES).unwrap())
}

fn pad(reference: &str, x: f64, y: f64, net: &str) -> String {
    format!("(footprint \"P\" (layer \"F.Cu\") (at {x} {y}) (property \"Reference\" \"{reference}\") (pad \"1\" smd rect (at 0 0) (size 1 0.6) (layers \"F.Cu\") {net}))")
}

const A: &str = "(net 1 \"A\")";
const B: &str = "(net 2 \"B\")";

fn track(b: &mut Board, from: (f64, f64), to: (f64, f64)) {
    let at = |p: (f64, f64)| Point::new(nm(p.0 * 1e-3), nm(p.1 * 1e-3));
    b.insert(Item { kind: Kind::Track, net: 2, layers: 1, shape: Shape::Capsule { a: at(from), b: at(to), r: nm(0.1e-3) }, hole: None, locked: false, span: None });
}

#[test]
fn a_pour_joins_the_pads_it_reaches() {
    let b = board("yes", 0.2, &[pad("U1", 5.0, 5.0, A), pad("U2", 15.0, 15.0, A), pad("U3", 10.0, 10.0, B)].concat());
    assert_eq!(connectivity_without_pours(&b).airlines.len(), 1);
    assert!(connectivity_poured(&b, &Pours::of(&b)).airlines.is_empty());
}

#[test]
fn a_pad_a_track_rings_off_is_not_joined() {
    let mut b = board("yes", 0.2, &[pad("U1", 5.0, 5.0, A), pad("U2", 15.0, 15.0, A)].concat());
    // A ring of net B round the second pad.
    for (from, to) in [((13.0, 13.0), (17.0, 13.0)), ((17.0, 13.0), (17.0, 17.0)), ((17.0, 17.0), (13.0, 17.0)), ((13.0, 17.0), (13.0, 13.0))] {
        track(&mut b, from, to);
    }
    assert_eq!(connectivity_poured(&b, &Pours::of(&b)).airlines.len(), 1);
}

#[test]
fn a_pad_with_reliefs_is_joined_by_its_spokes() {
    let b = board("", 0.2, &[pad("U1", 5.0, 5.0, A), pad("U2", 15.0, 15.0, A)].concat());
    assert!(b.items().any(|(_, it)| matches!(it.kind, Kind::Spoke { .. })));
    assert!(connectivity_poured(&b, &Pours::of(&b)).airlines.is_empty());
}

/// A pad between two near pads of another net, in a zone that keeps half a millimetre from
/// other nets: KiCad cuts that clearance out of the spokes as it does out of the pour, and none
/// is left to join the pad (kicad-cli 9.0.8 on CAL430FR's U2, 2026-11-13).
#[test]
fn a_pad_whose_spokes_other_nets_pads_cut_is_not_joined() {
    let parts = [pad("U1", 5.0, 5.0, A), pad("U2", 15.0, 15.0, A), pad("U3", 15.0, 14.2, B), pad("U4", 15.0, 15.8, B), pad("U5", 13.8, 15.0, B), pad("U6", 16.2, 15.0, B)].concat();
    let b = board("", 0.5, &parts);
    assert_eq!(connectivity_poured(&b, &Pours::of(&b)).airlines.iter().filter(|a| a.net == 1).count(), 1);
}

/// Two pads of the net too near each other for pour between them: each keeps the spoke that
/// meets the other's, as KiCad's filler does.
#[test]
fn spokes_that_meet_between_two_pads_of_the_net_are_kept() {
    let b = board("", 0.2, &[pad("U1", 10.0, 10.0, A), pad("U2", 11.8, 10.0, A)].concat());
    let between = b.items().filter(|(_, it)| matches!(it.kind, Kind::Spoke { .. })).filter(|(id, _)| b.bbox(*id).is_some_and(|x| x.min.x > nm(10.0e-3) - 1 && x.max.x < nm(11.8e-3) + 1)).count();
    assert_eq!(between, 2, "one from each pad, towards the other");
}

#[test]
fn a_via_stands_in_the_pour_where_all_of_it_is_pour() {
    let b = board("yes", 0.2, &[pad("U1", 5.0, 5.0, A), pad("U3", 10.0, 10.0, B)].concat());
    let pours = Pours::of(&b);
    let at = |x: f64, y: f64| Point::new(nm(x * 1e-3), nm(y * 1e-3));
    assert!(pours.piece(1, 0, at(15.0, 15.0), nm(0.3e-3)).is_some());
    assert!(pours.piece(1, 0, at(10.0, 10.6), nm(0.3e-3)).is_none(), "within the clearance round the other net's pad");
    assert!(pours.piece(1, 1, at(15.0, 15.0), nm(0.3e-3)).is_none(), "no pour on the other layer");
}

/// A wall of net B down the middle of net A's pour, with a gap in it: the pour is one piece,
/// narrow at the gap, where the wall's two ends pinch it; without the gap it is two pieces,
/// each joining its own pad.
#[test]
fn a_pour_narrowed_by_a_wall_has_a_neck_and_one_cut_through_is_two_pieces() {
    let pads = [pad("U1", 5.0, 10.0, A), pad("U2", 15.0, 10.0, A)].concat();
    let mut b = board("yes", 0.2, &pads);
    track(&mut b, (10.0, 1.0), (10.0, 9.5));
    track(&mut b, (10.0, 10.5), (10.0, 19.0));
    let zone = b.zone_areas[0].zone;
    let pieces = Pours::of(&b).pieces_of(&b, zone);
    assert_eq!(pieces.len(), 1);
    let neck = &pieces[0].necks[0];
    // 1 mm between the track ends, less 0.1 mm of track and 0.2 mm of clearance at each.
    assert!((neck.width - 400_000).abs() <= 50_000, "{neck:?}");
    assert!((neck.at.x - 10_000_000).abs() <= 100_000 && (neck.at.y - 10_000_000).abs() <= 300_000, "{neck:?}");
    assert!(neck.by.iter().all(|&(id, _)| matches!(b.item(id).unwrap().kind, Kind::Track)) && neck.by.len() >= 2, "{neck:?}");
    assert!(!neck.at_limit);
    assert_eq!(pieces[0].pads.len(), 2);
    // The two pads are joined inside the pour, the widest way between them through the gap.
    let links = Pours::of(&b).links_of(&b, zone);
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].pins.len(), 2);
    assert_eq!(links[0].groups.len(), 1);
    let weakest = links[0].groups[0].1.as_ref().unwrap();
    assert!((weakest.width - 400_000).abs() <= 50_000, "{weakest:?}");

    let mut b = board("yes", 0.2, &pads);
    track(&mut b, (10.0, 0.5), (10.0, 19.5));
    let pieces = Pours::of(&b).pieces_of(&b, zone);
    assert_eq!(pieces.len(), 2);
    assert!(pieces.iter().all(|p| p.pads.len() == 1 && p.necks.is_empty()), "{pieces:?}");
    let links = Pours::of(&b).links_of(&b, zone);
    assert_eq!(links[0].groups.len(), 2);
    assert!(links[0].groups.iter().all(|(m, w)| m.len() == 1 && w.is_none()));
}

/// A pour's sections: cut only by the nets let cut it. A wall of B with a gap leaves the two
/// pads one section, whole; with B let cut it, a wall without a gap makes two sections, each
/// whole; with B not let cut it, the same wall breaks the one section in two.
#[test]
fn sections_are_cut_only_by_the_nets_let_cut_them() {
    let pads = [pad("U1", 5.0, 10.0, A), pad("U2", 15.0, 10.0, A)].concat();
    let mut b = board("yes", 0.2, &pads);
    track(&mut b, (10.0, 1.0), (10.0, 9.5));
    track(&mut b, (10.0, 10.5), (10.0, 19.0));
    let zone = b.zone_areas[0].zone;
    let s = sections(&b, zone, &[], 300_000);
    assert_eq!(s.len(), 1);
    assert!(s[0].ok && s[0].groups.len() == 1, "{s:?}");
    assert!(!sections(&b, zone, &[], 500_000)[0].ok, "a 0.4 mm link is narrower than 0.5 mm");

    let mut b = board("yes", 0.2, &pads);
    track(&mut b, (10.0, 0.5), (10.0, 19.5));
    let s = sections(&b, zone, &[2], 100_000);
    assert_eq!(s.len(), 2);
    assert!(s.iter().all(|s| s.ok && s.pins.len() == 1), "{s:?}");
    let s = sections(&b, zone, &[], 100_000);
    assert_eq!(s.len(), 1);
    assert!(!s[0].ok && s[0].groups.len() == 2, "{s:?}");
}

/// A pour kept joined: once asked, a via of another net that closes the one gap left in a wall
/// breaks its section and is faulted; one that stands clear of the gap is not.
#[test]
fn a_via_that_breaks_a_kept_section_is_a_fault() {
    let pads = [pad("U1", 5.0, 10.0, A), pad("U2", 15.0, 10.0, A)].concat();
    let mut b = board("yes", 0.2, &pads);
    track(&mut b, (10.0, 1.0), (10.0, 9.5));
    track(&mut b, (10.0, 10.5), (10.0, 19.0));
    let zone = b.zone_areas[0].zone;
    let now = b.keep_joined(zone, &[], 100_000);
    assert!(now.len() == 1 && now[0].ok, "{now:?}");
    let via = |b: &mut Board, x: f64, y: f64| {
        let p = Point::new(nm(x * 1e-3), nm(y * 1e-3));
        b.insert(Item { kind: Kind::Via, net: 2, layers: 3, shape: Shape::Capsule { a: p, b: p, r: nm(0.3e-3) }, hole: Some((p, nm(0.15e-3))), locked: false, span: None })
    };
    let kept = |b: &Board, id| check_item(b, id).iter().filter(|v| v.rule == Rule::KeptJoined).count();
    let clear = via(&mut b, 5.0, 16.0);
    assert_eq!(kept(&b, clear), 0);
    let shut = via(&mut b, 10.0, 10.0);
    assert_eq!(kept(&b, shut), 1);
}

/// Copper of a net let cut a kept pour, laid after it was asked, makes new sections: a wall of
/// it across the pour splits it in two and is no fault.
#[test]
fn copper_of_a_net_let_cut_a_kept_pour_is_no_fault() {
    let pads = [pad("U1", 5.0, 10.0, A), pad("U2", 15.0, 10.0, A)].concat();
    let mut b = board("yes", 0.2, &pads);
    let zone = b.zone_areas[0].zone;
    assert_eq!(b.keep_joined(zone, &[2], 100_000).len(), 1);
    let at = |p: (f64, f64)| Point::new(nm(p.0 * 1e-3), nm(p.1 * 1e-3));
    let wall = b.insert(Item { kind: Kind::Track, net: 2, layers: 1, shape: Shape::Capsule { a: at((10.0, 0.5)), b: at((10.0, 19.5)), r: nm(0.1e-3) }, hole: None, locked: false, span: None });
    assert!(!check_item(&b, wall).iter().any(|v| v.rule == Rule::KeptJoined));
    assert_eq!(sections(&b, zone, &[2], 100_000).len(), 2);
}

/// What the pours join is left to them, for a router finishing a board whose power nets lie
/// on pours: a connection a pour makes is none to make, one it leaves apart still is, and
/// the pours worked out are not asked.
#[test]
fn what_the_pours_join_is_not_a_connection_to_make() {
    // Net A's pour joins U1 and U2; a ring of net B cuts U5 off from it. Net B has no pour.
    let mut b = board("yes", 0.2, &[pad("U1", 5.0, 5.0, A), pad("U2", 15.0, 5.0, A), pad("U5", 15.0, 15.0, A), pad("U3", 4.0, 10.0, B), pad("U4", 10.0, 10.0, B)].concat());
    for (from, to) in [((13.0, 13.0), (17.0, 13.0)), ((17.0, 13.0), (17.0, 17.0)), ((17.0, 17.0), (13.0, 17.0)), ((13.0, 17.0), (13.0, 13.0))] {
        track(&mut b, from, to);
    }
    let nets = |c: Connectivity| c.airlines.iter().map(|l| l.net).collect::<Vec<_>>();
    assert_eq!(nets(connectivity_without_pours(&b)), [1, 1, 2, 2]);
    let left = b.join_by_pours();
    // U1 to U2 is the pour's; U5 is still apart, and so are net B's pads and its ring.
    assert_eq!(nets(connectivity_without_pours(&b)), [1, 2, 2]);
    assert_eq!(nets(connectivity(&b)), [1, 2, 2]);
    assert_eq!(left, 3);
    assert_eq!(nets(connectivity_poured(&b, &Pours::likely(&b))), [1, 2, 2]);
}
