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
                let what = |v: usize| sheet.space.vert_of[v].map(|o| (sheet.space.obstacles[o].what, sheet.space.obstacles[o].net));
                let [a, b] = sheet.space.edges[e].v;
                assert!(sheet.fill(&board, e) <= 1.0 + 1e-6, "edge {e} is {:.3} full: {:.3} mm between {:?} and {:?}, strands of nets {:?}", sheet.fill(&board, e), sheet.space.edges[e].len, what(a), what(b), sheet.nets_on(e));
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

/// Settling the plan takes the unused via sites out of every layer and carries the strands
/// over: those that come through are still nested chords in every triangle, and each is either
/// carried or counted as lost.
#[test]
fn a_settled_plan_keeps_its_strands_nested() {
    let (board, plan) = planned();
    let (settled, what) = plan::settle(&board, &plan);
    let before: usize = plan.sheets.iter().map(|s| s.laid().count()).sum();
    let after: usize = settled.sheets.iter().map(|s| s.laid().count()).sum();
    assert_eq!(after + what.lost, before);
    assert!(after * 10 >= before * 9, "{} of {before} strands lost", what.lost);
    for sheet in &settled.sheets {
        assert!(sheet.space.sites.len() == settled.vias.len());
        for (t, tri) in sheet.space.tris.iter().enumerate() {
            if !sheet.space.free(t) {
                continue;
            }
            let l = tri.e.map(|e| sheet.on[e].len());
            assert!((l[0] + l[1] + l[2]) % 2 == 0 && l[0] <= l[1] + l[2] && l[1] <= l[0] + l[2] && l[2] <= l[0] + l[1], "triangle {t}: {l:?}");
        }
    }
}
