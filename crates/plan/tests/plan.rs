//! The plan's own rules, on a board from `fixtures/`: strands never cross, no gap holds more
//! than its room, and lifting a strand leaves the sheet as it was.

use std::path::PathBuf;

use plan::{plan, strip, Plan};

/// The fixture planned once for every test: planning it takes most of a minute.
fn planned() -> (board::Board, Plan) {
    static ONCE: std::sync::OnceLock<(board::Board, Plan)> = std::sync::OnceLock::new();
    ONCE.get_or_init(|| {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/earring.kicad_pcb");
        let (mut board, _) = board::Board::load(&path).unwrap();
        strip(&mut board);
        let plan = plan(&board);
        (board, plan)
    })
    .clone()
}

#[test]
fn some_connections_are_planned_and_every_other_is_reported() {
    let (board, plan) = planned();
    let laid: usize = plan.sheets.iter().map(|s| s.laid().count()).sum();
    assert!(laid > 0, "nothing planned");
    // A connection with vias is several strands.
    assert!(laid + plan.left.len() >= board::connectivity(&board).airlines.len());
}

/// In a triangle strands are chords that each cut one corner off, so the counts on its three
/// sides must be those of some number of chords round each corner: no side holds more than the
/// other two together, and the total is even.
#[test]
fn strands_in_a_triangle_are_nested_chords() {
    let (_, plan) = planned();
    for sheet in &plan.sheets {
        for (t, tri) in sheet.space.tris.iter().enumerate() {
            if !sheet.space.free(t) {
                continue;
            }
            let n = tri.e.map(|e| sheet.on[e].len());
            assert_eq!((n[0] + n[1] + n[2]) % 2, 0, "triangle {t}: {n:?}");
            assert!(n[0] <= n[1] + n[2] && n[1] <= n[0] + n[2] && n[2] <= n[0] + n[1], "triangle {t}: {n:?}");
        }
    }
}

#[test]
fn no_gap_is_over_its_room() {
    let (board, plan) = planned();
    for sheet in &plan.sheets {
        for e in 0..sheet.space.edges.len() {
            let both_free = sheet.space.edges[e].t.iter().all(|t| t.is_some_and(|t| sheet.space.free(t)));
            if both_free && !sheet.space.scaffold(e) {
                assert!(sheet.fill(&board, e) <= 1.0 + 1e-6, "edge {e} is {:.3} full", sheet.fill(&board, e));
            }
        }
    }
}

#[test]
fn a_lifted_strand_leaves_no_crossing() {
    let (_, mut plan) = planned();
    for sheet in &mut plan.sheets {
        for k in sheet.laid().collect::<Vec<_>>() {
            sheet.lift(k);
        }
        assert!(sheet.on.iter().all(Vec::is_empty));
    }
}
