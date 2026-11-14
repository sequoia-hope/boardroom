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
