use std::io::ErrorKind;
use std::path::PathBuf;

use crate::parse_launch_args;

#[test]
fn automation_shutdown_file_is_parsed_with_standard_launch_flags() {
    let parsed = parse_launch_args(["--no-remote", "--automation-shutdown-file", "target/product-gate/stop"])
        .expect("automation shutdown path should parse");

    assert!(parsed.no_remote);
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
