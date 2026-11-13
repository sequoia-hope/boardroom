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

/// How one layer's track runs, each piece measured in its own neighbourhood's frame.
#[derive(Debug, Clone, Serialize)]
pub struct LayerRun {
    pub layer: String,
    /// Track length, mm, along the frame's first axis (the one nearest the board's x) and along
    /// its second, at right angles to it.
    pub axis_mm: [f64; 2],
    /// Along either of the frame's diagonals.
    pub diagonal_mm: f64,
    /// On none of the frame's four directions.
    pub other_mm: f64,
}

impl LayerRun {
    /// The share of this layer's axis-running track that runs along the axis carrying more.
    pub fn along(&self) -> f64 {
        let of = self.axis_mm[0] + self.axis_mm[1];
        if of > 0.0 {
            self.axis_mm[0].max(self.axis_mm[1]) / of
        } else {
            0.0
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Frames {
    pub cell_mm: f64,
    /// The corner the neighbourhoods are counted from, and how many there are across and down.
    pub origin: P,
    pub nx: usize,
    pub ny: usize,
    /// Every neighbourhood's (frame, agreement), row by row: [`Frames::at`] reads it.
    #[serde(skip)]
    pub grid: Vec<(f64, f64)>,
    pub cells: Vec<Cell>,
    /// Each layer that carries track, in the board's layer order.
    pub runs: Vec<LayerRun>,
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

impl Frames {
    /// The (frame, agreement) of the neighbourhood holding `p`; a point outside the grid reads
    /// the nearest one. A neighbourhood with no track has the board's axes and no agreement.
    pub fn at(&self, p: P) -> (f64, f64) {
        if self.grid.is_empty() {
            return (0.0, 0.0);
        }
        let i = (((p[0] - self.origin[0]) / self.cell_mm).max(0.0) as usize).min(self.nx - 1);
        let j = (((p[1] - self.origin[1]) / self.cell_mm).max(0.0) as usize).min(self.ny - 1);
        self.grid[j * self.nx + i]
    }

    /// Over the layers that carry track, the share of the axis-running length that runs along
    /// its own layer's axis: 1 when every layer keeps to one direction, a half when none does.
    pub fn along_axis(&self) -> f64 {
        let of: f64 = self.runs.iter().map(|r| r.axis_mm[0] + r.axis_mm[1]).sum();
        let along: f64 = self
            .runs
            .iter()
            .map(|r| r.axis_mm[0].max(r.axis_mm[1]))
            .sum();
        if of > 0.0 {
            along / of
        } else {
            0.0
        }
    }
}

/// Track length of `board`, mm, more than [`ON_GRID`] degrees off the 45-degree grid of the
/// neighbourhood it lies in, as `frames` (another board's, on the same placement) has them.
pub fn off_frames_mm(board: &Board, frames: &Frames) -> f64 {
    let mut off = 0.0;
    for t in &board.tracks {
        for (mid, l) in pieces(t.a, t.b, frames.cell_mm) {
            if off_grid(t.a, t.b, frames.at(mid).0) > ON_GRID {
                off += l;
            }
        }
    }
    off
}

/// A track cut into pieces no longer than half a cell: each piece's middle and length.
fn pieces(a: P, b: P, cell_mm: f64) -> impl Iterator<Item = (P, f64)> {
    let l = geom::dist(a, b);
    let n = ((l / (cell_mm / 2.0)).ceil() as usize).max(1);
    (0..n).map(move |k| {
        (
            geom::add(a, geom::scale(geom::sub(b, a), (k as f64 + 0.5) / n as f64)),
            l / n as f64,
        )
    })
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
    let pieces = |t: &board::pcb::Track| pieces(t.a, t.b, cell_mm);
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
    let mut runs: Vec<LayerRun> = Vec::new();
    for t in &board.tracks {
        let at_layer = match runs.iter().position(|r| r.layer == t.layer) {
            Some(k) => k,
            None => {
                runs.push(LayerRun {
                    layer: t.layer.clone(),
                    axis_mm: [0.0, 0.0],
                    diagonal_mm: 0.0,
                    other_mm: 0.0,
                });
                runs.len() - 1
            }
        };
        let heading = (t.b[1] - t.a[1]).atan2(t.b[0] - t.a[0]).to_degrees();
        for (mid, l) in pieces(t) {
            let (f, agreement) = local[at(mid)];
            let run = &mut runs[at_layer];
            if off_grid(t.a, t.b, f) > ON_GRID {
                run.other_mm += l;
            } else {
                // Which of the frame's four directions: 0 its first axis, 2 its second.
                match ((heading - f).rem_euclid(180.0) / 45.0).round() as usize % 4 {
                    0 => run.axis_mm[0] += l,
                    2 => run.axis_mm[1] += l,
                    _ => run.diagonal_mm += l,
                }
            }
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
    let order = |l: &str| board.layers.iter().position(|x| x == l);
    runs.sort_by_key(|r| order(&r.layer));
    Frames {
        cell_mm,
        origin: lo,
        nx,
        ny,
        grid: local,
        cells,
        runs,
        board_deg,
        board_agreement,
        off_board_axes: share(off_board),
        off_local_frame: share(off_local),
        turned: share(turned),
    }
}
