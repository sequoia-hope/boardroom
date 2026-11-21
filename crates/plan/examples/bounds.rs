//! `bounds BOARD`: the loops the planner takes for the board's edge, and the ends of edge lines no other line ends at.
fn main() {
    let path = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let (board, _) = board::Board::load(&path).unwrap();
    println!("{} loops in the model, {} edge lines", board.outline.len(), board.outline_edges.len());
    for l in plan::space::bounds(&board) {
        let b = board::BBox::of(l.iter().copied());
        println!("  loop of {} corners, {:.2} x {:.2} mm at {:.2}, {:.2}", l.len(), (b.max.x - b.min.x) as f64 / 1e6, (b.max.y - b.min.y) as f64 / 1e6, b.min.x as f64 / 1e6, b.min.y as f64 / 1e6);
    }
    let pads: Vec<board::Point> = board.items().filter(|(_, i)| matches!(i.kind, board::Kind::Pad { .. })).filter_map(|(id, _)| board.bbox(id)).map(|b| board::Point::new((b.min.x + b.max.x) / 2, (b.min.y + b.max.y) / 2)).collect();
    for l in plan::space::closed_loops(&board) {
        let b = board::BBox::of(l.iter().copied());
        println!("  closed: {} corners, {:.2} x {:.2} mm at {:.2}, {:.2}, {} pads in it", l.len(), (b.max.x - b.min.x) as f64 / 1e6, (b.max.y - b.min.y) as f64 / 1e6, b.min.x as f64 / 1e6, b.min.y as f64 / 1e6, pads.iter().filter(|&&p| board::inside(p, &l)).count());
    }
    let mut ends: std::collections::HashMap<(i64, i64), usize> = Default::default();
    for w in board.outline_runs.iter().flat_map(|run| run.windows(2)) {
        *ends.entry((w[0].x, w[0].y)).or_default() += 1;
        *ends.entry((w[1].x, w[1].y)).or_default() += 1;
    }
    let mut odd: Vec<_> = ends.iter().filter(|(_, n)| **n != 2).collect();
    odd.sort();
    for (p, n) in odd.iter().take(12) {
        println!("  {n} lines end at {:.4}, {:.4}", p.0 as f64 / 1e6, p.1 as f64 / 1e6);
    }
    println!("  {} such ends", odd.len());
}
