//! What the designer did on a routed board, as JSON and pictures.
//!
//!     designer BOARD.kicad_pcb OUT_DIR
//!
//! Writes OUT_DIR/designer.json and, as SVG: `layer.<L>.svg` for each copper layer (pours,
//! tracks), `frames.svg`, `bundles.<L>.svg` for each layer with tracks, `repeats.svg`.

use std::path::Path;

use board::pcb::Board;
use examine::designer::{self, draw};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [pcb, out] = args.as_slice() else {
        eprintln!("usage: designer BOARD.kicad_pcb OUT_DIR");
        std::process::exit(2);
    };
    let board = Board::load(Path::new(pcb)).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1);
    });
    let out = Path::new(out);
    std::fs::create_dir_all(out).expect("out dir");
    let x = designer::extract(&board);
    let put = |name: String, text: String| {
        std::fs::write(out.join(&name), text).unwrap_or_else(|e| panic!("{name}: {e}"))
    };
    put(
        "designer.json".into(),
        serde_json::to_string_pretty(&x).expect("json"),
    );
    for l in &x.layers {
        let stem = l.layer.replace('.', "_");
        put(format!("layer.{stem}.svg"), draw::layer(&board, &l.layer));
        if l.tracks > 0 {
            put(
                format!("bundles.{stem}.svg"),
                draw::bundles(&board, &x, &l.layer),
            );
        }
    }
    put("frames.svg".into(), draw::frames(&board, &x));
    put("repeats.svg".into(), draw::repeats(&board, &x));
}
