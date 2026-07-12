use std::io::{Error, ErrorKind};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use tauri::window::Color;
use tauri::{Manager, Runtime, Url, WebviewUrl};

use crate::window_state::{WindowStatePersistence, restore_window_state, save_window_state};
#[cfg(target_os = "windows")]
use crate::windows_process_job::WindowsProcessJob;
use golden_engine::app::{ProjectLifecycle, create_new_project_engine};
use golden_engine::engine::Engine;
use golden_transport_server::{UiAsset, UiPreferencesConfig, UiServerConfig, run_with_ui_server_config};

const UI_STARTUP_TIMEOUT: Duration = Duration::from_secs(5);
const UI_PROBE_INTERVAL: Duration = Duration::from_millis(50);
const WINDOW_CLOSE_REQUESTED_SCRIPT: &str = "window.dispatchEvent(new CustomEvent('gc-window-close-requested'));";

#[derive(Debug, Clone, Copy, Default)]
/// Launch flags understood by the default desktop and headless runtime.
pub struct LaunchArgs {
    /// Runs the built-in UI server without launching the Tauri window.
    pub headless: bool,
    /// Launches against a live frontend dev server instead of bundled UI assets.
    pub dev: bool,
    /// Launches Tauri against an external frontend instead of serving bundled UI assets.
    pub no_frontend: bool,
    /// Forces the process to attach or create a console for stdout/stderr.
    pub show_output: bool,
    /// Forces the built-in UI server to bind to loopback only.
    pub no_remote: bool,
    /// Prints default launch usage instead of starting the app.
    pub show_help: bool,
}

#[derive(Debug, Clone)]
struct UiEndpoint {
    connect_addr: String,
}

#[derive(Debug, Clone, Copy)]
/// App-provided configuration for launching a frontend dev server.
pub struct FrontendDevServerConfig {
    /// Absolute working directory containing the frontend package.json.
    pub working_dir: &'static str,
    /// NPM script to execute for the frontend dev server.
    pub npm_script: &'static str,
    /// URL where the dev server should become reachable.
    pub url: &'static str,
}

pub(super) struct DevFrontendProcess {
    child: Child,
    #[cfg(target_os = "windows")]
    job: Option<WindowsProcessJob>,
}

impl DevFrontendProcess {
    pub(super) fn spawn(command: &mut Command) -> std::io::Result<Self> {
        #[cfg(target_os = "windows")]
        {
            let (child, job) = WindowsProcessJob::spawn(command)?;
            Ok(Self { child, job: Some(job) })
        }

        #[cfg(not(target_os = "windows"))]
        {
            command.spawn().map(|child| Self { child })
        }
    }

    fn terminate(&mut self) {
        #[cfg(target_os = "windows")]
        {
            // Closing a kill-on-close job terminates every descendant even when npm or a shell has
            // reparented it. Waiting afterwards reaps the direct child handle.
            drop(self.job.take());
            let _ = self.child.wait();
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

/// Parses the current process arguments and launches the app through the default host runtime.
pub fn run_default<T, R>(tauri_context: tauri::Context<R>) -> std::io::Result<()>
where
    T: ProjectLifecycle + 'static,
    R: Runtime,
{
    run_default_with_ui_assets::<T, R>(tauri_context, &[])
}

/// Parses the current process arguments and launches the app through the default host runtime with bundled UI assets.
pub fn run_default_with_ui_assets<T, R>(
    tauri_context: tauri::Context<R>,
    ui_assets: &'static [UiAsset],
) -> std::io::Result<()>
where
    T: ProjectLifecycle + 'static,
    R: Runtime,
{
    run_default_with_ui_assets_and_dev_server::<T, R>(tauri_context, ui_assets, None)
}

/// Parses the current process arguments and launches the app through the default host runtime with bundled UI assets
/// and an optional frontend dev server.
pub fn run_default_with_ui_assets_and_dev_server<T, R>(
    tauri_context: tauri::Context<R>,
    ui_assets: &'static [UiAsset],
    dev_server: Option<FrontendDevServerConfig>,
) -> std::io::Result<()>
where
    T: ProjectLifecycle + 'static,
    R: Runtime,
{
    maybe_enable_output_console_from_env()?;

    let args = parse_launch_args_from_env()?;
    if args.show_help {
        print_usage();
        return Ok(());
    }

    launch_with_ui_assets_and_dev_server::<T, R>(args, tauri_context, ui_assets, dev_server)
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
            "--dev" => parsed.dev = true,
            "--no-frontend" => parsed.no_frontend = true,
            "--show-output" => parsed.show_output = true,
            "--no-remote" => parsed.no_remote = true,
            "--help" | "-h" => parsed.show_help = true,
            other => {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    format!(
                        "unknown argument '{other}'. supported flags: --headless, --dev, --no-frontend, --show-output, --no-remote, --help"
                    ),
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
    launch_with_ui_assets::<T, R>(args, tauri_context, &[])
}

/// Creates the app's default new-project engine and launches it through the default host runtime with bundled UI.
pub fn launch_with_ui_assets<T, R>(
    args: LaunchArgs,
    tauri_context: tauri::Context<R>,
    ui_assets: &'static [UiAsset],
) -> std::io::Result<()>
where
    T: ProjectLifecycle + 'static,
    R: Runtime,
{
    launch_with_ui_assets_and_dev_server::<T, R>(args, tauri_context, ui_assets, None)
}

/// Creates the app's default new-project engine and launches it through the default host runtime with bundled UI and
/// an optional frontend dev server.
pub fn launch_with_ui_assets_and_dev_server<T, R>(
    args: LaunchArgs,
    tauri_context: tauri::Context<R>,
    ui_assets: &'static [UiAsset],
    dev_server: Option<FrontendDevServerConfig>,
) -> std::io::Result<()>
where
    T: ProjectLifecycle + 'static,
    R: Runtime,
{
    let engine = create_new_project_engine::<T>().map_err(Error::other)?;
    launch_engine_with_ui_assets_and_dev_server(engine, args, tauri_context, ui_assets, dev_server)
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
    launch_engine_with_ui_assets(engine, args, tauri_context, &[])
}

/// Launches a caller-provided engine through the default desktop or headless host runtime with bundled UI assets.
pub fn launch_engine_with_ui_assets<T, R>(
    engine: Engine<T>,
    args: LaunchArgs,
    tauri_context: tauri::Context<R>,
    ui_assets: &'static [UiAsset],
) -> std::io::Result<()>
where
    T: ProjectLifecycle + 'static,
    R: Runtime,
{
    launch_engine_with_ui_assets_and_dev_server(engine, args, tauri_context, ui_assets, None)
}

/// Launches a caller-provided engine through the default desktop or headless host runtime with bundled UI assets and
/// an optional frontend dev server.
pub fn launch_engine_with_ui_assets_and_dev_server<T, R>(
    engine: Engine<T>,
    args: LaunchArgs,
    tauri_context: tauri::Context<R>,
    ui_assets: &'static [UiAsset],
    dev_server: Option<FrontendDevServerConfig>,
) -> std::io::Result<()>
where
    T: ProjectLifecycle + 'static,
    R: Runtime,
{
    let mut config = UiServerConfig::default();
    let app_data_dir = default_app_data_dir::<T>()?;
    config.preferences = Some(UiPreferencesConfig {
        file_path: app_data_dir.join(T::preferences_file_name()),
        default_data_folder: app_data_dir.to_string_lossy().to_string(),
    });
    let frontend_assets = if args.no_frontend || args.dev { &[] } else { ui_assets };
    if let Ok(bind_addr) = std::env::var("GC_UI_BIND") {
        if !bind_addr.trim().is_empty() {
            config.bind_addr = bind_addr;
        }
    }

    if args.no_remote {
        config.bind_addr = force_loopback_bind_addr(&config.bind_addr);
    }

    let endpoint = resolve_ui_endpoint(&config.bind_addr);
    config.frontend_assets = frontend_assets;

    let configured_frontend_url = std::env::var("GC_UI_FRONTEND_URL")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let frontend_url = configured_frontend_url
        .clone()
        .unwrap_or_else(|| default_frontend_url(&endpoint, frontend_assets, args.dev, dev_server));

    if args.headless {
        let dev_server_process = if args.dev && configured_frontend_url.is_none() {
            spawn_frontend_dev_server(dev_server, &frontend_url, args.show_output)?
        } else {
            None
        };

        if let Some(connect_addr) = url_connect_addr(&frontend_url) {
            if let Err(err) = wait_for_ui_server(&connect_addr, UI_STARTUP_TIMEOUT) {
                eprintln!(
                    "warning: frontend UI at {frontend_url} was not reachable yet ({err}); continuing with headless runtime"
                );
            }
        }

        let run_result = run_with_ui_server_config(engine, config);
        drop(dev_server_process);
        return run_result;
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

    let dev_server_process = if args.dev && configured_frontend_url.is_none() {
        spawn_frontend_dev_server(dev_server, &frontend_url, args.show_output)?
    } else {
        None
    };

    if let Some(connect_addr) = url_connect_addr(&frontend_url) {
        if let Err(err) = wait_for_ui_server(&connect_addr, UI_STARTUP_TIMEOUT) {
            eprintln!(
                "warning: frontend UI at {frontend_url} was not reachable yet ({err}); continuing and launching Tauri anyway"
            );
        }
    }

    let run_result = run_tauri(&frontend_url, tauri_context, app_data_dir.join("window-state.json"));
    drop(dev_server_process);
    run_result
}

fn default_app_data_dir<T: ProjectLifecycle>() -> std::io::Result<PathBuf> {
    let mut dir =
        dirs::data_dir().ok_or_else(|| Error::new(ErrorKind::NotFound, "could not resolve the app data directory"))?;
    let app_dir_name = T::app_data_directory_name().trim();
    if app_dir_name.is_empty() {
        dir.push(T::project_file_spec().normalized_display_name());
    } else {
        dir.push(app_dir_name);
    }
    Ok(dir)
}

fn print_usage() {
    let executable = std::env::args().next().unwrap_or_else(|| "app".to_string());
    let program_name = Path::new(&executable)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("app");

    println!("Usage: {program_name} [--headless] [--dev] [--no-frontend] [--show-output] [--no-remote]");
    println!("  --headless   Run without launching the Tauri desktop window.");
    println!("  --dev   Launch against the frontend dev server instead of bundled UI assets.");
    println!("  --no-frontend  Launch Tauri against an external frontend instead of the bundled UI.");
    println!("  --show-output  Attach or create a console window for stdout/stderr logs.");
    println!("  --no-remote  Bind UI API to loopback only (blocks non-local browser access).");
}

#[cfg(target_os = "windows")]
fn maybe_enable_output_console_from_env() -> std::io::Result<()> {
    let should_show_output = std::env::args_os().skip(1).any(|arg| {
        let arg = arg.to_string_lossy();
        arg == "--show-output" || arg == "--help" || arg == "-h"
    });

    if should_show_output {
        ensure_console_attached()?;
    }

    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn maybe_enable_output_console_from_env() -> std::io::Result<()> {
    Ok(())
}

#[cfg(target_os = "windows")]
fn ensure_console_attached() -> std::io::Result<()> {
    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
    const ERROR_ACCESS_DENIED: i32 = 5;

    unsafe extern "system" {
        fn AllocConsole() -> i32;
        fn AttachConsole(dw_process_id: u32) -> i32;
    }

    // Prefer the parent terminal when one exists so `cargo run -- --show-output`
    // stays in the same console instead of opening a second window.
    let attached = unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
    if attached != 0 {
        return Ok(());
    }

    let attach_error = Error::last_os_error();
    if attach_error.raw_os_error() == Some(ERROR_ACCESS_DENIED) {
        return Ok(());
    }

    let allocated = unsafe { AllocConsole() };
    if allocated != 0 {
        return Ok(());
    }

    let alloc_error = Error::last_os_error();
    if alloc_error.raw_os_error() == Some(ERROR_ACCESS_DENIED) {
        return Ok(());
    }

    Err(alloc_error)
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

fn default_frontend_url(
    endpoint: &UiEndpoint,
    ui_assets: &[UiAsset],
    use_dev_server: bool,
    dev_server: Option<FrontendDevServerConfig>,
) -> String {
    if use_dev_server {
        return dev_server
            .map(|config| config.url.to_string())
            .unwrap_or_else(detect_or_default_frontend_url);
    }

    if !ui_assets.is_empty() {
        return format!("http://{}/", endpoint.connect_addr);
    }

    detect_or_default_frontend_url()
}

fn spawn_frontend_dev_server(
    dev_server: Option<FrontendDevServerConfig>,
    frontend_url: &str,
    show_output: bool,
) -> std::io::Result<Option<DevFrontendProcess>> {
    let Some(connect_addr) = url_connect_addr(frontend_url) else {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            format!("invalid frontend dev server url '{frontend_url}'"),
        ));
    };

    if wait_for_ui_server(&connect_addr, Duration::from_millis(250)).is_ok() {
        return Ok(None);
    }

    let Some(dev_server) = dev_server else {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "no frontend dev server configuration was provided by the app shell",
        ));
    };

    let mut command = Command::new(npm_command_name());
    command
        .arg("run")
        .arg(dev_server.npm_script)
        .current_dir(dev_server.working_dir);

    if show_output {
        command.stdin(Stdio::inherit());
        command.stdout(Stdio::inherit());
        command.stderr(Stdio::inherit());
    } else {
        command.stdin(Stdio::null());
        command.stdout(Stdio::null());
        command.stderr(Stdio::null());
    }

    let process = DevFrontendProcess::spawn(&mut command).map_err(|err| {
        Error::new(
            err.kind(),
            format!(
                "failed to start frontend dev server with '{} run {}' in {}: {err}",
                npm_command_name(),
                dev_server.npm_script,
                dev_server.working_dir
            ),
        )
    })?;

    match wait_for_ui_server(&connect_addr, UI_STARTUP_TIMEOUT) {
        Ok(()) => Ok(Some(process)),
        Err(err) => {
            drop(process);
            Err(Error::new(
                err.kind(),
                format!("frontend dev server at {frontend_url} did not become ready: {err}"),
            ))
        }
    }
}

fn npm_command_name() -> &'static str {
    if cfg!(windows) { "npm.cmd" } else { "npm" }
}

impl Drop for DevFrontendProcess {
    fn drop(&mut self) {
        self.terminate();
    }
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

fn run_tauri<R: Runtime>(
    ui_base_url: &str,
    tauri_context: tauri::Context<R>,
    window_state_path: PathBuf,
) -> std::io::Result<()> {
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

    // Runs at document-start, before <html> exists, so guard documentElement
    // (otherwise `document.documentElement.dataset` throws on null at launch).
    let init_script = format!(
        "window.__PLATFORM__ = '{os}'; (function () {{ var apply = function () {{ document.documentElement.dataset.platform = '{os}'; }}; if (document.documentElement) {{ apply(); }} else {{ document.addEventListener('DOMContentLoaded', apply); }} }})();"
    );

    let dispatch_close_requested_to_frontend = |window: &tauri::Window<R>| -> bool {
        let mut delivered = false;
        for webview in window.webviews() {
            match webview.eval(WINDOW_CLOSE_REQUESTED_SCRIPT) {
                Ok(()) => {
                    delivered = true;
                }
                Err(err) => {
                    eprintln!(
                        "warning: failed to dispatch close-request event to frontend for window '{}': {err}",
                        window.label()
                    );
                }
            }
        }
        delivered
    };

    let window_state = WindowStatePersistence::new(window_state_path);

    tauri::Builder::<R>::new()
        .manage(window_state.clone())
        .on_window_event(move |window, event| {
            if window.label() != "main" {
                return;
            }

            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if let Some(window_state) = window.try_state::<WindowStatePersistence>() {
                    save_window_state(window, &window_state);
                }

                if dispatch_close_requested_to_frontend(window) {
                    api.prevent_close();
                }
            }
        })
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
                // window_builder = window_builder.transparent(true);
            }

            let window = window_builder
                .build()
                .map_err(|err| Error::other(format!("failed creating Tauri window: {err}")))?;

            restore_window_state(&window, &window_state);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            crate::desktop_commands::window_minimize,
            crate::desktop_commands::window_toggle_maximize,
            crate::desktop_commands::window_close,
            crate::desktop_commands::window_destroy,
            crate::desktop_commands::window_is_maximized,
            crate::desktop_commands::start_drag,
            crate::desktop_commands::open_file_dialog,
            crate::desktop_commands::save_file_dialog,
            crate::desktop_commands::write_app_data_file,
            crate::desktop_commands::write_file_in_directory
        ])
        .append_invoke_initialization_script(&init_script)
        .run(tauri_context)
        .map_err(|err| Error::other(format!("tauri runtime failed: {err}")))
}
