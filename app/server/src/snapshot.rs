//! `boardroom snapshot <out> <folder>...`: a static copy of some projects for
//! the web-only build on GitHub Pages, which has no server to ask.
//!
//! Writes `<out>/projects.json` (the same summaries `/api/projects` returns,
//! plus each project's document list and the directory listings the Renders
//! tab reads) and copies the files those pages show into `<out>/<id>/`.

use crate::config;
use crate::project;
use std::path::{Path, PathBuf};

/// What a snapshot copies from each project. Datasheets and fab outputs stay out.
const COPY: [&str; 4] = ["design", "viewer", "index.html", "pcbview.toml"];
const SKIP: [&str; 1] = ["design/datasheets"];

pub fn write(args: &[String]) -> Result<(), String> {
    let (out, folders) = args.split_first().ok_or("usage: boardroom snapshot <out> <folder>...")?;
    if folders.is_empty() {
        return Err("which project folders?".into());
    }
    let out = PathBuf::from(out);
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let mut all = vec![];
    for folder in folders {
        let root = std::fs::canonicalize(folder).map_err(|e| format!("{folder}: {e}"))?;
        let id = config::project_id(Path::new(folder));
        let mut summary = serde_json::to_value(project::summarize(&id, &root, false)).map_err(|e| e.to_string())?;
        // The machine's absolute path means nothing on a web page.
        summary["path"] = folder.as_str().into();
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
        println!("{folder} -> {}/{id}", out.display());
        all.push(summary);
    }
    let json = serde_json::to_string_pretty(&all).map_err(|e| e.to_string())?;
    std::fs::write(out.join("projects.json"), json).map_err(|e| e.to_string())
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
