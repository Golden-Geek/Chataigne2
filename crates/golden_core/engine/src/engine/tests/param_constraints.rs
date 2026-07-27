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
