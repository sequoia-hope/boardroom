//! What a run leaves on disk while it is still going: a trace of its stages, and the board as
//! far as it has come, written again every so often under a new name. A run that is stopped,
//! killed or crashes hours in has then left what it had, and one still going can be looked at
//! (owner, 2026-11-24: two runs stopped after seven and a half hours had written nothing).
//!
//! All of it is in `partial/` of the run's output directory:
//!
//! - `trace.log`: a line as each stage of each way of [`wire::route_watched`] ends, with the
//!   time since the run began, the way ([`wire::WAYS`]), what was done, how many connections
//!   the plan has no way for and how many wires could not be drawn, and the memory the program
//!   holds. Each line is on disk before the next stage starts, so the last line of a run that
//!   hangs says what it finished last.
//! - `NNNN-WAY.kicad_pcb`: the board with that way's wires as they were just drawn, an
//!   ordinary board file. The first drawing of each way is written, then one whenever that way
//!   has fewer not done than at any time before (no sooner than a fifth of the interval after
//!   its last), and one at least every interval. The numbers run on across the ways in the
//!   order written; the trace says which is which and how much each has not done. A way's
//!   counts before its pours are without the connections of the nets left to them, which are
//!   planned only once the pours are filled: its line says so, the fewest starts again
//!   there, and a count before is not to be set beside one after. These are
//!   stages of work in progress: wires the run would yet take out or lay again are in them.
//!
//! A run that ends writes its result beside `partial/` as ever, moves the trace there
//! (`trace.log`) and takes `partial/` away. A run that finds a `partial/` already there, left
//! by one that did not end, keeps it as `partial.prev/` (one such is kept).

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;

/// How often, in seconds, a way's board is written again where nothing else has it written.
pub const SAVE_EVERY: u64 = 600;

pub(crate) struct Saver<'a> {
    dir: PathBuf,
    text: &'a str,
    original: &'a board::Board,
    /// Seconds between a way's boards; none are written at 0.
    every: f64,
    started: Instant,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    trace: Option<File>,
    /// Boards written so far.
    written: usize,
    /// For each way: when its last board was written, and the fewest it has had not done.
    last: [Option<f64>; 3],
    fewest: [Option<usize>; 3],
    /// For each way: whether its last stage was before its pours ([`wire::Step::pours_to_come`]).
    before_pours: [bool; 3],
    /// Writing has failed and been said to have: no more is tried.
    failed: bool,
}

impl<'a> Saver<'a> {
    /// Sets `out/partial/` up for a run of `path`, keeping what a run that did not end left.
    pub(crate) fn new(out: &Path, path: &Path, text: &'a str, original: &'a board::Board, every: u64) -> Result<Self, String> {
        let dir = out.join("partial");
        let said = |p: &Path, e: std::io::Error| format!("{}: {e}", p.display());
        if dir.exists() {
            let prev = out.join("partial.prev");
            if prev.exists() {
                std::fs::remove_dir_all(&prev).map_err(|e| said(&prev, e))?;
            }
            std::fs::rename(&dir, &prev).map_err(|e| said(&dir, e))?;
        }
        std::fs::create_dir_all(&dir).map_err(|e| said(&dir, e))?;
        let trace = File::create(dir.join("trace.log")).map_err(|e| said(&dir.join("trace.log"), e))?;
        let saver = Saver { dir, text, original, every: every as f64, started: Instant::now(), state: Mutex::new(State { trace: Some(trace), ..Default::default() }) };
        let unix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        saver.say(&format!("routing {} from {} UTC; {}", path.display(), utc(unix), if every > 0 { format!("each way's board written here at least every {every} s") } else { "no boards written before the end".to_string() }));
        Ok(saver)
    }

    /// A line of the trace, on disk before this returns.
    pub(crate) fn say(&self, line: &str) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        self.line(&mut state, line);
    }

    fn line(&self, state: &mut State, line: &str) {
        let at = self.started.elapsed().as_secs_f64();
        let held = held_mb().map_or(String::new(), |mb| format!("  [{mb} MB]"));
        if let Some(trace) = state.trace.as_mut() {
            let wrote = writeln!(trace, "{at:9.1} s  {line}{held}").and_then(|_| trace.flush());
            if let Err(e) = wrote {
                self.fail(state, &format!("{}: {e}", self.dir.join("trace.log").display()));
            }
        }
    }

    fn fail(&self, state: &mut State, why: &str) {
        if !state.failed {
            eprintln!("boardroom: the run's trace and partial boards are no longer written: {why}");
        }
        state.failed = true;
        state.trace = None;
    }

    /// A stage of a way has ended: a line of the trace, and that way's board if it is time.
    pub(crate) fn step(&self, step: &wire::Step) {
        let way = wire::WAYS.get(step.way).copied().unwrap_or("?");
        let laid: usize = step.plan.sheets.iter().map(|s| s.laid().count()).sum();
        let drawn = step.drawing.map_or(String::new(), |d| format!(", {} wires drawn and {} not", d.wires.len(), d.undrawn.len()));
        let at = self.started.elapsed().as_secs_f64();
        // Whether this drawing is to be written, and under what number: settled with the
        // trace's line, so that the numbers follow the order of the lines.
        let number = {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            self.line(&mut state, &format!("{way:<6} {}: {laid} strands, {} vias, {} with no way{drawn}", step.what, step.plan.vias.len(), step.plan.left.len()));
            if step.way < 3 && state.before_pours[step.way] != step.pours_to_come {
                // What the pours leave apart is counted from here on: the fewest starts again.
                state.before_pours[step.way] = step.pours_to_come;
                state.fewest[step.way] = None;
            }
            let Some(drawing) = step.drawing.filter(|_| self.every > 0.0 && !state.failed && step.way < 3) else { return };
            let not_done = step.plan.left.len() + drawing.undrawn.len();
            let since = state.last[step.way].map(|last| at - last);
            let fewer = state.fewest[step.way].is_none_or(|f| not_done < f);
            if fewer {
                state.fewest[step.way] = Some(not_done);
            }
            let due = since.is_none_or(|s| s >= self.every || (fewer && s >= self.every / 5.0));
            if !due {
                return;
            }
            state.last[step.way] = Some(at);
            state.written += 1;
            (state.written, not_done, step.pours_to_come)
        };
        let Some(drawing) = step.drawing else { return };
        let (n, not_done, before_pours) = number;
        let basis = if !before_pours { "" } else { "; before the pours, so what they will leave apart is not among them" };
        let name = format!("{n:04}-{way}.kicad_pcb");
        // Written whole under another name and then given its own: a run killed while it
        // writes leaves no half of a board.
        let (tmp, to) = (self.dir.join(format!("{name}.tmp")), self.dir.join(&name));
        let wrote = std::fs::write(&tmp, wire::write(self.text, self.original, step.plan, drawing)).and_then(|_| std::fs::rename(&tmp, &to));
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        match wrote {
            Ok(()) => self.line(&mut state, &format!("{way:<6} written as partial/{name}: {not_done} not done (no way, or not drawn){basis}")),
            Err(e) => self.fail(&mut state, &format!("{}: {e}", to.display())),
        }
    }

    /// The run has ended and its result is written: the trace goes beside it and `partial/`
    /// is taken away.
    pub(crate) fn done(self, line: &str) {
        self.say(line);
        let out = self.dir.parent().map(Path::to_path_buf).unwrap_or_default();
        let state = self.state.into_inner().unwrap_or_else(|e| e.into_inner());
        if state.failed {
            return;
        }
        drop(state);
        if std::fs::rename(self.dir.join("trace.log"), out.join("trace.log")).is_ok() {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }
}

/// The memory the program holds now, MB, where the system says (Linux).
fn held_mb() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let kb: u64 = status.lines().find_map(|l| l.strip_prefix("VmRSS:"))?.split_whitespace().next()?.parse().ok()?;
    Some(kb / 1024)
}

/// Seconds since 1970 as a date and time of day.
fn utc(unix: u64) -> String {
    let (days, rest) = ((unix / 86_400) as i64, unix % 86_400);
    // Days to a date of the calendar in use (Howard Hinnant's `civil_from_days`).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let (d, m) = (doy - (153 * mp + 2) / 5 + 1, if mp < 10 { mp + 3 } else { mp - 9 });
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}", rest / 3_600, rest % 3_600 / 60, rest % 60)
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

    /// A run that never comes to its end (here: nobody calls `done`) has left a board that
    /// loads, with the wire drawn so far in it, and a trace that names it.
    #[test]
    fn a_run_that_does_not_end_has_left_its_board_so_far() {
        let d = std::env::temp_dir().join(format!("route-partial-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let pcb = d.join("t.kicad_pcb");
        std::fs::write(&pcb, BOARD).unwrap();
        let (original, text) = board::Board::load(&pcb).unwrap();
        let mut bare = original.clone();
        plan::strip(&mut bare);
        for (every, boards) in [(600, 1), (0, 0)] {
            let out = d.join(format!("out-{every}"));
            let saver = super::Saver::new(&out, &pcb, &text, &original, every).unwrap();
            wire::route_watched(&bare, &|step| saver.step(step), &|_, _| ());
            let trace = std::fs::read_to_string(out.join("partial/trace.log")).unwrap();
            assert!(trace.contains("first  drawn: 1 strands, 0 vias, 0 with no way, 1 wires drawn and 0 not"), "{trace}");
            let written: Vec<_> = std::fs::read_dir(out.join("partial")).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).filter(|n| n.ends_with(".kicad_pcb")).collect();
            assert_eq!(written.len(), boards, "{written:?}");
            if boards == 1 {
                assert_eq!(written, ["0001-first.kicad_pcb"]);
                assert!(trace.contains("written as partial/0001-first.kicad_pcb: 0 not done (no way, or not drawn)  ["), "{trace}");
                let (so_far, _) = board::Board::load(&out.join("partial/0001-first.kicad_pcb")).unwrap();
                assert!(so_far.items().any(|(_, i)| matches!(i.kind, board::Kind::Track)), "the wire is in it");
                assert_eq!(board::connectivity_without_pours(&so_far).airlines.len(), 0);
            }
        }
        std::fs::remove_dir_all(&d).ok();
    }

    /// A board drawn before its pours are come to is said to be: the nets left to the pours
    /// are not in the plan yet, and its count is not one to set beside those after.
    #[test]
    fn a_board_written_before_the_pours_says_so() {
        let poured = BOARD.trim_end().strip_suffix(')').unwrap().replace("(net 1 \"A\")", "(net 1 \"A\") (net 2 \"G\")")
            + r#" (footprint "t:R" (layer "F.Cu") (at 0 0) (property "Reference" "R3") (property "Value" "")
   (pad "1" smd rect (at 4 4) (size 1 1) (layers "F.Cu") (net 2 "G")))
 (footprint "t:R" (layer "F.Cu") (at 0 0) (property "Reference" "R4") (property "Value" "")
   (pad "1" smd rect (at 16 16) (size 1 1) (layers "F.Cu") (net 2 "G")))
 (zone (net 2) (net_name "G") (layer "B.Cu") (hatch edge 0.5) (connect_pads (clearance 0.2)) (min_thickness 0.25)
   (fill yes (thermal_gap 0.5) (thermal_bridge_width 0.5)) (polygon (pts (xy 1 1) (xy 19 1) (xy 19 19) (xy 1 19)))))
"#;
        let d = std::env::temp_dir().join(format!("route-partial-pours-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let pcb = d.join("t.kicad_pcb");
        std::fs::write(&pcb, poured).unwrap();
        let (original, text) = board::Board::load(&pcb).unwrap();
        let mut bare = original.clone();
        plan::strip(&mut bare);
        let saver = super::Saver::new(&d.join("out"), &pcb, &text, &original, 600).unwrap();
        wire::route_watched(&bare, &|step| saver.step(step), &|_, _| ());
        let trace = std::fs::read_to_string(d.join("out/partial/trace.log")).unwrap();
        assert!(trace.contains("written as partial/0001-first.kicad_pcb: 0 not done (no way, or not drawn); before the pours"), "{trace}");
        assert!(trace.contains("first  poured"), "{trace}");
        std::fs::remove_dir_all(&d).ok();
    }

    #[test]
    fn a_time_is_told_as_its_date() {
        assert_eq!(super::utc(0), "1970-01-01 00:00:00");
        assert_eq!(super::utc(1_795_564_805), "2026-11-25 00:00:05");
        assert_eq!(super::utc(951_782_400), "2000-02-29 00:00:00");
    }
}
