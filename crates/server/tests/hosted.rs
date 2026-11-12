use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, Uri, header};
use board_interface::{
    Author, Driver, Element, ElementKind, Hole, Loop, Manifest, NM_PER_MM, Point, Record,
    SCHEMA_VERSION, seal, write,
};
use boardroom::config::{self, ProjectList};
use boardroom::links::{
    LinkState, LinkStatus, LinksCache, Locator, Published, PublishedState, PushResult, Unpushed,
    classify_git_error, published_status, remote_cache_key,
};
use http_body_util::BodyExt;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tower::ServiceExt;

static CASE_SEQ: AtomicUsize = AtomicUsize::new(1);

fn temp_case_dir(label: &str) -> PathBuf {
    let n = CASE_SEQ.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir()
        .join(format!("boardroom-hosted-it-{}", std::process::id()))
        .join(format!("{n:02}-{label}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

struct TempGuard(PathBuf);

impl Drop for TempGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct ServerHarness {
    _guard: TempGuard,
    cache_dir: PathBuf,
    port: u16,
    router: axum::Router,
    task: tokio::task::JoinHandle<()>,
}

impl ServerHarness {
    async fn start(label: &str, project_dirs: &[&Path]) -> (Self, Vec<String>) {
        let root = temp_case_dir(label);
        let cfg_dir = root.join("cfg");
        let web_dir = root.join("web");
        let cache_dir = root.join("cache/boardroom/links");
        std::fs::create_dir_all(&cfg_dir).unwrap();
        std::fs::create_dir_all(&web_dir).unwrap();
        std::fs::write(
            web_dir.join("index.html"),
            "<!doctype html><title>Boardroom</title>",
        )
        .unwrap();

        let mut list = ProjectList::default();
        let mut ids = Vec::new();
        for p in project_dirs {
            let canon = list.add(p).unwrap();
            ids.push(config::project_id(&canon));
        }
        list.save_to(&cfg_dir).unwrap();

        let (router, _) = boardroom::server::router_with_cache(
            cfg_dir,
            list,
            web_dir,
            LinksCache::with_dir(cache_dir.clone()),
        );
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let port = listener.local_addr().unwrap().port();
        let svc = router
            .clone()
            .into_make_service_with_connect_info::<SocketAddr>();
        let task = tokio::spawn(async move {
            let _ = axum::serve(listener, svc).await;
        });

        (
            Self {
                _guard: TempGuard(root),
                cache_dir,
                port,
                router,
                task,
            },
            ids,
        )
    }

    async fn req(
        &self,
        method: &str,
        path: &str,
        json_body: Option<serde_json::Value>,
    ) -> (StatusCode, Vec<u8>) {
        let client: Client<_, Body> = Client::builder(TokioExecutor::new()).build_http();
        let uri: Uri = format!("http://127.0.0.1:{}{path}", self.port)
            .parse()
            .unwrap();
        let mut builder = Request::builder().method(method).uri(uri);
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
}

impl Drop for ServerHarness {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn file_url(path: &Path) -> String {
    let canon = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    format!("file://{}", canon.to_str().expect("utf8 temp path"))
}

fn git_in(repo: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .expect("git command");
    assert!(
        out.status.success(),
        "git {args:?} in {} failed: {}",
        repo.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn init_bare_hosted(bare: &Path) {
    std::fs::create_dir_all(bare).unwrap();
    git_in(bare, &["init", "--bare", "-q", "--initial-branch=main"]);
    git_in(bare, &["config", "uploadpack.allowFilter", "true"]);
    git_in(bare, &["config", "uploadpack.allowAnySHA1InWant", "true"]);
}

fn init_work_repo(repo: &Path) {
    std::fs::create_dir_all(repo).unwrap();
    git_in(repo, &["init", "-q", "-b", "main"]);
    git_in(repo, &["config", "user.name", "Boardroom Test"]);
    git_in(repo, &["config", "user.email", "test@example.com"]);
}

fn init_boardroom_project(dir: &Path) {
    std::fs::create_dir_all(dir.join("design")).unwrap();
    std::fs::write(
        dir.join("design/status.json"),
        "{\n  \"schema\": 1,\n  \"board\": \"easy2350\",\n  \"phase\": 4,\n  \"needs_user\": []\n}\n",
    )
    .unwrap();
}

fn commit_all(repo: &Path, message: &str) -> String {
    git_in(repo, &["add", "-A"]);
    git_in(repo, &["commit", "-q", "-m", message]);
    git_in(repo, &["rev-parse", "HEAD"])
}

fn git_blob_bytes(repo: &Path, commit: &str, rel: &str) -> Vec<u8> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["cat-file", "blob", &format!("{commit}:{rel}")])
        .output()
        .unwrap();
    assert!(out.status.success());
    out.stdout
}

fn make_record(
    driver: Driver,
    parent: Option<String>,
    hole_dx_nm: i64,
    thickness_mm: f64,
) -> Record {
    let w = 50 * NM_PER_MM;
    let h = 30 * NM_PER_MM;
    let outline = Loop::new(vec![
        Element::new(
            Some("waffle:s1/e1".into()),
            ElementKind::Line {
                start: Point::new(0, 0),
                end: Point::new(w, 0),
            },
        ),
        Element::new(
            Some("waffle:s1/e2".into()),
            ElementKind::Line {
                start: Point::new(w, 0),
                end: Point::new(w, h),
            },
        ),
        Element::new(
            Some("waffle:s1/e3".into()),
            ElementKind::Line {
                start: Point::new(w, h),
                end: Point::new(0, h),
            },
        ),
        Element::new(
            Some("waffle:s1/e4".into()),
            ElementKind::Line {
                start: Point::new(0, h),
                end: Point::new(0, 0),
            },
        ),
    ]);
    let holes = vec![Hole {
        id: Some("waffle:s1/h1".into()),
        center: Point::new(10 * NM_PER_MM + hole_dx_nm, 10 * NM_PER_MM),
        diameter_nm: 3_200_000,
        plated: true,
        clearance_nm: 200_000,
    }];
    let mut rec = Record {
        outline,
        cutouts: vec![],
        holes,
        keepouts: vec![],
        manifest: Manifest {
            schema: SCHEMA_VERSION,
            board: "easy2350".into(),
            revision: String::new(),
            parent,
            author: Author {
                app: driver,
                source: "commit-test".into(),
                at: "2026-11-06T10:00:00Z".into(),
            },
            driver,
            last_handover: None,
            thickness_mm,
            geometry_hash: String::new(),
            approximate: false,
        },
    };
    seal(&mut rec);
    rec
}

fn write_interface(repo: &Path, rec: &Record) {
    let dir = repo.join("interface/easy2350");
    std::fs::create_dir_all(&dir).unwrap();
    let (dxf, json) = write(rec);
    std::fs::write(dir.join("board-shape.dxf"), dxf).unwrap();
    std::fs::write(dir.join("board-shape.json"), json).unwrap();
}

#[tokio::test]
async fn partial_mirror_over_file_url_materializes_and_detects_pushed_update() {
    let tmp = temp_case_dir("partial-setup");
    let _tmp_guard = TempGuard(tmp.clone());

    let hosted = tmp.join("enclosure-hosted.git");
    init_bare_hosted(&hosted);
    let hosted_url = file_url(&hosted);

    let author = tmp.join("enclosure-author");
    init_work_repo(&author);
    git_in(&author, &["remote", "add", "origin", &hosted_url]);

    let r1 = make_record(Driver::Waffle, None, 0, 1.6);
    write_interface(&author, &r1);
    // Include an unrelated blob so we can confirm the mirror really stays blob-less for non-interface files.
    std::fs::write(author.join("heavy.stl"), vec![b'X'; 4096]).unwrap();
    let c1 = commit_all(&author, "Initial enclosure outline");
    let heavy_oid = git_in(&author, &["rev-parse", "HEAD:heavy.stl"]);
    git_in(&author, &["push", "-q", "-u", "origin", "main"]);

    let proj = tmp.join("board-proj");
    init_boardroom_project(&proj);
    let (h, ids) = ServerHarness::start("partial-mirror", &[&proj]).await;
    let pid = &ids[0];

    let payload = serde_json::json!({
        "id": "enclosure",
        "kind": "waffle",
        "board": "easy2350",
        "remote": hosted_url,
        "path": "interface/easy2350/board-shape.json",
        "ref": { "type": "Branch", "name": "main" }
    });
    let (st, body) = h
        .req("POST", &format!("/api/projects/{pid}/links"), Some(payload))
        .await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    let added: LinkStatus = serde_json::from_slice(&body).unwrap();
    assert_eq!(added.state, LinkState::Current);
    assert!(!added.local_only);
    assert_eq!(added.hosted_remote, None);
    assert_eq!(added.resolved.commit, c1);

    // Mirror config has partialclonefilter = blob:none, and unrelated blob was never fetched!
    let key = remote_cache_key(&proj, &hosted_url).unwrap();
    let mirror_dir = h.cache_dir.join(&key);
    assert_eq!(
        git_in(
            &mirror_dir,
            &["config", "--get", "remote.origin.partialclonefilter"]
        ),
        "blob:none"
    );
    let has_heavy = Command::new("git")
        .arg("--no-lazy-fetch")
        .arg("-C")
        .arg(&mirror_dir)
        .args(["cat-file", "-e", &heavy_oid])
        .status()
        .unwrap()
        .success();
    assert!(
        !has_heavy,
        "partial clone mirror should not eagerly download non-interface blobs"
    );

    // Materialized bytes match git's exact bytes (lazily fetched via cat-file).
    let mat_dir = std::fs::canonicalize(&proj)
        .unwrap()
        .join(".boardroom/links/enclosure");
    assert_eq!(
        std::fs::read(mat_dir.join("board-shape.dxf")).unwrap(),
        git_blob_bytes(&hosted, &c1, "interface/easy2350/board-shape.dxf")
    );
    assert_eq!(
        std::fs::read(mat_dir.join("board-shape.json")).unwrap(),
        git_blob_bytes(&hosted, &c1, "interface/easy2350/board-shape.json")
    );

    // Push a new interface commit to the bare hosted repo -> UpdateAvailable after refresh.
    let r2 = make_record(
        Driver::Waffle,
        Some(r1.manifest.revision.clone()),
        1_500_000,
        1.6,
    );
    write_interface(&author, &r2);
    let c2 = commit_all(&author, "Move mounting hole");
    git_in(&author, &["push", "-q", "origin", "main"]);

    let (st, body) = h
        .req("GET", &format!("/api/projects/{pid}/links?refresh=1"), None)
        .await;
    assert_eq!(st, StatusCode::OK);
    let list: Vec<LinkStatus> = serde_json::from_slice(&body).unwrap();
    assert_eq!(list[0].state, LinkState::UpdateAvailable);
    assert_eq!(list[0].resolved.commit, c1);
    assert_eq!(list[0].latest.as_ref().unwrap().commit, c2);

    // Accepting the update lazily fetches c2's blobs into the partial mirror and materializes them.
    let (st, body) = h
        .req(
            "POST",
            &format!("/api/projects/{pid}/links/enclosure/update"),
            None,
        )
        .await;
    assert_eq!(st, StatusCode::OK);
    let updated: LinkStatus = serde_json::from_slice(&body).unwrap();
    assert_eq!(updated.state, LinkState::Current);
    assert_eq!(updated.resolved.commit, c2);
    assert_eq!(
        std::fs::read(mat_dir.join("board-shape.dxf")).unwrap(),
        git_blob_bytes(&hosted, &c2, "interface/easy2350/board-shape.dxf")
    );
}

#[tokio::test]
async fn unavailable_nonexistent_file_url_reports_error_and_unit_helpers_hold() {
    let tmp = temp_case_dir("unavailable-file-url");
    let _tmp_guard = TempGuard(tmp.clone());

    // Cache key spellings share one key.
    let root = Path::new("/tmp/dummy");
    let k1 = remote_cache_key(root, "https://github.com/a/b").unwrap();
    let k2 = remote_cache_key(root, "https://github.com/a/b.git").unwrap();
    let k3 = remote_cache_key(root, "git@github.com:a/b.git").unwrap();
    assert_eq!(k1, k2);
    assert_eq!(k1, k3);

    // classify_git_error sanity checks across auth & network.
    assert_eq!(
        classify_git_error(
            "https://github.com/a/b.git",
            "fatal: Authentication failed for 'https://github.com/a/b.git/'"
        )
        .as_deref(),
        Some("Sign in to github.com: run `gh auth login`, then refresh.")
    );
    assert_eq!(
        classify_git_error(
            "git@github.com:a/b.git",
            "git@github.com: Permission denied (publickey)."
        )
        .as_deref(),
        Some("Check your SSH key for github.com: `ssh -T git@github.com`.")
    );
    assert_eq!(
        classify_git_error(
            "https://github.com/a/b.git",
            "Could not resolve host: github.com"
        )
        .as_deref(),
        Some("github.com could not be reached. The accepted copy keeps working.")
    );

    // Link pointing at a nonexistent file:// repository reports Unavailable with an error.
    let proj = tmp.join("board-proj");
    init_boardroom_project(&proj);
    let missing_url = format!("file://{}/does-not-exist.git", tmp.display());
    let fake_commit = "0123456789abcdef0123456789abcdef01234567";
    std::fs::write(
        proj.join("design/links.toml"),
        format!(
            "[[links]]\n\
             id = \"enclosure\"\n\
             kind = \"waffle\"\n\
             board = \"easy2350\"\n\
             locator = {{ type = \"Git\", remote = {:?}, path = \"interface/easy2350/board-shape.json\", ref = {{ type = \"Branch\", name = \"main\" }} }}\n\
             resolved = {{ commit = {:?}, at = \"2026-11-06T10:00:00Z\" }}\n\
             interface_hash = \"\"\n\
             pin_reason = \"\"\n",
            missing_url, fake_commit
        ),
    )
    .unwrap();

    let (h, ids) = ServerHarness::start("unavailable-net", &[&proj]).await;
    let (st, body) = h
        .req("GET", &format!("/api/projects/{}/links", ids[0]), None)
        .await;
    assert_eq!(st, StatusCode::OK);
    let list: Vec<LinkStatus> = serde_json::from_slice(&body).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].state, LinkState::Unavailable);
    assert!(!list[0].local_only);
    assert!(list[0].error.as_deref().is_some_and(|e| !e.is_empty()));
    let first_err = list[0].error.clone().unwrap();

    // The repository appears, but an unforced check within a minute of the failure answers from
    // the remembered failure instead of going back to the network.
    init_bare_hosted(&tmp.join("does-not-exist.git"));
    let (_, body) = h
        .req("GET", &format!("/api/projects/{}/links", ids[0]), None)
        .await;
    let list: Vec<LinkStatus> = serde_json::from_slice(&body).unwrap();
    assert_eq!(list[0].error.as_deref(), Some(first_err.as_str()));

    // A forced refresh tries again: the clone now succeeds, and the link fails later, on the
    // made-up resolved commit, with a different error.
    let (_, body) = h
        .req(
            "GET",
            &format!("/api/projects/{}/links?refresh=1", ids[0]),
            None,
        )
        .await;
    let list: Vec<LinkStatus> = serde_json::from_slice(&body).unwrap();
    assert_eq!(list[0].state, LinkState::Unavailable);
    assert_ne!(list[0].error.as_deref(), Some(first_err.as_str()));
}

#[tokio::test]
async fn rehost_refuses_until_accepted_commit_pushed_and_preserves_toml_comments() {
    let tmp = temp_case_dir("rehost");
    let _tmp_guard = TempGuard(tmp.clone());

    let hosted = tmp.join("waffle-hosted.git");
    init_bare_hosted(&hosted);
    // Keep `.git` suffix so we verify `hosted_remote` is reported as written, not normalized.
    let hosted_url = file_url(&hosted);
    assert!(hosted_url.ends_with(".git"));

    let local_waffle = tmp.join("waffle-local");
    init_work_repo(&local_waffle);
    git_in(&local_waffle, &["remote", "add", "origin", &hosted_url]);

    // Push c1 to hosted, then commit c2 locally without pushing yet.
    let r1 = make_record(Driver::Waffle, None, 0, 1.6);
    write_interface(&local_waffle, &r1);
    let _c1 = commit_all(&local_waffle, "Initial pushed shape");
    git_in(&local_waffle, &["push", "-q", "-u", "origin", "main"]);

    let r2 = make_record(
        Driver::Waffle,
        Some(r1.manifest.revision.clone()),
        700_000,
        1.6,
    );
    write_interface(&local_waffle, &r2);
    let c2 = commit_all(&local_waffle, "Local-only shape edit");

    let proj = tmp.join("board-proj");
    init_boardroom_project(&proj);
    let local_canon = std::fs::canonicalize(&local_waffle).unwrap();
    let toml_path = proj.join("design/links.toml");
    let hand_written = format!(
        "# Header comment preserved by rehost\n\
         top_extra = \"preserved\"\n\n\
         [[links]]\n\
         # Link comment preserved\n\
         id = \"enclosure\"\n\
         kind = \"waffle\"\n\
         board = \"easy2350\"\n\
         custom_field = 99\n\
         locator = {{ type = \"Git\", remote = {:?}, path = \"interface/easy2350/board-shape.json\", ref = {{ type = \"Branch\", name = \"main\" }}, custom_loc = \"yes\" }}\n\
         resolved = {{ commit = {:?}, at = \"2026-11-06T10:05:00Z\" }}\n\
         interface_hash = {:?} # inline comment kept\n\
         pin_reason = \"\"\n",
        local_canon.to_str().unwrap(),
        c2,
        r2.manifest.geometry_hash
    );
    std::fs::write(&toml_path, &hand_written).unwrap();

    let (h, ids) = ServerHarness::start("rehost-flow", &[&proj]).await;
    let pid = &ids[0];

    // 1. GET /links reports `local_only = true` and `hosted_remote = Some(hosted_url)`.
    let (st, body) = h
        .req("GET", &format!("/api/projects/{pid}/links"), None)
        .await;
    assert_eq!(st, StatusCode::OK);
    let list: Vec<LinkStatus> = serde_json::from_slice(&body).unwrap();
    assert_eq!(list[0].state, LinkState::Current);
    assert!(list[0].local_only);
    assert_eq!(list[0].hosted_remote.as_deref(), Some(hosted_url.as_str()));

    // 2. Rehost before pushing `c2` is refused with 400 naming the short commit and hosted URL.
    let (st, body) = h
        .req(
            "POST",
            &format!("/api/projects/{pid}/links/enclosure/rehost"),
            None,
        )
        .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    let refusal = String::from_utf8_lossy(&body);
    let expected_refusal = format!(
        "commit {} is not on {hosted_url} yet: push it first",
        &c2[..7]
    );
    assert_eq!(refusal, expected_refusal);
    assert_eq!(std::fs::read_to_string(&toml_path).unwrap(), hand_written);

    // 3. Push `c2` to hosted -> rehost succeeds and preserves comments & unknown keys.
    git_in(&local_waffle, &["push", "-q", "origin", "main"]);
    let (st, body) = h
        .req(
            "POST",
            &format!("/api/projects/{pid}/links/enclosure/rehost"),
            None,
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    let rehosted: LinkStatus = serde_json::from_slice(&body).unwrap();
    assert_eq!(rehosted.state, LinkState::Current);
    assert!(!rehosted.local_only);
    assert_eq!(rehosted.hosted_remote, None);
    let Locator::Git { remote, .. } = &rehosted.locator;
    assert_eq!(remote, &hosted_url);

    let after_toml = std::fs::read_to_string(&toml_path).unwrap();
    for needle in [
        "# Header comment preserved by rehost",
        "top_extra = \"preserved\"",
        "# Link comment preserved",
        "custom_field = 99",
        "custom_loc = \"yes\"",
        "# inline comment kept",
        &hosted_url,
    ] {
        assert!(
            after_toml.contains(needle),
            "missing {needle:?} in:\n{after_toml}"
        );
    }
    assert!(!after_toml.contains(local_canon.to_str().unwrap()));

    // 4. Also verify CLI `boardroom link rehost` works end-to-end on a second project.
    let proj2 = tmp.join("board-proj-cli");
    init_boardroom_project(&proj2);
    let bin = env!("CARGO_BIN_EXE_boardroom");
    let out = Command::new(bin)
        .env("XDG_CACHE_HOME", tmp.join("cli-cache"))
        .args([
            "link",
            "add",
            proj2.to_str().unwrap(),
            "--id",
            "enclosure",
            "--board",
            "easy2350",
            "--remote",
            local_canon.to_str().unwrap(),
            "--path",
            "interface/easy2350/board-shape.json",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let out = Command::new(bin)
        .env("XDG_CACHE_HOME", tmp.join("cli-cache"))
        .args(["link", "rehost", proj2.to_str().unwrap(), "enclosure"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let recs = boardroom::links::read_links_file(&proj2).unwrap();
    let Locator::Git {
        remote: cli_remote, ..
    } = &recs[0].locator;
    assert_eq!(cli_remote, &hosted_url);
}

#[tokio::test]
async fn unpushed_reports_local_candidate_commits_and_dirty_files_never_accepting_them() {
    let tmp = temp_case_dir("unpushed");
    let _tmp_guard = TempGuard(tmp.clone());

    let hosted = tmp.join("waffle-hosted.git");
    init_bare_hosted(&hosted);
    let hosted_url = file_url(&hosted);

    let seed = tmp.join("seed");
    init_work_repo(&seed);
    git_in(&seed, &["remote", "add", "origin", &hosted_url]);
    let r1 = make_record(Driver::Waffle, None, 0, 1.6);
    write_interface(&seed, &r1);
    let c1 = commit_all(&seed, "Initial shape");
    git_in(&seed, &["push", "-q", "-u", "origin", "main"]);

    // Candidate local clone registered alongside the linking project.
    let cand = tmp.join("enclosure-checkout");
    let hosted_no_dotgit = hosted_url.trim_end_matches(".git").to_string();
    Command::new("git")
        .args(["clone", "-q", &hosted_no_dotgit, cand.to_str().unwrap()])
        .status()
        .unwrap();
    git_in(&cand, &["config", "user.name", "Waffle Local"]);
    git_in(&cand, &["config", "user.email", "local@example.com"]);
    init_boardroom_project(&cand);
    let cand_canon = std::fs::canonicalize(&cand).unwrap();

    let proj = tmp.join("board-proj");
    init_boardroom_project(&proj);
    let (h, ids) = ServerHarness::start("unpushed-srv", &[&proj, &cand]).await;
    let pid = &ids[0];

    let payload = serde_json::json!({
        "id": "enclosure",
        "kind": "waffle",
        "board": "easy2350",
        "remote": hosted_url,
        "path": "interface/easy2350/board-shape.json",
        "ref": { "type": "Branch", "name": "main" }
    });
    let (st, body) = h
        .req("POST", &format!("/api/projects/{pid}/links"), Some(payload))
        .await;
    assert_eq!(st, StatusCode::OK);
    let initial: LinkStatus = serde_json::from_slice(&body).unwrap();
    assert_eq!(initial.unpushed, None);

    // Unrelated commit in candidate does NOT trigger `unpushed`.
    std::fs::write(cand.join("README.md"), "notes\n").unwrap();
    commit_all(&cand, "Unrelated doc change");
    let (_, body) = h
        .req("GET", &format!("/api/projects/{pid}/links"), None)
        .await;
    let list: Vec<LinkStatus> = serde_json::from_slice(&body).unwrap();
    assert_eq!(list[0].unpushed, None);

    // Local commit in candidate touching the interface -> reported with commits = 1, uncommitted = false.
    let r2 = make_record(
        Driver::Waffle,
        Some(r1.manifest.revision.clone()),
        900_000,
        1.6,
    );
    write_interface(&cand, &r2);
    let c2_local = commit_all(&cand, "Local unpushed outline change");

    let (_, body) = h
        .req("GET", &format!("/api/projects/{pid}/links"), None)
        .await;
    let list: Vec<LinkStatus> = serde_json::from_slice(&body).unwrap();
    assert_eq!(list[0].state, LinkState::Current);
    assert_eq!(list[0].resolved.commit, c1);
    assert_eq!(
        list[0].unpushed,
        Some(Unpushed {
            project_path: cand_canon.clone(),
            branch: "main".into(),
            commits: 1,
            uncommitted: false,
        })
    );

    // Calling update NEVER accepts an unpushed commit from a local checkout: resolved stays c1.
    let (st, body) = h
        .req(
            "POST",
            &format!("/api/projects/{pid}/links/enclosure/update"),
            None,
        )
        .await;
    assert_eq!(st, StatusCode::OK);
    let after_update: LinkStatus = serde_json::from_slice(&body).unwrap();
    assert_eq!(after_update.resolved.commit, c1);
    assert_ne!(after_update.resolved.commit, c2_local);

    // Push candidate to origin -> unpushed clears to None, and refresh sees UpdateAvailable at c2_local.
    git_in(&cand, &["push", "-q", "origin", "main"]);
    let (_, body) = h
        .req("GET", &format!("/api/projects/{pid}/links?refresh=1"), None)
        .await;
    let list: Vec<LinkStatus> = serde_json::from_slice(&body).unwrap();
    assert_eq!(list[0].unpushed, None);
    assert_eq!(list[0].state, LinkState::UpdateAvailable);
    assert_eq!(list[0].latest.as_ref().unwrap().commit, c2_local);

    // Dirty interface file in candidate -> reported with uncommitted = true.
    let r3 = make_record(
        Driver::Waffle,
        Some(r2.manifest.revision.clone()),
        1_800_000,
        1.6,
    );
    write_interface(&cand, &r3);
    let (_, body) = h
        .req("GET", &format!("/api/projects/{pid}/links"), None)
        .await;
    let list: Vec<LinkStatus> = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        list[0].unpushed,
        Some(Unpushed {
            project_path: cand_canon,
            branch: "main".into(),
            commits: 0,
            uncommitted: true,
        })
    );
}

#[tokio::test]
async fn published_status_states_and_read_route() {
    let tmp = temp_case_dir("published");
    let _tmp_guard = TempGuard(tmp.clone());

    let hosted = tmp.join("board-hosted.git");
    init_bare_hosted(&hosted);
    let hosted_url = file_url(&hosted);

    let proj = tmp.join("board-proj");
    init_boardroom_project(&proj);
    let r1 = make_record(Driver::Composer, None, 0, 1.6);
    write_interface(&proj, &r1);

    // 1. Not a git repo yet -> NotInGit.
    let items = published_status(&proj);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].board, "easy2350");
    assert_eq!(items[0].path, "interface/easy2350/board-shape.json");
    assert_eq!(items[0].state, PublishedState::NotInGit);
    assert_eq!(
        items[0].revision.as_deref(),
        Some(r1.manifest.revision.as_str())
    );

    // 2. Git initialized, file still untracked and never committed -> NotInGit.
    init_work_repo(&proj);
    std::fs::write(proj.join(".gitignore"), ".boardroom/\n").unwrap();
    git_in(&proj, &["add", "design/status.json", ".gitignore"]);
    git_in(&proj, &["commit", "-q", "-m", "Init project"]);
    assert_eq!(published_status(&proj)[0].state, PublishedState::NotInGit);

    // 3. Commit interface with no upstream branch -> NoUpstream.
    let c1 = commit_all(&proj, "Publish initial board shape");
    let items = published_status(&proj);
    assert_eq!(items[0].state, PublishedState::NoUpstream);
    assert_eq!(items[0].last_commit.as_deref(), Some(c1.as_str()));
    assert_eq!(items[0].upstream, None);
    assert_eq!(items[0].commits_not_pushed, 0);

    // 4. Push with `-u origin main` -> Pushed.
    git_in(&proj, &["remote", "add", "origin", &hosted_url]);
    git_in(&proj, &["push", "-q", "-u", "origin", "main"]);
    let items = published_status(&proj);
    assert_eq!(items[0].state, PublishedState::Pushed);
    assert_eq!(items[0].upstream.as_deref(), Some("origin/main"));
    assert_eq!(items[0].upstream_commit.as_deref(), Some(c1.as_str()));
    assert_eq!(
        items[0].upstream_revision.as_deref(),
        Some(r1.manifest.revision.as_str())
    );
    assert_eq!(items[0].commits_not_pushed, 0);

    // 5. Modify working-tree interface without committing -> Uncommitted.
    let r2 = make_record(
        Driver::Composer,
        Some(r1.manifest.revision.clone()),
        600_000,
        1.6,
    );
    write_interface(&proj, &r2);
    let items = published_status(&proj);
    assert_eq!(items[0].state, PublishedState::Uncommitted);
    assert_eq!(
        items[0].revision.as_deref(),
        Some(r2.manifest.revision.as_str())
    );
    assert_eq!(
        items[0].upstream_revision.as_deref(),
        Some(r1.manifest.revision.as_str())
    );

    // 6. Commit locally without pushing -> NotPushed (commits_not_pushed = 1).
    let c2 = commit_all(&proj, "Publish second board shape");
    let items = published_status(&proj);
    assert_eq!(items[0].state, PublishedState::NotPushed);
    assert_eq!(items[0].commits_not_pushed, 1);
    assert_eq!(items[0].last_commit.as_deref(), Some(c2.as_str()));
    assert_eq!(items[0].upstream_commit.as_deref(), Some(c1.as_str()));
    assert_eq!(
        items[0].upstream_revision.as_deref(),
        Some(r1.manifest.revision.as_str())
    );

    // 7. GET /api/projects/{id}/published is readable from both loopback and non-loopback peers.
    let (h, ids) = ServerHarness::start("published-http", &[&proj]).await;
    let pid = &ids[0];
    let (st, body) = h
        .req("GET", &format!("/api/projects/{pid}/published"), None)
        .await;
    assert_eq!(st, StatusCode::OK);
    let via_http: Vec<Published> = serde_json::from_slice(&body).unwrap();
    assert_eq!(via_http, items);

    let remote_peer = SocketAddr::from(([192, 0, 2, 77], 54321));
    let mut req = Request::builder()
        .method("GET")
        .uri(format!("/api/projects/{pid}/published"))
        .body(Body::empty())
        .unwrap();
    req.extensions_mut().insert(ConnectInfo(remote_peer));
    let resp = h.router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn push_route_loopback_guard_success_bus_and_non_fast_forward_409() {
    let tmp = temp_case_dir("push-route");
    let _tmp_guard = TempGuard(tmp.clone());

    let hosted = tmp.join("board-hosted.git");
    init_bare_hosted(&hosted);
    let hosted_url = file_url(&hosted);

    let proj = tmp.join("board-proj");
    init_boardroom_project(&proj);
    init_work_repo(&proj);
    let r1 = make_record(Driver::Composer, None, 0, 1.6);
    write_interface(&proj, &r1);
    let _c1 = commit_all(&proj, "Initial outline");

    let (h, ids) = ServerHarness::start("push-srv", &[&proj]).await;
    let pid = &ids[0];

    // 1. Non-loopback peer is rejected with 403 FORBIDDEN on both `push` and `rehost`.
    let remote_peer = SocketAddr::from(([192, 0, 2, 42], 51234));
    for uri in [
        format!("/api/projects/{pid}/push"),
        format!("/api/projects/{pid}/links/enclosure/rehost"),
    ] {
        let mut req = Request::builder()
            .method("POST")
            .uri(&uri)
            .body(Body::empty())
            .unwrap();
        req.extensions_mut().insert(ConnectInfo(remote_peer));
        let resp = h.router.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN, "POST {uri}");
    }

    // 2. Loopback push with no upstream configured -> 400 BAD_REQUEST.
    let (st, body) = h
        .req("POST", &format!("/api/projects/{pid}/push"), None)
        .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert!(String::from_utf8_lossy(&body).contains("no upstream"));

    // Configure upstream and push initial commit, then test detached HEAD -> 400 BAD_REQUEST.
    git_in(&proj, &["remote", "add", "origin", &hosted_url]);
    git_in(&proj, &["push", "-q", "-u", "origin", "main"]);
    git_in(&proj, &["checkout", "-q", "--detach", "HEAD"]);
    let (st, body) = h
        .req("POST", &format!("/api/projects/{pid}/push"), None)
        .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert!(String::from_utf8_lossy(&body).contains("detached HEAD"));
    git_in(&proj, &["checkout", "-q", "main"]);

    // 3. Subscribe to `/api/bus`, make a new local commit, and push via POST /api/projects/{id}/push.
    let mut tcp = tokio::net::TcpStream::connect(("127.0.0.1", h.port))
        .await
        .unwrap();
    let handshake = format!(
        "GET /api/bus HTTP/1.1\r\n\
         Host: 127.0.0.1:{}\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
         Sec-WebSocket-Version: 13\r\n\r\n",
        h.port
    );
    tcp.write_all(handshake.as_bytes()).await.unwrap();
    let mut hdr_buf = vec![0u8; 1024];
    let mut filled = 0;
    loop {
        let n = tcp.read(&mut hdr_buf[filled..]).await.unwrap();
        assert!(n > 0);
        filled += n;
        if hdr_buf[..filled].windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
    }
    assert!(String::from_utf8_lossy(&hdr_buf[..filled]).starts_with("HTTP/1.1 101"));

    let r2 = make_record(
        Driver::Composer,
        Some(r1.manifest.revision.clone()),
        400_000,
        1.6,
    );
    write_interface(&proj, &r2);
    let c2 = commit_all(&proj, "Second outline commit");
    assert_eq!(published_status(&proj)[0].state, PublishedState::NotPushed);

    let (st, body) = h
        .req("POST", &format!("/api/projects/{pid}/push"), None)
        .await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    let pushed: PushResult = serde_json::from_slice(&body).unwrap();
    assert_eq!(pushed.remote, "origin");
    assert_eq!(pushed.branch, "main");
    assert_eq!(git_in(&hosted, &["rev-parse", "refs/heads/main"]), c2);
    assert_eq!(published_status(&proj)[0].state, PublishedState::Pushed);

    // Bus event emitted for the project.
    let frame = tokio::time::timeout(std::time::Duration::from_secs(3), async {
        let mut head = [0u8; 2];
        tcp.read_exact(&mut head).await.unwrap();
        let len = (head[1] & 0x7f) as usize;
        let payload_len = if len == 126 {
            let mut ext = [0u8; 2];
            tcp.read_exact(&mut ext).await.unwrap();
            u16::from_be_bytes(ext) as usize
        } else {
            len
        };
        let mut payload = vec![0u8; payload_len];
        tcp.read_exact(&mut payload).await.unwrap();
        String::from_utf8(payload).unwrap()
    })
    .await
    .expect("expected bus message on push");
    let bus_json: serde_json::Value = serde_json::from_str(&frame).unwrap();
    assert_eq!(bus_json["project"], *pid);

    // 4. Non-fast-forward: advance bare upstream from a second clone, commit locally, push -> 409!
    let other_clone = tmp.join("other-clone");
    Command::new("git")
        .args(["clone", "-q", &hosted_url, other_clone.to_str().unwrap()])
        .status()
        .unwrap();
    git_in(&other_clone, &["config", "user.name", "Other Machine"]);
    git_in(&other_clone, &["config", "user.email", "other@example.com"]);
    std::fs::write(other_clone.join("other.txt"), "diverged upstream\n").unwrap();
    let c_upstream = commit_all(&other_clone, "Upstream advances");
    git_in(&other_clone, &["push", "-q", "origin", "main"]);

    std::fs::write(proj.join("local.txt"), "diverged local\n").unwrap();
    let _c_local = commit_all(&proj, "Local diverges");

    let (st, body) = h
        .req("POST", &format!("/api/projects/{pid}/push"), None)
        .await;
    assert_eq!(st, StatusCode::CONFLICT);
    let stderr = String::from_utf8_lossy(&body);
    assert!(
        stderr.contains("rejected") || stderr.contains("non-fast-forward"),
        "expected git rejection stderr unchanged, got:\n{stderr}"
    );
    // Upstream was never force-pushed!
    assert_eq!(
        git_in(&hosted, &["rev-parse", "refs/heads/main"]),
        c_upstream
    );
}
