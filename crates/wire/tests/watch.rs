//! The picture of one strand's course (`WIRE_WATCH`) says under it what each mark stands for.

use board::Board;

/// A pad 1 mm square on the front.
fn pad(x: f64, y: f64, n: usize, net: &str) -> String {
    format!("(footprint \"P\" (layer \"F.Cu\") (at {x} {y}) (pad \"1\" smd rect (at 0 0) (size 1 1) (layers \"F.Cu\" \"F.Mask\") (net {n} \"{net}\")))")
}

/// Two pads of one net with a third between them: the one wire's picture has a key under it,
/// a line for each kind of mark, and the key says the wire was found. Before, the picture had
/// no words at all and its colours were told only in the code (the owner asked for a legend on
/// the BMS board's picture, 2026-11-23).
#[test]
fn the_picture_of_a_strands_course_says_what_its_marks_are() {
    let parts = [pad(5.0, 10.0, 1, "A"), pad(15.0, 10.0, 1, "A"), pad(10.0, 10.0, 2, "B")].concat();
    let text = format!(
        "(kicad_pcb (version 20240108) (generator \"pcbnew\") (general (thickness 1.6))
         (layers (0 \"F.Cu\" signal) (31 \"B.Cu\" signal) (38 \"B.Mask\" user) (39 \"F.Mask\" user) (44 \"Edge.Cuts\" user))
         (net 0 \"\") (net 1 \"A\") (net 2 \"B\")
         (gr_rect (start 0 0) (end 20 20) (layer \"Edge.Cuts\") (width 0.1))
         {parts})"
    );
    let rules = r#"{"board":{"design_settings":{"rules":{"min_clearance":0.2,"min_copper_edge_clearance":0.3,"min_track_width":0.2}}},
        "net_settings":{"classes":[{"name":"Default","clearance":0.2,"track_width":0.2,"via_diameter":0.8,"via_drill":0.4}]}}"#;
    let (pcb, copper) = kicad::parse_board(&text).unwrap();
    let board = Board::new(&pcb, &copper, kicad::Rules::parse(rules).unwrap());
    let plan = plan::plan(&board);
    let (drawing, picture) = wire::draw_watching(&board, &plan, Some((0, 0)));
    assert_eq!(drawing.wires.len(), 1);
    let picture = picture.expect("the strand watched has a picture");
    for says in ["pads and other copper in its way", "wires already drawn", "this connection as the plan has it", ">the wire drawn for it<", "its two ends", "the area it may bend in", "a corner on its left", "a corner on its right", "a side it keeps off", "a gap it goes through", "room in front of a corner"] {
        assert!(picture.contains(says), "the key has no line for: {says}");
    }
    // The key lies under the picture: each of its lines starts below every mark of the course.
    let foot = picture.lines().find(|l| l.contains("h") && l.starts_with("<path d='M") && l.contains("stroke='#999'") && !l.contains('L')).expect("the line across the picture's foot");
    let y: f64 = foot.split('\'').nth(1).and_then(|d| d.split_once(' ')).and_then(|(_, d)| d.split_once('h')).unwrap().0.parse().unwrap();
    assert!(y > 11.0, "the key starts at {y}, on the picture");
}
