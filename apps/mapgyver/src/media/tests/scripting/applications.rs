use super::*;

#[test]
fn invalid_named_app_arguments_never_enqueue_partial_operations_or_author_history() {
    let mut engine = fixture();
    let manager = manager(&engine).unwrap();
    let nil = golden_core::node::NodeUuid::nil().0.to_string();
    for (method, values) in [
        ("setApplicationFile", vec!["request", "not-an-owner-id", ""]),
        ("configureApplication", vec!["request", &nil, "{"]),
        ("launchMatrixCell", vec!["request", &nil, &nil, &nil, "Unsupported"]),
        ("inspectApplicationRuntime", vec!["request", "unexpected"]),
    ] {
        let before = to_sparse_project_json_pretty(&engine).unwrap();
        let history = engine.undo_len();
        engine.edits.push(Edit::CallNodeScriptMethod {
            node: manager,
            call: ScriptMethodCall::prepare(
                method,
                values.into_iter().map(|value| ParamValue::Str(value.into())).collect(),
            )
            .unwrap(),
        });
        assert!(engine.apply_edits().is_err(), "{method}");
        service_script_app_operations(&mut engine).unwrap();
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
        assert_eq!(engine.undo_len(), history);
    }
}
