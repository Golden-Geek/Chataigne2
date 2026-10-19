use std::fs;
use std::io;
use std::path::PathBuf;

use super::build_if_stale;

struct Fixture {
    _temp: tempfile::TempDir,
    input: PathBuf,
    output: PathBuf,
    stamp: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("source.txt");
        let output = temp.path().join("bundle");
        let stamp = temp.path().join("bundle.stamp");
        fs::write(&input, "source").unwrap();
        Self {
            _temp: temp,
            input,
            output,
            stamp,
        }
    }

    fn build(&self, context: &str, force: bool) -> bool {
        build_if_stale(
            &self.stamp,
            &self.output,
            std::slice::from_ref(&self.input),
            context,
            force,
            || {
                fs::create_dir_all(&self.output)?;
                fs::write(self.output.join("index.html"), "bundle")
            },
        )
        .unwrap()
    }
}

#[test]
fn unchanged_inputs_reuse_bundle_without_running_builder() {
    let fixture = Fixture::new();
    assert!(fixture.build("node-version", false));
    assert!(
        !build_if_stale(
            &fixture.stamp,
            &fixture.output,
            std::slice::from_ref(&fixture.input),
            "node-version",
            false,
            || panic!("cache hit must not execute the external builder"),
        )
        .unwrap()
    );
}

#[test]
fn edits_and_context_changes_invalidate_bundle() {
    let fixture = Fixture::new();
    assert!(fixture.build("node-v1", false));
    fs::write(&fixture.input, "edited").unwrap();
    assert!(fixture.build("node-v1", false));
    assert!(!fixture.build("node-v1", false));
    assert!(fixture.build("node-v2", false));
    assert!(fixture.build("node-v2", true));
}

#[test]
fn adding_or_removing_inputs_invalidates_bundle() {
    let fixture = Fixture::new();
    fixture.build("", false);
    let added = fixture.input.with_file_name("added.txt");
    fs::write(&added, "added").unwrap();
    assert!(
        build_if_stale(
            &fixture.stamp,
            &fixture.output,
            &[fixture.input.clone(), added],
            "",
            false,
            || Ok(()),
        )
        .unwrap()
    );
    assert!(fixture.build("", false));
}

#[test]
fn damaged_or_deleted_outputs_are_rebuilt() {
    let fixture = Fixture::new();
    fixture.build("", false);
    fs::write(fixture.output.join("index.html"), "corrupt").unwrap();
    assert!(fixture.build("", false));
    fs::remove_file(fixture.output.join("index.html")).unwrap();
    assert!(fixture.build("", false));
    fs::remove_file(fixture.output.join("index.html")).unwrap();
    fs::remove_dir(&fixture.output).unwrap();
    assert!(fixture.build("", false));
}

#[test]
fn invalid_stamp_is_treated_as_a_cache_miss() {
    let fixture = Fixture::new();
    fixture.build("", false);
    fs::write(&fixture.stamp, "interrupted write").unwrap();
    assert!(fixture.build("", false));
}

#[test]
fn failed_rebuild_invalidates_previous_stamp() {
    let fixture = Fixture::new();
    fixture.build("", false);
    let result = build_if_stale(
        &fixture.stamp,
        &fixture.output,
        std::slice::from_ref(&fixture.input),
        "",
        true,
        || Err(io::Error::other("builder failed")),
    );
    assert!(result.is_err());
    assert!(!fixture.stamp.exists());
    assert!(fixture.build("", false));
}

#[test]
fn successful_builder_without_outputs_does_not_populate_cache() {
    let fixture = Fixture::new();
    let result = build_if_stale(
        &fixture.stamp,
        &fixture.output,
        std::slice::from_ref(&fixture.input),
        "",
        false,
        || Ok(()),
    );
    assert!(result.is_err());
    assert!(!fixture.stamp.exists());
}

#[test]
fn inputs_edited_during_build_are_not_cached() {
    let fixture = Fixture::new();
    build_if_stale(
        &fixture.stamp,
        &fixture.output,
        std::slice::from_ref(&fixture.input),
        "",
        false,
        || {
            fs::create_dir_all(&fixture.output)?;
            fs::write(fixture.output.join("index.html"), "bundle")?;
            fs::write(&fixture.input, "edited during build")
        },
    )
    .unwrap();
    assert!(!fixture.stamp.exists());
    assert!(fixture.build("", false));
}
