//! Boardroom's desktop window: the same web app, in GTK 4 windows with
//! WebKitGTK, so it looks and behaves like the rest of the desktop.
//!
//!     boardroom-gtk [--port N] [--path /?project=…]
//!     boardroom-gtk --snapshot out.png [--path …] [--size 1440x900] [--eval JS|@FILE]
//!     boardroom-gtk --list-features
//!
//! --snapshot is a self-test: it loads the page, runs `--eval` (as an async JS
//! body) or opens one popup the way the page's window.open does, saves the view
//! to a PNG, prints what the page and window report, and quits.
//! --list-features prints WebKit's runtime features and their state.
//!
//! The server is the app; this is only its window. It uses the server on the
//! registered port (--port, then $PORT, then `proj port`), and starts one if
//! none answers, stopping it again on exit. Every `window.open` from the page
//! (the viewer, Circuit Composer, Waffle Iron) becomes a GTK window of its own
//! sharing one persistent `NetworkSession`; links to other sites open in the
//! default browser.

use gtk::prelude::*;
use gtk::{gio, glib};
use std::cell::{Cell, OnceCell, RefCell};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::rc::Rc;
use std::time::{Duration, Instant};
use webkit::prelude::*;

const APP_ID: &str = "io.github.sequoia_hope.Boardroom";

struct Options {
    port: u16,
    path: String,
    snapshot: Option<PathBuf>,
    eval: Option<String>,
    size: (i32, i32),
}

#[derive(Clone)]
struct NoticeBar {
    revealer: gtk::Revealer,
    label: gtk::Label,
    open_folder: gtk::Button,
    folder: Rc<RefCell<Option<PathBuf>>>,
}

type Notices = Rc<RefCell<Vec<(glib::WeakRef<gtk::ApplicationWindow>, NoticeBar)>>>;
type DownloadLog = Rc<RefCell<Vec<String>>>;

#[derive(Clone)]
struct ShellState {
    session: webkit::NetworkSession,
    notices: Notices,
    downloads: DownloadLog,
}

fn main() -> glib::ExitCode {
    // WebKitGTK keeps SharedArrayBuffer behind a JavaScriptCore option even on
    // cross-origin-isolated pages (checked 2026-10-30, WebKitGTK 2.52), and
    // UltraRoute Studio routes in the page on workers that share memory. The
    // web process inherits the environment, so set it before WebKit starts.
    if std::env::var_os("JSC_useSharedArrayBuffer").is_none() {
        // SAFETY: nothing else is running yet; no other thread reads the environment.
        unsafe { std::env::set_var("JSC_useSharedArrayBuffer", "1") };
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--list-features") {
        // WebKit's runtime features: what can be switched with set_feature_enabled.
        let Some(list) = webkit::Settings::all_features() else { return glib::ExitCode::FAILURE };
        for i in 0..list.length() {
            let Some(f) = list.get(i) else { continue };
            println!("{:<44} {:<5} {:?}", f.identifier().unwrap_or_default(), f.is_default_value(), f.status());
        }
        return glib::ExitCode::SUCCESS;
    }
    let options = match options(&args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("boardroom-gtk: {e}");
            return glib::ExitCode::FAILURE;
        }
    };
    let server = match ensure_server(options.port) {
        Ok(child) => child,
        Err(e) => {
            eprintln!("boardroom-gtk: {e}");
            return glib::ExitCode::FAILURE;
        }
    };

    let app = gtk::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    let origin = format!("http://localhost:{}", options.port);
    let url = format!("{origin}{}", options.path);
    let snapshot = options.snapshot.clone();
    let eval = options.eval.clone();
    let size = options.size;
    let session_cell: Rc<OnceCell<webkit::NetworkSession>> = Rc::new(OnceCell::new());
    let notices: Notices = Rc::new(RefCell::new(Vec::new()));
    let downloads: DownloadLog = Rc::new(RefCell::new(Vec::new()));

    app.connect_activate(move |app| {
        let session = session_cell
            .get_or_init(|| build_session(app, &notices, &downloads, snapshot.is_some()))
            .clone();
        let view = glib::Object::builder::<webkit::WebView>()
            .property("network-session", &session)
            .build();
        setup(&view, &origin, &notices, snapshot.is_some());
        let win = window(app, &view, size, &notices);
        view.load_uri(&url);
        if let Some(path) = snapshot.clone() {
            let state = ShellState { session, notices: notices.clone(), downloads: downloads.clone() };
            snapshot_when_loaded(&view, &win, state, path, eval.clone(), app.clone());
        }
        win.present();
    });
    // GTK must not see our own flags.
    let code = app.run_with_args::<&str>(&[]);
    if let Some(mut child) = server {
        let _ = child.kill();
        let _ = child.wait();
    }
    code
}

fn options(args: &[String]) -> Result<Options, String> {
    let flag = |name: &str| {
        args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
    };
    let port_text = flag("--port")
        .or_else(|| std::env::var("PORT").ok().filter(|p| !p.is_empty()))
        .or_else(proj_port)
        .ok_or("no port. Pass --port N or set PORT (or register the project with `proj add`).")?;
    let port = port_text.parse().map_err(|_| format!("port '{port_text}' is not a number"))?;
    let size = match flag("--size") {
        Some(s) => {
            let (w, h) = s.split_once('x').ok_or("--size wants WxH")?;
            (w.parse().map_err(|_| "--size wants WxH")?, h.parse().map_err(|_| "--size wants WxH")?)
        }
        None => (1440, 920),
    };
    let eval = match flag("--eval") {
        Some(s) if s.starts_with('@') => {
            let path = &s[1..];
            Some(std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?)
        }
        other => other,
    };
    Ok(Options {
        port,
        path: flag("--path").unwrap_or_else(|| "/".into()),
        snapshot: flag("--snapshot").map(PathBuf::from),
        eval,
        size,
    })
}

/// Persistent website data under `$XDG_DATA_HOME/boardroom-gtk` (default
/// `~/.local/share/boardroom-gtk`), matching WebKitGTK's default for this
/// binary so existing stored data stays in place while making the path
/// explicit regardless of `argv[0]` or how a window is opened.
fn data_dir() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(glib::user_data_dir)
        .join("boardroom-gtk")
}

fn cache_dir() -> PathBuf {
    std::env::var_os("XDG_CACHE_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(glib::user_cache_dir)
        .join("boardroom-gtk")
}

fn download_dir() -> PathBuf {
    std::env::var_os("BOARDROOM_DOWNLOAD_DIR")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| glib::user_special_dir(glib::UserDirectory::Downloads))
        .unwrap_or_else(|| glib::home_dir().join("Downloads"))
}

/// Picks a clean filename inside `dir`, numbering `name (1).ext` rather than
/// failing (`allow-overwrite` is false) or clobbering an earlier export.
fn unique_destination(dir: &Path, suggested: &str) -> PathBuf {
    let clean = Path::new(suggested)
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|n| !n.is_empty() && *n != "." && *n != "..")
        .unwrap_or("download");
    let first = dir.join(clean);
    if !first.exists() {
        return first;
    }
    let (stem, ext) = match clean.rfind('.') {
        Some(i) if i > 0 => (&clean[..i], &clean[i..]),
        _ => (clean, ""),
    };
    for n in 1u32.. {
        let candidate = dir.join(format!("{stem} ({n}){ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!()
}

/// One persistent `NetworkSession` shared by every Boardroom window and popup,
/// with downloads saved to the user's Downloads folder and announced in-window.
fn build_session(
    app: &gtk::Application,
    notices: &Notices,
    downloads: &DownloadLog,
    verbose: bool,
) -> webkit::NetworkSession {
    let (data, cache) = (data_dir(), cache_dir());
    let _ = std::fs::create_dir_all(&data);
    let _ = std::fs::create_dir_all(&cache);
    let session = webkit::NetworkSession::new(data.to_str(), cache.to_str());

    let app = app.clone();
    let notices = notices.clone();
    let downloads = downloads.clone();
    session.connect_download_started(move |_, download| {
        let notices_decide = notices.clone();
        let app_decide = app.clone();
        download.connect_decide_destination(move |dl, suggested| {
            let dir = download_dir();
            if let Err(e) = std::fs::create_dir_all(&dir) {
                eprintln!("boardroom-gtk: can't create {}: {e}", dir.display());
                return false;
            }
            let dest = unique_destination(&dir, suggested);
            let name = dest.file_name().and_then(|n| n.to_str()).unwrap_or(suggested);
            show_notice(&app_decide, &notices_decide, dl.web_view().as_ref(), &format!("Saving {name}…"), None);
            dl.set_destination(&dest.to_string_lossy());
            true
        });

        let failed = Rc::new(Cell::new(false));
        let failed_for_err = failed.clone();
        let notices_err = notices.clone();
        let app_err = app.clone();
        let downloads_err = downloads.clone();
        download.connect_failed(move |dl, err| {
            failed_for_err.set(true);
            let msg = format!("Download failed: {err}");
            eprintln!("boardroom-gtk: {msg}");
            downloads_err.borrow_mut().push(msg.clone());
            show_notice(&app_err, &notices_err, dl.web_view().as_ref(), &msg, None);
        });

        let notices_ok = notices.clone();
        let app_ok = app.clone();
        let downloads_ok = downloads.clone();
        download.connect_finished(move |dl| {
            if failed.get() {
                return;
            }
            let Some(dest) = dl.destination().map(|d| PathBuf::from(d.as_str())) else { return };
            let name = dest.file_name().and_then(|n| n.to_str()).unwrap_or("download");
            let folder = dest.parent().map(Path::to_path_buf);
            let where_shown = folder.as_deref().unwrap_or(dest.as_path()).display();
            let msg = format!("Saved {name} to {where_shown}");
            if verbose {
                println!("download: {}", dest.display());
            }
            downloads_ok.borrow_mut().push(dest.display().to_string());
            show_notice(&app_ok, &notices_ok, dl.web_view().as_ref(), &msg, folder);
        });
    });

    session
}

fn show_notice(
    app: &gtk::Application,
    notices: &Notices,
    view: Option<&webkit::WebView>,
    text: &str,
    folder: Option<PathBuf>,
) {
    let target = view
        .and_then(|v| v.root())
        .and_downcast::<gtk::ApplicationWindow>()
        .or_else(|| app.active_window().and_downcast::<gtk::ApplicationWindow>());
    let mut list = notices.borrow_mut();
    list.retain(|(w, _)| w.upgrade().is_some());
    let found = target
        .and_then(|win| list.iter().find(|(w, _)| w.upgrade().as_ref() == Some(&win)).map(|(_, b)| b.clone()))
        .or_else(|| list.last().map(|(_, b)| b.clone()));
    if let Some(bar) = found {
        bar.label.set_text(text);
        bar.open_folder.set_visible(folder.is_some());
        *bar.folder.borrow_mut() = folder;
        bar.revealer.set_reveal_child(true);
    }
}

/// The repository this was built from: where `proj port` finds Boardroom's
/// registered port, and where the server binary is.
fn repo_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::canonicalize(&dir).unwrap_or(dir)
}

fn proj_port() -> Option<String> {
    let out = Command::new("proj").arg("port").current_dir(repo_dir()).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn answering(port: u16) -> bool {
    TcpStream::connect_timeout(&([127, 0, 0, 1], port).into(), Duration::from_millis(300)).is_ok()
}

/// Uses the running server, or starts one for as long as this window lives.
fn ensure_server(port: u16) -> Result<Option<Child>, String> {
    if answering(port) {
        return Ok(None);
    }
    let bin = repo_dir().join("app/server/target/release/boardroom");
    if !bin.is_file() {
        return Err(format!("nothing answers on port {port}, and {} isn't built (app/build.sh)", bin.display()));
    }
    let child = Command::new(&bin)
        .args(["serve", "--port", &port.to_string()])
        .spawn()
        .map_err(|e| format!("{}: {e}", bin.display()))?;
    let start = Instant::now();
    while !answering(port) {
        if start.elapsed() > Duration::from_secs(10) {
            return Err(format!("started the server, but nothing answers on port {port}"));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(Some(child))
}

/// Settings and signals every Boardroom web view gets, the first and every
/// one the page opens.
fn setup(view: &webkit::WebView, origin: &str, notices: &Notices, verbose_console: bool) {
    if let Some(settings) = WebViewExt::settings(view) {
        settings.set_enable_developer_extras(true);
        settings.set_enable_webgl(true);
        settings.set_hardware_acceleration_policy(webkit::HardwareAccelerationPolicy::Always);
        settings.set_javascript_can_open_windows_automatically(true);
        if verbose_console {
            settings.set_enable_write_console_messages_to_stdout(true);
        }
    }
    if verbose_console && let Some(ucm) = view.user_content_manager() {
        let probe = webkit::UserScript::new(
            "window.__bootPerfStart = performance.now();\
             window.__consoleErrors = [];\
             (() => {\
               const orig = console.error.bind(console);\
               console.error = (...a) => { window.__consoleErrors.push(a.map(String).join(' ')); orig(...a); };\
               window.addEventListener('error', (e) => window.__consoleErrors.push('error: ' + e.message));\
               window.addEventListener('unhandledrejection', (e) => window.__consoleErrors.push('rejection: ' + e.reason));\
             })();",
            webkit::UserContentInjectedFrames::AllFrames,
            webkit::UserScriptInjectionTime::Start,
            &[],
            &[],
        );
        ucm.add_script(&probe);
    }

    // window.open: our own pages get a window of their own; anything else
    // goes to the default browser. `related-view` shares the parent's web
    // process and persistent `NetworkSession`.
    let origin_for_create = origin.to_string();
    let notices_for_create = notices.clone();
    view.connect_create(move |parent, action| {
        let uri = action.clone().request().and_then(|r| r.uri()).map(|u| u.to_string()).unwrap_or_default();
        if !uri.is_empty() && !uri.starts_with(&origin_for_create) {
            open_externally(&uri);
            return None;
        }
        let child = glib::Object::builder::<webkit::WebView>().property("related-view", parent).build();
        setup(&child, &origin_for_create, &notices_for_create, verbose_console);
        let app = parent.root().and_downcast::<gtk::ApplicationWindow>().and_then(|w| w.application());
        let notices_ready = notices_for_create.clone();
        child.connect_ready_to_show(move |child| {
            if let Some(app) = &app {
                window(app, child, (1400, 900), &notices_ready).present();
            }
        });
        Some(child.upcast())
    });

    // Links that navigate this view to another site open in the browser instead;
    // main-frame responses WebKit cannot display become downloads.
    let origin_for_policy = origin.to_string();
    view.connect_decide_policy(move |_, decision, kind| {
        if kind == webkit::PolicyDecisionType::Response {
            if let Some(resp) = decision.downcast_ref::<webkit::ResponsePolicyDecision>()
                && resp.is_main_frame_main_resource()
                && !resp.is_mime_type_supported()
            {
                decision.download();
                return true;
            }
            return false;
        }
        if kind != webkit::PolicyDecisionType::NavigationAction {
            return false;
        }
        let Some(nav) = decision.downcast_ref::<webkit::NavigationPolicyDecision>() else { return false };
        let uri = nav
            .navigation_action()
            .and_then(|a| a.request())
            .and_then(|r| r.uri())
            .map(|u| u.to_string())
            .unwrap_or_default();
        let ours = uri.starts_with(&origin_for_policy) || uri.starts_with("about:") || uri.starts_with("blob:") || uri.starts_with("data:");
        // Iframes (an embedded app) may load what they like; only top-level
        // navigation away from Boardroom leaves the app.
        let main_frame = nav.navigation_action().and_then(|a| a.frame_name()).is_none();
        if !ours && main_frame && (uri.starts_with("http:") || uri.starts_with("https:")) {
            open_externally(&uri);
            decision.ignore();
            return true;
        }
        false
    });
}

fn open_externally(uri: &str) {
    if let Err(e) = gio::AppInfo::launch_default_for_uri(uri, None::<&gio::AppLaunchContext>) {
        eprintln!("boardroom-gtk: can't open {uri}: {e}");
    }
}

/// A GTK window around a web view: a native header bar with the page's title,
/// a download notice bar, and keyboard focus kept on the page (with no shell
/// shortcuts taking Ctrl+Z, Ctrl+S, Delete or Escape).
fn window(
    app: &gtk::Application,
    view: &webkit::WebView,
    size: (i32, i32),
    notices: &Notices,
) -> gtk::ApplicationWindow {
    let header = gtk::HeaderBar::new();
    let reload = gtk::Button::from_icon_name("view-refresh-symbolic");
    reload.set_tooltip_text(Some("Reload"));
    reload.set_focus_on_click(false);
    let v = view.clone();
    reload.connect_clicked(move |_| v.reload());
    header.pack_start(&reload);
    let inspect = gtk::Button::from_icon_name("applications-engineering-symbolic");
    inspect.set_tooltip_text(Some("Web inspector"));
    inspect.set_focus_on_click(false);
    let v = view.clone();
    inspect.connect_clicked(move |_| {
        if let Some(i) = v.inspector() {
            i.show();
        }
    });
    header.pack_end(&inspect);

    let bar_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    bar_box.set_margin_start(12);
    bar_box.set_margin_end(8);
    bar_box.set_margin_top(6);
    bar_box.set_margin_bottom(6);
    let label = gtk::Label::new(None);
    label.set_hexpand(true);
    label.set_xalign(0.0);
    label.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    label.set_selectable(true);
    let folder: Rc<RefCell<Option<PathBuf>>> = Rc::new(RefCell::new(None));
    let open_folder = gtk::Button::with_label("Open folder");
    open_folder.set_focus_on_click(false);
    open_folder.set_visible(false);
    let folder_for_open = folder.clone();
    open_folder.connect_clicked(move |_| {
        if let Some(dir) = folder_for_open.borrow().as_ref() {
            let uri = gio::File::for_path(dir).uri();
            open_externally(&uri);
        }
    });
    let dismiss = gtk::Button::from_icon_name("window-close-symbolic");
    dismiss.set_tooltip_text(Some("Dismiss"));
    dismiss.set_focus_on_click(false);
    let bar_wrap = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let revealer = gtk::Revealer::builder()
        .transition_type(gtk::RevealerTransitionType::SlideDown)
        .reveal_child(false)
        .child(&bar_wrap)
        .build();
    let rev_for_dismiss = revealer.clone();
    let view_for_dismiss = view.clone();
    dismiss.connect_clicked(move |_| {
        rev_for_dismiss.set_reveal_child(false);
        view_for_dismiss.grab_focus();
    });
    bar_box.append(&label);
    bar_box.append(&open_folder);
    bar_box.append(&dismiss);
    bar_wrap.append(&bar_box);
    bar_wrap.append(&gtk::Separator::new(gtk::Orientation::Horizontal));

    view.set_vexpand(true);
    view.set_hexpand(true);
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.append(&revealer);
    content.append(view);

    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("Boardroom")
        .default_width(size.0)
        .default_height(size.1)
        .child(&content)
        .build();
    window.set_titlebar(Some(&header));
    GtkWindowExt::set_focus(&window, Some(view));

    notices.borrow_mut().push((
        window.downgrade(),
        NoticeBar { revealer, label, open_folder, folder },
    ));

    let w = window.clone();
    view.connect_title_notify(move |v| {
        let title = v.title().map(|t| t.to_string()).filter(|t| !t.is_empty());
        w.set_title(Some(title.as_deref().unwrap_or("Boardroom")));
    });
    let w = window.clone();
    view.connect_close(move |_| w.close());
    window
}

/// Lists any `gtk::ShortcutController` triggers attached to `widget`.
fn widget_shortcuts(widget: &impl IsA<gtk::Widget>) -> Vec<String> {
    let mut out = Vec::new();
    let controllers = widget.observe_controllers();
    for i in 0..controllers.n_items() {
        let Some(ctrl) = controllers.item(i).and_downcast::<gtk::ShortcutController>() else { continue };
        for j in 0..ctrl.n_items() {
            let Some(sc) = ctrl.item(j).and_downcast::<gtk::Shortcut>() else { continue };
            if let Some(t) = sc.trigger() {
                out.push(t.to_str().to_string());
            }
        }
    }
    out
}

/// --snapshot: once the page has loaded and settled (or finished `--eval`), save the view to a PNG and quit.
fn snapshot_when_loaded(
    view: &webkit::WebView,
    win: &gtk::ApplicationWindow,
    state: ShellState,
    path: PathBuf,
    eval: Option<String>,
    app: gtk::Application,
) {
    let done = Rc::new(RefCell::new(false));
    let win = win.clone();
    view.connect_load_changed(move |view, event| {
        if event != webkit::LoadEvent::Finished || *done.borrow() {
            return;
        }
        *done.borrow_mut() = true;
        let (view, win, state, path, app) = (view.clone(), win.clone(), state.clone(), path.clone(), app.clone());
        if let Some(script) = eval.clone() {
            let view_for_eval = view.clone();
            view.call_async_javascript_function(
                &script,
                None,
                None,
                None,
                None::<&gio::Cancellable>,
                move |res| {
                    match res {
                        Ok(v) => println!("eval: {}", v.to_str()),
                        Err(e) => println!("eval_error: {e}"),
                    }
                    finish_snapshot(&view_for_eval, &win, &state, &path, &app);
                },
            );
        } else {
            // Exercise window.open: it should become a second GTK window sharing the session.
            view.evaluate_javascript("window.open(location.href, 'probe', 'popup')", None, None, None::<&gio::Cancellable>, |_| {});
            // Give the page a moment to fetch its data and draw.
            glib::timeout_add_local_once(Duration::from_secs(6), move || {
                finish_snapshot(&view, &win, &state, &path, &app);
            });
        }
    });
}

fn finish_snapshot(
    view: &webkit::WebView,
    win: &gtk::ApplicationWindow,
    state: &ShellState,
    path: &Path,
    app: &gtk::Application,
) {
    let focused = GtkWindowExt::focus(win).as_ref() == Some(view.upcast_ref::<gtk::Widget>());
    let mut shortcuts = widget_shortcuts(win);
    if let Some(tb) = win.titlebar() {
        shortcuts.extend(widget_shortcuts(&tb));
    }
    let all_share_session = app.windows().into_iter().all(|w| {
        w.downcast::<gtk::ApplicationWindow>()
            .ok()
            .and_then(|aw| aw.child())
            .and_downcast::<gtk::Box>()
            .and_then(|b| b.last_child())
            .and_downcast::<webkit::WebView>()
            .and_then(|v| v.network_session())
            .as_ref()
            == Some(&state.session)
    });
    let data_dir_str = state
        .session
        .website_data_manager()
        .and_then(|m| m.base_data_directory())
        .map(|s| s.to_string())
        .unwrap_or_default();
    let (notice_shown, notice_text) = state
        .notices
        .borrow()
        .iter()
        .find(|(w, _)| w.upgrade().as_ref() == Some(win))
        .map(|(_, b)| (b.revealer.reveals_child(), b.label.text().to_string()))
        .unwrap_or((false, String::new()));
    let dl = state.downloads.borrow().clone();
    println!(
        "shell: windows={} session_shared={} webview_focused={} data_dir={:?} window_shortcuts={:?} downloads={:?} notice_shown={} notice_text={:?}",
        app.windows().len(),
        all_share_session,
        focused,
        data_dir_str,
        shortcuts,
        dl,
        notice_shown,
        notice_text,
    );

    let path = path.to_path_buf();
    let app = app.clone();
    let view_for_shot = view.clone();
    view.evaluate_javascript(
        "(() => {\
            const c = document.createElement('canvas');\
            const gl = c.getContext('webgl2');\
            const dbg = gl && gl.getExtension('WEBGL_debug_renderer_info');\
            return JSON.stringify({\
                isolated: self.crossOriginIsolated,\
                sab: typeof SharedArrayBuffer,\
                webgl: !!gl,\
                vendor: gl ? (dbg ? gl.getParameter(dbg.UNMASKED_VENDOR_WEBGL) : gl.getParameter(gl.VENDOR)) : null,\
                renderer: gl ? (dbg ? gl.getParameter(dbg.UNMASKED_RENDERER_WEBGL) : gl.getParameter(gl.RENDERER)) : null,\
                glRenderer: gl ? gl.getParameter(gl.RENDERER) : null,\
                title: document.title,\
                consoleErrors: window.__consoleErrors || []\
            });\
        })()",
        None,
        None,
        None::<&gio::Cancellable>,
        move |r| {
            match r {
                Ok(v) => println!("page: {}", v.to_str()),
                Err(e) => println!("page: {e}"),
            }
            view_for_shot.snapshot(
                webkit::SnapshotRegion::Visible,
                webkit::SnapshotOptions::NONE,
                None::<&gio::Cancellable>,
                move |result| {
                    println!("windows: {}", app.windows().len());
                    match result {
                        Ok(texture) => match texture.save_to_png(&path) {
                            Ok(()) => println!("saved {}", path.display()),
                            Err(e) => eprintln!("boardroom-gtk: {}: {e}", path.display()),
                        },
                        Err(e) => eprintln!("boardroom-gtk: snapshot: {e}"),
                    }
                    app.quit();
                },
            );
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_destination_sanitizes_and_numbers_collisions() {
        let dir = std::env::temp_dir().join(format!("boardroom-gtk-dl-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let p0 = unique_destination(&dir, "../../part.stl");
        assert_eq!(p0, dir.join("part.stl"));
        std::fs::write(&p0, b"first").unwrap();

        let p1 = unique_destination(&dir, "part.stl");
        assert_eq!(p1, dir.join("part (1).stl"));
        std::fs::write(&p1, b"second").unwrap();

        let p2 = unique_destination(&dir, "part.stl");
        assert_eq!(p2, dir.join("part (2).stl"));

        assert_eq!(unique_destination(&dir, ""), dir.join("download"));
        assert_eq!(unique_destination(&dir, ".."), dir.join("download"));

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
