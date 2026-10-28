use super::*;
use crate::project_host::upload_project_and_load_in_directory;
use golden_persistence::RecoveryPaths;

fn upload_fixture() -> (ProductionRuntime<ReplaceOrderTestNode>, Vec<String>) {
    let mut engine = Engine::<ReplaceOrderTestNode>::new(Folder::new("Authoritative Root").into());
    prepare_engine_for_runtime(&mut engine).unwrap();
    let runtime = production_runtime(engine);
    let candidate = Engine::<ReplaceOrderTestNode>::new(Folder::new("Foreign shared folder").into());
    let json = golden_engine::app::to_sparse_project_json_pretty(&candidate).unwrap();
    let mut foreign: serde_json::Value = serde_json::from_str(&json).unwrap();
    foreign["application_id"] = "foreign-application".into();
    let mut untagged = foreign.clone();
    untagged.as_object_mut().unwrap().remove("application_id");
    (runtime, vec![foreign.to_string(), untagged.to_string()])
}

fn temporary_directory(label: &str) -> std::path::PathBuf {
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    std::env::temp_dir().join(format!("golden-upload-{label}-{}-{nonce}", std::process::id()))
}

#[test]
fn rejected_upload_preserves_an_existing_target_and_its_backup() {
    let (runtime, uploads) = upload_fixture();
    let directory = temporary_directory("existing");
    fs::create_dir(&directory).unwrap();
    let file_name = sanitize_browser_upload_file_name("existing.json", &ReplaceOrderTestNode::project_file_spec());
    let paths = RecoveryPaths::for_target(directory.join(file_name)).unwrap();
    let own = Engine::<ReplaceOrderTestNode>::new(Folder::new("Existing own project").into());
    let primary = golden_engine::app::to_sparse_project_json_pretty(&own).unwrap();
    let previous = Engine::<ReplaceOrderTestNode>::new(Folder::new("Previous own project").into());
    let backup = golden_engine::app::to_sparse_project_json_pretty(&previous).unwrap();
    fs::write(&paths.target, &primary).unwrap();
    fs::write(&paths.backup, &backup).unwrap();
    let before = runtime.project_persistence_status().unwrap();

    for contents in uploads {
        for recover in [false, true] {
            let error =
                upload_project_and_load_in_directory(&runtime, "existing.json", &contents, None, recover, || {
                    Ok(directory.clone())
                })
                .err()
                .expect("invalid application identity must reject the upload");
            assert!(error.message.contains("application"), "{error}");
            assert_eq!(fs::read_to_string(&paths.target).unwrap(), primary);
            assert_eq!(fs::read_to_string(&paths.backup).unwrap(), backup);
            assert!(!paths.journal.exists());
            assert_eq!(fs::read_dir(&directory).unwrap().count(), 2);
            assert_eq!(runtime.project_persistence_status().unwrap(), before);
        }
    }
    fs::remove_file(paths.target).unwrap();
    fs::remove_file(paths.backup).unwrap();
    fs::remove_dir(directory).unwrap();
}

#[test]
fn rejected_upload_does_not_resolve_or_create_a_new_destination() {
    let (runtime, uploads) = upload_fixture();
    let directory = temporary_directory("absent");
    let before = runtime.project_persistence_status().unwrap();
    for contents in uploads {
        for recover in [false, true] {
            let error = upload_project_and_load_in_directory(&runtime, "new.json", &contents, None, recover, || {
                panic!("identity validation must precede destination resolution")
            })
            .err()
            .expect("invalid application identity must reject the upload");
            assert!(error.message.contains("application"), "{error}");
            assert!(!directory.exists());
            assert_eq!(runtime.project_persistence_status().unwrap(), before);
        }
    }
}
