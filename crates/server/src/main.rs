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
//!
//! The port comes from --port, then $PORT, then `proj port`; with none the
//! command fails rather than guessing (see docs/shell-spec.md).

use boardroom::links::{self, AddLinkRequest, GitRef, LinkStatus, LinksCache};
use boardroom::{config, project, repo_dir, server, snapshot};
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
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
        "help" | "-h" | "--help" => {
            // The usage is the module comment's command list.
            let usage: Vec<&str> = include_str!("main.rs")
                .lines()
                .filter_map(|l| l.strip_prefix("//!     "))
                .filter(|l| l.starts_with("boardroom"))
                .collect();
            println!("{}", usage.join("\n"));
            Ok(())
        }
        other => Err(format!("unknown command '{other}' (try: boardroom help)")),
    };
    if let Err(e) = result {
        eprintln!("boardroom: {e}");
        std::process::exit(1);
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
    let from_proj = || {
        let out = std::process::Command::new("proj")
            .arg("port")
            .current_dir(repo_dir())
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
    };
    let text = flag(args, "port")
        .or_else(|| std::env::var("PORT").ok().filter(|p| !p.is_empty()))
        .or_else(from_proj)
        .ok_or("no port. Pass --port N or set PORT (or register the project with `proj add`).")?;
    text.parse()
        .map_err(|_| format!("port '{text}' is not a number"))
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
