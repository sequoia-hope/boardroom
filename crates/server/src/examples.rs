//! `examples.toml`: the example projects, named by git remote or by a path in
//! this repository, never by a path on someone's machine.

use crate::project::{Summary, normalize_remote};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Example {
    pub name: String,
    /// Inside this repository.
    pub path: Option<String>,
    /// In its own repository, at `ref` (a branch or a commit).
    pub repo: Option<String>,
    #[serde(rename = "ref")]
    pub git_ref: Option<String>,
}

#[derive(Deserialize)]
struct File {
    #[serde(default)]
    examples: Vec<Example>,
}

pub fn load(repo_dir: &Path) -> Vec<Example> {
    let path = repo_dir.join("examples.toml");
    let Ok(text) = std::fs::read_to_string(&path) else { return vec![] };
    match toml::from_str::<File>(&text) {
        Ok(f) => f.examples,
        Err(e) => {
            eprintln!("boardroom: {}: {e}", path.display());
            vec![]
        }
    }
}

impl Example {
    /// Is this project folder (with this `origin`) a copy of the example?
    pub fn matches(&self, repo_dir: &Path, folder: &Path, remote: Option<&str>) -> bool {
        if let Some(rel) = &self.path {
            let inside = std::fs::canonicalize(repo_dir.join(rel)).ok();
            return inside.as_deref() == std::fs::canonicalize(folder).ok().as_deref();
        }
        match (&self.repo, remote) {
            (Some(repo), Some(remote)) => normalize_remote(repo) == normalize_remote(remote),
            _ => false,
        }
    }

    /// Where the example lives in this repository, if it does.
    pub fn local_path(&self, repo_dir: &Path) -> Option<PathBuf> {
        self.path.as_ref().map(|p| repo_dir.join(p))
    }
}

/// Marks each summary that is a copy of an example.
pub fn tag(summaries: &mut [Summary], examples: &[Example], repo_dir: &Path) {
    for s in summaries {
        let remote = s.git.as_ref().and_then(|g| g.remote.as_deref());
        s.example = examples.iter().find(|e| e.matches(repo_dir, &s.path, remote)).map(|e| e.name.clone());
    }
}
