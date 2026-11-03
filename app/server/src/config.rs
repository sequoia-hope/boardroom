//! Boardroom's configuration in `~/.config/boardroom/`:
//! `projects.toml` (the project list) and `config.toml` (machine paths such as
//! `ccomp`, `skill_upstream`, and static app builds like `waffle`).
//!
//! A project is a folder anywhere on disk. The list holds its path and whether
//! it is pinned to the top; everything else is read from the folder itself.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Config {
    pub waffle: Option<PathBuf>,
}

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

pub fn expand_tilde(path: &Path) -> PathBuf {
    match path.to_str().and_then(|s| s.strip_prefix("~/")) {
        Some(rest) => home().join(rest),
        None => path.to_path_buf(),
    }
}

impl Config {
    pub fn load_from(dir: &Path) -> Result<Self, String> {
        let path = dir.join("config.toml");
        let mut cfg: Self = match std::fs::read_to_string(&path) {
            Ok(text) => toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(e) => return Err(format!("{}: {e}", path.display())),
        };
        cfg.waffle = cfg.waffle.filter(|p| !p.as_os_str().is_empty()).map(|p| expand_tilde(&p));
        Ok(cfg)
    }
}

impl ProjectList {
    pub fn load() -> Result<Self, String> {
        Self::load_from(&config_dir())
    }

    pub fn load_from(dir: &Path) -> Result<Self, String> {
        let path = dir.join("projects.toml");
        match std::fs::read_to_string(&path) {
            Ok(text) => toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        self.save_to(&config_dir())
    }

    pub fn save_to(&self, dir: &Path) -> Result<(), String> {
        let path = dir.join("projects.toml");
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_reads_waffle_alongside_other_machine_keys() {
        let dir = std::env::temp_dir().join(format!("boardroom-config-{}-{:?}", std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // Missing config.toml is fine: static apps are simply unconfigured.
        assert_eq!(Config::load_from(&dir).unwrap().waffle, None);

        std::fs::write(
            dir.join("config.toml"),
            "skill_upstream = \"/tmp/skill\"\nccomp = \"/tmp/ccomp\"\nwaffle = \"~/waffle/app/build\"\n",
        )
        .unwrap();
        assert_eq!(Config::load_from(&dir).unwrap().waffle, Some(home().join("waffle/app/build")));

        std::fs::write(dir.join("config.toml"), "waffle = \"\"\n").unwrap();
        assert_eq!(Config::load_from(&dir).unwrap().waffle, None);

        std::fs::write(dir.join("config.toml"), "waffle = [not a path\n").unwrap();
        assert!(Config::load_from(&dir).is_err());

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
