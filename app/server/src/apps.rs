//! The suite's other apps, proxied under Boardroom's own address
//! (`/apps/<name>/…`), so every window and iframe shares one origin
//! (docs/shell-spec.md, "One origin").
//!
//! Each app runs its own server on its registered port; Boardroom asks the
//! registry for that port (`proj port <project>`) rather than keeping a copy.

use axum::{
    body::Body,
    extract::{Path as UrlPath, Request},
    http::{StatusCode, Uri},
    response::{IntoResponse, Response},
};
use hyper_util::{client::legacy::Client, rt::TokioExecutor};
use std::sync::OnceLock;

/// An app: its name in Boardroom's URLs, its registry project, and the path
/// its page is served under on its own server.
pub struct AppDef {
    pub name: &'static str,
    pub project: &'static str,
    pub base: &'static str,
}

pub const APPS: [AppDef; 1] = [AppDef { name: "composer", project: "circuit-composer", base: "/app/dist/" }];

fn registry_port(project: &str) -> Option<u16> {
    let out = std::process::Command::new("proj").args(["port", project]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

fn client() -> &'static Client<hyper_util::client::legacy::connect::HttpConnector, Body> {
    static CLIENT: OnceLock<Client<hyper_util::client::legacy::connect::HttpConnector, Body>> = OnceLock::new();
    CLIENT.get_or_init(|| Client::builder(TokioExecutor::new()).build_http())
}

pub async fn proxy(UrlPath((name, rest)): UrlPath<(String, String)>, req: Request) -> Response {
    let Some(app) = APPS.iter().find(|a| a.name == name) else {
        return (StatusCode::NOT_FOUND, format!("no app '{name}'")).into_response();
    };
    let project = app.project;
    let Some(port) = tokio::task::spawn_blocking(move || registry_port(project)).await.ok().flatten() else {
        return (StatusCode::BAD_GATEWAY, format!("{} has no registered port (proj port {})", app.name, app.project))
            .into_response();
    };
    let query = req.uri().query().map(|q| format!("?{q}")).unwrap_or_default();
    let target: Uri = match format!("http://127.0.0.1:{port}{}{rest}{query}", app.base).parse() {
        Ok(u) => u,
        Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    };
    let (mut parts, body) = req.into_parts();
    parts.uri = target;
    parts.headers.remove(axum::http::header::HOST);
    match client().request(Request::from_parts(parts, body)).await {
        Ok(resp) => resp.map(Body::new).into_response(),
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            format!("{} isn't running on port {port} (proj up {}): {e}", app.name, app.project),
        )
            .into_response(),
    }
}

/// `/apps/<name>` without the slash: send it to the slash, so relative URLs resolve.
pub async fn slash(UrlPath(name): UrlPath<String>) -> impl IntoResponse {
    axum::response::Redirect::permanent(&format!("/apps/{name}/"))
}
