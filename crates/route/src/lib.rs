//! The router's top level: a board file in; the board routed, its pictures and a report out
//! ([`route`]); and the checker's verdict on a board as it stands ([`check`]). `boardroom route`
//! and `boardroom check` are these.
//!
//! [`route`] strips the board, plans it, draws the plan as copper, checks the copper (and counts
//! the pads a refill would leave short of thermal spokes), and writes `routed.kicad_pcb`,
//! `bare.kicad_pcb` (the board stripped), `wires.json`, `report.json`, one `plan.LAYER.svg` and
//! one `wire.LAYER.svg` per layer that has pads. The report names each plane layer taken as solid
//! because nothing on the board says what may cross it. While it is at work it keeps a trace of
//! its stages and the board as far as it has come in `partial/` ([`partial`]), so that a run
//! stopped or killed has left what it had; a run that ends leaves the trace as `trace.log`.
//!
//! Environment, for looking into a run: `WIRE_WATCH=LAYER:STRAND` also writes `watch.svg`, that
//! strand's course as the drawing saw it; `WIRE_POURS` tells each connection still open, with
//! what its ends are, and what each pad of a poured net is joined to; `WIRE_TIMES` (in `wire`)
//! tells each round's stages, and with `WIRE_LEFT` names at each stage the connections the plan
//! has no way for (with how many gaps each has been barred from) and the wires not drawn (with
//! why); `WIRE_FIRST` (in `wire`) stops at the first plan and its drawing, with nothing laid
//! again and no pours; `PLAN_JITTER=SEED` (in `plan`, for measuring only) shakes the order the
//! connections are planned in a little, so that a hard board can be routed several times over
//! and a change judged against how far its result turns on the order alone. Copper text is shaped by `kicad-text` where
//! `ULTRAROUTE_TEXT_HELPER` names it; without it text is boxes, which cover pads its letters do
//! not, and the report says so.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{json, Value};

pub mod partial;

/// Routes the board at `path` and writes what [the module](self) says into `out`; the report.
/// With `packages` (Boardroom's `packages/`), each pour the board's pinned blocks keep joined
/// is kept so: no track or via laid may break a section of it that is whole on the bare board,
/// and a via in it costs dearly ([`board::Board::keep_joined`]).
///
/// `filled_vias` says whether the board is made with its vias filled and capped, so that one may
/// stand in a pad a part is soldered to; with none it is as the board's make-up has it
/// ([`board::Board::filled_vias`]).
pub fn route(path: &Path, out: &Path, packages: Option<&Path>, filled_vias: Option<bool>) -> Result<Value, String> {
    route_saving(path, out, packages, filled_vias, partial::SAVE_EVERY)
}

/// [`route`], with each way's board as far as it has come written into `out/partial/` at
/// least every `save_every` seconds while the run goes on ([`partial`]); none at 0, only the
/// trace.
pub fn route_saving(path: &Path, out: &Path, packages: Option<&Path>, filled_vias: Option<bool>, save_every: u64) -> Result<Value, String> {
    let (mut original, text) = board::Board::load(path).map_err(|e| e.to_string())?;
    if let Some(filled) = filled_vias {
        original.filled_vias = filled;
    }
    let mut notes: Vec<String> = Vec::new();
    // Copper text taken as boxes covers pads its letters do not: say so.
    if let Some(note) = original.text_note.as_deref().filter(|n| n.contains("as boxes")) {
        notes.push(note.to_string());
    }
    let mut board = original.clone();
    plan::strip(&mut board);
    let mut kept = Vec::new();
    if let Some(root) = packages {
        for packages::KeptPour { zone, name, cut_by, min_width } in packages::kept_joined(path, &board, root)? {
            let sections = board.keep_joined(zone, &cut_by, min_width);
            let whole = sections.iter().filter(|s| s.ok).count();
            if whole < sections.len() {
                notes.push(format!("{name}: {} of {} sections already broken on the bare board, by copper the router keeps; left as they are", sections.len() - whole, sections.len()));
            }
            kept.push(json!({"zone": name, "sections": sections.len(), "whole": whole, "min_width_mm": min_width as f64 / 1e6}));
        }
    }
    let started = std::time::Instant::now();
    // WIRE_WATCH=LAYER:STRAND writes that strand's course as watch.svg.
    let watch = std::env::var("WIRE_WATCH").ok().and_then(|w| {
        let (l, s) = w.split_once(':')?;
        Some((l.parse().ok()?, s.parse().ok()?))
    });
    let saver = partial::Saver::new(out, path, &text, &original, save_every)?;
    // A run that crashes says so in its trace, where whoever finds it stopped will look.
    let routed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| wire::route_watched(&board, &|step| saver.step(step))));
    let (plan, drawing) = routed.unwrap_or_else(|panic| {
        let why = panic.downcast_ref::<String>().map(String::as_str).or_else(|| panic.downcast_ref::<&str>().copied()).unwrap_or("no message");
        saver.say(&format!("the run crashed: {why}"));
        std::panic::resume_unwind(panic)
    });
    saver.say("the ways are done; the one with fewest open is checked and written");
    // The strand watched is drawn again, alone among the same wires, for its picture.
    let watched = watch.and_then(|_| wire::draw_watching(&board, &plan, watch).1);
    let seconds = started.elapsed().as_secs_f64();
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
    // The pads a refill leaves too few spokes to their pour, that had enough before.
    let before = board::Pours::likely(&board).starved(&board);
    for s in board::Pours::likely(&routed).starved(&routed).iter().filter(|s| !before.iter().any(|b| (b.pad, b.layer) == (s.pad, s.layer))) {
        *rules.entry("StarvedThermal".to_string()).or_insert(0) += 1;
        if let Some(b) = routed.bbox(s.pad) {
            faults[s.layer].push([(b.min.x + b.max.x) as f64 / 2e6, (b.min.y + b.max.y) as f64 / 2e6]);
        }
    }
    let pours = board::Pours::of(&routed);
    let lines = board::connectivity_poured(&routed, &pours).airlines;
    let open_nets = lines.iter().fold(BTreeMap::new(), |mut m, a| {
        *m.entry(routed.net_name(a.net).to_string()).or_insert(0) += 1;
        m
    });
    if std::env::var("WIRE_POURS").is_ok() {
        tell_pours(&board, &routed, &pours, &lines);
    }

    // The new vias that stand in a pad a part is soldered to: the board must be ordered with
    // its vias filled and capped, and whoever orders it is told so.
    let in_pads = new
        .iter()
        .filter_map(|&id| routed.item(id))
        .filter(|i| matches!(i.kind, board::Kind::Via))
        .filter_map(|i| i.hole)
        .filter(|&(at, _)| routed.soldered.iter().any(|&pad| routed.item(pad).is_some_and(|p| p.shape.contains(at))))
        .count();
    if in_pads > 0 {
        notes.push(format!("{in_pads} of the vias stand in a pad a part is soldered to: the board must be ordered with its vias filled and capped"));
    }

    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let write = |name: &str, bytes: &str| std::fs::write(out.join(name), bytes).map_err(|e| format!("{}: {e}", out.join(name).display()));
    if let Some(svg) = watched {
        write("watch.svg", &svg)?;
    }
    write("wires.json", &serde_json::to_string(&drawing).map_err(|e| e.to_string())?)?;
    write("routed.kicad_pcb", &wire::write(&text, &original, &plan, &drawing))?;
    // The board as the router was given it, for a judge to set the routed one beside.
    let bare = kicad::write::Edit {
        uuid_seed: Some(1),
        remove: original.items().filter(|(_, i)| matches!(i.kind, board::Kind::Track | board::Kind::Via) && !i.locked).filter_map(|(_, i)| i.span).collect(),
        ..Default::default()
    };
    write("bare.kicad_pcb", &kicad::write::apply(&text, &bare))?;
    let mut layers = Vec::new();
    for (l, sheet) in plan.sheets.iter().enumerate() {
        if !sheet.space.ends.keys().any(|e| matches!(e, plan::End::Item(_))) && sheet.laid().count() == 0 {
            continue;
        }
        write(&format!("wire.{}.svg", board.layers[l]), &wire::draw::layer(&board, &plan, &drawing, l, &faults[l], &pours.rects(l)))?;
        write(&format!("plan.{}.svg", board.layers[l]), &plan::draw::layer(&board, &plan, l))?;
        let wires: Vec<&wire::Wire> = drawing.wires.iter().filter(|w| w.layer == l).collect();
        layers.push(json!({
            "layer": board.layers[l],
            "strands": sheet.laid().count(),
            // A strand is drawn though some or all of its wire was spare and is taken out.
            "wires": sheet.laid().count() - drawing.undrawn.iter().filter(|u| u.layer == l).count(),
            "bends": wires.iter().map(|w| w.pts.len().saturating_sub(2)).sum::<usize>(),
            "plane": plan.planes.contains(&l), "closed": plan.closed.contains(&l),
        }));
    }
    let report = json!({
        "board": path.display().to_string(),
        "layers": layers,
        "solid": solid(&board, &plan),
        "strands": plan.sheets.iter().map(|s| s.laid().count()).sum::<usize>(),
        "wires": plan.sheets.iter().map(|s| s.laid().count()).sum::<usize>() - drawing.undrawn.len(),
        "spare": drawing.spare.len(),
        "as_planned": drawing.wires.iter().filter(|w| w.as_planned).count(),
        "undrawn": drawing.undrawn.len(),
        "why": drawing.undrawn.iter().fold(BTreeMap::new(), |mut m, u| {
            let why = u.why.split(" past gap").next().unwrap_or("").to_string();
            *m.entry(why).or_insert(0) += 1;
            m
        }),
        "left": plan.left.len(),
        "vias": plan.vias.len(),
        "filled_vias": board.filled_vias,
        "vias_in_soldered_pads": in_pads,
        "ties": drawing.ties.len(),
        "stitches": drawing.stitches.len(),
        "faults": rules,
        "open": lines.len(),
        "open_nets": open_nets,
        "notes": notes,
        "kept_joined": kept,
        // Kept as "plan" and "draw" for the bench's pages: one stage, both ends of it.
        "seconds": {"plan": seconds, "draw": seconds},
    });
    write("report.json", &serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?)?;
    saver.done(&format!("done: {} connections open, {} with no way, {} wires not drawn; routed.kicad_pcb and report.json written", lines.len(), plan.left.len(), drawing.undrawn.len()));
    Ok(report)
}

/// The plane layers the plan took as solid because nothing on the board says what may cross
/// them (owner, 2026-11-16): each with the net that pours most of it.
fn solid(board: &board::Board, plan: &plan::Plan) -> Vec<Value> {
    plan.closed
        .iter()
        .map(|&l| {
            let mut by: BTreeMap<u32, usize> = BTreeMap::new();
            for z in board.zone_areas.iter().filter(|z| z.layers & (1 << l) != 0 && z.net != 0) {
                *by.entry(z.net).or_insert(0) += 1;
            }
            let net = by.into_iter().max_by_key(|&(_, n)| n).map_or("", |(net, _)| board.net_name(net));
            json!({
                "layer": board.layers[l],
                "net": net,
                "why": "nothing on the board says what may cross it: no track crosses it, vias pass",
            })
        })
        .collect()
}

/// `WIRE_POURS`: each connection still open with its ends, and what each piece of a poured net
/// is joined to.
fn tell_pours(board: &board::Board, routed: &board::Board, pours: &board::Pours, lines: &[board::Airline]) {
    let what = |id: board::ItemId| {
        routed.item(id).map_or("?".to_string(), |i| {
            let kind = match i.kind {
                board::Kind::Pad { .. } => "a pad",
                board::Kind::Track => "a track",
                board::Kind::Via => "a via",
                _ => "copper",
            };
            let layers: Vec<&str> = (0..routed.layers.len()).filter(|l| i.on(*l)).map(|l| routed.layers[l].as_str()).collect();
            format!("{kind} on {}", layers.join(" "))
        })
    };
    for a in lines {
        eprintln!("open: {} from ({:.3}, {:.3}) {} to ({:.3}, {:.3}) {}", routed.net_name(a.net), a.from.x as f64 / 1e6, a.from.y as f64 / 1e6, what(a.from_item), a.to.x as f64 / 1e6, a.to.y as f64 / 1e6, what(a.to_item));
    }
    let joined = board::connectivity_poured(routed, pours);
    for (id, item) in routed.items() {
        if !board.zone_areas.iter().any(|z| z.net == item.net) || !item.is_copper() || matches!(item.kind, board::Kind::Pour { .. }) {
            continue;
        }
        let Some(b) = routed.bbox(id) else { continue };
        let pieces: Vec<(usize, Vec<usize>)> = (0..routed.layers.len()).map(|l| (l, pours.joined(routed, id, l))).filter(|(_, p)| !p.is_empty()).collect();
        let relief = board.zone_areas.iter().any(|z| routed.relief_pads.contains_key(&(id, z.zone)));
        eprintln!("{:?} net {} at {:.2},{:.2} relief {relief} cluster {:?} pieces {pieces:?}", item.kind, routed.net_name(item.net), (b.min.x + b.max.x) as f64 / 2e6, (b.min.y + b.max.y) as f64 / 2e6, joined.cluster.get(&id));
    }
}

/// The checker's verdict on the board at `path` as it stands, pours as filled in the file: each
/// fault with its rule, layer, place and the items in it, and each connection still open.
/// Each pour's pieces after a refill round the copper on the board, what each joins and where
/// it is narrow ([`board::Pours::pieces_of`]): the zones whose name contains `zone`, else all.
/// With `cut_by`, each zone's sections too ([`board::sections`]): its pins as the nets named
/// there alone would cut it, each joined inside by copper at least the fab's narrowest track for
/// the zone's layer of the board's stackup ([`board::fab::min_track`]).
pub fn pours(path: &Path, zone: Option<&str>, cut_by: &[String]) -> Result<Value, String> {
    let (board, text) = board::Board::load(path).map_err(|e| e.to_string())?;
    let (pcb, copper) = kicad::parse_board(&text).map_err(|e| e.to_string())?;
    let pours = board::Pours::likely(&board);
    let mut cut = Vec::new();
    for name in cut_by {
        cut.push(board.nets.iter().position(|n| n == name).ok_or(format!("no net {name}"))? as u32);
    }
    // The narrowest track the fab builds on the zone's layer of this stackup.
    let min_width = |z: &board::ZoneArea| {
        let name = board.layers.get(z.layers.trailing_zeros() as usize);
        let copper = name.and_then(|n| pcb.copper_thickness_m.iter().find(|(l, _)| l == n).map(|(_, t)| *t));
        board::fab::min_track(pcb.copper_layers, copper)
    };
    let mm = |v: f64| (v / 1e3).round() / 1e3;
    let at = |p: board::Point| [mm(p.x as f64), mm(p.y as f64)];
    let describe = |id: board::ItemId| -> String {
        board.item(id).map_or("?".into(), |i| {
            let what = match &i.kind {
                board::Kind::Pad { footprint, number, .. } => format!("pad {}.{number}", board.footprints[*footprint].reference),
                board::Kind::Track => "track".into(),
                board::Kind::Via => "via".into(),
                board::Kind::Pour { .. } => "pour".into(),
                _ => "other".into(),
            };
            match board.net_name(i.net) {
                "" => what,
                net => format!("{what} of {net}"),
            }
        })
    };
    let mut zones = Vec::new();
    for z in &board.zone_areas {
        let name = copper.zones.get(z.zone).map_or("", |c| c.name.as_str());
        if z.net == 0 || zone.is_some_and(|want| !name.contains(want)) {
            continue;
        }
        let pieces: Vec<Value> = pours
            .pieces_of(&board, z.zone)
            .iter()
            .map(|p| {
                json!({
                    "layer": board.layers[p.layer],
                    "area_mm2": (p.area / 1e10).round() / 100.0,
                    "at": at(p.at),
                    "pads": p.pads.iter().map(|&id| describe(id)).collect::<Vec<_>>(),
                    "vias": p.vias.len(),
                    "tracks": p.tracks.len(),
                    "necks": p.necks.iter().map(|n| json!({
                        "width_mm": mm(n.width as f64),
                        "at_limit": n.at_limit,
                        "at": at(n.at),
                        "by": n.by.iter().map(|&(id, gap)| json!({"item": describe(id), "gap_mm": mm(gap as f64)})).collect::<Vec<_>>(),
                    })).collect::<Vec<_>>(),
                })
            })
            .collect();
        if pieces.is_empty() {
            continue;
        }
        let pin = |id: board::ItemId| match board.item(id).map(|i| &i.kind) {
            Some(board::Kind::Pad { footprint, number, .. }) => format!("{}.{number}", board.footprints[*footprint].reference),
            _ => "?".into(),
        };
        let neck = |n: &board::Neck| json!({
            "width_mm": mm(n.width as f64),
            "at_limit": n.at_limit,
            "at": at(n.at),
            "by": n.by.iter().map(|&(id, gap)| json!({"item": describe(id), "gap_mm": mm(gap as f64)})).collect::<Vec<_>>(),
        });
        let links: Vec<Value> = pours
            .links_of(&board, z.zone)
            .iter()
            .map(|l| {
                json!({
                    "layer": board.layers[l.layer],
                    "pins": l.pins.len(),
                    "groups": l.groups.iter().map(|(m, w)| json!({"pins": m.iter().map(|&id| pin(id)).collect::<Vec<_>>(), "weakest": w.as_ref().map(neck)})).collect::<Vec<_>>(),
                })
            })
            .collect();
        let sections: Vec<Value> = if cut.is_empty() {
            Vec::new()
        } else {
            board::sections(&board, z.zone, &cut, min_width(z))
                .iter()
                .map(|x| {
                    json!({
                        "layer": board.layers[x.layer],
                        "pins": x.pins.iter().map(|&id| pin(id)).collect::<Vec<_>>(),
                        "groups": x.groups.iter().map(|g| g.iter().map(|&id| pin(id)).collect::<Vec<_>>()).collect::<Vec<_>>(),
                        "weakest": x.weakest.as_ref().map(neck),
                        "ok": x.ok,
                    })
                })
                .collect()
        };
        zones.push(json!({"zone": z.zone, "name": name, "net": board.net_name(z.net), "pieces": pieces, "links": links, "sections": sections, "min_width_mm": mm(min_width(z) as f64)}));
    }
    Ok(json!({"board": path.display().to_string(), "zones": zones}))
}

pub fn check(path: &Path) -> Result<Value, String> {
    let (board, _) = board::Board::load(path).map_err(|e| e.to_string())?;
    let describe = |id: board::ItemId| -> String {
        board.item(id).map_or("?".into(), |i| {
            let what = match &i.kind {
                board::Kind::Pad { footprint, number, .. } => format!("pad {}.{number}", board.footprints[*footprint].reference),
                board::Kind::Track => "track".into(),
                board::Kind::Via => "via".into(),
                board::Kind::Pour { .. } => "pour".into(),
                board::Kind::Keepout { .. } => "rule area".into(),
                board::Kind::Spoke { .. } => "thermal spoke".into(),
            };
            match board.net_name(i.net) {
                "" => what,
                net => format!("{what} of {net}"),
            }
        })
    };
    let faults: Vec<Value> = board::check(&board)
        .into_iter()
        .map(|v| {
            let at = board.bbox(v.a).map(|b| [(b.min.x + b.max.x) as f64 / 2e6, (b.min.y + b.max.y) as f64 / 2e6]);
            let items: Vec<String> = [Some(v.a), v.b].into_iter().flatten().map(describe).collect();
            json!({
                "rule": format!("{:?}", v.rule),
                "layer": v.layer.map(|l| board.layers[l].clone()),
                "at": at,
                "items": items,
            })
        })
        .collect();
    let open: Vec<Value> = board::connectivity(&board)
        .airlines
        .iter()
        .map(|a| json!({"net": board.net_name(a.net), "from": [a.from.x as f64 / 1e6, a.from.y as f64 / 1e6], "to": [a.to.x as f64 / 1e6, a.to.y as f64 / 1e6]}))
        .collect();
    let by_rule = faults.iter().fold(BTreeMap::new(), |mut m, f| {
        *m.entry(f["rule"].as_str().unwrap_or("").to_string()).or_insert(0) += 1;
        m
    });
    Ok(json!({"board": path.display().to_string(), "faults": faults.len(), "by_rule": by_rule, "open": open.len(), "fault_list": faults, "open_list": open, "warnings": board.warnings}))
}
