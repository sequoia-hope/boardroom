//! Apps served under Boardroom's own address (`/apps/<name>/…`), so every window and
//! iframe shares one origin (docs/shell-spec.md, "One origin").
//!
//! Compose and Route are becoming modes of this program (docs/one-system-spec.md); until
//! their crates arrive their addresses answer with a page saying so. Static builds such as
//! Waffle Iron (docs/suite-spec.md, "Stage 1: local native window") are served from the
//! directory named in `~/.config/boardroom/config.toml`, with unknown paths falling back to
//! `index.html` for client-side routing.

use ::project::config::Config;
use crate::server::Shared;
use axum::{
    body::Body,
    extract::{Path as UrlPath, Request, State},
    http::{HeaderValue, StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use std::path::Path;
use tower_http::services::{ServeDir, ServeFile};

pub enum AppKind {
    /// A mode of this program whose crates have not arrived yet.
    Pending { mode: &'static str, step: &'static str },
    /// A static build on disk, configured in `config.toml`.
    Static { key: &'static str, dir: fn(&Config) -> Option<&Path> },
}

pub struct AppDef {
    pub name: &'static str,
    pub kind: AppKind,
}

pub const APPS: [AppDef; 3] = [
    AppDef { name: "composer", kind: AppKind::Pending { mode: "Compose", step: "step 5" } },
    AppDef { name: "studio", kind: AppKind::Pending { mode: "Route", step: "step 6" } },
    AppDef { name: "waffle", kind: AppKind::Static { key: "waffle", dir: |c| c.waffle.as_deref() } },
];

pub async fn serve(
    State(state): State<Shared>,
    UrlPath((name, rest)): UrlPath<(String, String)>,
    req: Request,
) -> Response {
    let Some(app) = APPS.iter().find(|a| a.name == name) else {
        return (StatusCode::NOT_FOUND, format!("no app '{name}'")).into_response();
    };
    match app.kind {
        AppKind::Pending { mode, step } => pending(mode, step),
        AppKind::Static { key, dir } => serve_static(app.name, key, dir, &state.config_dir, &rest, req).await,
    }
}

/// The page a mode shows until it is built into this program.
fn pending(mode: &str, step: &str) -> Response {
    let page = format!(
        "<!doctype html><meta charset=utf-8><title>{mode}</title>\
         <body style=\"font:16px system-ui;margin:3rem;max-width:40rem\">\
         <h1>{mode}</h1><p>{mode} is being built into Boardroom as one of its modes \
         ({step} of <code>docs/one-system-spec.md</code>). It is not available yet.</p>"
    );
    (StatusCode::SERVICE_UNAVAILABLE, [(header::CONTENT_TYPE, "text/html; charset=utf-8")], page).into_response()
}

async fn serve_static(
    name: &str,
    key: &str,
    get_dir: fn(&Config) -> Option<&Path>,
    config_dir: &Path,
    rest: &str,
    mut req: Request,
) -> Response {
    let cfg = match Config::load_from(config_dir) {
        Ok(c) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, format!("{name} config error ({e})")).into_response(),
    };
    let Some(dir) = get_dir(&cfg).map(Path::to_path_buf) else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            format!(
                "{name} isn't configured: set {key} = \"<build dir>\" in {}/config.toml \
                 (build with BASE_PATH=/apps/{name} npm run build)",
                config_dir.display()
            ),
        )
            .into_response();
    };
    let index = dir.join("index.html");
    if !dir.is_dir() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            format!("{name}'s build folder {} is missing (run: BASE_PATH=/apps/{name} npm run build)", dir.display()),
        )
            .into_response();
    }
    if !index.is_file() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            format!("no {name} build at {} (missing index.html; run: BASE_PATH=/apps/{name} npm run build)", dir.display()),
        )
            .into_response();
    }

    let prefix = format!("/apps/{name}");
    let stripped = req.uri().path().strip_prefix(&prefix).unwrap_or("/");
    let sub_path = if stripped.is_empty() { "/" } else { stripped };
    let method = req.method().clone();
    let rewritten: Uri = match req.uri().query() {
        Some(q) => format!("{sub_path}?{q}"),
        None => sub_path.to_string(),
    }
    .parse()
    .unwrap_or_else(|_| Uri::from_static("/"));
    *req.uri_mut() = rewritten;

    let mut resp = match ServeDir::new(&dir).fallback(ServeFile::new(&index)).try_call(req).await {
        Ok(r) => r.into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };
    // ServeDir redirects `/sub` -> `/sub/` (with `Location`) whenever `sub` is a
    // folder on disk. Note: `is_redirection()` is true for every 3xx including
    // `304 Not Modified`, so check `Location` so cached `.js`/`.wasm` revalidations
    // keep their 304 instead of being replaced by `index.html`.
    if resp.status() != StatusCode::NOT_MODIFIED && resp.headers().contains_key(header::LOCATION) {
        let rel = rest.trim_matches('/');
        if !rel.is_empty() && !rel.contains("..") && dir.join(rel).join("index.html").is_file() {
            if let Some(loc) = resp.headers().get(header::LOCATION).and_then(|v| v.to_str().ok()).map(str::to_string)
                && let Ok(prefixed) = HeaderValue::from_str(&format!("{prefix}{loc}"))
            {
                resp.headers_mut().insert(header::LOCATION, prefixed);
            }
        } else {
            let fallback_req = Request::builder().method(method).uri("/index.html").body(Body::empty()).unwrap();
            resp = match ServeFile::new(&index).try_call(fallback_req).await {
                Ok(r) => r.into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
            };
        }
    }
    resp
}

/// `/apps/<name>` without the slash: send it to the slash, so relative URLs resolve.
pub async fn slash(UrlPath(name): UrlPath<String>) -> impl IntoResponse {
    axum::response::Redirect::permanent(&format!("/apps/{name}/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::project::config::ProjectList;
    use http_body_util::BodyExt;
    use std::path::PathBuf;

    struct TestServer {
        dir: PathBuf,
        port: u16,
        task: tokio::task::JoinHandle<()>,
    }

    impl TestServer {
        async fn start(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("boardroom-apps-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(dir.join("cfg")).unwrap();
            std::fs::create_dir_all(dir.join("web")).unwrap();
            std::fs::write(dir.join("web/index.html"), "<!doctype html><title>Boardroom Shell</title>").unwrap();

            let (router, _) = crate::server::router(dir.join("cfg"), ProjectList::default(), dir.join("web"));
            let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
            let port = listener.local_addr().unwrap().port();
            let svc = router.into_make_service_with_connect_info::<std::net::SocketAddr>();
            let task = tokio::spawn(async move {
                let _ = axum::serve(listener, svc).await;
            });
            Self { dir, port, task }
        }

        async fn get(&self, path: &str) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
            self.get_with_headers(path, &[]).await
        }

        async fn get_with_headers(
            &self,
            path: &str,
            extra: &[(header::HeaderName, &str)],
        ) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
            let uri: Uri = format!("http://127.0.0.1:{}{path}", self.port).parse().unwrap();
            let mut builder = Request::builder().method("GET").uri(uri);
            for (k, v) in extra {
                builder = builder.header(k, *v);
            }
            let req = builder.body(Body::empty()).unwrap();
            let client: hyper_util::client::legacy::Client<_, Body> =
                hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new()).build_http();
            let resp = client.request(req).await.unwrap();
            let status = resp.status();
            let headers = resp.headers().clone();
            let body = resp.into_body().collect().await.unwrap().to_bytes().to_vec();
            (status, headers, body)
        }
    }

    impl Drop for TestServer {
        fn drop(&mut self) {
            self.task.abort();
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn assert_isolated(headers: &axum::http::HeaderMap) {
        assert_eq!(headers.get("cross-origin-opener-policy").and_then(|v| v.to_str().ok()), Some("same-origin"));
        assert_eq!(headers.get("cross-origin-embedder-policy").and_then(|v| v.to_str().ok()), Some("require-corp"));
        assert_eq!(headers.get("cross-origin-resource-policy").and_then(|v| v.to_str().ok()), Some("same-origin"));
    }

    #[tokio::test]
    async fn modes_not_built_yet_say_so() {
        let srv = TestServer::start("pending").await;
        for (path, mode) in [("/apps/composer/", "Compose"), ("/apps/studio/studio/", "Route")] {
            let (status, _, body) = srv.get(path).await;
            assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
            assert!(String::from_utf8_lossy(&body).contains(mode));
        }
    }

    #[tokio::test]
    async fn waffle_reports_clearly_when_unconfigured_or_missing() {
        let srv = TestServer::start("missing").await;

        // 1. Not configured at all.
        let (status, headers, body) = srv.get("/apps/waffle/").await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_isolated(&headers);
        let msg = String::from_utf8(body).unwrap();
        assert!(msg.contains("waffle isn't configured") && msg.contains("config.toml"), "{msg}");

        // 2. Configured, but directory does not exist.
        let missing = srv.dir.join("no-such-build");
        std::fs::write(srv.dir.join("cfg/config.toml"), format!("waffle = {:?}\n", missing)).unwrap();
        let (status, headers, body) = srv.get("/apps/waffle/").await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_isolated(&headers);
        let msg = String::from_utf8(body).unwrap();
        assert!(msg.contains("missing") && msg.contains("no-such-build"), "{msg}");

        // 3. Directory exists, but index.html is missing.
        let empty = srv.dir.join("empty-build");
        std::fs::create_dir_all(&empty).unwrap();
        std::fs::write(srv.dir.join("cfg/config.toml"), format!("waffle = {:?}\n", empty)).unwrap();
        let (status, headers, body) = srv.get("/apps/waffle/").await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_isolated(&headers);
        let msg = String::from_utf8(body).unwrap();
        assert!(msg.contains("index.html"), "{msg}");
    }

    #[tokio::test]
    async fn waffle_serves_static_files_mime_types_spa_fallback_and_headers() {
        let srv = TestServer::start("static").await;
        let build = srv.dir.join("waffle-build");
        std::fs::create_dir_all(build.join("_app")).unwrap();
        std::fs::create_dir_all(build.join("pkg")).unwrap();
        let html = "<!doctype html><title>Waffle Iron</title>";
        std::fs::write(build.join("index.html"), html).unwrap();
        std::fs::write(build.join("_app/start.js"), "export const boot = true;\n").unwrap();
        std::fs::write(build.join("pkg/engine.wasm"), b"\0asm\x01\x00\x00\x00").unwrap();
        std::fs::write(srv.dir.join("secret.txt"), "top-secret").unwrap();
        std::fs::write(srv.dir.join("cfg/config.toml"), format!("waffle = {:?}\n", build)).unwrap();

        // Slash redirect on `/apps/waffle`.
        let (status, headers, _) = srv.get("/apps/waffle").await;
        assert_eq!(status, StatusCode::PERMANENT_REDIRECT);
        assert_eq!(headers.get(header::LOCATION).and_then(|v| v.to_str().ok()), Some("/apps/waffle/"));
        assert_isolated(&headers);

        // An unknown API path is a 404, so Waffle Iron's dev-server clients fall through to its
        // static files instead of taking Boardroom's page for data.
        let (status, _, _) = srv.get("/api/assay-cases").await;
        assert_eq!(status, StatusCode::NOT_FOUND);

        // Root serves Waffle's index.html.
        let (status, headers, body) = srv.get("/apps/waffle/").await;
        assert_eq!(status, StatusCode::OK);
        assert_isolated(&headers);
        let ct = headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("");
        assert!(ct.starts_with("text/html"), "content-type: {ct}");
        assert_eq!(body, html.as_bytes());

        // `.js` gets a JavaScript MIME type, and conditional revalidation keeps `304 Not Modified`.
        let (status, headers, body) = srv.get("/apps/waffle/_app/start.js").await;
        assert_eq!(status, StatusCode::OK);
        assert_isolated(&headers);
        let ct = headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("");
        assert!(ct.contains("javascript"), "content-type: {ct}");
        assert_eq!(body, b"export const boot = true;\n");
        let last_mod = headers.get(header::LAST_MODIFIED).and_then(|v| v.to_str().ok()).unwrap().to_string();
        let (status_304, headers_304, body_304) =
            srv.get_with_headers("/apps/waffle/_app/start.js", &[(header::IF_MODIFIED_SINCE, &last_mod)]).await;
        assert_eq!(status_304, StatusCode::NOT_MODIFIED);
        assert_isolated(&headers_304);
        assert!(body_304.is_empty());

        // `.wasm` gets `application/wasm`.
        let (status, headers, body) = srv.get("/apps/waffle/pkg/engine.wasm").await;
        assert_eq!(status, StatusCode::OK);
        assert_isolated(&headers);
        assert_eq!(headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()), Some("application/wasm"));
        assert_eq!(body, b"\0asm\x01\x00\x00\x00");

        // Unknown client-side route falls back to Waffle's index.html (not Boardroom's).
        let (status, headers, body) = srv.get("/apps/waffle/doc/abc-123?tab=part").await;
        assert_eq!(status, StatusCode::OK);
        assert_isolated(&headers);
        let ct = headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("");
        assert!(ct.starts_with("text/html"), "content-type: {ct}");
        assert_eq!(body, html.as_bytes());

        // An asset folder with no index.html of its own falls back cleanly rather than redirecting to `/pkg/`.
        let (status, headers, body) = srv.get("/apps/waffle/pkg").await;
        assert_eq!(status, StatusCode::OK);
        assert_isolated(&headers);
        assert_eq!(body, html.as_bytes());

        // Encoded traversal never leaves the build folder.
        let (_, _, body) = srv.get("/apps/waffle/%2e%2e/secret.txt").await;
        assert_ne!(body, b"top-secret");

        // Unknown app still 404s.
        let (status, headers, _) = srv.get("/apps/nope/").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_isolated(&headers);
    }
}
