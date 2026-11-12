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
