//! The path search on courses made by hand: pieces only in the eight directions, as few bends
//! as the course allows, and each gap crossed between what stands on its two sides.

use wire::path::{fewest_bends, Con, Course, Gap};

type P = [f64; 2];

fn square(lo: P, hi: P) -> Vec<P> {
    vec![lo, [hi[0], lo[1]], hi, [lo[0], hi[1]]]
}

fn in_eight_directions(pts: &[P]) -> bool {
    pts.windows(2).all(|w| {
        let (dx, dy) = ((w[1][0] - w[0][0]).abs(), (w[1][1] - w[0][1]).abs());
        dx < 1e-6 || dy < 1e-6 || (dx - dy).abs() < 1e-6
    })
}

fn open(a: P, b: P) -> Course {
    Course { a, b, regions: vec![square([-20.0, -20.0], [20.0, 20.0])], gaps: Vec::new(), halves: vec![Vec::new()], starts: Vec::new(), ends: Vec::new(), least: 0.5, frame: 0.0 }
}

#[test]
fn two_ends_in_line_are_one_piece() {
    let path = fewest_bends(&open([0.0, 0.0], [10.0, 0.0])).unwrap();
    assert_eq!(path.pts.len(), 2);
}

#[test]
fn two_ends_out_of_line_take_one_bend() {
    let path = fewest_bends(&open([0.0, 0.0], [10.0, 4.0])).unwrap();
    assert_eq!(path.pts.len(), 3, "{:?}", path.pts);
    assert!(in_eight_directions(&path.pts));
}

/// A gap one unit wide at x = 5 between y = -1 and y = 1, below both ends: the wire goes down
/// to it, through it no nearer either corner than it may, and up again.
#[test]
fn a_gap_out_of_the_way_is_crossed_between_its_corners() {
    let (low, high) = ([5.0, -1.0], [5.0, 1.0]);
    let cons = vec![
        Con { p: high, r: 0.5, left: true, toward: [0.0, 1.0] },
        Con { p: [5.0, 0.5], r: 0.0, left: true, toward: [0.0, 1.0] },
        Con { p: low, r: 0.5, left: false, toward: [0.0, -1.0] },
        Con { p: [5.0, -0.5], r: 0.0, left: false, toward: [0.0, -1.0] },
    ];
    let course = Course {
        a: [0.0, 4.0],
        b: [10.0, 4.0],
        regions: vec![square([-5.0, -10.0], [5.0, 10.0]), square([5.0, -10.0], [15.0, 10.0])],
        gaps: vec![Gap { ends: [low, high], forward: [1.0, 0.0], cons }],
        halves: vec![Vec::new(), Vec::new()],
        starts: Vec::new(),
        ends: Vec::new(),
        least: 0.5,
        frame: 0.0,
    };
    let path = fewest_bends(&course).unwrap();
    assert!(in_eight_directions(&path.pts), "{:?}", path.pts);
    let through = path.stages[0].last().unwrap();
    assert!((through[0] - 5.0).abs() < 1e-6 && through[1].abs() <= 0.5 + 1e-6, "{through:?}");
    // Every piece keeps half a unit from both corners.
    for w in path.pts.windows(2) {
        for c in [low, high] {
            let (ab, ap) = ([w[1][0] - w[0][0], w[1][1] - w[0][1]], [c[0] - w[0][0], c[1] - w[0][1]]);
            let t = ((ap[0] * ab[0] + ap[1] * ab[1]) / (ab[0] * ab[0] + ab[1] * ab[1])).clamp(0.0, 1.0);
            let d = ((ap[0] - t * ab[0]).powi(2) + (ap[1] - t * ab[1]).powi(2)).sqrt();
            assert!(d >= 0.5 - 1e-6, "{d} from {c:?}: {:?}", path.pts);
        }
    }
}

/// With no room between the corners there is no path, and the search says how far it got.
#[test]
fn a_gap_too_narrow_stops_the_wire() {
    let (low, high) = ([5.0, -0.2], [5.0, 0.2]);
    let cons = vec![
        Con { p: high, r: 0.5, left: true, toward: [0.0, 1.0] },
        Con { p: [5.0, -0.199], r: 0.0, left: true, toward: [0.0, 1.0] },
        Con { p: low, r: 0.5, left: false, toward: [0.0, -1.0] },
        Con { p: [5.0, 0.199], r: 0.0, left: false, toward: [0.0, -1.0] },
    ];
    let course = Course {
        a: [0.0, 0.0],
        b: [10.0, 0.0],
        regions: vec![square([-5.0, -10.0], [5.0, 10.0]), square([5.0, -10.0], [15.0, 10.0])],
        gaps: vec![Gap { ends: [low, high], forward: [1.0, 0.0], cons }],
        halves: vec![Vec::new(), Vec::new()],
        starts: Vec::new(),
        ends: Vec::new(),
        least: 0.5,
        frame: 0.0,
    };
    assert_eq!(fewest_bends(&course).unwrap_err().reached, 0);
}
