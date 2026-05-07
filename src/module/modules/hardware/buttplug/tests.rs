use golden_core::{
    node::DeclaredUserItemNode,
    script::ScriptSource,
};

use super::{buttplug_safety_warnings, normalize_buttplug_path, ButtplugModule, BUTTPLUG_SAFETY_MANIFEST_URL};

#[test]
fn buttplug_module_safety_warning_links_to_manifest() {
    let warnings = buttplug_safety_warnings();

    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].message.contains("Safety warning"));
    assert!(warnings[0].message.contains("Buttplug"));
    assert!(
        warnings[0]
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains(BUTTPLUG_SAFETY_MANIFEST_URL)),
        "safety warning should include the Buttplug manifest link"
    );
}

#[test]
fn buttplug_module_is_a_module_item() {
    assert_eq!(
        <ButtplugModule as DeclaredUserItemNode>::ITEM_KIND,
        crate::app::module::MODULE_ITEM_KIND
    );
    assert!(crate::app::declared_user_item_type_matches(
        ButtplugModule::NODE_TYPE,
        crate::app::module::MODULE_ITEM_KIND
    ));
}

#[test]
fn buttplug_script_template_scaffolds_buttplug_functions_and_callbacks() {
    let config = crate::app::module::script_api::module_script_config(ButtplugModule::NODE_TYPE);
    let ScriptSource::Inline(source) = config.source else {
        panic!("Buttplug module script template should resolve to inline source");
    };

    assert!(source.contains("local.vibrate(value, device = \"selected\")"));
    assert!(source.contains("local.setOutput(output, value"));
    assert!(source.contains("function buttplugDeviceAdded"));
    assert!(!source.contains("function noteOnReceived"));
}

#[test]
fn buttplug_path_is_normalized_for_websocket_url() {
    assert_eq!(normalize_buttplug_path(""), "/");
    assert_eq!(normalize_buttplug_path("ws"), "/ws");
    assert_eq!(normalize_buttplug_path("/"), "/");
}
