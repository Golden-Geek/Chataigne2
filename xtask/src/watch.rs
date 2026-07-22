use crate::WatchConfig;
use crate::output;
use crate::process::{OwnedChild, command_display};
use crate::readiness;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

pub fn run(config: WatchConfig) -> Result<u8, String> {
    if !cfg!(target_os = "windows") && config.shutdown_file.is_some() {
        return Err("--shutdown-file is supported only on Windows".to_string());
    }

    readiness::ensure_port_available("frontend", config.frontend_port)?;
    readiness::ensure_port_available("backend", config.backend_port)?;

    let workspace_root = workspace_root()?;
    let frontend_url = format!("http://127.0.0.1:{}", config.frontend_port);
    let backend_url = format!("http://127.0.0.1:{}", config.backend_port);
    let shutdown = install_shutdown_handler()?;
    let shutdown_file = config.shutdown_file.as_ref().map(|path| {
        if path.is_absolute() {
            path.clone()
        } else {
            workspace_root.join(path)
        }
    });
    if let Some(path) = &shutdown_file
        && path.exists()
    {
        return Err(format!("watch shutdown file already exists: {}", path.display()));
    }

    output::status("frontend", "starting", &frontend_url);
    let mut frontend_command = frontend_command(&workspace_root, &config);
    let mut frontend = OwnedChild::spawn("frontend", &mut frontend_command)?;

    output::status("backend", "starting", &backend_url);
    let mut backend_command = backend_command(&workspace_root, &config, &frontend_url);
    let mut backend = match OwnedChild::spawn("backend", &mut backend_command) {
        Ok(child) => child,
        Err(error) => {
            frontend.terminate();
            return Err(error);
        }
    };

    wait_for(
        "frontend",
        config.frontend_timeout,
        config.poll_interval,
        &shutdown,
        &mut frontend,
        &mut backend,
        || readiness::probe_frontend(config.frontend_port),
    )?;
    output::status("frontend", "ready", &frontend_url);

    wait_for(
        "backend",
        config.backend_timeout,
        config.poll_interval,
        &shutdown,
        &mut frontend,
        &mut backend,
        || readiness::probe_backend_health(config.backend_port),
    )?;
    output::status("backend", "ready", format!("{backend_url}/api/ui/health"));

    wait_for(
        "engine",
        config.engine_timeout,
        config.poll_interval,
        &shutdown,
        &mut frontend,
        &mut backend,
        || readiness::probe_engine_read_model(config.backend_port),
    )?;
    output::status("engine", "ready", format!("{backend_url}/api/ui/health"));

    let mut active_session = None;
    wait_for(
        "session",
        config.engine_timeout,
        config.poll_interval,
        &shutdown,
        &mut frontend,
        &mut backend,
        || {
            readiness::probe_active_ui_session(config.backend_port).map(|readiness| {
                active_session = Some(readiness);
            })
        },
    )?;
    let active_session = active_session.expect("successful session probe must publish readiness");
    output::status(
        "session",
        "ready",
        format!(
            "{} subscribed UI session(s)",
            active_session.active_subscribed_websocket_clients
        ),
    );
    output::ready(
        &frontend_url,
        &backend_url,
        config.frontend_port,
        config.backend_port,
        &active_session,
    );
    eprintln!("[watch] ready; press Ctrl+C or close the application to stop");

    supervise(&shutdown, shutdown_file.as_deref(), &mut frontend, &mut backend)
}

fn wait_for<F>(
    component: &str,
    timeout: Duration,
    poll_interval: Duration,
    shutdown: &AtomicBool,
    frontend: &mut OwnedChild,
    backend: &mut OwnedChild,
    mut probe: F,
) -> Result<(), String>
where
    F: FnMut() -> Result<(), String>,
{
    let deadline = Instant::now() + timeout;
    loop {
        if shutdown.load(Ordering::SeqCst) {
            return Err(format!("startup interrupted while waiting for {component}"));
        }
        ensure_running(frontend, backend)?;
        let last_error = match probe() {
            Ok(()) => return Ok(()),
            Err(error) => error,
        };
        if Instant::now() >= deadline {
            return Err(format!(
                "{component} did not become ready within {:.1}s; last probe: {last_error}",
                timeout.as_secs_f64()
            ));
        }
        thread::sleep(poll_interval);
    }
}

fn ensure_running(frontend: &mut OwnedChild, backend: &mut OwnedChild) -> Result<(), String> {
    if let Some(status) = frontend.try_wait()? {
        return Err(format!("frontend exited before readiness ({status})"));
    }
    if let Some(status) = backend.try_wait()? {
        return Err(format!("backend exited before readiness ({status})"));
    }
    Ok(())
}

fn supervise(
    shutdown: &AtomicBool,
    shutdown_file: Option<&Path>,
    frontend: &mut OwnedChild,
    backend: &mut OwnedChild,
) -> Result<u8, String> {
    loop {
        let shutdown_detail = if shutdown.load(Ordering::SeqCst) {
            Some("interrupt received")
        } else if shutdown_file.is_some_and(Path::exists) {
            Some("shutdown file observed")
        } else {
            None
        };
        if let Some(detail) = shutdown_detail {
            output::status("watch", "stopping", detail);
            backend.terminate();
            frontend.terminate();
            output::status("watch", "stopped", detail);
            return Ok(0);
        }
        if let Some(status) = backend.try_wait()? {
            frontend.terminate();
            if status.success() {
                output::status("watch", "stopped", "application closed");
                return Ok(0);
            }
            return Err(format!("backend exited unexpectedly ({status})"));
        }
        if let Some(status) = frontend.try_wait()? {
            backend.terminate();
            return Err(format!("frontend exited unexpectedly ({status})"));
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn frontend_command(workspace_root: &Path, config: &WatchConfig) -> Command {
    let npm = npm_command();
    let port = config.frontend_port.to_string();
    let args = [
        "run",
        "dev",
        "--",
        "--host",
        "127.0.0.1",
        "--port",
        port.as_str(),
        "--strictPort",
    ];
    eprintln!("[watch][frontend] command: {}", command_display(&npm, &args));
    let mut command = Command::new(npm);
    command.args(args).current_dir(workspace_root.join("apps/chataigne/ui"));
    command
}

fn backend_command(workspace_root: &Path, config: &WatchConfig, frontend_url: &str) -> Command {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let backend_bind = format!("127.0.0.1:{}", config.backend_port);
    let mut command = Command::new(&cargo);
    clear_parent_cargo_package_environment(&mut command);
    command
        .args(["run", "--package", "Chataigne2", "--", "--dev"])
        .current_dir(workspace_root)
        .env("GC_UI_BIND", backend_bind)
        .env("GC_UI_FRONTEND_URL", frontend_url);
    if config.headless {
        command.arg("--headless");
    }
    command.args(&config.app_args);
    eprintln!(
        "[watch][backend] command: cargo run --package Chataigne2 -- --dev{}{}",
        if config.headless { " --headless" } else { "" },
        if config.app_args.is_empty() {
            String::new()
        } else {
            format!(" {}", config.app_args.join(" "))
        }
    );
    command
}

fn clear_parent_cargo_package_environment(command: &mut Command) {
    for (name, _) in std::env::vars_os() {
        if is_parent_cargo_package_variable(&name) {
            command.env_remove(name);
        }
    }
}

pub(crate) fn is_parent_cargo_package_variable(name: &OsStr) -> bool {
    let Some(name) = name.to_str() else {
        return false;
    };

    matches!(
        name,
        "CARGO_BIN_NAME"
            | "CARGO_CRATE_NAME"
            | "CARGO_MANIFEST_DIR"
            | "CARGO_MANIFEST_PATH"
            | "CARGO_PRIMARY_PACKAGE"
            | "CARGO_TARGET_TMPDIR"
    ) || name.starts_with("CARGO_BIN_EXE_")
        || name.starts_with("CARGO_PKG_")
}

fn workspace_root() -> Result<PathBuf, String> {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(PathBuf::from)
        .ok_or_else(|| "xtask manifest directory has no workspace parent".to_string())
}

fn install_shutdown_handler() -> Result<Arc<AtomicBool>, String> {
    let shutdown = Arc::new(AtomicBool::new(false));
    let signal = Arc::clone(&shutdown);
    ctrlc::set_handler(move || signal.store(true, Ordering::SeqCst))
        .map_err(|error| format!("failed to install Ctrl+C handler: {error}"))?;
    Ok(shutdown)
}

#[cfg(windows)]
fn npm_command() -> OsString {
    OsString::from("npm.cmd")
}

#[cfg(not(windows))]
fn npm_command() -> OsString {
    OsString::from("npm")
}
