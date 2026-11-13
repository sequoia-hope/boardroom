//! `wire BOARD.kicad_pcb OUT_DIR`: strips the board, plans it, draws the plan as copper, checks
//! the copper, and writes `wires.json`, `routed.kicad_pcb`, one `plan.LAYER.svg` and one
//! `wire.LAYER.svg` per layer that has pads, and a line of counts. `WIRE_WATCH=LAYER:STRAND` also
//! writes `watch.svg`, that strand's course as the drawing saw it.

use std::collections::BTreeMap;
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: wire BOARD.kicad_pcb OUT_DIR");
        std::process::exit(2);
    }
    let (original, text) = board::Board::load(Path::new(&args[1])).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1)
    });
    let mut board = original.clone();
    plan::strip(&mut board);
    let started = std::time::Instant::now();
    let (plan, settled) = plan::settle(&board, &plan::plan(&board));
    let planned = started.elapsed().as_secs_f64();
    // WIRE_WATCH=LAYER:STRAND writes that strand's course as watch.svg.
    let watch = std::env::var("WIRE_WATCH").ok().and_then(|w| {
        let (l, s) = w.split_once(':')?;
        Some((l.parse().ok()?, s.parse().ok()?))
    });
    let (drawing, watched) = wire::draw_watching(&board, &plan, watch);
    let drawn = started.elapsed().as_secs_f64();
    let (routed, new) = wire::onto(&board, &plan, &drawing);
    // What the checker faults that the new copper is part of.
    let mut rules = BTreeMap::new();
    let mut faults: Vec<Vec<[f64; 2]>> = vec![Vec::new(); board.layers.len()];
    for v in board::check(&routed) {
        let Some(id) = [Some(v.a), v.b].into_iter().flatten().find(|id| new.contains(id)) else { continue };
        // A pour is filled again round new copper: KiCad's refill is the judge of that.
        if [Some(v.a), v.b].into_iter().flatten().any(|id| routed.item(id).is_some_and(|i| i.refilled())) {
            continue;
        }
        *rules.entry(format!("{:?}", v.rule)).or_insert(0) += 1;
        if let Some(b) = routed.bbox(id) {
            let at = [(b.min.x + b.max.x) as f64 / 2e6, (b.min.y + b.max.y) as f64 / 2e6];
            for (l, f) in faults.iter_mut().enumerate() {
                if v.layer.is_none_or(|x| x == l) && routed.item(id).is_some_and(|i| i.on(l)) {
                    f.push(at);
                }
            }
        }
    }
    let open = board::connectivity(&routed).airlines.len();
    let out = Path::new(&args[2]);
    std::fs::create_dir_all(out).unwrap();
    if let Some(svg) = watched {
        std::fs::write(out.join("watch.svg"), svg).unwrap();
    }
    std::fs::write(out.join("wires.json"), serde_json::to_string(&drawing).unwrap()).unwrap();
    std::fs::write(out.join("routed.kicad_pcb"), wire::write(&text, &original, &plan, &drawing)).unwrap();
    let mut layers = Vec::new();
    for (l, sheet) in plan.sheets.iter().enumerate() {
        if !sheet.space.ends.keys().any(|e| matches!(e, plan::End::Item(_))) && sheet.laid().count() == 0 {
            continue;
        }
        std::fs::write(out.join(format!("wire.{}.svg", board.layers[l])), wire::draw::layer(&board, &plan, &drawing, l, &faults[l])).unwrap();
        std::fs::write(out.join(format!("plan.{}.svg", board.layers[l])), plan::draw::layer(&board, &plan, l)).unwrap();
        let wires: Vec<&wire::Wire> = drawing.wires.iter().filter(|w| w.layer == l).collect();
        layers.push(serde_json::json!({
            "layer": board.layers[l],
            "strands": sheet.laid().count(),
            "wires": wires.len(),
            "bends": wires.iter().map(|w| w.pts.len().saturating_sub(2)).sum::<usize>(),
            "plane": plan.planes.contains(&l),
        }));
    }
    println!(
        "{}",
        serde_json::json!({
            "layers": layers,
            "strands": plan.sheets.iter().map(|s| s.laid().count()).sum::<usize>(),
            "wires": drawing.wires.len(),
            "undrawn": drawing.undrawn.len(),
            "why": drawing.undrawn.iter().fold(BTreeMap::new(), |mut m, u| {
                let why = u.why.split(" past gap").next().unwrap_or("").to_string();
                *m.entry(why).or_insert(0) += 1;
                m
            }),
            "left": plan.left.len(),
            "settled": settled,
            "vias": plan.vias.len(),
            "faults": rules,
            "open": open,
            "seconds": {"plan": planned, "draw": drawn},
        })
    );
}
