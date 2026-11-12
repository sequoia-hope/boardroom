use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, Uri, header};
use ::project::config::{self, ProjectList};
use ::project::links::LinksCache;
use server::terminals::{TermSummary, TerminalsOptions};
use futures_util::{SinkExt, StreamExt};
use http_body_util::BodyExt;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio_tungstenite::tungstenite;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tower::ServiceExt;

static CASE_SEQ: AtomicUsize = AtomicUsize::new(1);

struct Harness {
    root: PathBuf,
    cfg_dir: PathBuf,
    term_cache_dir: PathBuf,
    project_dir: PathBuf,
    project_id: String,
    project2_id: String,
    port: u16,
    app: server::server::Shared,
    task: tokio::task::JoinHandle<()>,
}

struct HarnessOpts {
    config_toml: Option<String>,
    term_command: Option<Vec<String>>,
    relay_port_fallback: Option<u16>,
    state_dir_override: Option<PathBuf>,
}

impl Default for HarnessOpts {
    fn default() -> Self {
        Self {
            config_toml: None,
            term_command: Some(vec!["/bin/sh".into()]),
            relay_port_fallback: None,
            state_dir_override: None,
        }
    }
}

impl Harness {
    async fn start(label: &str, opts: HarnessOpts) -> Self {
        let n = CASE_SEQ.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir()
            .join(format!("boardroom-term-it-{}", std::process::id()))
            .join(format!("{n:02}-{label}"));
        let _ = std::fs::remove_dir_all(&root);

        let cfg_dir = root.join("cfg");
        let web_dir = root.join("web");
        let links_cache_dir = root.join("cache/links");
        let term_cache_dir = root.join("cache/term");
        let state_dir = opts
            .state_dir_override
            .unwrap_or_else(|| root.join("state/boardroom"));
        let project_dir = root.join("proj-one");
        let project2_dir = root.join("proj-two");

        for d in [
            &cfg_dir,
            &web_dir,
            &links_cache_dir,
            &term_cache_dir,
            &state_dir,
            &project_dir.join("design"),
            &project2_dir.join("design"),
        ] {
            std::fs::create_dir_all(d).unwrap();
        }

        std::fs::write(
            web_dir.join("index.html"),
            "<!doctype html><title>Boardroom</title>",
        )
        .unwrap();
        std::fs::write(
            project_dir.join("design/status.json"),
            "{\n  \"schema\": 1,\n  \"board\": \"easy2350\",\n  \"phase\": 4,\n  \"needs_user\": []\n}\n",
        )
        .unwrap();
        std::fs::write(
            project2_dir.join("design/status.json"),
            "{\n  \"schema\": 1,\n  \"board\": \"auxboard\",\n  \"phase\": 2,\n  \"needs_user\": []\n}\n",
        )
        .unwrap();

        if let Some(toml_text) = opts.config_toml {
            std::fs::write(cfg_dir.join("config.toml"), toml_text).unwrap();
        }

        let mut list = ProjectList::default();
        let canon1 = list.add(&project_dir).unwrap();
        let canon2 = list.add(&project2_dir).unwrap();
        list.save_to(&cfg_dir).unwrap();
        let project_id = config::project_id(&canon1);
        let project2_id = config::project_id(&canon2);

        let fallback = opts.relay_port_fallback;
        let term_options = TerminalsOptions {
            state_dir,
            cache_dir: term_cache_dir.clone(),
            term_command: opts.term_command,
            relay_port_resolver: Arc::new(move || fallback),
        };

        let (router, app) = server::server::router_with_options(
            cfg_dir.clone(),
            list,
            web_dir,
            LinksCache::with_dir(links_cache_dir),
            term_options,
        );

        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let port = listener.local_addr().unwrap().port();
        app.set_port(port);

        let svc = router.into_make_service_with_connect_info::<SocketAddr>();
        let task = tokio::spawn(async move {
            let _ = axum::serve(listener, svc).await;
        });

        Self {
            root,
            cfg_dir,
            term_cache_dir,
            project_dir: canon1,
            project_id,
            project2_id,
            port,
            app,
            task,
        }
    }

    async fn req_with_origin(
        &self,
        method: &str,
        path: &str,
        origin: Option<&str>,
        json_body: Option<serde_json::Value>,
    ) -> (StatusCode, Vec<u8>) {
        let client: Client<_, Body> = Client::builder(TokioExecutor::new()).build_http();
        let uri: Uri = format!("http://127.0.0.1:{}{path}", self.port)
            .parse()
            .unwrap();
        let mut builder = Request::builder().method(method).uri(uri);
        if let Some(orig) = origin {
            builder = builder.header(header::ORIGIN, orig);
        }
        let body = match json_body {
            Some(v) => {
                builder = builder.header(header::CONTENT_TYPE, "application/json");
                Body::from(serde_json::to_vec(&v).unwrap())
            }
            None => Body::empty(),
        };
        let resp = client.request(builder.body(body).unwrap()).await.unwrap();
        let status = resp.status();
        let bytes = resp
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec();
        (status, bytes)
    }

    async fn req(
        &self,
        method: &str,
        path: &str,
        json_body: Option<serde_json::Value>,
    ) -> (StatusCode, Vec<u8>) {
        let own = format!("http://127.0.0.1:{}", self.port);
        self.req_with_origin(method, path, Some(&own), json_body)
            .await
    }

    async fn connect_term_ws(
        &self,
        term_id: &str,
        origin: Option<&str>,
    ) -> Result<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        tungstenite::Error,
    > {
        let url = format!("ws://127.0.0.1:{}/api/term/{term_id}", self.port);
        let mut req = url.into_client_request().unwrap();
        if let Some(orig) = origin {
            req.headers_mut()
                .insert(header::ORIGIN, orig.parse().unwrap());
        }
        let (ws, _) = tokio_tungstenite::connect_async(req).await?;
        Ok(ws)
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        self.task.abort();
        self.app.shutdown_terminals();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn allocate_ephemeral_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

async fn recv_until_contains(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    needle: &str,
) -> String {
    let mut acc = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let rem = deadline.saturating_duration_since(tokio::time::Instant::now());
        assert!(
            !rem.is_zero(),
            "timed out waiting for '{needle}'; got so far: {:?}",
            String::from_utf8_lossy(&acc)
        );
        match tokio::time::timeout(rem, ws.next()).await {
            Ok(Some(Ok(tungstenite::Message::Binary(b)))) => {
                acc.extend_from_slice(&b);
                let text = String::from_utf8_lossy(&acc);
                if text.contains(needle) {
                    return text.into_owned();
                }
            }
            Ok(Some(Ok(tungstenite::Message::Close(_)))) | Ok(None) => {
                let text = String::from_utf8_lossy(&acc).into_owned();
                assert!(
                    text.contains(needle),
                    "socket closed before seeing '{needle}'; got: {text:?}"
                );
                return text;
            }
            Ok(Some(Ok(_))) => {}
            Ok(Some(Err(e))) => panic!("ws read error waiting for '{needle}': {e}"),
            Err(_) => panic!(
                "timed out waiting for '{needle}'; got so far: {:?}",
                String::from_utf8_lossy(&acc)
            ),
        }
    }
}

#[tokio::test]
async fn start_write_echo_multiple_watchers_and_reconnect_replay() {
    let h = Harness::start("echo-replay", HarnessOpts::default()).await;
    let own_origin = format!("http://127.0.0.1:{}", h.port);

    let (st, body) = h
        .req(
            "POST",
            "/api/term",
            Some(serde_json::json!({"project": h.project_id, "kind": "project"})),
        )
        .await;
    assert_eq!(st, StatusCode::OK);
    let started: TermSummary = serde_json::from_slice(&body).unwrap();
    assert!(started.alive);
    assert_eq!(started.project, h.project_id);
    assert_eq!(started.kind, "project");
    assert_eq!(started.title, "easy2350");

    // Second POST for the same (project, kind) returns the existing live terminal.
    let (st2, body2) = h
        .req(
            "POST",
            "/api/term",
            Some(serde_json::json!({"project": h.project_id, "kind": "project"})),
        )
        .await;
    assert_eq!(st2, StatusCode::OK);
    let again: TermSummary = serde_json::from_slice(&body2).unwrap();
    assert_eq!(again.id, started.id);

    // GET /api/term lists it.
    let (lst_st, lst_body) = h.req("GET", "/api/term", None).await;
    assert_eq!(lst_st, StatusCode::OK);
    let list: Vec<TermSummary> = serde_json::from_slice(&lst_body).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, started.id);
    assert!(list[0].alive);

    // Connect WebSocket 1, send "echo hi\n", read "hi" back.
    let mut ws1 = h
        .connect_term_ws(&started.id, Some(&own_origin))
        .await
        .unwrap();
    ws1.send(tungstenite::Message::Binary(b"echo hi\n".to_vec().into()))
        .await
        .unwrap();
    let out1 = recv_until_contains(&mut ws1, "hi").await;
    assert!(out1.contains("hi"));

    // Connect a second simultaneous watcher: gets replay ("hi") and live output.
    let mut ws2 = h
        .connect_term_ws(&started.id, Some(&own_origin))
        .await
        .unwrap();
    let replay2 = recv_until_contains(&mut ws2, "hi").await;
    assert!(replay2.contains("hi"));

    ws1.send(tungstenite::Message::Binary(
        b"echo dual-watcher-ok\n".to_vec().into(),
    ))
    .await
    .unwrap();
    recv_until_contains(&mut ws1, "dual-watcher-ok").await;
    recv_until_contains(&mut ws2, "dual-watcher-ok").await;

    // Disconnect both sockets, reconnect, and verify scrollback is replayed before live output.
    let _ = ws1.close(None).await;
    let _ = ws2.close(None).await;
    drop(ws1);
    drop(ws2);

    let mut ws3 = h
        .connect_term_ws(&started.id, Some(&own_origin))
        .await
        .unwrap();
    let replayed = recv_until_contains(&mut ws3, "dual-watcher-ok").await;
    assert!(replayed.contains("hi"));
    assert!(replayed.contains("dual-watcher-ok"));

    ws3.send(tungstenite::Message::Binary(
        b"echo after-reconnect\n".to_vec().into(),
    ))
    .await
    .unwrap();
    recv_until_contains(&mut ws3, "after-reconnect").await;
}

#[tokio::test]
async fn resize_frame_accepted_and_osc_title_recorded() {
    let h = Harness::start("resize-title", HarnessOpts::default()).await;
    let own_origin = format!("http://127.0.0.1:{}", h.port);

    let (_, body) = h
        .req(
            "POST",
            "/api/term",
            Some(serde_json::json!({"project": h.project_id, "kind": "project"})),
        )
        .await;
    let term: TermSummary = serde_json::from_slice(&body).unwrap();

    let mut ws = h
        .connect_term_ws(&term.id, Some(&own_origin))
        .await
        .unwrap();
    ws.send(tungstenite::Message::Text(
        r#"{"type":"resize","cols":113,"rows":39}"#.into(),
    ))
    .await
    .unwrap();
    // Give the kernel PTY ioctl a moment before running stty size.
    tokio::time::sleep(Duration::from_millis(50)).await;
    ws.send(tungstenite::Message::Binary(b"stty size\n".to_vec().into()))
        .await
        .unwrap();
    recv_until_contains(&mut ws, "39 113").await;

    // Emit an OSC 0 title escape sequence and check GET /api/term reflects it.
    ws.send(tungstenite::Message::Binary(
        b"printf '\\033]0;Board Shell Title\\007\\ndone-osc\\n'\n"
            .to_vec()
            .into(),
    ))
    .await
    .unwrap();
    recv_until_contains(&mut ws, "done-osc").await;

    let (_, list_body) = h.req("GET", "/api/term", None).await;
    let list: Vec<TermSummary> = serde_json::from_slice(&list_body).unwrap();
    assert_eq!(list[0].title, "Board Shell Title");
}

#[tokio::test]
async fn child_exit_keeps_scrollback_marks_dead_post_restarts_and_delete_ends() {
    let h = Harness::start("exit-restart", HarnessOpts::default()).await;
    let own_origin = format!("http://localhost:{}", h.port);

    let (_, body) = h
        .req_with_origin(
            "POST",
            "/api/term",
            Some(&own_origin),
            Some(serde_json::json!({"project": h.project_id, "kind": "project"})),
        )
        .await;
    let first: TermSummary = serde_json::from_slice(&body).unwrap();

    let mut ws = h
        .connect_term_ws(&first.id, Some(&own_origin))
        .await
        .unwrap();
    ws.send(tungstenite::Message::Binary(
        b"echo goodbye-scrollback && exit 0\n".to_vec().into(),
    ))
    .await
    .unwrap();
    recv_until_contains(&mut ws, "goodbye-scrollback").await;

    // Poll GET /api/term until alive becomes false.
    let mut marked_dead = false;
    for _ in 0..40 {
        let (_, lb) = h.req("GET", "/api/term", None).await;
        let list: Vec<TermSummary> = serde_json::from_slice(&lb).unwrap();
        if list.len() == 1 && !list[0].alive {
            marked_dead = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(
        marked_dead,
        "expected exited terminal to be marked alive=false"
    );

    // Reconnecting to the exited terminal still replays its scrollback buffer!
    let mut dead_ws = h
        .connect_term_ws(&first.id, Some(&own_origin))
        .await
        .unwrap();
    let dead_replay = recv_until_contains(&mut dead_ws, "goodbye-scrollback").await;
    assert!(dead_replay.contains("goodbye-scrollback"));

    // POST /api/term starts a fresh live terminal for the project.
    let (st2, body2) = h
        .req_with_origin(
            "POST",
            "/api/term",
            Some(&own_origin),
            Some(serde_json::json!({"project": h.project_id, "kind": "project"})),
        )
        .await;
    assert_eq!(st2, StatusCode::OK);
    let restarted: TermSummary = serde_json::from_slice(&body2).unwrap();
    assert_ne!(restarted.id, first.id);
    assert!(restarted.alive);

    // DELETE /api/term/<id> terminates and removes it.
    let (del_st, _) = h
        .req_with_origin(
            "DELETE",
            &format!("/api/term/{}", restarted.id),
            Some(&own_origin),
            None,
        )
        .await;
    assert_eq!(del_st, StatusCode::NO_CONTENT);

    let (_, after_del) = h.req("GET", "/api/term", None).await;
    let remaining: Vec<TermSummary> = serde_json::from_slice(&after_del).unwrap();
    assert!(remaining.is_empty());

    let (del_missing, _) = h
        .req("DELETE", &format!("/api/term/{}", restarted.id), None)
        .await;
    assert_eq!(del_missing, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn non_loopback_refused_on_all_term_and_selection_routes() {
    let n = CASE_SEQ.fetch_add(1, Ordering::SeqCst);
    let root = std::env::temp_dir()
        .join(format!("boardroom-term-it-{}", std::process::id()))
        .join(format!("{n:02}-non-loopback"));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("cfg")).unwrap();
    std::fs::create_dir_all(root.join("web")).unwrap();
    std::fs::write(root.join("web/index.html"), "<!doctype html>").unwrap();

    let (router, app) = server::server::router_with_options(
        root.join("cfg"),
        ProjectList::default(),
        root.join("web"),
        LinksCache::with_dir(root.join("cache/links")),
        TerminalsOptions {
            state_dir: root.join("state"),
            cache_dir: root.join("cache/term"),
            term_command: Some(vec!["/bin/sh".into()]),
            relay_port_resolver: Arc::new(|| None),
        },
    );
    app.set_port(allocate_ephemeral_port());

    let remote_peer = SocketAddr::from(([192, 0, 2, 99], 52100));
    for (method, uri, is_ws) in [
        ("GET", "/api/term", false),
        ("POST", "/api/term", false),
        ("GET", "/api/term/t-1", true),
        ("DELETE", "/api/term/t-1", false),
        ("GET", "/api/selection?project=abc", false),
    ] {
        let mut builder = Request::builder().method(method).uri(uri);
        if is_ws {
            builder = builder
                .header(header::CONNECTION, "Upgrade")
                .header(header::UPGRADE, "websocket")
                .header("sec-websocket-version", "13")
                .header("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ==");
        } else if method == "POST" {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
        }
        let body = if method == "POST" {
            Body::from(r#"{"project":"abc","kind":"project"}"#)
        } else {
            Body::empty()
        };
        let mut req = builder.body(body).unwrap();
        req.extensions_mut().insert(ConnectInfo(remote_peer));
        let resp = router.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN, "{method} {uri}");
    }

    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn foreign_origin_refused_on_post_delete_and_websocket() {
    let h = Harness::start("foreign-origin", HarnessOpts::default()).await;
    let other_port = {
        let mut p = allocate_ephemeral_port();
        while p == h.port {
            p = allocate_ephemeral_port();
        }
        p
    };
    let wrong_port_origin = format!("http://127.0.0.1:{other_port}");

    // Foreign Origins refused on POST /api/term.
    for bad_origin in [
        "https://evil.example",
        "http://evil.example",
        "null",
        wrong_port_origin.as_str(),
    ] {
        let (st, _) = h
            .req_with_origin(
                "POST",
                "/api/term",
                Some(bad_origin),
                Some(serde_json::json!({"project": h.project_id, "kind": "project"})),
            )
            .await;
        assert_eq!(st, StatusCode::FORBIDDEN, "POST with Origin {bad_origin}");
    }

    // Start legitimately with own origin.
    let (_, body) = h
        .req(
            "POST",
            "/api/term",
            Some(serde_json::json!({"project": h.project_id, "kind": "project"})),
        )
        .await;
    let started: TermSummary = serde_json::from_slice(&body).unwrap();

    // Foreign Origin refused on GET /api/term.
    let (get_st, _) = h
        .req_with_origin("GET", "/api/term", Some("https://evil.example"), None)
        .await;
    assert_eq!(get_st, StatusCode::FORBIDDEN);

    // Foreign Origin refused on WebSocket upgrade (GET /api/term/<id>).
    for bad_origin in ["https://evil.example", "null", wrong_port_origin.as_str()] {
        let err = h
            .connect_term_ws(&started.id, Some(bad_origin))
            .await
            .expect_err("foreign Origin WebSocket must fail");
        match err {
            tungstenite::Error::Http(resp) => {
                assert_eq!(
                    resp.status(),
                    StatusCode::FORBIDDEN,
                    "WS with Origin {bad_origin}"
                );
            }
            other => panic!("expected HTTP 403 error for {bad_origin}, got {other:?}"),
        }
    }

    // Foreign Origin refused on DELETE /api/term/<id>.
    let (del_bad, _) = h
        .req_with_origin(
            "DELETE",
            &format!("/api/term/{}", started.id),
            Some("https://evil.example"),
            None,
        )
        .await;
    assert_eq!(del_bad, StatusCode::FORBIDDEN);

    // Terminal is still alive after refused DELETE, and localhost Origin succeeds.
    let localhost_origin = format!("http://localhost:{}", h.port);
    let ws_ok = h
        .connect_term_ws(&started.id, Some(&localhost_origin))
        .await
        .expect("localhost Origin WebSocket should succeed");
    drop(ws_ok);

    let (del_ok, _) = h
        .req_with_origin(
            "DELETE",
            &format!("/api/term/{}", started.id),
            Some(&localhost_origin),
            None,
        )
        .await;
    assert_eq!(del_ok, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn waffle_mcp_config_written_when_configured_and_absent_otherwise() {
    // 1. When waffle_relay is unset: no mcp.json, no warning line.
    {
        let h = Harness::start("waffle-unset", HarnessOpts::default()).await;
        std::fs::write(
            h.project_dir.join("design/links.toml"),
            "[[links]]\nid = \"enclosure\"\nkind = \"waffle\"\nboard = \"easy2350\"\n",
        )
        .unwrap();

        let (_, body) = h
            .req(
                "POST",
                "/api/term",
                Some(serde_json::json!({"project": h.project_id, "kind": "project"})),
            )
            .await;
        let summary: TermSummary = serde_json::from_slice(&body).unwrap();
        assert!(!summary.waffle);
        assert!(!h.term_cache_dir.join(&summary.id).join("mcp.json").exists());
    }

    // 2. When waffle_relay and waffle_relay_port are set in config.toml:
    {
        let relay_test_port = allocate_ephemeral_port();
        let cfg_toml = format!(
            "waffle_relay = \"uv run --project /tmp/fake-waffle-relay waffle-mcp-relay\"\n\
             waffle_relay_port = {relay_test_port}\n"
        );
        let h = Harness::start(
            "waffle-set",
            HarnessOpts {
                config_toml: Some(cfg_toml),
                ..HarnessOpts::default()
            },
        )
        .await;

        // Project 1 has a waffle link in design/links.toml; project 2 does not.
        std::fs::write(
            h.project_dir.join("design/links.toml"),
            "[[links]]\nid = \"enclosure\"\nkind = \"waffle\"\nboard = \"easy2350\"\n",
        )
        .unwrap();

        // Starting project 2 without {"waffle": true} gets no mcp.json and explains why.
        let (_, p2_body) = h
            .req(
                "POST",
                "/api/term",
                Some(serde_json::json!({"project": h.project2_id, "kind": "project"})),
            )
            .await;
        let p2_term: TermSummary = serde_json::from_slice(&p2_body).unwrap();
        assert!(!p2_term.waffle);
        assert!(!h.term_cache_dir.join(&p2_term.id).join("mcp.json").exists());
        let mut p2_ws = h.connect_term_ws(&p2_term.id, None).await.unwrap();
        recv_until_contains(&mut p2_ws, "no waffle link").await;
        drop(p2_ws);
        h.req("DELETE", &format!("/api/term/{}", p2_term.id), None)
            .await;

        // Starting project 1 writes mcp.json with the relay's exact flags.
        let (_, p1_body) = h
            .req(
                "POST",
                "/api/term",
                Some(serde_json::json!({"project": h.project_id, "kind": "project"})),
            )
            .await;
        let p1_term: TermSummary = serde_json::from_slice(&p1_body).unwrap();
        assert!(p1_term.waffle);

        let mcp_file = h.term_cache_dir.join(&p1_term.id).join("mcp.json");
        assert!(
            mcp_file.is_file(),
            "expected {} to exist",
            mcp_file.display()
        );
        let mcp: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&mcp_file).unwrap()).unwrap();
        assert_eq!(mcp["mcpServers"]["waffle"]["command"], "uv");
        let args: Vec<String> = mcp["mcpServers"]["waffle"]["args"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        assert_eq!(
            args,
            vec![
                "run".to_string(),
                "--project".to_string(),
                "/tmp/fake-waffle-relay".to_string(),
                "waffle-mcp-relay".to_string(),
                "--port".to_string(),
                relay_test_port.to_string(),
                "--app-url".to_string(),
                format!("http://127.0.0.1:{}/apps/waffle/", h.port),
                "--allow-origin".to_string(),
                format!("http://127.0.0.1:{}", h.port),
                "--allow-origin".to_string(),
                format!("http://localhost:{}", h.port),
            ]
        );

        // While p1_term holds the relay port, project 2 started even with {"waffle": true}
        // starts without Waffle tools and says so on its first line.
        let (_, p2b_body) = h
            .req(
                "POST",
                "/api/term",
                Some(serde_json::json!({
                    "project": h.project2_id,
                    "kind": "project",
                    "waffle": true
                })),
            )
            .await;
        let p2b_term: TermSummary = serde_json::from_slice(&p2b_body).unwrap();
        assert!(!p2b_term.waffle);
        assert!(
            !h.term_cache_dir
                .join(&p2b_term.id)
                .join("mcp.json")
                .exists()
        );
        let mut p2b_ws = h.connect_term_ws(&p2b_term.id, None).await.unwrap();
        recv_until_contains(
            &mut p2b_ws,
            &format!("already held by terminal {}", p1_term.id),
        )
        .await;
        drop(p2b_ws);

        // Ending p1_term removes its mcp.json and frees the relay slot.
        h.req("DELETE", &format!("/api/term/{}", p1_term.id), None)
            .await;
        assert!(!mcp_file.exists());
    }

    // 3. When waffle_relay is set but neither waffle_relay_port nor proj port provides a port:
    {
        let h = Harness::start(
            "waffle-no-port",
            HarnessOpts {
                config_toml: Some("waffle_relay = \"waffle-mcp-relay\"\n".into()),
                relay_port_fallback: None,
                ..HarnessOpts::default()
            },
        )
        .await;
        let (_, body) = h
            .req(
                "POST",
                "/api/term",
                Some(serde_json::json!({
                    "project": h.project_id,
                    "kind": "project",
                    "waffle": true
                })),
            )
            .await;
        let term: TermSummary = serde_json::from_slice(&body).unwrap();
        assert!(!term.waffle);
        assert!(!h.term_cache_dir.join(&term.id).join("mcp.json").exists());
        let mut ws = h.connect_term_ws(&term.id, None).await.unwrap();
        recv_until_contains(&mut ws, "no relay port configured").await;
    }
}

#[tokio::test]
async fn claude_override_passes_system_prompt_mcp_and_continue_across_restart() {
    use std::os::unix::fs::PermissionsExt;

    let shared_root = std::env::temp_dir()
        .join(format!("boardroom-term-it-{}", std::process::id()))
        .join("fake-claude-shared");
    let _ = std::fs::remove_dir_all(&shared_root);
    std::fs::create_dir_all(&shared_root).unwrap();

    let state_dir = shared_root.join("state");
    let log_file = shared_root.join("invocations.log");
    let fake_claude = shared_root.join("fake-claude.sh");

    std::fs::write(
        &fake_claude,
        format!(
            "#!/bin/sh\n\
             {{\n\
               printf 'PWD=%s\\n' \"$PWD\"\n\
               printf 'TERM=%s\\n' \"$TERM\"\n\
               for a in \"$@\"; do printf 'ARG=%s\\n' \"$a\"; done\n\
               printf -- '---\\n'\n\
             }} >> '{}'\n\
             printf 'fake-claude-ran\\n'\n",
            log_file.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&fake_claude, std::fs::Permissions::from_mode(0o755)).unwrap();

    let relay_port = allocate_ephemeral_port();
    let cfg_toml = format!(
        "claude = '{}'\n\
         waffle_relay = 'waffle-mcp-relay'\n\
         waffle_relay_port = {relay_port}\n",
        fake_claude.display()
    );

    // First server instance: project has never had a terminal before -> no --continue.
    let h1 = Harness::start(
        "claude-first",
        HarnessOpts {
            config_toml: Some(cfg_toml.clone()),
            term_command: None,
            relay_port_fallback: None,
            state_dir_override: Some(state_dir.clone()),
        },
    )
    .await;
    let canon_proj1 = h1.project_dir.clone();

    let (_, b1) = h1
        .req(
            "POST",
            "/api/term",
            Some(serde_json::json!({
                "project": h1.project_id,
                "kind": "project",
                "waffle": true
            })),
        )
        .await;
    let t1: TermSummary = serde_json::from_slice(&b1).unwrap();
    let mut ws1 = h1.connect_term_ws(&t1.id, None).await.unwrap();
    recv_until_contains(&mut ws1, "fake-claude-ran").await;
    drop(ws1);
    drop(h1);

    assert!(state_dir.join("terminals.json").is_file());

    let log1 = std::fs::read_to_string(&log_file).unwrap();
    assert!(log1.contains(&format!("PWD={}", canon_proj1.display())));
    assert!(log1.contains("TERM=xterm-256color"));
    assert!(!log1.contains("ARG=--continue"));
    assert!(log1.contains("ARG=--mcp-config"));
    assert!(log1.contains("ARG=--append-system-prompt"));
    assert!(log1.contains("this is Boardroom's project terminal; the pcb-design skill applies"));

    // Second server instance with the same state_dir and same project path -> passes --continue.
    // Reuse the exact project path so project_id matches!
    let cfg_dir2 = shared_root.join("cfg2");
    let web_dir2 = shared_root.join("web2");
    std::fs::create_dir_all(&cfg_dir2).unwrap();
    std::fs::create_dir_all(&web_dir2).unwrap();
    std::fs::write(web_dir2.join("index.html"), "<!doctype html>").unwrap();
    std::fs::write(cfg_dir2.join("config.toml"), &cfg_toml).unwrap();
    // Recreate canon_proj1 since dropping h1 removed its temp root.
    std::fs::create_dir_all(canon_proj1.join("design")).unwrap();
    std::fs::write(
        canon_proj1.join("design/status.json"),
        "{\"schema\": 1, \"board\": \"easy2350\", \"phase\": 4}\n",
    )
    .unwrap();
    let mut list2 = ProjectList::default();
    let canon_again = list2.add(&canon_proj1).unwrap();
    let pid2 = config::project_id(&canon_again);

    let (router2, app2) = server::server::router_with_options(
        cfg_dir2,
        list2,
        web_dir2,
        LinksCache::with_dir(shared_root.join("links2")),
        TerminalsOptions {
            state_dir: state_dir.clone(),
            cache_dir: shared_root.join("term2"),
            term_command: None,
            relay_port_resolver: Arc::new(|| None),
        },
    );
    let listener2 = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let port2 = listener2.local_addr().unwrap().port();
    app2.set_port(port2);
    let svc2 = router2.into_make_service_with_connect_info::<SocketAddr>();
    let task2 = tokio::spawn(async move {
        let _ = axum::serve(listener2, svc2).await;
    });

    let client: Client<_, Body> = Client::builder(TokioExecutor::new()).build_http();
    let req = Request::builder()
        .method("POST")
        .uri(format!("http://127.0.0.1:{port2}/api/term"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_vec(&serde_json::json!({"project": pid2, "kind": "project"})).unwrap(),
        ))
        .unwrap();
    let resp = client.request(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let t2_bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let t2: TermSummary = serde_json::from_slice(&t2_bytes).unwrap();

    let (mut ws2, _) =
        tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port2}/api/term/{}", t2.id))
            .await
            .unwrap();
    recv_until_contains(&mut ws2, "fake-claude-ran").await;
    drop(ws2);
    task2.abort();

    let log_both = std::fs::read_to_string(&log_file).unwrap();
    let second_run = log_both.split("---\n").nth(1).unwrap_or("");
    assert!(
        second_run.contains("ARG=--continue"),
        "expected second invocation after restart to pass --continue, got:\n{second_run}"
    );

    let _ = std::fs::remove_dir_all(&canon_proj1);
    let _ = std::fs::remove_dir_all(&shared_root);
}

#[tokio::test]
async fn selection_endpoint_and_cli_return_latest_bus_selection() {
    let h = Harness::start("selection", HarnessOpts::default()).await;

    // Default selection before any bus message is empty.
    let (st0, b0) = h
        .req(
            "GET",
            &format!("/api/selection?project={}", h.project_id),
            None,
        )
        .await;
    assert_eq!(st0, StatusCode::OK);
    let initial: serde_json::Value = serde_json::from_slice(&b0).unwrap();
    assert_eq!(initial["refs"], serde_json::json!([]));

    // Publish a selection event on /api/bus.
    let (mut bus_ws, _) =
        tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{}/api/bus", h.port))
            .await
            .unwrap();
    let event = serde_json::json!({
        "type": "select",
        "from": "test",
        "project": h.project_id,
        "board": "easy2350",
        "refs": ["U1", "C4"],
        "nets": ["GND"]
    });
    bus_ws
        .send(tungstenite::Message::Text(event.to_string().into()))
        .await
        .unwrap();
    // Wait until the bus echoes our message back so we know the server processed it.
    let echoed = tokio::time::timeout(Duration::from_secs(3), bus_ws.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(matches!(echoed, tungstenite::Message::Text(_)));

    // Verify GET /api/selection?project=<id>.
    let (st1, b1) = h
        .req(
            "GET",
            &format!("/api/selection?project={}", h.project_id),
            None,
        )
        .await;
    assert_eq!(st1, StatusCode::OK);
    let got: serde_json::Value = serde_json::from_slice(&b1).unwrap();
    assert_eq!(got["board"], "easy2350");
    assert_eq!(got["refs"], serde_json::json!(["U1", "C4"]));
    assert_eq!(got["nets"], serde_json::json!(["GND"]));

    // Verify `boardroom selection --port <port> --project <dir>` CLI.
    let bin = env!("CARGO_BIN_EXE_boardroom");
    let out = tokio::process::Command::new(bin)
        .args([
            "selection",
            "--port",
            &h.port.to_string(),
            "--project",
            h.project_dir.to_str().unwrap(),
        ])
        .env("XDG_CONFIG_HOME", h.cfg_dir.parent().unwrap())
        .output()
        .await
        .unwrap();
    assert!(
        out.status.success(),
        "boardroom selection failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let cli_val: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(cli_val["refs"], serde_json::json!(["U1", "C4"]));

    // Verify `boardroom selection` fails loudly when no --port, no $PORT, and no `proj` in PATH.
    let fail_out = tokio::process::Command::new(bin)
        .arg("selection")
        .env_remove("PORT")
        .env("PATH", "/nonexistent")
        .output()
        .await
        .unwrap();
    assert!(!fail_out.status.success());
    let stderr = String::from_utf8_lossy(&fail_out.stderr);
    assert!(stderr.contains("no port"), "unexpected stderr: {stderr}");
}
