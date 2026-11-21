//! `left BOARD.kicad_pcb`: each connection the first plan has no strand for, and on each layer
//! both its pads are on whether a strand alone would have a way between them; where not, the
//! gaps too narrow for it that shut each pad in. With `LEFT_GAPS` set, also each gap a strand
//! would leave either pad by: its two corners with what they belong to, its room, what a
//! strand of the connection's net and width needs there alone, and what the strands already in
//! it need.

use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("usage: left BOARD.kicad_pcb");
        std::process::exit(2);
    }
    let (mut board, _) = board::Board::load(Path::new(&args[1])).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1)
    });
    plan::strip(&mut board);
    let plan = plan::plan(&board);
    let name = |id: board::ItemId| match board.item(id).map(|it| &it.kind) {
        Some(board::Kind::Pad { footprint, number, .. }) => format!("{}-{number}", board.footprints[*footprint].reference),
        _ => "?".to_string(),
    };
    println!("{} left of {}", plan.left.len(), plan.wants.len());
    for left in &plan.left {
        let want = &plan.wants[left.connection];
        println!("{} {} to {}, {:.3} mm wide: {}", board.net_name(left.net), name(left.from), name(left.to), plan::wide(&board, want), left.why);
        // On each layer both pads are on: whether a strand alone, with nothing else laid, has a
        // way from one to the other, and if not the gaps too narrow for it that shut in what it
        // can reach from each end, the least short first.
        for (l, sheet) in plan.sheets.iter().enumerate() {
            let space = &sheet.space;
            let (Some(&a), Some(&b)) = (space.ends.get(&plan::End::Item(left.from)), space.ends.get(&plan::End::Item(left.to))) else { continue };
            let wide = plan::wide(&board, want);
            let need = |e: usize| space.keep(&board, space.edges[e].v[0], left.net) + space.keep(&board, space.edges[e].v[1], left.net) + wide;
            let at = |o: usize| -> Vec<usize> { (0..space.tris.len()).filter(|&t| space.free(t) && space.tris[t].v.iter().any(|&v| space.vert_of[v] == Some(o))).collect() };
            let reach = |o: usize| -> (Vec<bool>, Vec<(f64, usize)>) {
                let mut seen = vec![false; space.tris.len()];
                let mut short: Vec<(f64, usize)> = Vec::new();
                let mut todo = at(o);
                for &t in &todo {
                    seen[t] = true;
                }
                while let Some(t) = todo.pop() {
                    for &e in &space.tris[t].e {
                        let Some(next) = space.edges[e].t.iter().flatten().copied().find(|&y| y != t) else { continue };
                        // A side of an obstacle with room on both sides of it: no strand
                        // crosses it, whatever its length.
                        if !space.gap(e) && space.free(next) && !seen[next] {
                            short.push((f64::INFINITY, e));
                        }
                        if !space.gap(e) || !space.free(next) || seen[next] || space.barred(next, left.net) {
                            continue;
                        }
                        if need(e) > space.width(e) + 1e-9 {
                            short.push((need(e) - space.width(e), e));
                            continue;
                        }
                        seen[next] = true;
                        todo.push(next);
                    }
                }
                short.retain(|&(_, e)| !space.edges[e].t.iter().flatten().all(|&t| seen[t]));
                short.sort_by(|x, y| y.0.is_infinite().cmp(&x.0.is_infinite()).then(x.0.total_cmp(&y.0)));
                short.dedup_by_key(|x| x.1);
                (seen, short)
            };
            let (from, shut) = reach(a);
            if at(b).iter().any(|&t| from[t]) {
                println!("  alone on {}: a way", board.layers[l]);
                continue;
            }
            println!("  alone on {}: NO WAY", board.layers[l]);
            for (which, shut) in [(left.from, shut), (left.to, reach(b).1)] {
                println!("    shut in round {}:", name(which));
                for (by, e) in shut.into_iter().take(8) {
                    let [p, q] = space.edges[e].v.map(|v| space.verts[v]);
                    let of = |v: usize| space.vert_of[v].map_or("nothing".to_string(), |k| format!("{:?} {}", space.obstacles[k].what, space.obstacles[k].item.map_or(String::new(), name)));
                    if by.is_infinite() {
                        println!("      {:>7.3},{:>7.3} to {:>7.3},{:>7.3}: both corners of one obstacle ({}), taken for its side, with room on both sides", p[0], p[1], q[0], q[1], of(space.edges[e].v[0]));
                        continue;
                    }
                    println!("      {:>7.3},{:>7.3} ({}) to {:>7.3},{:>7.3} ({}): room {:.3}, short by {:.3}", p[0], p[1], of(space.edges[e].v[0]), q[0], q[1], of(space.edges[e].v[1]), space.width(e), by);
                }
            }
        }
        if std::env::var_os("LEFT_GAPS").is_none() {
            continue;
        }
        for (l, sheet) in plan.sheets.iter().enumerate() {
            let space = &sheet.space;
            for end in [left.from, left.to] {
                let Some(&o) = space.ends.get(&plan::End::Item(end)) else { continue };
                println!("  {} on {}{}", name(end), board.layers[l], if plan.closed.contains(&l) { " (closed)" } else { "" });
                let of = |v: usize| match space.vert_of[v] {
                    None => "nothing".to_string(),
                    Some(k) if k == o => "itself".to_string(),
                    Some(k) => {
                        let ob = &space.obstacles[k];
                        format!("{:?} {}{}", ob.what, ob.item.map_or(String::new(), name), if ob.net != 0 { format!(" ({})", board.net_name(ob.net)) } else { String::new() })
                    }
                };
                // The gaps out of the triangles that have a corner on the pad: the sides with none.
                let mut seen = std::collections::BTreeSet::new();
                for (t, tri) in space.tris.iter().enumerate() {
                    if !space.free(t) || !tri.v.iter().any(|&v| space.vert_of[v] == Some(o)) {
                        continue;
                    }
                    for (i, &e) in tri.e.iter().enumerate() {
                        let edge = &space.edges[e];
                        if space.vert_of[tri.v[i]] != Some(o) || !space.gap(e) || !seen.insert(e) {
                            continue;
                        }
                        let [a, b] = edge.v.map(|v| space.verts[v]);
                        let alone = space.keep(&board, edge.v[0], left.net) + space.keep(&board, edge.v[1], left.net) + plan::wide(&board, want);
                        let others: Vec<String> = sheet.nets_on(e).iter().map(|&(n, w)| format!("{} {w:.2}", board.net_name(n))).collect();
                        println!(
                            "    {:>7.3},{:>7.3} ({}) to {:>7.3},{:>7.3} ({}): room {:.3}, needs {:.3} alone{}{}{}",
                            a[0], a[1], of(edge.v[0]), b[0], b[1], of(edge.v[1]), space.width(e), alone,
                            if alone > space.width(e) + 1e-9 { " TOO NARROW" } else { "" },
                            if others.is_empty() { String::new() } else { format!("; in it: {} needing {:.3}", others.join(", "), sheet.need(&board, e, None)) },
                            if space.barred(t, left.net) { "; barred to the net" } else { "" }
                        );
                    }
                }
            }
        }
    }
}
