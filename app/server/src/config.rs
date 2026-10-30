//! The project list: `~/.config/boardroom/projects.toml`.
//!
//! A project is a folder anywhere on disk. The list holds its path and whether
//! it is pinned to the top; everything else is read from the folder itself.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub path: PathBuf,
    #[serde(default)]
    pub pinned: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ProjectList {
    #[serde(default)]
    pub projects: Vec<Entry>,
}

pub fn config_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".config"));
    base.join("boardroom")
}

pub fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"))
}

fn list_path() -> PathBuf {
    config_dir().join("projects.toml")
}

impl ProjectList {
    pub fn load() -> Result<Self, String> {
        let path = list_path();
        match std::fs::read_to_string(&path) {
            Ok(text) => toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let path = list_path();
        std::fs::create_dir_all(config_dir()).map_err(|e| e.to_string())?;
        let text = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        // Write then rename, so a crash never leaves half a list.
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
    }

    /// Adds a folder; returns its canonical path. Adding one twice is not an error.
    pub fn add(&mut self, path: &Path) -> Result<PathBuf, String> {
        let path = std::fs::canonicalize(path).map_err(|e| format!("{}: {e}", path.display()))?;
        if !path.is_dir() {
            return Err(format!("{} is not a folder", path.display()));
        }
        if !self.projects.iter().any(|p| p.path == path) {
            self.projects.push(Entry { path: path.clone(), pinned: false });
        }
        Ok(path)
    }

    pub fn remove(&mut self, path: &Path) -> bool {
        let before = self.projects.len();
        self.projects.retain(|p| p.path != path);
        self.projects.len() != before
    }
}

/// A short, stable id for a project path, used in URLs (FNV-1a, 64 bit).
pub fn project_id(path: &Path) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in path.as_os_str().as_encoded_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{:012x}", hash & 0xffff_ffff_ffff)
}
