//! One routing of a board beside the designer's, as the sums the scoreboard adds up.
//!
//!     like DESIGNER.kicad_pcb OURS.kicad_pcb
//!
//! Prints `examine::designer::like` as one line of JSON.

use std::path::Path;

use board::pcb::Board;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [designer, ours] = args.as_slice() else {
        eprintln!("usage: like DESIGNER.kicad_pcb OURS.kicad_pcb");
        std::process::exit(2);
    };
    let load = |p: &String| {
        Board::load(Path::new(p)).unwrap_or_else(|e| {
            eprintln!("{p}: {e}");
            std::process::exit(1);
        })
    };
    let like = examine::designer::like::like(&load(designer), &load(ours));
    println!("{}", serde_json::to_string(&like).expect("json"));
}
