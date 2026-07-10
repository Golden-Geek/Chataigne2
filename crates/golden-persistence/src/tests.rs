use std::fs;
use std::io::Write;
use std::path::PathBuf;

use atomic_write_file::AtomicWriteFile;
use golden_model::Revision;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::*;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct TestProject {
    name: String,
    values: Vec<u64>,
}

fn codec() -> ProjectCodec<TestProject> {
    ProjectCodec::new(
        "test",
        "1.0.0",
        1,
        PersistenceLimits::default(),
        MigrationRegistry::default(),
    )
    .unwrap()
}

fn validate(project: &TestProject) -> Result<(), String> {
    (!project.name.is_empty())
        .then_some(())
        .ok_or_else(|| "name is empty".into())
}

#[test]
fn project_round_trip_preserves_revision_and_large_documents() {
    let temp = TestDirectory::new();
    let path = temp.path.join("large.golden.json");
    let project = TestProject {
        name: "large".into(),
        values: (0..100_000).collect(),
    };
    let snapshot = SaveSnapshot::new(Revision::new(42), project.clone());
    let store = PersistenceStore::new(3).unwrap();
    store.save(&path, &codec(), &snapshot, &validate).unwrap();
    let loaded = store.load_or_recover(&path, &codec(), &validate).unwrap();
    assert_eq!(loaded.source, RecoverySource::Primary);
    assert_eq!(loaded.snapshot.revision, Revision::new(42));
    assert_eq!(loaded.snapshot.document(), &project);
}

#[test]
fn interrupted_atomic_write_keeps_the_last_good_project() {
    let temp = TestDirectory::new();
    let path = temp.path.join("interrupted.golden.json");
    let store = PersistenceStore::new(3).unwrap();
    store
        .save(
            &path,
            &codec(),
            &SaveSnapshot::new(
                Revision::new(1),
                TestProject {
                    name: "good".into(),
                    values: vec![1],
                },
            ),
            &validate,
        )
        .unwrap();
    {
        let mut interrupted = AtomicWriteFile::open(&path).unwrap();
        interrupted.write_all(b"incomplete replacement").unwrap();
    }
    let loaded = store.load_or_recover(&path, &codec(), &validate).unwrap();
    assert_eq!(loaded.snapshot.document().name, "good");
}

#[test]
fn corrupt_primary_recovers_the_newest_valid_backup() {
    let temp = TestDirectory::new();
    let path = temp.path.join("recover.golden.json");
    let store = PersistenceStore::new(2).unwrap();
    for revision in 1..=3 {
        store
            .save(
                &path,
                &codec(),
                &SaveSnapshot::new(
                    Revision::new(revision),
                    TestProject {
                        name: format!("revision-{revision}"),
                        values: vec![revision],
                    },
                ),
                &validate,
            )
            .unwrap();
    }
    fs::write(&path, b"corrupt").unwrap();
    let loaded = store.load_or_recover(&path, &codec(), &validate).unwrap();
    assert!(matches!(loaded.source, RecoverySource::Backup(_)));
    assert_eq!(loaded.snapshot.document().name, "revision-2");
    assert!(loaded.primary_error.is_some());
}

#[test]
fn limits_and_validation_run_before_application() {
    let strict = ProjectCodec::new(
        "test",
        "1.0.0",
        1,
        PersistenceLimits {
            maximum_file_bytes: 64,
            maximum_json_values: 8,
            maximum_json_depth: 4,
        },
        MigrationRegistry::default(),
    )
    .unwrap();
    let error = strict
        .encode(
            &SaveSnapshot::new(
                Revision::ZERO,
                TestProject {
                    name: "large".into(),
                    values: vec![1; 100],
                },
            ),
            &validate,
        )
        .unwrap_err();
    assert!(matches!(error, PersistenceError::FileTooLarge { .. }));
}

#[test]
fn migrations_are_ordered_and_start_from_the_clean_v1_schema() {
    let mut migrations = MigrationRegistry::default();
    migrations
        .register(1, |mut document| {
            document["name"] = serde_json::Value::String("migrated".into());
            Ok(document)
        })
        .unwrap();
    let codec = ProjectCodec::<TestProject>::new("test", "2.0.0", 2, PersistenceLimits::default(), migrations).unwrap();
    let old = ProjectFile {
        schema_version: 1,
        application_id: "test".into(),
        application_version: "1.0.0".into(),
        revision: Revision::new(9),
        document: serde_json::json!({ "values": [1, 2, 3] }),
    };
    let bytes = serde_json::to_vec(&old).unwrap();
    let migrated = codec.decode(&bytes, &validate).unwrap();
    assert_eq!(migrated.document().name, "migrated");
    assert_eq!(migrated.revision, Revision::new(9));
}

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("golden-persistence-{}", Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        Self { path }
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
