use crate::define_node_enum;
use crate::edit::Edit;
use crate::engine::Engine;
use crate::events::EventKind;
use crate::node::{Folder, Node, NodeId};
use crate::parameter::{
    ParamValue, Parameter, ParameterChangeCheck, ParameterConstraintPolicy, ParameterConstraints, RangeConstraint,
};

define_node_enum!(
    enum ParamConstraintsTestNode {}
);

fn first_child<T: Node>(engine: &Engine<T>, parent: NodeId) -> NodeId {
    engine
        .nodes
        .get(parent)
        .and_then(|node| node.node_data().first_child)
        .expect("parent should have one child")
}

fn param_snapshot(engine: &Engine<ParamConstraintsTestNode>, param: NodeId) -> crate::parameter::ParameterSnapshot {
    engine
        .nodes
        .get(param)
        .and_then(|node| node.engine_param_snapshot())
        .expect("parameter snapshot should exist")
}

#[test]
fn set_param_constraints_emits_events_and_supports_undo_redo() {
    let root: ParamConstraintsTestNode = Folder::new("root").into();
    let mut engine = Engine::new(root);
    engine.add_node(
        Parameter::new("Gain", ParamValue::Float(2.0), ParameterChangeCheck::ValueChange).into(),
        None,
    );
    engine.apply_edits().expect("parameter should attach");

    let param = first_child(&engine, engine.root);
    engine.inbox.clear();

    let constraints = ParameterConstraints {
        range: RangeConstraint::uniform(Some(0.0), Some(1.0)),
        policy: ParameterConstraintPolicy::ClampAdapt,
        ..Default::default()
    };

    engine.edits.push(Edit::SetParamConstraints {
        node: param,
        constraints: constraints.clone(),
    });
    engine
        .apply_edits()
        .expect("setting parameter constraints should succeed");

    let snapshot = param_snapshot(&engine, param);
    assert_eq!(snapshot.value, ParamValue::Float(1.0));
    assert_eq!(snapshot.constraints, constraints);
    assert!(
        engine.inbox.events.iter().any(|event| matches!(
            &event.kind,
            EventKind::ParamChanged {
                param: changed_param,
                old_value: ParamValue::Float(old),
                new_value: ParamValue::Float(new),
            } if *changed_param == param && (*old - 2.0).abs() < f64::EPSILON && (*new - 1.0).abs() < f64::EPSILON
        )),
        "expected a value change event after clamping the current value"
    );
    assert!(
        engine.inbox.events.iter().any(|event| matches!(
            &event.kind,
            EventKind::ParamConstraintsChanged {
                param: changed_param,
                old_constraints,
                new_constraints,
            } if *changed_param == param
                && *old_constraints == ParameterConstraints::default()
                && **new_constraints == constraints
        )),
        "expected a constraints change event"
    );

    assert!(engine.undo().expect("undo should succeed"));
    let undone = param_snapshot(&engine, param);
    assert_eq!(undone.value, ParamValue::Float(2.0));
    assert_eq!(undone.constraints, ParameterConstraints::default());

    assert!(engine.redo().expect("redo should succeed"));
    let redone = param_snapshot(&engine, param);
    assert_eq!(redone.value, ParamValue::Float(1.0));
    assert_eq!(redone.constraints, constraints);
}

fn option(value: &str) -> crate::parameter::ParameterEnumOption {
    crate::parameter::ParameterEnumOption {
        variant_id: value.into(),
        value: ParamValue::Enum(value.into()),
        label: value.into(),
        tags: Vec::new(),
        ordering: None,
    }
}

#[test]
fn enum_domains_share_live_snapshots_and_retained_history_with_correct_replay() {
    let first: crate::parameter::ParameterEnumOptions = vec![option("first")].into();
    let second: crate::parameter::ParameterEnumOptions = vec![option("first"), option("second")].into();
    let mut parameter = Parameter::new(
        "Device",
        ParamValue::Enum("first".into()),
        ParameterChangeCheck::ValueChange,
    );
    parameter.constraints.enum_options = first.clone();
    let mut engine = Engine::new(parameter);
    let before = engine.build_process_tree_snapshot();
    engine.edits.push(Edit::SetParamConstraints {
        node: engine.root,
        constraints: ParameterConstraints {
            enum_options: second.clone(),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    let current = engine.nodes.get(engine.root).unwrap().engine_param_snapshot().unwrap();
    assert!(current.constraints.enum_options.same_storage(&second));
    assert_eq!(current.value, ParamValue::Enum("first".into()));
    assert!(
        before
            .node(engine.root)
            .unwrap()
            .param_constraints
            .as_ref()
            .unwrap()
            .enum_options
            .same_storage(&first)
    );
    assert!(engine.undo().unwrap());
    let undone = engine.nodes.get(engine.root).unwrap().engine_param_snapshot().unwrap();
    // Replay callbacks restore authored data through serialization. The retained
    // history and pre-edit snapshot still own their original immutable domain.
    assert_eq!(undone.constraints.enum_options, first);
    assert!(std::sync::Arc::strong_count(first.shared()) >= 3);
    assert_eq!(undone.value, ParamValue::Enum("first".into()));
    assert!(engine.redo().unwrap());
    assert!(
        engine
            .nodes
            .get(engine.root)
            .unwrap()
            .engine_param_snapshot()
            .unwrap()
            .constraints
            .enum_options
            .eq(&second)
    );
    assert!(std::sync::Arc::strong_count(second.shared()) >= 3);
}

#[test]
fn native_option_quota_survives_live_state_snapshot_and_exported_constraint_owners() {
    let pool = golden_io::PayloadPool::new("native-enum-domain-test", 1, 1024 * 1024).unwrap();
    let shared = pool
        .try_reserve(1024)
        .unwrap()
        .share_retained(vec![option("first")], 1024)
        .unwrap();
    let options = crate::parameter::ParameterEnumOptions::from_shared(shared.arc().clone());
    let mut parameter = Parameter::new(
        "Device",
        ParamValue::Enum("first".into()),
        ParameterChangeCheck::ValueChange,
    );
    parameter.constraints.enum_options = options.clone();
    let engine = Engine::new(parameter);
    let snapshot = engine.build_process_tree_snapshot();
    let exported = snapshot.node(engine.root).unwrap().param_constraints.clone().unwrap();
    drop((engine, snapshot, shared, options));
    assert!(
        pool.try_reserve(1).is_err(),
        "exported constraints retain the original native quota"
    );
    drop(exported);
    assert!(pool.wait_for_idle(std::time::Duration::from_secs(3)));
}
