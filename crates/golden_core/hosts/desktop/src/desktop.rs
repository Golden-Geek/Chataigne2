use std::env;
use std::io::{Error, ErrorKind};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use tauri::{Runtime, Url};

use crate::desktop_window::run_tauri;
use crate::host_config::{DesktopHostConfig, FrontendDevServerConfig, apply_listener_overrides};
use crate::host_services::HostServices;
use crate::native_presentation::{NativePresentation, host::validate_platform};
#[cfg(target_os = "windows")]
use crate::windows_process_job::WindowsProcessJob;
use golden_engine::app::{ProjectLifecycle, create_new_project_engine};
use golden_engine::application::ProductionRuntime;
use golden_engine::engine::Engine;
use golden_transport_server::{UiAsset, UiPreferencesConfig, UiServerConfig, run_with_ui_server_config_and_ready};

const UI_STARTUP_TIMEOUT: Duration = Duration::from_secs(5);
const UI_PROBE_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Debug, Clone, Default)]
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
    /// Stops the headless or Windows desktop host when this automation sentinel appears.
    pub automation_shutdown_file: Option<PathBuf>,
    /// Prints default launch usage instead of starting the app.
    pub show_help: bool,
}

#[derive(Debug, Clone)]
struct UiEndpoint {
    connect_addr: String,
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
    run_default_with_config::<T, R>(
        tauri_context,
        DesktopHostConfig {
            ui_server: UiServerConfig {
                frontend_assets: ui_assets,
                ..Default::default()
            },
            dev_server,
        },
    )
}

/// Parses process arguments and launches the app with its shared host configuration.
pub fn run_default_with_config<T, R>(tauri_context: tauri::Context<R>, config: DesktopHostConfig) -> std::io::Result<()>
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

    launch_with_config::<T, R>(args, tauri_context, config)
}

/// Launches the shared host with app-owned services for the selected desktop/headless mode.
pub fn run_default_with_services<T, R>(
    tauri_context: tauri::Context<R>,
    config: DesktopHostConfig,
    services: HostServices<T, R>,
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
    let engine = create_new_project_engine::<T>().map_err(Error::other)?;
    launch_engine_with_services(engine, args, tauri_context, config, services)
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

    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_ref() {
            "--headless" => parsed.headless = true,
            "--dev" => parsed.dev = true,
            "--no-frontend" => parsed.no_frontend = true,
            "--show-output" => parsed.show_output = true,
            "--no-remote" => parsed.no_remote = true,
            "--automation-shutdown-file" => {
                let path = args
                    .next()
                    .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "--automation-shutdown-file requires a path"))?;
                parsed.automation_shutdown_file = Some(PathBuf::from(path.as_ref()));
            }
            "--help" | "-h" => parsed.show_help = true,
            other => {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    format!(
                        "unknown argument '{other}'. supported flags: --headless, --dev, --no-frontend, --show-output, --no-remote, --automation-shutdown-file PATH, --help"
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
    launch_with_config::<T, R>(
        args,
        tauri_context,
        DesktopHostConfig {
            ui_server: UiServerConfig {
                frontend_assets: ui_assets,
                ..Default::default()
            },
            dev_server,
        },
    )
}

/// Creates a new-project engine and launches it with app-provided shared host defaults.
pub fn launch_with_config<T, R>(
    args: LaunchArgs,
    tauri_context: tauri::Context<R>,
    config: DesktopHostConfig,
) -> std::io::Result<()>
where
    T: ProjectLifecycle + 'static,
    R: Runtime,
{
    let engine = create_new_project_engine::<T>().map_err(Error::other)?;
    launch_engine_with_config(engine, args, tauri_context, config)
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
    launch_engine_with_config(
        engine,
        args,
        tauri_context,
        DesktopHostConfig {
            ui_server: UiServerConfig {
                frontend_assets: ui_assets,
                ..Default::default()
            },
            dev_server,
        },
    )
}

/// Launches a caller-provided engine with app-provided shared host defaults.
pub fn launch_engine_with_config<T, R>(
    engine: Engine<T>,
    args: LaunchArgs,
    tauri_context: tauri::Context<R>,
    host_config: DesktopHostConfig,
) -> std::io::Result<()>
where
    T: ProjectLifecycle + 'static,
    R: Runtime,
{
    launch_engine_with_services(engine, args, tauri_context, host_config, HostServices::default())
}

/// Launches a caller-provided engine with a host-owned native presentation lifecycle.
///
/// Preparation runs on the UI thread after the transparent editor window is constructed.
/// The returned task runs on its dedicated worker. Exit waits asynchronously for task
/// retirement, then joins after the event loop returns. Headless mode never prepares the task.
pub fn launch_engine_with_native_presentation<T, R>(
    engine: Engine<T>,
    args: LaunchArgs,
    tauri_context: tauri::Context<R>,
    host_config: DesktopHostConfig,
    presentation: impl FnOnce(ProductionRuntime<T>) -> std::io::Result<NativePresentation<R>> + Send + 'static,
) -> std::io::Result<()>
where
    T: ProjectLifecycle + 'static,
    R: Runtime,
{
    launch_engine_with_services(
        engine,
        args,
        tauri_context,
        host_config,
        HostServices::default().with_native_presentation(presentation),
    )
}

/// Launches a provided engine with mode-specific services on the shared authoritative host.
pub fn launch_engine_with_services<T, R>(
    engine: Engine<T>,
    args: LaunchArgs,
    tauri_context: tauri::Context<R>,
    host_config: DesktopHostConfig,
    services: HostServices<T, R>,
) -> std::io::Result<()>
where
    T: ProjectLifecycle + 'static,
    R: Runtime,
{
    let HostServices { presentation, headless } = services;
    validate_platform(presentation.is_some(), args.headless)?;
    if !cfg!(target_os = "windows") && !args.headless && args.automation_shutdown_file.is_some() {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "--automation-shutdown-file is supported only on Windows",
        ));
    }
    if let Some(path) = args.automation_shutdown_file.as_ref()
        && path.exists()
    {
        return Err(Error::new(
            ErrorKind::AlreadyExists,
            format!("automation shutdown file already exists: {}", path.display()),
        ));
    }

    let DesktopHostConfig {
        ui_server: mut config,
        dev_server,
    } = host_config;
    let app_data_dir = default_app_data_dir::<T>()?;
    config.preferences = Some(UiPreferencesConfig {
        file_path: app_data_dir.join(T::preferences_file_name()),
        default_data_folder: app_data_dir.to_string_lossy().to_string(),
    });
    let frontend_assets = if args.no_frontend || args.dev {
        &[]
    } else {
        config.frontend_assets
    };
    apply_listener_overrides(
        &mut config,
        std::env::var("GC_UI_BIND").ok().as_deref(),
        std::env::var("GC_UI_ALLOWED_HOSTS").ok().as_deref(),
        args.no_remote,
    );

    let endpoint = resolve_ui_endpoint(&config.bind_addr);
    config.frontend_assets = frontend_assets;

    let configured_frontend_url = std::env::var("GC_UI_FRONTEND_URL")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let frontend_url = configured_frontend_url
        .clone()
        .unwrap_or_else(|| default_frontend_url(&endpoint, frontend_assets, args.dev, dev_server));
    configure_frontend_origin(&mut config, &args, &frontend_url, configured_frontend_url.is_some())?;

    if args.headless {
        let shutdown = config.shutdown.clone().unwrap_or_default();
        config.shutdown = Some(shutdown.clone());
        let watcher = crate::headless_shutdown::ShutdownFileWatcher::start(args.automation_shutdown_file, shutdown)?;
        let dev_server_process = if args.dev && configured_frontend_url.is_none() {
            spawn_frontend_dev_server(dev_server, &frontend_url, &endpoint.connect_addr, args.show_output)?
        } else {
            None
        };

        // Headless mode renders no frontend. A requested dev server is already gated by
        // `spawn_frontend_dev_server`; bundled assets start with the UI server below.
        let run_result = run_with_ui_server_config_and_ready(engine, config, move |runtime| {
            headless.map(|factory| factory(runtime)).transpose()
        });
        drop(dev_server_process);
        let watched = watcher.map_or(Ok(()), |watcher| watcher.finish());
        return run_result.and(watched);
    }

    let (startup_tx, startup_rx) = mpsc::channel::<std::io::Result<()>>();
    let (runtime_tx, runtime_rx) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let result = run_with_ui_server_config_and_ready(engine, config, move |runtime| {
            let _ = runtime_tx.send(runtime);
            Ok(())
        });
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

    let presentation = presentation
        .map(|factory| {
            let runtime = runtime_rx
                .recv_timeout(UI_STARTUP_TIMEOUT)
                .map_err(|error| Error::other(format!("authoritative runtime was not ready: {error}")))?;
            factory(runtime)
        })
        .transpose()?;

    let dev_server_process = if args.dev && configured_frontend_url.is_none() {
        spawn_frontend_dev_server(dev_server, &frontend_url, &endpoint.connect_addr, args.show_output)?
    } else {
        None
    };

    if let Some(connect_addr) = url_connect_addr(&frontend_url)
        && let Err(err) = wait_for_ui_server(&connect_addr, UI_STARTUP_TIMEOUT)
    {
        eprintln!(
            "warning: frontend UI at {frontend_url} was not reachable yet ({err}); continuing and launching Tauri anyway"
        );
    }

    let run_result = run_tauri(
        &frontend_url,
        T::application_display_name().to_string(),
        tauri_context,
        app_data_dir.join("window-state.json"),
        args.automation_shutdown_file,
        presentation,
    );
    drop(dev_server_process);
    run_result
}

fn default_app_data_dir<T: ProjectLifecycle>() -> std::io::Result<PathBuf> {
    let mut dir = environment_data_dir_override()
        .or_else(dirs::data_dir)
        .ok_or_else(|| Error::new(ErrorKind::NotFound, "could not resolve the app data directory"))?;
    let app_dir_name = T::app_data_directory_name().trim();
    if app_dir_name.is_empty() {
        dir.push(T::project_file_spec().normalized_display_name());
    } else {
        dir.push(app_dir_name);
    }
    Ok(dir)
}

fn environment_data_dir_override() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    let value = env::var_os("APPDATA");
    #[cfg(not(target_os = "windows"))]
    let value = env::var_os("XDG_DATA_HOME");

    value
        .filter(|value| !value.to_string_lossy().trim().is_empty())
        .map(PathBuf::from)
}

fn print_usage() {
    let executable = std::env::args().next().unwrap_or_else(|| "app".to_string());
    let program_name = Path::new(&executable)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("app");

    println!(
        "Usage: {program_name} [--headless] [--dev] [--no-frontend] [--show-output] [--no-remote] [--automation-shutdown-file PATH]"
    );
    println!("  --headless   Run without launching the Tauri desktop window.");
    println!("  --dev   Launch against the frontend dev server instead of bundled UI assets.");
    println!("  --no-frontend  Launch Tauri against an external frontend instead of the bundled UI.");
    println!("  --show-output  Attach or create a console window for stdout/stderr logs.");
    println!("  --no-remote  Bind UI API to loopback only (blocks non-local browser access).");
    println!("  --automation-shutdown-file PATH  Stop the host when PATH appears.");
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

pub(super) fn force_loopback_bind_addr(bind_addr: &str) -> String {
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
    if !use_dev_server && !ui_assets.is_empty() {
        return format!("http://{}/", endpoint.connect_addr);
    }

    dev_server
        .map(|config| config.url.to_string())
        .unwrap_or_else(detect_or_default_frontend_url)
}

fn configure_frontend_origin(
    config: &mut UiServerConfig,
    args: &LaunchArgs,
    frontend_url: &str,
    has_frontend_override: bool,
) -> std::io::Result<()> {
    if args.dev || args.no_frontend || has_frontend_override {
        let frontend: Url = frontend_url
            .parse()
            .map_err(|error| Error::new(ErrorKind::InvalidInput, error))?;
        config.allowed_origins.push(frontend.origin().ascii_serialization());
    }
    Ok(())
}

#[cfg(test)]
mod tests;

fn spawn_frontend_dev_server(
    dev_server: Option<FrontendDevServerConfig>,
    frontend_url: &str,
    backend_connect_addr: &str,
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
    let backend: Url = format!("http://{backend_connect_addr}")
        .parse()
        .map_err(|error| Error::new(ErrorKind::InvalidInput, error))?;
    let backend_port = backend
        .port_or_known_default()
        .filter(|port| *port > 0)
        .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "invalid UI backend port"))?;
    command
        .arg("run")
        .arg(dev_server.npm_script)
        .env("VITE_GC_UI_BACKEND_PORT", backend_port.to_string())
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
