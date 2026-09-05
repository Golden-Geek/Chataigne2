use std::time::Duration;

use golden_core::{
    edit::Edit,
    node::{Folder, Node, NodeId, NodeMetaPatch},
    parameter::{ParamValue, ParameterEventBehaviour},
    process_ctx::ExecutionPhase,
    script::ScriptSource,
};

use super::streamdeck_runtime::SimulatedStreamDeck;
use super::StreamDeckModule;

/// "mini" model => 6 keys.
const MODEL_KEYS: usize = 6;

#[test]
fn streamdeck_script_template_scaffolds_functions_and_callbacks() {
    let config =
        crate::app::module::script_api::module_script_config(StreamDeckModule::NODE_TYPE);
    let ScriptSource::Inline { text: source } = config.source else {
        panic!("Stream Deck module script template should resolve to inline source");
    };

    for expected in [
        "local.addPage",
        "local.removePage",
        "local.pressKey",
        "local.releaseKey",
        "function streamDeckKeyPressed",
        "function streamDeckKeyReleased",
        "function streamDeckPageChanged",
    ] {
        assert!(
            source.contains(expected),
            "Stream Deck script template is missing `{expected}`"
        );
    }
}

#[test]
fn generates_one_based_control_and_flat_inputs() {
    let (engine, module_id) = create_streamdeck_module();

    for number in 1..=MODEL_KEYS {
        for field in ["Color", "Text", "Image", "Unpaged"] {
            assert!(
                find_path(&engine, module_id, &format!("parameters/keys/Key {number}/{field}")).is_some(),
                "control key {number} should expose {field}"
            );
        }
        assert_eq!(
            bool_param(&engine, module_id, &format!("values/keys/Key {number}")),
            Some(false),
            "values/keys/Key {number} should be a flat input boolean"
        );
    }
    assert!(
        find_path(&engine, module_id, "parameters/keys/Key 0").is_none(),
        "keys must be 1-based"
    );
}

#[test]
fn changing_model_resizes_every_key_collection() {
    let (mut engine, module_id) = create_streamdeck_module();
    set_model(&mut engine, module_id, "pedal"); // 3 keys

    assert!(find_path(&engine, module_id, "parameters/keys/Key 3").is_some());
    assert!(
        find_path(&engine, module_id, "parameters/keys/Key 4").is_none(),
        "shrinking the model should remove surplus control keys"
    );
    assert!(find_path(&engine, module_id, "values/keys/Key 4").is_none());
}

#[test]
fn paging_injects_active_page_selector() {
    let (engine, module_id) = create_streamdeck_module();
    assert_eq!(
        param_value(&engine, module_id, "parameters/active_page"),
        Some(ParamValue::Enum("default".to_string())),
        "an active_page selector should default to the fixed page"
    );
}

#[test]
fn deriving_a_page_creates_control_page_and_values_mirror() {
    let (mut engine, module_id) = create_streamdeck_module();
    create_page(&mut engine, module_id, "Lighting");

    // Control page (beside `keys`, not inside it) cloned from the default layout.
    assert!(
        find_path(&engine, module_id, "parameters/keys/pages").is_none(),
        "pages must live beside keys, not inside them"
    );
    for number in 1..=MODEL_KEYS {
        assert!(
            find_path(&engine, module_id, &format!("parameters/pages/lighting/Key {number}/Color")).is_some(),
            "control page should clone key {number}"
        );
        // Values mirror: a flat boolean per key, named after the page.
        assert_eq!(
            bool_param(&engine, module_id, &format!("values/pages/lighting/Key {number}")),
            Some(false),
            "values mirror should expose key {number}"
        );
    }
    let options = enum_option_ids(&engine, active_page_param(&engine, module_id));
    assert!(options.contains(&"default".to_string()) && options.contains(&"lighting".to_string()));
}

#[test]
fn input_routes_to_the_active_pages_values() {
    let (mut engine, module_id) = create_streamdeck_module();
    create_page(&mut engine, module_id, "Lighting");
    let active = active_page_param(&engine, module_id);
    set_param(&mut engine, active, ParamValue::Enum("lighting".to_string()));
    run_tick(&mut engine);

    press_key(&mut engine, module_id, 2); // physical slot 2 => 1-based "Key 3"
    run_tick(&mut engine);

    assert_eq!(
        bool_param(&engine, module_id, "values/pages/lighting/Key 3"),
        Some(true),
        "input should land on the active page's values mirror"
    );
    assert_eq!(
        bool_param(&engine, module_id, "values/keys/Key 3"),
        Some(false),
        "the default page inputs should be untouched while another page is active"
    );
}

#[test]
fn feedback_pushes_active_page_color_to_device() {
    let (mut engine, module_id) = create_streamdeck_module();
    let color = find_path(&engine, module_id, "parameters/keys/Key 1/Color").expect("key 1 color");
    set_param(&mut engine, color, ParamValue::Color(1.0, 0.0, 0.0, 1.0));
    run_tick(&mut engine);
    with_module(&engine, module_id, |module| {
        assert_eq!(module.simulated().rendered(0).expect("slot 0 visual").color, (1.0, 0.0, 0.0, 1.0));
    });
}

#[test]
fn unpaged_key_always_shows_the_default_page_appearance() {
    let (mut engine, module_id) = create_streamdeck_module();
    create_page(&mut engine, module_id, "Lighting");

    // `Unpaged` lives on the default key only and is not cloned into derived pages.
    assert!(
        find_path(&engine, module_id, "parameters/pages/lighting/Key 1/Unpaged").is_none(),
        "derived pages must not carry the default-only Unpaged flag"
    );

    let default_color = find_path(&engine, module_id, "parameters/keys/Key 1/Color").expect("default color");
    set_param(&mut engine, default_color, ParamValue::Color(1.0, 0.0, 0.0, 1.0));
    let page_color = find_path(&engine, module_id, "parameters/pages/lighting/Key 1/Color").expect("page color");
    set_param(&mut engine, page_color, ParamValue::Color(0.0, 0.0, 1.0, 1.0));
    // Mark the DEFAULT key un-paged: the slot should keep the default appearance on every page.
    let default_unpaged = find_path(&engine, module_id, "parameters/keys/Key 1/Unpaged").expect("default unpaged");
    set_param(&mut engine, default_unpaged, ParamValue::Bool(true));
    let active = active_page_param(&engine, module_id);
    set_param(&mut engine, active, ParamValue::Enum("lighting".to_string()));
    run_tick(&mut engine);

    with_module(&engine, module_id, |module| {
        assert_eq!(
            module.simulated().rendered(0).expect("slot 0 visual").color,
            (1.0, 0.0, 0.0, 1.0),
            "an un-paged key keeps the default-page appearance"
        );
    });
}

#[test]
fn changing_model_adapts_pages_instead_of_resetting_them() {
    let (mut engine, module_id) = create_streamdeck_module();
    create_page(&mut engine, module_id, "Lighting");
    set_model(&mut engine, module_id, "pedal"); // 3 keys

    assert!(
        find_path(&engine, module_id, "parameters/pages/lighting/Key 3").is_some(),
        "the page should survive a model change"
    );
    assert!(
        find_path(&engine, module_id, "parameters/pages/lighting/Key 4").is_none(),
        "the page should shrink with the model"
    );
    assert!(find_path(&engine, module_id, "values/pages/lighting/Key 3").is_some());
}

#[test]
fn values_pages_mirror_is_removed_when_the_last_page_is_deleted() {
    let (mut engine, module_id) = create_streamdeck_module();
    create_page(&mut engine, module_id, "Lighting");
    assert!(find_path(&engine, module_id, "values/pages").is_some());

    let page = find_path(&engine, module_id, "parameters/pages/lighting").expect("control page");
    engine.edits.push(Edit::RemoveNode { node: page });
    engine.apply_edits().expect("remove page");
    run_tick(&mut engine);
    run_tick(&mut engine);

    assert!(
        find_path(&engine, module_id, "values/pages").is_none(),
        "the values mirror should disappear when no page remains"
    );
}

#[test]
fn renaming_a_control_page_syncs_the_values_mirror() {
    let (mut engine, module_id) = create_streamdeck_module();
    create_page(&mut engine, module_id, "Lighting");

    let page = find_path(&engine, module_id, "parameters/pages/lighting").expect("control page");
    rename_node(&mut engine, page, "FX");
    run_tick(&mut engine);
    run_tick(&mut engine);

    let mirror = find_path(&engine, module_id, "values/pages/lighting").expect("values mirror page");
    assert_eq!(
        engine.nodes.get(mirror).unwrap().node_data().meta.label,
        "FX",
        "the values mirror label should follow the control page rename (id stays stable)"
    );
}

#[test]
fn brightness_reaches_device() {
    let (mut engine, module_id) = create_streamdeck_module();
    // Brightness is a 0..1 float; the device receives it as a 0..100 percent.
    let brightness = find_path(&engine, module_id, "parameters/brightness").expect("brightness");
    set_param(&mut engine, brightness, ParamValue::Float(0.42));
    run_tick(&mut engine);
    with_module(&engine, module_id, |module| {
        assert_eq!(module.simulated().brightness(), 42);
    });
}

// --- harness helpers --------------------------------------------------------

fn create_streamdeck_module() -> (crate::app::AppEngine, NodeId) {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(StreamDeckModule::create().into(), None);
    engine.apply_edits().expect("Stream Deck module should attach");
    for _ in 0..6 {
        engine.apply_edits().expect("defaults should materialize");
    }
    engine.resolve().expect("schedule should resolve");

    let module_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("module should be attached");

    module_mut(&mut engine, module_id).install_simulated_device(SimulatedStreamDeck::new("sim-1", MODEL_KEYS));
    set_model(&mut engine, module_id, "mini");
    (engine, module_id)
}

fn set_model(engine: &mut crate::app::AppEngine, module_id: NodeId, model: &str) {
    let model_param = find_path(engine, module_id, "parameters/model").expect("model parameter");
    set_param(engine, model_param, ParamValue::Enum(model.to_string()));
    for _ in 0..3 {
        run_tick(engine);
    }
}

fn press_key(engine: &mut crate::app::AppEngine, module_id: NodeId, index: usize) {
    module_mut(engine, module_id).simulated_mut().press(index);
}

fn rename_node(engine: &mut crate::app::AppEngine, node: NodeId, label: &str) {
    engine.edits.push(Edit::PatchMeta {
        node,
        patch: NodeMetaPatch {
            label: Some(label.to_string()),
            ..Default::default()
        },
    });
    engine.apply_edits().expect("rename should apply");
}

fn create_page(engine: &mut crate::app::AppEngine, module_id: NodeId, name: &str) {
    run_tick(engine); // ensure the PageHost exists
    let container = find_path(engine, module_id, "parameters/pages").expect("pages container");
    engine
        .queue_catalog_create(container, "folder", Some(name.to_string()), None)
        .expect("create page");
    engine.apply_edits().expect("apply page create");
    for _ in 0..3 {
        run_tick(engine);
    }
}

fn module_mut(engine: &mut crate::app::AppEngine, module_id: NodeId) -> &mut StreamDeckModule {
    match engine.nodes.get_mut(module_id).expect("module should exist") {
        crate::app::AppNode::StreamDeckModule(module) => module,
        _ => panic!("expected StreamDeckModule node"),
    }
}

fn with_module<R>(engine: &crate::app::AppEngine, module_id: NodeId, f: impl FnOnce(&StreamDeckModule) -> R) -> R {
    match engine.nodes.get(module_id).expect("module should exist") {
        crate::app::AppNode::StreamDeckModule(module) => f(module),
        _ => panic!("expected StreamDeckModule node"),
    }
}

fn run_tick(engine: &mut crate::app::AppEngine) {
    engine.dispatch_inbox(ExecutionPhase::EngineTick).expect("inbox should dispatch");
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
        .map(|snapshot| snapshot.constraints.enum_options.iter().map(|option| option.variant_id.clone()).collect())
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
    engine.nodes.get(param_id)?.engine_param_snapshot().map(|snapshot| snapshot.value)
}

fn bool_param(engine: &crate::app::AppEngine, start: NodeId, path: &str) -> Option<bool> {
    match param_value(engine, start, path)? {
        ParamValue::Bool(value) => Some(value),
        _ => None,
    }
}
