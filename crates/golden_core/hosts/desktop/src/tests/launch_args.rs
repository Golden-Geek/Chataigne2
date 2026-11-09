use std::io::ErrorKind;
use std::path::PathBuf;

use crate::parse_launch_args;

#[test]
fn automation_shutdown_file_is_parsed_with_standard_launch_flags() {
    let parsed = parse_launch_args([
        "--headless",
        "--no-remote",
        "--automation-shutdown-file",
        "target/product-gate/stop",
    ])
    .expect("automation shutdown path should parse");

    assert!(parsed.no_remote);
    assert!(parsed.headless);
    assert_eq!(
        parsed.automation_shutdown_file,
        Some(PathBuf::from("target/product-gate/stop"))
    );
}

#[test]
fn automation_shutdown_file_requires_a_path() {
    let error = parse_launch_args(["--automation-shutdown-file"])
        .expect_err("missing automation shutdown path should be rejected");

    assert_eq!(error.kind(), ErrorKind::InvalidInput);
    assert!(error.to_string().contains("requires a path"));
}

#[test]
fn webview_automation_rejects_invalid_ports_and_incomplete_ownership() {
    for arguments in [
        vec!["--automation-webview-debug-port"],
        vec!["--automation-webview-debug-port", "0"],
        vec!["--automation-webview-debug-port", "65536"],
        vec!["--automation-webview-debug-port", "abc"],
        vec!["--automation-webview-debug-port", "9222"],
        vec![
            "--no-remote",
            "--automation-shutdown-file",
            "relative/stop",
            "--automation-webview-debug-port",
            "9222",
        ],
        vec![
            "--headless",
            "--no-remote",
            "--automation-shutdown-file",
            "C:/owned/stop",
            "--automation-webview-debug-port",
            "9222",
        ],
    ] {
        assert_eq!(
            parse_launch_args(arguments).unwrap_err().kind(),
            ErrorKind::InvalidInput
        );
    }
}

#[test]
fn webview_automation_is_available_only_in_debug_windows_desktop_builds() {
    let parsed = parse_launch_args([
        "--no-remote",
        "--automation-shutdown-file",
        "C:/owned/stop",
        "--automation-webview-debug-port",
        "9222",
    ]);
    if cfg!(all(target_os = "windows", debug_assertions)) {
        assert_eq!(parsed.unwrap().automation_webview_debug_port.unwrap().get(), 9222);
    } else {
        assert_eq!(parsed.unwrap_err().kind(), ErrorKind::InvalidInput);
    }
}
