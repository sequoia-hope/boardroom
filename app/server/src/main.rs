//! Boardroom: the project manager and main window of the board design suite.
//!
//!     boardroom serve [--port N] [--host ADDR]   run the server
//!     boardroom open  [--port N] [--browser]     open the Boardroom window
//!     boardroom add <folder>                     add a project to the list
//!     boardroom remove <folder>                  take one off the list
//!     boardroom list                             the list, with each project's phase
//!     boardroom snapshot <out> <folder>...       a static copy for the web-only build
//!
//! The port comes from --port, then $PORT, then `proj port`; with none the
//! command fails rather than guessing (see docs/shell-spec.md).

mod apps;
mod config;
mod project;
mod server;
mod snapshot;

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

/// The repository this binary was built from: where `proj port` resolves to
/// Boardroom's registered port, and where the web build lives.
fn repo_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn port(args: &[String]) -> Result<u16, String> {
    let from_proj = || {
        let out = std::process::Command::new("proj").arg("port").current_dir(repo_dir()).output().ok()?;
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
    };
    let text = flag(args, "port")
        .or_else(|| std::env::var("PORT").ok().filter(|p| !p.is_empty()))
        .or_else(from_proj)
        .ok_or("no port. Pass --port N or set PORT (or register the project with `proj add`).")?;
    text.parse().map_err(|_| format!("port '{text}' is not a number"))
}

fn serve(args: &[String]) -> Result<(), String> {
    let port = port(args)?;
    // Localhost only unless asked: the server opens files and starts programs.
    let host = flag(args, "host").unwrap_or_else(|| "127.0.0.1".into());
    let web = flag(args, "web").map(PathBuf::from).unwrap_or_else(|| repo_dir().join("app/web/dist"));
    if !web.join("index.html").is_file() {
        return Err(format!("no web build at {} (run: npm run build in app/web)", web.display()));
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
            return spawn(browser, &[&profile_arg, &app_arg, "--no-first-run", "--no-default-browser-check"]);
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
            println!("added {} (no design/status.json yet, so it shows as a plain folder)", path.display());
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
        let phase = s.phase.map(|p| format!("phase {p} {}", project::PHASES[p as usize])).unwrap_or_else(|| "-".into());
        let flag = if !s.exists { "  (missing)" } else { "" };
        println!("{:<24} {:<28} {}{flag}", s.board.unwrap_or(s.name), phase, entry.path.display());
    }
    Ok(())
}
