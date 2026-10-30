//! The HTTP server: the web app, the project API, project files, and the bus.
//!
//! Every response carries the cross-origin isolation headers the suite's apps
//! need (docs/shell-spec.md, "One origin").

use crate::apps;
use crate::config::{self, ProjectList};
use crate::examples;
use crate::project;
use axum::{
    Json, Router,
    extract::{Path as UrlPath, State, ws::{Message, WebSocket, WebSocketUpgrade}},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use notify::{RecursiveMode, Watcher};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::broadcast;
use tower_http::{services::{ServeDir, ServeFile}, set_header::SetResponseHeaderLayer};

struct App {
    list: Mutex<ProjectList>,
    /// Bus messages, already JSON.
    bus: broadcast::Sender<String>,
    watcher: Mutex<Option<notify::RecommendedWatcher>>,
}

type Shared = Arc<App>;
type ApiResult<T> = Result<T, (StatusCode, String)>;

fn bad(status: StatusCode, msg: impl Into<String>) -> (StatusCode, String) {
    (status, msg.into())
}

pub async fn run(host: &str, port: u16, web: PathBuf) -> Result<(), String> {
    let (bus, _) = broadcast::channel(256);
    let app = Arc::new(App { list: Mutex::new(ProjectList::load()?), bus, watcher: Mutex::new(None) });
    start_watcher(&app);

    let index = ServeFile::new(web.join("index.html"));
    let router = Router::new()
        .route("/api/projects", get(projects).post(add_project))
        .route("/api/projects/{id}", delete(remove_project))
        .route("/api/examples", get(list_examples))
        .route("/api/projects/{id}/docs", get(docs))
        .route("/api/projects/{id}/files/{*path}", get(file))
        .route("/api/projects/{id}/open/{what}", post(open))
        .route("/api/pick-folder", post(pick_folder))
        .route("/api/bus", get(bus_socket))
        .route("/apps/{name}", get(apps::slash))
        .route("/apps/{name}/", get(|UrlPath(name): UrlPath<String>, req| apps::proxy(UrlPath((name, String::new())), req)))
        .route("/apps/{name}/{*rest}", get(apps::proxy))
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
        .layer(SetResponseHeaderLayer::if_not_present(header::CACHE_CONTROL, HeaderValue::from_static("no-cache")))
        .with_state(app);

    let listener = tokio::net::TcpListener::bind((host, port)).await.map_err(|e| format!("{host}:{port}: {e}"))?;
    println!("Boardroom: http://localhost:{port}/");
    axum::serve(listener, router).await.map_err(|e| e.to_string())
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

async fn projects(State(app): State<Shared>) -> impl IntoResponse {
    let entries = app.list.lock().unwrap().projects.clone();
    let summaries = tokio::task::spawn_blocking(move || {
        let mut out: Vec<_> = entries
            .iter()
            .map(|e| project::summarize(&config::project_id(&e.path), &e.path, e.pinned))
            .collect();
        examples::tag(&mut out, &examples::load(&crate::repo_dir()), &crate::repo_dir());
        out
    })
    .await
    .unwrap_or_default();
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

async fn add_project(State(app): State<Shared>, Json(body): Json<AddBody>) -> ApiResult<Json<serde_json::Value>> {
    let raw = body.path.trim();
    let expanded = match raw.strip_prefix("~/") {
        Some(rest) => config::home().join(rest),
        None => PathBuf::from(raw),
    };
    let path = {
        let mut list = app.list.lock().unwrap();
        let path = list.add(&expanded).map_err(|e| bad(StatusCode::BAD_REQUEST, e))?;
        list.save().map_err(|e| bad(StatusCode::INTERNAL_SERVER_ERROR, e))?;
        path
    };
    watch(&app, &path);
    let _ = app.bus.send(serde_json::json!({"type": "projects"}).to_string());
    Ok(Json(serde_json::json!({"id": config::project_id(&path)})))
}

async fn remove_project(State(app): State<Shared>, UrlPath(id): UrlPath<String>) -> ApiResult<StatusCode> {
    let path = project_path(&app, &id)?;
    {
        let mut list = app.list.lock().unwrap();
        list.remove(&path);
        list.save().map_err(|e| bad(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    }
    if let Some(w) = app.watcher.lock().unwrap().as_mut() {
        let _ = w.unwatch(&path.join("design"));
    }
    let _ = app.bus.send(serde_json::json!({"type": "projects"}).to_string());
    Ok(StatusCode::NO_CONTENT)
}

/// The project's markdown documents: design/*.md and design/gates/*.md.
async fn docs(State(app): State<Shared>, UrlPath(id): UrlPath<String>) -> ApiResult<Json<Vec<String>>> {
    let root = project_path(&app, &id)?;
    let mut out = vec![];
    for dir in ["design", "design/gates"] {
        let Ok(entries) = std::fs::read_dir(root.join(dir)) else { continue };
        let mut names: Vec<String> = entries
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "md"))
            .map(|e| format!("{dir}/{}", e.file_name().to_string_lossy()))
            .collect();
        names.sort_by(|a, b| natural(a).cmp(&natural(b)));
        out.extend(names);
    }
    Ok(Json(out))
}

/// Sorts phase2 before phase10.
fn natural(s: &str) -> (String, u64, String) {
    let digits_at = s.find(|c: char| c.is_ascii_digit()).unwrap_or(s.len());
    let (head, tail) = s.split_at(digits_at);
    let end = tail.find(|c: char| !c.is_ascii_digit()).unwrap_or(tail.len());
    (head.to_string(), tail[..end].parse().unwrap_or(0), tail[end..].to_string())
}

/// A file inside a project folder, or a directory listing as JSON.
async fn file(State(app): State<Shared>, UrlPath((id, rel)): UrlPath<(String, String)>) -> ApiResult<Response> {
    let root = project_path(&app, &id)?;
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
    let bytes = tokio::fs::read(&path).await.map_err(|e| bad(StatusCode::NOT_FOUND, e.to_string()))?;
    Ok(([(header::CONTENT_TYPE, mime(&path))], bytes).into_response())
}

/// Resolves `rel` under `root`, refusing anything that leaves the folder.
fn inside(root: &Path, rel: &str) -> ApiResult<PathBuf> {
    let root = std::fs::canonicalize(root).map_err(|e| bad(StatusCode::NOT_FOUND, e.to_string()))?;
    let path = std::fs::canonicalize(root.join(rel)).map_err(|_| bad(StatusCode::NOT_FOUND, "not found"))?;
    if !path.starts_with(&root) {
        return Err(bad(StatusCode::FORBIDDEN, "outside the project"));
    }
    Ok(path)
}

fn mime(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase().as_str() {
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
        "md" | "csv" | "toml" | "txt" | "kicad_sch" | "kicad_pcb" | "kicad_dru" | "net" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// Opens something for the person, on this machine's desktop.
async fn open(State(app): State<Shared>, UrlPath((id, what)): UrlPath<(String, String)>) -> ApiResult<StatusCode> {
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
        .args(["--file-selection", "--directory", "--title=Add a board project"])
        .output()
        .await
        .map_err(|e| bad(StatusCode::NOT_IMPLEMENTED, format!("no folder dialog (zenity: {e}); paste the path instead")))?;
    let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok(Json(serde_json::json!({"path": if out.status.success() && !path.is_empty() { Some(path) } else { None }})))
}

async fn bus_socket(State(app): State<Shared>, ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(move |socket| bus_client(socket, app))
}

async fn bus_client(mut socket: WebSocket, app: Shared) {
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
                Some(Ok(Message::Text(text))) => { let _ = app.bus.send(text.to_string()); }
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
    let roots: Vec<PathBuf> = app.list.lock().unwrap().projects.iter().map(|p| p.path.clone()).collect();
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
            let projects: Vec<PathBuf> = app.list.lock().unwrap().projects.iter().map(|p| p.path.clone()).collect();
            let ids: BTreeSet<String> = changed
                .iter()
                .filter_map(|c| projects.iter().find(|root| c.starts_with(root)))
                .map(|root| config::project_id(root))
                .collect();
            for id in ids {
                let _ = app.bus.send(serde_json::json!({"type": "status", "project": id}).to_string());
            }
        }
    });
}

fn watch(app: &App, root: &Path) {
    if let Some(w) = app.watcher.lock().unwrap().as_mut() {
        let design = root.join("design");
        if design.is_dir() {
            if let Err(e) = w.watch(&design, RecursiveMode::Recursive) {
                eprintln!("boardroom: can't watch {}: {e}", design.display());
            }
        }
    }
}
