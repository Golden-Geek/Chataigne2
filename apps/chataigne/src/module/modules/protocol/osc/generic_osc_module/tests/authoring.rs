use super::*;

#[test]
fn osc_module_declares_compiled_runtime_kernel() {
    assert_eq!(
        GenericOscModule::create().execution_rule().compiled_kernel_key,
        Some("chataigne.runtime.osc")
    );
}

#[test]
fn osc_module_script_descriptor_advertises_message_send_method() {
    let descriptor = GenericOscModule::create().engine_script_descriptor();

    assert!(
        descriptor.methods.iter().any(|candidate| candidate == "sendMessage"),
        "osc script descriptor should advertise 'sendMessage'"
    );
}

#[test]
fn osc_module_script_template_scaffolds_osc_callbacks_only() {
    let osc_module_type = <GenericOscModule as golden_core::node::DeclaredUserItemNode>::ITEM_NODE_TYPE;
    let config = crate::app::module::script_api::module_script_config(osc_module_type);
    let ScriptSource::Inline { text: source } = config.source else {
        panic!("osc module script template should resolve to inline source");
    };

    assert!(
        source.contains("function messageReceived"),
        "osc module template should include OSC callbacks; node_type={}, source={source}",
        osc_module_type,
    );
    assert!(source.contains("// Golden Core script template."));
    assert!(source.contains("function init()"));
    assert!(!source.contains("function noteOnReceived"));
    assert!(!source.contains("function clientConnected"));
}

#[test]
fn creating_script_under_osc_module_scaffolds_osc_callbacks() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(GenericOscModule::create().into(), None);
    engine.apply_edits().expect("osc module should attach");
    for _ in 0..4 {
        engine.apply_edits().expect("osc defaults should materialize");
    }

    let module_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("module should be attached under root");
    let module = engine.nodes.get(module_id).expect("module should exist");
    let module_type = module.get_type().to_string();
    let module_decl_id = module.node_data().meta.decl_id.0.clone();

    let create_ack = engine.apply_ui_intent(UiEditIntent::CreateUserItem {
        parent: module_id,
        node_type: "script".to_string(),
        label: Some("Script".to_string()),
        initial_params: Vec::new(),
    });
    assert!(
        create_ack.success,
        "creating a script under an OSC module should succeed"
    );

    let script_id = find_child_by_key(&engine, module_id, "Script").unwrap_or_else(|| {
        let children = child_labels(&engine, module_id);
        panic!("created OSC module should expose a Script child; children were {children:?}");
    });
    let script_state = engine
        .ui_script_state(script_id)
        .expect("created script should expose UI script state");
    let ScriptUiSource::Inline { text } = script_state.config.source else {
        panic!("created OSC module script should use inline source");
    };

    assert!(
        text.contains("function messageReceived"),
        "created OSC module script should scaffold OSC callbacks; module_type={module_type}, module_decl_id={module_decl_id}, source={text}"
    );
    assert!(text.contains("// Golden Core script template."));
    assert!(text.contains("function init()"));
    assert!(
        !text.contains("function noteOnReceived"),
        "created OSC module script should not scaffold MIDI callbacks; module_type={module_type}, module_decl_id={module_decl_id}, source={text}"
    );
    assert!(
        !text.contains("function clientConnected"),
        "created OSC module script should not scaffold server stream callbacks; module_type={module_type}, module_decl_id={module_decl_id}, source={text}"
    );
}

#[test]
fn new_module_command_tester_starts_empty() {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(GenericOscModule::create().into(), None);
    engine.apply_edits().expect("osc module should attach");
    for _ in 0..4 {
        engine.apply_edits().expect("osc defaults should materialize");
    }

    let module_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("module should be attached under root");
    let command_tester_id = find_path(&engine, module_id, "command_tester").expect("command tester should exist");

    assert!(
        engine
            .nodes
            .get(command_tester_id)
            .and_then(|node| node.node_data().first_child)
            .is_none(),
        "new OSC modules should not seed command tester commands"
    );
}
