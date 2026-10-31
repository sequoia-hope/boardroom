//! Reading a project folder into the summary the shell shows.
//!
//! `design/status.json` comes in two shapes: the original one the pcb-design
//! skill writes (`needs_user` a string, `phase` like "10 release",
//! `gates_passed` a list) and version 1 from `docs/shell-spec.md`. Both are
//! read into the same `Summary`.

use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const PHASES: [&str; 11] = [
    "Requirements",
    "Architecture and parts",
    "Schematic",
    "Net classes",
    "Mechanical brief",
    "Stackup and rules",
    "Placement",
    "Routing",
    "Simulation",
    "Silkscreen",
    "Release",
];

/// Phases whose gate the person signs off themselves (the skill's table).
pub const SIGNOFF: [u32; 3] = [0, 4, 10];

#[derive(Serialize)]
pub struct Summary {
    pub id: String,
    pub path: PathBuf,
    pub name: String,
    pub exists: bool,
    pub is_project: bool,
    pub pinned: bool,
    pub board: Option<String>,
    pub phase: Option<u32>,
    pub doing: Option<String>,
    pub needs_user: Vec<Need>,
    pub checks: Vec<Check>,
    pub gates: Vec<Gate>,
    pub links: Vec<Link>,
    pub apps: Value,
    pub updated: Option<String>,
    pub thumbnail: Option<String>,
    /// pcbview's site, if the project has built one.
    pub viewer: Option<String>,
    pub boards: Vec<Board>,
    pub git: Option<Git>,
    /// The name of the example (examples.toml) this project is a copy of.
    pub example: Option<String>,
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct Need {
    pub id: String,
    pub kind: String,
    pub text: String,
    pub gate: Option<u32>,
    pub options: Vec<String>,
}

#[derive(Serialize)]
pub struct Check {
    pub name: String,
    pub result: String,
    /// true = pass, false = fail, None = can't tell from the text.
    pub ok: Option<bool>,
}

#[derive(Serialize)]
pub struct Gate {
    pub phase: u32,
    pub name: &'static str,
    /// passed | current | open | stale | not_started
    pub state: String,
    pub signoff: bool,
    pub file: Option<String>,
    pub stale_because: Option<String>,
}

#[derive(Serialize)]
pub struct Link {
    pub label: String,
    pub href: String,
}

#[derive(Serialize)]
pub struct Board {
    pub board: String,
    pub variant: String,
    pub kicad_pro: Option<String>,
    /// The board file (`board_pcb`), when the variant has one.
    pub pcb: Option<String>,
}

#[derive(Serialize)]
pub struct Git {
    pub branch: String,
    pub dirty: usize,
    pub last_commit: String,
    pub last_date: String,
    /// `origin`, as an https URL when it is one GitHub-style host can show.
    pub remote: Option<String>,
    /// Commits ahead of and behind the branch's upstream, as of the last fetch.
    pub ahead: Option<u32>,
    pub behind: Option<u32>,
}

/// A remote URL in one comparable form: https, no `.git`, no trailing slash.
pub fn normalize_remote(url: &str) -> String {
    let url = url.trim().trim_end_matches('/').trim_end_matches(".git");
    if let Some(rest) = url.strip_prefix("git@") {
        if let Some((host, path)) = rest.split_once(':') {
            return format!("https://{host}/{path}");
        }
    }
    url.replacen("http://", "https://", 1)
}

pub fn summarize(id: &str, path: &Path, pinned: bool) -> Summary {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut s = Summary {
        id: id.to_string(),
        path: path.to_path_buf(),
        name,
        exists: path.is_dir(),
        is_project: false,
        pinned,
        board: None,
        phase: None,
        doing: None,
        needs_user: vec![],
        checks: vec![],
        gates: vec![],
        links: vec![],
        apps: Value::Null,
        updated: None,
        thumbnail: None,
        viewer: None,
        boards: vec![],
        git: None,
        example: None,
        error: None,
    };
    if !s.exists {
        return s;
    }
    let status_path = path.join("design/status.json");
    let status: Value = match std::fs::read_to_string(&status_path) {
        Ok(text) => match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => {
                s.error = Some(format!("design/status.json: {e}"));
                Value::Null
            }
        },
        Err(_) => Value::Null,
    };
    s.is_project = status_path.is_file();

    s.board = str_field(&status, "board");
    s.phase = match status.get("phase") {
        Some(Value::Number(n)) => n.as_u64().map(|n| n as u32),
        // The original shape: "10 release".
        Some(Value::String(text)) => text.split_whitespace().next().and_then(|t| t.parse().ok()),
        _ => None,
    };
    s.doing = str_field(&status, "doing");
    s.updated = str_field(&status, "updated");
    s.apps = status.get("apps").cloned().unwrap_or(Value::Null);
    s.needs_user = read_needs(&status);
    s.needs_user.extend(studio_failures(&status));
    s.checks = read_checks(&status);
    s.links = read_links(&status);
    s.gates = read_gates(&status, path, s.phase);
    s.thumbnail = ["design/renders/top.png", "design/renders/3d-top.png"]
        .into_iter()
        .find(|p| path.join(p).is_file())
        .map(str::to_string);
    s.viewer = path.join("viewer/index.html").is_file().then(|| "viewer/index.html".to_string());
    s.boards = find_boards(path);
    s.git = read_git(path);
    s
}

fn str_field(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string)
}

fn read_needs(status: &Value) -> Vec<Need> {
    match status.get("needs_user") {
        // Original shape: one free-text question.
        Some(Value::String(text)) if !text.trim().is_empty() => vec![Need {
            id: "needs-user".into(),
            kind: "question".into(),
            text: text.clone(),
            gate: None,
            options: vec![],
        }],
        Some(Value::Array(items)) => items
            .iter()
            .enumerate()
            .filter_map(|(i, item)| {
                let text = str_field(item, "text")?;
                Some(Need {
                    id: str_field(item, "id").unwrap_or_else(|| format!("need-{i}")),
                    kind: str_field(item, "kind").unwrap_or_else(|| "question".into()),
                    text,
                    gate: item.get("gate").and_then(Value::as_u64).map(|g| g as u32),
                    options: item
                        .get("options")
                        .and_then(Value::as_array)
                        .map(|o| o.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
                        .unwrap_or_default(),
                })
            })
            .collect(),
        _ => vec![],
    }
}

/// A Studio run that crashed, or left connections open when its clock ran
/// out, is something to look at: `apps.studio.<board>.failed`.
fn studio_failures(status: &Value) -> Vec<Need> {
    let Some(Value::Object(boards)) = status.pointer("/apps/studio") else { return vec![] };
    boards
        .iter()
        .filter_map(|(board, v)| {
            let failed = str_field(v, "failed")?;
            let opens = v.get("opens").and_then(Value::as_u64).unwrap_or(0);
            let text = match failed.as_str() {
                "crash" => format!("The router crashed routing {board}; see its run report"),
                "deadline" => format!("Routing {board} ran out of time with {opens} connection(s) open"),
                other => format!("Routing {board} failed ({other})"),
            };
            Some(Need { id: format!("studio-{board}-{failed}"), kind: "alert".into(), text, gate: Some(7), options: vec![] })
        })
        .collect()
}

fn read_checks(status: &Value) -> Vec<Check> {
    let Some(Value::Object(map)) = status.get("checks") else { return vec![] };
    map.iter()
        .map(|(name, v)| match v {
            // v1: {"result": "...", "ok": true}
            Value::Object(o) => Check {
                name: name.clone(),
                result: o.get("result").and_then(Value::as_str).unwrap_or("").to_string(),
                ok: o.get("ok").and_then(Value::as_bool),
            },
            // Original: free text. Judge only what the text says outright.
            other => {
                let result = other.as_str().map(str::to_string).unwrap_or_else(|| other.to_string());
                Check { name: name.clone(), ok: judge(&result), result }
            }
        })
        .collect()
}

fn judge(text: &str) -> Option<bool> {
    let t = text.to_ascii_lowercase();
    if t.contains("fail") {
        Some(false)
    } else if t.starts_with("pass") || t.starts_with("0 errors") || t.contains(" pass") {
        Some(true)
    } else {
        None
    }
}

fn read_links(status: &Value) -> Vec<Link> {
    status
        .get("links")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|l| Some(Link { label: str_field(l, "label")?, href: str_field(l, "href")? }))
                .collect()
        })
        .unwrap_or_default()
}

fn read_gates(status: &Value, root: &Path, phase: Option<u32>) -> Vec<Gate> {
    let passed: Vec<u32> = status
        .get("gates_passed")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|g| g.as_u64().map(|g| g as u32)).collect())
        .unwrap_or_default();
    let v1 = status.get("gates").and_then(Value::as_object);
    (0..PHASES.len() as u32)
        .map(|n| {
            let file = format!("design/gates/phase{n}.md");
            let file = root.join(&file).is_file().then_some(file);
            let entry = v1.and_then(|g| g.get(&n.to_string()));
            let stale_because = entry.and_then(|e| str_field(e, "stale_because"));
            // A reason to be stale wins over whatever state is written beside it.
            let state = if stale_because.is_some() {
                "stale".into()
            } else if let Some(state) = entry.and_then(|e| str_field(e, "state")) {
                state
            } else if passed.contains(&n) {
                "passed".into()
            } else if phase == Some(n) {
                "current".into()
            } else if file.is_some() {
                "open".into()
            } else {
                "not_started".into()
            };
            Gate {
                phase: n,
                name: PHASES[n as usize],
                state,
                signoff: SIGNOFF.contains(&n),
                file,
                stale_because,
            }
        })
        .collect()
}

/// A variant's board file, relative to the project: the `.kicad_pcb` named as its `.kicad_pro`
/// is (servodrive's ethernet board is `hw/ethernet/e/servodrive_E.kicad_pcb`), else
/// `<board>.kicad_pcb`, else the folder's only `.kicad_pcb`. Not the routing runs below it.
pub fn board_pcb(root: &Path, board: &str, variant: &str) -> Option<String> {
    let dir = format!("hw/{board}/{variant}");
    let files: Vec<String> = std::fs::read_dir(root.join(&dir))
        .ok()?
        .flatten()
        .filter(|e| e.path().is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    let pcbs: Vec<&String> = files.iter().filter(|f| f.ends_with(".kicad_pcb")).collect();
    let by_pro = files.iter().find_map(|f| f.strip_suffix(".kicad_pro")).map(|stem| format!("{stem}.kicad_pcb"));
    let named = format!("{board}.kicad_pcb");
    let chosen = by_pro
        .filter(|p| pcbs.contains(&p))
        .or_else(|| pcbs.contains(&&named).then_some(named))
        .or_else(|| (pcbs.len() == 1).then(|| pcbs[0].clone()))?;
    Some(format!("{dir}/{chosen}"))
}

/// KiCad projects live in `hw/<board>/<variant>/`.
fn find_boards(root: &Path) -> Vec<Board> {
    let mut boards = vec![];
    let Ok(hw) = std::fs::read_dir(root.join("hw")) else { return boards };
    for board in hw.flatten().filter(|e| e.path().is_dir()) {
        let Ok(variants) = std::fs::read_dir(board.path()) else { continue };
        for variant in variants.flatten().filter(|e| e.path().is_dir()) {
            let kicad_pro = std::fs::read_dir(variant.path()).ok().and_then(|files| {
                files
                    .flatten()
                    .map(|f| f.path())
                    .find(|p| p.extension().is_some_and(|e| e == "kicad_pro"))
                    .and_then(|p| p.strip_prefix(root).ok().map(|r| r.to_string_lossy().into_owned()))
            });
            let (name, var) = (board.file_name().to_string_lossy().into_owned(), variant.file_name().to_string_lossy().into_owned());
            let pcb = board_pcb(root, &name, &var);
            boards.push(Board { board: name, variant: var, kicad_pro, pcb });
        }
    }
    boards.sort_by(|a, b| (&a.board, &a.variant).cmp(&(&b.board, &b.variant)));
    boards
}

fn read_git(root: &Path) -> Option<Git> {
    let git = |args: &[&str]| -> Option<String> {
        let out = Command::new("git").arg("-C").arg(root).args(args).output().ok()?;
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
    };
    // Only report git for a folder that is its own repository's top, not a
    // folder that happens to sit inside some other checkout.
    let top = git(&["rev-parse", "--show-toplevel"])?;
    let inside = std::fs::canonicalize(&top).ok()? == std::fs::canonicalize(root).ok()?;
    let scope: &[&str] = if inside { &[] } else { &["--", "."] };
    let mut status_args = vec!["status", "--porcelain"];
    status_args.extend_from_slice(scope);
    let dirty = git(&status_args).map(|s| s.lines().filter(|l| !l.is_empty()).count()).unwrap_or(0);
    let mut log_args = vec!["log", "-1", "--format=%s%x1f%cs"];
    log_args.extend_from_slice(scope);
    let log = git(&log_args).unwrap_or_default();
    let (last_commit, last_date) = log.split_once('\x1f').unwrap_or((&log, ""));
    // "behind<TAB>ahead" against the upstream; no upstream, no numbers.
    let counts = git(&["rev-list", "--left-right", "--count", "@{upstream}...HEAD"]);
    let (behind, ahead) = counts
        .as_deref()
        .and_then(|c| c.split_once('\t'))
        .map(|(b, a)| (b.parse().ok(), a.parse().ok()))
        .unwrap_or((None, None));
    Some(Git {
        branch: git(&["branch", "--show-current"]).unwrap_or_default(),
        dirty,
        last_commit: last_commit.to_string(),
        last_date: last_date.to_string(),
        remote: git(&["remote", "get-url", "origin"]).map(|r| normalize_remote(&r)),
        ahead,
        behind,
    })
}
