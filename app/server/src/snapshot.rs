//! `boardroom snapshot <out> <folder>...`: a static copy of some projects for
//! the web-only build on GitHub Pages, which has no server to ask.
//!
//! Writes `<out>/projects.json` (the same summaries `/api/projects` returns,
//! plus each project's document list and the directory listings the Renders
//! tab reads) and copies the files those pages show into `<out>/<id>/`.
//!
//! The apps the page opens read through the store (`app/store/store.js`), which falls through
//! to this copy, so it carries what they open too: each KiCad project
//! (`hw/<board>/<variant>/*.kicad_{pcb,pro,dru}`, not the routing runs in folders below) and,
//! for Composer, each board's placement problem, `compose/<board>/problem.json`. That is the
//! project's own if it has one, else `ccomp floorplan --problem` made from the board itself
//! (`--ccomp PATH`, or `$CCOMP`). `projects.json` lists them under `compose`.

use crate::config;
use crate::examples;
use crate::project;
use std::path::{Path, PathBuf};

/// What a snapshot copies from each project. Datasheets and fab outputs stay out.
const COPY: [&str; 4] = ["design", "viewer", "index.html", "pcbview.toml"];
const SKIP: [&str; 1] = ["design/datasheets"];
/// The KiCad files the apps open.
const KICAD: [&str; 3] = ["kicad_pcb", "kicad_pro", "kicad_dru"];

pub fn write(args: &[String]) -> Result<(), String> {
    const USAGE: &str = "usage: boardroom snapshot [--ccomp PATH] <out> <folder>...";
    let mut args = args.to_vec();
    let mut ccomp = std::env::var_os("CCOMP").map(PathBuf::from);
    if let Some(i) = args.iter().position(|a| a == "--ccomp") {
        args.remove(i);
        if i >= args.len() {
            return Err(USAGE.into());
        }
        ccomp = Some(PathBuf::from(args.remove(i)));
    }
    let (out, folders) = args.split_first().ok_or(USAGE)?;
    if folders.is_empty() {
        return Err("which project folders?".into());
    }
    let ccomp = ccomp.ok_or(
        "the snapshot makes each board's placement problem for Composer with circuit-composer's \
         ccomp: pass --ccomp PATH or set CCOMP (build it with `cargo build --release -p ccomp`)",
    )?;
    // A ccomp that won't run is a broken build, not a board Composer can't open.
    std::process::Command::new(&ccomp).arg("--version").output().ok().filter(|o| o.status.success()).ok_or(format!("{}: ccomp doesn't run", ccomp.display()))?;
    let out = PathBuf::from(out);
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let list = examples::load(&crate::repo_dir());
    let mut all = vec![];
    for folder in folders {
        let root = std::fs::canonicalize(folder).map_err(|e| format!("{folder}: {e}"))?;
        let id = config::project_id(Path::new(folder));
        let mut s = project::summarize(&id, &root, false);
        examples::tag(std::slice::from_mut(&mut s), &list, &crate::repo_dir());
        let boards: Vec<(String, String)> = s.boards.iter().map(|b| (b.board.clone(), b.variant.clone())).collect();
        // The machine's absolute path means nothing on a web page: show where
        // the project comes from instead.
        let shown = list
            .iter()
            .find(|e| Some(&e.name) == s.example.as_ref())
            .and_then(|e| e.repo.clone())
            .unwrap_or_else(|| folder.clone());
        let mut summary = serde_json::to_value(s).map_err(|e| e.to_string())?;
        summary["path"] = shown.into();
        summary["docs"] = docs(&root).into();
        let mut listings = serde_json::Map::new();
        for dir in walk_dirs(&root.join("design/renders")) {
            let rel = rel(&root, &dir);
            listings.insert(rel, listing(&dir));
        }
        summary["listings"] = listings.into();
        for item in COPY {
            copy(&root, &root.join(item), &out.join(&id))?;
        }
        for (board, variant) in &boards {
            for f in kicad_files(&root.join("hw").join(board).join(variant)) {
                copy(&root, &f, &out.join(&id))?;
            }
        }
        summary["compose"] = problems(&ccomp, &root, &out.join(&id), &boards).into();
        println!("{folder} -> {}/{id}", out.display());
        all.push(summary);
    }
    let json = serde_json::to_string_pretty(&all).map_err(|e| e.to_string())?;
    std::fs::write(out.join("projects.json"), json).map_err(|e| e.to_string())
}

/// A variant folder's own KiCad files, sorted.
fn kicad_files(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|entries| entries.flatten().map(|e| e.path()).filter(|p| p.is_file() && p.extension().is_some_and(|x| KICAD.iter().any(|k| x == *k))).collect())
        .unwrap_or_default();
    out.sort();
    out
}

/// Each board's placement problem, written to `<to>/compose/<board>/problem.json`, from its
/// last variant: `[{board, variant, pcb, problem}]`, or `error` in place of `problem` when there
/// is none. A board ccomp can't read doesn't stop the snapshot; Composer says it can't open it.
fn problems(ccomp: &Path, root: &Path, to: &Path, boards: &[(String, String)]) -> Vec<serde_json::Value> {
    let mut last: Vec<(&String, &String)> = vec![];
    for (board, variant) in boards {
        match last.last_mut() {
            Some((b, v)) if *b == board => *v = variant,
            _ => last.push((board, variant)),
        }
    }
    last.into_iter()
        .map(|(board, variant)| {
            let pcb = crate::project::board_pcb(root, board, variant).unwrap_or_else(|| format!("hw/{board}/{variant}/{board}.kicad_pcb"));
            let problem = format!("compose/{board}/problem.json");
            let mut entry = serde_json::json!({"board": board, "variant": variant, "pcb": pcb});
            let made = if root.join(&problem).is_file() {
                copy(root, &root.join(&problem), to).map(|_| "the project's own")
            } else if !root.join(&pcb).is_file() {
                Err(format!("no {pcb}"))
            } else {
                make_problem(ccomp, &root.join(&pcb), &to.join(&problem)).map(|_| "made from the board")
            };
            match made {
                Ok(how) => {
                    println!("  {problem}: {how}");
                    entry["problem"] = problem.into();
                }
                Err(e) => {
                    eprintln!("  {problem}: none: {e}");
                    entry["error"] = e.into();
                }
            }
            entry
        })
        .collect()
}

pub(crate) fn make_problem(ccomp: &Path, pcb: &Path, out: &Path) -> Result<(), String> {
    std::fs::create_dir_all(out.parent().unwrap()).map_err(|e| e.to_string())?;
    let run = std::process::Command::new(ccomp)
        .arg("floorplan")
        .arg(pcb)
        .args(["--nets-from-board", "--outline-from-board", "--problem", "-o"])
        .arg(out)
        .stdout(std::process::Stdio::null())
        .output()
        .map_err(|e| format!("{}: {e}", ccomp.display()))?;
    if !run.status.success() {
        let _ = std::fs::remove_file(out);
        let err = String::from_utf8_lossy(&run.stderr);
        return Err(format!("ccomp failed: {}", err.trim().lines().last().unwrap_or("")));
    }
    Ok(())
}

fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root).unwrap_or(path).to_string_lossy().into_owned()
}

fn docs(root: &Path) -> Vec<String> {
    let mut out = vec![];
    for dir in ["design", "design/gates"] {
        let Ok(entries) = std::fs::read_dir(root.join(dir)) else { continue };
        let mut names: Vec<String> = entries
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "md"))
            .map(|e| format!("{dir}/{}", e.file_name().to_string_lossy()))
            .collect();
        names.sort();
        out.extend(names);
    }
    out
}

fn walk_dirs(dir: &Path) -> Vec<PathBuf> {
    if !dir.is_dir() {
        return vec![];
    }
    let mut out = vec![dir.to_path_buf()];
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten().filter(|e| e.path().is_dir()) {
            out.extend(walk_dirs(&e.path()));
        }
    }
    out
}

fn listing(dir: &Path) -> serde_json::Value {
    let mut names: Vec<serde_json::Value> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|e| serde_json::json!({"name": e.file_name().to_string_lossy(), "dir": e.path().is_dir()}))
                .collect()
        })
        .unwrap_or_default();
    names.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    names.into()
}

fn copy(root: &Path, from: &Path, out: &Path) -> Result<(), String> {
    let rel = rel(root, from);
    if SKIP.iter().any(|s| rel == *s) || !from.exists() {
        return Ok(());
    }
    let to = out.join(&rel);
    if from.is_dir() {
        std::fs::create_dir_all(&to).map_err(|e| format!("{}: {e}", to.display()))?;
        for e in std::fs::read_dir(from).map_err(|e| e.to_string())?.flatten() {
            copy(root, &e.path(), out)?;
        }
        Ok(())
    } else {
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::copy(from, &to).map(|_| ()).map_err(|e| format!("{}: {e}", from.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn the_snapshot_carries_each_board_and_its_placement_problem() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("boardroom-snapshot-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let put = |rel: &str, body: &str| {
            let p = dir.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, body).unwrap();
        };
        // A stand-in for ccomp: `--version`, and the problem written to `-o` unless the board says no.
        put("ccomp", "#!/bin/sh\n[ \"$1\" = --version ] && exit 0\ngrep -q unreadable \"$2\" && { echo 'error: not a board' >&2; exit 1; }\nfor a; do [ \"$prev\" = -o ] && echo '{\"parts\":[]}' > \"$a\"; prev=$a; done\n");
        std::fs::set_permissions(dir.join("ccomp"), std::fs::Permissions::from_mode(0o755)).unwrap();
        crate::problem::runnable(&dir.join("ccomp"));
        put("p/hw/b/a/b.kicad_pcb", "(kicad_pcb old)");
        put("p/hw/b/b/b.kicad_pcb", "(kicad_pcb)");
        put("p/hw/b/b/b.kicad_pro", "{}");
        put("p/hw/b/b/b.kicad_dru", "(rules)");
        put("p/hw/b/b/b.kicad_prl", "{}");
        put("p/hw/b/b/route/s0.kicad_pcb", "(kicad_pcb run)");
        put("p/hw/own/a/own.kicad_pcb", "(kicad_pcb)");
        put("p/compose/own/problem.json", "{\"own\":true}");
        put("p/hw/bad/a/bad.kicad_pcb", "unreadable");
        let (p, out) = (dir.join("p"), dir.join("out"));
        let args = ["--ccomp", dir.join("ccomp").to_str().unwrap(), out.to_str().unwrap(), p.to_str().unwrap()].map(String::from);
        write(&args).unwrap();

        let id = config::project_id(&p);
        let to = out.join(&id);
        for f in ["hw/b/a/b.kicad_pcb", "hw/b/b/b.kicad_pcb", "hw/b/b/b.kicad_pro", "hw/b/b/b.kicad_dru"] {
            assert!(to.join(f).is_file(), "{f}");
        }
        // Not the editor's local settings, nor the routing runs.
        assert!(!to.join("hw/b/b/b.kicad_prl").exists());
        assert!(!to.join("hw/b/b/route").exists());
        assert_eq!(std::fs::read_to_string(to.join("compose/b/problem.json")).unwrap().trim(), "{\"parts\":[]}");
        assert_eq!(std::fs::read_to_string(to.join("compose/own/problem.json")).unwrap(), "{\"own\":true}");
        assert!(!to.join("compose/bad/problem.json").exists());

        let all: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(out.join("projects.json")).unwrap()).unwrap();
        let compose = &all[0]["compose"];
        assert_eq!(compose[0], serde_json::json!({"board": "b", "variant": "b", "pcb": "hw/b/b/b.kicad_pcb", "problem": "compose/b/problem.json"}));
        assert_eq!(compose[1]["board"], "bad");
        assert_eq!(compose[1]["error"], "ccomp failed: error: not a board");
        assert_eq!(compose[2]["problem"], "compose/own/problem.json");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
