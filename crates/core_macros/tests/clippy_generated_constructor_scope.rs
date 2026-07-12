use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn generated_constructor_exemption_does_not_suppress_handwritten_functions() {
    let fixture_dir = temporary_fixture_dir();
    let source_dir = fixture_dir.join("src");
    fs::create_dir_all(&source_dir).expect("fixture source directory should be created");

    let engine_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../core")
        .to_string_lossy()
        .replace('\\', "/");
    fs::write(
        fixture_dir.join("Cargo.toml"),
        format!(
            r#"[package]
name = "golden_core_macro_clippy_scope_fixture"
version = "0.0.0"
edition = "2024"
publish = false

[workspace]

[features]
handwritten = []

[dependencies]
golden_core = {{ package = "golden_engine", path = '{engine_path}' }}
serde_json = "1"
"#,
        ),
    )
    .expect("fixture manifest should be written");
    fs::write(
        source_dir.join("lib.rs"),
        r#"use golden_core::node;

#[node("generated_high_arity_node")]
pub struct GeneratedHighArityNode {
    first: u8,
    second: u8,
    third: u8,
    fourth: u8,
    fifth: u8,
    sixth: u8,
    seventh: u8,
    eighth: u8,
}

#[cfg(feature = "handwritten")]
pub fn handwritten_high_arity(
    first: u8,
    second: u8,
    third: u8,
    fourth: u8,
    fifth: u8,
    sixth: u8,
    seventh: u8,
    eighth: u8,
) -> u16 {
    [first, second, third, fourth, fifth, sixth, seventh, eighth]
        .into_iter()
        .map(u16::from)
        .sum()
}
"#,
    )
    .expect("fixture source should be written");

    let manifest = fixture_dir.join("Cargo.toml");
    let target_dir = fixture_dir.join("target");
    let generated_only = run_clippy(&manifest, &target_dir, false);
    let with_handwritten = run_clippy(&manifest, &target_dir, true);
    fs::remove_dir_all(&fixture_dir).ok();

    assert!(
        generated_only.status.success(),
        "generated high-arity constructor should be exempt:\n{}",
        command_output(&generated_only)
    );
    assert!(
        !with_handwritten.status.success(),
        "handwritten high-arity function should remain denied"
    );
    let handwritten_diagnostics = command_output(&with_handwritten);
    assert!(
        handwritten_diagnostics.contains("too many arguments")
            && handwritten_diagnostics.contains("handwritten_high_arity"),
        "expected the handwritten function's Clippy diagnostic:\n{handwritten_diagnostics}"
    );
}

fn temporary_fixture_dir() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after the Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "golden_core_macro_clippy_scope_{}_{}",
        std::process::id(),
        nonce
    ))
}

fn run_clippy(manifest: &Path, target_dir: &Path, handwritten: bool) -> Output {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut command = Command::new(cargo);
    command
        .arg("clippy")
        .arg("--manifest-path")
        .arg(manifest)
        .arg("--offline")
        .arg("--quiet")
        .arg("--lib")
        .env("CARGO_TARGET_DIR", target_dir)
        .env("CARGO_TERM_COLOR", "never")
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS");
    if handwritten {
        command.arg("--features").arg("handwritten");
    }
    command
        .arg("--")
        .arg("-Dclippy::too_many_arguments")
        .arg("-Adead_code")
        .output()
        .expect("fixture Clippy command should run")
}

fn command_output(output: &Output) -> String {
    format!(
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}
