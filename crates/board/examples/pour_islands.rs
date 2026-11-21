//! Where our pours join what KiCad's fill leaves apart: for a board KiCad has refilled, each
//! piece of pour by our fill that lies over two or more of the islands KiCad stored for the
//! same zone and layer, with the place those islands come nearest each other.
//!
//!     cargo run --release -p board --example pour_islands -- routed.filled.kicad_pcb

use board::{BBox, Board, Point};

fn main() {
    let path = std::env::args().nth(1).expect("a .kicad_pcb, refilled by KiCad");
    let (board, text) = Board::load(std::path::Path::new(&path)).expect("board");
    let (_, copper) = kicad::parse_board(&text).expect("parse");
    // As KiCad most likely fills them; with STRICT set, only what is surely joined.
    let pours = if std::env::var_os("STRICT").is_some() { board::Pours::of(&board) } else { board::Pours::likely(&board) };
    let mut found = 0;
    for z in &board.zone_areas {
        let zone = &copper.zones[z.zone];
        for layer in (0..board.layers.len()).filter(|l| z.layers & (1 << l) != 0) {
            let name = &board.layers[layer];
            let islands: Vec<Vec<Point>> = zone.filled.iter().filter(|(l, _)| l == name).map(|(_, p)| p.iter().copied().map(Point::from_m).collect()).collect();
            let boxes: Vec<BBox> = islands.iter().map(|i| BBox::of(i.iter().copied())).collect();
            // Every eighth cell each way: a piece is thousands of them.
            let mut over: std::collections::BTreeMap<usize, std::collections::BTreeMap<usize, usize>> = Default::default();
            for (p, piece) in pours.cells_of(z.zone, layer).into_iter().filter(|(p, _)| (p.x / 25_000) % 8 == 0 && (p.y / 25_000) % 8 == 0) {
                let inside = |k: &usize| p.x >= boxes[*k].min.x && p.x <= boxes[*k].max.x && p.y >= boxes[*k].min.y && p.y <= boxes[*k].max.y && board::inside(p, &islands[*k]);
                if let Some(k) = (0..islands.len()).find(inside) {
                    *over.entry(piece).or_default().entry(k).or_default() += 1;
                }
            }
            for (piece, on) in over {
                let on: Vec<usize> = on.into_iter().filter(|(_, n)| *n >= 3).map(|(k, _)| k).collect();
                for (i, &a) in on.iter().enumerate() {
                    for &b in &on[i + 1..] {
                        let near = islands[a].iter().flat_map(|p| islands[b].iter().map(move |q| (p.distance(*q), *p, *q))).min_by(|x, y| x.0.total_cmp(&y.0));
                        if let Some((d, p, q)) = near {
                            found += 1;
                            println!(
                                "{} on {name}: our piece {piece} lies over KiCad's islands {a} and {b}, {:.3} mm apart at ({:.3}, {:.3}) and ({:.3}, {:.3})",
                                if zone.name.is_empty() { &zone.net_name } else { &zone.name },
                                d / 1e6,
                                p.x as f64 / 1e6,
                                p.y as f64 / 1e6,
                                q.x as f64 / 1e6,
                                q.y as f64 / 1e6
                            );
                        }
                    }
                }
            }
        }
    }
    println!("{found} such pairs");
}
