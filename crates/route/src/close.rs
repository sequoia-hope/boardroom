//! The old router finishing what the new one left open (`boardroom route --close old`): the
//! new router's `routed.kicad_pcb` handed to the old router as it is, its copper kept and not
//! the old router's to rewrite, with what the pours join left to the pours
//! ([`board::Board::join_by_pours`]) and inner plane layers kept free of signals. What comes of
//! it is `closed.kicad_pcb`, beside the new router's own board, which stays as it was.
//!
//! The old router's copper is kept as it lays it. It moves the copper near what it closes
//! (its plough) and draws at any angle: the report says how many nets' copper it changed, and
//! the board is to be looked at. On easy2350 that is a dozen nets round the processor; on
//! rgb-earring it is the back of half the board (lab `planned/closer-2026-11-26`).
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

/// Each net's tracks and vias, as figures to the micrometre.
fn copper(board: &board::Board) -> BTreeMap<u32, BTreeSet<(u64, [i64; 5])>> {
    let mut out: BTreeMap<u32, BTreeSet<(u64, [i64; 5])>> = BTreeMap::new();
    for (_, item) in board.items() {
        if !matches!(item.kind, board::Kind::Track | board::Kind::Via) {
            continue;
        }
        if let board::Shape::Capsule { a, b, r } = &item.shape {
            let (a, b) = ((a.x / 1000, a.y / 1000), (b.x / 1000, b.y / 1000));
            let (a, b) = (a.min(b), a.max(b));
            out.entry(item.net).or_default().insert((item.layers, [a.0, a.1, b.0, b.1, r / 1000]));
        }
    }
    out
}

/// Hands `out/routed.kicad_pcb`, the new router's board of the board at `path`, to the old
/// router for `seconds` on `threads` threads, and writes `out/closed.kicad_pcb` where that
/// leaves fewer connections open, no more faults and no more pads short of spokes. What happened, which is also put into
/// `out/report.json` as `closed`.
pub fn close(path: &Path, out: &Path, seconds: f64, threads: usize) -> Result<Value, String> {
    let routed = out.join("routed.kicad_pcb");
    // The board's rules go with it: the project and the custom rules beside each board file.
    for stem in ["routed", "closed"] {
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
        let mut words: Vec<String> = ["route", &routed.to_string_lossy(), "-o", "closed.kicad_pcb", "--recipe", "default", "--pours-join", "--no-retries", "--seed", "0", "--uuid-seed", "0"].map(String::from).to_vec();
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
        json!({
            "ran": true, "kept": kept,
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
        let said = super::close(&d.join("t.kicad_pcb"), &d, 60.0, 1).unwrap();
        assert_eq!((said["ran"].as_bool(), said["kept"].as_bool()), (Some(true), Some(true)), "{said}");
        assert_eq!((said["open_before"].as_u64(), said["open_after"].as_u64(), said["nets_changed"].as_u64()), (Some(1), Some(0), Some(1)), "{said}");
        let (closed, _) = board::Board::load(&d.join("closed.kicad_pcb")).unwrap();
        assert!(closed.items().any(|(_, i)| matches!(i.kind, board::Kind::Track)), "the old router's track is in it");
        assert_eq!(board::connectivity_without_pours(&closed).airlines.len(), 0);
        let report: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(d.join("report.json")).unwrap()).unwrap();
        assert_eq!(report["closed"]["open_after"].as_u64(), Some(0));
        // The closed board has nothing open: asked again of it, the old router is not run.
        std::fs::copy(d.join("closed.kicad_pcb"), d.join("routed.kicad_pcb")).unwrap();
        std::fs::remove_file(d.join("closed.kicad_pcb")).unwrap();
        let said = super::close(&d.join("t.kicad_pcb"), &d, 60.0, 1).unwrap();
        assert_eq!(said["ran"].as_bool(), Some(false), "{said}");
        assert!(!d.join("closed.kicad_pcb").exists());
        std::fs::remove_dir_all(&d).ok();
    }
}
