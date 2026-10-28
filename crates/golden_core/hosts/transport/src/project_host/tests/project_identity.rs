use super::*;
use crate::project_host::load_project_recovering;
use golden_engine::application::ProjectSaveRequest;

#[test]
fn rejected_foreign_or_untagged_load_keeps_the_live_project_active_and_unchanged() {
    let mut engine = Engine::<ReplaceOrderTestNode>::new(Folder::new("Authoritative Root").into());
    prepare_engine_for_runtime(&mut engine).unwrap();
    let runtime = production_runtime(engine);
    let before = runtime.project_persistence_status().unwrap();
    let candidate = Engine::<ReplaceOrderTestNode>::new(Folder::new("Foreign shared folder").into());
    let json = golden_engine::app::to_sparse_project_json_pretty(&candidate).unwrap();
    let mut wire: serde_json::Value = serde_json::from_str(&json).unwrap();
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let path = std::env::temp_dir().join(format!(
        "golden-application-isolation-{}-{nonce}.json",
        std::process::id()
    ));

    for identity in [Some("foreign-application"), None] {
        if let Some(identity) = identity {
            wire["application_id"] = identity.into();
        } else {
            wire.as_object_mut().unwrap().remove("application_id");
        }
        fs::write(&path, wire.to_string()).unwrap();
        for recovering in [false, true] {
            let result = if recovering {
                load_project_recovering(&runtime, path.to_string_lossy().as_ref(), None)
            } else {
                load_project(&runtime, path.to_string_lossy().as_ref(), None)
            };
            let error = result.err().expect("identity rejection must precede replacement");
            assert!(error.control.is_none());
            assert_eq!(runtime.project_persistence_status().unwrap(), before);
            assert!(matches!(
                runtime.project_runtime_status().unwrap(),
                ProjectRuntimeStatus::Active { .. }
            ));
            assert!(
                runtime
                    .read_model()
                    .current_snapshot()
                    .nodes
                    .iter()
                    .any(|node| node.meta.label == "Authoritative Root")
            );
            assert!(
                !runtime
                    .read_model()
                    .current_snapshot()
                    .nodes
                    .iter()
                    .any(|node| node.meta.label == "Foreign shared folder")
            );
        }
    }
    fs::remove_file(path).unwrap();
}

#[test]
fn runtime_save_projection_retains_the_application_identity() {
    let mut engine = Engine::<ReplaceOrderTestNode>::new(Folder::new("Saved Root").into());
    prepare_engine_for_runtime(&mut engine).unwrap();
    let runtime = production_runtime(engine);
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let path = std::env::temp_dir().join(format!("golden-application-save-{}-{nonce}.json", std::process::id()));
    runtime
        .save_project(ProjectSaveRequest {
            path: path.to_string_lossy().into_owned(),
            ui_state: None,
        })
        .unwrap();
    let json = fs::read_to_string(&path).unwrap();
    let document: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(document["application_id"], ReplaceOrderTestNode::application_id());
    golden_engine::app::from_sparse_project_json::<ReplaceOrderTestNode>(&json).unwrap();
    fs::remove_file(path).unwrap();
}
