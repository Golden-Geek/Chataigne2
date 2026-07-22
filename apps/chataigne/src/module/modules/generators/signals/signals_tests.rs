use golden_core::{
    edit::Edit,
    node::{Folder, Node, NodeId, NodeMetaPatch},
    parameter::{
        ParamValue, Parameter, ParameterChangeCheck, ParameterEventBehaviour, RangeConstraint,
    },
    process_ctx::ExecutionPhase,
    script::ScriptSource,
};

use super::SignalsModule;
use std::collections::HashMap;

#[test]
fn signals_module_is_declared_under_generators_menu() {
    let item = crate::app::declared_user_creatable_items(crate::app::module::MODULE_ITEM_KIND)
        .into_iter()
        .find(|item| item.node_type == SignalsModule::NODE_TYPE)
        .expect("Signals module should be exposed in the module catalog");

    assert_eq!(item.label, "Signals");
    assert_eq!(item.menu_path, vec!["Generators".to_string()]);
}

#[test]
fn signals_script_descriptor_advertises_reset_methods() {
    let descriptor = SignalsModule::create().engine_script_descriptor();

    for method in ["resetSignals", "resetSignal"] {
        assert!(descriptor.methods.iter().any(|candidate| candidate == method));
    }
}

#[test]
fn signals_create_default_sine_item_and_direct_ranged_value() {
    let (engine, module_id) = create_signals_module();

    let signals = find_path(&engine, module_id, "parameters/signals").expect("signals list");
    assert!(
        !engine
            .nodes
            .get(signals)
            .expect("signals list node")
            .node_data()
            .meta
            .can_be_disabled,
        "Signal manager should not be disableable"
    );
    let default_signal = nth_child(&engine, signals, 0).expect("default signal");
    assert!(
        engine
            .nodes
            .get(default_signal)
            .expect("default signal node")
            .node_data()
            .meta
            .can_be_disabled,
        "Signal items should remain disableable"
    );
    assert!(
        find_child_by_key(&engine, default_signal, "enabled").is_none(),
        "Signal items should use node enablement, not a duplicated Enabled parameter"
    );
    let shape = find_child_by_key(&engine, default_signal, "shape").expect("shape");
    assert!(matches!(
        param_value(&engine, shape),
        Some(ParamValue::Enum(value)) if value == "sine"
    ));

    let values_root = find_path(&engine, module_id, "values").expect("values root");
    assert!(
        find_child_by_key(&engine, values_root, "signal_values").is_none(),
        "signal values should be direct value params, not wrapped in a Signals folder"
    );
    let signal_value =
        find_child_by_key(&engine, values_root, "Signal").expect("default signal value");
    assert_eq!(
        count_children_by_key(&engine, values_root, "Signal"),
        1,
        "creating a Signals module should materialize exactly one default signal value"
    );
    let signal_snapshot = engine
        .nodes
        .get(signal_value)
        .and_then(|node| node.engine_param_snapshot())
        .expect("signal output should be a parameter");
    assert!(matches!(signal_snapshot.value, ParamValue::Float(_)));
    assert_eq!(
        signal_snapshot.constraints.range,
        RangeConstraint::uniform(Some(0.0), Some(1.0))
    );
    assert!(find_child_by_key(&engine, values_root, "phase").is_none());
    assert!(find_child_by_key(&engine, values_root, "cycle").is_none());
}

#[test]
fn sparse_reload_preserves_referenced_default_signal_value() {
    let (mut engine, module_id) = create_signals_module();
    let values_root = find_path(&engine, module_id, "values").expect("values root");
    let signal_value =
        find_child_by_key(&engine, values_root, "Signal").expect("default signal value");
    let signal_value_uuid = engine
        .nodes
        .get(signal_value)
        .expect("signal value should exist")
        .node_data()
        .meta
        .uuid;

    engine.add_node(
        Parameter::new(
            "Signal Reference",
            ParamValue::from(signal_value_uuid),
            ParameterChangeCheck::ValueChange,
        )
        .into(),
        None,
    );
    stabilize(&mut engine);

    let json = golden_core::app::to_sparse_project_json_pretty(&engine)
        .expect("sparse project should encode");
    let mut loaded = golden_core::app::from_sparse_project_json::<crate::app::AppNode>(&json)
        .expect("sparse project should decode");
    stabilize(&mut loaded);

    let loaded_module = loaded
        .nodes
        .iter()
        .find(|(_, node)| node.get_type() == SignalsModule::NODE_TYPE)
        .map(|(id, _)| id)
        .expect("Signals module should reload");
    let loaded_values_root = find_path(&loaded, loaded_module, "values").expect("values root");
    let loaded_signal_value =
        find_child_by_key(&loaded, loaded_values_root, "Signal").expect("signal value");

    assert_eq!(
        loaded
            .nodes
            .get(loaded_signal_value)
            .expect("loaded signal value should exist")
            .node_data()
            .meta
            .uuid,
        signal_value_uuid,
        "referenced generated signal value should keep its persisted UUID"
    );
    let loaded_reference =
        find_child_by_key(&loaded, loaded.root, "Signal Reference").expect("reference parameter");
    assert!(
        matches!(
            param_value(&loaded, loaded_reference),
            Some(ParamValue::Reference(reference)) if reference.uuid() == signal_value_uuid
        ),
        "persisted references should still resolve to the generated signal value"
    );
}

#[test]
fn signal_config_uses_recursive_node_enablement() {
    let (mut engine, module_id) = create_signals_module();
    let signals = find_path(&engine, module_id, "parameters/signals").expect("signals list");
    let signal = nth_child(&engine, signals, 0).expect("default signal");

    patch_enabled(&mut engine, signal, false);
    stabilize(&mut engine);
    let snapshot = engine.process_tree_snapshot();
    let config = super::signal_config(snapshot.as_ref(), signal).expect("signal config");
    assert!(!config.enabled, "disabled signal item should not be processed");

    patch_enabled(&mut engine, signal, true);
    patch_enabled(&mut engine, module_id, false);
    stabilize(&mut engine);
    let snapshot = engine.process_tree_snapshot();
    let config = super::signal_config(snapshot.as_ref(), signal).expect("signal config");
    assert!(
        !config.enabled,
        "disabled module ancestor should disable signal processing"
    );
}

#[test]
fn signal_shape_dependencies_materialize_selected_parameters() {
    let (mut engine, module_id) = create_signals_module();
    let signals = find_path(&engine, module_id, "parameters/signals").expect("signals list");
    let signal = nth_child(&engine, signals, 0).expect("default signal");

    assert!(find_child_by_key(&engine, signal, "seed").is_none());
    assert!(find_child_by_key(&engine, signal, "curve").is_none());

    let shape = find_child_by_key(&engine, signal, "shape").expect("shape");
    set_param(&mut engine, shape, ParamValue::Enum("curve".to_string()));
    stabilize(&mut engine);
    assert!(find_child_by_key(&engine, signal, "seed").is_none());
    assert!(find_child_by_key(&engine, signal, "curve").is_some());
    assert_eq!(count_children_by_key(&engine, signal, "curve"), 1);

    set_param(&mut engine, shape, ParamValue::Enum("randomPure".to_string()));
    stabilize(&mut engine);
    assert!(find_child_by_key(&engine, signal, "seed").is_some());
    assert!(find_child_by_key(&engine, signal, "curve").is_none());
}

#[test]
fn sine_signal_generates_continuous_value_in_range() {
    let config = test_signal_config(super::SignalShape::Sine, -1.0, 1.0);
    let mut state = super::SignalRuntimeState {
        elapsed_seconds: 0.25,
        ..Default::default()
    };
    let value = super::sample_signal(&config, &mut state).value;
    assert!(
        (value - 1.0).abs() < 0.000_001,
        "quarter-cycle sine should hit the top of the default range, got {value}",
    );

    state.elapsed_seconds = 0.5;
    let value = super::sample_signal(&config, &mut state).value;
    assert!(
        value.abs() < 0.000_001,
        "half-cycle sine should return to the middle of the default range, got {value}",
    );
}

#[test]
fn signal_range_and_reverse_saw_shape_are_applied() {
    let config = test_signal_config(super::SignalShape::ReverseSaw, 10.0, 20.0);
    let mut state = super::SignalRuntimeState {
        elapsed_seconds: 0.25,
        ..Default::default()
    };
    let value = super::sample_signal(&config, &mut state).value;
    assert!(
        (value - 17.5).abs() < 0.000_001,
        "reverse saw at quarter cycle over 10..20 should be 17.5, got {value}",
    );
}

#[test]
fn signal_worker_fixture_is_deterministic_across_cycles() {
    let config = test_signal_config(super::SignalShape::Sine, -1.0, 1.0);
    let worker_config = super::runtime::SignalWorkerConfig {
        update_rate_hz: 60,
        signals: vec![config],
    };
    let mut states = HashMap::new();

    let first = super::runtime::compute_signal_update(&worker_config, &mut states, 0.25);
    let first = first.samples.get(&NodeId(1)).expect("first signal sample");
    assert!((first.value - 1.0).abs() < 0.000_001);
    assert_eq!((first.cycle, first.cycles), (0, 0));

    let second = super::runtime::compute_signal_update(&worker_config, &mut states, 1.0);
    let second = second.samples.get(&NodeId(1)).expect("second signal sample");
    assert!((second.value - 1.0).abs() < 0.000_001);
    assert_eq!((second.cycle, second.cycles), (1, 1));
}

#[test]
fn signal_cycle_callback_payload_preserves_cycle_multiplicity_and_details() {
    let sample = super::runtime::SignalWorkerSample {
        item_id: NodeId(7),
        label: "Orbit".to_string(),
        value: 0.75,
        cycle: 12,
        cycles: 3,
    };

    assert_eq!(
        super::signal_cycle_callback_args(&sample),
        vec![
            serde_json::json!("Orbit"),
            serde_json::json!(3),
            serde_json::json!({
                "name": "Orbit",
                "cycles": 3,
                "cycle": 12,
                "value": 0.75,
            }),
        ]
    );
}

#[test]
fn signals_script_template_scaffolds_generator_functions_and_callbacks() {
    let config = crate::app::module::script_api::module_script_config(SignalsModule::NODE_TYPE);
    let ScriptSource::Inline(source) = config.source else {
        panic!("Signals module script template should resolve to inline source");
    };

    assert!(source.contains("Signals module functions"));
    assert!(source.contains("local.resetSignals()"));
    assert!(source.contains("local.resetSignal(nameOrIndex)"));
    assert!(source.contains("function signalCycle"));
}

fn test_signal_config(shape: super::SignalShape, range_min: f64, range_max: f64) -> super::SignalConfig {
    super::SignalConfig {
        item_id: NodeId(1),
        value_decl_id: "signal_test".to_string(),
        label: "Signal".to_string(),
        enabled: true,
        shape,
        frequency_hz: 1.0,
        phase: 0.0,
        range_min,
        range_max,
        curve: None,
        seed: 0,
    }
}

fn create_signals_module() -> (crate::app::AppEngine, NodeId) {
    let root: crate::app::AppNode = Folder::new("root").into();
    let mut engine = crate::app::AppEngine::new(root);
    engine.add_node(SignalsModule::create().into(), None);
    stabilize(&mut engine);
    let module_id = engine
        .nodes
        .get(engine.root)
        .and_then(|root| root.node_data().first_child)
        .expect("Signals module should be attached under root");
    (engine, module_id)
}

fn stabilize(engine: &mut crate::app::AppEngine) {
    for _ in 0..8 {
        engine.apply_edits().expect("edits should apply");
        engine
            .dispatch_inbox(ExecutionPhase::EndOfTickStabilization)
            .expect("stabilization inbox should dispatch");
    }
    engine.resolve().expect("schedule should resolve");
}

fn set_param(engine: &mut crate::app::AppEngine, node: NodeId, value: ParamValue) {
    engine.edits.push(Edit::SetParam {
        node,
        value,
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    engine.apply_edits().expect("parameter edit should apply");
}

fn patch_enabled(engine: &mut crate::app::AppEngine, node: NodeId, enabled: bool) {
    engine.edits.push(Edit::PatchMeta {
        node,
        patch: NodeMetaPatch {
            enabled: Some(enabled),
            ..Default::default()
        },
    });
    engine.apply_edits().expect("enabled patch should apply");
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
        let meta = &child.node_data().meta;
        if meta.decl_id.0 == key
            || meta.decl_id.0.rsplit('/').next() == Some(key)
            || meta.short_name == key
            || meta.label == key
        {
            return Some(child_id);
        }
        current = child.node_data().next_sibling;
    }
    None
}

fn count_children_by_key(engine: &crate::app::AppEngine, parent: NodeId, key: &str) -> usize {
    let mut current = engine.nodes.get(parent).and_then(|node| node.node_data().first_child);
    let mut count = 0usize;
    while let Some(child_id) = current {
        let Some(child) = engine.nodes.get(child_id) else {
            break;
        };
        let meta = &child.node_data().meta;
        if meta.decl_id.0 == key
            || meta.decl_id.0.rsplit('/').next() == Some(key)
            || meta.short_name == key
            || meta.label == key
        {
            count += 1;
        }
        current = child.node_data().next_sibling;
    }
    count
}

fn nth_child(engine: &crate::app::AppEngine, parent: NodeId, index: usize) -> Option<NodeId> {
    let mut current = engine.nodes.get(parent)?.node_data().first_child;
    let mut current_index = 0usize;
    while let Some(child_id) = current {
        if current_index == index {
            return Some(child_id);
        }
        current_index += 1;
        current = engine.nodes.get(child_id)?.node_data().next_sibling;
    }
    None
}

fn param_value(engine: &crate::app::AppEngine, node: NodeId) -> Option<ParamValue> {
    engine
        .nodes
        .get(node)?
        .engine_param_snapshot()
        .map(|snapshot| snapshot.value)
}
