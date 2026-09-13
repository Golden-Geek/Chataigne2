use chataigne_alchemist::{
    AlchemistGraphTransaction, AlchemistRuntime, DebugCaptureMode, EvaluationCtx, RuntimeInputSnapshot,
    RuntimeRegistries,
};
use golden_core::node::{
    GRADIENT_STOP_COLOR_DECL_ID, GRADIENT_STOP_NODE_TYPE, PARAMETER_ANIMATION_KEY_NODE_TYPE,
    PARAMETER_ANIMATION_KEY_VALUE_DECL_ID,
};
use std::time::Duration;

use super::*;

#[test]
fn hosted_curve_key_edit_changes_materialized_curve_resource() {
    let (mut engine, formula) = engine_with_formula();
    let anode = create_anode(&mut engine, formula, "curve_remap", 0.0, 0.0);
    let config = find_child_by_decl(&engine, anode, "config").unwrap();
    let curve_node = find_child_by_decl(&engine, config, "config/curve").unwrap();
    let before = engine.process_tree_snapshot();
    let before_instance = anode_from_snapshot(&before, anode).unwrap();
    assert_eq!(
        evaluate_resource_anode(before_instance.clone(), "value", "result", RuntimeValue::Float(0.5)),
        RuntimeValue::Float(0.5)
    );
    let initial = before_instance.config.get("curve").unwrap();
    let RuntimeValue::Extension(initial) = initial else {
        panic!("Curve resource must be typed");
    };
    let initial_curve: golden_core::node::Curve = serde_json::from_slice(&initial.payload).unwrap();
    assert_eq!(initial_curve.sample(0.5), Some(0.5));

    let key = before
        .child_ids(curve_node)
        .into_iter()
        .find(|key| {
            before
                .node(*key)
                .is_some_and(|node| node.node_type == PARAMETER_ANIMATION_KEY_NODE_TYPE)
                && before.find_child_by_decl_id(*key, "position").is_some_and(|position| {
                    before.node(position).and_then(|node| node.param_value.as_ref()) == Some(&ParamValue::Float(1.0))
                })
        })
        .unwrap();
    let value = before
        .find_child_by_decl_id(key, PARAMETER_ANIMATION_KEY_VALUE_DECL_ID)
        .unwrap();
    let ack = engine.apply_ui_intent(UiEditIntent::SetParam {
        node: value,
        value: ParamValue::Float(3.0),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    assert!(ack.success, "Curve key edit should succeed: {ack:?}");
    let after = engine.process_tree_snapshot();
    let after_instance = anode_from_snapshot(&after, anode).unwrap();
    assert_eq!(
        evaluate_resource_anode(after_instance.clone(), "value", "result", RuntimeValue::Float(0.5)),
        RuntimeValue::Float(1.5)
    );
    let RuntimeValue::Extension(resource) = after_instance.config.get("curve").unwrap() else {
        panic!("Curve resource must remain typed");
    };
    let curve: golden_core::node::Curve = serde_json::from_slice(&resource.payload).unwrap();
    assert_eq!(curve.sample(0.5), Some(1.5));

    let json = golden_core::app::to_sparse_project_json_pretty(&engine).unwrap();
    let loaded = golden_core::app::from_sparse_project_json::<AppNode>(&json).unwrap();
    let loaded_anode = loaded
        .nodes
        .iter()
        .find(|(id, _)| anode_type(&loaded, *id).as_deref() == Some("curve_remap"))
        .map(|(id, _)| id)
        .unwrap();
    let loaded_instance = anode_from_snapshot(&loaded.process_tree_snapshot(), loaded_anode).unwrap();
    let RuntimeValue::Extension(loaded_resource) = loaded_instance.config.get("curve").unwrap() else {
        panic!("reloaded curve resource must remain typed");
    };
    let loaded_curve: golden_core::node::Curve = serde_json::from_slice(&loaded_resource.payload).unwrap();
    assert_eq!(loaded_curve.sample(0.5), Some(1.5));
    assert_eq!(
        evaluate_resource_anode(loaded_instance, "value", "result", RuntimeValue::Float(0.5)),
        RuntimeValue::Float(1.5)
    );
}

#[test]
fn hosted_gradient_stop_edit_changes_materialized_gradient_resource() {
    let (mut engine, formula) = engine_with_formula();
    let anode = create_anode(&mut engine, formula, "gradient_sampler", 0.0, 0.0);
    let config = find_child_by_decl(&engine, anode, "config").unwrap();
    let gradient_node = find_child_by_decl(&engine, config, "config/gradient").unwrap();
    let before = engine.process_tree_snapshot();
    let stop = before
        .child_ids(gradient_node)
        .into_iter()
        .find(|stop| {
            before
                .node(*stop)
                .is_some_and(|node| node.node_type == GRADIENT_STOP_NODE_TYPE)
        })
        .unwrap();
    let color = before.find_child_by_decl_id(stop, GRADIENT_STOP_COLOR_DECL_ID).unwrap();
    let ack = engine.apply_ui_intent(UiEditIntent::SetParam {
        node: color,
        value: ParamValue::Color(0.2, 0.4, 0.6, 1.0),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    assert!(ack.success, "Gradient stop edit should succeed: {ack:?}");
    let after = engine.process_tree_snapshot();
    let instance = anode_from_snapshot(&after, anode).unwrap();
    assert_eq!(
        evaluate_resource_anode(instance.clone(), "position", "color", RuntimeValue::Float(0.0)),
        RuntimeValue::Color(golden_values::ColorValue {
            red: 0.2,
            green: 0.4,
            blue: 0.6,
            alpha: 1.0,
        })
    );
    let RuntimeValue::Array(stops) = instance.config.get("gradient").unwrap() else {
        panic!("Gradient resource must materialize as stops");
    };
    assert!(stops.iter().any(|stop| matches!(stop, RuntimeValue::Array(fields)
    if fields.get(1) == Some(&RuntimeValue::Color(golden_values::ColorValue {
        red: 0.2,
        green: 0.4,
        blue: 0.6,
        alpha: 1.0,
    })))));

    let json = golden_core::app::to_sparse_project_json_pretty(&engine).unwrap();
    let loaded = golden_core::app::from_sparse_project_json::<AppNode>(&json).unwrap();
    let loaded_anode = loaded
        .nodes
        .iter()
        .find(|(id, _)| anode_type(&loaded, *id).as_deref() == Some("gradient_sampler"))
        .map(|(id, _)| id)
        .unwrap();
    let loaded_instance = anode_from_snapshot(&loaded.process_tree_snapshot(), loaded_anode).unwrap();
    let RuntimeValue::Array(loaded_stops) = loaded_instance.config.get("gradient").unwrap() else {
        panic!("reloaded gradient resource must contain stops");
    };
    assert_eq!(loaded_stops.len(), 2);
    assert_eq!(
        evaluate_resource_anode(loaded_instance.clone(), "position", "color", RuntimeValue::Float(0.0)),
        RuntimeValue::Color(golden_values::ColorValue {
            red: 0.2,
            green: 0.4,
            blue: 0.6,
            alpha: 1.0,
        })
    );
    assert_eq!(
        evaluate_resource_anode(loaded_instance, "position", "color", RuntimeValue::Float(1.0)),
        RuntimeValue::Color(golden_values::ColorValue {
            red: 1.0,
            green: 1.0,
            blue: 1.0,
            alpha: 1.0,
        })
    );
}

fn evaluate_resource_anode(
    mut instance: ANodeInstance,
    input_socket: &str,
    output_socket: &str,
    input: RuntimeValue,
) -> RuntimeValue {
    let author = instance.id;
    instance.input_defaults.insert(SocketId::new(input_socket), input);
    let value_types = chataigne_state_machine::alchemist::value_type_registry();
    let nodes = chataigne_state_machine::alchemist::node_registry();
    let domain = AlchemistGraphDomain::new(nodes.clone(), value_types.clone(), None);
    let mut document = AlchemistGraphDomain::new_document();
    let mut transaction = AlchemistGraphTransaction::for_document(&document);
    AlchemistGraphDomain::insert_node(&mut transaction, instance);
    transaction.commit(&mut document, &domain).unwrap();
    let compiled = compile_graph(
        &document,
        &CompileCtx {
            value_types: &value_types,
            nodes: &nodes,
            properties: None,
        },
    );
    assert!(!compiled.has_errors(), "{:?}", compiled.diagnostics);
    let inputs = RuntimeInputSnapshot::default();
    let registries = RuntimeRegistries {
        value_types: &value_types,
    };
    let output = AlchemistRuntime::new(compiled.compiled.unwrap()).evaluate_with_capture_mode(
        &EvaluationCtx {
            logical_tick: 1,
            delta_time: Duration::ZERO,
            events: &[],
            inputs: &inputs,
            registries: &registries,
        },
        DebugCaptureMode::All { history_len: 8 },
    );
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    output
        .debug_samples
        .into_iter()
        .find(|sample| sample.author_node_id == author && sample.output_socket.as_str() == output_socket)
        .unwrap_or_else(|| panic!("missing {output_socket} output"))
        .value
}
