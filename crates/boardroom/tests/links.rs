use axum::body::Body;
use axum::http::{Request, StatusCode, Uri, header};
use board_interface::{
    Author, Change, ChangeKind, Classification, Driver, Element, ElementKind, Handover, Hole, Loop,
    Manifest, NM_PER_MM, Point, Record, Role, SCHEMA_VERSION, seal, write,
};
use ::project::config::{self, ProjectList};
use ::project::links::{LinkState, LinkStatus, LinksCache, sha256_hex};
use http_body_util::BodyExt;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

static CASE_SEQ: AtomicUsize = AtomicUsize::new(1);

struct Harness {
    dir: PathBuf,
    waffle_repo: PathBuf,
    project_dir: PathBuf,
    project_id: String,
    cache_dir: PathBuf,
    port: u16,
    task: tokio::task::JoinHandle<()>,
}

impl Harness {
    async fn start(label: &str) -> Self {
        let n = CASE_SEQ.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir()
            .join(format!("boardroom-links-it-{}", std::process::id()))
            .join(format!("{n:02}-{label}"));
        let _ = std::fs::remove_dir_all(&root);

        let cfg_dir = root.join("cfg");
        let web_dir = root.join("web");
        let cache_dir = root.join("cache/boardroom/links");
        let waffle_repo = root.join("waffle");
        let project_dir = root.join("board-proj");

        for d in [
            &cfg_dir,
            &web_dir,
            &waffle_repo,
            &project_dir.join("design"),
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

        init_git_repo(&waffle_repo);

        let mut list = ProjectList::default();
        let canon_proj = list.add(&project_dir).unwrap();
        list.save_to(&cfg_dir).unwrap();
        let project_id = config::project_id(&canon_proj);

        let (router, _) = server::server::router_with_cache(
            cfg_dir,
            list,
            web_dir,
            LinksCache::with_dir(cache_dir.clone()),
        );
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let port = listener.local_addr().unwrap().port();
        let svc = router.into_make_service_with_connect_info::<SocketAddr>();
        let task = tokio::spawn(async move {
            let _ = axum::serve(listener, svc).await;
        });

        Self {
            dir: root,
            waffle_repo,
            project_dir: canon_proj,
            project_id,
            cache_dir,
            port,
            task,
        }
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

    async fn get_links(&self, refresh: bool) -> Vec<LinkStatus> {
        let q = if refresh { "?refresh=1" } else { "" };
        let (st, body) = self
            .req(
                "GET",
                &format!("/api/projects/{}/links{q}", self.project_id),
                None,
            )
            .await;
        assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
        serde_json::from_slice(&body).unwrap()
    }

    async fn add_link_default(&self) -> LinkStatus {
        let payload = serde_json::json!({
            "id": "enclosure",
            "kind": "waffle",
            "board": "easy2350",
            "remote": self.waffle_repo.to_str().unwrap(),
            "path": "interface/easy2350/board-shape.json",
            "ref": { "type": "Branch", "name": "main" }
        });
        let (st, body) = self
            .req(
                "POST",
                &format!("/api/projects/{}/links", self.project_id),
                Some(payload),
            )
            .await;
        assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
        serde_json::from_slice(&body).unwrap()
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        self.task.abort();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
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
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
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

fn init_git_repo(repo: &Path) {
    git_in(repo, &["init", "-q", "-b", "main"]);
    git_in(repo, &["config", "user.name", "Waffle Test"]);
    git_in(repo, &["config", "user.email", "waffle@example.com"]);
}

fn commit_all(repo: &Path, message: &str) -> String {
    git_in(repo, &["add", "."]);
    git_in(repo, &["commit", "-q", "-m", message]);
    git_in(repo, &["rev-parse", "HEAD"])
}

fn make_record(
    driver: Driver,
    parent: Option<String>,
    hole3_dx_nm: i64,
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
    let holes = vec![
        Hole {
            id: Some("waffle:s1/h1".into()),
            center: Point::new(4 * NM_PER_MM, 4 * NM_PER_MM),
            diameter_nm: 3_200_000,
            plated: true,
            clearance_nm: 200_000,
        },
        Hole {
            id: Some("waffle:s1/h3".into()),
            center: Point::new(46 * NM_PER_MM + hole3_dx_nm, 26 * NM_PER_MM),
            diameter_nm: 3_200_000,
            plated: true,
            clearance_nm: 200_000,
        },
    ];
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
                at: "2026-11-06T01:00:00Z".into(),
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
async fn link_current_and_materialization_matches_git_bytes() {
    let h = Harness::start("current").await;
    let r1 = make_record(Driver::Composer, None, 0, 1.6);
    write_interface(&h.waffle_repo, &r1);
    let c1 = commit_all(&h.waffle_repo, "Initial board shape");

    let status = h.add_link_default().await;
    assert_eq!(status.state, LinkState::Current);
    assert_eq!(status.locked_by, None);
    assert_eq!(status.resolved.commit, c1);
    assert!(status.changes.is_empty());

    // Bare mirror exists under `<cache_dir>/<sha256 of remote>/`.
    let expected_mirror = h
        .cache_dir
        .join(sha256_hex(h.waffle_repo.to_str().unwrap()));
    assert!(expected_mirror.join("HEAD").is_file());

    // Materialized files match git's exact bytes.
    let mat_dir = h.project_dir.join(".boardroom/links/enclosure");
    assert_eq!(
        status.following_path,
        mat_dir.join("board-shape.json").display().to_string()
    );
    assert_eq!(
        std::fs::read(mat_dir.join("board-shape.dxf")).unwrap(),
        git_blob_bytes(&h.waffle_repo, &c1, "interface/easy2350/board-shape.dxf")
    );
    assert_eq!(
        std::fs::read(mat_dir.join("board-shape.json")).unwrap(),
        git_blob_bytes(&h.waffle_repo, &c1, "interface/easy2350/board-shape.json")
    );
}

#[tokio::test]
async fn link_new_revision_no_interface_change_advances_resolved_for_branch() {
    let h = Harness::start("no-interface-change").await;
    let r1 = make_record(Driver::Composer, None, 0, 1.6);
    write_interface(&h.waffle_repo, &r1);
    let c1 = commit_all(&h.waffle_repo, "Initial board shape");
    h.add_link_default().await;

    // Unrelated commit: enclosure rib change, interface untouched.
    std::fs::write(h.waffle_repo.join("enclosure.waffle"), "rib = 2.0\n").unwrap();
    let c2 = commit_all(&h.waffle_repo, "Add enclosure rib");
    assert_ne!(c1, c2);

    let list = h.get_links(true).await;
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].state, LinkState::NewRevisionNoInterfaceChange);
    assert_eq!(list[0].resolved.commit, c2);

    // `design/links.toml` on disk quietly advanced `resolved.commit` to c2.
    let stored = ::project::links::read_links_file(&h.project_dir).unwrap();
    assert_eq!(stored[0].resolved.commit, c2);

    // Next check sees `tip == resolved` -> `Current`.
    let next = h.get_links(false).await;
    assert_eq!(next[0].state, LinkState::Current);
    assert_eq!(next[0].resolved.commit, c2);
}

#[tokio::test]
async fn link_update_available_reports_hole_move_and_accepts_tip() {
    let h = Harness::start("update-available").await;
    let r1 = make_record(Driver::Composer, None, 0, 1.6);
    write_interface(&h.waffle_repo, &r1);
    let c1 = commit_all(&h.waffle_repo, "Initial board shape");
    h.add_link_default().await;

    // Move hole waffle:s1/h3 1.20 mm right (+X).
    let r2 = make_record(
        Driver::Composer,
        Some(r1.manifest.revision.clone()),
        1_200_000,
        1.6,
    );
    write_interface(&h.waffle_repo, &r2);
    let c2 = commit_all(&h.waffle_repo, "Move mounting hole h3");

    let list = h.get_links(true).await;
    assert_eq!(list[0].state, LinkState::UpdateAvailable);
    assert_eq!(list[0].resolved.commit, c1);
    assert_eq!(list[0].latest.as_ref().unwrap().commit, c2);
    assert_eq!(
        list[0].changes,
        vec![Change {
            id: Some("waffle:s1/h3".into()),
            role: Role::Hole,
            kind: ChangeKind::Moved {
                dx_nm: 1_200_000,
                dy_nm: 0,
            },
        }]
    );

    // Before accepting, materialized files still hold c1's bytes.
    let mat_dir = h.project_dir.join(".boardroom/links/enclosure");
    assert_eq!(
        std::fs::read(mat_dir.join("board-shape.dxf")).unwrap(),
        git_blob_bytes(&h.waffle_repo, &c1, "interface/easy2350/board-shape.dxf")
    );

    // Accept update.
    let (st, body) = h
        .req(
            "POST",
            &format!("/api/projects/{}/links/enclosure/update", h.project_id),
            None,
        )
        .await;
    assert_eq!(st, StatusCode::OK);
    let updated: LinkStatus = serde_json::from_slice(&body).unwrap();
    assert_eq!(updated.state, LinkState::Current);
    assert_eq!(updated.resolved.commit, c2);
    assert!(updated.changes.is_empty());
    assert_eq!(
        std::fs::read(mat_dir.join("board-shape.dxf")).unwrap(),
        git_blob_bytes(&h.waffle_repo, &c2, "interface/easy2350/board-shape.dxf")
    );
    assert_eq!(
        std::fs::read(mat_dir.join("board-shape.json")).unwrap(),
        git_blob_bytes(&h.waffle_repo, &c2, "interface/easy2350/board-shape.json")
    );
}

#[tokio::test]
async fn link_pinned_behind_and_follow_branch() {
    let h = Harness::start("pinned-behind").await;
    let r1 = make_record(Driver::Composer, None, 0, 1.6);
    write_interface(&h.waffle_repo, &r1);
    let c1 = commit_all(&h.waffle_repo, "Initial board shape");
    h.add_link_default().await;

    // Pin at c1 with a reason.
    let (st, body) = h
        .req(
            "POST",
            &format!("/api/projects/{}/links/enclosure/pin", h.project_id),
            Some(serde_json::json!({ "reason": "Hold outline for gate 4 review" })),
        )
        .await;
    assert_eq!(st, StatusCode::OK);
    let pinned: LinkStatus = serde_json::from_slice(&body).unwrap();
    assert_eq!(pinned.state, LinkState::Current);
    assert_eq!(pinned.pin_reason, "Hold outline for gate 4 review");

    // Advance `main` with both a hole move and thickness change.
    let r2 = make_record(
        Driver::Composer,
        Some(r1.manifest.revision.clone()),
        -2_500_000,
        2.0,
    );
    write_interface(&h.waffle_repo, &r2);
    let c2 = commit_all(&h.waffle_repo, "Move hole and thicken board");

    let list = h.get_links(true).await;
    assert_eq!(list[0].state, LinkState::PinnedBehind);
    assert_eq!(list[0].resolved.commit, c1);
    assert_eq!(list[0].latest.as_ref().unwrap().commit, c2);
    assert_eq!(list[0].thickness_changed, Some((1_600_000, 2_000_000)));
    assert_eq!(list[0].changes.len(), 1);

    // PinnedBehind is information, never an inbox warning.
    let (_, proj_bytes) = h.req("GET", "/api/projects", None).await;
    let summaries: Vec<serde_json::Value> = serde_json::from_slice(&proj_bytes).unwrap();
    assert_eq!(summaries[0]["needs_user"].as_array().unwrap().len(), 0);

    // Follow `main` again -> transitions to `UpdateAvailable` (explicit sync, not auto-accepted).
    let (st, body) = h
        .req(
            "POST",
            &format!("/api/projects/{}/links/enclosure/follow", h.project_id),
            Some(serde_json::json!({ "branch": "main" })),
        )
        .await;
    assert_eq!(st, StatusCode::OK);
    let followed: LinkStatus = serde_json::from_slice(&body).unwrap();
    assert_eq!(followed.state, LinkState::UpdateAvailable);
    assert_eq!(followed.resolved.commit, c1);
    assert_eq!(followed.pin_reason, "");
}

#[tokio::test]
async fn link_locked_by_waffle_alongside_current_and_update_available() {
    let h = Harness::start("locked").await;
    let r1 = make_record(Driver::Waffle, None, 0, 1.6);
    write_interface(&h.waffle_repo, &r1);
    commit_all(&h.waffle_repo, "Waffle drives outline");

    let status = h.add_link_default().await;
    assert_eq!(status.state, LinkState::Current);
    assert_eq!(status.locked_by, Some(Driver::Waffle));

    let r2 = make_record(
        Driver::Waffle,
        Some(r1.manifest.revision.clone()),
        800_000,
        1.6,
    );
    write_interface(&h.waffle_repo, &r2);
    commit_all(&h.waffle_repo, "Waffle moves hole");

    let list = h.get_links(true).await;
    assert_eq!(list[0].state, LinkState::UpdateAvailable);
    assert_eq!(list[0].locked_by, Some(Driver::Waffle));
}

#[tokio::test]
async fn link_changed_outside_suite_surfaces_in_inbox_without_writing_status_json() {
    let h = Harness::start("changed-outside-suite").await;
    let r1 = make_record(Driver::Waffle, None, 0, 1.6);
    write_interface(&h.waffle_repo, &r1);
    commit_all(&h.waffle_repo, "Sealed shape");
    h.add_link_default().await;

    let status_json_before =
        std::fs::read_to_string(h.project_dir.join("design/status.json")).unwrap();

    // Edit DXF bytes directly (move hole X from 46.000000000 to 47.500000000) without updating
    // `board-shape.json`.
    let dxf_path = h.waffle_repo.join("interface/easy2350/board-shape.dxf");
    let dxf_text = std::fs::read_to_string(&dxf_path).unwrap();
    let edited_dxf = dxf_text.replacen("46.000000000", "47.500000000", 1);
    assert_ne!(dxf_text, edited_dxf);
    std::fs::write(&dxf_path, edited_dxf).unwrap();
    commit_all(&h.waffle_repo, "Third-party CAD edit of DXF");

    let list = h.get_links(true).await;
    assert_eq!(list[0].state, LinkState::ChangedOutsideSuite);
    assert_eq!(
        list[0].classification,
        Some(Classification::ChangedOutsideSuite)
    );

    // Derived into `/api/projects` needs_user, while `design/status.json` on disk stays untouched.
    let (_, proj_bytes) = h.req("GET", "/api/projects", None).await;
    let summaries: Vec<serde_json::Value> = serde_json::from_slice(&proj_bytes).unwrap();
    let needs = summaries[0]["needs_user"].as_array().unwrap();
    assert_eq!(needs.len(), 1);
    assert_eq!(needs[0]["id"], "link:enclosure:ChangedOutsideSuite");
    assert_eq!(
        needs[0]["text"],
        "Linked outline enclosure was changed outside the suite: import it?"
    );
    assert_eq!(
        std::fs::read_to_string(h.project_dir.join("design/status.json")).unwrap(),
        status_json_before
    );
}

#[tokio::test]
async fn link_competing_take_overs_surfaces_in_inbox_without_writing_status_json() {
    let h = Harness::start("competing-takeovers").await;
    // KiCad's r0 is in the linked repo; the link is added there.
    let r0 = make_record(Driver::Kicad, None, 0, 1.6);
    write_interface(&h.waffle_repo, &r0);
    commit_all(&h.waffle_repo, "KiCad outline");
    h.add_link_default().await;
    let status_json_before =
        std::fs::read_to_string(h.project_dir.join("design/status.json")).unwrap();

    // Composer takes over from r0 and publishes into this project's own interface/.
    let mut r_comp = make_record(
        Driver::Composer,
        Some(r0.manifest.revision.clone()),
        500_000,
        1.6,
    );
    r_comp.manifest.last_handover = Some(Handover {
        by: Driver::Composer,
        at: "2026-11-06T01:05:00Z".into(),
        from_revision: r0.manifest.revision.clone(),
    });
    seal(&mut r_comp);
    write_interface(&h.project_dir, &r_comp);

    // Waffle ALSO took over from r0, before seeing Composer's, in the linked repo.
    let mut r_waffle = make_record(
        Driver::Waffle,
        Some(r0.manifest.revision.clone()),
        -500_000,
        1.6,
    );
    r_waffle.manifest.last_handover = Some(Handover {
        by: Driver::Waffle,
        at: "2026-11-06T01:06:00Z".into(),
        from_revision: r0.manifest.revision.clone(),
    });
    seal(&mut r_waffle);
    write_interface(&h.waffle_repo, &r_waffle);
    commit_all(&h.waffle_repo, "Competing Waffle take over from r0");

    let list = h.get_links(true).await;
    assert_eq!(list[0].state, LinkState::CompetingTakeOvers);
    assert_eq!(list[0].locked_by, None);

    let (_, proj_bytes) = h.req("GET", "/api/projects", None).await;
    let summaries: Vec<serde_json::Value> = serde_json::from_slice(&proj_bytes).unwrap();
    let needs = summaries[0]["needs_user"].as_array().unwrap();
    assert_eq!(needs.len(), 1);
    assert_eq!(needs[0]["id"], "link:enclosure:CompetingTakeOvers");
    assert_eq!(
        needs[0]["text"],
        "Two take overs of easy2350's outline: pick one"
    );
    assert_eq!(
        std::fs::read_to_string(h.project_dir.join("design/status.json")).unwrap(),
        status_json_before
    );
}

#[tokio::test]
async fn link_stale_take_over_when_composer_took_an_old_revision() {
    let h = Harness::start("stale-takeover").await;
    let r1 = make_record(Driver::Waffle, None, 0, 1.6);
    write_interface(&h.waffle_repo, &r1);
    commit_all(&h.waffle_repo, "Waffle drives");
    h.add_link_default().await;

    // Composer takes over from r1 ...
    let mut r_comp = make_record(
        Driver::Composer,
        Some(r1.manifest.revision.clone()),
        300_000,
        1.6,
    );
    r_comp.manifest.last_handover = Some(Handover {
        by: Driver::Composer,
        at: "2026-11-06T02:00:00Z".into(),
        from_revision: r1.manifest.revision.clone(),
    });
    seal(&mut r_comp);
    write_interface(&h.project_dir, &r_comp);

    // ... while Waffle, not having seen it, kept driving: r2 on top of r1.
    let r2 = make_record(
        Driver::Waffle,
        Some(r1.manifest.revision.clone()),
        800_000,
        1.6,
    );
    write_interface(&h.waffle_repo, &r2);
    commit_all(&h.waffle_repo, "Waffle keeps driving");

    let list = h.get_links(true).await;
    // Same parent, Waffle merely continued: a stale take over, not a pair of take overs.
    assert_eq!(list[0].state, LinkState::StaleTakeOver);
    let (_, proj_bytes) = h.req("GET", "/api/projects", None).await;
    let summaries: Vec<serde_json::Value> = serde_json::from_slice(&proj_bytes).unwrap();
    let needs = summaries[0]["needs_user"].as_array().unwrap();
    assert_eq!(needs.len(), 1);
    assert_eq!(needs[0]["id"], "link:enclosure:StaleTakeOver");
}

#[tokio::test]
async fn link_not_locked_after_composer_took_over_and_published_again() {
    let h = Harness::start("composer-drives").await;
    let r1 = make_record(Driver::Waffle, None, 0, 1.6);
    write_interface(&h.waffle_repo, &r1);
    commit_all(&h.waffle_repo, "Waffle drives");
    let status = h.add_link_default().await;
    assert_eq!(status.locked_by, Some(Driver::Waffle));

    // Composer takes over from r1, then publishes once more (board-interface 0.2.1 called this a
    // stale take over; 0.2.2 says the linked side is behind).
    let mut t = make_record(Driver::Composer, Some(r1.manifest.revision.clone()), 0, 1.6);
    t.manifest.last_handover = Some(Handover {
        by: Driver::Composer,
        at: "2026-11-06T03:00:00Z".into(),
        from_revision: r1.manifest.revision.clone(),
    });
    seal(&mut t);
    let mut e1 = t.clone();
    e1.manifest.parent = Some(t.manifest.revision.clone());
    e1.manifest.thickness_mm = 1.2;
    seal(&mut e1);
    write_interface(&h.project_dir, &e1);

    let list = h.get_links(true).await;
    assert_eq!(list[0].state, LinkState::Current);
    assert_eq!(list[0].locked_by, None);
    let (_, proj_bytes) = h.req("GET", "/api/projects", None).await;
    let summaries: Vec<serde_json::Value> = serde_json::from_slice(&proj_bytes).unwrap();
    assert_eq!(summaries[0]["needs_user"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn link_unavailable_on_bad_remote_keeps_last_materialized_version() {
    let h = Harness::start("unavailable").await;
    let r1 = make_record(Driver::Waffle, None, 0, 1.6);
    write_interface(&h.waffle_repo, &r1);
    let c1 = commit_all(&h.waffle_repo, "Initial");
    h.add_link_default().await;

    let mat_dir = h.project_dir.join(".boardroom/links/enclosure");
    let expected_dxf = git_blob_bytes(&h.waffle_repo, &c1, "interface/easy2350/board-shape.dxf");
    let expected_json = git_blob_bytes(&h.waffle_repo, &c1, "interface/easy2350/board-shape.json");

    // Adding a brand-new link to a non-existent remote is rejected on POST.
    let bad_add = serde_json::json!({
        "id": "broken",
        "kind": "waffle",
        "board": "easy2350",
        "remote": h.dir.join("no-such-repo").to_str().unwrap(),
        "path": "interface/easy2350/board-shape.json",
        "ref": { "type": "Branch", "name": "main" }
    });
    let (st, _) = h
        .req(
            "POST",
            &format!("/api/projects/{}/links", h.project_id),
            Some(bad_add),
        )
        .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);

    // Move the remote away -> forced check reports `Unavailable` with error, and materialized
    // files from `c1` stay intact.
    let moved = h.dir.join("waffle-offline");
    std::fs::rename(&h.waffle_repo, &moved).unwrap();

    let list = h.get_links(true).await;
    assert_eq!(list[0].state, LinkState::Unavailable);
    assert!(list[0].error.as_deref().is_some_and(|e| !e.is_empty()));
    assert_eq!(
        std::fs::read(mat_dir.join("board-shape.dxf")).unwrap(),
        expected_dxf
    );
    assert_eq!(
        std::fs::read(mat_dir.join("board-shape.json")).unwrap(),
        expected_json
    );
}

#[tokio::test]
async fn link_tampered_when_stored_interface_hash_mismatches_base() {
    let h = Harness::start("tampered").await;
    let r1 = make_record(Driver::Waffle, None, 0, 1.6);
    write_interface(&h.waffle_repo, &r1);
    commit_all(&h.waffle_repo, "Initial");
    h.add_link_default().await;

    let toml_path = h.project_dir.join("design/links.toml");
    let raw = std::fs::read_to_string(&toml_path).unwrap();
    let tampered = raw.replace(&r1.manifest.geometry_hash, "sha256:0000000000000000");
    std::fs::write(&toml_path, tampered).unwrap();

    let list = h.get_links(false).await;
    assert_eq!(list[0].state, LinkState::Tampered);
    assert!(
        list[0]
            .error
            .as_deref()
            .is_some_and(|e| e.contains("does not match geometry_hash"))
    );
}

#[tokio::test]
async fn links_toml_round_trip_preserves_comments_and_unknown_keys() {
    let h = Harness::start("toml-preserve").await;
    let r1 = make_record(Driver::Waffle, None, 0, 1.6);
    write_interface(&h.waffle_repo, &r1);
    let c1 = commit_all(&h.waffle_repo, "Initial");

    let toml_path = h.project_dir.join("design/links.toml");
    let hand_written = format!(
        "# Project-wide comment kept across Boardroom writes\n\
         custom_top = \"keep-me\"\n\n\
         [[links]]\n\
         # Enclosure link comment\n\
         id = \"enclosure\"\n\
         kind = \"waffle\"\n\
         board = \"easy2350\"\n\
         extra_link_key = 42\n\
         locator = {{ type = \"Git\", remote = {:?}, path = \"interface/easy2350/board-shape.json\", ref = {{ type = \"Branch\", name = \"main\" }}, extra_loc = true }}\n\
         resolved = {{ commit = {:?}, at = \"2026-11-06T00:00:00Z\", extra_res = \"kept\" }}\n\
         interface_hash = {:?}     # of what was last accepted\n\
         pin_reason = \"\"               # why, when ref is a Commit\n",
        h.waffle_repo.to_str().unwrap(),
        c1,
        r1.manifest.geometry_hash
    );
    std::fs::write(&toml_path, hand_written).unwrap();

    // Exercise pin -> follow -> update!
    let (st, _) = h
        .req(
            "POST",
            &format!("/api/projects/{}/links/enclosure/pin", h.project_id),
            Some(serde_json::json!({ "reason": "check comments" })),
        )
        .await;
    assert_eq!(st, StatusCode::OK);

    let (st, _) = h
        .req(
            "POST",
            &format!("/api/projects/{}/links/enclosure/follow", h.project_id),
            Some(serde_json::json!({ "branch": "main" })),
        )
        .await;
    assert_eq!(st, StatusCode::OK);

    let r2 = make_record(
        Driver::Waffle,
        Some(r1.manifest.revision.clone()),
        400_000,
        1.6,
    );
    write_interface(&h.waffle_repo, &r2);
    let c2 = commit_all(&h.waffle_repo, "Update shape");

    let (st, _) = h
        .req(
            "POST",
            &format!("/api/projects/{}/links/enclosure/update", h.project_id),
            None,
        )
        .await;
    assert_eq!(st, StatusCode::OK);

    let after = std::fs::read_to_string(&toml_path).unwrap();
    for expected in [
        "# Project-wide comment kept across Boardroom writes",
        "custom_top = \"keep-me\"",
        "# Enclosure link comment",
        "extra_link_key = 42",
        "extra_loc = true",
        "extra_res = \"kept\"",
        "# of what was last accepted",
        "# why, when ref is a Commit",
        &c2,
        &r2.manifest.geometry_hash,
    ] {
        assert!(
            after.contains(expected),
            "missing {expected:?} in:\n{after}"
        );
    }
    // And the resulting TOML still parses cleanly.
    let parsed = ::project::links::read_links_file(&h.project_dir).unwrap();
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].resolved.commit, c2);
}

#[tokio::test]
async fn cli_subcommands_and_bus_events_work() {
    let h = Harness::start("cli-and-bus").await;
    let r1 = make_record(Driver::Waffle, None, 0, 1.6);
    write_interface(&h.waffle_repo, &r1);
    let c1 = commit_all(&h.waffle_repo, "Initial");

    // 1. CLI `link add`, `link list --json`, `link pin`, `link follow`, `link update`, `link remove`.
    let bin = env!("CARGO_BIN_EXE_boardroom");
    let run_cli = |args: &[&str]| -> String {
        let out = Command::new(bin)
            .env("XDG_CACHE_HOME", h.dir.join("cli-cache"))
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "boardroom {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).into_owned()
    };

    let p_str = h.project_dir.to_str().unwrap();
    run_cli(&[
        "link",
        "add",
        p_str,
        "--id",
        "enclosure",
        "--board",
        "easy2350",
        "--remote",
        h.waffle_repo.to_str().unwrap(),
        "--path",
        "interface/easy2350/board-shape.json",
        "--branch",
        "main",
    ]);

    let list_json = run_cli(&["link", "list", p_str, "--json"]);
    let statuses: Vec<LinkStatus> = serde_json::from_str(&list_json).unwrap();
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].resolved.commit, c1);

    run_cli(&["link", "pin", p_str, "enclosure", "--reason", "cli-pin"]);
    run_cli(&["link", "follow", p_str, "enclosure", "--branch", "main"]);

    // 2. Connect a WebSocket client to `/api/bus` and confirm link updates publish a bus event.
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
    assert!(
        String::from_utf8_lossy(&hdr_buf[..filled]).starts_with("HTTP/1.1 101"),
        "ws upgrade failed"
    );

    let r2 = make_record(
        Driver::Waffle,
        Some(r1.manifest.revision.clone()),
        900_000,
        1.6,
    );
    write_interface(&h.waffle_repo, &r2);
    let c2 = commit_all(&h.waffle_repo, "Second commit");

    let (st, _) = h
        .req(
            "POST",
            &format!("/api/projects/{}/links/enclosure/update", h.project_id),
            None,
        )
        .await;
    assert_eq!(st, StatusCode::OK);

    // Read WebSocket text frame.
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
    .expect("expected bus message");
    let bus_json: serde_json::Value = serde_json::from_str(&frame).unwrap();
    assert_eq!(bus_json["project"], h.project_id);

    // CLI update & remove.
    let after_update: Vec<LinkStatus> =
        serde_json::from_str(&run_cli(&["link", "list", p_str, "--json"])).unwrap();
    assert_eq!(after_update[0].resolved.commit, c2);

    run_cli(&["link", "remove", p_str, "enclosure"]);
    assert!(!h.project_dir.join(".boardroom/links/enclosure").exists());
}

#[tokio::test]
async fn link_writes_rejected_from_non_loopback_peers() {
    use axum::extract::ConnectInfo;
    use tower::ServiceExt;

    let n = CASE_SEQ.fetch_add(1, Ordering::SeqCst);
    let root = std::env::temp_dir()
        .join(format!("boardroom-links-it-{}", std::process::id()))
        .join(format!("{n:02}-loopback-guard"));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("cfg")).unwrap();
    std::fs::create_dir_all(root.join("web")).unwrap();
    std::fs::write(root.join("web/index.html"), "<!doctype html>").unwrap();

    let (router, _) = server::server::router_with_cache(
        root.join("cfg"),
        ProjectList::default(),
        root.join("web"),
        LinksCache::with_dir(root.join("cache")),
    );

    let remote_peer = SocketAddr::from(([192, 0, 2, 42], 51234));
    for (method, uri) in [
        ("POST", "/api/projects/p1/links"),
        ("POST", "/api/projects/p1/links/enclosure/update"),
        ("POST", "/api/projects/p1/links/enclosure/pin"),
        ("POST", "/api/projects/p1/links/enclosure/follow"),
        ("DELETE", "/api/projects/p1/links/enclosure"),
    ] {
        let mut req = Request::builder()
            .method(method)
            .uri(uri)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from("{}"))
            .unwrap();
        req.extensions_mut().insert(ConnectInfo(remote_peer));
        let resp = router.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN, "{method} {uri}");
    }
    let _ = std::fs::remove_dir_all(&root);
}
