//! A board whose edge is not drawn as a closed loop still has room to route in: the least
//! convex shape round its edge's lines and everything on it. And what of its edge does close
//! is taken: an outline with an end a hair short of the next line's, and the cut-outs in it.

use board::Board;

/// Two pads of net A either side of a locked track of net B, on a board whose edge is drawn
/// as `edge`.
fn board(edge: &str) -> Board {
    let text = format!(
        "(kicad_pcb (version 20240108) (generator \"pcbnew\") (general (thickness 1.6))
         (layers (0 \"F.Cu\" signal) (31 \"B.Cu\" signal) (44 \"Edge.Cuts\" user))
         (net 0 \"\") (net 1 \"A\") (net 2 \"B\")
         {edge}
         (footprint \"P\" (layer \"F.Cu\") (at 5 10) (pad \"1\" smd rect (at 0 0) (size 1 1) (layers \"F.Cu\") (net 1 \"A\")))
         (footprint \"P\" (layer \"F.Cu\") (at 15 10) (pad \"1\" smd rect (at 0 0) (size 1 1) (layers \"F.Cu\") (net 1 \"A\")))
         (segment (start 10 5) (end 10 15) (width 0.25) (layer \"F.Cu\") (net 2) (locked yes) (uuid \"5e000000-0000-4000-8000-000000000001\")))"
    );
    let rules = r#"{"board":{"design_settings":{"rules":{"min_clearance":0.2,"min_copper_edge_clearance":0.3,"min_track_width":0.2}}},
        "net_settings":{"classes":[{"name":"Default","clearance":0.2,"track_width":0.25,"via_diameter":0.8,"via_drill":0.4}]}}"#;
    let (pcb, copper) = kicad::parse_board(&text).unwrap();
    Board::new(&pcb, &copper, kicad::Rules::parse(rules).unwrap())
}

fn line(a: (f64, f64), b: (f64, f64)) -> String {
    format!("(gr_line (start {} {}) (end {} {}) (layer \"Edge.Cuts\") (width 0.1))", a.0, a.1, b.0, b.1)
}

/// The box round a board's bounds, mm: its least and greatest corners.
fn box_of(board: &Board) -> [f64; 4] {
    let bounds = plan::space::bounds(board);
    assert_eq!(bounds.len(), 1);
    let (xs, ys): (Vec<f64>, Vec<f64>) = bounds[0].iter().map(|p| (p.x as f64 / 1e6, p.y as f64 / 1e6)).unzip();
    let (least, most) = (|v: &[f64]| v.iter().copied().fold(f64::MAX, f64::min), |v: &[f64]| v.iter().copied().fold(f64::MIN, f64::max));
    [least(&xs), least(&ys), most(&xs), most(&ys)]
}

#[test]
fn a_board_whose_edge_does_not_close_is_routed_in_the_box_round_it() {
    // Three sides of a 20 mm square, and the fourth in two pieces that overlap and so are no
    // loop (LadyBugShield's edge).
    let edge = [line((0.0, 0.0), (0.0, 20.0)), line((0.0, 20.0), (1.0, 20.0)), line((0.5, 20.0), (20.0, 20.0)), line((20.0, 20.0), (20.0, 0.0)), line((20.0, 0.0), (0.0, 0.0))].concat();
    let board = board(&edge);
    assert!(board.outline.is_empty(), "the edge is no closed loop");
    assert_eq!(box_of(&board), [0.0, 0.0, 20.0, 20.0]);
    // The wire goes round the end of the track between its pads, or under it: outside the
    // pads and the track there would be no room for either.
    let (plan, drawing) = wire::route(&board);
    assert!(plan.left.is_empty(), "{:?}", plan.left);
    assert!(drawing.undrawn.is_empty());
    assert_eq!(wire::open(&board, &plan, &drawing), 0);
}

#[test]
fn a_board_with_no_edge_drawn_reaches_a_little_past_what_is_on_it() {
    let board = board("");
    // The pads' outer sides are at 4.5 and 15.5, the track's ends at 5 and 15 less its half
    // width; 2 mm more all round.
    let b = box_of(&board);
    assert!((b[0] - 2.5).abs() < 1e-6 && (b[2] - 17.5).abs() < 1e-6 && (b[1] - 2.875).abs() < 1e-6 && (b[3] - 17.125).abs() < 1e-6, "{b:?}");
    let (plan, drawing) = wire::route(&board);
    assert!(plan.left.is_empty() && drawing.undrawn.is_empty());
    assert_eq!(wire::open(&board, &plan, &drawing), 0);
}

#[test]
fn a_closed_edge_is_the_bounds() {
    let edge = "(gr_rect (start 0 0) (end 20 20) (layer \"Edge.Cuts\") (width 0.1))";
    assert_eq!(box_of(&board(edge)), [0.0, 0.0, 20.0, 20.0]);
    assert_eq!(plan::space::bounds(&board(edge)), board(edge).outline);
}

#[test]
fn an_edge_that_does_not_close_bounds_the_board_by_its_own_lines() {
    // A board with a corner cut off, its edge drawn with a gap in it: the bounds follow the
    // cut, not the box round it, so that no wire is planned where the checker will refuse it.
    let edge = [line((0.0, 0.0), (14.0, 0.0)), line((14.0, 0.0), (20.0, 6.0)), line((20.0, 6.0), (20.0, 20.0)), line((20.0, 20.0), (0.0, 20.0)), line((0.0, 19.0), (0.0, 0.0))].concat();
    let board = board(&edge);
    assert!(board.outline.is_empty());
    let bounds = plan::space::bounds(&board);
    let mut corners: Vec<(f64, f64)> = bounds[0].iter().map(|p| (p.x as f64 / 1e6, p.y as f64 / 1e6)).collect();
    corners.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(corners, [(0.0, 0.0), (0.0, 20.0), (14.0, 0.0), (20.0, 6.0), (20.0, 20.0)]);
}

/// A square from `a` to `b`, drawn as four lines.
fn square(a: (f64, f64), b: (f64, f64)) -> String {
    [line(a, (b.0, a.1)), line((b.0, a.1), b), line(b, (a.0, b.1)), line((a.0, b.1), a)].concat()
}

/// Whether any wire of `drawing` has a corner strictly inside the box from `a` to `b`, mm, or
/// crosses it from side to side.
fn enters(drawing: &wire::Drawing, a: (f64, f64), b: (f64, f64)) -> bool {
    drawing.wires.iter().any(|w| {
        w.pts.windows(2).any(|s| {
            (0..=20).any(|k| {
                let t = k as f64 / 20.0;
                let (x, y) = (s[0][0] + (s[1][0] - s[0][0]) * t, s[0][1] + (s[1][1] - s[0][1]) * t);
                x > a.0 && x < b.0 && y > a.1 && y < b.1
            })
        })
    })
}

#[test]
fn an_edge_a_hair_short_of_closing_is_the_outline_with_its_cut_outs() {
    // A 20 mm square with one corner notched, its last line ending 0.05 mm short of the
    // first's start (the board model takes no loop from it), and a cut-out 2 mm wide and 8 mm
    // high in it: the bounds are the outline, by its own six corners, notch and all, and the
    // cut-out.
    let outline = |short: f64| [line((0.0, 0.0), (0.0, 20.0)), line((0.0, 20.0), (20.0, 20.0)), line((20.0, 20.0), (20.0, 5.0)), line((20.0, 5.0), (15.0, 5.0)), line((15.0, 5.0), (15.0, 0.0)), line((15.0, 0.0), (short, 0.0))].concat();
    let edge = [outline(0.05), square((7.0, 6.0), (9.0, 14.0))].concat();
    let short = board(&edge);
    assert!(short.outline.is_empty(), "the edge is no closed loop to the board model");
    let bounds = plan::space::bounds(&short);
    assert_eq!(bounds.len(), 2, "the outline and the cut-out");
    let boxes: Vec<[f64; 4]> = bounds
        .iter()
        .map(|l| {
            let (xs, ys): (Vec<f64>, Vec<f64>) = l.iter().map(|p| (p.x as f64 / 1e6, p.y as f64 / 1e6)).unzip();
            [xs.iter().copied().fold(f64::MAX, f64::min), ys.iter().copied().fold(f64::MAX, f64::min), xs.iter().copied().fold(f64::MIN, f64::max), ys.iter().copied().fold(f64::MIN, f64::max)]
        })
        .collect();
    assert_eq!((bounds[0].len(), bounds[1].len()), (6, 4));
    assert_eq!(boxes, vec![[0.0, 0.0, 20.0, 20.0], [7.0, 6.0, 9.0, 14.0]]);
    // A gap copper could pass, 0.5 mm, is no such thing: the edge is open, and the room is
    // the shape round everything, the notch filled, with the cut-out in it all the same.
    let open = [outline(0.5), square((7.0, 6.0), (9.0, 14.0))].concat();
    let bounds = plan::space::bounds(&board(&open));
    assert_eq!((bounds.len(), bounds[0].len()), (2, 5));
}

#[test]
fn an_edge_that_branches_still_gives_its_cut_outs() {
    // The same square, closed, with a fifth line out of one corner: three lines meet there,
    // and neither the board model nor the walk round it takes it for a loop. The room is the
    // shape round everything, and the cut-out, which does close, is still kept out of.
    let edge = [square((0.0, 0.0), (20.0, 20.0)), line((0.0, 0.0), (0.0, -1.0)), square((7.0, 6.0), (9.0, 14.0))].concat();
    let board = board(&edge);
    assert!(board.outline.is_empty(), "the edge is no closed loop to the board model");
    let bounds = plan::space::bounds(&board);
    assert_eq!(bounds.len(), 2, "the shape round everything and the cut-out");
    let (plan, drawing) = wire::route(&board);
    assert!(plan.left.is_empty() && drawing.undrawn.is_empty(), "left {:?}, undrawn {}", plan.left, drawing.undrawn.len());
    assert_eq!(wire::open(&board, &plan, &drawing), 0);
    assert!(!enters(&drawing, (7.0, 6.0), (9.0, 14.0)), "a wire crosses the cut-out");
}

#[test]
fn a_loop_with_pads_in_it_is_no_cut_out() {
    // Two boards of a panel side by side, a pad in each: neither is the outline of the whole,
    // and neither is a hole in it. The room is the shape round both.
    let panel = [square((0.0, 0.0), (9.5, 20.0)), square((10.5, 0.0), (20.0, 20.0))].concat();
    assert_eq!(box_of(&board(&panel)), [0.0, 0.0, 20.0, 20.0]);
    // And a square drawn round one pad of a board whose outline branches.
    let round = [square((0.0, 0.0), (20.0, 20.0)), line((0.0, 0.0), (0.0, -1.0)), square((3.0, 8.0), (7.0, 12.0))].concat();
    assert_eq!(plan::space::bounds(&board(&round)).len(), 1);
}

#[test]
fn an_outline_of_many_short_lines_is_taken_with_fewer_corners() {
    // A circle of 12 mm radius as 720 lines, the last a hair short of the first: the bounds
    // follow it to a hundredth of a millimetre with a fraction of the corners.
    let at = |k: usize, short: f64| {
        let a = std::f64::consts::TAU * k as f64 / 720.0 - short;
        (10.0 + 12.0 * a.cos(), 10.0 + 12.0 * a.sin())
    };
    let edge: String = (0..720).map(|k| line(at(k, 0.0), at(k + 1, if k == 719 { 0.002 } else { 0.0 }))).collect();
    let board = board(&edge);
    assert!(board.outline.is_empty(), "the edge is no closed loop to the board model");
    let bounds = plan::space::bounds(&board);
    assert_eq!(bounds.len(), 1);
    assert!(bounds[0].len() > 30 && bounds[0].len() < 200, "{} corners", bounds[0].len());
    for p in &bounds[0] {
        let r = (p.x as f64 / 1e6 - 10.0).hypot(p.y as f64 / 1e6 - 10.0);
        assert!((r - 12.0).abs() < 0.011, "a corner {r} mm from the middle");
    }
}

#[test]
fn an_arc_a_hair_off_the_line_it_meets_still_closes_the_outline() {
    // A 20 mm square with one corner rounded inwards, a quarter circle of 5 mm about the
    // corner, and the line that meets the arc starting 0.03 mm from the arc's end (espalarm's
    // edge, of the gate boards). The checker follows an arc on both sides; the outline is
    // one loop all the same, the square less the quarter circle.
    let arc = "(gr_arc (start 20 5) (mid 16.464466 3.535534) (end 15 0) (layer \"Edge.Cuts\") (width 0.1))";
    let edge = [line((0.0, 0.0), (0.0, 20.0)), line((0.0, 20.0), (20.0, 20.0)), line((20.0, 20.0), (20.0, 5.0)), arc.to_string(), line((14.97, 0.0), (0.0, 0.0))].concat();
    let board = board(&edge);
    assert!(board.outline.is_empty(), "the edge is no closed loop to the board model");
    let bounds = plan::space::bounds(&board);
    assert_eq!(bounds.len(), 1);
    let l = &bounds[0];
    let area = (0..l.len()).map(|i| l[i].x as f64 * l[(i + 1) % l.len()].y as f64 - l[(i + 1) % l.len()].x as f64 * l[i].y as f64).sum::<f64>().abs() / 2e12;
    let whole = 400.0 - std::f64::consts::PI * 25.0 / 4.0;
    assert!((area - whole).abs() < 0.3, "{area} mm2, not {whole}: the shape round everything is 387.5");
}
