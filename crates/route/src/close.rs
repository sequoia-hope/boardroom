//! The old router finishing what the new one left open (`boardroom route --close old`): the
//! new router's `routed.kicad_pcb` handed to the old router as it is, its copper kept and not
//! the old router's to rewrite, with what the pours join left to the pours
//! ([`board::Board::join_by_pours`]) and inner plane layers kept free of signals. What comes of
//! it is `closed.kicad_pcb`, beside the new router's own board, which stays as it was.
//!
//! The old router moves the copper near what it closes (its plough) and draws at any angle
//! and its own widths: the report says how many nets' copper it changed. On easy2350 that is
//! a dozen nets round the processor; on rgb-earring it is the back of half the board (lab
//! `planned/closer-2026-11-26`). `closed.kicad_pcb` keeps that copper as it was laid.
//! `--close grid` runs only its grid search ([`With::Grid`]): nothing on the board is moved, so
//! a few nets change and the board keeps the new router's look, and fewer connections close.
//!
//! The new router then draws it again ([`redraw`]; owner, 2026-11-26: "have the new router
//! redraw what the old one closes"): the way each of the old router's tracks takes is kept,
//! and its copper is drawn in 0/45/90 at our widths with our vias, the rest of the board as
//! copper that stays. That is `redrawn.kicad_pcb`. Copper that cannot be drawn again so is
//! left in it as the old router laid it, and the report says how much and why.
//!
//! It works to a time limit, so a closing that runs into it is not the same byte for byte
//! every time, as the new router's board is: the report says whether it did.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use serde_json::{Value, json};

/// How long the old router is given, seconds, where nothing else is said.
pub const SECONDS: f64 = 300.0;

/// The connections open on `board` with the pours filled round its copper, as the new
/// router's report counts them.
fn open(board: &board::Board) -> usize {
    board::connectivity_poured(board, &board::Pours::of(board)).airlines.len()
}

/// The checker's faults on `board` between copper and copper, and the pads a refill would
/// leave with too few thermal spokes. A pour as the file has it filled is no part of the
/// first (it is filled again round new copper, and KiCad's refill is the judge of that), nor
/// is the room kept for a spoke: copper in that room is no fault to KiCad, and what it costs
/// is the second figure.
fn faults(board: &board::Board) -> (usize, usize) {
    let aside = |id: board::ItemId| board.item(id).is_some_and(|i| i.refilled() || matches!(i.kind, board::Kind::Spoke { .. }));
    let between = board::check(board).iter().filter(|v| ![Some(v.a), v.b].into_iter().flatten().any(aside)).count();
    (between, board::Pours::likely(board).starved(board).len())
}

/// A track or via as figures to the micrometre: none for anything else.
fn figure(item: &board::Item) -> Option<(u64, [i64; 5])> {
    if !matches!(item.kind, board::Kind::Track | board::Kind::Via) {
        return None;
    }
    let board::Shape::Capsule { a, b, r } = &item.shape else { return None };
    let (a, b) = ((a.x / 1000, a.y / 1000), (b.x / 1000, b.y / 1000));
    let (a, b) = (a.min(b), a.max(b));
    Some((item.layers, [a.0, a.1, b.0, b.1, r / 1000]))
}

/// Each net's tracks and vias, as figures to the micrometre.
fn copper(board: &board::Board) -> BTreeMap<u32, BTreeSet<(u64, [i64; 5])>> {
    let mut out: BTreeMap<u32, BTreeSet<(u64, [i64; 5])>> = BTreeMap::new();
    for (_, item) in board.items() {
        if let Some(f) = figure(item) {
            out.entry(item.net).or_default().insert(f);
        }
    }
    out
}

/// How much of the old router closes what the new one leaves open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum With {
    /// Its grid search and its clean-up only: a way for each open connection through the room
    /// there is, nothing already on the board moved. Seconds a board, and a handful of nets
    /// changed (lab `patches/redraw-2026-11-27`, the ladder).
    Grid,
    /// Its whole recipe, the plough with it: copper in the way moved and laid again. It closes
    /// more, takes its whole time on a hard board, and changes many nets.
    Old,
}

impl With {
    /// The word for it on the command line and in the report.
    pub fn word(self) -> &'static str {
        match self {
            With::Grid => "grid",
            With::Old => "old",
        }
    }
}

/// Hands `out/routed.kicad_pcb`, the new router's board of the board at `path`, to the old
/// router (as much of it as `with` says) for `seconds` on `threads` threads, and writes `out/closed.kicad_pcb` where that
/// leaves fewer connections open, no more faults and no more pads short of spokes. What happened, which is also put into
/// `out/report.json` as `closed`.
///
/// The copper the old router laid is then drawn again by the new one ([`redraw`]), as far as
/// it can be: `out/redrawn.kicad_pcb`, where that is no worse than `closed.kicad_pcb`, and
/// what was drawn again and what left as laid in the report's `closed.redrawn`. `filled_vias`
/// is as for [`crate::route`].
pub fn close(path: &Path, out: &Path, with: With, seconds: f64, threads: usize, filled_vias: Option<bool>) -> Result<Value, String> {
    let routed = out.join("routed.kicad_pcb");
    // The board's rules go with it: the project and the custom rules beside each board file.
    for stem in ["routed", "closed", "redrawn"] {
        for suffix in ["kicad_pro", "kicad_dru"] {
            let (from, to) = (path.with_extension(suffix), out.join(format!("{stem}.{suffix}")));
            if from.exists() && !to.exists() {
                std::fs::copy(&from, &to).map_err(|e| format!("{}: {e}", to.display()))?;
            }
        }
    }
    let (before, text) = board::Board::load(&routed).map_err(|e| e.to_string())?;
    let (open_before, faults_before) = (open(&before), faults(&before));
    let said = if open_before == 0 {
        json!({"ran": false, "open_before": 0, "why": "nothing is open"})
    } else {
        let planes = !before.plane_layers(board::PLANE_SHARE).is_empty();
        let mut words: Vec<String> = ["route", &routed.to_string_lossy(), "-o", "closed.kicad_pcb", "--recipe", if with == With::Grid { "minimal" } else { "default" }, "--pours-join", "--no-retries", "--seed", "0", "--uuid-seed", "0"].map(String::from).to_vec();
        if planes {
            words.push("--auto-planes".into());
        }
        words.extend(["--threads".to_string(), threads.to_string(), "--timeout".to_string(), seconds.to_string()]);
        let mut args = ultraroute::parse(&words)?;
        // Its grid sized to the memory there is, as its own command does before it starts.
        ultraroute::fit_memory(&mut args, &before)?;
        let started = std::time::Instant::now();
        let closed = ultraroute::route_retrying(&args, before.clone(), &text, None, Arc::new(AtomicBool::new(false)));
        let took = started.elapsed().as_secs_f64();
        let file = out.join("closed.kicad_pcb");
        std::fs::write(&file, &closed.text).map_err(|e| format!("{}: {e}", file.display()))?;
        let (after, _) = board::Board::load(&file).map_err(|e| e.to_string())?;
        let (open_after, faults_after) = (open(&after), faults(&after));
        let (was, now) = (copper(&before), copper(&after));
        let changed: Vec<String> = was.keys().chain(now.keys()).collect::<BTreeSet<_>>().into_iter().filter(|n| was.get(n) != now.get(n)).map(|&n| before.net_name(n).to_string()).collect();
        let kept = open_after < open_before && faults_after.0 <= faults_before.0 && faults_after.1 <= faults_before.1;
        if !kept {
            std::fs::remove_file(&file).ok();
        }
        let again = out.join("redrawn.kicad_pcb");
        std::fs::remove_file(&again).ok();
        let redrawn = if kept { redraw(&routed, &file, &again, filled_vias)? } else { Value::Null };
        json!({
            "ran": true, "with": with.word(), "kept": kept, "redrawn": redrawn,
            "open_before": open_before, "open_after": open_after,
            "faults_before": faults_before.0, "faults_after": faults_after.0,
            "starved_before": faults_before.1, "starved_after": faults_after.1,
            "nets": was.len().max(now.len()), "nets_changed": changed.len(), "changed": changed,
            "planes_kept": planes, "seconds": (took * 10.0).round() / 10.0,
            "out_of_time": took >= seconds * 0.95,
            "board": if kept { Some("closed.kicad_pcb") } else { None },
        })
    };
    let report = out.join("report.json");
    if let Ok(mut whole) = std::fs::read_to_string(&report).map_err(|e| e.to_string()).and_then(|t| serde_json::from_str::<Value>(&t).map_err(|e| e.to_string())) {
        whole["closed"] = said.clone();
        std::fs::write(&report, serde_json::to_string_pretty(&whole).map_err(|e| e.to_string())?).map_err(|e| format!("{}: {e}", report.display()))?;
    }
    Ok(said)
}

/// The tracks and vias the board at `after` has that the board at `before` has not, drawn
/// again on `after` by the new router ([`wire::redraw`]): the way each takes kept, its copper
/// in 0/45/90 at our widths and our vias. Written to `out` where that leaves no more open, no
/// more faults and no more pads short of spokes than `after` has. A net that cannot be drawn
/// again so keeps its copper as `after` has it, and what comes back names each with why.
pub fn redraw(before: &Path, after: &Path, out: &Path, filled_vias: Option<bool>) -> Result<Value, String> {
    let load = |path: &Path| {
        let (mut board, text) = board::Board::load(path).map_err(|e| e.to_string())?;
        if let Some(filled) = filled_vias {
            board.filled_vias = filled;
        }
        Ok::<_, String>((board, text))
    };
    let (was, _) = load(before)?;
    let (now, text) = load(after)?;
    let (a, b) = (copper(&was), copper(&now));
    let nets: Vec<u32> = a.keys().chain(b.keys()).collect::<BTreeSet<_>>().into_iter().filter(|n| a.get(n) != b.get(n)).copied().collect();
    // What is to be drawn again: the tracks and vias `after` has that `before` has not. The
    // rest of each net is the new router's own, as it drew it, and stays.
    let new: Vec<board::ItemId> = now.items().filter(|(_, i)| !i.locked && figure(i).is_some_and(|f| !a.get(&i.net).is_some_and(|had| had.contains(&f)))).map(|(id, _)| id).collect();
    let started = std::time::Instant::now();
    let redrawn = wire::redraw::redraw(&now, &new);
    let took = started.elapsed().as_secs_f64();
    std::fs::write(out, wire::write_over(&text, &now, &redrawn.plan, &redrawn.drawing, &|id| redrawn.lifted.contains(&id))).map_err(|e| format!("{}: {e}", out.display()))?;
    let (again, _) = load(out)?;
    let (open_before, faults_before) = (open(&now), faults(&now));
    let (open_after, faults_after) = (open(&again), faults(&again));
    let kept = !redrawn.lifted.is_empty() && open_after <= open_before && faults_after.0 <= faults_before.0 && faults_after.1 <= faults_before.1;
    if !kept {
        std::fs::remove_file(out).ok();
    }
    let names = |ids: &mut dyn Iterator<Item = board::ItemId>| ids.filter_map(|id| now.item(id)).map(|i| now.net_name(i.net).to_string()).collect::<BTreeSet<String>>();
    let (touched, drawn) = (names(&mut new.iter().copied()), names(&mut redrawn.lifted.iter().copied()));
    let left = names(&mut new.iter().copied().filter(|id| !redrawn.lifted.contains(id)));
    Ok(json!({
        "kept": kept,
        "nets_changed": nets.len(),
        "pieces_new": new.len(), "pieces_redrawn": redrawn.lifted.len(),
        "nets_new": touched.len(),
        "nets_redrawn_whole": drawn.difference(&left).count(),
        "nets_redrawn_in_part": drawn.intersection(&left).count(),
        "nets_as_laid": left.difference(&drawn).count(),
        "as_laid": redrawn.as_laid.iter().map(|(n, net, why)| json!({"pieces": n, "net": now.net_name(*net), "why": why})).collect::<Vec<_>>(),
        "wires": redrawn.drawing.wires.len(), "vias": redrawn.plan.vias.len(),
        "as_planned": redrawn.drawing.wires.iter().filter(|w| w.as_planned).count(),
        "open_before": open_before, "open_after": open_after,
        "faults_before": faults_before.0, "faults_after": faults_after.0,
        "starved_before": faults_before.1, "starved_after": faults_after.1,
        "seconds": (took * 10.0).round() / 10.0,
        "board": if kept { out.file_name().map(|n| n.to_string_lossy().to_string()) } else { None },
    }))
}

#[cfg(test)]
mod tests {
    const BOARD: &str = r#"(kicad_pcb (version 20240108) (generator "pcbnew") (general (thickness 1.6))
 (layers (0 "F.Cu" signal) (31 "B.Cu" signal) (44 "Edge.Cuts" user))
 (net 0 "") (net 1 "A")
 (gr_rect (start 0 0) (end 20 20) (layer "Edge.Cuts") (width 0.1))
 (footprint "t:R" (layer "F.Cu") (at 0 0) (property "Reference" "R1") (property "Value" "")
   (pad "1" smd rect (at 4 10) (size 1 1) (layers "F.Cu") (net 1 "A")))
 (footprint "t:R" (layer "F.Cu") (at 0 0) (property "Reference" "R2") (property "Value" "")
   (pad "1" smd rect (at 16 10) (size 1 1) (layers "F.Cu") (net 1 "A"))))
"#;

    /// A board the new router left a connection open on is closed by the old router, and what
    /// it did is in the report; a board with nothing open is left alone.
    #[test]
    fn what_is_left_open_is_closed_by_the_old_router() {
        let d = std::env::temp_dir().join(format!("route-close-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        // As if the new router had drawn nothing: its board is the bare one.
        std::fs::write(d.join("routed.kicad_pcb"), BOARD).unwrap();
        std::fs::write(d.join("report.json"), "{}").unwrap();
        // By its grid search alone first: this one needs nothing moved.
        let said = super::close(&d.join("t.kicad_pcb"), &d, super::With::Grid, 60.0, 1, None).unwrap();
        assert_eq!((said["with"].as_str(), said["kept"].as_bool(), said["open_after"].as_u64()), (Some("grid"), Some(true), Some(0)), "{said}");
        assert_eq!(said["redrawn"]["pieces_redrawn"], said["redrawn"]["pieces_new"], "{said}");
        let said = super::close(&d.join("t.kicad_pcb"), &d, super::With::Old, 60.0, 1, None).unwrap();
        assert_eq!((said["with"].as_str(), said["ran"].as_bool(), said["kept"].as_bool()), (Some("old"), Some(true), Some(true)), "{said}");
        assert_eq!((said["open_before"].as_u64(), said["open_after"].as_u64(), said["nets_changed"].as_u64()), (Some(1), Some(0), Some(1)), "{said}");
        let (closed, _) = board::Board::load(&d.join("closed.kicad_pcb")).unwrap();
        assert!(closed.items().any(|(_, i)| matches!(i.kind, board::Kind::Track)), "the old router's track is in it");
        // And the new router has drawn that track again: all of it, and the pads still joined.
        let again = &said["redrawn"];
        assert_eq!(again["kept"].as_bool(), Some(true), "{again}");
        assert_eq!(again["pieces_redrawn"], again["pieces_new"], "{again}");
        assert_eq!((again["nets_redrawn_whole"].as_u64(), again["open_after"].as_u64()), (Some(1), Some(0)), "{again}");
        let (redrawn, _) = board::Board::load(&d.join("redrawn.kicad_pcb")).unwrap();
        assert_eq!(board::connectivity_without_pours(&redrawn).airlines.len(), 0);
        assert_eq!(board::connectivity_without_pours(&closed).airlines.len(), 0);
        let report: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(d.join("report.json")).unwrap()).unwrap();
        assert_eq!(report["closed"]["open_after"].as_u64(), Some(0));
        // The closed board has nothing open: asked again of it, the old router is not run.
        std::fs::copy(d.join("closed.kicad_pcb"), d.join("routed.kicad_pcb")).unwrap();
        std::fs::remove_file(d.join("closed.kicad_pcb")).unwrap();
        let said = super::close(&d.join("t.kicad_pcb"), &d, super::With::Old, 60.0, 1, None).unwrap();
        assert_eq!(said["ran"].as_bool(), Some(false), "{said}");
        assert!(!d.join("closed.kicad_pcb").exists());
        std::fs::remove_dir_all(&d).ok();
    }

    /// The copper one board has and another has not is drawn again on it by the new router,
    /// in 0/45/90, and the rest stays as it is: here a track laid at an odd angle beside one
    /// both boards have.
    #[test]
    fn copper_one_board_has_and_another_has_not_is_drawn_again() {
        let d = std::env::temp_dir().join(format!("route-redraw-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let with = |copper: &str| BOARD.trim_end().strip_suffix(')').unwrap().to_string() + copper + ")\n";
        let stays = r#" (footprint "t:R" (layer "F.Cu") (at 0 0) (property "Reference" "R3") (property "Value" "")
   (pad "1" smd rect (at 4 14) (size 1 1) (layers "F.Cu") (net 1 "A")))
 (segment (start 4 10) (end 4 14) (width 0.25) (layer "F.Cu") (net 1))"#;
        let laid = r#" (segment (start 4 10) (end 9.3 8.1) (width 0.25) (layer "F.Cu") (net 1))
 (segment (start 9.3 8.1) (end 16 10) (width 0.25) (layer "F.Cu") (net 1))"#;
        std::fs::write(d.join("before.kicad_pcb"), with(stays)).unwrap();
        std::fs::write(d.join("after.kicad_pcb"), with(&format!("{stays}{laid}"))).unwrap();
        let said = super::redraw(&d.join("before.kicad_pcb"), &d.join("after.kicad_pcb"), &d.join("out.kicad_pcb"), None).unwrap();
        assert_eq!((said["kept"].as_bool(), said["pieces_new"].as_u64(), said["pieces_redrawn"].as_u64()), (Some(true), Some(2), Some(2)), "{said}");
        let (out, _) = board::Board::load(&d.join("out.kicad_pcb")).unwrap();
        assert_eq!(board::connectivity_without_pours(&out).airlines.len(), 0);
        let mm = |p: board::Point| (p.x as f64 / 1e6, p.y as f64 / 1e6);
        let tracks: Vec<((f64, f64), (f64, f64))> = out
            .items()
            .filter(|(_, i)| matches!(i.kind, board::Kind::Track))
            .filter_map(|(_, i)| match i.shape {
                board::Shape::Capsule { a, b, .. } => Some((mm(a), mm(b))),
                _ => None,
            })
            .collect();
        // The track both boards have is as it was, and no track lies at an odd angle.
        assert!(tracks.iter().any(|&(a, b)| (a, b) == ((4.0, 10.0), (4.0, 14.0)) || (b, a) == ((4.0, 10.0), (4.0, 14.0))), "{tracks:?}");
        for (a, b) in &tracks {
            let (dx, dy) = ((a.0 - b.0).abs(), (a.1 - b.1).abs());
            assert!(dx < 1e-6 || dy < 1e-6 || (dx - dy).abs() < 1e-6, "{tracks:?}");
        }
        std::fs::remove_dir_all(&d).ok();
    }
}
