//! Boardroom's desktop window: the same web app, in GTK 4 windows with
//! WebKitGTK, so it looks and behaves like the rest of the desktop.
//!
//!     boardroom-gtk [--port N] [--path /?project=…]
//!     boardroom-gtk --snapshot out.png [--path …] [--size 1440x900]
//!     boardroom-gtk --list-features
//!
//! --snapshot is a self-test: it loads the page, opens one popup the way the
//! page's window.open does, saves the view to a PNG, prints what the page
//! reports (cross-origin isolation, SharedArrayBuffer, WebGL2) and quits.
//! --list-features prints WebKit's runtime features and their state.
//!
//! The server is the app; this is only its window. It uses the server on the
//! registered port (--port, then $PORT, then `proj port`), and starts one if
//! none answers, stopping it again on exit. Every `window.open` from the page
//! (the viewer, Circuit Composer) becomes a GTK window of its own; links to
//! other sites open in the default browser.

use gtk::prelude::*;
use gtk::{gio, glib};
use std::cell::RefCell;
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::rc::Rc;
use std::time::{Duration, Instant};
use webkit::prelude::*;

const APP_ID: &str = "io.github.sequoia_hope.Boardroom";

struct Options {
    port: u16,
    path: String,
    snapshot: Option<PathBuf>,
    size: (i32, i32),
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
    let size = options.size;
    app.connect_activate(move |app| {
        let view = webkit::WebView::new();
        setup(&view, &origin);
        let window = window(app, &view, size);
        view.load_uri(&url);
        if let Some(path) = snapshot.clone() {
            snapshot_when_loaded(&view, path, app.clone());
        }
        window.present();
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
    Ok(Options {
        port,
        path: flag("--path").unwrap_or_else(|| "/".into()),
        snapshot: flag("--snapshot").map(PathBuf::from),
        size,
    })
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
fn setup(view: &webkit::WebView, origin: &str) {
    if let Some(settings) = WebViewExt::settings(view) {
        settings.set_enable_developer_extras(true);
        settings.set_enable_webgl(true);
        settings.set_hardware_acceleration_policy(webkit::HardwareAccelerationPolicy::Always);
        settings.set_javascript_can_open_windows_automatically(true);
    }

    // window.open: our own pages get a window of their own; anything else
    // goes to the default browser.
    let origin_for_create = origin.to_string();
    view.connect_create(move |parent, action| {
        let uri = action.clone().request().and_then(|r| r.uri()).map(|u| u.to_string()).unwrap_or_default();
        if !uri.is_empty() && !uri.starts_with(&origin_for_create) {
            open_externally(&uri);
            return None;
        }
        let child = glib::Object::builder::<webkit::WebView>().property("related-view", parent).build();
        setup(&child, &origin_for_create);
        let app = parent.root().and_downcast::<gtk::ApplicationWindow>().and_then(|w| w.application());
        child.connect_ready_to_show(move |child| {
            if let Some(app) = &app {
                window(app, child, (1400, 900)).present();
            }
        });
        Some(child.upcast())
    });

    // Links that navigate this view to another site open in the browser instead.
    let origin_for_policy = origin.to_string();
    view.connect_decide_policy(move |_, decision, kind| {
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

/// A GTK window around a web view: a native header bar with the page's title.
fn window(app: &gtk::Application, view: &webkit::WebView, size: (i32, i32)) -> gtk::ApplicationWindow {
    let header = gtk::HeaderBar::new();
    let reload = gtk::Button::from_icon_name("view-refresh-symbolic");
    reload.set_tooltip_text(Some("Reload"));
    let v = view.clone();
    reload.connect_clicked(move |_| v.reload());
    header.pack_start(&reload);
    let inspect = gtk::Button::from_icon_name("applications-engineering-symbolic");
    inspect.set_tooltip_text(Some("Web inspector"));
    let v = view.clone();
    inspect.connect_clicked(move |_| {
        if let Some(i) = v.inspector() {
            i.show();
        }
    });
    header.pack_end(&inspect);

    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("Boardroom")
        .default_width(size.0)
        .default_height(size.1)
        .child(view)
        .build();
    window.set_titlebar(Some(&header));
    view.set_vexpand(true);
    view.set_hexpand(true);

    let w = window.clone();
    view.connect_title_notify(move |v| {
        let title = v.title().map(|t| t.to_string()).filter(|t| !t.is_empty());
        w.set_title(Some(title.as_deref().unwrap_or("Boardroom")));
    });
    let w = window.clone();
    view.connect_close(move |_| w.close());
    window
}

/// --snapshot: once the page has loaded and settled, save the view to a PNG and quit.
fn snapshot_when_loaded(view: &webkit::WebView, path: PathBuf, app: gtk::Application) {
    let done = Rc::new(RefCell::new(false));
    view.connect_load_changed(move |view, event| {
        if event != webkit::LoadEvent::Finished || *done.borrow() {
            return;
        }
        *done.borrow_mut() = true;
        let (view, path, app) = (view.clone(), path.clone(), app.clone());
        // Exercise window.open: it should become a second GTK window.
        view.evaluate_javascript("window.open(location.href, 'probe', 'popup')", None, None, None::<&gio::Cancellable>, |_| {});
        // Give the page a moment to fetch its data and draw.
        glib::timeout_add_local_once(Duration::from_secs(6), move || {
            let report = view.clone();
            report.evaluate_javascript(
                "JSON.stringify({isolated: self.crossOriginIsolated, sab: typeof SharedArrayBuffer, webgl: !!document.createElement('canvas').getContext('webgl2'), title: document.title})",
                None,
                None,
                None::<&gio::Cancellable>,
                |r| match r {
                    Ok(v) => println!("page: {}", v.to_str()),
                    Err(e) => println!("page: {e}"),
                },
            );
            view.snapshot(
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
        });
    });
}
