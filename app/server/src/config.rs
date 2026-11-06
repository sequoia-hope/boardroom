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
    pub claude: Option<PathBuf>,
    pub waffle_relay: Option<String>,
    pub waffle_relay_port: Option<u16>,
    #[serde(default, deserialize_with = "deserialize_term_command")]
    pub term_command: Option<Vec<String>>,
}

fn deserialize_term_command<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Raw {
        Line(String),
        Argv(Vec<String>),
    }
    match Option::<Raw>::deserialize(deserializer)? {
        None => Ok(None),
        Some(Raw::Line(s)) => {
            if s.trim().is_empty() {
                Ok(None)
            } else {
                split_cmdline(&s)
                    .map(Some)
                    .map_err(serde::de::Error::custom)
            }
        }
        Some(Raw::Argv(v)) => {
            if v.is_empty() || v[0].trim().is_empty() {
                Ok(None)
            } else {
                Ok(Some(v.into_iter().map(|s| expand_tilde_str(&s)).collect()))
            }
        }
    }
}

pub fn expand_tilde_str(s: &str) -> String {
    match s.strip_prefix("~/") {
        Some(rest) => home().join(rest).to_string_lossy().into_owned(),
        None => s.to_string(),
    }
}

/// Splits a command line into arguments (whitespace, single and double quotes,
/// backslash escapes), expanding a leading `~/` on each word.
pub fn split_cmdline(input: &str) -> Result<Vec<String>, String> {
    let mut args = Vec::new();
    let mut cur = String::new();
    let mut in_word = false;
    let mut quote: Option<char> = None;
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        match quote {
            Some('\'') => {
                if c == '\'' {
                    quote = None;
                } else {
                    cur.push(c);
                }
            }
            Some('"') => match c {
                '"' => quote = None,
                '\\' => match chars.peek().copied() {
                    Some(esc @ ('"' | '\\' | '$' | '`')) => {
                        chars.next();
                        cur.push(esc);
                    }
                    Some(_) => cur.push('\\'),
                    None => return Err("trailing backslash in command line".into()),
                },
                other => cur.push(other),
            },
            Some(_) => unreachable!(),
            None => match c {
                ws if ws.is_whitespace() => {
                    if in_word {
                        args.push(expand_tilde_str(&cur));
                        cur.clear();
                        in_word = false;
                    }
                }
                '\'' | '"' => {
                    in_word = true;
                    quote = Some(c);
                }
                '\\' => {
                    in_word = true;
                    let esc = chars
                        .next()
                        .ok_or_else(|| "trailing backslash in command line".to_string())?;
                    cur.push(esc);
                }
                other => {
                    in_word = true;
                    cur.push(other);
                }
            },
        }
    }
    if quote.is_some() {
        return Err("unclosed quote in command line".into());
    }
    if in_word {
        args.push(expand_tilde_str(&cur));
    }
    if args.is_empty() {
        return Err("empty command line".into());
    }
    Ok(args)
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
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
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
        cfg.waffle = cfg
            .waffle
            .filter(|p| !p.as_os_str().is_empty())
            .map(|p| expand_tilde(&p));
        cfg.claude = cfg
            .claude
            .filter(|p| !p.as_os_str().is_empty())
            .map(|p| expand_tilde(&p));
        cfg.waffle_relay = cfg
            .waffle_relay
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
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
            self.projects.push(Entry {
                path: path.clone(),
                pinned: false,
            });
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
        let dir = std::env::temp_dir().join(format!(
            "boardroom-config-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // Missing config.toml is fine: static apps are simply unconfigured.
        assert_eq!(Config::load_from(&dir).unwrap().waffle, None);

        std::fs::write(
            dir.join("config.toml"),
            "skill_upstream = \"/tmp/skill\"\nccomp = \"/tmp/ccomp\"\nwaffle = \"~/waffle/app/build\"\n",
        )
        .unwrap();
        assert_eq!(
            Config::load_from(&dir).unwrap().waffle,
            Some(home().join("waffle/app/build"))
        );

        std::fs::write(dir.join("config.toml"), "waffle = \"\"\n").unwrap();
        assert_eq!(Config::load_from(&dir).unwrap().waffle, None);

        std::fs::write(dir.join("config.toml"), "waffle = [not a path\n").unwrap();
        assert!(Config::load_from(&dir).is_err());

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn config_reads_terminal_and_relay_keys() {
        let dir = std::env::temp_dir().join(format!(
            "boardroom-config-term-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let ephemeral_port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();

        std::fs::write(
            dir.join("config.toml"),
            format!(
                "claude = \"~/bin/claude\"\n\
                 waffle_relay = \"uv run --project '/tmp/my relay' waffle-mcp-relay\"\n\
                 waffle_relay_port = {ephemeral_port}\n\
                 term_command = \"/bin/sh -c 'echo ok'\"\n"
            ),
        )
        .unwrap();

        let cfg = Config::load_from(&dir).unwrap();
        assert_eq!(cfg.claude, Some(home().join("bin/claude")));
        assert_eq!(
            cfg.waffle_relay.as_deref(),
            Some("uv run --project '/tmp/my relay' waffle-mcp-relay")
        );
        assert_eq!(cfg.waffle_relay_port, Some(ephemeral_port));
        assert_eq!(
            cfg.term_command,
            Some(vec!["/bin/sh".into(), "-c".into(), "echo ok".into()])
        );

        let argv = split_cmdline(cfg.waffle_relay.as_deref().unwrap()).unwrap();
        assert_eq!(
            argv,
            vec![
                "uv",
                "run",
                "--project",
                "/tmp/my relay",
                "waffle-mcp-relay"
            ]
        );

        std::fs::write(
            dir.join("config.toml"),
            "claude = \"\"\nwaffle_relay = \"   \"\nterm_command = [\"/bin/sh\"]\n",
        )
        .unwrap();
        let cfg2 = Config::load_from(&dir).unwrap();
        assert_eq!(cfg2.claude, None);
        assert_eq!(cfg2.waffle_relay, None);
        assert_eq!(cfg2.term_command, Some(vec!["/bin/sh".into()]));

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
