use std::io::{Error, ErrorKind};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream};
use std::path::Path;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use tauri::window::Color;
use tauri::{Url, WebviewUrl};

use super::{UiServerConfig, run_app_with_config};
use crate::engine::Engine;
use crate::node::Node;

const UI_STARTUP_TIMEOUT: Duration = Duration::from_secs(5);
const UI_PROBE_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Debug, Clone, Copy, Default)]
struct LaunchArgs {
    headless: bool,
    no_remote: bool,
    show_help: bool,
}

#[derive(Debug, Clone)]
struct UiEndpoint {
    connect_addr: String,
}

/// Boots an engine and runs the default app host:
/// - built-in UI/API server
/// - optional Tauri desktop window (unless `--headless`)
pub fn run_app<T: Node + 'static>(engine: Engine<T>) -> std::io::Result<()> {
    let args = parse_launch_args()?;
    if args.show_help {
        print_usage();
        return Ok(());
    }

    run_with_frontends(engine, args)
}

fn parse_launch_args() -> std::io::Result<LaunchArgs> {
    let mut args = LaunchArgs::default();

    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--headless" => args.headless = true,
            "--no-remote" => args.no_remote = true,
            "--help" | "-h" => args.show_help = true,
            other => {
                return Err(Error::new(ErrorKind::InvalidInput, format!("unknown argument '{other}'. supported flags: --headless, --no-remote, --help")));
            }
        }
    }

    Ok(args)
}

fn print_usage() {
    let executable = std::env::args().next().unwrap_or_else(|| "app".to_string());
    let program_name = Path::new(&executable).file_name().and_then(|name| name.to_str()).unwrap_or("app");

    println!("Usage: {program_name} [--headless] [--no-remote]");
    println!("  --headless   Run without launching the Tauri desktop window.");
    println!("  --no-remote  Bind UI API to loopback only (blocks non-local browser access).");
}

fn run_with_frontends<T: Node + 'static>(engine: Engine<T>, args: LaunchArgs) -> std::io::Result<()> {
    let mut config = UiServerConfig::default();
    if let Ok(bind_addr) = std::env::var("GC_UI_BIND") {
        if !bind_addr.trim().is_empty() {
            config.bind_addr = bind_addr;
        }
    }

    let frontend_url = std::env::var("GC_UI_FRONTEND_URL").ok().filter(|value| !value.trim().is_empty()).unwrap_or_else(detect_or_default_frontend_url);

    if args.no_remote {
        config.bind_addr = force_loopback_bind_addr(&config.bind_addr);
    }

    let endpoint = resolve_ui_endpoint(&config.bind_addr);
    if args.headless {
        return run_app_with_config(engine, config);
    }

    let (startup_tx, startup_rx) = mpsc::channel::<std::io::Result<()>>();
    thread::spawn(move || {
        let result = run_app_with_config(engine, config);
        let _ = startup_tx.send(result);
    });

    match startup_rx.recv_timeout(Duration::from_millis(250)) {
        Ok(result) => return result,
        Err(RecvTimeoutError::Disconnected) => {
            return Err(Error::other("ui server thread exited before startup completed"));
        }
        Err(RecvTimeoutError::Timeout) => {}
    }

    wait_for_ui_server(&endpoint.connect_addr, UI_STARTUP_TIMEOUT)?;

    if let Some(connect_addr) = url_connect_addr(&frontend_url) {
        if let Err(err) = wait_for_ui_server(&connect_addr, UI_STARTUP_TIMEOUT) {
            eprintln!("warning: frontend UI at {frontend_url} was not reachable yet ({err}); continuing and launching Tauri anyway");
        }
    }

    run_tauri(&frontend_url)
}

fn force_loopback_bind_addr(bind_addr: &str) -> String {
    if let Ok(socket_addr) = bind_addr.parse::<SocketAddr>() {
        let loopback_ip = match socket_addr.ip() {
            IpAddr::V4(_) => IpAddr::V4(Ipv4Addr::LOCALHOST),
            IpAddr::V6(_) => IpAddr::V6(Ipv6Addr::LOCALHOST),
        };
        return SocketAddr::new(loopback_ip, socket_addr.port()).to_string();
    }

    bind_addr.to_string()
}

fn resolve_ui_endpoint(bind_addr: &str) -> UiEndpoint {
    if let Ok(socket_addr) = bind_addr.parse::<SocketAddr>() {
        let connect_ip = match socket_addr.ip() {
            IpAddr::V4(ip) if ip.is_unspecified() => IpAddr::V4(Ipv4Addr::LOCALHOST),
            IpAddr::V6(ip) if ip.is_unspecified() => IpAddr::V6(Ipv6Addr::LOCALHOST),
            ip => ip,
        };

        let connect_addr = SocketAddr::new(connect_ip, socket_addr.port()).to_string();
        return UiEndpoint { connect_addr };
    }

    UiEndpoint { connect_addr: bind_addr.to_string() }
}

fn wait_for_ui_server(connect_addr: &str, timeout: Duration) -> std::io::Result<()> {
    let started_at = Instant::now();
    let mut last_error = None::<std::io::Error>;

    while started_at.elapsed() < timeout {
        match TcpStream::connect(connect_addr) {
            Ok(stream) => {
                drop(stream);
                return Ok(());
            }
            Err(err) => last_error = Some(err),
        }
        thread::sleep(UI_PROBE_INTERVAL);
    }

    let details = last_error.map(|err| format!(": {err}")).unwrap_or_default();
    Err(Error::new(ErrorKind::TimedOut, format!("ui server did not become reachable at {connect_addr} within {}ms{details}", timeout.as_millis())))
}

fn url_connect_addr(url: &str) -> Option<String> {
    let parsed: Url = url.parse().ok()?;
    let host = parsed.host_str()?;

    let port = parsed.port_or_known_default()?;

    let host = if host.contains(':') { format!("[{host}]") } else { host.to_string() };

    Some(format!("{host}:{port}"))
}

fn detect_or_default_frontend_url() -> String {
    let candidates = [5173u16, 5174, 5175, 5176, 4173];
    for port in candidates {
        let connect_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port).to_string();
        if wait_for_ui_server(&connect_addr, Duration::from_millis(150)).is_ok() {
            return format!("http://localhost:{port}");
        }
    }

    "http://localhost:5173".to_string()
}

#[tauri::command]
fn window_minimize(window: tauri::Window) -> Result<(), String> {
    window.minimize().map_err(|err| err.to_string())
}

#[tauri::command]
fn window_toggle_maximize(window: tauri::Window) -> Result<(), String> {
    let is_maximized = window.is_maximized().map_err(|err| err.to_string())?;
    if is_maximized { window.unmaximize().map_err(|err| err.to_string()) } else { window.maximize().map_err(|err| err.to_string()) }
}

#[tauri::command]
fn window_close(window: tauri::Window) -> Result<(), String> {
    window.close().map_err(|err| err.to_string())
}

#[tauri::command]
fn window_is_maximized(window: tauri::Window) -> Result<bool, String> {
    window.is_maximized().map_err(|err| err.to_string())
}

#[tauri::command]
fn start_drag(window: tauri::Window) -> Result<(), String> {
    println!("Start drag");
    window.start_dragging().map_err(|err| err.to_string())
}

#[tauri::command]
fn open_file_dialog(allowed_extensions: Option<Vec<String>>) -> Result<Option<String>, String> {
    let mut dialog = rfd::FileDialog::new();

    let normalized_extensions = allowed_extensions
        .unwrap_or_default()
        .into_iter()
        .map(|value| value.trim().trim_start_matches('.').to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .collect::<std::collections::BTreeSet<String>>()
        .into_iter()
        .collect::<Vec<String>>();

    if !normalized_extensions.is_empty() {
        let extension_refs = normalized_extensions.iter().map(String::as_str).collect::<Vec<&str>>();
        dialog = dialog.add_filter("Allowed files", &extension_refs);
    }

    Ok(dialog.pick_file().map(|path| path.to_string_lossy().to_string()))
}

fn run_tauri(ui_base_url: &str) -> std::io::Result<()> {
    let external_url: Url = ui_base_url.parse().map_err(|err| Error::new(ErrorKind::InvalidInput, format!("invalid UI URL '{ui_base_url}': {err}")))?;

    // WebView2 currently has drag/drop issues with transparent frameless windows.
    // Keep transparency on non-Windows platforms, but disable it on Windows so
    // in-app DnD (Dockview tabs/panels) remains reliable.

    #[cfg(target_os = "linux")]
    {
        unsafe {
            std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
        }
    }

    let os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "macos"
    };

    // This is the "Tauri 2.0/1.x" way to inject script BEFORE the page loads
    let init_script = format!("window.__PLATFORM__ = '{}'; document.documentElement.dataset.platform = '{}';", os, os);

    tauri::Builder::default()
        .setup(move |app| {
            let mut window_builder = tauri::WebviewWindowBuilder::new(app, "main", WebviewUrl::External(external_url.clone()))
                .title("Chataigne 2")
                .decorations(false)
                .shadow(true)
                .accept_first_mouse(true)
                .inner_size(75.0 * 16.0, 50.0 * 16.0);

            if cfg!(target_os = "windows") {
                window_builder = window_builder.disable_drag_drop_handler().background_color(Color(20, 20, 20, 255));
            } else {
                window_builder = window_builder.transparent(true);
            }

            window_builder.build().map_err(|err| Error::other(format!("failed creating Tauri window: {err}")))?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![window_minimize, window_toggle_maximize, window_close, window_is_maximized, start_drag, open_file_dialog])
        .append_invoke_initialization_script(&init_script)
        .run(tauri::generate_context!("../../../../tauri.conf.json"))
        .map_err(|err| Error::other(format!("tauri runtime failed: {err}")))
}
