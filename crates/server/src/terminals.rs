//! Project terminals: interactive `claude` sessions in server-owned PTYs
//! (`docs/shell-spec.md`, "Terminals", and `docs/suite-spec.md`, milestone 9).
//!
//! Each terminal outlives the browser tab or GTK window viewing it: a 1 MiB
//! scrollback ring buffer is replayed to every connecting WebSocket before live
//! output begins, and multiple sockets may watch one terminal at once.
//!
//! Waffle Iron's MCP relay (`waffle-mcp-relay`) can be attached per terminal by
//! setting in `~/.config/boardroom/config.toml`:
//!
//! ```toml
//! waffle_relay = "uv run --project /path/to/waffle-iron/relay waffle-mcp-relay"
//! ```
//!
//! Its WebSocket port comes from `waffle_relay_port` in `config.toml`, else
//! `proj port boardroom-waffle-relay`. Because one relay holds that port, at
//! most one live terminal gets Waffle tools at a time (the first terminal of a
//! project with a `waffle` link in `design/links.toml`, or started with
//! `{"waffle": true}`).

use ::project::config::{self, Config};
use ::project::project;
use axum::body::Bytes;
use axum::extract::ws::{Message, WebSocket};
use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, VecDeque};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{broadcast, mpsc};

/// Scrollback buffer kept per terminal (~1 MiB).
pub const MAX_SCROLLBACK: usize = 1024 * 1024;

const BROADCAST_CAPACITY: usize = 512;
const INPUT_QUEUE_CAPACITY: usize = 256;
const DEFAULT_COLS: u16 = 80;
const DEFAULT_ROWS: u16 = 24;

pub type RelayPortResolver = Arc<dyn Fn() -> Option<u16> + Send + Sync>;

/// Options controlling state/cache locations and test overrides for `Terminals`.
#[derive(Clone)]
pub struct TerminalsOptions {
    /// `$XDG_STATE_HOME/boardroom` (holds `terminals.json`).
    pub state_dir: PathBuf,
    /// `$XDG_CACHE_HOME/boardroom/term` (holds `<id>/mcp.json`).
    pub cache_dir: PathBuf,
    /// Optional command override for tests so no test ever launches real `claude`.
    pub term_command: Option<Vec<String>>,
    /// Fallback resolver for the Waffle relay port (`proj port boardroom-waffle-relay`).
    pub relay_port_resolver: RelayPortResolver,
}

impl Default for TerminalsOptions {
    fn default() -> Self {
        Self {
            state_dir: default_state_dir(),
            cache_dir: default_term_cache_dir(),
            term_command: None,
            relay_port_resolver: Arc::new(proj_waffle_relay_port),
        }
    }
}

pub fn default_state_dir() -> PathBuf {
    let base = std::env::var_os("XDG_STATE_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| config::home().join(".local/state"));
    base.join("boardroom")
}

pub fn default_term_cache_dir() -> PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| config::home().join(".cache"));
    base.join("boardroom/term")
}

pub fn proj_waffle_relay_port() -> Option<u16> {
    let out = std::process::Command::new("proj")
        .args(["port", "boardroom-waffle-relay"])
        .stdin(std::process::Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

#[derive(Debug, Clone, Deserialize)]
pub struct StartTermRequest {
    pub project: String,
    #[serde(default = "default_kind")]
    pub kind: String,
    #[serde(default)]
    pub waffle: bool,
    pub cols: Option<u16>,
    pub rows: Option<u16>,
}

fn default_kind() -> String {
    "project".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TermSummary {
    pub id: String,
    pub project: String,
    pub kind: String,
    pub started: String,
    pub alive: bool,
    pub title: String,
    pub waffle: bool,
}

#[derive(Debug, Deserialize)]
struct ResizeFrame {
    #[serde(rename = "type")]
    frame_type: String,
    cols: u16,
    rows: u16,
}

#[derive(Clone)]
enum TermEvent {
    Chunk { end_offset: u64, data: Bytes },
    Exited,
}

struct Scrollback {
    buf: VecDeque<u8>,
    total_written: u64,
}

impl Scrollback {
    fn new() -> Self {
        Self {
            buf: VecDeque::new(),
            total_written: 0,
        }
    }

    fn push(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        self.total_written = self.total_written.saturating_add(bytes.len() as u64);
        if bytes.len() >= MAX_SCROLLBACK {
            self.buf.clear();
            self.buf
                .extend(bytes[bytes.len() - MAX_SCROLLBACK..].iter().copied());
            return;
        }
        let excess = (self.buf.len() + bytes.len()).saturating_sub(MAX_SCROLLBACK);
        if excess > 0 {
            self.buf.drain(..excess);
        }
        self.buf.extend(bytes.iter().copied());
    }

    fn snapshot(&self) -> (Vec<u8>, u64) {
        let (a, b) = self.buf.as_slices();
        let mut out = Vec::with_capacity(self.buf.len());
        out.extend_from_slice(a);
        out.extend_from_slice(b);
        (out, self.total_written)
    }
}

struct TermInner {
    scrollback: Scrollback,
    alive: bool,
    title: String,
    tx: broadcast::Sender<TermEvent>,
    /// Incomplete OSC 0/2 escape sequence tail across read chunks.
    osc_tail: Vec<u8>,
}

struct Terminal {
    id: String,
    project: String,
    kind: String,
    started: String,
    waffle: bool,
    inner: Mutex<TermInner>,
    master: Mutex<Option<Box<dyn MasterPty + Send>>>,
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
    input_tx: Mutex<Option<mpsc::Sender<Vec<u8>>>>,
}

impl Terminal {
    fn summary(&self) -> TermSummary {
        let inner = self.inner.lock().unwrap();
        TermSummary {
            id: self.id.clone(),
            project: self.project.clone(),
            kind: self.kind.clone(),
            started: self.started.clone(),
            alive: inner.alive,
            title: inner.title.clone(),
            waffle: self.waffle && inner.alive,
        }
    }

    fn is_alive(&self) -> bool {
        self.inner.lock().unwrap().alive
    }

    fn resize(&self, cols: u16, rows: u16) {
        let cols = cols.clamp(1, 500);
        let rows = rows.clamp(1, 200);
        if let Some(master) = self.master.lock().unwrap().as_ref() {
            let _ = master.resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            });
        }
    }

    fn terminate(&self) {
        let _ = self.killer.lock().unwrap().kill();
        *self.input_tx.lock().unwrap() = None;
        *self.master.lock().unwrap() = None;
        let mut inner = self.inner.lock().unwrap();
        if inner.alive {
            inner.alive = false;
            let _ = inner.tx.send(TermEvent::Exited);
        }
    }
}

#[derive(Debug)]
pub enum TermError {
    BadRequest(String),
    NotFound(String),
    Internal(String),
}

impl std::fmt::Display for TermError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadRequest(m) | Self::NotFound(m) | Self::Internal(m) => f.write_str(m),
        }
    }
}

pub struct Terminals {
    options: TerminalsOptions,
    start_gate: tokio::sync::Mutex<()>,
    terms: Mutex<Vec<Arc<Terminal>>>,
    waffle_holder: Arc<Mutex<Option<String>>>,
    next_seq: AtomicU64,
}

impl Default for Terminals {
    fn default() -> Self {
        Self::new(TerminalsOptions::default())
    }
}

impl Drop for Terminals {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl Terminals {
    pub fn new(options: TerminalsOptions) -> Self {
        Self {
            options,
            start_gate: tokio::sync::Mutex::new(()),
            terms: Mutex::new(Vec::new()),
            waffle_holder: Arc::new(Mutex::new(None)),
            next_seq: AtomicU64::new(1),
        }
    }

    pub fn shutdown(&self) {
        let list: Vec<Arc<Terminal>> = self.terms.lock().unwrap().drain(..).collect();
        *self.waffle_holder.lock().unwrap() = None;
        for t in list {
            t.terminate();
            cleanup_mcp_file(&self.options.cache_dir, &t.id);
        }
    }

    pub fn list(&self) -> Vec<TermSummary> {
        self.terms
            .lock()
            .unwrap()
            .iter()
            .map(|t| t.summary())
            .collect()
    }

    pub fn exists(&self, id: &str) -> bool {
        self.terms.lock().unwrap().iter().any(|t| t.id == id)
    }

    /// Starts a terminal for `(req.project, req.kind)` or returns the live one
    /// that already exists.
    pub async fn start_or_get(
        &self,
        config_dir: &Path,
        project_root: &Path,
        req: StartTermRequest,
        boardroom_port: u16,
        bus: &broadcast::Sender<String>,
    ) -> Result<TermSummary, TermError> {
        validate_kind(&req.kind)?;
        if !project_root.is_dir() {
            return Err(TermError::BadRequest(format!(
                "project folder {} is missing",
                project_root.display()
            )));
        }

        let _guard = self.start_gate.lock().await;

        // Return existing live terminal for (project, kind).
        if let Some(existing) = self
            .terms
            .lock()
            .unwrap()
            .iter()
            .find(|t| t.project == req.project && t.kind == req.kind && t.is_alive())
            .cloned()
        {
            if let (Some(cols), Some(rows)) = (req.cols, req.rows) {
                existing.resize(cols, rows);
            }
            return Ok(existing.summary());
        }

        let cfg = Config::load_from(config_dir).map_err(TermError::Internal)?;
        let id = format!("t-{}", self.next_seq.fetch_add(1, Ordering::Relaxed));

        let cols = req.cols.unwrap_or(DEFAULT_COLS).clamp(2, 500);
        let rows = req.rows.unwrap_or(DEFAULT_ROWS).clamp(2, 200);

        let config_dir_owned = config_dir.to_path_buf();
        let root_owned = project_root.to_path_buf();
        let options = self.options.clone();
        let waffle_holder = self.waffle_holder.clone();
        let bus_clone = bus.clone();
        let req_clone = req.clone();
        let id_clone = id.clone();

        let created = tokio::task::spawn_blocking(move || {
            let _ = config_dir_owned;
            spawn_terminal(
                &options,
                &cfg,
                &root_owned,
                &req_clone,
                &id_clone,
                cols,
                rows,
                boardroom_port,
                waffle_holder,
                bus_clone,
            )
        })
        .await
        .map_err(|e| TermError::Internal(e.to_string()))??;

        let summary = created.summary();
        let mut stale_ids = Vec::new();
        {
            let mut list = self.terms.lock().unwrap();
            list.retain(|t| {
                if t.project == req.project && t.kind == req.kind && !t.is_alive() {
                    stale_ids.push(t.id.clone());
                    false
                } else {
                    true
                }
            });
            list.push(created);
        }
        for old_id in stale_ids {
            cleanup_mcp_file(&self.options.cache_dir, &old_id);
        }

        let _ = bus.send(
            serde_json::json!({
                "type": "term",
                "project": req.project,
                "id": id,
                "alive": true,
            })
            .to_string(),
        );

        Ok(summary)
    }

    pub fn delete(&self, id: &str, bus: &broadcast::Sender<String>) -> Result<(), TermError> {
        let removed = {
            let mut list = self.terms.lock().unwrap();
            let pos = list
                .iter()
                .position(|t| t.id == id)
                .ok_or_else(|| TermError::NotFound("no such terminal".into()))?;
            list.remove(pos)
        };
        removed.terminate();
        {
            let mut holder = self.waffle_holder.lock().unwrap();
            if holder.as_deref() == Some(id) {
                *holder = None;
            }
        }
        cleanup_mcp_file(&self.options.cache_dir, id);
        let _ = bus.send(
            serde_json::json!({
                "type": "term",
                "project": removed.project,
                "id": removed.id,
                "alive": false,
            })
            .to_string(),
        );
        Ok(())
    }

    pub async fn attach(&self, id: &str, mut socket: WebSocket) {
        let Some(term) = self
            .terms
            .lock()
            .unwrap()
            .iter()
            .find(|t| t.id == id)
            .cloned()
        else {
            let _ = socket.send(Message::Close(None)).await;
            return;
        };

        let (mut rx, replay, mut sent_offset, alive) = {
            let inner = term.inner.lock().unwrap();
            let rx = inner.tx.subscribe();
            let (replay, offset) = inner.scrollback.snapshot();
            (rx, replay, offset, inner.alive)
        };

        if !replay.is_empty() && socket.send(Message::Binary(replay.into())).await.is_err() {
            return;
        }
        if !alive {
            let _ = socket.send(Message::Close(None)).await;
            return;
        }

        loop {
            tokio::select! {
                evt = rx.recv() => match evt {
                    Ok(TermEvent::Chunk { end_offset, data }) => {
                        if end_offset <= sent_offset {
                            continue;
                        }
                        let start_offset = end_offset.saturating_sub(data.len() as u64);
                        let slice = if start_offset < sent_offset {
                            data.slice((sent_offset - start_offset) as usize..)
                        } else {
                            data
                        };
                        if socket.send(Message::Binary(slice)).await.is_err() {
                            break;
                        }
                        sent_offset = end_offset;
                    }
                    Ok(TermEvent::Exited) => {
                        let _ = socket.send(Message::Close(None)).await;
                        break;
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        let (snap, now_offset, still_alive) = {
                            let inner = term.inner.lock().unwrap();
                            let (s, o) = inner.scrollback.snapshot();
                            (s, o, inner.alive)
                        };
                        if now_offset > sent_offset && !snap.is_empty() {
                            let snap_start = now_offset.saturating_sub(snap.len() as u64);
                            let tail = if snap_start < sent_offset {
                                snap[(sent_offset - snap_start) as usize..].to_vec()
                            } else {
                                snap
                            };
                            if !tail.is_empty()
                                && socket.send(Message::Binary(tail.into())).await.is_err()
                            {
                                break;
                            }
                            sent_offset = now_offset;
                        }
                        if !still_alive {
                            let _ = socket.send(Message::Close(None)).await;
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                },
                incoming = socket.recv() => match incoming {
                    Some(Ok(Message::Binary(bytes))) => {
                        if !bytes.is_empty() {
                            let tx = term.input_tx.lock().unwrap().clone();
                            if let Some(tx) = tx {
                                let _ = tx.send(bytes.to_vec()).await;
                            }
                        }
                    }
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(frame) = serde_json::from_str::<ResizeFrame>(&text)
                            && frame.frame_type == "resize"
                        {
                            term.resize(frame.cols, frame.rows);
                        }
                    }
                    Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) => {}
                    Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                }
            }
        }
    }
}

fn validate_kind(kind: &str) -> Result<(), TermError> {
    if kind.is_empty()
        || kind.len() > 32
        || !kind
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(TermError::BadRequest(format!(
            "invalid terminal kind '{kind}'"
        )));
    }
    Ok(())
}

struct WaffleSetup {
    mcp_path: Option<PathBuf>,
    banner: Option<String>,
    claimed_holder: bool,
}

fn prepare_waffle_mcp(
    options: &TerminalsOptions,
    cfg: &Config,
    project_root: &Path,
    req: &StartTermRequest,
    term_id: &str,
    boardroom_port: u16,
    waffle_holder: &Mutex<Option<String>>,
) -> Result<WaffleSetup, TermError> {
    let Some(relay_line) = cfg.waffle_relay.as_deref() else {
        return Ok(WaffleSetup {
            mcp_path: None,
            banner: None,
            claimed_holder: false,
        });
    };

    let relay_argv = match config::split_cmdline(relay_line) {
        Ok(v) => v,
        Err(e) => {
            return Ok(WaffleSetup {
                mcp_path: None,
                banner: Some(format!(
                    "Waffle Iron tools unavailable: invalid waffle_relay in config.toml ({e}).\r\n"
                )),
                claimed_holder: false,
            });
        }
    };

    let wants_waffle = req.waffle || has_waffle_link(project_root);
    if !wants_waffle {
        return Ok(WaffleSetup {
            mcp_path: None,
            banner: Some(
                "No Waffle Iron tools in this terminal: project has no waffle link in design/links.toml.\r\n"
                    .into(),
            ),
            claimed_holder: false,
        });
    }

    if boardroom_port == 0 {
        return Ok(WaffleSetup {
            mcp_path: None,
            banner: Some(
                "Waffle Iron tools unavailable: Boardroom's bound port is not known.\r\n".into(),
            ),
            claimed_holder: false,
        });
    }

    let mut holder = waffle_holder.lock().unwrap();
    if let Some(other) = holder.as_deref() {
        return Ok(WaffleSetup {
            mcp_path: None,
            banner: Some(format!(
                "No Waffle Iron tools in this terminal: the Waffle MCP relay port is already held by terminal {other}.\r\n"
            )),
            claimed_holder: false,
        });
    }

    let Some(relay_port) = cfg
        .waffle_relay_port
        .or_else(|| (options.relay_port_resolver)())
    else {
        return Ok(WaffleSetup {
            mcp_path: None,
            banner: Some(
                "Waffle Iron tools unavailable: no relay port configured (set waffle_relay_port in config.toml or register boardroom-waffle-relay with proj).\r\n"
                    .into(),
            ),
            claimed_holder: false,
        });
    };

    if relay_port < 1024 {
        return Ok(WaffleSetup {
            mcp_path: None,
            banner: Some(format!(
                "Waffle Iron tools unavailable: relay port {relay_port} is outside 1024..=65535.\r\n"
            )),
            claimed_holder: false,
        });
    }

    let (cmd, base_args) = relay_argv.split_first().unwrap();
    let mut args: Vec<String> = base_args.to_vec();
    args.extend([
        "--port".into(),
        relay_port.to_string(),
        "--app-url".into(),
        format!("http://127.0.0.1:{boardroom_port}/apps/waffle/"),
        "--allow-origin".into(),
        format!("http://127.0.0.1:{boardroom_port}"),
        "--allow-origin".into(),
        format!("http://localhost:{boardroom_port}"),
    ]);

    let term_dir = options.cache_dir.join(term_id);
    std::fs::create_dir_all(&term_dir)
        .map_err(|e| TermError::Internal(format!("{}: {e}", term_dir.display())))?;
    let mcp_path = term_dir.join("mcp.json");
    let mcp_json = serde_json::json!({
        "mcpServers": {
            "waffle": {
                "command": cmd,
                "args": args,
            }
        }
    });
    let bytes =
        serde_json::to_vec_pretty(&mcp_json).map_err(|e| TermError::Internal(e.to_string()))?;
    std::fs::write(&mcp_path, bytes)
        .map_err(|e| TermError::Internal(format!("{}: {e}", mcp_path.display())))?;

    *holder = Some(term_id.to_string());
    Ok(WaffleSetup {
        mcp_path: Some(mcp_path),
        banner: None,
        claimed_holder: true,
    })
}

fn cleanup_mcp_file(cache_dir: &Path, term_id: &str) {
    let dir = cache_dir.join(term_id);
    let _ = std::fs::remove_file(dir.join("mcp.json"));
    let _ = std::fs::remove_dir(dir);
}

#[derive(Deserialize, Default)]
struct LooseLinksFile {
    #[serde(default)]
    links: Vec<LooseLink>,
}

#[derive(Deserialize, Default)]
struct LooseLink {
    #[serde(default)]
    kind: String,
}

fn has_waffle_link(project_root: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(project_root.join("design/links.toml")) else {
        return false;
    };
    toml::from_str::<LooseLinksFile>(&text)
        .map(|f| f.links.iter().any(|l| l.kind == "waffle"))
        .unwrap_or(false)
}

#[derive(Serialize, Deserialize, Default)]
struct SeenProjects {
    #[serde(default)]
    projects: BTreeSet<String>,
}

fn load_seen_projects(state_dir: &Path) -> BTreeSet<String> {
    let path = state_dir.join("terminals.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return BTreeSet::new();
    };
    if let Ok(parsed) = serde_json::from_str::<SeenProjects>(&text) {
        return parsed.projects;
    }
    serde_json::from_str::<BTreeSet<String>>(&text).unwrap_or_default()
}

fn record_seen_project(state_dir: &Path, project_id: &str) -> Result<(), String> {
    let mut projects = load_seen_projects(state_dir);
    if !projects.insert(project_id.to_string()) && state_dir.join("terminals.json").is_file() {
        return Ok(());
    }
    std::fs::create_dir_all(state_dir).map_err(|e| format!("{}: {e}", state_dir.display()))?;
    let path = state_dir.join("terminals.json");
    let tmp = state_dir.join(format!("terminals.json.tmp.{}", std::process::id()));
    let body = serde_json::to_vec_pretty(&SeenProjects { projects }).map_err(|e| e.to_string())?;
    std::fs::write(&tmp, body).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn build_system_prompt(project_name: &str, project_root: &Path) -> String {
    format!(
        "Project: {project_name}\nFolder: {}\nNote: this is Boardroom's project terminal; the pcb-design skill applies.",
        project_root.display()
    )
}

#[allow(clippy::too_many_arguments)]
fn spawn_terminal(
    options: &TerminalsOptions,
    cfg: &Config,
    project_root: &Path,
    req: &StartTermRequest,
    id: &str,
    cols: u16,
    rows: u16,
    boardroom_port: u16,
    waffle_holder: Arc<Mutex<Option<String>>>,
    bus: broadcast::Sender<String>,
) -> Result<Arc<Terminal>, TermError> {
    let proj_summary = project::summarize(&req.project, project_root, false);
    let title = proj_summary
        .board
        .clone()
        .unwrap_or_else(|| proj_summary.name.clone());

    let had_before = load_seen_projects(&options.state_dir).contains(&req.project);

    let waffle_setup = prepare_waffle_mcp(
        options,
        cfg,
        project_root,
        req,
        id,
        boardroom_port,
        &waffle_holder,
    )?;

    let rollback_waffle = || {
        if waffle_setup.claimed_holder {
            let mut h = waffle_holder.lock().unwrap();
            if h.as_deref() == Some(id) {
                *h = None;
            }
            cleanup_mcp_file(&options.cache_dir, id);
        }
    };

    let cmd =
        if let Some(override_argv) = options.term_command.as_ref().or(cfg.term_command.as_ref()) {
            let (prog, rest) = override_argv
                .split_first()
                .ok_or_else(|| TermError::BadRequest("empty term_command".into()))?;
            let mut c = CommandBuilder::new(prog);
            c.args(rest);
            c
        } else {
            let prog = cfg
                .claude
                .as_deref()
                .map(|p| p.as_os_str().to_os_string())
                .unwrap_or_else(|| "claude".into());
            let mut c = CommandBuilder::new(prog);
            if had_before {
                c.arg("--continue");
            }
            if let Some(mcp) = &waffle_setup.mcp_path {
                c.arg("--mcp-config");
                c.arg(mcp.as_os_str());
            }
            c.arg("--append-system-prompt");
            c.arg(build_system_prompt(&title, project_root));
            c
        };

    let mut cmd = cmd;
    cmd.cwd(project_root);
    cmd.env("TERM", "xterm-256color");
    cmd.env_remove("CLAUDECODE");

    let pty_system = native_pty_system();
    let pair = match pty_system.openpty(PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    }) {
        Ok(p) => p,
        Err(e) => {
            rollback_waffle();
            return Err(TermError::Internal(format!("openpty: {e}")));
        }
    };

    let mut child = match pair.slave.spawn_command(cmd) {
        Ok(c) => c,
        Err(e) => {
            rollback_waffle();
            return Err(TermError::Internal(format!("spawn terminal: {e}")));
        }
    };
    // Drop the slave side in the parent immediately so master read sees EOF when the child exits.
    drop(pair.slave);

    if let Err(e) = record_seen_project(&options.state_dir, &req.project) {
        eprintln!("boardroom: couldn't record terminal state: {e}");
    }

    let mut reader = match pair.master.try_clone_reader() {
        Ok(r) => r,
        Err(e) => {
            let _ = child.kill();
            rollback_waffle();
            return Err(TermError::Internal(format!("pty reader: {e}")));
        }
    };
    let mut writer = match pair.master.take_writer() {
        Ok(w) => w,
        Err(e) => {
            let _ = child.kill();
            rollback_waffle();
            return Err(TermError::Internal(format!("pty writer: {e}")));
        }
    };

    let killer = child.clone_killer();
    let (tx, _) = broadcast::channel(BROADCAST_CAPACITY);
    let mut scrollback = Scrollback::new();
    if let Some(banner) = &waffle_setup.banner {
        scrollback.push(banner.as_bytes());
    }

    let (input_tx, mut input_rx) = mpsc::channel::<Vec<u8>>(INPUT_QUEUE_CAPACITY);
    let started = now_iso8601();

    let term = Arc::new(Terminal {
        id: id.to_string(),
        project: req.project.clone(),
        kind: req.kind.clone(),
        started,
        waffle: waffle_setup.claimed_holder,
        inner: Mutex::new(TermInner {
            scrollback,
            alive: true,
            title,
            tx,
            osc_tail: Vec::new(),
        }),
        master: Mutex::new(Some(pair.master)),
        killer: Mutex::new(killer),
        input_tx: Mutex::new(Some(input_tx)),
    });

    // Dedicated writer thread so large pastes never block an async worker.
    std::thread::Builder::new()
        .name(format!("boardroom-pty-in-{id}"))
        .spawn(move || {
            use std::io::Write as _;
            while let Some(chunk) = input_rx.blocking_recv() {
                if writer.write_all(&chunk).is_err() || writer.flush().is_err() {
                    break;
                }
            }
        })
        .map_err(|e| TermError::Internal(format!("pty writer thread: {e}")))?;

    // Dedicated reader + reaper thread: drains all output into scrollback before marking not alive.
    let term_for_reader = term.clone();
    let id_for_reader = id.to_string();
    let project_for_reader = req.project.clone();
    std::thread::Builder::new()
        .name(format!("boardroom-pty-out-{id}"))
        .spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let bytes = Bytes::copy_from_slice(&buf[..n]);
                        let mut inner = term_for_reader.inner.lock().unwrap();
                        inner.scrollback.push(&bytes);
                        let end_offset = inner.scrollback.total_written;
                        update_osc_title(&mut inner, &bytes);
                        let _ = inner.tx.send(TermEvent::Chunk {
                            end_offset,
                            data: bytes,
                        });
                    }
                }
            }
            let _ = child.wait();
            *term_for_reader.input_tx.lock().unwrap() = None;
            *term_for_reader.master.lock().unwrap() = None;
            {
                let mut holder = waffle_holder.lock().unwrap();
                if holder.as_deref() == Some(id_for_reader.as_str()) {
                    *holder = None;
                }
            }
            {
                let mut inner = term_for_reader.inner.lock().unwrap();
                inner.alive = false;
                let _ = inner.tx.send(TermEvent::Exited);
            }
            let _ = bus.send(
                serde_json::json!({
                    "type": "term",
                    "project": project_for_reader,
                    "id": id_for_reader,
                    "alive": false,
                })
                .to_string(),
            );
        })
        .map_err(|e| TermError::Internal(format!("pty reader thread: {e}")))?;

    Ok(term)
}

/// Scans PTY output for OSC 0 / OSC 2 window title sequences (`ESC ] 0|2 ; title BEL|ST`).
fn update_osc_title(inner: &mut TermInner, chunk: &[u8]) {
    const MAX_OSC_BUF: usize = 512;
    if inner.osc_tail.is_empty() && !chunk.contains(&0x1b) {
        return;
    }
    let mut scan = std::mem::take(&mut inner.osc_tail);
    scan.extend_from_slice(chunk);

    let mut i = 0;
    while i < scan.len() {
        let Some(rel) = scan[i..].iter().position(|&b| b == 0x1b) else {
            break;
        };
        let start = i + rel;
        let rest = &scan[start..];
        if rest.len() < 4 {
            if rest.len() <= MAX_OSC_BUF {
                inner.osc_tail.extend_from_slice(rest);
            }
            return;
        }
        if rest[1] == b']' && (rest[2] == b'0' || rest[2] == b'2') && rest[3] == b';' {
            let body = &rest[4..];
            let mut end_body = None;
            let mut next_pos = None;
            for (j, &b) in body.iter().enumerate() {
                if b == 0x07 {
                    end_body = Some(j);
                    next_pos = Some(start + 4 + j + 1);
                    break;
                }
                if b == 0x1b && body.get(j + 1) == Some(&b'\\') {
                    end_body = Some(j);
                    next_pos = Some(start + 4 + j + 2);
                    break;
                }
            }
            if let (Some(eb), Some(np)) = (end_body, next_pos) {
                let raw = String::from_utf8_lossy(&body[..eb]);
                let cleaned: String = raw.chars().filter(|c| !c.is_control()).take(120).collect();
                let trimmed = cleaned.trim();
                if !trimmed.is_empty() {
                    inner.title = trimmed.to_string();
                }
                i = np;
                continue;
            }
            if rest.len() <= MAX_OSC_BUF {
                inner.osc_tail.extend_from_slice(rest);
            }
            return;
        }
        i = start + 1;
    }
}

fn now_iso8601() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}
