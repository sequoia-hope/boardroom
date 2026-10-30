//! The suite's other apps, proxied under Boardroom's own address
//! (`/apps/<name>/…`), so every window and iframe shares one origin
//! (docs/shell-spec.md, "One origin").
//!
//! Each app runs its own server on its registered port; Boardroom asks the
//! registry for that port (`proj port <project>`) rather than keeping a copy.
//! WebSockets pass through too (Studio drives its router over one).

use axum::{
    body::Body,
    extract::{ConnectInfo, Path as UrlPath, Request},
    http::{StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use hyper_util::{client::legacy::Client, rt::{TokioExecutor, TokioIo}};
use std::sync::OnceLock;

/// An app: its name in Boardroom's URLs, its registry project, and the path
/// its page is served under on its own server.
pub struct AppDef {
    pub name: &'static str,
    pub project: &'static str,
    pub base: &'static str,
}

pub const APPS: [AppDef; 2] = [
    AppDef { name: "composer", project: "circuit-composer", base: "/app/dist/" },
    // ultraroute's server: the studio's page is /apps/studio/studio/, its socket /apps/studio/ws.
    AppDef { name: "studio", project: "ultraroute", base: "/" },
];

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

pub async fn proxy(
    ConnectInfo(peer): ConnectInfo<std::net::SocketAddr>,
    UrlPath((name, rest)): UrlPath<(String, String)>,
    mut req: Request,
) -> Response {
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
    // A socket can route and save a board, so, like any change, only from this machine.
    let upgrade = req.headers().contains_key(header::UPGRADE);
    if upgrade && !peer.ip().is_loopback() {
        return (StatusCode::FORBIDDEN, "Boardroom takes changes only from the machine it runs on").into_response();
    }
    let browser_side = upgrade.then(|| hyper::upgrade::on(&mut req));
    let (mut parts, body) = req.into_parts();
    parts.uri = target;
    parts.headers.remove(header::HOST);
    match client().request(Request::from_parts(parts, body)).await {
        Ok(mut resp) => {
            if let (StatusCode::SWITCHING_PROTOCOLS, Some(browser_side)) = (resp.status(), browser_side) {
                let app_side = hyper::upgrade::on(&mut resp);
                tokio::spawn(async move {
                    if let (Ok(a), Ok(b)) = (browser_side.await, app_side.await) {
                        let _ = tokio::io::copy_bidirectional(&mut TokioIo::new(a), &mut TokioIo::new(b)).await;
                    }
                });
            }
            resp.map(Body::new).into_response()
        }
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
