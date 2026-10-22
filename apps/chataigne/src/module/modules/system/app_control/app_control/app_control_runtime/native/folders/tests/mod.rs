use super::super::super::super::WatchedFolderEntry;
use super::*;

fn entry(path: &Path) -> WatchedFolderEntry {
    WatchedFolderEntry {
        label: "fixture".into(),
        item_id: NodeId(1),
        uuid: NodeUuid::nil(),
        target_id: None,
        changed_id: None,
        target_path: path.to_string_lossy().into_owned(),
    }
}
#[test]
fn owned_folder_baselines_preserve_created_modified_and_removed_occurrences() {
    let dir = tempfile::tempdir().unwrap();
    let entry = entry(dir.path());
    let mut stored = HashMap::new();
    let first = sample(&mut stored, &entry, &mut Budget::default()).unwrap();
    assert!(!first.update.has_changes());
    fs::write(dir.path().join("first.txt"), b"one").unwrap();
    let created = sample(&mut stored, &entry, &mut Budget::default()).unwrap();
    assert_eq!(created.update.created, vec!["first.txt"]);
    fs::write(dir.path().join("first.txt"), b"longer").unwrap();
    let modified = sample(&mut stored, &entry, &mut Budget::default()).unwrap();
    assert_eq!(modified.update.modified, vec!["first.txt"]);
    fs::remove_file(dir.path().join("first.txt")).unwrap();
    let removed = sample(&mut stored, &entry, &mut Budget::default()).unwrap();
    assert_eq!(removed.update.removed, vec!["first.txt"]);
}
#[test]
fn rejected_inventory_does_not_advance_the_previous_baseline() {
    let dir = tempfile::tempdir().unwrap();
    let entry = entry(dir.path());
    let mut stored = HashMap::new();
    sample(&mut stored, &entry, &mut Budget::default()).unwrap();
    fs::write(dir.path().join("created.txt"), b"fixture").unwrap();
    let mut exhausted = Budget {
        entries: MAX_ENTRIES,
        bytes: 0,
    };
    assert!(sample(&mut stored, &entry, &mut exhausted).is_err());
    let next = sample(&mut stored, &entry, &mut Budget::default()).unwrap();
    assert_eq!(next.update.created, vec!["created.txt"]);
}
#[test]
fn a_new_target_starts_a_new_baseline_without_removing_the_previous_roots_entries() {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let mut stored = HashMap::new();
    fs::write(first.path().join("old.txt"), b"fixture").unwrap();
    sample(&mut stored, &entry(first.path()), &mut Budget::default()).unwrap();
    let reset = sample(&mut stored, &entry(second.path()), &mut Budget::default()).unwrap();
    assert!(!reset.update.has_changes());
    assert_eq!(reset.update.entry_count, 0);
}
#[test]
fn depth_and_combined_root_entry_caps_reject_before_storing_a_partial_baseline() {
    let dir = tempfile::tempdir().unwrap();
    let mut nested = dir.path().to_path_buf();
    for _ in 0..33 {
        nested.push("d");
        fs::create_dir(&nested).unwrap();
    }
    let mut stored = HashMap::new();
    assert!(sample(&mut stored, &entry(dir.path()), &mut Budget::default()).is_err());
    assert!(stored.is_empty());
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    fs::write(first.path().join("one"), b"one").unwrap();
    fs::write(second.path().join("two"), b"two").unwrap();
    let mut budget = Budget {
        entries: MAX_ENTRIES - 1,
        bytes: 0,
    };
    sample(&mut stored, &entry(first.path()), &mut budget).unwrap();
    let mut other = entry(second.path());
    other.item_id = NodeId(2);
    assert!(sample(&mut stored, &other, &mut budget).is_err());
    assert!(!stored.contains_key(&NodeId(2)));
}
