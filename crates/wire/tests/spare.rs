//! Copper a net has no need of: where a wire joins its net's copper a second time, one of the
//! two ways round is taken out, and everything stays joined.

use board::Board;
use wire::{Drawing, Wire};

/// A 20 by 20 mm board of two layers with pads of net A at `pads`: tracks 0.1 mm wide, vias
/// 0.3 mm across.
fn board(pads: &[[f64; 2]]) -> Board {
    let pads: String = pads.iter().map(|p| format!("(pad \"1\" smd rect (at {} {}) (size 0.6 0.6) (layers \"F.Cu\") (net 1 \"A\"))", p[0], p[1])).collect();
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

fn wire(layer: usize, width: f64, pts: &[[f64; 2]]) -> Wire {
    Wire { layer, strand: 0, net: 1, width, pts: pts.to_vec(), as_planned: false }
}

fn plan(board: &Board, vias: &[[f64; 2]]) -> plan::Plan {
    let mut plan = plan::start(board);
    plan.vias = vias.iter().enumerate().map(|(site, &at)| plan::Via { net: 1, site, at }).collect();
    plan
}

fn length(wires: &[Wire]) -> f64 {
    wires.iter().flat_map(|w| w.pts.windows(2)).map(|s| (s[1][0] - s[0][0]).hypot(s[1][1] - s[0][1])).sum()
}

/// How many connections are open with the drawing on the board.
fn open(board: &Board, plan: &plan::Plan, drawing: &Drawing) -> usize {
    board::connectivity_without_pours(&wire::onto(board, plan, drawing).0).airlines.len()
}

/// A wire from a pad to a via, and a second wire from the same pad that runs through that
/// via on its way to another pad: the two shut in a ring between the pad and the via. The
/// longer way from the pad to the via is taken out, and all three stay joined. The wire that
/// is left runs through the via's copper 0.05 mm from its middle and nothing ends there: a
/// short piece from the wire to the via's middle joins the two, so that the via moved by
/// hand takes its track with it (owner, 2026-11-24: "sitting nearby and intersecting but
/// would not drag with it - needs a joining trace").
#[test]
fn a_wire_that_passes_a_via_of_its_net_leaves_no_ring_with_the_wire_to_that_via() {
    let board = board(&[[5.0, 10.0], [15.0, 10.0]]);
    let plan = plan(&board, &[[8.0, 9.0]]);
    let to_via = wire(0, 0.1, &[[5.0, 10.0], [6.0, 9.0], [8.0, 9.0]]);
    let past = wire(0, 0.1, &[[5.0, 10.0], [7.0, 10.0], [8.0, 9.05], [9.0, 10.0], [15.0, 10.0]]);
    let drawing = Drawing { wires: vec![to_via.clone(), past.clone()], ..Default::default() };
    assert_eq!(open(&board, &plan, &drawing), 0);
    let (less, taken) = wire::spare::spare(&board, &plan, &drawing, &[]);
    assert_eq!(taken, 1);
    assert_eq!(less.wires.len(), 2);
    assert_eq!(less.wires[0].pts, past.pts);
    assert_eq!(less.wires[1].pts, vec![[8.0, 9.05], [8.0, 9.0]], "from the wire to the via's middle");
    assert_eq!(less.spare.len(), 1);
    assert_eq!(less.spare[0].pts, to_via.pts);
    assert_eq!(open(&board, &plan, &less), 0);
    // Left alone, the net is as it was.
    assert_eq!(wire::spare::spare(&board, &plan, &drawing, &[1]).1, 0);
    // And what is left has no ring in it, and is joined to its via already.
    let (again, taken) = wire::spare::spare(&board, &plan, &less, &[]);
    assert_eq!((taken, again.wires.len()), (0, 2));
}

/// The same with the wire to the via the wider of the two: the narrower way goes, and the
/// second wire starts where it comes into the via, with a short piece from there to the via's
/// middle.
#[test]
fn of_two_ways_round_the_narrower_is_taken_out() {
    let board = board(&[[5.0, 10.0], [15.0, 10.0]]);
    let plan = plan(&board, &[[8.0, 9.0]]);
    let to_via = wire(0, 0.2, &[[5.0, 10.0], [6.0, 9.0], [8.0, 9.0]]);
    let past = wire(0, 0.1, &[[5.0, 10.0], [7.0, 10.0], [8.0, 9.05], [9.0, 10.0], [15.0, 10.0]]);
    let drawing = Drawing { wires: vec![to_via.clone(), past], ..Default::default() };
    let (less, taken) = wire::spare::spare(&board, &plan, &drawing, &[]);
    assert_eq!(taken, 1);
    assert_eq!(less.wires.len(), 3);
    assert_eq!(less.wires[0].pts, to_via.pts);
    assert_eq!(less.wires[1].pts, vec![[8.0, 9.05], [9.0, 10.0], [15.0, 10.0]]);
    // And it is joined to the via's middle, which it stops 0.05 mm short of.
    assert_eq!(less.wires[2].pts, vec![[8.0, 9.05], [8.0, 9.0]]);
    assert_eq!(open(&board, &plan, &less), 0);
}

/// Two wires leave a pad along the same line and part: the second bends away at a point on
/// the first. What they share is drawn once.
#[test]
fn two_wires_that_leave_a_pad_together_are_one_track_as_far_as_they_go_together() {
    let board = board(&[[5.0, 10.0], [12.0, 11.0], [12.0, 9.0]]);
    let plan = plan(&board, &[]);
    let one = wire(0, 0.1, &[[5.0, 10.0], [9.0, 10.0], [10.0, 11.0], [12.0, 11.0]]);
    let two = wire(0, 0.1, &[[5.0, 10.0], [8.5, 10.0], [9.5, 9.0], [12.0, 9.0]]);
    let drawing = Drawing { wires: vec![one, two], ..Default::default() };
    assert_eq!(open(&board, &plan, &drawing), 0);
    let (less, taken) = wire::spare::spare(&board, &plan, &drawing, &[]);
    assert_eq!(taken, 1);
    assert!((length(&drawing.wires) - length(&less.wires) - 3.5).abs() < 1e-9);
    assert!((length(&less.spare) - 3.5).abs() < 1e-9);
    assert_eq!(open(&board, &plan, &less), 0);
}

/// Wires that join nothing twice stay as they are.
#[test]
fn a_net_with_no_ring_is_left_as_it_is() {
    let board = board(&[[5.0, 10.0], [12.0, 11.0], [12.0, 9.0]]);
    let plan = plan(&board, &[]);
    let drawing = Drawing { wires: vec![wire(0, 0.1, &[[5.0, 10.0], [12.0, 11.0]]), wire(0, 0.1, &[[5.0, 10.0], [12.0, 9.0]])], ..Default::default() };
    let (less, taken) = wire::spare::spare(&board, &plan, &drawing, &[]);
    assert_eq!(taken, 0);
    assert_eq!(less.wires.len(), 2);
    assert!(less.spare.is_empty());
}

/// Two pads joined on the top layer, and again by two vias and a long track underneath. The
/// long track is the longest way round, but without it each via has one track and leads
/// nowhere: the track on top goes instead.
#[test]
fn a_via_is_not_left_with_one_track_where_another_way_can_go() {
    let board = board(&[[5.0, 10.0], [15.0, 10.0]]);
    let plan = plan(&board, &[[5.0, 12.0], [15.0, 12.0]]);
    let on_top = wire(0, 0.1, &[[5.0, 10.0], [15.0, 10.0]]);
    let wires = vec![
        wire(0, 0.1, &[[5.0, 10.0], [5.0, 12.0]]),
        wire(1, 0.1, &[[5.0, 12.0], [5.0, 16.0], [15.0, 16.0], [15.0, 12.0]]),
        wire(0, 0.1, &[[15.0, 12.0], [15.0, 10.0]]),
        on_top.clone(),
    ];
    let drawing = Drawing { wires, ..Default::default() };
    let (less, taken) = wire::spare::spare(&board, &plan, &drawing, &[]);
    assert_eq!(taken, 1);
    assert_eq!(less.spare.len(), 1);
    assert_eq!(less.spare[0].pts, on_top.pts);
    assert_eq!(open(&board, &plan, &less), 0);
}

/// A ring from a pad down through one via and up through another: via V1 also leads on along
/// the back to the far pad. Taking out the pad's track to V1 would leave V1 two tracks, both on
/// the back, and KiCad calls a via that joins one layer only dangling; taking out either of the
/// others would leave V2 one track. The ring stays.
#[test]
fn a_via_is_never_left_joining_one_layer_only() {
    let board = board(&[[5.0, 10.0], [15.0, 10.0]]);
    let plan = plan(&board, &[[7.0, 10.0], [7.0, 12.0], [11.0, 10.0]]);
    let wires = vec![
        wire(0, 0.1, &[[5.0, 10.0], [7.0, 10.0]]),
        wire(0, 0.1, &[[5.0, 10.0], [7.0, 12.0]]),
        wire(1, 0.1, &[[7.0, 10.0], [7.0, 12.0]]),
        wire(1, 0.1, &[[7.0, 10.0], [11.0, 10.0]]),
        wire(0, 0.1, &[[11.0, 10.0], [15.0, 10.0]]),
    ];
    let drawing = Drawing { wires, ..Default::default() };
    assert_eq!(open(&board, &plan, &drawing), 0);
    let (less, taken) = wire::spare::spare(&board, &plan, &drawing, &[]);
    assert_eq!(taken, 0, "{:?}", less.spare);
    assert_eq!(less.wires.len(), 5);
}

/// A wire on the front runs past a via of its net, 0.09 mm from its middle, and the via's own
/// track is on the back: the via and the wire are joined by nothing but lying on each other.
/// And a second track on the layer of one that ends at the via's middle, stopping in its
/// copper short of the middle. Each is joined to the middle by a short piece.
#[test]
fn every_wire_that_comes_into_a_via_and_does_not_end_at_its_middle_is_joined_to_it() {
    let board = board(&[[13.66, 11.94], [12.57, 9.0], [10.75, 11.61]]);
    let plan = plan(&board, &[[12.4795, 11.609656]]);
    let past = wire(0, 0.1, &[[13.663344, 11.936656], [12.777466, 11.936656], [12.571634, 11.730824], [12.571634, 9.0]]);
    let own = wire(1, 0.1, &[[12.4795, 11.609656], [10.75, 11.609656]]);
    let drawing = Drawing { wires: vec![past.clone(), own.clone()], ..Default::default() };
    let (less, taken) = wire::spare::spare(&board, &plan, &drawing, &[]);
    assert_eq!(taken, 0);
    assert_eq!(less.wires.len(), 3, "{:?}", less.wires.iter().map(|w| &w.pts).collect::<Vec<_>>());
    assert_eq!(less.wires[2].layer, 0);
    assert_eq!(less.wires[2].pts[1], [12.4795, 11.609656]);
    // One that ends at the middle and one that stops 0.1 mm short of it, both on the back.
    let short = wire(1, 0.1, &[[12.4795, 11.713], [13.2, 11.713]]);
    let drawing = Drawing { wires: vec![own, short], ..Default::default() };
    let (less, _) = wire::spare::spare(&board, &plan, &drawing, &[]);
    assert_eq!(less.wires.len(), 3, "{:?}", less.wires.iter().map(|w| &w.pts).collect::<Vec<_>>());
    assert_eq!(less.wires[2].pts, vec![[12.4795, 11.713], [12.4795, 11.609656]]);
}
