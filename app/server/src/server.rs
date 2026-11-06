//! The HTTP server: the web app, the project API, project files, and the bus.
//!
//! Every response carries the cross-origin isolation headers the suite's apps
//! need (docs/shell-spec.md, "One origin").

use crate::apps;
use crate::config::{self, ProjectList};
use crate::examples;
use crate::files::{self, Expect, WriteError};
use crate::links::{
    self, AddLinkRequest, LinkOpError, LinkState, LinkStatus, LinksCache, Published, PushError,
    PushResult,
};
use crate::problem;
use crate::project;
use crate::terminals::{StartTermRequest, TermError, TermSummary, Terminals, TerminalsOptions};
use axum::{
    Json, Router,
    body::Bytes,
    extract::{
        ConnectInfo, DefaultBodyLimit, Path as UrlPath, Query, Request, State,
        ws::{Message, WebSocket, WebSocketUpgrade, rejection::WebSocketUpgradeRejection},
    },
    http::HeaderMap,
    http::Method,
    http::{HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{any, delete, get, post},
};
use notify::{RecursiveMode, Watcher};
use serde::Deserialize;
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::broadcast;
use tower_http::{
    services::{ServeDir, ServeFile},
    set_header::SetResponseHeaderLayer,
};

type LinkFingerprint = (String, LinkState, String, Option<board_interface::Driver>);

pub struct App {
    pub(crate) config_dir: PathBuf,
    list: Mutex<ProjectList>,
    /// Bus messages, already JSON.
    bus: broadcast::Sender<String>,
    watcher: Mutex<Option<notify::RecommendedWatcher>>,
    pub(crate) links_cache: LinksCache,
    link_states: Mutex<HashMap<String, Vec<LinkFingerprint>>>,
    port: AtomicU16,
    pub(crate) terminals: Terminals,
    selections: Mutex<HashMap<String, serde_json::Value>>,
}

impl App {
    pub fn set_port(&self, port: u16) {
        self.port.store(port, Ordering::Release);
    }

    pub fn port(&self) -> u16 {
        self.port.load(Ordering::Acquire)
    }

    pub fn shutdown_terminals(&self) {
        self.terminals.shutdown();
    }

    fn effective_port(&self, headers: &HeaderMap) -> Option<u16> {
        let bound = self.port();
        if bound != 0 {
            return Some(bound);
        }
        let host = headers.get(header::HOST)?.to_str().ok()?.trim();
        let (h, p) = host.rsplit_once(':')?;
        if matches!(h, "127.0.0.1" | "localhost") {
            p.parse().ok().filter(|&n| n != 0)
        } else {
            None
        }
    }
}

pub type Shared = Arc<App>;
type ApiResult<T> = Result<T, (StatusCode, String)>;

fn bad(status: StatusCode, msg: impl Into<String>) -> (StatusCode, String) {
    (status, msg.into())
}

fn bad_link_op(err: LinkOpError) -> (StatusCode, String) {
    match err {
        LinkOpError::NotFound(m) => (StatusCode::NOT_FOUND, m),
        LinkOpError::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
        LinkOpError::Io(m) => (StatusCode::INTERNAL_SERVER_ERROR, m),
    }
}

fn bad_push_op(err: PushError) -> (StatusCode, String) {
    match err {
        PushError::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
        PushError::Failed(stderr) => (StatusCode::CONFLICT, stderr),
        PushError::Io(m) => (StatusCode::INTERNAL_SERVER_ERROR, m),
    }
}

fn bad_term_op(err: TermError) -> (StatusCode, String) {
    match err {
        TermError::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
        TermError::NotFound(m) => (StatusCode::NOT_FOUND, m),
        TermError::Internal(m) => (StatusCode::INTERNAL_SERVER_ERROR, m),
    }
}

pub fn router(config_dir: PathBuf, list: ProjectList, web: PathBuf) -> (Router, Shared) {
    router_with_cache(config_dir, list, web, LinksCache::new())
}

pub fn router_with_cache(
    config_dir: PathBuf,
    list: ProjectList,
    web: PathBuf,
    links_cache: LinksCache,
) -> (Router, Shared) {
    router_with_options(
        config_dir,
        list,
        web,
        links_cache,
        TerminalsOptions::default(),
    )
}

pub fn router_with_options(
    config_dir: PathBuf,
    list: ProjectList,
    web: PathBuf,
    links_cache: LinksCache,
    term_options: TerminalsOptions,
) -> (Router, Shared) {
    let (bus, _) = broadcast::channel(256);
    let app = Arc::new(App {
        config_dir,
        list: Mutex::new(list),
        bus,
        watcher: Mutex::new(None),
        links_cache,
        link_states: Mutex::new(HashMap::new()),
        port: AtomicU16::new(0),
        terminals: Terminals::new(term_options),
        selections: Mutex::new(HashMap::new()),
    });
    let index = ServeFile::new(web.join("index.html"));
    let router = Router::new()
        .route("/api/projects", get(projects).post(add_project))
        .route("/api/projects/{id}", delete(remove_project))
        .route("/api/examples", get(list_examples))
        .route("/api/projects/{id}/docs", get(docs))
        // A board file is a few megabytes; the default 2 MB limit is too small for some.
        .route(
            "/api/projects/{id}/files/{*path}",
            get(file)
                .put(put_file)
                .layer(DefaultBodyLimit::max(64 << 20)),
        )
        .route("/api/projects/{id}/history", get(history))
        .route(
            "/api/projects/{id}/links",
            get(project_links).post(add_project_link),
        )
        .route(
            "/api/projects/{id}/links/{link}",
            delete(delete_project_link),
        )
        .route(
            "/api/projects/{id}/links/{link}/update",
            post(update_project_link),
        )
        .route(
            "/api/projects/{id}/links/{link}/pin",
            post(pin_project_link),
        )
        .route(
            "/api/projects/{id}/links/{link}/follow",
            post(follow_project_link),
        )
        .route(
            "/api/projects/{id}/links/{link}/rehost",
            post(rehost_project_link),
        )
        .route("/api/projects/{id}/published", get(project_published))
        .route("/api/projects/{id}/push", post(push_project))
        .route("/api/projects/{id}/open/{what}", post(open))
        .route("/api/pick-folder", post(pick_folder))
        .route("/api/term", get(list_terms).post(start_term))
        .route("/api/term/{id}", get(term_socket).delete(delete_term))
        .route("/api/selection", get(get_selection))
        .route("/api/bus", get(bus_socket))
        // An unknown API path is a 404, not the web app's page with a 200.
        .route(
            "/api/{*rest}",
            any(|| async { (StatusCode::NOT_FOUND, "no such API") }),
        )
        .route("/apps/{name}", get(apps::slash))
        .route(
            "/apps/{name}/",
            any(|state, peer, UrlPath(name): UrlPath<String>, req| {
                apps::serve(state, peer, UrlPath((name, String::new())), req)
            }),
        )
        .route("/apps/{name}/{*rest}", any(apps::serve))
        .fallback_service(ServeDir::new(&web).fallback(index))
        .layer(SetResponseHeaderLayer::overriding(
            header::HeaderName::from_static("cross-origin-opener-policy"),
            HeaderValue::from_static("same-origin"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::HeaderName::from_static("cross-origin-embedder-policy"),
            HeaderValue::from_static("require-corp"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::HeaderName::from_static("cross-origin-resource-policy"),
            HeaderValue::from_static("same-origin"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-cache"),
        ))
        .layer(middleware::from_fn(local_changes_only))
        .with_state(app.clone());
    (router, app)
}

pub async fn run(host: &str, port: u16, web: PathBuf) -> Result<(), String> {
    let config_dir = config::config_dir();
    let (router, app) = router(
        config_dir.clone(),
        ProjectList::load_from(&config_dir)?,
        web,
    );
    start_watcher(&app);

    let listener = tokio::net::TcpListener::bind((host, port))
        .await
        .map_err(|e| format!("{host}:{port}: {e}"))?;
    let actual_port = listener.local_addr().map(|a| a.port()).unwrap_or(port);
    app.set_port(actual_port);
    println!("Boardroom: http://localhost:{actual_port}/");
    let _ = std::io::Write::flush(&mut std::io::stdout());
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await
    .map_err(|e| e.to_string())
}

/// Other devices may look but not act: anything but a read (adding projects,
/// opening KiCad or a folder, the folder dialog) must come from this machine.
/// Reads stay inside the listed projects, so a reachable port shows what
/// Boardroom shows and nothing more.
async fn local_changes_only(
    ConnectInfo(peer): ConnectInfo<std::net::SocketAddr>,
    req: Request,
    next: Next,
) -> Response {
    let reading = matches!(*req.method(), Method::GET | Method::HEAD | Method::OPTIONS);
    if !reading && !peer.ip().is_loopback() {
        return (
            StatusCode::FORBIDDEN,
            "Boardroom takes changes only from the machine it runs on",
        )
            .into_response();
    }
    next.run(req).await
}

/// Looks a project up by id; the path is always one on the list.
fn project_path(app: &App, id: &str) -> ApiResult<PathBuf> {
    let list = app.list.lock().unwrap();
    list.projects
        .iter()
        .find(|p| config::project_id(&p.path) == id)
        .map(|p| p.path.clone())
        .ok_or_else(|| bad(StatusCode::NOT_FOUND, "no such project"))
}

/// Other registered project paths on this machine (excluding `exclude`).
fn candidate_paths(app: &App, exclude: &Path) -> Vec<PathBuf> {
    let list = app.list.lock().unwrap();
    list.projects
        .iter()
        .filter(|p| p.path != exclude)
        .map(|p| p.path.clone())
        .collect()
}

async fn projects(
    State(app): State<Shared>,
    ConnectInfo(peer): ConnectInfo<std::net::SocketAddr>,
) -> impl IntoResponse {
    let may_write = peer.ip().is_loopback();
    let entries = app.list.lock().unwrap().projects.clone();
    let all_paths: Vec<PathBuf> = entries.iter().map(|e| e.path.clone()).collect();
    // Evaluate links first when `design/links.toml` exists so `.boardroom/links/` is
    // materialized and quiet `NewRevisionNoInterfaceChange` commit advances land before
    // `project::summarize` checks gate input records and Gate 10's release commits.
    let mut preloaded: HashMap<PathBuf, Vec<LinkStatus>> = HashMap::new();
    for e in &entries {
        if e.path.is_dir() && e.path.join("design/links.toml").is_file() {
            let cands: Vec<PathBuf> = all_paths
                .iter()
                .filter(|p| *p != &e.path)
                .cloned()
                .collect();
            if let Ok(statuses) =
                links::list_links(&e.path, &app.links_cache, false, may_write, &cands).await
            {
                note_link_states(&app, &config::project_id(&e.path), &statuses, false);
                preloaded.insert(e.path.clone(), statuses);
            }
        }
    }

    let mut summaries = tokio::task::spawn_blocking(move || {
        let mut out: Vec<_> = entries
            .iter()
            .map(|e| project::summarize(&config::project_id(&e.path), &e.path, e.pinned))
            .collect();
        examples::tag(
            &mut out,
            &examples::load(&crate::repo_dir()),
            &crate::repo_dir(),
        );
        out
    })
    .await
    .unwrap_or_default();

    for s in &mut summaries {
        if let Some(statuses) = preloaded.get(&s.path) {
            for need in links::needs_from_statuses(statuses) {
                if !s.needs_user.iter().any(|n| n.id == need.id) {
                    s.needs_user.push(need);
                }
            }
        }
    }

    Json(summaries)
}

/// The examples, each with the id of your copy of it if one is on the list.
async fn list_examples(State(app): State<Shared>) -> impl IntoResponse {
    let entries = app.list.lock().unwrap().projects.clone();
    let found = tokio::task::spawn_blocking(move || {
        let repo = crate::repo_dir();
        let list = examples::load(&repo);
        let summaries: Vec<_> = entries
            .iter()
            .map(|e| project::summarize(&config::project_id(&e.path), &e.path, e.pinned))
            .collect();
        list.into_iter()
            .map(|ex| {
                let mine = summaries
                    .iter()
                    .find(|s| ex.matches(&repo, &s.path, s.git.as_ref().and_then(|g| g.remote.as_deref())))
                    .map(|s| s.id.clone());
                let in_repo = ex.local_path(&repo).map(|p| p.display().to_string());
                serde_json::json!({"name": ex.name, "repo": ex.repo, "ref": ex.git_ref, "in_repo": in_repo, "project": mine})
            })
            .collect::<Vec<_>>()
    })
    .await
    .unwrap_or_default();
    Json(found)
}

#[derive(Deserialize)]
struct AddBody {
    path: String,
}

async fn add_project(
    State(app): State<Shared>,
    Json(body): Json<AddBody>,
) -> ApiResult<Json<serde_json::Value>> {
    let raw = body.path.trim();
    let expanded = match raw.strip_prefix("~/") {
        Some(rest) => config::home().join(rest),
        None => PathBuf::from(raw),
    };
    let path = {
        let mut list = app.list.lock().unwrap();
        let path = list
            .add(&expanded)
            .map_err(|e| bad(StatusCode::BAD_REQUEST, e))?;
        list.save_to(&app.config_dir)
            .map_err(|e| bad(StatusCode::INTERNAL_SERVER_ERROR, e))?;
        path
    };
    watch(&app, &path);
    let _ = app
        .bus
        .send(serde_json::json!({"type": "projects"}).to_string());
    Ok(Json(serde_json::json!({"id": config::project_id(&path)})))
}

async fn remove_project(
    State(app): State<Shared>,
    UrlPath(id): UrlPath<String>,
) -> ApiResult<StatusCode> {
    let path = project_path(&app, &id)?;
    {
        let mut list = app.list.lock().unwrap();
        list.remove(&path);
        list.save_to(&app.config_dir)
            .map_err(|e| bad(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    }
    if let Some(w) = app.watcher.lock().unwrap().as_mut() {
        for sub in ["design", "hw", "interface"] {
            let _ = w.unwatch(&path.join(sub));
        }
    }
    let _ = app
        .bus
        .send(serde_json::json!({"type": "projects"}).to_string());
    Ok(StatusCode::NO_CONTENT)
}

/// The project's markdown documents: design/*.md and design/gates/*.md.
async fn docs(
    State(app): State<Shared>,
    UrlPath(id): UrlPath<String>,
) -> ApiResult<Json<Vec<String>>> {
    let root = project_path(&app, &id)?;
    let mut out = vec![];
    for dir in ["design", "design/gates"] {
        let Ok(entries) = std::fs::read_dir(root.join(dir)) else {
            continue;
        };
        let mut names: Vec<String> = entries
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "md"))
            .map(|e| format!("{dir}/{}", e.file_name().to_string_lossy()))
            .collect();
        names.sort_by_key(|a| natural(a));
        out.extend(names);
    }
    Ok(Json(out))
}

/// Sorts phase2 before phase10.
fn natural(s: &str) -> (String, u64, String) {
    let digits_at = s.find(|c: char| c.is_ascii_digit()).unwrap_or(s.len());
    let (head, tail) = s.split_at(digits_at);
    let end = tail
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(tail.len());
    (
        head.to_string(),
        tail[..end].parse().unwrap_or(0),
        tail[end..].to_string(),
    )
}

/// A file inside a project folder, or a directory listing as JSON.
async fn file(
    State(app): State<Shared>,
    UrlPath((id, rel)): UrlPath<(String, String)>,
) -> ApiResult<Response> {
    let root = project_path(&app, &id)?;
    // Composer's placement problem, when the project has none of its own: made from the board.
    if let Some(board) = problem::board_of(&rel).map(str::to_string)
        && !root.join(&rel).exists()
    {
        return made_problem(root, board).await;
    }
    let path = inside(&root, &rel)?;
    if path.is_dir() {
        let mut names: Vec<serde_json::Value> = std::fs::read_dir(&path)
            .map_err(|e| bad(StatusCode::NOT_FOUND, e.to_string()))?
            .flatten()
            .map(|e| serde_json::json!({"name": e.file_name().to_string_lossy(), "dir": e.path().is_dir()}))
            .collect();
        names.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
        return Ok(Json(names).into_response());
    }
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|e| bad(StatusCode::NOT_FOUND, e.to_string()))?;
    let etag = format!("\"{}\"", files::rev_of(&bytes));
    // When the file last changed: Boardroom's drawn tabs compare the board's against when they were drawn.
    let modified = std::fs::metadata(&path)
        .and_then(|m| m.modified())
        .map(httpdate::fmt_http_date)
        .unwrap_or_default();
    Ok((
        [
            (header::CONTENT_TYPE, mime(&path).to_string()),
            (header::ETAG, etag),
            (header::LAST_MODIFIED, modified),
        ],
        bytes,
    )
        .into_response())
}

/// A board's placement problem made by ccomp (problem.rs), served as if it were the file.
async fn made_problem(root: PathBuf, board: String) -> ApiResult<Response> {
    let ccomp = problem::ccomp().ok_or_else(|| {
        bad(StatusCode::NOT_FOUND, "Boardroom makes Composer's placement problem with ccomp: set CCOMP, or ccomp = \"…\" in ~/.config/boardroom/config.toml")
    })?;
    let bytes = tokio::task::spawn_blocking(move || problem::make(&root, &board, &ccomp))
        .await
        .map_err(|e| bad(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(|e| bad(StatusCode::NOT_FOUND, e))?;
    let etag = format!("\"{}\"", files::rev_of(&bytes));
    Ok((
        [
            (header::CONTENT_TYPE, "application/json".to_string()),
            (header::ETAG, etag),
        ],
        bytes,
    )
        .into_response())
}

/// Writes a project file for an app (the store's server backend). `If-Match` names the revision
/// the app started from and `If-None-Match: *` asks that there be none; a file that moved on is
/// 412, with its revision now as the ETag. The bus then says which file changed and which app
/// changed it (`x-boardroom-app`, `x-boardroom-message`).
async fn put_file(
    State(app): State<Shared>,
    UrlPath((id, rel)): UrlPath<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> ApiResult<Response> {
    let root = project_path(&app, &id)?;
    let header = |name: &str| {
        headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)
    };
    let expect = match (header("if-match"), header("if-none-match")) {
        (Some(rev), _) => Expect::Rev(
            rev.trim()
                .trim_start_matches("W/")
                .trim_matches('"')
                .to_string(),
        ),
        (None, Some(star)) if star.trim() == "*" => Expect::Absent,
        _ => Expect::Any,
    };
    let (root2, rel2) = (root.clone(), rel.clone());
    let written = tokio::task::spawn_blocking(move || files::write(&root2, &rel2, &body, &expect))
        .await
        .map_err(|e| bad(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    match written {
        Ok(rev) => {
            let who = header("x-boardroom-app").unwrap_or_else(|| "unknown".into());
            let message = header("x-boardroom-message")
                .map(|m| percent_decode(&m))
                .unwrap_or_default();
            let _ = app.bus.send(serde_json::json!({"type": "changed", "project": id, "paths": [rel], "app": who, "message": message}).to_string());
            Ok((
                [(header::ETAG, format!("\"{rev}\""))],
                Json(serde_json::json!({"rev": rev})),
            )
                .into_response())
        }
        Err(WriteError::Conflict { current }) => {
            let mut res = (
                StatusCode::PRECONDITION_FAILED,
                format!("{rel} changed since you opened it"),
            )
                .into_response();
            if let Some(rev) = current.and_then(|r| HeaderValue::from_str(&format!("\"{r}\"")).ok())
            {
                res.headers_mut().insert(header::ETAG, rev);
            }
            Ok(res)
        }
        Err(WriteError::Refused(why)) => Err(bad(StatusCode::FORBIDDEN, why)),
        Err(WriteError::Io(why)) => Err(bad(StatusCode::INTERNAL_SERVER_ERROR, why)),
    }
}

#[derive(Deserialize)]
struct HistoryQuery {
    path: Option<String>,
}

/// The project's history, or one file's (`?path=`): git's, newest first.
async fn history(
    State(app): State<Shared>,
    UrlPath(id): UrlPath<String>,
    Query(q): Query<HistoryQuery>,
) -> ApiResult<Json<Vec<serde_json::Value>>> {
    let root = project_path(&app, &id)?;
    let list = tokio::task::spawn_blocking(move || files::history(&root, q.path.as_deref()))
        .await
        .map_err(|e| bad(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(list))
}

#[derive(Deserialize, Default)]
struct LinksQuery {
    refresh: Option<String>,
}

#[derive(Deserialize, Default)]
struct PinBody {
    #[serde(default)]
    reason: String,
}

#[derive(Deserialize)]
struct FollowBody {
    branch: String,
}

fn note_link_states(app: &App, project_id: &str, statuses: &[LinkStatus], force_emit: bool) {
    let fp: Vec<LinkFingerprint> = statuses
        .iter()
        .map(|s| {
            (
                s.id.clone(),
                s.state,
                s.resolved.commit.clone(),
                s.locked_by,
            )
        })
        .collect();
    let advanced = statuses
        .iter()
        .any(|s| s.state == LinkState::NewRevisionNoInterfaceChange);
    let changed = {
        let mut map = app.link_states.lock().unwrap();
        let prev = map.insert(project_id.to_string(), fp.clone());
        force_emit || advanced || prev.is_some_and(|old| old != fp)
    };
    if changed {
        let _ = app
            .bus
            .send(serde_json::json!({"type": "links", "project": project_id}).to_string());
    }
}

async fn refresh_after_link_write(
    app: &App,
    project_id: &str,
    root: &Path,
    fallback: LinkStatus,
) -> LinkStatus {
    let cands = candidate_paths(app, root);
    if let Ok(all) = links::list_links(root, &app.links_cache, false, true, &cands).await {
        note_link_states(app, project_id, &all, true);
        if let Some(found) = all.into_iter().find(|s| s.id == fallback.id) {
            return found;
        }
    } else {
        let _ = app
            .bus
            .send(serde_json::json!({"type": "links", "project": project_id}).to_string());
    }
    fallback
}

async fn project_links(
    State(app): State<Shared>,
    ConnectInfo(peer): ConnectInfo<std::net::SocketAddr>,
    UrlPath(id): UrlPath<String>,
    Query(q): Query<LinksQuery>,
) -> ApiResult<Json<Vec<LinkStatus>>> {
    let root = project_path(&app, &id)?;
    let cands = candidate_paths(&app, &root);
    let force = q
        .refresh
        .as_deref()
        .is_some_and(|v| matches!(v, "1" | "true" | "yes"));
    let statuses = links::list_links(
        &root,
        &app.links_cache,
        force,
        peer.ip().is_loopback(),
        &cands,
    )
    .await
    .map_err(|e| bad(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    note_link_states(&app, &id, &statuses, false);
    Ok(Json(statuses))
}

async fn add_project_link(
    State(app): State<Shared>,
    UrlPath(id): UrlPath<String>,
    Json(body): Json<AddLinkRequest>,
) -> ApiResult<Json<LinkStatus>> {
    let root = project_path(&app, &id)?;
    let status = links::add_link(&root, body, &app.links_cache)
        .await
        .map_err(bad_link_op)?;
    Ok(Json(
        refresh_after_link_write(&app, &id, &root, status).await,
    ))
}

async fn update_project_link(
    State(app): State<Shared>,
    UrlPath((id, link)): UrlPath<(String, String)>,
) -> ApiResult<Json<LinkStatus>> {
    let root = project_path(&app, &id)?;
    let status = links::update_link(&root, &link, &app.links_cache)
        .await
        .map_err(bad_link_op)?;
    Ok(Json(
        refresh_after_link_write(&app, &id, &root, status).await,
    ))
}

async fn pin_project_link(
    State(app): State<Shared>,
    UrlPath((id, link)): UrlPath<(String, String)>,
    body: Bytes,
) -> ApiResult<Json<LinkStatus>> {
    let root = project_path(&app, &id)?;
    let parsed: PinBody = if body.is_empty() {
        PinBody::default()
    } else {
        serde_json::from_slice(&body)
            .map_err(|e| bad(StatusCode::BAD_REQUEST, format!("invalid pin JSON: {e}")))?
    };
    let status = links::pin_link(&root, &link, parsed.reason, &app.links_cache)
        .await
        .map_err(bad_link_op)?;
    Ok(Json(
        refresh_after_link_write(&app, &id, &root, status).await,
    ))
}

async fn follow_project_link(
    State(app): State<Shared>,
    UrlPath((id, link)): UrlPath<(String, String)>,
    Json(body): Json<FollowBody>,
) -> ApiResult<Json<LinkStatus>> {
    let root = project_path(&app, &id)?;
    let status = links::follow_link(&root, &link, body.branch, &app.links_cache)
        .await
        .map_err(bad_link_op)?;
    Ok(Json(
        refresh_after_link_write(&app, &id, &root, status).await,
    ))
}

async fn rehost_project_link(
    State(app): State<Shared>,
    UrlPath((id, link)): UrlPath<(String, String)>,
) -> ApiResult<Json<LinkStatus>> {
    let root = project_path(&app, &id)?;
    let status = links::rehost_link(&root, &link, &app.links_cache)
        .await
        .map_err(bad_link_op)?;
    Ok(Json(
        refresh_after_link_write(&app, &id, &root, status).await,
    ))
}

async fn delete_project_link(
    State(app): State<Shared>,
    UrlPath((id, link)): UrlPath<(String, String)>,
) -> ApiResult<StatusCode> {
    let root = project_path(&app, &id)?;
    let cands = candidate_paths(&app, &root);
    let root2 = root.clone();
    tokio::task::spawn_blocking(move || links::remove_link(&root2, &link))
        .await
        .map_err(|e| bad(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map_err(bad_link_op)?;
    if let Ok(all) = links::list_links(&root, &app.links_cache, false, true, &cands).await {
        note_link_states(&app, &id, &all, true);
    } else {
        let _ = app
            .bus
            .send(serde_json::json!({"type": "links", "project": id}).to_string());
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn project_published(
    State(app): State<Shared>,
    UrlPath(id): UrlPath<String>,
) -> ApiResult<Json<Vec<Published>>> {
    let root = project_path(&app, &id)?;
    let items = tokio::task::spawn_blocking(move || links::published_status(&root))
        .await
        .map_err(|e| bad(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(items))
}

async fn push_project(
    State(app): State<Shared>,
    ConnectInfo(peer): ConnectInfo<std::net::SocketAddr>,
    UrlPath(id): UrlPath<String>,
) -> ApiResult<Json<PushResult>> {
    if !peer.ip().is_loopback() {
        return Err(bad(
            StatusCode::FORBIDDEN,
            "Boardroom takes changes only from the machine it runs on",
        ));
    }
    let root = project_path(&app, &id)?;
    let res = links::push_upstream(&root).await.map_err(bad_push_op)?;
    let _ = app
        .bus
        .send(serde_json::json!({"type": "links", "project": id}).to_string());
    Ok(Json(res))
}

/// `%XX` escapes undone (the store sends a message so, as headers are ASCII).
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Some(v) = std::str::from_utf8(&b[i + 1..i + 3])
                .ok()
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Resolves `rel` under `root`, refusing anything that leaves the folder.
fn inside(root: &Path, rel: &str) -> ApiResult<PathBuf> {
    let root =
        std::fs::canonicalize(root).map_err(|e| bad(StatusCode::NOT_FOUND, e.to_string()))?;
    let path = std::fs::canonicalize(root.join(rel))
        .map_err(|_| bad(StatusCode::NOT_FOUND, "not found"))?;
    if !path.starts_with(&root) {
        return Err(bad(StatusCode::FORBIDDEN, "outside the project"));
    }
    Ok(path)
}

fn mime(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "pdf" => "application/pdf",
        "json" => "application/json",
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript",
        "css" => "text/css",
        "glb" => "model/gltf-binary",
        "md" | "csv" | "toml" | "txt" | "dxf" | "kicad_sch" | "kicad_pcb" | "kicad_dru" | "net" => {
            "text/plain; charset=utf-8"
        }
        _ => "application/octet-stream",
    }
}

/// Opens something for the person, on this machine's desktop.
async fn open(
    State(app): State<Shared>,
    UrlPath((id, what)): UrlPath<(String, String)>,
) -> ApiResult<StatusCode> {
    let root = project_path(&app, &id)?;
    let (program, arg) = match what.as_str() {
        "folder" => ("xdg-open", root.clone()),
        "kicad" => {
            let pro = project::summarize(&id, &root, false)
                .boards
                .into_iter()
                .find_map(|b| b.kicad_pro)
                .ok_or_else(|| bad(StatusCode::NOT_FOUND, "no KiCad project under hw/"))?;
            ("kicad", root.join(pro))
        }
        _ => return Err(bad(StatusCode::BAD_REQUEST, format!("can't open '{what}'"))),
    };
    let mut child = tokio::process::Command::new(program)
        .arg(&arg)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| bad(StatusCode::INTERNAL_SERVER_ERROR, format!("{program}: {e}")))?;
    tokio::spawn(async move {
        let _ = child.wait().await;
    });
    Ok(StatusCode::NO_CONTENT)
}

/// The desktop's own folder dialog; a page can't learn a real path itself.
async fn pick_folder() -> ApiResult<Json<serde_json::Value>> {
    let out = tokio::process::Command::new("zenity")
        .args([
            "--file-selection",
            "--directory",
            "--title=Add a board project",
        ])
        .output()
        .await
        .map_err(|e| {
            bad(
                StatusCode::NOT_IMPLEMENTED,
                format!("no folder dialog (zenity: {e}); paste the path instead"),
            )
        })?;
    let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok(Json(
        serde_json::json!({"path": if out.status.success() && !path.is_empty() { Some(path) } else { None }}),
    ))
}

/// Security gate for `/api/term` and `/api/selection`: loopback peers only, and
/// any `Origin` header present must be this server's own origin
/// (`http://127.0.0.1:<port>` or `http://localhost:<port>`).
fn check_term_access(app: &App, peer: std::net::SocketAddr, headers: &HeaderMap) -> ApiResult<()> {
    if !peer.ip().is_loopback() {
        return Err(bad(
            StatusCode::FORBIDDEN,
            "terminals are available only from loopback",
        ));
    }
    let origins: Vec<_> = headers.get_all(header::ORIGIN).iter().collect();
    if origins.is_empty() {
        return Ok(());
    }
    if origins.len() > 1 {
        return Err(bad(StatusCode::FORBIDDEN, "multiple Origin headers"));
    }
    let raw = origins[0]
        .to_str()
        .map_err(|_| bad(StatusCode::FORBIDDEN, "invalid Origin header"))?;
    let Some(port) = app.effective_port(headers) else {
        return Err(bad(StatusCode::FORBIDDEN, "unknown bound server port"));
    };
    let ok = raw == format!("http://127.0.0.1:{port}") || raw == format!("http://localhost:{port}");
    if !ok {
        return Err(bad(
            StatusCode::FORBIDDEN,
            format!("Origin '{raw}' is not this Boardroom server"),
        ));
    }
    Ok(())
}

async fn list_terms(
    State(app): State<Shared>,
    ConnectInfo(peer): ConnectInfo<std::net::SocketAddr>,
    headers: HeaderMap,
) -> ApiResult<Json<Vec<TermSummary>>> {
    check_term_access(&app, peer, &headers)?;
    Ok(Json(app.terminals.list()))
}

async fn start_term(
    State(app): State<Shared>,
    ConnectInfo(peer): ConnectInfo<std::net::SocketAddr>,
    headers: HeaderMap,
    Json(body): Json<StartTermRequest>,
) -> ApiResult<Json<TermSummary>> {
    check_term_access(&app, peer, &headers)?;
    let root = project_path(&app, &body.project)?;
    let port = app.effective_port(&headers).unwrap_or(0);
    let summary = app
        .terminals
        .start_or_get(&app.config_dir, &root, body, port, &app.bus)
        .await
        .map_err(bad_term_op)?;
    Ok(Json(summary))
}

async fn delete_term(
    State(app): State<Shared>,
    ConnectInfo(peer): ConnectInfo<std::net::SocketAddr>,
    UrlPath(id): UrlPath<String>,
    headers: HeaderMap,
) -> ApiResult<StatusCode> {
    check_term_access(&app, peer, &headers)?;
    app.terminals.delete(&id, &app.bus).map_err(bad_term_op)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn term_socket(
    State(app): State<Shared>,
    ConnectInfo(peer): ConnectInfo<std::net::SocketAddr>,
    UrlPath(id): UrlPath<String>,
    headers: HeaderMap,
    ws: Result<WebSocketUpgrade, WebSocketUpgradeRejection>,
) -> Response {
    if let Err(err) = check_term_access(&app, peer, &headers) {
        return err.into_response();
    }
    if !app.terminals.exists(&id) {
        return bad(StatusCode::NOT_FOUND, "no such terminal").into_response();
    }
    let ws = match ws {
        Ok(u) => u,
        Err(rej) => return rej.into_response(),
    };
    ws.on_upgrade(move |socket| async move {
        app.terminals.attach(&id, socket).await;
    })
}

#[derive(Deserialize)]
struct SelectionQuery {
    project: String,
}

async fn get_selection(
    State(app): State<Shared>,
    ConnectInfo(peer): ConnectInfo<std::net::SocketAddr>,
    headers: HeaderMap,
    Query(q): Query<SelectionQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    check_term_access(&app, peer, &headers)?;
    let _ = project_path(&app, &q.project)?;
    let val = app
        .selections
        .lock()
        .unwrap()
        .get(&q.project)
        .cloned()
        .unwrap_or_else(|| {
            serde_json::json!({
                "type": "select",
                "project": q.project,
                "board": serde_json::Value::Null,
                "refs": [],
                "nets": [],
            })
        });
    Ok(Json(val))
}

fn note_bus_selection(app: &App, text: &str) {
    let Ok(mut val) = serde_json::from_str::<serde_json::Value>(text) else {
        return;
    };
    if val.get("type").and_then(|t| t.as_str()) != Some("select") {
        return;
    }
    let Some(project_id) = val
        .get("project")
        .and_then(|p| p.as_str())
        .filter(|p| !p.is_empty())
        .map(str::to_string)
    else {
        return;
    };
    if let Some(obj) = val.as_object_mut() {
        obj.entry("board").or_insert(serde_json::Value::Null);
        obj.entry("refs").or_insert_with(|| serde_json::json!([]));
        obj.entry("nets").or_insert_with(|| serde_json::json!([]));
    }
    app.selections.lock().unwrap().insert(project_id, val);
}

async fn bus_socket(
    State(app): State<Shared>,
    ConnectInfo(peer): ConnectInfo<std::net::SocketAddr>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    let is_local = peer.ip().is_loopback();
    ws.on_upgrade(move |socket| bus_client(socket, app, is_local))
}

async fn bus_client(mut socket: WebSocket, app: Shared, is_local: bool) {
    let mut rx = app.bus.subscribe();
    loop {
        tokio::select! {
            msg = rx.recv() => match msg {
                Ok(text) => if socket.send(Message::Text(text.into())).await.is_err() { break },
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => break,
            },
            incoming = socket.recv() => match incoming {
                // Clients may publish to the bus (selection, focus): relay it.
                Some(Ok(Message::Text(text))) => {
                    if is_local {
                        note_bus_selection(&app, &text);
                    }
                    let _ = app.bus.send(text.to_string());
                }
                Some(Ok(_)) => {}
                _ => break,
            },
        }
    }
}

/// Watches each project's design/ folder and tells the bus which projects
/// changed, at most every 300 ms per project.
fn start_watcher(app: &Shared) {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<PathBuf>();
    let watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        if let Ok(event) = event {
            for path in event.paths {
                let _ = tx.send(path);
            }
        }
    });
    match watcher {
        Ok(w) => *app.watcher.lock().unwrap() = Some(w),
        Err(e) => {
            eprintln!("boardroom: file watching is off ({e}); pages refresh only when reloaded");
            return;
        }
    }
    let roots: Vec<PathBuf> = app
        .list
        .lock()
        .unwrap()
        .projects
        .iter()
        .map(|p| p.path.clone())
        .collect();
    for root in roots {
        watch(app, &root);
    }
    let app = app.clone();
    tokio::spawn(async move {
        loop {
            let Some(first) = rx.recv().await else { return };
            let mut changed = vec![first];
            tokio::time::sleep(Duration::from_millis(300)).await;
            while let Ok(p) = rx.try_recv() {
                changed.push(p);
            }
            let projects: Vec<PathBuf> = app
                .list
                .lock()
                .unwrap()
                .projects
                .iter()
                .map(|p| p.path.clone())
                .collect();
            let ids: BTreeSet<String> = changed
                .iter()
                .filter_map(|c| projects.iter().find(|root| c.starts_with(root)))
                .map(|root| config::project_id(root))
                .collect();
            for id in ids {
                let _ = app
                    .bus
                    .send(serde_json::json!({"type": "status", "project": id}).to_string());
            }
        }
    });
}

fn watch(app: &App, root: &Path) {
    if let Some(w) = app.watcher.lock().unwrap().as_mut() {
        for sub in ["design", "hw", "interface"] {
            let dir = root.join(sub);
            if dir.is_dir()
                && let Err(e) = w.watch(&dir, RecursiveMode::Recursive)
            {
                eprintln!("boardroom: can't watch {}: {e}", dir.display());
            }
        }
    }
}
