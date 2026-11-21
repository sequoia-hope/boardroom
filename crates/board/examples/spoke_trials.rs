//! `spoke_trials BOARD.kicad_pcb`: every thermal spoke a pad of the board could have, one to a
//! line: the pad (reference and number), the layer, the way the spoke runs in degrees, the
//! middle of the pad's gap on its axis and a point on it just off the pad, in millimetres, and
//! what was made of it (`Kept`, or why not). `bench/spokes.py` sets these beside the fills KiCad wrote into the board.

use std::path::PathBuf;

use board::{Board, Kind};

fn main() {
    let path = PathBuf::from(std::env::args().nth(1).expect("usage: spoke_trials BOARD.kicad_pcb"));
    let (board, _) = Board::load(&path).unwrap_or_else(|e| panic!("{e}"));
    for t in &board.spoke_trials {
        let pad = match board.item(t.pad).map(|it| &it.kind) {
            Some(Kind::Pad { footprint, number, .. }) => format!("{}-{}", board.footprints[*footprint].reference, number),
            _ => String::new(),
        };
        let mm = |p: board::Point| format!("{:.4}\t{:.4}", p.x as f64 / 1e6, p.y as f64 / 1e6);
        println!("{pad}\t{}\t{}\t{}\t{}\t{:?}", board.layers[t.layer], t.angle, mm(t.mid), mm(t.by_pad), t.made);
    }
}
