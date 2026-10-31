//! Composer's placement problem on this machine: `compose/<board>/problem.json`, made on read
//! (docs/demo-spec.md, milestone 6). On Pages the snapshot carries it; here, when a project has
//! no file of its own, Boardroom's server makes it from the board's current `.kicad_pcb` with
//! ccomp, the same way the snapshot does, and serves it without writing it into the project.
//! It takes ccomp a few tens of milliseconds, so it is made fresh on every read and always
//! matches the board.

use std::path::{Path, PathBuf};

/// The board a `compose/<board>/problem.json` path names.
pub fn board_of(rel: &str) -> Option<&str> {
    let board = rel.strip_prefix("compose/")?.strip_suffix("/problem.json")?;
    (!board.is_empty() && !board.contains('/') && board != "..").then_some(board)
}

/// Where ccomp is: `$CCOMP`, else `ccomp = "…"` in `~/.config/boardroom/config.toml`, else
/// `ccomp` on the PATH.
pub fn ccomp() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("CCOMP") {
        return Some(PathBuf::from(p));
    }
    let configured = std::fs::read_to_string(crate::config::config_dir().join("config.toml"))
        .ok()
        .and_then(|t| t.parse::<toml::Table>().ok())
        .and_then(|t| t.get("ccomp").and_then(|v| v.as_str()).map(PathBuf::from));
    configured.or_else(|| {
        std::env::var_os("PATH").and_then(|paths| std::env::split_paths(&paths).map(|d| d.join("ccomp")).find(|p| p.is_file()))
    })
}

/// The board's problem, made by `ccomp` from its last variant's `.kicad_pcb`, as the snapshot
/// picks it.
pub fn make(root: &Path, board: &str, ccomp: &Path) -> Result<Vec<u8>, String> {
    let mut variants: Vec<String> = std::fs::read_dir(root.join("hw").join(board))
        .map_err(|_| format!("the project has no hw/{board}/"))?
        .flatten()
        .filter(|e| e.path().join(format!("{board}.kicad_pcb")).is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    variants.sort();
    let variant = variants.pop().ok_or(format!("no hw/{board}/<variant>/{board}.kicad_pcb"))?;
    let pcb = root.join(format!("hw/{board}/{variant}/{board}.kicad_pcb"));
    let out = std::env::temp_dir().join(format!("boardroom-problem-{}-{board}-{:?}.json", std::process::id(), std::thread::current().id()));
    let made = crate::snapshot::make_problem(ccomp, &pcb, &out).and_then(|_| std::fs::read(&out).map_err(|e| e.to_string()));
    let _ = std::fs::remove_file(&out);
    made
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_boards_problem_path_names_a_board() {
        assert_eq!(board_of("compose/easy2350/problem.json"), Some("easy2350"));
        for rel in ["compose/problem.json", "compose/a/b/problem.json", "compose/../problem.json", "compose/a/placement.json", "hw/a/problem.json"] {
            assert_eq!(board_of(rel), None, "{rel}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn the_problem_is_made_from_the_last_variant_and_not_left_in_the_project() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("boardroom-problem-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let put = |rel: &str, body: &str| {
            let p = dir.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, body).unwrap();
        };
        // A stand-in for ccomp: the problem names the board file it was made from.
        put("ccomp", "#!/bin/sh\ngrep -q unreadable \"$2\" && { echo 'error: not a board' >&2; exit 1; }\nfor a; do [ \"$prev\" = -o ] && printf '{\"from\":\"%s\"}' \"$2\" > \"$a\"; prev=$a; done\n");
        std::fs::set_permissions(dir.join("ccomp"), std::fs::Permissions::from_mode(0o755)).unwrap();
        put("p/hw/b/a/b.kicad_pcb", "(kicad_pcb old)");
        put("p/hw/b/b/b.kicad_pcb", "(kicad_pcb)");
        put("p/hw/bad/a/bad.kicad_pcb", "unreadable");
        let (p, cc) = (dir.join("p"), dir.join("ccomp"));
        let made = String::from_utf8(make(&p, "b", &cc).unwrap()).unwrap();
        assert!(made.ends_with("hw/b/b/b.kicad_pcb\"}"), "{made}");
        assert!(!p.join("compose").exists(), "nothing written into the project");
        assert_eq!(make(&p, "bad", &cc), Err("ccomp failed: error: not a board".into()));
        assert_eq!(make(&p, "none", &cc), Err("the project has no hw/none/".into()));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
