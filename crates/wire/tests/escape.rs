//! The way out of a fine-pitch part, settled for the whole part: a row of pads has its vias
//! in a row, one for each pad, in the pads' order.

use board::Board;

/// A part with two rows of six pads 0.5 mm apart (a small-outline chip: the rows at y = 10 and
/// y = 14, pads 0.3 by 1 mm), each pad on its own net with a pad far off; and `extra`.
fn board(extra: &str) -> Board {
    let nets: String = (1..=12).map(|n| format!("(net {n} \"N{n}\")")).collect();
    let pads: String = (0..12)
        .map(|k| {
            let (x, y) = (-1.25 + 0.5 * (k % 6) as f64, if k < 6 { -2.0 } else { 2.0 });
            format!("(pad \"{}\" smd rect (at {x} {y}) (size 0.3 1) (layers \"F.Cu\") (net {} \"N{}\"))", k + 1, k + 1, k + 1)
        })
        .collect();
    let far: String = (0..12)
        .map(|k| {
            let (x, y) = (3.0 + 2.2 * (k % 6) as f64, if k < 6 { 2.0 } else { 22.0 });
            format!("(footprint \"P\" (layer \"F.Cu\") (at {x} {y}) (pad \"1\" smd rect (at 0 0) (size 1 1) (layers \"F.Cu\") (net {} \"N{}\")))", k + 1, k + 1)
        })
        .collect();
    let text = format!(
        "(kicad_pcb (version 20240108) (generator \"pcbnew\") (general (thickness 1.6))
         (layers (0 \"F.Cu\" signal) (31 \"B.Cu\" signal) (44 \"Edge.Cuts\" user))
         (net 0 \"\") {nets}
         (gr_rect (start 0 0) (end 20 24) (layer \"Edge.Cuts\") (width 0.1))
         (footprint \"U\" (layer \"F.Cu\") (at 10 12) (property \"Reference\" \"U1\") {pads})
         {far} {extra})"
    );
    let rules = r#"{"board":{"design_settings":{"rules":{"min_clearance":0.2,"min_copper_edge_clearance":0.3,"min_track_width":0.2}}},
        "net_settings":{"classes":[{"name":"Default","clearance":0.2,"track_width":0.25,"via_diameter":0.8,"via_drill":0.4}]}}"#;
    let (pcb, copper) = kicad::parse_board(&text).unwrap();
    Board::new(&pcb, &copper, kicad::Rules::parse(rules).unwrap())
}

/// The escape of `board`: its spots, and for each pad of U1 (by its place along x, top row
/// first) the spot its stub may end at.
fn escape(board: &Board) -> (plan::space::Spots, Vec<Vec<[f64; 2]>>, Vec<plan::Want>) {
    let wants = plan::wants(&board::connectivity_without_pours(board).airlines);
    let (spots, stubs) = plan::escape(board, &wants, plan::Fan::Spread);
    let mid = |id: board::ItemId| board.bbox(id).map(|b| [(b.min.x + b.max.x) as f64 / 2e6, (b.min.y + b.max.y) as f64 / 2e6]).unwrap();
    let mut rows = vec![Vec::new(), Vec::new()];
    let mut by_x: Vec<&plan::Want> = stubs.iter().collect();
    by_x.sort_by(|a, b| mid(a.from)[0].total_cmp(&mid(b.from)[0]));
    for stub in by_x {
        assert!(stub.stub && stub.from == stub.to);
        // Its own place is the first; the rest are any that are within reach.
        rows[usize::from(mid(stub.from)[1] > 12.0)].push(spots.at[stub.into[0]]);
    }
    (spots, rows, stubs)
}

#[test]
fn a_row_of_pads_has_its_vias_in_a_row_in_the_pads_order() {
    let board = board("");
    let (spots, rows, stubs) = escape(&board);
    assert_eq!(stubs.len(), 12);
    for (row, out) in rows.iter().zip([-1.0, 1.0]) {
        assert_eq!(row.len(), 6);
        // In a line, out from the part past the pads' ends, a via's pitch apart, in order.
        assert!(row.iter().all(|p| (p[1] - row[0][1]).abs() < 1e-9));
        assert!((row[0][1] - 12.0) * out > 2.5 + spots.radius, "{row:?}");
        assert!(row.windows(2).all(|w| w[1][0] - w[0][0] >= spots.apart), "{row:?}");
        // Spread evenly about the row of pads, which is far narrower.
        assert!((row[0][0] + row[5][0] - 20.0).abs() < 1e-6 && row[5][0] - row[0][0] > 5.0, "{row:?}");
    }
    // No other via may stand among a row's stubs.
    let own: Vec<[f64; 2]> = rows.concat();
    for p in spots.at.iter().filter(|p| !own.contains(p)) {
        let among = (p[0] - 10.0).abs() < 2.5 && (p[1] - 12.0).abs() > 2.5 && (p[1] - 12.0).abs() < (rows[1][0][1] - 12.0).abs();
        assert!(!among, "{p:?}");
    }
}

#[test]
fn the_stubs_fan_out_and_draw() {
    let board = board("");
    let wants = plan::wants(&board::connectivity_without_pours(&board).airlines);
    let (spots, stubs) = plan::escape(&board, &wants, plan::Fan::Spread);
    let mut plan = plan::start_from(&board, spots);
    plan::add(&board, &mut plan, stubs);
    assert!(plan.left.is_empty());
    assert_eq!(plan.vias.len(), 12, "a via for every pad");
    let drawing = wire::draw(&board, &plan);
    assert!(drawing.undrawn.is_empty(), "{:?}", drawing.undrawn.iter().map(|u| &u.why).collect::<Vec<_>>());
    assert_eq!(drawing.wires.len(), 12);
}

#[test]
fn a_row_of_vias_stands_nearer_or_farther_where_something_is_in_its_way() {
    // A pad of no net right where the top row's vias would stand.
    let open = escape(&board("")).1;
    let y = open[0][0][1];
    let blocker = format!("(footprint \"P\" (layer \"F.Cu\") (at 10 {y}) (pad \"1\" smd rect (at 0 0) (size 2 0.3) (layers \"F.Cu\")))");
    let (_, rows, stubs) = escape(&board(&blocker));
    assert_eq!(stubs.len(), 12);
    assert_eq!(rows[0].len(), 6, "all six still have their own place");
    assert!((rows[0][0][1] - y).abs() > 0.05, "moved off: {:?}", rows[0]);
    assert!(rows[0].iter().all(|p| (p[1] - rows[0][0][1]).abs() < 1e-9));
    assert_eq!(rows[1], open[1], "the other row is where it was");
}

/// A fine-pitch row as on a QFN: eight pads 0.4 mm apart (0.2 by 0.8 mm, at y = 10), each on
/// its own net with a pad far off, under four-layer fab rules (0.1 mm clearance, 0.3/0.15 mm
/// vias, 0.2 mm from a hole).
fn fine() -> Board {
    let nets: String = (1..=8).map(|n| format!("(net {n} \"N{n}\")")).collect();
    let pads: String = (0..8).map(|k| format!("(pad \"{}\" smd rect (at {} 0) (size 0.2 0.8) (layers \"F.Cu\") (net {} \"N{}\"))", k + 1, -1.4 + 0.4 * k as f64, k + 1, k + 1)).collect();
    let far: String = (0..8)
        .map(|k| format!("(footprint \"P\" (layer \"F.Cu\") (at {} 3) (pad \"1\" smd rect (at 0 0) (size 0.6 0.6) (layers \"F.Cu\") (net {} \"N{}\")))", 3.0 + 2.0 * k as f64, k + 1, k + 1))
        .collect();
    // A second pad beside the row's middle, off the row's line, so that the row is one side of
    // a part whose middle is above it: its vias go down.
    let text = format!(
        "(kicad_pcb (version 20240108) (generator \"pcbnew\") (general (thickness 1.6))
         (layers (0 \"F.Cu\" signal) (1 \"In1.Cu\" signal) (2 \"In2.Cu\" signal) (31 \"B.Cu\" signal) (44 \"Edge.Cuts\" user))
         (net 0 \"\") {nets}
         (gr_rect (start 0 0) (end 20 20) (layer \"Edge.Cuts\") (width 0.1))
         (footprint \"U\" (layer \"F.Cu\") (at 10 10) (property \"Reference\" \"U1\") {pads}
           (pad \"9\" smd rect (at 0 -2) (size 1 1) (layers \"F.Cu\")))
         {far})"
    );
    let rules = r#"{"board":{"design_settings":{"rules":{"min_clearance":0.1,"min_copper_edge_clearance":0.3,"min_track_width":0.1,"min_hole_clearance":0.2,"min_hole_to_hole":0.25,"min_via_diameter":0.3,"min_through_hole_diameter":0.15}}},
        "net_settings":{"classes":[{"name":"Default","clearance":0.1,"track_width":0.1,"via_diameter":0.3,"via_drill":0.15}]}}"#;
    let (pcb, copper) = kicad::parse_board(&text).unwrap();
    Board::new(&pcb, &copper, kicad::Rules::parse(rules).unwrap())
}

#[test]
fn a_fine_pitch_row_has_its_vias_at_its_pads_tips_staggered() {
    let board = fine();
    let wants = plan::wants(&board::connectivity_without_pours(&board).airlines);
    let (spots, stubs) = plan::escape(&board, &wants, plan::Fan::Tight);
    assert_eq!(stubs.len(), 8);
    let mid = |id: board::ItemId| board.bbox(id).map(|b| [(b.min.x + b.max.x) as f64 / 2e6, (b.min.y + b.max.y) as f64 / 2e6]).unwrap();
    let mut row: Vec<([f64; 2], [f64; 2])> = stubs.iter().map(|s| (mid(s.from), spots.at[s.into[0]])).collect();
    row.sort_by(|a, b| a.0[0].total_cmp(&b.0[0]));
    let tip = 10.4 + spots.radius;
    for (k, (pad, via)) in row.iter().enumerate() {
        // Straight out from its own pad, away from the part's middle, its copper at the tip.
        assert!((via[0] - pad[0]).abs() < 1e-6, "{k}: {via:?} off its pad {pad:?}");
        assert!(via[1] >= tip - 1e-6 && via[1] < tip + 0.5, "{k}: {via:?}");
    }
    // Every other one farther out, and each a via's pitch from the rest.
    assert!(row.windows(2).all(|w| (w[0].1[1] - w[1].1[1]).abs() > 0.1), "{row:?}");
    for (i, a) in row.iter().enumerate() {
        for b in &row[i + 1..] {
            assert!(((a.1[0] - b.1[0]).powi(2) + (a.1[1] - b.1[1]).powi(2)).sqrt() >= spots.apart - 1e-6, "{a:?} {b:?}");
        }
    }
    // And the plan lays and draws every stub.
    let mut plan = plan::start_from(&board, spots);
    plan::add(&board, &mut plan, stubs);
    assert!(plan.left.is_empty());
    assert_eq!(plan.vias.len(), 8);
    let drawing = wire::draw(&board, &plan);
    assert!(drawing.undrawn.is_empty(), "{:?}", drawing.undrawn.iter().map(|u| &u.why).collect::<Vec<_>>());
}
