//! A board project on disk and this machine's list of them: the project's status, gates,
//! staleness, files, links to other repositories, examples and the static snapshot. No HTTP
//! here, so commands can use it without a server (docs/one-system-spec.md).

pub mod config;
pub mod examples;
pub mod files;
pub mod gates;
pub mod links;
pub mod problem;
pub mod project;
pub mod snapshot;

use std::path::PathBuf;

/// The repository this binary was built from: where `proj port` resolves to
/// Boardroom's registered port, and where the web build lives.
pub fn repo_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::canonicalize(&dir).unwrap_or(dir)
}

/// Resolves Boardroom's port: an explicit `--port` value, then `$PORT`, then
/// `proj port` in [`repo_dir`]. Fails rather than guessing when none is set.
pub fn port(flag: Option<&str>) -> Result<u16, String> {
    let env_port = std::env::var("PORT").ok();
    port_with(flag, env_port.as_deref(), proj_port)
}

fn proj_port() -> Option<String> {
    let out = std::process::Command::new("proj")
        .arg("port")
        .current_dir(repo_dir())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!s.is_empty()).then_some(s)
}

fn port_with(
    flag: Option<&str>,
    env_port: Option<&str>,
    from_proj: impl FnOnce() -> Option<String>,
) -> Result<u16, String> {
    let text = flag
        .map(str::to_owned)
        .or_else(|| env_port.filter(|p| !p.is_empty()).map(str::to_owned))
        .or_else(|| from_proj().filter(|p| !p.is_empty()))
        .ok_or_else(|| {
            "no port. Pass --port N or set PORT (or register the project with `proj add`)."
                .to_string()
        })?;
    text.parse()
        .map_err(|_| format!("port '{text}' is not a number"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flag_wins_over_env_and_proj() {
        assert_eq!(port(Some("8123")), Ok(8123));
        assert_eq!(
            port_with(Some("8123"), Some("9000"), || Some("9001".into())),
            Ok(8123)
        );
    }

    #[test]
    fn env_port_wins_over_proj_and_empty_env_falls_through() {
        assert_eq!(
            port_with(None, Some("9000"), || Some("9001".into())),
            Ok(9000)
        );
        assert_eq!(
            port_with(None, Some(""), || Some("9001".into())),
            Ok(9001)
        );
    }

    #[test]
    fn errors_when_no_port_or_not_a_number() {
        let err = port_with(None, None, || None).unwrap_err();
        assert!(err.contains("no port"), "{err}");
        let err_empty = port_with(None, Some(""), || None).unwrap_err();
        assert!(err_empty.contains("no port"), "{err_empty}");
        let err_bad = port_with(Some("abc"), None, || None).unwrap_err();
        assert_eq!(err_bad, "port 'abc' is not a number");
    }
}
