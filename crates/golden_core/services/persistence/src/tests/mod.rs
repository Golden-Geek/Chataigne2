use std::fs;

use tempfile::tempdir;

use super::{read_recovery_candidates, restore_primary_from_backup, write_file_atomically_with_recovery};

#[test]
fn atomic_replacement_keeps_previous_complete_file_and_clears_journal() {
    let directory = tempdir().expect("temporary persistence directory should be created");
    let target = directory.path().join("project.noisette");

    write_file_atomically_with_recovery(&target, b"first").expect("initial atomic save should succeed");
    let paths =
        write_file_atomically_with_recovery(&target, b"second").expect("replacement atomic save should succeed");

    assert_eq!(fs::read(&target).expect("primary should exist"), b"second");
    assert_eq!(fs::read(&paths.backup).expect("backup should exist"), b"first");
    assert!(!paths.journal.exists(), "completed save must clear its journal");
}

#[test]
fn recovery_candidates_preserve_corrupt_primary_and_last_complete_backup() {
    let directory = tempdir().expect("temporary persistence directory should be created");
    let target = directory.path().join("project.noisette");
    write_file_atomically_with_recovery(&target, br#"{"version":"1"}"#).expect("initial save should succeed");
    write_file_atomically_with_recovery(&target, br#"{"version":"2"}"#).expect("replacement save should succeed");
    fs::write(&target, b"{").expect("test should corrupt only the primary");

    let candidates = read_recovery_candidates(&target).expect("backup should keep recovery possible");
    assert_eq!(candidates.primary.as_deref(), Some(b"{".as_slice()));
    assert_eq!(candidates.backup.as_deref(), Some(br#"{"version":"1"}"#.as_slice()));

    let backup = candidates.backup.as_deref().expect("backup should be readable");
    restore_primary_from_backup(&candidates.paths, backup)
        .expect("validated backup should atomically repair the primary");
    assert_eq!(fs::read(&target).expect("primary should be repaired"), backup);
    assert_eq!(
        fs::read(&candidates.paths.backup).expect("backup should remain intact"),
        backup
    );
    assert!(!candidates.paths.journal.exists());
}
