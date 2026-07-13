use crate::watch::is_parent_cargo_package_variable;
use std::ffi::OsStr;

#[cfg(not(target_os = "windows"))]
use crate::cli::WatchConfig;
#[cfg(not(target_os = "windows"))]
use std::path::PathBuf;
#[cfg(not(target_os = "windows"))]
use std::time::Duration;

#[test]
fn nested_cargo_drops_parent_package_context_but_keeps_user_configuration() {
    for name in [
        "CARGO_BIN_NAME",
        "CARGO_CRATE_NAME",
        "CARGO_MANIFEST_DIR",
        "CARGO_MANIFEST_PATH",
        "CARGO_PRIMARY_PACKAGE",
        "CARGO_TARGET_TMPDIR",
        "CARGO_BIN_EXE_Chataigne2",
        "CARGO_PKG_VERSION",
    ] {
        assert!(
            is_parent_cargo_package_variable(OsStr::new(name)),
            "{name} should not leak from xtask into nested Cargo"
        );
    }

    for name in [
        "CARGO",
        "CARGO_HOME",
        "CARGO_INCREMENTAL",
        "CARGO_MAKEFLAGS",
        "CARGO_TARGET_DIR",
        "GC_UI_ASSUME_BUILT",
    ] {
        assert!(
            !is_parent_cargo_package_variable(OsStr::new(name)),
            "{name} is user or shared Cargo configuration and must be preserved"
        );
    }
}

#[cfg(not(target_os = "windows"))]
#[test]
fn shutdown_file_is_rejected_where_signal_shutdown_is_authoritative() {
    let config = WatchConfig {
        frontend_port: 5173,
        backend_port: 7010,
        frontend_timeout: Duration::from_secs(30),
        backend_timeout: Duration::from_secs(30),
        engine_timeout: Duration::from_secs(30),
        poll_interval: Duration::from_millis(200),
        headless: false,
        shutdown_file: Some(PathBuf::from("stop")),
        app_args: Vec::new(),
    };

    assert_eq!(
        crate::watch::run(config),
        Err("--shutdown-file is supported only on Windows".to_string())
    );
}
