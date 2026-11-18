//! The raster cells of one zone's fill, as `x y piece` lines in mm, to set beside KiCad's.
//!
//!     cargo run --example pour_cells -- board.kicad_pcb "<zone name>"

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = std::path::Path::new(&args[1]);
    let (board, text) = board::Board::load(path).unwrap();
    let (_, copper) = kicad::parse_board(&text).unwrap();
    let pours = board::Pours::likely(&board);
    let z = board.zone_areas.iter().find(|z| copper.zones[z.zone].name == args[2]).expect("no such zone");
    for layer in (0..board.layers.len()).filter(|l| z.layers & (1 << l) != 0) {
        for (p, piece) in pours.cells_of(z.zone, layer) {
            println!("{} {} {piece}", p.x as f64 / 1e6, p.y as f64 / 1e6);
        }
    }
}
