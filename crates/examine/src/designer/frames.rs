//! The frame of the routing in each neighbourhood: the rotation, between -22.5 and 22.5
//! degrees, of the 45-degree grid of directions the tracks there follow.
//!
//! A track at heading h belongs to the frame h modulo 45. Headings are averaged on that circle,
//! weighted by length: each piece of track adds a vector of its length at angle 8h, and the sum's
//! direction over 8 is the frame, its length over the track length how well the tracks agree
//! (1: all on one 45-degree grid; near 0: no grid).

use board::pcb::geom::{self, P};
use board::pcb::Board;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Cell {
    pub centre: P,
    /// Track length in this cell, mm.
    pub track_mm: f64,
    /// The frame of this cell and its eight neighbours, degrees.
    pub frame_deg: f64,
    /// 0 to 1: how much of the neighbourhood's track agrees with the frame.
    pub agreement: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Frames {
    pub cell_mm: f64,
    pub cells: Vec<Cell>,
    /// One frame for the whole board, and how well the board agrees with it.
    pub board_deg: f64,
    pub board_agreement: f64,
    /// Share of track length more than [`ON_GRID`] degrees off the 45-degree grid of the board's
    /// own x and y.
    pub off_board_axes: f64,
    /// The same against each track's neighbourhood frame.
    pub off_local_frame: f64,
    /// Share of track length in neighbourhoods that agree on a frame turned from the board's axes.
    pub turned: f64,
}

/// A track within this many degrees of a grid direction is on it.
pub const ON_GRID: f64 = 2.0;
/// A neighbourhood has a frame when its agreement is at least this.
pub const AGREES: f64 = 0.6;

/// h modulo 45 as a unit vector at 8h.
fn vote(a: P, b: P) -> (f64, f64) {
    let h = (b[1] - a[1]).atan2(b[0] - a[0]);
    ((8.0 * h).cos(), (8.0 * h).sin())
}

fn frame(sum: (f64, f64), len: f64) -> (f64, f64) {
    if len <= 0.0 {
        return (0.0, 0.0);
    }
    (
        sum.1.atan2(sum.0).to_degrees() / 8.0,
        (sum.0 * sum.0 + sum.1 * sum.1).sqrt() / len,
    )
}

/// Degrees from heading a->b to the nearest direction of the 45-degree grid turned by `frame`.
pub fn off_grid(a: P, b: P, frame_deg: f64) -> f64 {
    let h = (b[1] - a[1]).atan2(b[0] - a[0]).to_degrees() - frame_deg;
    let r = h.rem_euclid(45.0);
    r.min(45.0 - r)
}

pub fn frames(board: &Board) -> Frames {
    let (lo, hi) = super::extent(board);
    let (w, h) = (hi[0] - lo[0], hi[1] - lo[1]);
    // About a dozen neighbourhoods across the board, never finer than a small IC.
    let cell_mm = ((w * h).sqrt() / 12.0).clamp(3.0, 10.0);
    let (nx, ny) = (
        ((w / cell_mm).ceil() as usize).max(1),
        ((h / cell_mm).ceil() as usize).max(1),
    );
    let at = |p: P| {
        let i = (((p[0] - lo[0]) / cell_mm) as usize).min(nx - 1);
        let j = (((p[1] - lo[1]) / cell_mm) as usize).min(ny - 1);
        j * nx + i
    };
    let mut sum = vec![(0.0, 0.0); nx * ny];
    let mut len = vec![0.0; nx * ny];
    // Tracks cut into pieces no longer than half a cell, each counted where its middle is.
    let pieces = |t: &board::pcb::Track| {
        let l = geom::dist(t.a, t.b);
        let n = ((l / (cell_mm / 2.0)).ceil() as usize).max(1);
        let (a, b) = (t.a, t.b);
        (0..n).map(move |k| {
            (
                geom::add(a, geom::scale(geom::sub(b, a), (k as f64 + 0.5) / n as f64)),
                l / n as f64,
            )
        })
    };
    let mut all = (0.0, 0.0);
    let mut total = 0.0;
    for t in &board.tracks {
        let v = vote(t.a, t.b);
        for (mid, l) in pieces(t) {
            let c = at(mid);
            sum[c].0 += v.0 * l;
            sum[c].1 += v.1 * l;
            len[c] += l;
        }
        let l = geom::dist(t.a, t.b);
        all.0 += v.0 * l;
        all.1 += v.1 * l;
        total += l;
    }
    let mut local = vec![(0.0, 0.0); nx * ny];
    for j in 0..ny {
        for i in 0..nx {
            let (mut s, mut l) = ((0.0, 0.0), 0.0);
            for jj in j.saturating_sub(1)..(j + 2).min(ny) {
                for ii in i.saturating_sub(1)..(i + 2).min(nx) {
                    s.0 += sum[jj * nx + ii].0;
                    s.1 += sum[jj * nx + ii].1;
                    l += len[jj * nx + ii];
                }
            }
            local[j * nx + i] = frame(s, l);
        }
    }
    let (mut off_board, mut off_local, mut turned) = (0.0, 0.0, 0.0);
    for t in &board.tracks {
        for (mid, l) in pieces(t) {
            let (f, agreement) = local[at(mid)];
            if off_grid(t.a, t.b, 0.0) > ON_GRID {
                off_board += l;
            }
            if off_grid(t.a, t.b, f) > ON_GRID {
                off_local += l;
            }
            if agreement >= AGREES && f.abs() > ON_GRID {
                turned += l;
            }
        }
    }
    let cells = (0..nx * ny)
        .filter(|&c| len[c] > 0.0)
        .map(|c| Cell {
            centre: [
                lo[0] + (c % nx) as f64 * cell_mm + cell_mm / 2.0,
                lo[1] + (c / nx) as f64 * cell_mm + cell_mm / 2.0,
            ],
            track_mm: len[c],
            frame_deg: local[c].0,
            agreement: local[c].1,
        })
        .collect();
    let (board_deg, board_agreement) = frame(all, total);
    let share = |v: f64| if total > 0.0 { v / total } else { 0.0 };
    Frames {
        cell_mm,
        cells,
        board_deg,
        board_agreement,
        off_board_axes: share(off_board),
        off_local_frame: share(off_local),
        turned: share(turned),
    }
}
