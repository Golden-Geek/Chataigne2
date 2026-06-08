use std::time::Duration;

use golden_core::{
    edit::Edit,
    node::{Folder, Node, NodeId},
    parameter::{ParamValue, ParameterEventBehaviour},
    process_ctx::ExecutionPhase,
};

use super::streamdeck_runtime::SimulatedStreamDeck;
use super::{StreamDeckModule, STREAMDECK_KEY_COUNT};

const SIM_KEYS: usize = STREAMDECK_KEY_COUNT;

#[test]
fn streamdeck_materializes_pageable_key_layout() {
    let (engine, module_id) = create_streamdeck_module();

    assert!(
        find_path(&engine, module_id, "values/keys").is_some(),
        "pageable keys folder should exist"
    );
    for index in 0..SIM_KEYS {
        let key_path = format!("values/keys/key_{index}");
        assert!(
            find_path(&engine, module_id, &key_path).is_some(),
            "declared key folder {key_path} should exist"
        );
        for field in ["Color", "Text", "Image", "Pressed"] {
            assert!(
                find_path(&engine, module_id, &format!("{key_path}/{field}")).is_some(),
                "key {index} should declare the {field} primitive"
            );
        }
    }
}

#[test]
fn keys_folder_is_tagged_pageable() {
    let (engine, module_id) = create_streamdeck_module();
    let keys = find_path(&engine, module_id, "values/keys").expect("keys folder");
    let tags = &engine.nodes.get(keys).expect("keys node").node_data().meta.tags;
    assert!(
        tags.iter().any(|tag| tag == "pageable"),
        "the declared keys folder should carry the reserved `pageable` tag, got {tags:?}"
    );
}

#[test]
fn paging_injects_active_page_selector_defaulting_to_default() {
    let (mut engine, module_id) = create_streamdeck_module();
    install_simulated_device(&mut engine, module_id);
    run_tick(&mut engine);

    let active_page = param_value(&engine, module_id, "parameters/active_page");
    assert_eq!(
        active_page,
        Some(ParamValue::Enum("default".to_string())),
        "an active_page selector should be injected, defaulting to the fixed `default` page"
    );
}

#[test]
fn deriving_a_page_clones_the_declared_layout() {
    let (mut engine, module_id) = create_streamdeck_module();
    install_simulated_device(&mut engine, module_id);
    run_tick(&mut engine);

    create_page(&mut engine, module_id, "Lighting");

    // The derived page mirrors the declared template structure under pages.
    assert!(
        find_path(&engine, module_id, "values/keys/pages/lighting").is_some(),
        "a derived `lighting` page should be created under pages"
    );
    for index in 0..SIM_KEYS {
        let cloned_key = format!("values/keys/pages/lighting/Key {index}/Pressed");
        assert!(
            find_path(&engine, module_id, &cloned_key).is_some(),
            "derived page should clone the {cloned_key} primitive"
        );
    }

    // The selector now offers both pages.
    let active = active_page_param(&engine, module_id);
    let options = enum_option_ids(&engine, active);
    assert!(
        options.contains(&"default".to_string()) && options.contains(&"lighting".to_string()),
        "active_page options should list every page, got {options:?}"
    );
}

#[test]
fn viewport_routes_input_to_the_active_page_only() {
    let (mut engine, module_id) = create_streamdeck_module();
    install_simulated_device(&mut engine, module_id);
    run_tick(&mut engine);
    create_page(&mut engine, module_id, "Lighting");

    // Switch the surface to the derived page, then press physical key 2.
    let active = active_page_param(&engine, module_id);
    set_param(&mut engine, active, ParamValue::Enum("lighting".to_string()));
    run_tick(&mut engine);

    press_key(&mut engine, module_id, 2);
    run_tick(&mut engine);

    assert_eq!(
        bool_param(&engine, module_id, "values/keys/pages/lighting/Key 2/Pressed"),
        Some(true),
        "input should be routed to the active (lighting) page key"
    );
    assert_eq!(
        bool_param(&engine, module_id, "values/keys/Key 2/Pressed"),
        Some(false),
        "the fixed default page must NOT receive input while another page is active"
    );
}

#[test]
fn feedback_pushes_active_page_key_color_to_device() {
    let (mut engine, module_id) = create_streamdeck_module();
    install_simulated_device(&mut engine, module_id);
    run_tick(&mut engine);

    let color_param = find_path(&engine, module_id, "values/keys/Key 1/Color").expect("key 1 color");
    set_param(&mut engine, color_param, ParamValue::Color(1.0, 0.0, 0.0, 1.0));
    run_tick(&mut engine);

    with_module(&engine, module_id, |module| {
        let rendered = module.simulated().rendered(1).expect("key 1 visual");
        assert_eq!(
            rendered.color,
            (1.0, 0.0, 0.0, 1.0),
            "feedback color should be pushed to the device for the active page"
        );
    });
}

#[test]
fn release_clears_pressed_and_brightness_reaches_device() {
    let (mut engine, module_id) = create_streamdeck_module();
    install_simulated_device(&mut engine, module_id);
    run_tick(&mut engine);

    press_key(&mut engine, module_id, 0);
    run_tick(&mut engine);
    assert_eq!(
        bool_param(&engine, module_id, "values/keys/Key 0/Pressed"),
        Some(true),
        "press should set the key activity primitive"
    );

    release_key(&mut engine, module_id, 0);
    run_tick(&mut engine);
    assert_eq!(
        bool_param(&engine, module_id, "values/keys/Key 0/Pressed"),
        Some(false),
        "release should clear the key activity primitive"
    );

    let brightness = find_path(&engine, module_id, "parameters/brightness").expect("brightness parameter");
    set_param(&mut engine, brightness, ParamValue::Int(42));
    run_tick(&mut engine);
    with_module(&engine, module_id, |module| {
        assert_eq!(module.simulated().brightness(), 42, "brightness should propagate to the device");
    });
}

// --- harness helpers --------------------------------------------------------

fn create_streamdeck_module() -> (crate::app::AppEngine, NodeId) {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    let module = StreamDeckModule::create();
    engine.add_node(module.into(), None);
    engine.apply_edits().expect("Stream Deck module should attach");
    for _ in 0..6 {
        engine.apply_edits().expect("Stream Deck defaults should materialize");
    }
    engine.resolve().expect("Stream Deck schedule should resolve");

    let module_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("Stream Deck module should be attached under root");

    (engine, module_id)
}

fn install_simulated_device(engine: &mut crate::app::AppEngine, module_id: NodeId) {
    module_mut(engine, module_id).install_simulated_device(SimulatedStreamDeck::new("sim-1", SIM_KEYS));
}

fn press_key(engine: &mut crate::app::AppEngine, module_id: NodeId, index: usize) {
    module_mut(engine, module_id).simulated_mut().press(index);
}

fn release_key(engine: &mut crate::app::AppEngine, module_id: NodeId, index: usize) {
    module_mut(engine, module_id).simulated_mut().release(index);
}

fn create_page(engine: &mut crate::app::AppEngine, module_id: NodeId, name: &str) {
    let new_page = find_path(engine, module_id, "parameters/new_page").expect("new_page parameter");
    set_param(engine, new_page, ParamValue::Str(name.to_string()));
    run_tick(engine);
    run_tick(engine);
}

fn module_mut<'a>(engine: &'a mut crate::app::AppEngine, module_id: NodeId) -> &'a mut StreamDeckModule {
    match engine.nodes.get_mut(module_id).expect("Stream Deck module should exist") {
        crate::app::AppNode::StreamDeckModule(module) => module,
        _ => panic!("expected StreamDeckModule node"),
    }
}

fn with_module<R>(engine: &crate::app::AppEngine, module_id: NodeId, f: impl FnOnce(&StreamDeckModule) -> R) -> R {
    match engine.nodes.get(module_id).expect("Stream Deck module should exist") {
        crate::app::AppNode::StreamDeckModule(module) => f(module),
        _ => panic!("expected StreamDeckModule node"),
    }
}

fn run_tick(engine: &mut crate::app::AppEngine) {
    engine
        .dispatch_inbox(ExecutionPhase::EngineTick)
        .expect("inbox should dispatch");
    engine.run_tick(Duration::from_millis(20)).expect("tick should run");
    engine.apply_edits().expect("tick edits should apply");
}

fn active_page_param(engine: &crate::app::AppEngine, module_id: NodeId) -> NodeId {
    find_path(engine, module_id, "parameters/active_page").expect("active_page parameter")
}

fn enum_option_ids(engine: &crate::app::AppEngine, param_id: NodeId) -> Vec<String> {
    engine
        .nodes
        .get(param_id)
        .and_then(|node| node.engine_param_snapshot())
        .map(|snapshot| {
            snapshot
                .constraints
                .enum_options
                .iter()
                .map(|option| option.variant_id.clone())
                .collect()
        })
        .unwrap_or_default()
}

fn set_param(engine: &mut crate::app::AppEngine, node: NodeId, value: ParamValue) {
    engine.edits.push(Edit::SetParam {
        node,
        value,
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("set param should apply");
}

fn find_path(engine: &crate::app::AppEngine, start: NodeId, path: &str) -> Option<NodeId> {
    let mut current = start;
    for segment in path.split('/') {
        if segment.trim().is_empty() {
            continue;
        }
        current = find_child_by_key(engine, current, segment)?;
    }
    Some(current)
}

fn find_child_by_key(engine: &crate::app::AppEngine, parent: NodeId, key: &str) -> Option<NodeId> {
    let mut current = engine.nodes.get(parent)?.node_data().first_child;
    while let Some(child_id) = current {
        let child = engine.nodes.get(child_id)?;
        if node_key_matches(child.node_data(), key) {
            return Some(child_id);
        }
        current = child.node_data().next_sibling;
    }
    None
}

fn node_key_matches(node: &golden_core::node::NodeData, key: &str) -> bool {
    node.meta.decl_id.0 == key
        || node.meta.decl_id.0.rsplit('/').next() == Some(key)
        || node.meta.short_name == key
        || node.meta.label == key
}

fn param_value(engine: &crate::app::AppEngine, start: NodeId, path: &str) -> Option<ParamValue> {
    let param_id = find_path(engine, start, path)?;
    engine
        .nodes
        .get(param_id)?
        .engine_param_snapshot()
        .map(|snapshot| snapshot.value)
}

fn bool_param(engine: &crate::app::AppEngine, start: NodeId, path: &str) -> Option<bool> {
    match param_value(engine, start, path)? {
        ParamValue::Bool(value) => Some(value),
        _ => None,
    }
}
