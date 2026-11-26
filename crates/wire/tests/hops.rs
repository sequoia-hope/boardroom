//! A net that steps up to another layer and down again for nothing is joined on the layer it
//! leaves on, and the vias go.

use board::Board;
use wire::{Drawing, Wire};

/// A 20 by 20 mm board of two layers with front pads of net A at `pads` and back ones at
/// `back`: tracks 0.1 mm wide, vias 0.3 mm across.
fn board(pads: &[[f64; 2]], back: &[[f64; 2]]) -> Board {
    let pad = |p: &[f64; 2], layer: &str| format!("(pad \"1\" smd rect (at {} {}) (size 0.6 0.6) (layers \"{layer}\") (net 1 \"A\"))", p[0], p[1]);
    let pads: String = pads.iter().map(|p| pad(p, "F.Cu")).chain(back.iter().map(|p| pad(p, "B.Cu"))).collect();
    let text = format!(
        "(kicad_pcb (version 20240108) (generator \"pcbnew\") (general (thickness 1.6))
         (layers (0 \"F.Cu\" signal) (31 \"B.Cu\" signal) (44 \"Edge.Cuts\" user))
         (net 0 \"\") (net 1 \"A\")
         (gr_rect (start 0 0) (end 20 20) (layer \"Edge.Cuts\") (width 0.1))
         (footprint \"P\" (layer \"F.Cu\") (at 0 0) {pads}))"
    );
    let rules = r#"{"board":{"design_settings":{"rules":{"min_clearance":0.1,"min_copper_edge_clearance":0.3,"min_track_width":0.1,"min_hole_clearance":0.2,"min_hole_to_hole":0.25,"min_via_diameter":0.2,"min_through_hole_diameter":0.15}}},
            "net_settings":{"classes":[{"name":"Default","clearance":0.1,"track_width":0.1,"via_diameter":0.3,"via_drill":0.15}]}}"#;
    let (pcb, copper) = kicad::parse_board(&text).unwrap();
    Board::new(&pcb, &copper, kicad::Rules::parse(rules).unwrap())
}

fn wire(layer: usize, pts: &[[f64; 2]]) -> Wire {
    Wire { layer, strand: 0, net: 1, width: 0.1, pts: pts.to_vec(), as_planned: false }
}

fn open(board: &Board, plan: &plan::Plan, drawing: &Drawing) -> usize {
    board::connectivity_without_pours(&wire::onto(board, plan, drawing).0).airlines.len()
}

/// Three front pads, each with a wire to a via of its own, the three vias in a column 0.43 mm
/// apart and joined to each other on the back by a short island that touches nothing else (as
/// SW2_4 on p2-132-usb6). The island goes, the vias are joined on the front by two short
/// pieces, and no via is left.
#[test]
fn vias_joined_only_to_each_other_on_one_layer_are_joined_on_the_other_and_go() {
    let board = board(&[[4.0, 9.57], [16.0, 10.0], [10.0, 16.0]], &[]);
    let mut plan = plan::start(&board);
    let vias = [[10.0, 9.57], [10.0, 10.0], [10.0, 10.43]];
    plan.vias = vias.iter().enumerate().map(|(site, &at)| plan::Via { net: 1, site, at }).collect();
    let drawing = Drawing {
        wires: vec![
            wire(0, &[[4.0, 9.57], [10.0, 9.57]]),
            wire(0, &[[16.0, 10.0], [10.0, 10.0]]),
            wire(0, &[[10.0, 16.0], [10.0, 10.43]]),
            // The island on the back: from the top via out and round to the other two.
            wire(1, &[[10.0, 9.57], [10.4, 9.97], [10.4, 10.03], [10.0, 10.43]]),
            wire(1, &[[10.0, 10.0], [10.37, 10.0]]),
        ],
        ..Default::default()
    };
    assert_eq!(open(&board, &plan, &drawing), 0);
    let (plan2, less, gone) = wire::hops::hops(&board, plan.clone(), drawing.clone());
    assert_eq!(gone, 3);
    assert!(plan2.vias.is_empty());
    assert!(less.wires.iter().all(|w| w.layer == 0), "{:?}", less.wires);
    assert_eq!(less.wires.len(), 5, "the three wires and two joins: {:?}", less.wires);
    assert_eq!(open(&board, &plan2, &less), 0);
    assert!(board::check(&wire::onto(&board, &plan2, &less).0).is_empty());
    // Done again it finds nothing.
    let (_, _, again) = wire::hops::hops(&board, plan2, less);
    assert_eq!(again, 0);
}

/// A back pad fanned out through three vias of its own, 0.43 mm apart, each going on to a
/// front pad (as SW2_4 on p2-132-usb6, from LED D81's pad on the front to In2): one via is
/// kept, the other two go and are joined to it on the front, and the back stubs that led to
/// them are cut back to the pad's way to the kept via.
#[test]
fn a_pad_fanned_out_through_vias_of_its_own_keeps_one() {
    let board = board(&[[4.0, 9.57], [16.0, 10.0], [10.0, 16.0]], &[[11.2, 10.0]]);
    let mut plan = plan::start(&board);
    let vias = [[10.0, 9.57], [10.0, 10.0], [10.0, 10.43]];
    plan.vias = vias.iter().enumerate().map(|(site, &at)| plan::Via { net: 1, site, at }).collect();
    let drawing = Drawing {
        wires: vec![
            wire(0, &[[4.0, 9.57], [10.0, 9.57]]),
            wire(0, &[[16.0, 10.0], [10.0, 10.0]]),
            wire(0, &[[10.0, 16.0], [10.0, 10.43]]),
            wire(1, &[[11.0, 10.0], [10.57, 9.57], [10.0, 9.57]]),
            wire(1, &[[11.0, 10.0], [10.0, 10.0]]),
            wire(1, &[[11.0, 10.0], [10.57, 10.43], [10.0, 10.43]]),
        ],
        ..Default::default()
    };
    assert_eq!(open(&board, &plan, &drawing), 0);
    let (plan2, less, gone) = wire::hops::hops(&board, plan, drawing);
    assert_eq!((gone, plan2.vias.len()), (2, 1));
    // The middle via is kept: its way to the pad is the shortest.
    assert_eq!(plan2.vias[0].at, [10.0, 10.0]);
    assert_eq!(less.wires.iter().filter(|w| w.layer == 1).count(), 1, "one way from the pad to the via: {:?}", less.wires);
    assert_eq!(open(&board, &plan2, &less), 0);
    assert!(board::check(&wire::onto(&board, &plan2, &less).0).is_empty());
}

/// Two vias joined on the back, where the front wire into the lower one comes down to it from
/// above and to the right: the only join on the front, straight down from the upper via, would
/// leave the lower via's place at 45 degrees to that wire, a sharp wedge of copper. The vias
/// stay (gb-hardware's NT1).
#[test]
fn a_join_that_would_meet_a_wire_at_a_sharp_angle_is_not_laid() {
    let board = board(&[[4.0, 9.57], [11.0, 4.0]], &[]);
    let mut plan = plan::start(&board);
    plan.vias = [[10.0, 9.57], [10.0, 12.0]].iter().enumerate().map(|(site, &at)| plan::Via { net: 1, site, at }).collect();
    let drawing = Drawing {
        wires: vec![wire(0, &[[4.0, 9.57], [10.0, 9.57]]), wire(0, &[[11.0, 4.0], [11.0, 11.0], [10.0, 12.0]]), wire(1, &[[10.0, 9.57], [10.0, 12.0]])],
        ..Default::default()
    };
    assert_eq!(open(&board, &plan, &drawing), 0);
    let (plan2, _, gone) = wire::hops::hops(&board, plan.clone(), drawing.clone());
    assert_eq!((gone, plan2.vias.len()), (0, 2));
    // The same where the wire ends in a sliver 0.0005 mm long pointing another way, as a wire
    // drawn as planned can: the way it leaves the via is the way of the wire, not the sliver.
    let mut sliver = drawing;
    sliver.wires[1].pts = vec![[11.0, 4.0], [11.0, 11.0], [9.9999, 12.0002], [10.0, 12.0]];
    let (plan2, _, gone) = wire::hops::hops(&board, plan, sliver);
    assert_eq!((gone, plan2.vias.len()), (0, 2));
}

/// Two vias joined on the front, where the back wire into one comes up from below and turns
/// at 45 degrees into it, and the other via lies up and to the left: joined from via to via on
/// the back, the track would turn a right angle at the first via's place and run back the way
/// it came, a spike no designer draws (saiboard_8x8's GND, 2026-11-26). The back wire is cut
/// back to its last corner and joined from there: one straight track.
#[test]
fn a_wire_into_a_via_that_goes_is_cut_back_to_its_corner_and_joined_from_there() {
    let board = board(&[], &[[6.0, 2.0], [6.0, 16.0]]);
    let mut plan = plan::start(&board);
    plan.vias = [[6.0, 6.0], [10.0, 10.0]].iter().enumerate().map(|(site, &at)| plan::Via { net: 1, site, at }).collect();
    let drawing = Drawing {
        wires: vec![wire(1, &[[6.0, 2.0], [6.0, 6.0]]), wire(1, &[[6.0, 16.0], [6.0, 14.0], [10.0, 10.0]]), wire(0, &[[6.0, 6.0], [10.0, 6.0], [10.0, 10.0]])],
        ..Default::default()
    };
    assert_eq!(open(&board, &plan, &drawing), 0);
    let (plan2, less, gone) = wire::hops::hops(&board, plan, drawing);
    assert_eq!((gone, plan2.vias.len()), (2, 0));
    assert!(less.wires.iter().all(|w| w.layer == 1 && w.pts.iter().all(|p| (p[0] - 6.0).abs() < 1e-9)), "one straight track at x = 6: {:?}", less.wires);
    assert_eq!(open(&board, &plan2, &less), 0);
    assert!(board::check(&wire::onto(&board, &plan2, &less).0).is_empty());
}

/// The same where the back wire is one straight piece into the via, so there is no corner to
/// cut back to: of the two ways on to the other via, the one that leaves the via's place at a
/// right angle to the wire and bends back is not laid; the one that leaves it straight up is.
#[test]
fn where_a_via_goes_a_join_does_not_turn_a_right_angle_there_and_bend_back() {
    let board = board(&[], &[[7.0, 2.0], [6.0, 14.0]]);
    let mut plan = plan::start(&board);
    plan.vias = [[10.0, 10.0], [7.0, 5.0]].iter().enumerate().map(|(site, &at)| plan::Via { net: 1, site, at }).collect();
    let drawing = Drawing {
        wires: vec![wire(1, &[[7.0, 2.0], [7.0, 5.0]]), wire(1, &[[6.0, 14.0], [10.0, 10.0]]), wire(0, &[[7.0, 5.0], [12.0, 5.0], [12.0, 10.0], [10.0, 10.0]])],
        ..Default::default()
    };
    let (plan2, less, gone) = wire::hops::hops(&board, plan, drawing);
    assert_eq!((gone, plan2.vias.len()), (2, 0));
    let join = less.wires.iter().find(|w| w.pts.len() == 3 && w.pts.iter().any(|p| p == &[10.0, 10.0])).expect("the join");
    assert!(join.pts.contains(&[10.0, 8.0]), "straight up from the via's place first: {:?}", join.pts);
    assert_eq!(open(&board, &plan2, &less), 0);
}
