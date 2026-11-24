//! A triangle whose strands do not pair up is no way for another, and no end of the run.

use board::Board;

/// Two pads on an empty board. A strand is put across one side only of a free triangle that
/// has a pad's side for another: it goes in and never comes out, which no strand laid by the
/// plan does, but which the plan of parallelogram p2-132-usb6 came to 13 minutes into a run
/// (2026-11-24: how is not known). Asked how many strands pass a corner there, the sheet
/// took one count from another unguarded, had 2^63 for an answer, and the run stopped trying
/// to make room for as many. Now such a triangle has no way through it.
#[test]
fn a_triangle_whose_strands_do_not_pair_up_is_no_way_through() {
    let pad = |x: f64, y: f64, n: u32, net: &str| format!("(footprint \"P\" (layer \"F.Cu\") (at {x} {y}) (pad \"1\" smd rect (at 0 0) (size 1 1) (layers \"F.Cu\") (net {n} \"{net}\")))");
    let text = format!(
        "(kicad_pcb (version 20240108) (generator \"pcbnew\") (general (thickness 1.6))
         (layers (0 \"F.Cu\" signal) (31 \"B.Cu\" signal) (44 \"Edge.Cuts\" user))
         (net 0 \"\") (net 1 \"A\") (net 2 \"B\")
         (gr_rect (start 0 0) (end 10 10) (layer \"Edge.Cuts\") (width 0.1))
         {} {})",
        pad(3.0, 5.0, 1, "A"),
        pad(7.0, 5.0, 2, "B"),
    );
    let rules = r#"{"board":{"design_settings":{"rules":{"min_clearance":0.2,"min_copper_edge_clearance":0.3,"min_track_width":0.2}}},
        "net_settings":{"classes":[{"name":"Default","clearance":0.2,"track_width":0.2,"via_diameter":0.8,"via_drill":0.4}]}}"#;
    let (pcb, copper) = kicad::parse_board(&text).unwrap();
    let board = Board::new(&pcb, &copper, kicad::Rules::parse(rules).unwrap());
    let mut plan = plan::start(&board);
    let sheet = &mut plan.sheets[0];
    // A free triangle with a side that has no free triangle beyond it, and another side that
    // has: the wall, and the gap the strand is put across.
    let free_beyond = |sheet: &plan::strands::Sheet, t: usize, e: usize| sheet.space.edges[e].t.iter().flatten().any(|&x| x != t && sheet.space.free(x));
    let (t, wall, gap) = (0..sheet.space.tris.len())
        .filter(|&t| sheet.space.free(t))
        .find_map(|t| {
            let sides = sheet.space.tris[t].e;
            let wall = (0..3).find(|&i| !free_beyond(sheet, t, sides[i]))?;
            let gap = (0..3).find(|&i| i != wall && free_beyond(sheet, t, sides[i]))?;
            Some((t, wall, gap))
        })
        .expect("a triangle against a wall");
    let side = sheet.space.tris[t].e[gap];
    let end = plan::End::Site(0);
    sheet.lay(1, 0, 0.2, end, end, &[(side, 0)]);
    assert!(!sheet.paired(t));
    assert!(!sheet.sound(&board));
    // Round the corner opposite the wall, and through the triangle from its gap.
    assert!(!sheet.throat(&board, t, wall, 2, 0.2));
    assert!(sheet.ways_on(t, side, 0).is_empty());
    assert_eq!(sheet.corners(t), [0; 3]);
}
