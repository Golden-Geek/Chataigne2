
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::json;

use super::ScriptEvent;

#[test]
fn custom_event_payload_can_target_named_script_callback() {
    let event = ScriptEvent {
        kind: "custom".to_string(),
        origin: None,
        old_value: None,
        payload: json!({
            "Custom": {
                "topic": "chataigne.module.script.callback",
                "payload": {
                    "callback": "messageReceived",
                    "args": ["/address", [1, 2, 3]]
                }
            }
        }),
    };

    let invocation = event
        .custom_callback_invocation()
        .expect("custom module callback payload should decode");

    assert_eq!(invocation.name, "messageReceived");
    assert_eq!(invocation.args, vec![json!("/address"), json!([1, 2, 3])]);
}

#[test]
fn custom_event_payload_ignores_empty_callback_name() {
    let event = ScriptEvent {
        kind: "custom".to_string(),
        origin: None,
        old_value: None,
        payload: json!({
            "Custom": {
                "payload": {
                    "callback": "  ",
                    "args": ["ignored"]
                }
            }
        }),
    };

    assert!(event.custom_callback_invocation().is_none());
}

#[test]
fn callback_node_argument_marker_decodes_node_id() {
    assert_eq!(
        super::QuickJsRuntime::script_callback_arg_node_id(&json!({
            "kind": "node",
            "id": 42
        })),
        Some(42)
    );
    assert_eq!(
        super::QuickJsRuntime::script_callback_arg_node_id(&json!({
            "kind": "value",
            "id": 42
        })),
        None
    );
}

#[test]
fn custom_template_can_include_core_default_template_by_namespace() {
    let root = create_temp_template_root("core-include");
    let template_path = root.join("custom.js");
    fs::write(&template_path, "{{include:core/default.js}}\n").expect("custom template should be written");

    let source = super::read_template_from_path(&template_path, &root)
        .expect("namespaced core include should resolve from Golden Core root");

    assert!(source.contains("// Default script template for Golden Core script nodes."));
    assert!(source.contains("function init()"));

    remove_temp_template_root(&root);
}

#[test]
fn custom_template_does_not_fall_back_to_core_without_namespace() {
    let root = create_temp_template_root("local-only-include");
    let template_path = root.join("custom.js");
    fs::write(&template_path, "{{include:snippets/header.js}}\n").expect("custom template should be written");

    let error = super::read_template_from_path(&template_path, &root)
        .expect_err("plain includes should stay scoped to the current template root");

    assert!(error.contains("snippets/header.js"));
    assert!(error.contains(&root.join("snippets/header.js").display().to_string()));

    remove_temp_template_root(&root);
}

fn create_temp_template_root(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("golden-script-template-tests-{label}-{unique}"));
    fs::create_dir_all(&root).expect("temp template root should be created");
    root
}

fn remove_temp_template_root(root: &PathBuf) {
    let _ = fs::remove_dir_all(root);
}
