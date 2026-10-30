//! Writing project files for the apps, and their history: the server side of the suite's store
//! (`app/store/store.js`, docs/demo-spec.md).
//!
//! A file's revision is a hash of its bytes, sent as its ETag. A write names the revision it
//! started from, and is refused if the file has moved on since, so two apps (or two windows)
//! never silently overwrite each other. History is git's.

use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;

/// What a write requires of the file it replaces.
#[derive(Debug, Clone, PartialEq)]
pub enum Expect {
    /// Anything: no check.
    Any,
    /// No file there yet (`If-None-Match: *`).
    Absent,
    /// This revision (`If-Match`).
    Rev(String),
}

#[derive(Debug, PartialEq)]
pub enum WriteError {
    /// The file is at `current` (none: not there), not what the write started from.
    Conflict { current: Option<String> },
    /// A path that is not a file inside the project.
    Refused(String),
    Io(String),
}

/// One check-and-write at a time, so two writes from the same revision cannot both pass.
static WRITING: Mutex<()> = Mutex::new(());

/// A file's revision: FNV-1a over its bytes, 16 hex digits.
pub fn rev_of(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h = (h ^ *b as u64).wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

/// `rel` under `root` for writing: a plain relative path, not into `.git`, and still inside the
/// project once its existing folders' links are followed.
pub fn target(root: &Path, rel: &str) -> Result<PathBuf, WriteError> {
    let refused = |why: &str| WriteError::Refused(format!("{rel}: {why}"));
    let path = Path::new(rel);
    if rel.is_empty() || path.components().any(|c| !matches!(c, Component::Normal(_))) {
        return Err(refused("not a plain path inside the project"));
    }
    if path.components().next().is_some_and(|c| c.as_os_str() == ".git") {
        return Err(refused("git's own files are not written here"));
    }
    let root = std::fs::canonicalize(root).map_err(|e| WriteError::Io(e.to_string()))?;
    // The deepest folder that exists must resolve inside the project.
    let mut probe = root.join(path);
    probe.pop();
    while !probe.exists() {
        probe.pop();
    }
    let real = std::fs::canonicalize(&probe).map_err(|e| WriteError::Io(e.to_string()))?;
    if !real.starts_with(&root) {
        return Err(refused("outside the project"));
    }
    let full = root.join(path);
    if full.is_dir() {
        return Err(refused("a folder"));
    }
    Ok(full)
}

/// Write `bytes` to `rel` if the file is as `expect` says; the new revision.
pub fn write(root: &Path, rel: &str, bytes: &[u8], expect: &Expect) -> Result<String, WriteError> {
    let path = target(root, rel)?;
    let _one = WRITING.lock().unwrap_or_else(|e| e.into_inner());
    let current = std::fs::read(&path).ok().map(|b| rev_of(&b));
    let ok = match expect {
        Expect::Any => true,
        Expect::Absent => current.is_none(),
        Expect::Rev(r) => current.as_deref() == Some(r.as_str()),
    };
    if !ok {
        return Err(WriteError::Conflict { current });
    }
    let dir = path.parent().expect("a file has a folder");
    std::fs::create_dir_all(dir).map_err(|e| WriteError::Io(format!("{}: {e}", dir.display())))?;
    // Written beside it and renamed over it, so a reader never sees half a file.
    let tmp = dir.join(format!(".{}.boardroom-{}", path.file_name().unwrap().to_string_lossy(), std::process::id()));
    std::fs::write(&tmp, bytes).map_err(|e| WriteError::Io(format!("{}: {e}", tmp.display())))?;
    std::fs::rename(&tmp, &path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        WriteError::Io(format!("{}: {e}", path.display()))
    })?;
    Ok(rev_of(bytes))
}

/// The project's history, or one file's, newest first: what is not committed yet, then git's
/// last hundred commits. `[{rev, path, paths, app, time, message}]`; `app` is the commit's
/// author. A folder that is not in git has only the first.
pub fn history(root: &Path, rel: Option<&str>) -> Vec<serde_json::Value> {
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
    };
    let scope = rel.unwrap_or(".");
    let mut out = vec![];
    if let Some(status) = git(&["status", "--porcelain", "--untracked-files=all", "--", scope]) {
        let paths: Vec<String> = status.lines().filter(|l| l.len() > 3).map(|l| l[3..].trim_matches('"').to_string()).collect();
        if !paths.is_empty() {
            out.push(serde_json::json!({
                "rev": "uncommitted",
                "path": paths[0],
                "paths": paths,
                "app": null,
                "time": null,
                "message": format!("Not committed: {} file{}", paths.len(), if paths.len() == 1 { "" } else { "s" }),
            }));
        }
    }
    let log = git(&["log", "-n", "100", "--name-only", "--format=%x1e%H%x1f%an%x1f%aI%x1f%s", "--", scope]).unwrap_or_default();
    for record in log.split('\u{1e}').filter(|r| !r.trim().is_empty()) {
        let mut lines = record.lines();
        let head: Vec<&str> = lines.next().unwrap_or("").split('\u{1f}').collect();
        let [rev, author, time, subject] = head[..] else { continue };
        let paths: Vec<&str> = lines.filter(|l| !l.is_empty()).collect();
        out.push(serde_json::json!({
            "rev": rev,
            "path": rel.or(paths.first().copied()),
            "paths": paths,
            "app": author,
            "time": time,
            "message": subject,
        }));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("boardroom-files-{}-{:?}", std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("hw")).unwrap();
        dir
    }

    #[test]
    fn a_write_needs_the_revision_it_started_from() {
        let root = project();
        let r1 = write(&root, "compose/b/placement.json", b"one", &Expect::Absent).unwrap();
        assert_eq!(std::fs::read(root.join("compose/b/placement.json")).unwrap(), b"one");
        assert_eq!(write(&root, "compose/b/placement.json", b"again", &Expect::Absent), Err(WriteError::Conflict { current: Some(r1.clone()) }));
        let r2 = write(&root, "compose/b/placement.json", b"two", &Expect::Rev(r1.clone())).unwrap();
        // A second write from the first revision is someone else's stale copy.
        assert_eq!(write(&root, "compose/b/placement.json", b"three", &Expect::Rev(r1)), Err(WriteError::Conflict { current: Some(r2.clone()) }));
        assert_eq!(rev_of(&std::fs::read(root.join("compose/b/placement.json")).unwrap()), r2);
        write(&root, "compose/b/placement.json", b"four", &Expect::Any).unwrap();
        let left: Vec<_> = std::fs::read_dir(root.join("compose/b")).unwrap().flatten().map(|e| e.file_name()).collect();
        assert_eq!(left.len(), 1, "no temporary file left: {left:?}");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn only_plain_paths_inside_the_project_are_written() {
        let root = project();
        for rel in ["../out.txt", "/etc/passwd", "hw/../../x", ".git/config", "", "hw"] {
            assert!(matches!(write(&root, rel, b"x", &Expect::Any), Err(WriteError::Refused(_))), "{rel}");
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(std::env::temp_dir(), root.join("link")).unwrap();
            assert!(matches!(write(&root, "link/escape.txt", b"x", &Expect::Any), Err(WriteError::Refused(_))));
        }
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn history_is_gits_with_what_is_not_committed_first() {
        let root = project();
        let git = |args: &[&str]| assert!(std::process::Command::new("git").arg("-C").arg(&root).args(args).output().unwrap().status.success(), "git {args:?}");
        git(&["init", "-q"]);
        std::fs::write(root.join("hw/a.txt"), "a").unwrap();
        git(&["add", "."]);
        git(&["-c", "user.name=Composer", "-c", "user.email=c@example.com", "commit", "-q", "-m", "Placement"]);
        assert_eq!(history(&root, None).len(), 1);
        write(&root, "hw/a.txt", b"b", &Expect::Any).unwrap();
        let h = history(&root, Some("hw/a.txt"));
        assert_eq!(h[0]["rev"], "uncommitted");
        assert_eq!(h[1]["app"], "Composer");
        assert_eq!(h[1]["message"], "Placement");
        assert_eq!(h[1]["paths"][0], "hw/a.txt");
        std::fs::remove_dir_all(&root).unwrap();
    }
}
