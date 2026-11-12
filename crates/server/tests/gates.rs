use axum::body::Body;
use axum::http::{Request, StatusCode, Uri, header};
use board_interface::{
    Author, Driver, Element, ElementKind, Hole, Loop, Manifest, NM_PER_MM, Point, Record,
    SCHEMA_VERSION, seal, write,
};
use boardroom::config::{self, ProjectList};
use boardroom::gates;
use boardroom::links::{LinkState, LinkStatus, LinksCache};
use boardroom::project;
use http_body_util::BodyExt;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static CASE_SEQ: AtomicUsize = AtomicUsize::new(1);

const SAMPLE_PCB: &str = "(kicad_pcb
\t(version 20240108)
\t(generator \"pcbnew\")
\t(layers
\t\t(0 \"F.Cu\" signal)
\t\t(31 \"B.Cu\" signal)
\t\t(44 \"Edge.Cuts\" user)
\t)
\t(gr_line
\t\t(start 100 100)
\t\t(end 150 100)
\t\t(stroke (width 0.05) (type default))
\t\t(layer \"Edge.Cuts\")
\t\t(uuid \"11111111-1111-1111-1111-111111111111\")
\t)
\t(gr_line
\t\t(start 150 100)
\t\t(end 150 130)
\t\t(stroke (width 0.05) (type default))
\t\t(layer \"Edge.Cuts\")
\t\t(uuid \"22222222-2222-2222-2222-222222222222\")
\t)
\t(segment
\t\t(start 110 110)
\t\t(end 120 110)
\t\t(width 0.2)
\t\t(layer \"F.Cu\")
\t\t(net 1)
\t\t(uuid \"33333333-3333-3333-3333-333333333333\")
\t)
)\n";

struct TempProj {
    dir: PathBuf,
    proj: PathBuf,
}

impl TempProj {
    fn new(label: &str) -> Self {
        let n = CASE_SEQ.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir()
            .join(format!("boardroom-gates-it-{}", std::process::id()))
            .join(format!("{n:02}-{label}"));
        let _ = std::fs::remove_dir_all(&dir);
        let proj = dir.join("proj");
        std::fs::create_dir_all(proj.join("design/gates")).unwrap();
        std::fs::create_dir_all(proj.join("design/parts")).unwrap();
        std::fs::create_dir_all(proj.join("hw/demo/a/fab")).unwrap();

        std::fs::write(
            proj.join("pcbview.toml"),
            "title = \"demo\"\n\n[[boards]]\nid = \"demo-a\"\nname = \"demo A\"\npcb = \"hw/demo/a/demo.kicad_pcb\"\nsch = \"hw/demo/a/demo.kicad_sch\"\n",
        )
        .unwrap();
        std::fs::write(proj.join("design/intent.md"), "# Intent\n3.3 V board.\n").unwrap();
        std::fs::write(proj.join("design/parts/u1.md"), "# U1\nRP2350\n").unwrap();
        std::fs::write(
            proj.join("design/nets.csv"),
            "net,class,reason\nGND,power,ref\n",
        )
        .unwrap();
        std::fs::write(proj.join("design/brief.md"), "# Brief\n50x30 mm.\n").unwrap();
        std::fs::write(proj.join("design/stackup.md"), "# Stackup\n2L 1.6 mm.\n").unwrap();
        std::fs::write(proj.join("hw/demo/a/demo.kicad_pro"), "{\"board\":{}}\n").unwrap();
        std::fs::write(proj.join("hw/demo/a/demo.kicad_dru"), "(version 1)\n").unwrap();
        std::fs::write(
            proj.join("hw/demo/a/demo.kicad_sch"),
            "(kicad_sch (version 20231120))\n",
        )
        .unwrap();
        std::fs::write(proj.join("hw/demo/a/demo.kicad_pcb"), SAMPLE_PCB).unwrap();
        std::fs::write(
            proj.join("hw/demo/a/fab/bom.csv"),
            "Comment,Designator\n1k,R1\n",
        )
        .unwrap();

        Self { dir, proj }
    }

    fn record_gate(&self, gate: u32) {
        let script = boardroom::repo_dir().join(".claude/skills/pcb-design/scripts/gate_inputs.py");
        let out = Command::new("python3")
            .arg(&script)
            .arg("record")
            .arg(gate.to_string())
            .arg("--project")
            .arg(&self.proj)
            .output()
            .expect("python3 gate_inputs.py");
        assert!(
            out.status.success(),
            "gate_inputs.py record {gate} failed: {}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn write_status(&self, val: serde_json::Value) {
        std::fs::write(
            self.proj.join("design/status.json"),
            serde_json::to_string_pretty(&val).unwrap() + "\n",
        )
        .unwrap();
    }

    fn summary(&self) -> project::Summary {
        let id = config::project_id(&self.proj);
        project::summarize(&id, &self.proj, false)
    }
}

impl Drop for TempProj {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn init_git_repo(repo: &Path) {
    for args in [
        &["init", "-q", "-b", "main"][..],
        &["config", "user.name", "Gate Test"][..],
        &["config", "user.email", "gates@example.com"][..],
    ] {
        let out = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success());
    }
}

fn commit_all(repo: &Path, msg: &str) -> String {
    let add = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["add", "."])
        .output()
        .unwrap();
    assert!(add.status.success());
    let cm = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["commit", "-q", "-m", msg])
        .output()
        .unwrap();
    assert!(cm.status.success());
    let rev = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    assert!(rev.status.success());
    String::from_utf8_lossy(&rev.stdout).trim().to_string()
}

fn make_shape_record(parent: Option<String>, hole_dx_nm: i64) -> Record {
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
        center: Point::new(4 * NM_PER_MM + hole_dx_nm, 4 * NM_PER_MM),
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
            board: "demo".into(),
            revision: String::new(),
            parent,
            author: Author {
                app: Driver::Waffle,
                source: "test".into(),
                at: "2026-11-06T01:00:00Z".into(),
            },
            driver: Driver::Waffle,
            last_handover: None,
            thickness_mm: 1.6,
            geometry_hash: String::new(),
            approximate: false,
        },
    };
    seal(&mut rec);
    rec
}

fn write_shape(repo: &Path, rec: &Record) {
    let dir = repo.join("interface/demo");
    std::fs::create_dir_all(&dir).unwrap();
    let (dxf, json) = write(rec);
    std::fs::write(dir.join("board-shape.dxf"), dxf).unwrap();
    std::fs::write(dir.join("board-shape.json"), json).unwrap();
}

#[test]
fn record_present_and_unchanged_is_passed() {
    let t = TempProj::new("record-unchanged");
    t.write_status(serde_json::json!({
        "board": "demo",
        "phase": "6 placement",
        "gates_passed": [0, 1, 2, 3, 4, 5],
    }));
    for g in 0..=5 {
        t.record_gate(g);
    }

    let s = t.summary();
    for g in 0..=5 {
        assert_eq!(s.gates[g].state, "passed", "gate {g} should be passed");
        assert_eq!(
            s.gates[g].stale_because, None,
            "gate {g} should have no stale_because"
        );
    }
    assert_eq!(s.gates[6].state, "current");
    for g in 7..=10 {
        assert_eq!(s.gates[g].state, "not_started");
    }
}

#[test]
fn schematic_edit_stales_gate_2_and_downstream_passed_gates() {
    let t = TempProj::new("sch-downstream");
    t.write_status(serde_json::json!({
        "board": "demo",
        "phase": "7 routing",
        "gates_passed": [0, 1, 2, 3, 4, 5, 6],
    }));
    for g in 0..=6 {
        t.record_gate(g);
    }

    // Edit the schematic after gates 0..=6 passed.
    std::fs::write(
        t.proj.join("hw/demo/a/demo.kicad_sch"),
        "(kicad_sch (version 20231120) (paper \"A3\"))\n",
    )
    .unwrap();

    let s = t.summary();
    // Gates 0 and 1 do not judge .kicad_sch -> remain passed.
    assert_eq!(s.gates[0].state, "passed");
    assert_eq!(s.gates[1].state, "passed");

    // Gate 2 directly judged .kicad_sch -> stale naming the file.
    assert_eq!(s.gates[2].state, "stale");
    assert_eq!(
        s.gates[2].stale_because.as_deref(),
        Some("hw/demo/a/demo.kicad_sch changed since gate 2")
    );

    // Every later passed gate (3..=6) is stale downstream because gate 2 is stale.
    for g in 3..=6 {
        assert_eq!(
            s.gates[g].state, "stale",
            "gate {g} should be stale downstream"
        );
        assert_eq!(
            s.gates[g].stale_because.as_deref(),
            Some("gate 2 is stale"),
            "gate {g} downstream reason"
        );
    }

    // Gate 7 (current, not passed) and 8..=10 (not started) are not marked stale.
    assert_eq!(s.gates[7].state, "current");
    assert_eq!(s.gates[7].stale_because, None);
    for g in 8..=10 {
        assert_eq!(s.gates[g].state, "not_started");
    }
}

#[test]
fn gate_4_edge_cuts_vs_copper_track_and_own_interface() {
    let t = TempProj::new("edge-cuts-and-own");
    let own_dir = t.proj.join("interface/demo");
    std::fs::create_dir_all(&own_dir).unwrap();
    std::fs::write(
        own_dir.join("board-shape.json"),
        serde_json::json!({
            "schema": 1,
            "board": "demo",
            "revision": "rev:sha256:own1",
            "geometry_hash": "sha256:1111aaaa"
        })
        .to_string(),
    )
    .unwrap();

    t.write_status(serde_json::json!({
        "board": "demo",
        "phase": "8 simulation",
        "gates_passed": [0, 1, 2, 3, 4, 5, 6, 7],
    }));
    t.record_gate(4);
    t.record_gate(6);
    t.record_gate(7);

    // Copper track change: stales gate 7, not gate 4 (outline) or gate 6 (placement).
    std::fs::write(
        t.proj.join("hw/demo/a/demo.kicad_pcb"),
        SAMPLE_PCB.replace("(end 120 110)", "(end 126 112)"),
    )
    .unwrap();
    let s1 = t.summary();
    assert_eq!(s1.gates[4].state, "passed");
    assert_eq!(s1.gates[6].state, "passed");
    assert_eq!(s1.gates[7].state, "stale");
    assert_eq!(
        s1.gates[7].stale_because.as_deref(),
        Some("hw/demo/a/demo.kicad_pcb (copper) changed since gate 7")
    );

    // Edge.Cuts change: stales gate 4 (and downstream 5..=7).
    std::fs::write(
        t.proj.join("hw/demo/a/demo.kicad_pcb"),
        SAMPLE_PCB.replace("(end 150 100)", "(end 151 100)"),
    )
    .unwrap();
    let s2 = t.summary();
    assert_eq!(s2.gates[4].state, "stale");
    assert_eq!(
        s2.gates[4].stale_because.as_deref(),
        Some("hw/demo/a/demo.kicad_pcb (Edge.Cuts) changed since gate 4")
    );
    for g in 5..=7 {
        assert_eq!(s2.gates[g].state, "stale");
        assert_eq!(
            s2.gates[g].stale_because.as_deref(),
            Some("gate 4 is stale")
        );
    }

    // Restore PCB, change own_interface geometry_hash -> "own outline changed".
    std::fs::write(t.proj.join("hw/demo/a/demo.kicad_pcb"), SAMPLE_PCB).unwrap();
    std::fs::write(
        own_dir.join("board-shape.json"),
        serde_json::json!({
            "schema": 1,
            "board": "demo",
            "revision": "rev:sha256:own2",
            "geometry_hash": "sha256:2222bbbb"
        })
        .to_string(),
    )
    .unwrap();
    let s3 = t.summary();
    assert_eq!(s3.gates[4].state, "stale");
    assert_eq!(
        s3.gates[4].stale_because.as_deref(),
        Some("own outline changed")
    );
}

#[tokio::test]
async fn link_same_geometry_advance_not_stale_and_new_geometry_stales_gate_4_and_updates_release() {
    let t = TempProj::new("link-geometry");
    let waffle_repo = t.dir.join("waffle");
    std::fs::create_dir_all(&waffle_repo).unwrap();
    init_git_repo(&waffle_repo);

    let r1 = make_shape_record(None, 0);
    write_shape(&waffle_repo, &r1);
    let c1 = commit_all(&waffle_repo, "Initial enclosure shape");

    let cfg_dir = t.dir.join("cfg");
    let web_dir = t.dir.join("web");
    let cache_dir = t.dir.join("cache/links");
    std::fs::create_dir_all(&cfg_dir).unwrap();
    std::fs::create_dir_all(&web_dir).unwrap();
    std::fs::write(web_dir.join("index.html"), "<!doctype html>").unwrap();

    t.write_status(serde_json::json!({
        "schema": 1,
        "board": "demo",
        "phase": 10,
        "gates_passed": [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
    }));

    let mut list = ProjectList::default();
    let canon_proj = list.add(&t.proj).unwrap();
    list.save_to(&cfg_dir).unwrap();
    let project_id = config::project_id(&canon_proj);

    let (router, _) = boardroom::server::router_with_cache(
        cfg_dir,
        list,
        web_dir,
        LinksCache::with_dir(cache_dir),
    );
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let port = listener.local_addr().unwrap().port();
    let server_task = tokio::spawn(async move {
        let _ = axum::serve(
            listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await;
    });

    let req = |method: &'static str, path: String, body: Option<serde_json::Value>| async move {
        let client: Client<_, Body> = Client::builder(TokioExecutor::new()).build_http();
        let uri: Uri = format!("http://127.0.0.1:{port}{path}").parse().unwrap();
        let mut b = Request::builder().method(method).uri(uri);
        let payload = match body {
            Some(v) => {
                b = b.header(header::CONTENT_TYPE, "application/json");
                Body::from(serde_json::to_vec(&v).unwrap())
            }
            None => Body::empty(),
        };
        let resp = client.request(b.body(payload).unwrap()).await.unwrap();
        let st = resp.status();
        let bytes = resp
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec();
        (st, bytes)
    };

    // Add link at commit c1.
    let (st, body) = req(
        "POST",
        format!("/api/projects/{project_id}/links"),
        Some(serde_json::json!({
            "id": "enclosure",
            "kind": "waffle",
            "board": "demo",
            "remote": waffle_repo.to_str().unwrap(),
            "path": "interface/demo/board-shape.json",
            "ref": { "type": "Branch", "name": "main" }
        })),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{}", String::from_utf8_lossy(&body));

    // Record gates 2, 4, and 10 against c1.
    t.record_gate(2);
    t.record_gate(4);
    t.record_gate(10);

    // 1) Advance enclosure repo with an unrelated commit (same board-shape geometry):
    //    `NewRevisionNoInterfaceChange` quietly advances `links.toml` to c2, and
    //    Gate 4 + Gate 10 MUST remain `passed` (NOT stale).
    std::fs::write(waffle_repo.join("README.md"), "enclosure rib note\n").unwrap();
    let c2 = commit_all(&waffle_repo, "Enclosure README change only");
    assert_ne!(c1, c2);

    // Force-refresh links so the mirror fetches c2 immediately (bypassing 60s TTL).
    let (st_links, links_bytes) = req(
        "GET",
        format!("/api/projects/{project_id}/links?refresh=1"),
        None,
    )
    .await;
    assert_eq!(st_links, StatusCode::OK);
    let statuses: Vec<LinkStatus> = serde_json::from_slice(&links_bytes).unwrap();
    assert_eq!(statuses[0].state, LinkState::NewRevisionNoInterfaceChange);
    assert_eq!(statuses[0].resolved.commit, c2);

    let (st_proj, proj_bytes) = req("GET", "/api/projects".into(), None).await;
    assert_eq!(st_proj, StatusCode::OK);
    let projs: Vec<serde_json::Value> = serde_json::from_slice(&proj_bytes).unwrap();
    let p = &projs[0];
    assert_eq!(p["gates"][4]["state"], "passed");
    assert_eq!(p["gates"][4]["stale_because"], serde_json::Value::Null);
    assert_eq!(p["gates"][10]["state"], "passed");
    // Release record still reports c1 ("built against") AND notes links.toml is now at c2!
    assert_eq!(p["release"]["links"][0]["id"], "enclosure");
    assert_eq!(p["release"]["links"][0]["commit"], c1);
    assert_eq!(p["release"]["links"][0]["revision"], r1.manifest.revision);
    assert_eq!(p["release"]["links"][0]["resolved_commit"], c2);

    // 2) Now publish a new enclosure commit c3 that moves mounting hole h1 (new geometry_hash),
    //    and accept the link update -> Gate 4 goes stale ("linked outline enclosure moved to ..."),
    //    Gates 5..=10 go stale downstream ("gate 4 is stale"), while Gates 0..=3 stay passed!
    let r3 = make_shape_record(Some(r1.manifest.revision.clone()), 2 * NM_PER_MM);
    assert_ne!(r3.manifest.geometry_hash, r1.manifest.geometry_hash);
    write_shape(&waffle_repo, &r3);
    let c3 = commit_all(&waffle_repo, "Move mounting hole 2 mm");

    let (st_ref, _) = req(
        "GET",
        format!("/api/projects/{project_id}/links?refresh=1"),
        None,
    )
    .await;
    assert_eq!(st_ref, StatusCode::OK);
    let (st_upd, upd_bytes) = req(
        "POST",
        format!("/api/projects/{project_id}/links/enclosure/update"),
        None,
    )
    .await;
    assert_eq!(
        st_upd,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&upd_bytes)
    );

    let s = t.summary();
    for g in 0..=3 {
        assert_eq!(s.gates[g].state, "passed", "gate {g} should stay passed");
    }
    assert_eq!(s.gates[4].state, "stale");
    let expected_reason = format!(
        "linked outline enclosure moved to {} (was {})",
        &c3[..7],
        &c1[..7]
    );
    assert_eq!(
        s.gates[4].stale_because.as_deref(),
        Some(expected_reason.as_str())
    );
    for g in 5..=10 {
        assert_eq!(
            s.gates[g].state, "stale",
            "gate {g} should be stale downstream"
        );
        assert_eq!(s.gates[g].stale_because.as_deref(), Some("gate 4 is stale"));
    }

    server_task.abort();
}

#[test]
fn hand_written_stale_because_always_wins() {
    let t = TempProj::new("hand-written-wins");
    t.record_gate(2);
    t.record_gate(4);

    // Edit schematic so gate 2 auto-stales and propagates "gate 2 is stale" downstream,
    // while gate 1 (clean record / no record) and gate 5 have hand-written stale_because.
    std::fs::write(
        t.proj.join("hw/demo/a/demo.kicad_sch"),
        "(kicad_sch (version 20231120) (paper \"A2\"))\n",
    )
    .unwrap();

    t.write_status(serde_json::json!({
        "board": "demo",
        "phase": "7 routing",
        "gates_passed": [0, 1, 2, 3, 4, 5, 6],
        "gates": {
            "2": { "stale_because": "hand-written reason on gate 2" },
            "5": { "stale_because": "hand-written reason on gate 5" }
        }
    }));

    let s = t.summary();
    assert_eq!(s.gates[0].state, "passed");
    assert_eq!(s.gates[1].state, "passed");
    // Hand-written reason beats derived file-change reason on gate 2:
    assert_eq!(s.gates[2].state, "stale");
    assert_eq!(
        s.gates[2].stale_because.as_deref(),
        Some("hand-written reason on gate 2")
    );
    // Downstream gates without hand-written reason get "gate 2 is stale":
    assert_eq!(s.gates[3].stale_because.as_deref(), Some("gate 2 is stale"));
    assert_eq!(s.gates[4].stale_because.as_deref(), Some("gate 2 is stale"));
    // Hand-written reason beats downstream "gate 2 is stale" on gate 5:
    assert_eq!(s.gates[5].state, "stale");
    assert_eq!(
        s.gates[5].stale_because.as_deref(),
        Some("hand-written reason on gate 5")
    );
    assert_eq!(s.gates[6].stale_because.as_deref(), Some("gate 2 is stale"));
}

#[test]
fn gate_without_record_is_never_auto_stale_including_easy2350() {
    // 1) Temp project with passed gates and NO phaseN.inputs.json records:
    //    editing schematic/board never auto-stales them, and status.json is untouched.
    let t = TempProj::new("no-record");
    t.write_status(serde_json::json!({
        "board": "demo",
        "phase": "7 routing",
        "gates_passed": [0, 1, 2, 3, 4, 5, 6],
    }));
    let before_status = std::fs::read_to_string(t.proj.join("design/status.json")).unwrap();
    std::fs::write(
        t.proj.join("hw/demo/a/demo.kicad_sch"),
        "(kicad_sch (version 20231120) (paper \"B4\"))\n",
    )
    .unwrap();
    std::fs::write(
        t.proj.join("hw/demo/a/demo.kicad_pcb"),
        SAMPLE_PCB.replace("(end 150 100)", "(end 160 100)"),
    )
    .unwrap();

    let s = t.summary();
    for g in 0..=6 {
        assert_eq!(s.gates[g].state, "passed");
        assert_eq!(s.gates[g].stale_because, None);
    }
    let after_status = std::fs::read_to_string(t.proj.join("design/status.json")).unwrap();
    assert_eq!(before_status, after_status);

    // 2) examples/easy2350 (read-only, no backfill records): gates 0..=6 stay passed,
    //    gates 7..=10 keep their hand-written stale_because.
    let easy = boardroom::repo_dir().join("examples/easy2350");
    let es = project::summarize("easy2350", &easy, false);
    for g in 0..=6 {
        assert!(
            !easy
                .join(format!("design/gates/phase{g}.inputs.json"))
                .exists(),
            "examples/easy2350 must not have backfilled inputs.json"
        );
        assert_eq!(es.gates[g].state, "passed");
        assert_eq!(es.gates[g].stale_because, None);
    }
    for g in 7..=10 {
        assert_eq!(es.gates[g].state, "stale");
        assert!(es.gates[g].stale_because.is_some());
    }
    assert_eq!(es.release, None);
}

#[test]
fn python_and_rust_board_aspect_hashes_match_on_easy2350() {
    let repo = boardroom::repo_dir();
    let pcb = repo.join("examples/easy2350/hw/easy2350/a/easy2350.kicad_pcb");
    let script_dir = repo.join(".claude/skills/pcb-design/scripts");
    let mut seen = std::collections::HashSet::new();
    for name in ["edge_cuts", "placement", "copper"] {
        let aspect = gates::Aspect::parse(name).unwrap();
        let rust_hash = gates::pcb_aspect_hash(&pcb, aspect).expect("rust pcb_aspect_hash");
        assert!(rust_hash.starts_with("sha256:"));
        assert_eq!(rust_hash.len(), 7 + 64);

        let py = Command::new("python3")
            .args([
                "-c",
                "import sys; from pathlib import Path; sys.path.insert(0, sys.argv[1]); import gate_inputs; print(gate_inputs.pcb_aspect_hash(Path(sys.argv[2]), sys.argv[3]))",
            ])
            .arg(&script_dir)
            .arg(&pcb)
            .arg(name)
            .output()
            .expect("python3 pcb_aspect_hash");
        assert!(
            py.status.success(),
            "python failed: {}",
            String::from_utf8_lossy(&py.stderr)
        );
        let py_hash = String::from_utf8_lossy(&py.stdout).trim().to_string();
        assert_eq!(
            rust_hash, py_hash,
            "Rust and Python {name} hashes must match on easy2350"
        );
        // Each aspect selects something different on a real board.
        assert!(
            seen.insert(rust_hash),
            "{name} hash repeats another aspect's"
        );
    }
}
