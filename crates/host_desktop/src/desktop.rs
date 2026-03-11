use std::io::{Error, ErrorKind};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream};
use std::path::Path;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use tauri::window::Color;
use tauri::{Runtime, Url, WebviewUrl};

use golden_engine::app::{ProjectLifecycle, create_new_project_engine};
use golden_engine::engine::Engine;
use golden_transport_server::{UiServerConfig, run_with_ui_server_config};

const UI_STARTUP_TIMEOUT: Duration = Duration::from_secs(5);
const UI_PROBE_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Debug, Clone, Copy, Default)]
/// Launch flags understood by the default desktop and headless runtime.
pub struct LaunchArgs {
    /// Runs the built-in UI server without launching the Tauri window.
    pub headless: bool,
    /// Forces the built-in UI server to bind to loopback only.
    pub no_remote: bool,
    /// Prints default launch usage instead of starting the app.
    pub show_help: bool,
}

#[derive(Debug, Clone)]
struct UiEndpoint {
    connect_addr: String,
}

/// Parses the current process arguments and launches the app through the default host runtime.
pub fn run_default<T, R>(tauri_context: tauri::Context<R>) -> std::io::Result<()>
where
    T: ProjectLifecycle + 'static,
    R: Runtime,
{
    let args = parse_launch_args_from_env()?;
    if args.show_help {
        print_usage();
        return Ok(());
    }

    launch_with_args::<T, R>(args, tauri_context)
}

/// Parses launch flags from the current process environment.
pub fn parse_launch_args_from_env() -> std::io::Result<LaunchArgs> {
    parse_launch_args(std::env::args().skip(1))
}

/// Parses the default launch flags from an argument iterator.
pub fn parse_launch_args<I, S>(args: I) -> std::io::Result<LaunchArgs>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut parsed = LaunchArgs::default();

    for arg in args {
        match arg.as_ref() {
            "--headless" => parsed.headless = true,
            "--no-remote" => parsed.no_remote = true,
            "--help" | "-h" => parsed.show_help = true,
            other => {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    format!("unknown argument '{other}'. supported flags: --headless, --no-remote, --help"),
                ));
            }
        }
    }

    Ok(parsed)
}

/// Creates the app's default new-project engine and launches it through the default host runtime.
pub fn launch_with_args<T, R>(args: LaunchArgs, tauri_context: tauri::Context<R>) -> std::io::Result<()>
where
    T: ProjectLifecycle + 'static,
    R: Runtime,
{
    let engine = create_new_project_engine::<T>().map_err(Error::other)?;
    launch_engine_with_args(engine, args, tauri_context)
}

/// Launches a caller-provided engine through the default desktop or headless host runtime.
pub fn launch_engine_with_args<T, R>(
    engine: Engine<T>,
    args: LaunchArgs,
    tauri_context: tauri::Context<R>,
) -> std::io::Result<()>
where
    T: ProjectLifecycle + 'static,
    R: Runtime,
{
    let mut config = UiServerConfig::default();
    if let Ok(bind_addr) = std::env::var("GC_UI_BIND") {
        if !bind_addr.trim().is_empty() {
            config.bind_addr = bind_addr;
        }
    }

    let frontend_url = std::env::var("GC_UI_FRONTEND_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(detect_or_default_frontend_url);

    if args.no_remote {
        config.bind_addr = force_loopback_bind_addr(&config.bind_addr);
    }

    let endpoint = resolve_ui_endpoint(&config.bind_addr);
    if args.headless {
        return run_with_ui_server_config(engine, config);
    }

    let (startup_tx, startup_rx) = mpsc::channel::<std::io::Result<()>>();
    thread::spawn(move || {
        let result = run_with_ui_server_config(engine, config);
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
            eprintln!(
                "warning: frontend UI at {frontend_url} was not reachable yet ({err}); continuing and launching Tauri anyway"
            );
        }
    }

    run_tauri(&frontend_url, tauri_context)
}

fn print_usage() {
    let executable = std::env::args().next().unwrap_or_else(|| "app".to_string());
    let program_name = Path::new(&executable)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("app");

    println!("Usage: {program_name} [--headless] [--no-remote]");
    println!("  --headless   Run without launching the Tauri desktop window.");
    println!("  --no-remote  Bind UI API to loopback only (blocks non-local browser access).");
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

    UiEndpoint {
        connect_addr: bind_addr.to_string(),
    }
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
    Err(Error::new(
        ErrorKind::TimedOut,
        format!(
            "ui server did not become reachable at {connect_addr} within {}ms{details}",
            timeout.as_millis()
        ),
    ))
}

fn url_connect_addr(url: &str) -> Option<String> {
    let parsed: Url = url.parse().ok()?;
    let host = parsed.host_str()?;
    let port = parsed.port_or_known_default()?;

    let host = if host.contains(':') {
        format!("[{host}]")
    } else {
        host.to_string()
    };

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

fn run_tauri<R: Runtime>(ui_base_url: &str, tauri_context: tauri::Context<R>) -> std::io::Result<()> {
    let external_url: Url = ui_base_url.parse().map_err(|err| {
        Error::new(
            ErrorKind::InvalidInput,
            format!("invalid UI URL '{ui_base_url}': {err}"),
        )
    })?;

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

    let init_script = format!(
        "window.__PLATFORM__ = '{}'; document.documentElement.dataset.platform = '{}';",
        os, os
    );

    tauri::Builder::<R>::new()
        .setup(move |app| {
            let mut window_builder =
                tauri::WebviewWindowBuilder::new(app, "main", WebviewUrl::External(external_url.clone()))
                    .title("Chataigne 2")
                    .decorations(false)
                    .shadow(true)
                    .accept_first_mouse(true)
                    .inner_size(75.0 * 16.0, 50.0 * 16.0);

            if cfg!(target_os = "windows") {
                window_builder = window_builder
                    .disable_drag_drop_handler()
                    .background_color(Color(20, 20, 20, 255));
            } else {
                window_builder = window_builder.transparent(true);
            }

            window_builder
                .build()
                .map_err(|err| Error::other(format!("failed creating Tauri window: {err}")))?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            crate::desktop_commands::window_minimize,
            crate::desktop_commands::window_toggle_maximize,
            crate::desktop_commands::window_close,
            crate::desktop_commands::window_is_maximized,
            crate::desktop_commands::start_drag,
            crate::desktop_commands::open_file_dialog,
            crate::desktop_commands::save_file_dialog
        ])
        .append_invoke_initialization_script(&init_script)
        .run(tauri_context)
        .map_err(|err| Error::other(format!("tauri runtime failed: {err}")))
}
