use serde_json::json;

use crate::parameter::ParamValue;

use super::{DeclId, NodeData, NodeScriptDescriptor, PresentationHint};

#[test]
fn presentation_hint_defaults_to_nested_inspector_visibility() {
    assert!(PresentationHint::default().show_in_nested_inspector);
}

#[test]
fn presentation_hint_serializes_nested_inspector_flag_when_disabled() {
    let hint = PresentationHint {
        show_in_nested_inspector: false,
        ..Default::default()
    };

    let serialized = serde_json::to_value(hint).expect("presentation hint should serialize");

    assert_eq!(
        serialized,
        json!({
            "show_in_nested_inspector": false
        })
    );
}

#[test]
fn presentation_hint_serializes_inspector_content_flag_when_disabled() {
    let hint = PresentationHint {
        show_in_inspector_content: false,
        ..Default::default()
    };

    let serialized = serde_json::to_value(hint).expect("presentation hint should serialize");

    assert_eq!(
        serialized,
        json!({
            "show_in_inspector_content": false
        })
    );
}

#[test]
fn presentation_hint_omits_default_nested_inspector_visibility() {
    let serialized = serde_json::to_value(PresentationHint::default()).expect("presentation hint should serialize");

    assert_eq!(serialized, json!({}));
}

#[test]
fn presentation_hint_serializes_collapsed_when_enabled() {
    let hint = PresentationHint {
        collapsed: true,
        ..Default::default()
    };

    let serialized = serde_json::to_value(hint).expect("presentation hint should serialize");

    assert_eq!(
        serialized,
        json!({
            "collapsed": true
        })
    );
}

#[test]
fn node_script_descriptor_for_node_exposes_standard_proxy_surface() {
    let mut node_data = NodeData::new("Test Node".to_string());
    node_data.meta.enabled = false;
    node_data.meta.decl_id = DeclId("test_decl".to_string());

    let descriptor = NodeScriptDescriptor::for_node(&node_data, "test_type");

    assert_eq!(
        descriptor.properties.get("name"),
        Some(&ParamValue::Str("Test Node".to_string()))
    );
    assert_eq!(descriptor.properties.get("enabled"), Some(&ParamValue::Bool(false)));
    assert_eq!(
        descriptor.properties.get("type"),
        Some(&ParamValue::Str("test_type".to_string()))
    );
    assert_eq!(
        descriptor.properties.get("declId"),
        Some(&ParamValue::Str("test_decl".to_string()))
    );
    assert!(descriptor.methods.iter().any(|method| method == "setParam"));
    assert!(descriptor.methods.iter().any(|method| method == "getChild"));
}

#[test]
fn node_script_descriptor_add_methods_keeps_unique_method_names() {
    let mut descriptor = NodeScriptDescriptor::default();

    descriptor.add_methods(["sendText", "sendText", "sendBytes"]);

    assert_eq!(
        descriptor.methods,
        vec!["sendText".to_string(), "sendBytes".to_string()]
    );
}
