//! Boardroom: the project manager and main window of the board design suite.
//!
//!     boardroom serve [--port N] [--host ADDR]      run the server
//!     boardroom open  [--port N] [--browser]        open the Boardroom window
//!     boardroom add <folder>                        add a project to the list
//!     boardroom remove <folder>                     take one off the list
//!     boardroom list                                the list, with each project's phase
//!     boardroom selection [--port N] [--project ID|DIR]  the current bus selection as JSON
//!     boardroom link add <project-path> --id <id> --board <board> --remote <remote> --path <json> [--branch main|--tag T|--commit C]
//!     boardroom link list <project-path> [--refresh] [--json]
//!     boardroom link update <project-path> <id>
//!     boardroom link pin <project-path> <id> [--reason <why>]
//!     boardroom link follow <project-path> <id> [--branch <name>]
//!     boardroom link rehost <project-path> <id>
//!     boardroom link remove <project-path> <id>
//!     boardroom push <project-path>                 push the current branch to its upstream
//!     boardroom snapshot [--ccomp P] <out> <dir>... a static copy for the web-only build
//!     boardroom route <board.kicad_pcb> [--out DIR] [--packages DIR] [--filled-vias yes|no] [--save-every SECONDS]  route a board (into DIR, else route/<board>/ in its project), keeping its blocks' pours joined; a via may stand in a soldered pad where the board's vias are filled and capped (six layers or more, unless said); while it runs, a trace and the board so far are kept in DIR/partial/ (a board at least every 600 s, or as said; 0 for the trace alone)
//!     boardroom route <board.kicad_pcb> --close old [--close-seconds S]  the same, and then the old router on what is left open (300 s unless said), the new router's copper kept: `closed.kicad_pcb` beside `routed.kicad_pcb` where it leaves fewer open, with how many nets it changed; and then the old router's copper drawn again by the new one, as far as it can be: `redrawn.kicad_pcb`
//!     boardroom route <board.kicad_pcb> --close grid  the same with the old router's grid search only: nothing already on the board moved, a few nets changed, seconds
//!     boardroom close <board.kicad_pcb> [--out DIR] [--with old|grid] [--close-seconds S]  the closing alone (and the drawing again), on the `routed.kicad_pcb` an earlier `boardroom route` left in DIR
//!     boardroom redraw <before.kicad_pcb> <after.kicad_pcb> --out <out.kicad_pcb> [--filled-vias yes|no]  the tracks and vias `after` has that `before` has not, drawn again by the new router: each one's way kept, in 0/45/90 at our widths; what cannot be is left as laid, with why
//!     boardroom route --mode old <board.kicad_pcb> -o OUT.kicad_pcb [...]  the old router on the same board model, with its own words (`boardroom route --mode old --help`)
//!     boardroom check <board.kicad_pcb> [--json]    the checker's faults and open connections
//!     boardroom pours <board.kicad_pcb> [--zone NAME] [--cut-by NET[,NET]] [--json]  each pour's pieces after a refill and where each is narrow
//!     boardroom blocks <board.kicad_pcb> [--pin REF=PACKAGE/LAYOUT] [--accept REF[:NAME]|all] [--decline REF:NAME]  each chip's block against its package
//!     boardroom blocks --behind <dir>               the boards under dir behind their packages
//!
//! The port comes from --port, then $PORT, then `proj port`; with none the
//! command fails rather than guessing (see docs/shell-spec.md).

use ::project::links::{self, AddLinkRequest, GitRef, LinkStatus, LinksCache};
use ::project::{config, project, repo_dir, snapshot};
use ::server::server;
use std::path::PathBuf;

/// Run one command line (without the program name); the exit code.
pub fn run(args: Vec<String>) -> i32 {
    let (command, rest) = match args.split_first() {
        Some((c, r)) => (c.as_str(), r),
        None => ("help", &[][..]),
    };
    let result = match command {
        "serve" => serve(rest),
        "open" => open(rest),
        "add" => edit_list(rest, true),
        "remove" => edit_list(rest, false),
        "list" => list(),
        "selection" => selection_cmd(rest),
        "link" => link_cmd(rest),
        "push" => push_cmd(rest),
        "snapshot" => snapshot::write(rest),
        "route" => route_cmd(rest),
        "redraw" => redraw_cmd(rest),
        "close" => close_cmd(rest),
        "check" => check_cmd(rest),
        "pours" => pours_cmd(rest),
        "blocks" => blocks_cmd(rest),
        "help" | "-h" | "--help" => {
            // The usage is the module comment's command list.
            let usage: Vec<&str> = include_str!("cli.rs")
                .lines()
                .filter_map(|l| l.strip_prefix("//!     "))
                .filter(|l| l.starts_with("boardroom"))
                .collect();
            println!("{}", usage.join("\n"));
            Ok(())
        }
        other => Err(format!("unknown command '{other}' (try: boardroom help)")),
    };
    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("boardroom: {e}");
            1
        }
    }
}

/// `--name value` or `--name=value`.
fn flag(args: &[String], name: &str) -> Option<String> {
    let long = format!("--{name}");
    let prefix = format!("--{name}=");
    args.iter().enumerate().find_map(|(i, a)| {
        if *a == long {
            args.get(i + 1).cloned()
        } else {
            a.strip_prefix(&prefix).map(str::to_string)
        }
    })
}

fn has_switch(args: &[String], name: &str) -> bool {
    let long = format!("--{name}");
    args.contains(&long)
}

/// Positional arguments after skipping `--key value` flags in `valued` and `--switch` booleans in
/// `switches`.
fn positionals<'a>(
    args: &'a [String],
    valued: &[&str],
    switches: &[&str],
) -> Result<Vec<&'a str>, String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        if let Some(rest) = arg.strip_prefix("--") {
            if let Some((k, _)) = rest.split_once('=') {
                if !valued.contains(&k) {
                    return Err(format!("unknown option '--{k}'"));
                }
                i += 1;
                continue;
            }
            if switches.contains(&rest) {
                i += 1;
                continue;
            }
            if valued.contains(&rest) {
                if i + 1 >= args.len() {
                    return Err(format!("option '--{rest}' requires a value"));
                }
                i += 2;
                continue;
            }
            return Err(format!("unknown option '{arg}'"));
        }
        out.push(arg);
        i += 1;
    }
    Ok(out)
}

fn port(args: &[String]) -> Result<u16, String> {
    ::project::port(flag(args, "port").as_deref())
}

fn serve(args: &[String]) -> Result<(), String> {
    let port = port(args)?;
    // Localhost only unless asked: the server opens files and starts programs.
    let host = flag(args, "host").unwrap_or_else(|| "127.0.0.1".into());
    let web = flag(args, "web")
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_dir().join("web/dist"));
    if !web.join("index.html").is_file() {
        return Err(format!(
            "no web build at {} (run: npm run build in web)",
            web.display()
        ));
    }
    let runtime = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    runtime.block_on(server::run(&host, port, web))
}

fn open(args: &[String]) -> Result<(), String> {
    let url = format!("http://localhost:{}/", port(args)?);
    if args.iter().any(|a| a == "--browser") {
        return spawn("xdg-open", &[&url]);
    }
    // A Chromium app window: no tabs or address bar, its own taskbar entry.
    // Its own profile keeps it apart from the person's browsing.
    let profile = config::config_dir().join("chrome");
    let profile_arg = format!("--user-data-dir={}", profile.display());
    let app_arg = format!("--app={url}");
    for browser in ["google-chrome", "chromium", "chromium-browser"] {
        if which(browser) {
            return spawn(
                browser,
                &[
                    &profile_arg,
                    &app_arg,
                    "--no-first-run",
                    "--no-default-browser-check",
                ],
            );
        }
    }
    Err("no Chrome or Chromium found; use --browser to open it in the default browser".into())
}

fn which(program: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join(program).is_file()))
        .unwrap_or(false)
}

fn spawn(program: &str, args: &[&str]) -> Result<(), String> {
    std::process::Command::new(program)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("{program}: {e}"))
}

fn edit_list(args: &[String], add: bool) -> Result<(), String> {
    let folder = args.first().ok_or("which folder?")?;
    let mut list = config::ProjectList::load()?;
    if add {
        let path = list.add(&PathBuf::from(folder))?;
        list.save()?;
        if !path.join("design/status.json").is_file() {
            println!(
                "added {} (no design/status.json yet, so it shows as a plain folder)",
                path.display()
            );
        } else {
            println!("added {}", path.display());
        }
    } else {
        let path = std::fs::canonicalize(folder).unwrap_or_else(|_| PathBuf::from(folder));
        if !list.remove(&path) {
            return Err(format!("{} is not on the list", path.display()));
        }
        list.save()?;
        println!("removed {}", path.display());
    }
    Ok(())
}

fn list() -> Result<(), String> {
    for entry in config::ProjectList::load()?.projects {
        let s = project::summarize(&config::project_id(&entry.path), &entry.path, entry.pinned);
        let phase = s
            .phase
            .map(|p| format!("phase {p} {}", project::PHASES[p as usize]))
            .unwrap_or_else(|| "-".into());
        let flag = if !s.exists { "  (missing)" } else { "" };
        println!(
            "{:<24} {:<28} {}{flag}",
            s.board.unwrap_or(s.name),
            phase,
            entry.path.display()
        );
    }
    Ok(())
}

fn selection_cmd(args: &[String]) -> Result<(), String> {
    use axum::body::Body;
    use axum::http::{Request, StatusCode, Uri};
    use http_body_util::BodyExt;
    use hyper_util::{client::legacy::Client, rt::TokioExecutor};

    let pos = positionals(args, &["port", "project"], &[])?;
    let port = port(args)?;
    let target = flag(args, "project").or_else(|| pos.first().map(|s| (*s).to_string()));
    let project_id = match target {
        Some(t) => {
            let p = PathBuf::from(&t);
            if p.is_dir() {
                let canon =
                    std::fs::canonicalize(&p).map_err(|e| format!("{}: {e}", p.display()))?;
                config::project_id(&canon)
            } else {
                t
            }
        }
        None => resolve_cwd_project_id()?,
    };

    let encoded_id: String = project_id
        .bytes()
        .flat_map(|b| {
            if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
                vec![b as char]
            } else {
                format!("%{b:02X}").chars().collect()
            }
        })
        .collect();

    let uri: Uri = format!("http://127.0.0.1:{port}/api/selection?project={encoded_id}")
        .parse()
        .map_err(|e| format!("invalid selection URI: {e}"))?;

    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    let (status, bytes) = rt.block_on(async move {
        let client: Client<_, Body> = Client::builder(TokioExecutor::new()).build_http();
        let req = Request::builder()
            .method("GET")
            .uri(uri)
            .body(Body::empty())
            .map_err(|e| e.to_string())?;
        let resp = client
            .request(req)
            .await
            .map_err(|e| format!("can't reach Boardroom on 127.0.0.1:{port}: {e}"))?;
        let status = resp.status();
        let bytes = resp
            .into_body()
            .collect()
            .await
            .map_err(|e| e.to_string())?
            .to_bytes()
            .to_vec();
        Ok::<_, String>((status, bytes))
    })?;

    if status != StatusCode::OK {
        let msg = String::from_utf8_lossy(&bytes).trim().to_string();
        return Err(if msg.is_empty() {
            format!("server returned {status}")
        } else {
            msg
        });
    }
    let val: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|e| format!("invalid JSON from server: {e}"))?;
    println!(
        "{}",
        serde_json::to_string_pretty(&val).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn resolve_cwd_project_id() -> Result<String, String> {
    let cwd = std::env::current_dir().map_err(|e| format!("current_dir: {e}"))?;
    let canon = std::fs::canonicalize(&cwd).unwrap_or(cwd);
    if let Ok(list) = config::ProjectList::load()
        && let Some(entry) = list
            .projects
            .iter()
            .filter(|e| canon.starts_with(&e.path))
            .max_by_key(|e| e.path.as_os_str().len())
    {
        return Ok(config::project_id(&entry.path));
    }
    for anc in canon.ancestors() {
        if anc.join("design/status.json").is_file() {
            return Ok(config::project_id(anc));
        }
    }
    Ok(config::project_id(&canon))
}

fn link_cmd(args: &[String]) -> Result<(), String> {
    let (sub, rest) = args.split_first().ok_or(
        "usage: boardroom link <add|list|update|pin|follow|rehost|remove> <project-path> ...",
    )?;
    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    let cache = LinksCache::new();

    match sub.as_str() {
        "add" => {
            let pos = positionals(
                rest,
                &[
                    "id", "kind", "board", "remote", "path", "branch", "tag", "commit",
                ],
                &[],
            )?;
            let project_dir = pos
                .first()
                .ok_or("usage: boardroom link add <project-path> --id <id> --board <board> --remote <remote> --path <interface json path> [--branch main|--tag T|--commit C]")?;
            let root = PathBuf::from(project_dir);
            if !root.is_dir() {
                return Err(format!("{} is not a folder", root.display()));
            }
            let id = flag(rest, "id").ok_or("missing --id <id>")?;
            let board = flag(rest, "board").ok_or("missing --board <board>")?;
            let remote = flag(rest, "remote").ok_or("missing --remote <remote>")?;
            let path = flag(rest, "path").ok_or("missing --path <interface json path>")?;
            let kind = flag(rest, "kind").unwrap_or_else(|| "waffle".into());

            let branch = flag(rest, "branch");
            let tag = flag(rest, "tag");
            let commit = flag(rest, "commit");
            let ref_count = usize::from(branch.is_some())
                + usize::from(tag.is_some())
                + usize::from(commit.is_some());
            if ref_count > 1 {
                return Err("pass at most one of --branch, --tag, or --commit".into());
            }
            let git_ref = if let Some(name) = branch {
                GitRef::Branch { name }
            } else if let Some(name) = tag {
                GitRef::Tag { name }
            } else if let Some(c) = commit {
                GitRef::Commit { commit: c }
            } else {
                GitRef::Branch {
                    name: "main".into(),
                }
            };

            let req = AddLinkRequest {
                id,
                kind,
                board,
                remote: Some(remote),
                path: Some(path),
                git_ref: Some(git_ref),
                locator: None,
                pin_reason: String::new(),
            };
            let status = rt
                .block_on(links::add_link(&root, req, &cache))
                .map_err(|e| e.to_string())?;
            print_link_status(&status);
            Ok(())
        }
        "list" => {
            let pos = positionals(rest, &[], &["refresh", "json"])?;
            let project_dir = pos
                .first()
                .ok_or("usage: boardroom link list <project-path> [--refresh] [--json]")?;
            let root = PathBuf::from(project_dir);
            if !root.is_dir() {
                return Err(format!("{} is not a folder", root.display()));
            }
            let refresh = has_switch(rest, "refresh");
            let cands: Vec<PathBuf> = config::ProjectList::load()
                .map(|l| l.projects.into_iter().map(|p| p.path).collect())
                .unwrap_or_default();
            let statuses = rt.block_on(links::list_links(&root, &cache, refresh, true, &cands))?;
            if has_switch(rest, "json") {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&statuses).map_err(|e| e.to_string())?
                );
            } else if statuses.is_empty() {
                println!("no links in {}/design/links.toml", root.display());
            } else {
                for s in &statuses {
                    print_link_status(s);
                }
            }
            Ok(())
        }
        "update" => {
            let pos = positionals(rest, &["id"], &[])?;
            let project_dir = pos
                .first()
                .ok_or("usage: boardroom link update <project-path> <id>")?;
            let id = flag(rest, "id")
                .or_else(|| pos.get(1).map(|s| (*s).to_string()))
                .ok_or("usage: boardroom link update <project-path> <id>")?;
            let status = rt
                .block_on(links::update_link(
                    std::path::Path::new(project_dir),
                    &id,
                    &cache,
                ))
                .map_err(|e| e.to_string())?;
            print_link_status(&status);
            Ok(())
        }
        "pin" => {
            let pos = positionals(rest, &["id", "reason"], &[])?;
            let project_dir = pos
                .first()
                .ok_or("usage: boardroom link pin <project-path> <id> [--reason <why>]")?;
            let id = flag(rest, "id")
                .or_else(|| pos.get(1).map(|s| (*s).to_string()))
                .ok_or("usage: boardroom link pin <project-path> <id> [--reason <why>]")?;
            let reason = flag(rest, "reason").unwrap_or_default();
            let status = rt
                .block_on(links::pin_link(
                    std::path::Path::new(project_dir),
                    &id,
                    reason,
                    &cache,
                ))
                .map_err(|e| e.to_string())?;
            print_link_status(&status);
            Ok(())
        }
        "follow" => {
            let pos = positionals(rest, &["id", "branch"], &[])?;
            let project_dir = pos
                .first()
                .ok_or("usage: boardroom link follow <project-path> <id> [--branch <name>]")?;
            let id_from_flag = flag(rest, "id");
            let id = id_from_flag
                .clone()
                .or_else(|| pos.get(1).map(|s| (*s).to_string()))
                .ok_or("usage: boardroom link follow <project-path> <id> [--branch <name>]")?;
            let positional_branch_idx = if id_from_flag.is_some() { 1 } else { 2 };
            let branch = flag(rest, "branch")
                .or_else(|| pos.get(positional_branch_idx).map(|s| (*s).to_string()))
                .unwrap_or_else(|| "main".into());
            let status = rt
                .block_on(links::follow_link(
                    std::path::Path::new(project_dir),
                    &id,
                    branch,
                    &cache,
                ))
                .map_err(|e| e.to_string())?;
            print_link_status(&status);
            Ok(())
        }
        "rehost" => {
            let pos = positionals(rest, &["id"], &[])?;
            let project_dir = pos
                .first()
                .ok_or("usage: boardroom link rehost <project-path> <id>")?;
            let id = flag(rest, "id")
                .or_else(|| pos.get(1).map(|s| (*s).to_string()))
                .ok_or("usage: boardroom link rehost <project-path> <id>")?;
            let status = rt
                .block_on(links::rehost_link(
                    std::path::Path::new(project_dir),
                    &id,
                    &cache,
                ))
                .map_err(|e| e.to_string())?;
            print_link_status(&status);
            Ok(())
        }
        "remove" => {
            let pos = positionals(rest, &["id"], &[])?;
            let project_dir = pos
                .first()
                .ok_or("usage: boardroom link remove <project-path> <id>")?;
            let id = flag(rest, "id")
                .or_else(|| pos.get(1).map(|s| (*s).to_string()))
                .ok_or("usage: boardroom link remove <project-path> <id>")?;
            links::remove_link(std::path::Path::new(project_dir), &id)
                .map_err(|e| e.to_string())?;
            println!("removed link {id}");
            Ok(())
        }
        other => Err(format!(
            "unknown link subcommand '{other}' (expected add, list, update, pin, follow, rehost, remove)"
        )),
    }
}

fn push_cmd(args: &[String]) -> Result<(), String> {
    let pos = positionals(args, &[], &[])?;
    let project_dir = pos.first().ok_or("usage: boardroom push <project-path>")?;
    let root = PathBuf::from(project_dir);
    if !root.is_dir() {
        return Err(format!("{} is not a folder", root.display()));
    }
    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    let res = rt
        .block_on(links::push_upstream(&root))
        .map_err(|e| e.to_string())?;
    if res.output.trim().is_empty() {
        println!("pushed to {}/{}", res.remote, res.branch);
    } else {
        print!("{}", res.output);
        if !res.output.ends_with('\n') {
            println!();
        }
    }
    Ok(())
}

fn print_link_status(s: &LinkStatus) {
    let short = &s.resolved.commit[..s.resolved.commit.len().min(7)];
    let locked = s
        .locked_by
        .map(|d| format!("  [locked by {}]", d.as_str()))
        .unwrap_or_default();
    let scope = if s.local_only {
        "  [this machine only]"
    } else {
        ""
    };
    println!(
        "{:<16} {:<12} {:<28} @ {}{}{}",
        s.id,
        format!("{}:{}", s.kind, s.board),
        s.state.as_str(),
        short,
        locked,
        scope
    );
    if let Some(err) = &s.error {
        println!("  error: {err}");
    }
    if let Some(hint) = &s.hint {
        println!("  hint: {hint}");
    }
    if s.local_only
        && let Some(hosted) = &s.hosted_remote
    {
        println!("  hosted remote: {hosted}");
    }
    if let Some(u) = &s.unpushed {
        let name = u
            .project_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| u.project_path.display().to_string());
        println!(
            "  unpushed: {name} on {} (commits={}, uncommitted={})",
            u.branch, u.commits, u.uncommitted
        );
    }
    if !s.pin_reason.is_empty() {
        println!("  pin reason: {}", s.pin_reason);
    }
    if let Some((old_t, new_t)) = s.thickness_changed {
        println!(
            "  thickness changed from {:.2} mm to {:.2} mm",
            old_t as f64 / 1_000_000.0,
            new_t as f64 / 1_000_000.0
        );
    }
    for c in &s.changes {
        println!("  {}", format_change_line(c));
    }
}

fn format_change_line(c: &board_interface::Change) -> String {
    let role = match c.role {
        board_interface::Role::Outline => "outline",
        board_interface::Role::Cutout => "cutout",
        board_interface::Role::Hole => "hole",
        board_interface::Role::Keepout => "keepout",
    };
    let label = match &c.id {
        Some(id) => format!("{role} {id}"),
        None => role.to_string(),
    };
    match c.kind {
        board_interface::ChangeKind::Added => format!("{label} added"),
        board_interface::ChangeKind::Removed => format!("{label} removed"),
        board_interface::ChangeKind::Changed => format!("{label} changed"),
        board_interface::ChangeKind::Moved { dx_nm, dy_nm } => {
            let mut parts = Vec::new();
            if dx_nm != 0 {
                let dir = if dx_nm > 0 { "right" } else { "left" };
                parts.push(format!("{:.2} mm {dir}", dx_nm.abs() as f64 / 1_000_000.0));
            }
            if dy_nm != 0 {
                let dir = if dy_nm > 0 { "down" } else { "up" };
                parts.push(format!("{:.2} mm {dir}", dy_nm.abs() as f64 / 1_000_000.0));
            }
            format!("{label} moved {}", parts.join(", "))
        }
    }
}

/// The folder a board's routing goes in: `route/<board>/` in its project, the folder above its
/// `hw/` (`hw/<board>/<variant>/<board>.kicad_pcb`), else beside the board.
fn route_dir(board: &std::path::Path) -> PathBuf {
    let stem = board.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "board".into());
    let root = board.ancestors().find(|a| a.file_name().is_some_and(|n| n == "hw")).and_then(|hw| hw.parent()).or_else(|| board.parent()).unwrap_or(std::path::Path::new("."));
    root.join("route").join(stem)
}

fn route_cmd(args: &[String]) -> Result<(), String> {
    // The old router, whole and with its own words (`boardroom route --mode old IN.kicad_pcb
    // -o OUT.kicad_pcb ...`, `--help` for them): the same code on the same board model, a
    // second way to route beside the new one.
    let mode = flag(args, "mode");
    let args: Vec<String> = match args.iter().position(|a| a == "--mode") {
        Some(i) => args.iter().enumerate().filter(|(k, _)| *k != i && *k != i + 1).map(|(_, a)| a.clone()).collect(),
        None => args.to_vec(),
    };
    let args = &args[..];
    match mode.as_deref() {
        None | Some("new") => {}
        Some("old") => {
            ultraroute::cli::run(std::iter::once("route".to_string()).chain(args.iter().cloned()).collect());
            return Ok(());
        }
        Some(other) => return Err(format!("--mode is old or new, not {other}")),
    }
    // The board: the first word that is neither a flag nor the value after `--out`.
    const USAGE: &str = "usage: boardroom route <board.kicad_pcb> [--out DIR] [--packages DIR] [--filled-vias yes|no] [--save-every SECONDS] [--close old|grid [--close-seconds S]]";
    let board = args.iter().enumerate().find(|(i, a)| !a.starts_with("--") && (*i == 0 || !["--out", "--packages", "--filled-vias", "--save-every", "--close", "--close-seconds"].contains(&args[i - 1].as_str()))).map(|(_, a)| a).ok_or(USAGE)?;
    let board = PathBuf::from(board);
    let out = flag(args, "out").map(PathBuf::from).unwrap_or_else(|| route_dir(&board));
    let packages = flag(args, "packages").map(PathBuf::from).unwrap_or_else(|| repo_dir().join("packages"));
    // Whether the board is ordered with its vias filled and capped: said here, or taken from
    // how many layers it has.
    let filled = match flag(args, "filled-vias").as_deref() {
        None => None,
        Some("yes") => Some(true),
        Some("no") => Some(false),
        Some(_) => return Err(USAGE.into()),
    };
    // How often the board so far is written while the run goes on.
    let every = match flag(args, "save-every") {
        None => route::partial::SAVE_EVERY,
        Some(s) => s.parse().map_err(|_| USAGE)?,
    };
    let report = route::route_saving(&board, &out, Some(&packages), filled, every)?;
    // The report on stdout, one line, for scripts; what matters on stderr, for people.
    println!("{report}");
    for note in report["notes"].as_array().into_iter().flatten() {
        eprintln!("note: {}", note.as_str().unwrap_or(""));
    }
    for s in report["solid"].as_array().into_iter().flatten() {
        eprintln!("{} ({}) taken as solid: {}", s["layer"].as_str().unwrap_or(""), s["net"].as_str().unwrap_or(""), s["why"].as_str().unwrap_or(""));
    }
    eprintln!("{} connections open, {} faults; written to {}", report["open"], report["faults"].as_object().map_or(0, |f| f.values().filter_map(|v| v.as_u64()).sum::<u64>()), out.display());
    // The old router on what is left open, its copper kept as it lays it.
    match flag(args, "close").as_deref() {
        None => Ok(()),
        Some(word @ ("old" | "grid")) => closing(args, &board, &out, word, filled, USAGE),
        Some(_) => Err(USAGE.into()),
    }
}

/// The closing of `boardroom route --close WORD`, on `out/routed.kicad_pcb`, and what it did
/// said on stderr.
fn closing(args: &[String], board: &std::path::Path, out: &std::path::Path, word: &str, filled: Option<bool>, usage: &str) -> Result<(), String> {
    let with = if word == "grid" { route::close::With::Grid } else { route::close::With::Old };
    let seconds = match flag(args, "close-seconds") {
        None => route::close::SECONDS,
        Some(s) => s.parse().map_err(|_| usage.to_string())?,
    };
    let threads = std::env::var("RESERVED_CORES").ok().and_then(|c| c.parse().ok()).unwrap_or(4);
    let closed = route::close::close(board, out, with, seconds, threads, filled)?;
    if closed["ran"] == true {
        eprintln!(
            "closed by the old router{}: {} open to {}, {} of {} nets' copper changed{}, {} s{}; {}",
            if with == route::close::With::Grid { "'s grid search" } else { "" },
            closed["open_before"], closed["open_after"], closed["nets_changed"], closed["nets"],
            match closed["put_back"].as_array().map_or(0, |b| b.len()) {
                0 => String::new(),
                n => format!(" ({n} more put back as they were: nothing closed on them)"),
            },
            closed["seconds"],
            if closed["out_of_time"] == true { " (out of time)" } else { "" },
            if closed["kept"] == true { "closed.kicad_pcb written: look at it" } else { "not kept: no fewer open, or more faults or pads short of spokes" }
        );
        let again = &closed["redrawn"];
        if again.is_object() {
            eprintln!(
                "drawn again by the new router: {} of the old router's {} tracks and vias ({} nets wholly, {} in part, {} left as laid), {} s; {}",
                again["pieces_redrawn"], again["pieces_new"], again["nets_redrawn_whole"], again["nets_redrawn_in_part"], again["nets_as_laid"], again["seconds"],
                if again["kept"] == true { "redrawn.kicad_pcb written: look at it" } else { "not kept: nothing could be drawn again, or it left more open or more faults" }
            );
        }
    } else {
        eprintln!("nothing open: the old router was not run");
    }
    Ok(())
}

/// `boardroom close`: the closing alone, on a board already routed.
fn close_cmd(args: &[String]) -> Result<(), String> {
    const USAGE: &str = "usage: boardroom close <board.kicad_pcb> [--out DIR] [--with old|grid] [--close-seconds S] [--filled-vias yes|no]";
    let board = args.iter().enumerate().find(|(i, a)| !a.starts_with("--") && (*i == 0 || !["--out", "--with", "--filled-vias", "--close-seconds"].contains(&args[i - 1].as_str()))).map(|(_, a)| a).ok_or(USAGE)?;
    let board = PathBuf::from(board);
    let out = flag(args, "out").map(PathBuf::from).unwrap_or_else(|| route_dir(&board));
    let filled = match flag(args, "filled-vias").as_deref() {
        None => None,
        Some("yes") => Some(true),
        Some("no") => Some(false),
        Some(_) => return Err(USAGE.into()),
    };
    if !out.join("routed.kicad_pcb").exists() {
        return Err(format!("{}: no routed.kicad_pcb there: route the board first (boardroom route)", out.display()));
    }
    match flag(args, "with").as_deref() {
        None | Some("old") => closing(args, &board, &out, "old", filled, USAGE),
        Some("grid") => closing(args, &board, &out, "grid", filled, USAGE),
        Some(_) => Err(USAGE.into()),
    }
}

fn redraw_cmd(args: &[String]) -> Result<(), String> {
    const USAGE: &str = "usage: boardroom redraw <before.kicad_pcb> <after.kicad_pcb> --out <out.kicad_pcb> [--filled-vias yes|no]";
    let words = positionals(args, &["out", "filled-vias"], &[])?;
    let (Some(before), Some(after), Some(out)) = (words.first(), words.get(1), flag(args, "out")) else { return Err(USAGE.into()) };
    let filled = match flag(args, "filled-vias").as_deref() {
        None => None,
        Some("yes") => Some(true),
        Some("no") => Some(false),
        Some(_) => return Err(USAGE.into()),
    };
    let said = route::close::redraw(std::path::Path::new(before), std::path::Path::new(after), std::path::Path::new(&out), filled)?;
    println!("{said}");
    eprintln!(
        "{} of {} new tracks and vias drawn again ({} nets wholly, {} in part, {} left as laid); {} open to {}, {} faults to {}; {}",
        said["pieces_redrawn"], said["pieces_new"], said["nets_redrawn_whole"], said["nets_redrawn_in_part"], said["nets_as_laid"],
        said["open_before"], said["open_after"], said["faults_before"], said["faults_after"],
        if said["kept"] == true { format!("{out} written") } else { "not kept".to_string() }
    );
    for l in said["as_laid"].as_array().into_iter().flatten() {
        eprintln!("  as laid: {} of {}: {}", l["pieces"], l["net"].as_str().unwrap_or(""), l["why"].as_str().unwrap_or(""));
    }
    Ok(())
}

fn check_cmd(args: &[String]) -> Result<(), String> {
    let board = args.iter().find(|a| !a.starts_with("--")).ok_or("usage: boardroom check <board.kicad_pcb> [--json]")?;
    let report = route::check(std::path::Path::new(board))?;
    if has_switch(args, "json") {
        println!("{report}");
    } else {
        for f in report["fault_list"].as_array().into_iter().flatten() {
            let at = f["at"].as_array().map(|a| format!(" at ({:.3}, {:.3})", a[0].as_f64().unwrap_or(0.0), a[1].as_f64().unwrap_or(0.0))).unwrap_or_default();
            let items: Vec<&str> = f["items"].as_array().into_iter().flatten().filter_map(|i| i.as_str()).collect();
            println!("{} on {}{at}: {}", f["rule"].as_str().unwrap_or(""), f["layer"].as_str().unwrap_or("all layers"), items.join(", "));
        }
        for o in report["open_list"].as_array().into_iter().flatten() {
            println!("open: {} from ({:.3}, {:.3}) to ({:.3}, {:.3})", o["net"].as_str().unwrap_or(""), o["from"][0].as_f64().unwrap_or(0.0), o["from"][1].as_f64().unwrap_or(0.0), o["to"][0].as_f64().unwrap_or(0.0), o["to"][1].as_f64().unwrap_or(0.0));
        }
        println!("{} faults, {} connections open", report["faults"], report["open"]);
    }
    if report["faults"].as_u64() == Some(0) && report["open"].as_u64() == Some(0) {
        Ok(())
    } else {
        Err("the board is not clean".into())
    }
}

/// Each pour's pieces after a refill: the pads each joins, its own vias and tracks, and its
/// necks, narrowest first, with the copper that pinches each. Without `--zone`, one line a zone.
fn pours_cmd(args: &[String]) -> Result<(), String> {
    let words = positionals(args, &["zone", "cut-by"], &["json"])?;
    let board = words.first().ok_or("usage: boardroom pours <board.kicad_pcb> [--zone NAME] [--cut-by NET[,NET]] [--json]")?;
    let zone = flag(args, "zone");
    let cut_by: Vec<String> = flag(args, "cut-by").map(|c| c.split(',').map(str::to_string).collect()).unwrap_or_default();
    let report = route::pours(std::path::Path::new(board), zone.as_deref(), &cut_by)?;
    if has_switch(args, "json") {
        println!("{report}");
        return Ok(());
    }
    let narrowest = |p: &serde_json::Value| p["necks"][0]["width_mm"].as_f64();
    for z in report["zones"].as_array().into_iter().flatten() {
        let pieces = z["pieces"].as_array().cloned().unwrap_or_default();
        let neck = pieces.iter().filter_map(narrowest).fold(None, |m: Option<f64>, w| Some(m.map_or(w, |m| m.min(w))));
        let name = z["name"].as_str().filter(|n| !n.is_empty()).map_or(format!("zone {}", z["zone"]), str::to_string);
        let links = z["links"].as_array().cloned().unwrap_or_default();
        let joined: Vec<String> = links
            .iter()
            .filter(|l| l["pins"].as_u64().unwrap_or(0) > 1)
            .map(|l| {
                let groups = l["groups"].as_array().map_or(0, Vec::len);
                let weakest = l["groups"].as_array().into_iter().flatten().filter_map(|g| g["weakest"]["width_mm"].as_f64()).fold(f64::INFINITY, f64::min);
                let how = if groups == 1 { "all joined inside".to_string() } else { format!("in {groups} groups inside") };
                format!("{} pins on {} {how}{}", l["pins"], l["layer"].as_str().unwrap_or(""), if weakest.is_finite() { format!(", weakest link {weakest:.3} mm") } else { String::new() })
            })
            .collect();
        println!("{name} ({}): {} piece{}{}{}", z["net"].as_str().unwrap_or(""), pieces.len(), if pieces.len() == 1 { "" } else { "s" }, neck.map_or(String::new(), |w| format!(", narrowest neck {w:.3} mm")), if joined.is_empty() { String::new() } else { format!("; {}", joined.join("; ")) });
        if zone.is_none() {
            continue;
        }
        let describe_neck = |n: &serde_json::Value| {
            let by: Vec<String> = n["by"].as_array().into_iter().flatten().map(|b| format!("{} ({:.3} mm)", b["item"].as_str().unwrap_or(""), b["gap_mm"].as_f64().unwrap_or(0.0))).collect();
            let limit = if n["at_limit"].as_bool() == Some(true) { " (at the pour's minimum width: KiCad may drop it)" } else { "" };
            format!("{:.3} mm{limit} at ({:.2}, {:.2}), pinched by {}", n["width_mm"].as_f64().unwrap_or(0.0), n["at"][0].as_f64().unwrap_or(0.0), n["at"][1].as_f64().unwrap_or(0.0), if by.is_empty() { "the zone's edge".into() } else { by.join(", ") })
        };
        let sections = z["sections"].as_array().cloned().unwrap_or_default();
        if !sections.is_empty() {
            let ok = sections.iter().filter(|x| x["ok"].as_bool() == Some(true)).count();
            println!("  {} sections as {} alone cut it, {ok} whole with every link at least {:.3} mm (the fab's narrowest track for this stackup):", sections.len(), cut_by.join(", "), z["min_width_mm"].as_f64().unwrap_or(0.0));
            for x in &sections {
                let pins: Vec<&str> = x["pins"].as_array().into_iter().flatten().filter_map(|v| v.as_str()).collect();
                let groups = x["groups"].as_array().map_or(0, Vec::len);
                let state = match (x["ok"].as_bool() == Some(true), groups) {
                    (true, _) => "whole".to_string(),
                    (false, 1) => "whole but too narrow".to_string(),
                    (false, n) => format!("broken in {n}: {}", x["groups"].as_array().into_iter().flatten().map(|g| g.as_array().into_iter().flatten().filter_map(|v| v.as_str()).collect::<Vec<_>>().join("+")).collect::<Vec<_>>().join(" / ")),
                };
                let weakest = if x["weakest"].is_null() { String::new() } else { format!(", weakest link {}", describe_neck(&x["weakest"])) };
                println!("    pins {} on {}: {state}{weakest}", pins.join(", "), x["layer"].as_str().unwrap_or(""));
            }
            continue;
        }
        for l in &links {
            for g in l["groups"].as_array().into_iter().flatten() {
                let pins: Vec<&str> = g["pins"].as_array().into_iter().flatten().filter_map(|v| v.as_str()).collect();
                match g["weakest"].is_null() {
                    true => println!("  pins {} on {}: alone", pins.join(", "), l["layer"].as_str().unwrap_or("")),
                    false => println!("  pins {} on {}: joined inside, weakest link {}", pins.join(", "), l["layer"].as_str().unwrap_or(""), describe_neck(&g["weakest"])),
                }
            }
        }
        for (i, p) in pieces.iter().enumerate() {
            let pads: Vec<&str> = p["pads"].as_array().into_iter().flatten().filter_map(|v| v.as_str()).collect();
            println!("  piece {} on {}: {:.2} mm² at ({:.2}, {:.2}); {} own vias, {} own tracks; pads: {}", i + 1, p["layer"].as_str().unwrap_or(""), p["area_mm2"].as_f64().unwrap_or(0.0), p["at"][0].as_f64().unwrap_or(0.0), p["at"][1].as_f64().unwrap_or(0.0), p["vias"], p["tracks"], if pads.is_empty() { "none".into() } else { pads.join(", ") });
            for n in p["necks"].as_array().into_iter().flatten().take(3) {
                println!("    neck {}", describe_neck(n));
            }
        }
    }
    Ok(())
}

const BLOCKS_USAGE: &str = "usage: boardroom blocks <board.kicad_pcb> [--pin REF=PACKAGE/LAYOUT] [--accept REF[:NAME]|all] [--decline REF:NAME] [--packages DIR]\n       boardroom blocks --behind <dir> [--packages DIR]";

/// Each pinned chip of a board against its package: what differs, accepted (written from the
/// source) or declined (recorded beside the board, not asked again until the source changes it).
fn blocks_cmd(args: &[String]) -> Result<(), String> {
    let root = flag(args, "packages").map(PathBuf::from).unwrap_or_else(|| repo_dir().join("packages"));
    if let Some(dir) = flag(args, "behind") {
        return blocks_behind(&PathBuf::from(dir), &root);
    }
    let words = positionals(args, &["pin", "accept", "decline", "packages"], &[])?;
    let board = PathBuf::from(words.first().ok_or(BLOCKS_USAGE)?);
    let files = packages::BoardFiles::load(&board)?;
    let mut pins = packages::Pins::load(&board)?;
    if let Some(pin) = flag(args, "pin") {
        let (chip, source) = pin.split_once('=').ok_or(BLOCKS_USAGE)?;
        let (package, layout) = source.split_once('/').ok_or(BLOCKS_USAGE)?;
        // Placing it once checks the chip matches the layout before the pin is recorded.
        let p = packages::Package::load(&root.join(package))?;
        packages::place(&p, layout, &files.pcb, chip)?;
        pins.chips.insert(chip.to_string(), packages::Pin { package: package.into(), version: String::new(), layout: layout.into(), declined: Default::default() });
        pins.save(&board)?;
    }
    let mut chips = packages::chips(&files, &pins, &root)?;
    let pick = |spec: &str, chips: &std::collections::BTreeMap<String, packages::Chip>| -> Result<Vec<(String, usize)>, String> {
        let (chip, name) = spec.split_once(':').map_or((spec, None), |(c, n)| (c, Some(n)));
        let mut out = Vec::new();
        for (c, ch) in chips.iter().filter(|(c, _)| chip == "all" || c.as_str() == chip) {
            for i in ch.outstanding() {
                if name.is_none_or(|n| ch.placed.entries[i].name == n) {
                    out.push((c.clone(), i));
                }
            }
        }
        if out.is_empty() {
            return Err(format!("nothing waiting matches {spec}"));
        }
        Ok(out)
    };
    if let Some(spec) = flag(args, "decline") {
        for (c, i) in pick(&spec, &chips)? {
            let ch = chips.get_mut(&c).unwrap();
            let e = &ch.placed.entries[i];
            ch.pin.declined.insert(e.name.clone(), e.since.clone());
            pins.chips.insert(c.clone(), ch.pin.clone());
            eprintln!("{c}: declined {}", e.name);
        }
    }
    if let Some(spec) = flag(args, "accept") {
        let chosen = pick(&spec, &chips)?;
        let (mut text, mut dru) = (files.text.clone(), files.dru.clone());
        // One chip at a time, each against the files as the last left them.
        for c in chosen.iter().map(|(c, _)| c.clone()).collect::<std::collections::BTreeSet<_>>() {
            let ch = &chips[&c];
            let mine: Vec<usize> = chosen.iter().filter(|(x, _)| *x == c).map(|(_, i)| *i).collect();
            let zones = packages::zones_of(&text)?;
            let found = packages::compare(&ch.placed, &zones, &dru);
            (text, dru) = packages::accept(&text, &zones, &dru, &ch.placed, &found, &mine);
            for &i in &mine {
                eprintln!("{c}: wrote {} from {}@{}", ch.placed.entries[i].name, ch.placed.package, ch.placed.version);
            }
        }
        files.write(&text, &dru)?;
        let files = packages::BoardFiles::load(&board)?;
        chips = packages::chips(&files, &pins, &root)?;
    }
    // A chip with nothing waiting has taken its source's version.
    for (c, ch) in chips.iter_mut() {
        if ch.outstanding().is_empty() && ch.pin.version != ch.placed.version {
            ch.pin.version = ch.placed.version.clone();
            pins.chips.insert(c.clone(), ch.pin.clone());
        }
    }
    if pins != packages::Pins::load(&board)? {
        pins.save(&board)?;
    }
    if chips.is_empty() {
        println!("no chip of {} is pinned to a package (pin one with --pin U3=rp2350/qfn60-reference)", board.display());
    }
    let files_now = packages::BoardFiles::load(&board)?;
    for (c, ch) in &chips {
        let p = &ch.placed;
        let pinned = if ch.pin.version.is_empty() { "nothing taken yet".to_string() } else { format!("at {}", ch.pin.version) };
        println!("{c}: {} {} ({pinned}; source {})", p.package, p.layout, p.version);
        for (i, e) in p.entries.iter().enumerate() {
            let what = if e.is_pour() { "copper" } else { "rule area" };
            let mark = match &ch.found[i].state {
                packages::State::Same => continue,
                _ if ch.pin.declines(e) => "declined",
                packages::State::Missing => "missing",
                packages::State::Differs(_) => "differs",
            };
            println!("  {mark:8} {} ({what}, since {}): {}", e.name, e.since, e.why);
            if let packages::State::Differs(d) = &ch.found[i].state {
                for d in d {
                    println!("           - {d}");
                }
            }
        }
        if ch.placed.entries.iter().any(|e| e.keep_joined.is_some()) {
            let checked = files_now.checked()?;
            for (i, sections) in ch.sections(&checked, &files_now.pcb) {
                let e = &p.entries[i];
                let ok = sections.iter().filter(|s| s.ok).count();
                let cut = e.keep_joined.as_deref().unwrap_or_default().join(", ");
                let min = ch.found[i].zones.first().map_or(0, |&z| packages::fab_min(&checked, &files_now.pcb, z));
                println!("  joined   {}: {ok} of {} sections whole (cut only by {cut}; each link at least {:.3} mm, the fab's narrowest track for this stackup)", e.name, sections.len(), min as f64 / 1e6);
                for s in &sections {
                    println!("           - {}", packages::describe_section(&checked, s));
                }
            }
        }
        let waiting = ch.outstanding().len();
        let same = ch.found.iter().filter(|f| f.state == packages::State::Same).count();
        println!("  {same} of {} as the source has them, {waiting} waiting", p.entries.len());
    }
    Ok(())
}

/// Every board under `dir` with pins, and whether it is behind its sources.
fn blocks_behind(dir: &std::path::Path, root: &std::path::Path) -> Result<(), String> {
    fn walk(d: &std::path::Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        let mut entries: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
        entries.sort();
        for p in entries {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if p.is_dir() && !name.starts_with('.') && !matches!(name, "target" | "node_modules" | "vendor") {
                walk(&p, out);
            } else if name.ends_with(".blocks.json") {
                out.push(p);
            }
        }
    }
    let mut found = Vec::new();
    walk(dir, &mut found);
    let mut behind = 0;
    for pins_file in &found {
        let board = PathBuf::from(pins_file.to_string_lossy().replace(".blocks.json", ".kicad_pcb"));
        let files = packages::BoardFiles::load(&board)?;
        let chips = packages::chips(&files, &packages::Pins::load(&board)?, root)?;
        for (c, ch) in &chips {
            if ch.behind() {
                behind += 1;
                println!("{} {c}: {} at {} (source {}), {} waiting", board.display(), ch.placed.package, if ch.pin.version.is_empty() { "-" } else { &ch.pin.version }, ch.placed.version, ch.outstanding().len());
            }
        }
    }
    println!("{} boards with pins, {behind} chips behind their sources", found.len());
    Ok(())
}

#[cfg(test)]
mod route_tests {
    use super::route_dir;
    use std::path::{Path, PathBuf};

    #[test]
    fn a_board_in_hw_routes_into_its_projects_route_folder() {
        assert_eq!(route_dir(Path::new("/p/hw/pendant/a/pendant.kicad_pcb")), PathBuf::from("/p/route/pendant"));
        assert_eq!(route_dir(Path::new("/x/y/board.kicad_pcb")), PathBuf::from("/x/y/route/board"));
    }
}
