//! `plan BOARD.kicad_pcb OUT_DIR`: strips the board, plans it, and writes `plan.json`, one
//! `plan.LAYER.svg` per layer that has pads, and a line of counts.

use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: plan BOARD.kicad_pcb OUT_DIR");
        std::process::exit(2);
    }
    let (mut board, _) = board::Board::load(Path::new(&args[1])).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1)
    });
    plan::strip(&mut board);
    let started = std::time::Instant::now();
    let plan = plan::plan(&board);
    let seconds = started.elapsed().as_secs_f64();
    let out = Path::new(&args[2]);
    std::fs::create_dir_all(out).unwrap();
    std::fs::write(out.join("plan.json"), serde_json::to_string(&plan).unwrap()).unwrap();
    let mut layers = Vec::new();
    for (l, sheet) in plan.sheets.iter().enumerate() {
        if sheet.space.of_item.is_empty() {
            continue;
        }
        std::fs::write(out.join(format!("plan.{}.svg", board.layers[l])), plan::draw::layer(&board, &plan, l)).unwrap();
        layers.push(serde_json::json!({"layer": board.layers[l], "strands": sheet.laid().count(), "triangles": sheet.space.tris.len(), "plane": plan.planes.contains(&l)}));
    }
    let mut why = std::collections::BTreeMap::new();
    for l in &plan.left {
        *why.entry(l.why.clone()).or_insert(0) += 1;
    }
    println!("{}", serde_json::json!({"layers": layers, "planned": plan.sheets.iter().map(|s| s.laid().count()).sum::<usize>(), "left": plan.left.len(), "why": why, "seconds": seconds}));
}
