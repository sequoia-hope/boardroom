//! Project links (`design/links.toml`): resolving another repository's board-shape interface
//! (`board-shape.dxf` + `board-shape.json`), classifying link state with `board_interface`, and
//! materializing the accepted version at `<project>/.boardroom/links/<id>/board-shape.{dxf,json}`
//! for Circuit Composer (`docs/suite-spec.md`, "Linked projects").

use crate::config;
use crate::project::Need;
use board_interface::{Change, Classification, Driver, Record, RematchedId, UncertainMatch};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const GIT_TIMEOUT: Duration = Duration::from_secs(15);
const FETCH_TTL: Duration = Duration::from_secs(60);
const LOCK_STALE_AFTER: Duration = Duration::from_secs(30);
const LOCK_WAIT_LIMIT: Duration = Duration::from_secs(10);

static TMP_SEQ: AtomicU64 = AtomicU64::new(1);

/// One `[[links]]` entry in `design/links.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkRecord {
    pub id: String,
    pub kind: String,
    pub board: String,
    pub locator: Locator,
    pub resolved: Resolved,
    pub interface_hash: String,
    #[serde(default)]
    pub pin_reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Locator {
    Git {
        remote: String,
        path: String,
        #[serde(rename = "ref")]
        git_ref: GitRef,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum GitRef {
    Branch { name: String },
    Tag { name: String },
    Commit { commit: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resolved {
    pub commit: String,
    pub at: String,
}

#[derive(Debug, Default, Deserialize)]
struct LinksFile {
    #[serde(default)]
    links: Vec<LinkRecord>,
}

/// Link state reported to the UI and CLI (`docs/suite-spec.md`, "Link states").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkState {
    Current,
    NewRevisionNoInterfaceChange,
    UpdateAvailable,
    PinnedBehind,
    ChangedOutsideSuite,
    CompetingTakeOvers,
    StaleTakeOver,
    Unavailable,
    Tampered,
}

impl LinkState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Current => "Current",
            Self::NewRevisionNoInterfaceChange => "NewRevisionNoInterfaceChange",
            Self::UpdateAvailable => "UpdateAvailable",
            Self::PinnedBehind => "PinnedBehind",
            Self::ChangedOutsideSuite => "ChangedOutsideSuite",
            Self::CompetingTakeOvers => "CompetingTakeOvers",
            Self::StaleTakeOver => "StaleTakeOver",
            Self::Unavailable => "Unavailable",
            Self::Tampered => "Tampered",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Latest {
    pub commit: String,
    pub at: String,
    pub revision: String,
    pub driver: Driver,
}

/// Evaluated status of a link returned by `GET /api/projects/{id}/links` and `boardroom link list`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkStatus {
    pub id: String,
    pub kind: String,
    pub board: String,
    pub locator: Locator,
    pub resolved: Resolved,
    pub pin_reason: String,
    pub state: LinkState,
    pub locked_by: Option<Driver>,
    pub classification: Option<Classification>,
    pub changes: Vec<Change>,
    pub thickness_changed: Option<(i64, i64)>,
    pub rematched: Vec<RematchedId>,
    pub uncertain: Vec<UncertainMatch>,
    pub latest: Option<Latest>,
    pub following_path: String,
    pub error: Option<String>,
}

/// Payload for adding a new link (`POST /api/projects/{id}/links` or `boardroom link add`).
#[derive(Debug, Clone, Deserialize)]
pub struct AddLinkRequest {
    pub id: String,
    #[serde(default = "default_kind")]
    pub kind: String,
    pub board: String,
    #[serde(default)]
    pub remote: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(rename = "ref", default)]
    pub git_ref: Option<GitRef>,
    #[serde(default)]
    pub locator: Option<Locator>,
    #[serde(default)]
    pub pin_reason: String,
}

fn default_kind() -> String {
    "waffle".into()
}

#[derive(Debug)]
pub enum LinkOpError {
    NotFound(String),
    BadRequest(String),
    Io(String),
}

impl std::fmt::Display for LinkOpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(m) | Self::BadRequest(m) | Self::Io(m) => f.write_str(m),
        }
    }
}

struct RemoteSlot {
    gate: tokio::sync::Mutex<Option<Instant>>,
}

/// Bare git mirror cache under `$XDG_CACHE_HOME/boardroom/links/<sha256 of remote>/`.
pub struct LinksCache {
    root: PathBuf,
    remotes: tokio::sync::Mutex<HashMap<String, Arc<RemoteSlot>>>,
}

impl Default for LinksCache {
    fn default() -> Self {
        Self::new()
    }
}

impl LinksCache {
    pub fn new() -> Self {
        Self::with_dir(default_cache_dir())
    }

    pub fn with_dir(root: PathBuf) -> Self {
        Self {
            root,
            remotes: tokio::sync::Mutex::new(HashMap::new()),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.root
    }

    async fn slot_for(&self, key: &str) -> Arc<RemoteSlot> {
        let mut map = self.remotes.lock().await;
        map.entry(key.to_string())
            .or_insert_with(|| {
                Arc::new(RemoteSlot {
                    gate: tokio::sync::Mutex::new(None),
                })
            })
            .clone()
    }

    /// Ensures the bare mirror for `remote` exists and is fetched (at most once per 60 s unless
    /// `force` is true).
    pub async fn ensure_mirror(
        &self,
        project_root: &Path,
        remote: &str,
        force: bool,
    ) -> Result<PathBuf, String> {
        let effective = resolve_remote_target(project_root, remote)?;
        let key = sha256_hex(&effective);
        let mirror_dir = self.root.join(&key);
        let slot = self.slot_for(&key).await;
        let mut last_fetched = slot.gate.lock().await;

        let marker = mirror_dir.join(".boardroom-fetched");
        let initialized = mirror_dir.join("HEAD").is_file() && mirror_dir.join("objects").is_dir();

        if !initialized {
            std::fs::create_dir_all(&self.root)
                .map_err(|e| format!("{}: {e}", self.root.display()))?;
            let seq = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
            let tmp_dir = self
                .root
                .join(format!(".clone-{key}-{}-{seq}", std::process::id()));
            if tmp_dir.exists() {
                let _ = std::fs::remove_dir_all(&tmp_dir);
            }
            let clone_res = run_git(&[
                OsStr::new("clone"),
                OsStr::new("--mirror"),
                OsStr::new("--"),
                OsStr::new(&effective),
                tmp_dir.as_os_str(),
            ])
            .await;
            if let Err(e) = clone_res {
                let _ = std::fs::remove_dir_all(&tmp_dir);
                return Err(e);
            }
            if mirror_dir.exists() {
                let _ = std::fs::remove_dir_all(&mirror_dir);
            }
            std::fs::rename(&tmp_dir, &mirror_dir).map_err(|e| {
                let _ = std::fs::remove_dir_all(&tmp_dir);
                format!("{}: {e}", mirror_dir.display())
            })?;
            let _ = std::fs::write(&marker, now_utc_iso().as_bytes());
            *last_fetched = Some(Instant::now());
            return Ok(mirror_dir);
        }

        let fresh_mem = last_fetched.is_some_and(|t| t.elapsed() < FETCH_TTL);
        let fresh_disk = !fresh_mem
            && std::fs::metadata(&marker)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|m| m.elapsed().ok())
                .is_some_and(|age| age < FETCH_TTL);

        if force || (!fresh_mem && !fresh_disk) {
            run_git(&[
                OsStr::new("-C"),
                mirror_dir.as_os_str(),
                OsStr::new("fetch"),
                OsStr::new("--prune"),
                OsStr::new("origin"),
            ])
            .await?;
            let _ = std::fs::write(&marker, now_utc_iso().as_bytes());
            *last_fetched = Some(Instant::now());
        }

        Ok(mirror_dir)
    }
}

pub fn default_cache_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| config::home().join(".cache"));
    base.join("boardroom/links")
}

pub fn sha256_hex(input: &str) -> String {
    let mut h = Sha256::new();
    h.update(input.as_bytes());
    let digest = h.finalize();
    let mut out = String::with_capacity(64);
    for b in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut out, "{b:02x}");
    }
    out
}

pub fn remote_cache_key(project_root: &Path, remote: &str) -> Result<String, String> {
    Ok(sha256_hex(&resolve_remote_target(project_root, remote)?))
}

fn resolve_remote_target(project_root: &Path, remote: &str) -> Result<String, String> {
    let trimmed = remote.trim();
    if trimmed.is_empty() {
        return Err("remote must not be empty".into());
    }
    if trimmed.starts_with('-') {
        return Err("remote must not start with '-'".into());
    }
    if trimmed.contains("://") || trimmed.starts_with("git@") || trimmed.starts_with('/') {
        return Ok(trimmed.to_string());
    }
    if let Some(rest) = trimmed.strip_prefix("~/") {
        return Ok(config::home().join(rest).to_string_lossy().into_owned());
    }
    // Relative filesystem path (stage 1 local repos) if not a host:pathscp form.
    if !trimmed.contains(':') {
        let joined = project_root.join(trimmed);
        return Ok(std::fs::canonicalize(&joined)
            .unwrap_or(joined)
            .to_string_lossy()
            .into_owned());
    }
    Ok(trimmed.to_string())
}

fn validate_link_id(id: &str) -> Result<(), String> {
    if id.is_empty() || id.contains('/') || id.contains('\\') || id.contains('\0') {
        return Err(format!("invalid link id '{id}'"));
    }
    let mut comps = Path::new(id).components();
    match (comps.next(), comps.next()) {
        (Some(Component::Normal(_)), None) => Ok(()),
        _ => Err(format!("invalid link id '{id}'")),
    }
}

/// Validates `locator.path` (which names `board-shape.json`) and returns `(dxf_path, json_path)`
/// as repo-relative forward-slash paths.
fn interface_git_paths(raw: &str) -> Result<(String, String), String> {
    let trimmed = raw.trim().trim_start_matches("./");
    if trimmed.is_empty()
        || trimmed.starts_with('/')
        || trimmed.contains('\\')
        || trimmed.contains('\0')
    {
        return Err(format!("invalid interface path '{raw}'"));
    }
    for part in trimmed.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            return Err(format!("invalid interface path '{raw}'"));
        }
    }
    let stem = trimmed.strip_suffix(".json").ok_or_else(|| {
        format!(
            "interface path '{raw}' must end with .json (e.g. interface/<board>/board-shape.json)"
        )
    })?;
    if stem.is_empty() || stem.ends_with('/') {
        return Err(format!("invalid interface path '{raw}'"));
    }
    Ok((format!("{stem}.dxf"), trimmed.to_string()))
}

fn is_hex_oid(s: &str) -> bool {
    (4..=64).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn short_sha(sha: &str) -> &str {
    &sha[..sha.len().min(7)]
}

async fn run_git(args: &[&OsStr]) -> Result<Vec<u8>, String> {
    let mut cmd = tokio::process::Command::new("git");
    cmd.args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let out = match tokio::time::timeout(GIT_TIMEOUT, cmd.output()).await {
        Ok(Ok(o)) => o,
        Ok(Err(e)) => return Err(format!("git: {e}")),
        Err(_) => return Err("git timed out after 15 s".into()),
    };

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let msg = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            format!("git exited with {}", out.status)
        };
        return Err(msg);
    }
    Ok(out.stdout)
}

async fn git_rev_parse(mirror: &Path, spec: &str) -> Result<String, String> {
    let out = run_git(&[
        OsStr::new("-C"),
        mirror.as_os_str(),
        OsStr::new("rev-parse"),
        OsStr::new("--verify"),
        OsStr::new("--quiet"),
        OsStr::new(spec),
    ])
    .await?;
    let oid = String::from_utf8_lossy(&out).trim().to_string();
    if !is_hex_oid(&oid) {
        return Err(format!("could not resolve '{spec}'"));
    }
    Ok(oid)
}

async fn resolve_git_ref(mirror: &Path, git_ref: &GitRef) -> Result<String, String> {
    match git_ref {
        GitRef::Branch { name } => {
            if name.trim().is_empty() || name.starts_with('-') {
                return Err(format!("invalid branch name '{name}'"));
            }
            git_rev_parse(mirror, &format!("refs/heads/{name}^{{commit}}"))
                .await
                .map_err(|_| format!("branch '{name}' not found on remote"))
        }
        GitRef::Tag { name } => {
            if name.trim().is_empty() || name.starts_with('-') {
                return Err(format!("invalid tag name '{name}'"));
            }
            git_rev_parse(mirror, &format!("refs/tags/{name}^{{commit}}"))
                .await
                .map_err(|_| format!("tag '{name}' not found on remote"))
        }
        GitRef::Commit { commit } => {
            let c = commit.trim();
            if !is_hex_oid(c) {
                return Err(format!("invalid commit '{commit}'"));
            }
            git_rev_parse(mirror, &format!("{c}^{{commit}}"))
                .await
                .map_err(|_| format!("commit '{commit}' not found on remote"))
        }
    }
}

async fn resolve_default_head(mirror: &Path, fallback_commit: &str) -> Result<String, String> {
    for candidate in [
        "HEAD^{commit}",
        "refs/heads/main^{commit}",
        "refs/heads/master^{commit}",
    ] {
        if let Ok(oid) = git_rev_parse(mirror, candidate).await {
            return Ok(oid);
        }
    }
    let out = run_git(&[
        OsStr::new("-C"),
        mirror.as_os_str(),
        OsStr::new("for-each-ref"),
        OsStr::new("--count=1"),
        OsStr::new("--format=%(objectname)"),
        OsStr::new("refs/heads"),
    ])
    .await?;
    let first = String::from_utf8_lossy(&out).trim().to_string();
    if is_hex_oid(&first) {
        return git_rev_parse(mirror, &format!("{first}^{{commit}}")).await;
    }
    Ok(fallback_commit.to_string())
}

async fn git_cat_blob(mirror: &Path, commit: &str, rel_path: &str) -> Result<Vec<u8>, String> {
    if !is_hex_oid(commit) {
        return Err(format!("invalid commit '{commit}'"));
    }
    let spec = format!("{commit}:{rel_path}");
    run_git(&[
        OsStr::new("-C"),
        mirror.as_os_str(),
        OsStr::new("cat-file"),
        OsStr::new("blob"),
        OsStr::new(&spec),
    ])
    .await
    .map_err(|e| format!("{rel_path} at {}: {e}", short_sha(commit)))
}

async fn git_commit_time(mirror: &Path, commit: &str) -> Result<String, String> {
    if !is_hex_oid(commit) {
        return Err(format!("invalid commit '{commit}'"));
    }
    let out = run_git(&[
        OsStr::new("-C"),
        mirror.as_os_str(),
        OsStr::new("show"),
        OsStr::new("-s"),
        OsStr::new("--format=%cI"),
        OsStr::new(commit),
    ])
    .await?;
    Ok(String::from_utf8_lossy(&out).trim().to_string())
}

pub fn following_path_for(project_root: &Path, link_id: &str) -> PathBuf {
    let base = std::fs::canonicalize(project_root).unwrap_or_else(|_| project_root.to_path_buf());
    base.join(".boardroom/links")
        .join(link_id)
        .join("board-shape.json")
}

fn materialized_dir(project_root: &Path, link_id: &str) -> PathBuf {
    project_root.join(".boardroom/links").join(link_id)
}

fn materialized_exists(project_root: &Path, link_id: &str) -> bool {
    let dir = materialized_dir(project_root, link_id);
    dir.join("board-shape.dxf").is_file() && dir.join("board-shape.json").is_file()
}

fn materialize_bytes(
    project_root: &Path,
    link_id: &str,
    dxf_bytes: &[u8],
    json_bytes: &[u8],
) -> Result<(), String> {
    validate_link_id(link_id)?;
    let dir = materialized_dir(project_root, link_id);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    atomic_write(&dir.join("board-shape.dxf"), dxf_bytes)?;
    atomic_write(&dir.join("board-shape.json"), json_bytes)?;
    Ok(())
}

fn cleanup_materialized(project_root: &Path, link_id: &str) {
    if validate_link_id(link_id).is_err() {
        return;
    }
    let dir = materialized_dir(project_root, link_id);
    let _ = std::fs::remove_file(dir.join("board-shape.dxf"));
    let _ = std::fs::remove_file(dir.join("board-shape.json"));
    let _ = std::fs::remove_dir(&dir);
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let dir = path
        .parent()
        .ok_or_else(|| format!("{}: no parent dir", path.display()))?;
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let seq = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let tmp = dir.join(format!(".{name}.boardroom-{}-{seq}", std::process::id()));
    std::fs::write(&tmp, bytes).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("{}: {e}", path.display())
    })
}

/// Exclusive lock on `design/links.toml.lock` using `O_EXCL` (`create_new(true)`) with stale lock
/// eviction.
struct LinksLock {
    path: PathBuf,
}

impl LinksLock {
    fn acquire(project_root: &Path) -> Result<Self, String> {
        let design = project_root.join("design");
        std::fs::create_dir_all(&design).map_err(|e| format!("{}: {e}", design.display()))?;
        let lock_path = design.join("links.toml.lock");
        let start = Instant::now();
        loop {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&lock_path)
            {
                Ok(mut f) => {
                    let _ = writeln!(f, "pid={} at={}", std::process::id(), now_utc_iso());
                    return Ok(Self { path: lock_path });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    let is_stale = std::fs::metadata(&lock_path)
                        .and_then(|m| m.modified())
                        .ok()
                        .and_then(|t| t.elapsed().ok())
                        .is_some_and(|age| age >= LOCK_STALE_AFTER);
                    if is_stale {
                        let _ = std::fs::remove_file(&lock_path);
                        continue;
                    }
                    if start.elapsed() >= LOCK_WAIT_LIMIT {
                        return Err(format!(
                            "timed out waiting for lock {}",
                            lock_path.display()
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(15));
                }
                Err(e) => return Err(format!("{}: {e}", lock_path.display())),
            }
        }
    }
}

impl Drop for LinksLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Reads `design/links.toml` (returns an empty Vec when the file does not exist).
pub fn read_links_file(project_root: &Path) -> Result<Vec<LinkRecord>, String> {
    let path = project_root.join("design/links.toml");
    match std::fs::read_to_string(&path) {
        Ok(text) => {
            let parsed: LinksFile =
                toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
            Ok(parsed.links)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(vec![]),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// Locked read-modify-write of `design/links.toml` preserving comments and unknown keys via
/// `toml_edit`.
fn edit_links_toml<R>(
    project_root: &Path,
    mutate: impl FnOnce(&mut toml_edit::DocumentMut, Vec<LinkRecord>) -> Result<R, LinkOpError>,
) -> Result<R, LinkOpError> {
    let _lock = LinksLock::acquire(project_root).map_err(LinkOpError::Io)?;
    let path = project_root.join("design/links.toml");
    let raw = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(LinkOpError::Io(format!("{}: {e}", path.display()))),
    };
    let current: Vec<LinkRecord> = if raw.trim().is_empty() {
        vec![]
    } else {
        toml::from_str::<LinksFile>(&raw)
            .map_err(|e| LinkOpError::Io(format!("{}: {e}", path.display())))?
            .links
    };
    let mut doc: toml_edit::DocumentMut = raw
        .parse()
        .map_err(|e| LinkOpError::Io(format!("{}: {e}", path.display())))?;

    let out = mutate(&mut doc, current)?;
    let rendered = doc.to_string();
    atomic_write(&path, rendered.as_bytes()).map_err(LinkOpError::Io)?;
    Ok(out)
}

fn set_table_str(table: &mut toml_edit::Table, key: &str, val: &str) {
    if let Some(toml_edit::Item::Value(existing)) = table.get_mut(key) {
        let decor = existing.decor().clone();
        let mut next = toml_edit::Value::from(val);
        *next.decor_mut() = decor;
        *existing = next;
    } else {
        table.insert(key, toml_edit::value(val));
    }
}

fn set_inline_str(inline: &mut toml_edit::InlineTable, key: &str, val: &str) {
    if let Some(existing) = inline.get_mut(key) {
        let decor = existing.decor().clone();
        let mut next = toml_edit::Value::from(val);
        *next.decor_mut() = decor;
        *existing = next;
    } else {
        inline.insert(key, toml_edit::Value::from(val));
    }
}

fn make_git_ref_inline(git_ref: &GitRef) -> toml_edit::InlineTable {
    let mut t = toml_edit::InlineTable::new();
    match git_ref {
        GitRef::Branch { name } => {
            t.insert("type", toml_edit::Value::from("Branch"));
            t.insert("name", toml_edit::Value::from(name.as_str()));
        }
        GitRef::Tag { name } => {
            t.insert("type", toml_edit::Value::from("Tag"));
            t.insert("name", toml_edit::Value::from(name.as_str()));
        }
        GitRef::Commit { commit } => {
            t.insert("type", toml_edit::Value::from("Commit"));
            t.insert("commit", toml_edit::Value::from(commit.as_str()));
        }
    }
    t.fmt();
    t
}

fn update_git_ref_inline(inline: &mut toml_edit::InlineTable, git_ref: &GitRef) {
    // Preserve trailing space decor if switching between `name` and `commit`.
    let prev_tail_suffix = inline
        .get("name")
        .or_else(|| inline.get("commit"))
        .and_then(|v| v.decor().suffix()?.as_str())
        .map(str::to_string);

    match git_ref {
        GitRef::Branch { name } => {
            set_inline_str(inline, "type", "Branch");
            inline.remove("commit");
            let had_name = inline.contains_key("name");
            set_inline_str(inline, "name", name);
            if !had_name && let Some(v) = inline.get_mut("name") {
                v.decor_mut()
                    .set_suffix(prev_tail_suffix.as_deref().unwrap_or(" "));
            }
        }
        GitRef::Tag { name } => {
            set_inline_str(inline, "type", "Tag");
            inline.remove("commit");
            let had_name = inline.contains_key("name");
            set_inline_str(inline, "name", name);
            if !had_name && let Some(v) = inline.get_mut("name") {
                v.decor_mut()
                    .set_suffix(prev_tail_suffix.as_deref().unwrap_or(" "));
            }
        }
        GitRef::Commit { commit } => {
            set_inline_str(inline, "type", "Commit");
            inline.remove("name");
            let had_commit = inline.contains_key("commit");
            set_inline_str(inline, "commit", commit);
            if !had_commit && let Some(v) = inline.get_mut("commit") {
                v.decor_mut()
                    .set_suffix(prev_tail_suffix.as_deref().unwrap_or(" "));
            }
        }
    }
}

fn update_git_ref_table(table: &mut toml_edit::Table, git_ref: &GitRef) {
    match git_ref {
        GitRef::Branch { name } => {
            set_table_str(table, "type", "Branch");
            table.remove("commit");
            set_table_str(table, "name", name);
        }
        GitRef::Tag { name } => {
            set_table_str(table, "type", "Tag");
            table.remove("commit");
            set_table_str(table, "name", name);
        }
        GitRef::Commit { commit } => {
            set_table_str(table, "type", "Commit");
            table.remove("name");
            set_table_str(table, "commit", commit);
        }
    }
}

fn make_locator_inline(locator: &Locator) -> toml_edit::InlineTable {
    let mut loc = toml_edit::InlineTable::new();
    match locator {
        Locator::Git {
            remote,
            path,
            git_ref,
        } => {
            loc.insert("type", toml_edit::Value::from("Git"));
            loc.insert("remote", toml_edit::Value::from(remote.as_str()));
            loc.insert("path", toml_edit::Value::from(path.as_str()));
            loc.insert(
                "ref",
                toml_edit::Value::InlineTable(make_git_ref_inline(git_ref)),
            );
        }
    }
    loc.fmt();
    loc
}

fn make_resolved_inline(resolved: &Resolved) -> toml_edit::InlineTable {
    let mut res = toml_edit::InlineTable::new();
    res.insert("commit", toml_edit::Value::from(resolved.commit.as_str()));
    res.insert("at", toml_edit::Value::from(resolved.at.as_str()));
    res.fmt();
    res
}

fn apply_record_to_table(table: &mut toml_edit::Table, rec: &LinkRecord) {
    set_table_str(table, "id", &rec.id);
    set_table_str(table, "kind", &rec.kind);
    set_table_str(table, "board", &rec.board);

    let Locator::Git {
        remote,
        path,
        git_ref,
    } = &rec.locator;

    match table.get_mut("locator") {
        Some(toml_edit::Item::Value(toml_edit::Value::InlineTable(loc))) => {
            set_inline_str(loc, "type", "Git");
            set_inline_str(loc, "remote", remote);
            set_inline_str(loc, "path", path);
            match loc.get_mut("ref") {
                Some(toml_edit::Value::InlineTable(r)) => update_git_ref_inline(r, git_ref),
                _ => {
                    loc.insert(
                        "ref",
                        toml_edit::Value::InlineTable(make_git_ref_inline(git_ref)),
                    );
                }
            }
        }
        Some(toml_edit::Item::Table(loc)) => {
            set_table_str(loc, "type", "Git");
            set_table_str(loc, "remote", remote);
            set_table_str(loc, "path", path);
            match loc.get_mut("ref") {
                Some(toml_edit::Item::Value(toml_edit::Value::InlineTable(r))) => {
                    update_git_ref_inline(r, git_ref);
                }
                Some(toml_edit::Item::Table(r)) => update_git_ref_table(r, git_ref),
                _ => {
                    loc.insert(
                        "ref",
                        toml_edit::Item::Value(toml_edit::Value::InlineTable(make_git_ref_inline(
                            git_ref,
                        ))),
                    );
                }
            }
        }
        _ => {
            table.insert(
                "locator",
                toml_edit::Item::Value(toml_edit::Value::InlineTable(make_locator_inline(
                    &rec.locator,
                ))),
            );
        }
    }

    match table.get_mut("resolved") {
        Some(toml_edit::Item::Value(toml_edit::Value::InlineTable(res))) => {
            set_inline_str(res, "commit", &rec.resolved.commit);
            set_inline_str(res, "at", &rec.resolved.at);
        }
        Some(toml_edit::Item::Table(res)) => {
            set_table_str(res, "commit", &rec.resolved.commit);
            set_table_str(res, "at", &rec.resolved.at);
        }
        _ => {
            table.insert(
                "resolved",
                toml_edit::Item::Value(toml_edit::Value::InlineTable(make_resolved_inline(
                    &rec.resolved,
                ))),
            );
        }
    }

    set_table_str(table, "interface_hash", &rec.interface_hash);
    set_table_str(table, "pin_reason", &rec.pin_reason);
}

fn find_and_update_in_doc(doc: &mut toml_edit::DocumentMut, rec: &LinkRecord) -> bool {
    match doc.get_mut("links") {
        Some(toml_edit::Item::ArrayOfTables(aot)) => {
            for tbl in aot.iter_mut() {
                if tbl.get("id").and_then(|i| i.as_str()) == Some(rec.id.as_str()) {
                    apply_record_to_table(tbl, rec);
                    return true;
                }
            }
            false
        }
        Some(toml_edit::Item::Value(toml_edit::Value::Array(arr))) => {
            for val in arr.iter_mut() {
                if let toml_edit::Value::InlineTable(it) = val
                    && it.get("id").and_then(|v| v.as_str()) == Some(rec.id.as_str())
                {
                    set_inline_str(it, "kind", &rec.kind);
                    set_inline_str(it, "board", &rec.board);
                    it.insert(
                        "locator",
                        toml_edit::Value::InlineTable(make_locator_inline(&rec.locator)),
                    );
                    it.insert(
                        "resolved",
                        toml_edit::Value::InlineTable(make_resolved_inline(&rec.resolved)),
                    );
                    set_inline_str(it, "interface_hash", &rec.interface_hash);
                    set_inline_str(it, "pin_reason", &rec.pin_reason);
                    return true;
                }
            }
            false
        }
        _ => false,
    }
}

fn append_record_to_doc(doc: &mut toml_edit::DocumentMut, rec: &LinkRecord) {
    let mut tbl = toml_edit::Table::new();
    tbl.insert("id", toml_edit::value(rec.id.as_str()));
    tbl.insert("kind", toml_edit::value(rec.kind.as_str()));
    tbl.insert("board", toml_edit::value(rec.board.as_str()));
    tbl.insert(
        "locator",
        toml_edit::Item::Value(toml_edit::Value::InlineTable(make_locator_inline(
            &rec.locator,
        ))),
    );
    tbl.insert(
        "resolved",
        toml_edit::Item::Value(toml_edit::Value::InlineTable(make_resolved_inline(
            &rec.resolved,
        ))),
    );
    tbl.insert(
        "interface_hash",
        toml_edit::value(rec.interface_hash.as_str()),
    );
    tbl.insert("pin_reason", toml_edit::value(rec.pin_reason.as_str()));

    if !doc.contains_key("links") {
        doc.insert(
            "links",
            toml_edit::Item::ArrayOfTables(toml_edit::ArrayOfTables::new()),
        );
    }
    if let Some(toml_edit::Item::ArrayOfTables(aot)) = doc.get_mut("links") {
        aot.push(tbl);
    }
}

fn remove_record_from_doc(doc: &mut toml_edit::DocumentMut, link_id: &str) -> bool {
    match doc.get_mut("links") {
        Some(toml_edit::Item::ArrayOfTables(aot)) => {
            let idx = aot
                .iter()
                .position(|t| t.get("id").and_then(|v| v.as_str()) == Some(link_id));
            if let Some(idx) = idx {
                aot.remove(idx);
                let empty = aot.is_empty();
                if empty {
                    doc.remove("links");
                }
                return true;
            }
            false
        }
        Some(toml_edit::Item::Value(toml_edit::Value::Array(arr))) => {
            let idx = arr.iter().position(|v| {
                v.as_inline_table()
                    .and_then(|t| t.get("id"))
                    .and_then(|v| v.as_str())
                    == Some(link_id)
            });
            if let Some(idx) = idx {
                arr.remove(idx);
                return true;
            }
            false
        }
        _ => false,
    }
}

/// Evaluates every link in `<project_root>/design/links.toml`.
/// `may_write` is false for requests from other devices, which Boardroom answers read-only: the
/// quiet advance of `resolved` and the re-materialization are then skipped.
pub async fn list_links(
    project_root: &Path,
    cache: &LinksCache,
    force_refresh: bool,
    may_write: bool,
) -> Result<Vec<LinkStatus>, String> {
    let root = project_root.to_path_buf();
    let records = tokio::task::spawn_blocking(move || read_links_file(&root))
        .await
        .map_err(|e| e.to_string())??;
    let mut out = Vec::with_capacity(records.len());
    for rec in records {
        out.push(evaluate_link(project_root, rec, cache, force_refresh, may_write).await);
    }
    Ok(out)
}

/// Reads this project's own published outline, `interface/<board>/board-shape.{dxf,json}`, if both
/// files exist and read cleanly.
fn read_own_record(project_root: &Path, board: &str) -> Option<Record> {
    validate_link_id(board).ok()?;
    let dir = project_root.join("interface").join(board);
    let dxf = std::fs::read(dir.join("board-shape.dxf")).ok()?;
    let json = std::fs::read(dir.join("board-shape.json")).ok()?;
    board_interface::read(&dxf, &json).ok()
}

/// Who drives the outline and whether take overs conflict, judged from this project's own outline
/// against the linked latest with `compare(own, latest)`. Without an own outline, the linked side
/// drives whenever its driver isn't Composer.
fn drive_against_own(
    project_root: &Path,
    board: &str,
    tip: &Record,
) -> (Option<Driver>, Option<LinkState>) {
    let theirs_drives = (tip.manifest.driver != Driver::Composer).then_some(tip.manifest.driver);
    let Some(own) = read_own_record(project_root, board) else {
        return (theirs_drives, None);
    };
    match board_interface::compare(&own, tip).classification {
        // Composer published on top of the linked latest, or took the drive from it: Composer drives.
        Classification::TheirsBehind => (None, None),
        Classification::CompetingTakeOvers => (None, Some(LinkState::CompetingTakeOvers)),
        Classification::StaleTakeOver => (None, Some(LinkState::StaleTakeOver)),
        _ => (theirs_drives, None),
    }
}

async fn evaluate_link(
    project_root: &Path,
    mut link: LinkRecord,
    cache: &LinksCache,
    force_refresh: bool,
    may_write: bool,
) -> LinkStatus {
    let following_path = following_path_for(project_root, &link.id)
        .display()
        .to_string();

    let unavailable = |link: &LinkRecord, err: String| LinkStatus {
        id: link.id.clone(),
        kind: link.kind.clone(),
        board: link.board.clone(),
        locator: link.locator.clone(),
        resolved: link.resolved.clone(),
        pin_reason: link.pin_reason.clone(),
        state: LinkState::Unavailable,
        locked_by: None,
        classification: None,
        changes: vec![],
        thickness_changed: None,
        rematched: vec![],
        uncertain: vec![],
        latest: None,
        following_path: following_path.clone(),
        error: Some(err),
    };

    if let Err(e) = validate_link_id(&link.id) {
        return unavailable(&link, e);
    }

    let Locator::Git {
        remote,
        path,
        git_ref,
    } = link.locator.clone();

    let (dxf_rel, json_rel) = match interface_git_paths(&path) {
        Ok(p) => p,
        Err(e) => return unavailable(&link, e),
    };

    let mirror = match cache
        .ensure_mirror(project_root, &remote, force_refresh)
        .await
    {
        Ok(m) => m,
        Err(e) => return unavailable(&link, e),
    };

    let base_commit = match git_rev_parse(
        &mirror,
        &format!("{}^{{commit}}", link.resolved.commit.trim()),
    )
    .await
    {
        Ok(c) => c,
        Err(e) => {
            return unavailable(
                &link,
                format!("resolved commit {}: {e}", link.resolved.commit),
            );
        }
    };

    let base_dxf = match git_cat_blob(&mirror, &base_commit, &dxf_rel).await {
        Ok(b) => b,
        Err(e) => return unavailable(&link, e),
    };
    let base_json = match git_cat_blob(&mirror, &base_commit, &json_rel).await {
        Ok(b) => b,
        Err(e) => return unavailable(&link, e),
    };

    let base_record = match board_interface::read(&base_dxf, &base_json) {
        Ok(r) => r,
        Err(e) => {
            return unavailable(
                &link,
                format!("{json_rel} at {}: {e}", short_sha(&base_commit)),
            );
        }
    };

    let base_hash = board_interface::hash(&base_record);
    if !link.interface_hash.is_empty() && link.interface_hash != base_hash {
        return LinkStatus {
            id: link.id,
            kind: link.kind,
            board: link.board,
            locator: link.locator,
            resolved: link.resolved,
            pin_reason: link.pin_reason,
            state: LinkState::Tampered,
            locked_by: (base_record.manifest.driver != Driver::Composer)
                .then_some(base_record.manifest.driver),
            classification: None,
            changes: vec![],
            thickness_changed: None,
            rematched: vec![],
            uncertain: vec![],
            latest: None,
            following_path,
            error: Some(format!(
                "stored interface_hash {} does not match geometry_hash {} at {}",
                link.interface_hash,
                base_hash,
                short_sha(&base_commit)
            )),
        };
    }

    // Materialize if `.boardroom/links/<id>` is missing (e.g. fresh clone of the project).
    if may_write && !materialized_exists(project_root, &link.id) {
        let root = project_root.to_path_buf();
        let id = link.id.clone();
        let dxf = base_dxf.clone();
        let js = base_json.clone();
        if let Err(e) =
            tokio::task::spawn_blocking(move || materialize_bytes(&root, &id, &dxf, &js))
                .await
                .map_err(|e| e.to_string())
                .and_then(|r| r)
        {
            return unavailable(&link, e);
        }
    }

    // Resolve tip commit (for Commit refs, tip = remote's default branch HEAD to report PinnedBehind).
    let tip_commit = match &git_ref {
        GitRef::Branch { .. } | GitRef::Tag { .. } => {
            match resolve_git_ref(&mirror, &git_ref).await {
                Ok(c) => c,
                Err(e) => return unavailable(&link, e),
            }
        }
        GitRef::Commit { commit } => {
            if let Err(e) = resolve_git_ref(
                &mirror,
                &GitRef::Commit {
                    commit: commit.clone(),
                },
            )
            .await
            {
                return unavailable(&link, e);
            }
            match resolve_default_head(&mirror, &base_commit).await {
                Ok(c) => c,
                Err(e) => return unavailable(&link, e),
            }
        }
    };

    let tip_at = match git_commit_time(&mirror, &tip_commit).await {
        Ok(t) => t,
        Err(e) => return unavailable(&link, e),
    };

    let (tip_dxf, tip_json, tip_record) = if tip_commit == base_commit {
        (base_dxf, base_json, base_record.clone())
    } else {
        let td = match git_cat_blob(&mirror, &tip_commit, &dxf_rel).await {
            Ok(b) => b,
            Err(e) => return unavailable(&link, e),
        };
        let tj = match git_cat_blob(&mirror, &tip_commit, &json_rel).await {
            Ok(b) => b,
            Err(e) => return unavailable(&link, e),
        };
        let tr = match board_interface::read(&td, &tj) {
            Ok(r) => r,
            Err(e) => {
                return unavailable(
                    &link,
                    format!("{json_rel} at {}: {e}", short_sha(&tip_commit)),
                );
            }
        };
        (td, tj, tr)
    };

    // Who drives, and whether two take overs conflict, is a question about this project's own
    // outline (`interface/<board>/`, which Composer publishes) against the linked latest. The
    // accepted-vs-latest comparison below only says how the linked file moved.
    let (locked_by, conflict) = drive_against_own(project_root, &link.board, &tip_record);
    let latest = Some(Latest {
        commit: tip_commit.clone(),
        at: tip_at,
        revision: tip_record.manifest.revision.clone(),
        driver: tip_record.manifest.driver,
    });

    if tip_commit == base_commit {
        let diff = board_interface::compare(&base_record, &tip_record);
        return LinkStatus {
            id: link.id,
            kind: link.kind,
            board: link.board,
            locator: link.locator,
            resolved: link.resolved,
            pin_reason: link.pin_reason,
            state: conflict.unwrap_or(LinkState::Current),
            locked_by,
            classification: Some(diff.classification),
            changes: vec![],
            thickness_changed: None,
            rematched: vec![],
            uncertain: vec![],
            latest,
            following_path,
            error: None,
        };
    }

    let diff = board_interface::compare(&base_record, &tip_record);
    let tip_hash = board_interface::hash(&tip_record);
    let same_interface = tip_hash == base_hash
        && tip_record.manifest.revision == base_record.manifest.revision
        && diff.classification == Classification::InSync;

    match &git_ref {
        GitRef::Commit { .. } => {
            if same_interface {
                LinkStatus {
                    id: link.id,
                    kind: link.kind,
                    board: link.board,
                    locator: link.locator,
                    resolved: link.resolved,
                    pin_reason: link.pin_reason,
                    state: conflict.unwrap_or(LinkState::NewRevisionNoInterfaceChange),
                    locked_by,
                    classification: Some(diff.classification),
                    changes: vec![],
                    thickness_changed: None,
                    rematched: vec![],
                    uncertain: vec![],
                    latest,
                    following_path,
                    error: None,
                }
            } else {
                LinkStatus {
                    id: link.id,
                    kind: link.kind,
                    board: link.board,
                    locator: link.locator,
                    resolved: link.resolved,
                    pin_reason: link.pin_reason,
                    state: conflict.unwrap_or(LinkState::PinnedBehind),
                    locked_by,
                    classification: Some(diff.classification),
                    changes: diff.changes,
                    thickness_changed: diff.thickness_changed,
                    rematched: diff.rematched,
                    uncertain: diff.uncertain,
                    latest,
                    following_path,
                    error: None,
                }
            }
        }
        GitRef::Branch { .. } | GitRef::Tag { .. } => {
            if same_interface && may_write {
                let new_resolved = Resolved {
                    commit: tip_commit,
                    at: now_utc_iso(),
                };
                link.resolved = new_resolved.clone();
                let root = project_root.to_path_buf();
                let updated_link = link.clone();
                let advance_res = tokio::task::spawn_blocking(move || {
                    edit_links_toml(&root, |doc, _| {
                        find_and_update_in_doc(doc, &updated_link);
                        Ok(())
                    })
                    .map_err(|e| e.to_string())?;
                    materialize_bytes(&root, &updated_link.id, &tip_dxf, &tip_json)
                })
                .await
                .map_err(|e| e.to_string())
                .and_then(|r| r);

                if let Err(e) = advance_res {
                    return unavailable(&link, e);
                }

                return LinkStatus {
                    id: link.id,
                    kind: link.kind,
                    board: link.board,
                    locator: link.locator,
                    resolved: new_resolved,
                    pin_reason: link.pin_reason,
                    state: conflict.unwrap_or(LinkState::NewRevisionNoInterfaceChange),
                    locked_by,
                    classification: Some(diff.classification),
                    changes: vec![],
                    thickness_changed: None,
                    rematched: vec![],
                    uncertain: vec![],
                    latest,
                    following_path,
                    error: None,
                };
            }

            // Accepted vs latest are two versions of the linked file, so only "changed outside
            // the suite" is meaningful here; take-over conflicts come from `drive_against_own`.
            let state = match diff.classification {
                Classification::ChangedOutsideSuite => LinkState::ChangedOutsideSuite,
                // Read-only request: same answer as the quiet advance above, without the write.
                _ if same_interface => conflict.unwrap_or(LinkState::NewRevisionNoInterfaceChange),
                _ => conflict.unwrap_or(LinkState::UpdateAvailable),
            };

            LinkStatus {
                id: link.id,
                kind: link.kind,
                board: link.board,
                locator: link.locator,
                resolved: link.resolved,
                pin_reason: link.pin_reason,
                state,
                locked_by,
                classification: Some(diff.classification),
                changes: diff.changes,
                thickness_changed: diff.thickness_changed,
                rematched: diff.rematched,
                uncertain: diff.uncertain,
                latest,
                following_path,
                error: None,
            }
        }
    }
}

/// Adds a link to `design/links.toml`, validating it once against the remote, setting `resolved`
/// and `interface_hash` to the resolved tip, and materializing the files for Composer.
pub async fn add_link(
    project_root: &Path,
    req: AddLinkRequest,
    cache: &LinksCache,
) -> Result<LinkStatus, LinkOpError> {
    let id = req.id.trim().to_string();
    validate_link_id(&id).map_err(LinkOpError::BadRequest)?;
    let board = req.board.trim().to_string();
    if board.is_empty() {
        return Err(LinkOpError::BadRequest("board must not be empty".into()));
    }
    let kind = if req.kind.trim().is_empty() {
        "waffle".into()
    } else {
        req.kind.trim().to_string()
    };

    let (remote, path, mut git_ref) = match req.locator {
        Some(Locator::Git {
            remote,
            path,
            git_ref,
        }) => (remote.trim().to_string(), path.trim().to_string(), git_ref),
        None => {
            let remote = req
                .remote
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| LinkOpError::BadRequest("remote is required".into()))?
                .to_string();
            let path = req
                .path
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| LinkOpError::BadRequest("path is required".into()))?
                .to_string();
            let git_ref = req.git_ref.unwrap_or_else(|| GitRef::Branch {
                name: "main".into(),
            });
            (remote, path, git_ref)
        }
    };

    let (dxf_rel, json_rel) = interface_git_paths(&path).map_err(LinkOpError::BadRequest)?;
    let mirror = cache
        .ensure_mirror(project_root, &remote, true)
        .await
        .map_err(LinkOpError::BadRequest)?;
    let resolved_commit = resolve_git_ref(&mirror, &git_ref)
        .await
        .map_err(LinkOpError::BadRequest)?;
    if let GitRef::Commit { commit } = &mut git_ref {
        *commit = resolved_commit.clone();
    }

    let dxf_bytes = git_cat_blob(&mirror, &resolved_commit, &dxf_rel)
        .await
        .map_err(LinkOpError::BadRequest)?;
    let json_bytes = git_cat_blob(&mirror, &resolved_commit, &json_rel)
        .await
        .map_err(LinkOpError::BadRequest)?;
    let record = board_interface::read(&dxf_bytes, &json_bytes)
        .map_err(|e| LinkOpError::BadRequest(format!("{json_rel}: {e}")))?;
    let interface_hash = board_interface::hash(&record);

    let link_rec = LinkRecord {
        id: id.clone(),
        kind,
        board,
        locator: Locator::Git {
            remote,
            path: json_rel,
            git_ref,
        },
        resolved: Resolved {
            commit: resolved_commit,
            at: now_utc_iso(),
        },
        interface_hash,
        pin_reason: req.pin_reason,
    };

    let root = project_root.to_path_buf();
    let rec_for_write = link_rec.clone();
    tokio::task::spawn_blocking(move || {
        edit_links_toml(&root, |doc, existing| {
            if existing.iter().any(|l| l.id == rec_for_write.id) {
                return Err(LinkOpError::BadRequest(format!(
                    "link '{}' already exists",
                    rec_for_write.id
                )));
            }
            append_record_to_doc(doc, &rec_for_write);
            Ok(())
        })?;
        materialize_bytes(&root, &rec_for_write.id, &dxf_bytes, &json_bytes)
            .map_err(LinkOpError::Io)
    })
    .await
    .map_err(|e| LinkOpError::Io(e.to_string()))??;

    Ok(evaluate_link(project_root, link_rec, cache, false, true).await)
}

/// Accepts the current tip for `link_id`: sets `resolved` to the tip commit, sets `interface_hash`
/// to `board_interface::hash(&tip)`, and rewrites the materialized files.
pub async fn update_link(
    project_root: &Path,
    link_id: &str,
    cache: &LinksCache,
) -> Result<LinkStatus, LinkOpError> {
    validate_link_id(link_id).map_err(LinkOpError::BadRequest)?;
    let root = project_root.to_path_buf();
    let id = link_id.to_string();
    let existing = tokio::task::spawn_blocking(move || {
        read_links_file(&root)
            .map_err(LinkOpError::Io)?
            .into_iter()
            .find(|l| l.id == id)
            .ok_or_else(|| LinkOpError::NotFound(format!("no link '{id}'")))
    })
    .await
    .map_err(|e| LinkOpError::Io(e.to_string()))??;

    let Locator::Git {
        remote,
        path,
        mut git_ref,
    } = existing.locator.clone();
    let (dxf_rel, json_rel) = interface_git_paths(&path).map_err(LinkOpError::BadRequest)?;
    let mirror = cache
        .ensure_mirror(project_root, &remote, true)
        .await
        .map_err(LinkOpError::BadRequest)?;

    let tip_commit = match &git_ref {
        GitRef::Branch { .. } | GitRef::Tag { .. } => resolve_git_ref(&mirror, &git_ref)
            .await
            .map_err(LinkOpError::BadRequest)?,
        GitRef::Commit { .. } => resolve_default_head(&mirror, &existing.resolved.commit)
            .await
            .map_err(LinkOpError::BadRequest)?,
    };
    if let GitRef::Commit { commit } = &mut git_ref {
        *commit = tip_commit.clone();
    }

    let dxf_bytes = git_cat_blob(&mirror, &tip_commit, &dxf_rel)
        .await
        .map_err(LinkOpError::BadRequest)?;
    let json_bytes = git_cat_blob(&mirror, &tip_commit, &json_rel)
        .await
        .map_err(LinkOpError::BadRequest)?;
    let tip_record = board_interface::read(&dxf_bytes, &json_bytes)
        .map_err(|e| LinkOpError::BadRequest(format!("{json_rel}: {e}")))?;
    let interface_hash = board_interface::hash(&tip_record);

    let root = project_root.to_path_buf();
    let id = link_id.to_string();
    let now = now_utc_iso();
    let updated = tokio::task::spawn_blocking(move || {
        let updated = edit_links_toml(&root, |doc, list| {
            let mut rec = list
                .into_iter()
                .find(|l| l.id == id)
                .ok_or_else(|| LinkOpError::NotFound(format!("no link '{id}'")))?;
            rec.locator = Locator::Git {
                remote,
                path: json_rel,
                git_ref,
            };
            rec.resolved = Resolved {
                commit: tip_commit,
                at: now,
            };
            rec.interface_hash = interface_hash;
            find_and_update_in_doc(doc, &rec);
            Ok(rec)
        })?;
        materialize_bytes(&root, &updated.id, &dxf_bytes, &json_bytes).map_err(LinkOpError::Io)?;
        Ok::<_, LinkOpError>(updated)
    })
    .await
    .map_err(|e| LinkOpError::Io(e.to_string()))??;

    Ok(evaluate_link(project_root, updated, cache, false, true).await)
}

/// Pins `link_id` at `resolved.commit` (`ref = Commit { commit: resolved.commit }`) with `reason`.
pub async fn pin_link(
    project_root: &Path,
    link_id: &str,
    reason: String,
    cache: &LinksCache,
) -> Result<LinkStatus, LinkOpError> {
    validate_link_id(link_id).map_err(LinkOpError::BadRequest)?;
    let root = project_root.to_path_buf();
    let id = link_id.to_string();
    let updated = tokio::task::spawn_blocking(move || {
        edit_links_toml(&root, |doc, list| {
            let mut rec = list
                .into_iter()
                .find(|l| l.id == id)
                .ok_or_else(|| LinkOpError::NotFound(format!("no link '{id}'")))?;
            let Locator::Git { remote, path, .. } = rec.locator;
            rec.locator = Locator::Git {
                remote,
                path,
                git_ref: GitRef::Commit {
                    commit: rec.resolved.commit.clone(),
                },
            };
            rec.pin_reason = reason;
            find_and_update_in_doc(doc, &rec);
            Ok(rec)
        })
    })
    .await
    .map_err(|e| LinkOpError::Io(e.to_string()))??;

    Ok(evaluate_link(project_root, updated, cache, false, true).await)
}

/// Switches `link_id` to follow `Branch { name: branch }` (clearing `pin_reason`).
pub async fn follow_link(
    project_root: &Path,
    link_id: &str,
    branch: String,
    cache: &LinksCache,
) -> Result<LinkStatus, LinkOpError> {
    validate_link_id(link_id).map_err(LinkOpError::BadRequest)?;
    let branch = branch.trim().to_string();
    if branch.is_empty() || branch.starts_with('-') {
        return Err(LinkOpError::BadRequest(
            "branch name must not be empty".into(),
        ));
    }
    let root = project_root.to_path_buf();
    let id = link_id.to_string();
    let updated = tokio::task::spawn_blocking(move || {
        edit_links_toml(&root, |doc, list| {
            let mut rec = list
                .into_iter()
                .find(|l| l.id == id)
                .ok_or_else(|| LinkOpError::NotFound(format!("no link '{id}'")))?;
            let Locator::Git { remote, path, .. } = rec.locator;
            rec.locator = Locator::Git {
                remote,
                path,
                git_ref: GitRef::Branch { name: branch },
            };
            rec.pin_reason.clear();
            find_and_update_in_doc(doc, &rec);
            Ok(rec)
        })
    })
    .await
    .map_err(|e| LinkOpError::Io(e.to_string()))??;

    Ok(evaluate_link(project_root, updated, cache, true, true).await)
}

/// Removes `link_id` from `design/links.toml` and deletes its materialized files.
pub fn remove_link(project_root: &Path, link_id: &str) -> Result<(), LinkOpError> {
    validate_link_id(link_id).map_err(LinkOpError::BadRequest)?;
    edit_links_toml(project_root, |doc, _| {
        if !remove_record_from_doc(doc, link_id) {
            return Err(LinkOpError::NotFound(format!("no link '{link_id}'")));
        }
        Ok(())
    })?;
    cleanup_materialized(project_root, link_id);
    Ok(())
}

/// Derives Inbox `needs_user` items for links in `ChangedOutsideSuite` or `CompetingTakeOvers`
/// (`docs/suite-spec.md`, "Link states"), without modifying `design/status.json`.
pub fn needs_from_statuses(statuses: &[LinkStatus]) -> Vec<Need> {
    statuses
        .iter()
        .filter_map(|s| match s.state {
            LinkState::ChangedOutsideSuite => Some(Need {
                id: format!("link:{}:{}", s.id, s.state.as_str()),
                kind: "alert".into(),
                text: format!(
                    "Linked outline {} was changed outside the suite: import it?",
                    s.id
                ),
                gate: None,
                options: vec![],
            }),
            LinkState::CompetingTakeOvers => Some(Need {
                id: format!("link:{}:{}", s.id, s.state.as_str()),
                kind: "alert".into(),
                text: format!("Two take overs of {}'s outline: pick one", s.board),
                gate: None,
                options: vec![],
            }),
            LinkState::StaleTakeOver => Some(Need {
                id: format!("link:{}:{}", s.id, s.state.as_str()),
                kind: "alert".into(),
                text: format!(
                    "{}'s outline was taken over from an older revision while the other side kept driving: pick one",
                    s.board
                ),
                gate: None,
                options: vec![],
            }),
            _ => None,
        })
        .collect()
}

/// Formats `SystemTime::now()` as RFC 3339 / ISO 8601 UTC (`YYYY-MM-DDTHH:MM:SSZ`).
pub fn now_utc_iso() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format_unix_utc(secs)
}

fn format_unix_utc(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let sod = secs % 86_400;
    let h = sod / 3_600;
    let m = (sod % 3_600) / 60;
    let s = sod % 60;

    // Howard Hinnant's civil_from_days algorithm (proleptic Gregorian calendar).
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if mo <= 2 { y + 1 } else { y };

    format!("{year:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_timestamp_formatting_matches_known_dates() {
        assert_eq!(format_unix_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_unix_utc(1_793_664_000), "2026-11-03T00:00:00Z");
    }
}
