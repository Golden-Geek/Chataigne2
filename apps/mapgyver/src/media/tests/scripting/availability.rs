use super::*;

#[test]
fn hosted_method_rejects_removed_standalone_application_operations_without_history() {
    for kind in [
        "Create",
        "Remove",
        "Duplicate",
        "Rename",
        "SetEnabled",
        "InspectCatalog",
        "InspectReferences",
    ] {
        let mut engine = fixture();
        let request = serde_json::json!({"kind":"Applications", "edit":{"kind":kind}}).to_string();
        let history = engine.undo_len();
        let before = to_sparse_project_json_pretty(&engine).unwrap();
        let node = manager(&engine).unwrap();
        engine.edits.push(Edit::CallNodeScriptMethod {
            node,
            call: ScriptMethodCall::prepare(
                "editMedia",
                vec![ParamValue::Str("obsolete".into()), ParamValue::Str(request)],
            )
            .unwrap(),
        });
        let error = engine.apply_edits().unwrap_err().to_string();
        assert!(error.contains("unknown variant"), "{kind}: {error}");
        assert_eq!(engine.undo_len(), history);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
        assert!(replies(&engine).is_empty());
    }
}
