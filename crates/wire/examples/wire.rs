//! `wire BOARD.kicad_pcb OUT_DIR`: strips the board, plans it, draws the plan as copper, checks
//! the copper, and writes `wires.json`, `routed.kicad_pcb`, one `plan.LAYER.svg` and one
//! `wire.LAYER.svg` per layer that has pads, and a line of counts. `WIRE_WATCH=LAYER:STRAND` also
//! writes `watch.svg`, that strand's course as the drawing saw it. `WIRE_POURS` tells what each
//! pad of a poured net is joined to; `WIRE_TIMES` (in the library) tells each round's stages.
//! Copper text is shaped by `kicad-text` where `ULTRAROUTE_TEXT_HELPER` names it (it is not
//! beside an example); without it text is boxes, which cover pads its letters do not, and this
//! says so.

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
    // Copper text taken as boxes covers pads its letters do not: say so.
    if let Some(note) = original.text_note.as_deref().filter(|n| n.contains("as boxes")) {
        eprintln!("{note}");
    }
    let mut board = original.clone();
    plan::strip(&mut board);
    let started = std::time::Instant::now();
    // WIRE_WATCH=LAYER:STRAND writes that strand's course as watch.svg.
    let watch = std::env::var("WIRE_WATCH").ok().and_then(|w| {
        let (l, s) = w.split_once(':')?;
        Some((l.parse().ok()?, s.parse().ok()?))
    });
    let (plan, drawing) = wire::route(&board);
    // The strand watched is drawn again, alone among the same wires, for its picture.
    let watched = watch.and_then(|_| wire::draw_watching(&board, &plan, watch).1);
    let planned = started.elapsed().as_secs_f64();
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
    let pours = board::Pours::of(&routed);
    let lines = board::connectivity_poured(&routed, &pours).airlines;
    let open = lines.len();
    let open_nets = lines.iter().fold(BTreeMap::new(), |mut m, a| {
        *m.entry(routed.net_name(a.net).to_string()).or_insert(0) += 1;
        m
    });
    if std::env::var("WIRE_POURS").is_ok() {
        let joined = board::connectivity_poured(&routed, &pours);
        for (id, item) in routed.items() {
            if !board.zone_areas.iter().any(|z| z.net == item.net) || !item.is_copper() || matches!(item.kind, board::Kind::Pour { .. }) {
                continue;
            }
            let Some(b) = routed.bbox(id) else { continue };
            let pieces: Vec<(usize, Vec<usize>)> = (0..routed.layers.len()).map(|l| (l, pours.joined(&routed, id, l))).filter(|(_, p)| !p.is_empty()).collect();
            let relief = board.zone_areas.iter().any(|z| routed.relief_pads.contains_key(&(id, z.zone)));
            eprintln!("{:?} net {} at {:.2},{:.2} relief {relief} cluster {:?} pieces {pieces:?}", item.kind, routed.net_name(item.net), (b.min.x + b.max.x) as f64 / 2e6, (b.min.y + b.max.y) as f64 / 2e6, joined.cluster.get(&id));
        }
    }
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
        std::fs::write(out.join(format!("wire.{}.svg", board.layers[l])), wire::draw::layer(&board, &plan, &drawing, l, &faults[l], &pours.rects(l))).unwrap();
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
            "as_planned": drawing.wires.iter().filter(|w| w.as_planned).count(),
            "undrawn": drawing.undrawn.len(),
            "why": drawing.undrawn.iter().fold(BTreeMap::new(), |mut m, u| {
                let why = u.why.split(" past gap").next().unwrap_or("").to_string();
                *m.entry(why).or_insert(0) += 1;
                m
            }),
            "left": plan.left.len(),
            "vias": plan.vias.len(),
            "faults": rules,
            "open": open,
            "open_nets": open_nets,
            "seconds": {"plan": planned, "draw": drawn},
        })
    );
}
