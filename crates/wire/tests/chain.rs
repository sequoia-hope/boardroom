//! The chain search on courses made by hand: pieces only in the eight directions, as few bends
//! as the course allows, each corner passed no nearer than it may be, and each gap crossed
//! where the wire may cross it.

use wire::chain::{chain, Corner, Course, Gap, Stage};

type P = [f64; 2];

fn in_eight_directions(pts: &[P]) -> bool {
    pts.windows(2).all(|w| {
        let (dx, dy) = ((w[1][0] - w[0][0]).abs(), (w[1][1] - w[0][1]).abs());
        dx < 1e-6 || dy < 1e-6 || (dx - dy).abs() < 1e-6
    })
}

fn nearest(pts: &[P], p: P) -> f64 {
    pts.windows(2)
        .map(|w| {
            let d = [w[1][0] - w[0][0], w[1][1] - w[0][1]];
            let l2 = d[0] * d[0] + d[1] * d[1];
            let t = (((p[0] - w[0][0]) * d[0] + (p[1] - w[0][1]) * d[1]) / l2).clamp(0.0, 1.0);
            ((p[0] - w[0][0] - d[0] * t).powi(2) + (p[1] - w[0][1] - d[1] * t).powi(2)).sqrt()
        })
        .fold(f64::MAX, f64::min)
}

fn open(a: P, b: P) -> Course {
    Course { a, b, stages: vec![Stage::default()], least: 0.1, ..Default::default() }
}

#[test]
fn two_ends_in_line_are_one_piece() {
    let path = chain(&open([0.0, 0.0], [10.0, 0.0]), 12).unwrap();
    assert_eq!(path.pts.len(), 2);
}

#[test]
fn two_ends_out_of_line_take_one_bend() {
    let path = chain(&open([0.0, 0.0], [10.0, 4.0]), 12).unwrap();
    assert_eq!(path.pts.len(), 3, "{:?}", path.pts);
    assert!(in_eight_directions(&path.pts));
}

/// A wire told to leave and arrive along the first axis, between ends that are not in line,
/// takes two bends: out, across at 45 degrees, and in.
#[test]
fn ends_left_and_come_to_along_an_axis_take_two_bends() {
    let course = Course { out: vec![0, 4], into: vec![0, 4], stub: [0.5, 0.5], ..open([0.0, 0.0], [10.0, 4.0]) };
    let path = chain(&course, 12).unwrap();
    assert_eq!(path.pts.len(), 4, "{:?}", path.pts);
    assert!(in_eight_directions(&path.pts));
    assert!((path.pts[1][1]).abs() < 1e-9 && (path.pts[2][1] - 4.0).abs() < 1e-9);
}

/// A gap one unit wide at x = 5 between y = -1 and y = 1, below both ends, each corner kept
/// 0.3 from: the wire goes down to it, through it no nearer either corner than it may, and up
/// again.
#[test]
fn a_gap_out_of_the_way_is_crossed_between_its_corners() {
    let (low, high) = ([5.0, -1.0], [5.0, 1.0]);
    let corners = vec![Corner { at: low, r: 0.3, left: false }, Corner { at: high, r: 0.3, left: true }];
    let course = Course {
        gaps: vec![Gap { ends: [low, high], lo: 0.15, hi: 0.85, forward: [1.0, 0.0], mid: [5.0, 0.0] }],
        stages: vec![Stage { corners: corners.clone(), walls: Vec::new() }, Stage { corners, walls: Vec::new() }],
        ..open([0.0, 6.0], [10.0, 6.0])
    };
    let path = chain(&course, 12).unwrap();
    assert!(in_eight_directions(&path.pts), "{:?}", path.pts);
    assert!(nearest(&path.pts, low) >= 0.3 - 1e-6 && nearest(&path.pts, high) >= 0.3 - 1e-6, "{:?}", path.pts);
    // It crosses x = 5 between the corners.
    let across = path.pts.windows(2).find(|w| (w[0][0] - 5.0) * (w[1][0] - 5.0) <= 0.0).unwrap();
    let y = across[0][1] + (across[1][1] - across[0][1]) * (5.0 - across[0][0]) / (across[1][0] - across[0][0]).abs().max(1e-12) * (across[1][0] - across[0][0]).signum();
    assert!(y > -1.0 && y < 1.0, "crosses at y = {y}: {:?}", path.pts);
}

/// A gap too narrow for the wire: no chain, and the search says it got across no gap.
#[test]
fn a_gap_too_narrow_stops_the_wire() {
    let (low, high) = ([5.0, -0.2], [5.0, 0.2]);
    let corners = vec![Corner { at: low, r: 0.3, left: false }, Corner { at: high, r: 0.3, left: true }];
    let course = Course {
        gaps: vec![Gap { ends: [low, high], lo: 0.75, hi: 0.25, forward: [1.0, 0.0], mid: [5.0, 0.0] }],
        stages: vec![Stage { corners: corners.clone(), walls: Vec::new() }, Stage { corners, walls: Vec::new() }],
        ..open([0.0, 0.0], [10.0, 0.0])
    };
    assert_eq!(chain(&course, 12).err(), Some(0));
}

/// Two wires past the same corner, the second held one track further out: they turn together
/// and never come nearer than the pitch between them.
#[test]
fn two_wires_round_one_corner_are_nested() {
    let corner = [5.0, 0.0];
    let wire = |r: f64, a: P, b: P| {
        let gap = Gap { ends: [corner, [5.0, -10.0]], lo: r / 10.0, hi: 0.99, forward: [1.0, 0.0], mid: [5.0, -r] };
        let corners = vec![Corner { at: corner, r, left: true }];
        let course = Course { gaps: vec![gap], stages: vec![Stage { corners: corners.clone(), walls: Vec::new() }, Stage { corners, walls: Vec::new() }], ..open(a, b) };
        chain(&course, 12).unwrap().pts
    };
    let inner = wire(0.3, [0.0, 2.0], [10.0, 2.0]);
    let outer = wire(0.7, [0.0, 2.6], [10.0, 2.6]);
    assert!(in_eight_directions(&inner) && in_eight_directions(&outer));
    for p in &outer {
        assert!(nearest(&inner, *p) >= 0.4 - 1e-6, "outer {outer:?} comes within the pitch of inner {inner:?}");
    }
    for p in &inner {
        assert!(nearest(&outer, *p) >= 0.4 - 1e-6, "inner {inner:?} comes within the pitch of outer {outer:?}");
    }
}
