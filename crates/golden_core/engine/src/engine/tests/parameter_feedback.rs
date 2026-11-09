use crate::{
    app::to_sparse_project_json_pretty,
    define_node_enum,
    edit::Edit,
    engine::Engine,
    events::EventKind,
    node::{Folder, Node, NodeId},
    parameter::{
        ParamValue, Parameter, ParameterChangeCheck, ParameterConstraints, ParameterEventBehaviour, RangeConstraint,
    },
};

#[crate::node("transient_feedback_test")]
#[children(position: f64 = 0.0 (transient_value = true);)]
struct TransientNode {}
#[crate::node("transient_feedback_test", from_struct)]
impl Node for TransientNode {}

define_node_enum!(
    enum FeedbackNode {
        Transient(TransientNode),
    }
);
impl crate::app::ProjectLifecycle for FeedbackNode {}

fn fixture() -> (Engine<FeedbackNode>, NodeId) {
    let mut engine = Engine::<FeedbackNode>::new(Folder::new("root").into());
    let mut parameter = Parameter::new("Status", ParamValue::Float(2.0), ParameterChangeCheck::ValueChange);
    parameter.read_only = true;
    engine.add_node(parameter.into(), None);
    engine.apply_edits().unwrap();
    let id = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    (engine, id)
}
fn live(engine: &Engine<FeedbackNode>, id: NodeId) -> ParamValue {
    engine.nodes.get(id).unwrap().engine_param_snapshot().unwrap().value
}
fn feedback(engine: &mut Engine<FeedbackNode>, node: NodeId, value: Option<ParamValue>) {
    let expected_input = engine
        .nodes
        .get(node)
        .unwrap()
        .as_any()
        .downcast_ref::<Parameter>()
        .unwrap()
        .runtime_input_time;
    engine.edits.push_from(
        engine.root,
        Edit::SetParamFeedback {
            node,
            value,
            expected_input,
            guard: None,
        },
    );
    engine.apply_edits().unwrap();
}
fn authored(engine: &Engine<FeedbackNode>, id: NodeId) -> ParamValue {
    engine
        .nodes
        .get(id)
        .unwrap()
        .as_any()
        .downcast_ref::<Parameter>()
        .unwrap()
        .value
        .clone()
}
fn write(engine: &mut Engine<FeedbackNode>, node: NodeId, value: f64) {
    engine.edits.push(Edit::SetParam {
        node,
        value: ParamValue::Float(value),
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.apply_edits().unwrap();
}

#[test]
fn feedback_changes_live_reads_and_events_without_history_or_saved_value() {
    let (mut engine, id) = fixture();
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    engine.inbox.clear();
    feedback(&mut engine, id, Some(ParamValue::Float(8.0)));
    assert_eq!(live(&engine, id), ParamValue::Float(8.0));
    assert_eq!(
        engine.process_tree_snapshot().node(id).unwrap().param_value,
        Some(ParamValue::Float(8.0))
    );
    assert_eq!(authored(&engine, id), ParamValue::Float(2.0));
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    assert!(engine.inbox.events.iter().any(|event| matches!(&event.kind,
        EventKind::ParamChanged { param, old_value: ParamValue::Float(2.0), new_value: ParamValue::Float(8.0), source: Some(source) }
        if *param == id && *source == engine.root)));
    feedback(&mut engine, id, None);
    assert_eq!(live(&engine, id), ParamValue::Float(2.0));
    assert_eq!(engine.undo_len(), history);
}

#[test]
fn feedback_preserves_redo_and_manual_writes_take_over_at_either_value() {
    let (mut engine, id) = fixture();
    write(&mut engine, id, 5.0);
    assert!(engine.undo().unwrap());
    feedback(&mut engine, id, Some(ParamValue::Float(8.0)));
    assert!(engine.redo().unwrap(), "runtime feedback must not clear redo");
    assert_eq!(live(&engine, id), ParamValue::Float(5.0));
    feedback(&mut engine, id, Some(ParamValue::Float(8.0)));
    write(&mut engine, id, 8.0);
    assert_eq!(authored(&engine, id), ParamValue::Float(8.0));
    assert!(engine.undo().unwrap());
    assert_eq!(live(&engine, id), ParamValue::Float(5.0));
    feedback(&mut engine, id, Some(ParamValue::Float(9.0)));
    let history = engine.undo_len();
    write(&mut engine, id, 5.0);
    assert_eq!(live(&engine, id), ParamValue::Float(5.0));
    assert_eq!(
        engine.undo_len(),
        history,
        "clearing feedback alone is not an authored edit"
    );
}

#[test]
fn constraint_history_never_promotes_feedback_into_authored_value() {
    let (mut engine, id) = fixture();
    feedback(&mut engine, id, Some(ParamValue::Float(4.0)));
    engine.edits.push(Edit::SetParamConstraints {
        node: id,
        constraints: ParameterConstraints {
            range: RangeConstraint::uniform(Some(0.0), Some(5.0)),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    assert_eq!(authored(&engine, id), ParamValue::Float(2.0));
    assert_eq!(live(&engine, id), ParamValue::Float(4.0));
    assert!(engine.undo().unwrap());
    assert_eq!(authored(&engine, id), ParamValue::Float(2.0));
    assert_eq!(live(&engine, id), ParamValue::Float(4.0));
    assert!(engine.redo().unwrap());
    assert_eq!(authored(&engine, id), ParamValue::Float(2.0));
    assert_eq!(live(&engine, id), ParamValue::Float(4.0));
}

#[test]
fn rejected_feedback_leaves_authoring_and_live_state_unchanged() {
    let (mut engine, id) = fixture();
    feedback(&mut engine, id, Some(ParamValue::Float(8.0)));
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    engine.edits.push(Edit::SetParamFeedback {
        node: id,
        value: Some(ParamValue::Str("bad".into())),
        expected_input: None,
        guard: None,
    });
    assert!(engine.apply_edits().is_err());
    assert_eq!(live(&engine, id), ParamValue::Float(8.0));
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    assert_eq!(engine.undo_len(), history);
}

#[test]
fn transient_writes_emit_repeated_intents_without_saved_values_or_history() {
    let (mut engine, id) = fixture();
    let parameter = engine
        .nodes
        .get_mut(id)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<Parameter>()
        .unwrap();
    parameter.read_only = false;
    parameter.transient_value = true;
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    write(&mut engine, id, 8.0);
    engine.inbox.clear();
    write(&mut engine, id, 8.0);
    assert!(
        engine
            .inbox
            .events
            .iter()
            .any(|event| matches!(event.kind, EventKind::ParamChanged { param, .. } if param == id))
    );
    assert_eq!(live(&engine, id), ParamValue::Float(8.0));
    assert_eq!(authored(&engine, id), ParamValue::Float(2.0));
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    feedback(&mut engine, id, Some(ParamValue::Float(9.0)));
    assert_eq!(live(&engine, id), ParamValue::Float(9.0));
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
}

#[test]
fn runtime_range_clamps_live_state_and_validates_all_writes_without_persisting_bounds() {
    let (mut engine, id) = fixture();
    let parameter = engine
        .nodes
        .get_mut(id)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<Parameter>()
        .unwrap();
    parameter.read_only = false;
    parameter.transient_value = true;
    parameter.constraints.policy = crate::parameter::ParameterConstraintPolicy::Reject;
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    write(&mut engine, id, 8.0);
    engine.edits.push_from(
        engine.root,
        Edit::SetParamRuntimeRange {
            node: id,
            range: RangeConstraint::uniform(Some(0.0), Some(5.0)),
            guard: None,
        },
    );
    engine.apply_edits().unwrap();
    assert_eq!(live(&engine, id), ParamValue::Float(5.0));
    let snapshot = engine.nodes.get(id).unwrap().engine_param_snapshot().unwrap();
    assert_eq!(
        snapshot.constraints.range,
        RangeConstraint::uniform(Some(0.0), Some(5.0))
    );
    assert_eq!(
        engine
            .process_tree_snapshot()
            .node(id)
            .unwrap()
            .param_constraints
            .as_ref()
            .unwrap()
            .range,
        snapshot.constraints.range
    );
    engine.edits.push(Edit::SetParam {
        node: id,
        value: ParamValue::Float(6.0),
        behaviour: ParameterEventBehaviour::Append,
    });
    assert!(engine.apply_edits().is_err());
    assert_eq!(live(&engine, id), ParamValue::Float(5.0));
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    engine.edits.push(Edit::SetParamRuntimeRange {
        node: id,
        range: None,
        guard: None,
    });
    engine.apply_edits().unwrap();
    write(&mut engine, id, 6.0);
    assert_eq!(live(&engine, id), ParamValue::Float(6.0));
    assert_eq!(engine.undo_len(), history);
}

#[test]
fn declared_transient_parameter_reloads_at_default_and_keeps_authored_undo_separate() {
    let mut engine = Engine::<FeedbackNode>::new(Folder::new("root").into());
    engine.add_node(TransientNode::new().into(), None);
    engine.settle_edit_notifications(16).unwrap();
    let owner = engine.nodes.get(engine.root).unwrap().node_data().first_child.unwrap();
    let FeedbackNode::Transient(owner) = engine.nodes.get(owner).unwrap() else {
        panic!()
    };
    let id = owner.position.id();
    let parameter = engine
        .nodes
        .get(id)
        .unwrap()
        .as_any()
        .downcast_ref::<Parameter>()
        .unwrap();
    assert!(parameter.transient_value);
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    write(&mut engine, id, 8.0);
    engine.edits.push(Edit::SetParamRuntimeRange {
        node: id,
        range: RangeConstraint::uniform(Some(0.0), Some(5.0)),
        guard: None,
    });
    engine.apply_edits().unwrap();
    assert_eq!(live(&engine, id), ParamValue::Float(5.0));
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    engine.edits.push(Edit::SetParamConstraints {
        node: id,
        constraints: ParameterConstraints {
            step: Some(0.5),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    assert!(engine.undo().unwrap());
    let parameter = engine
        .nodes
        .get(id)
        .unwrap()
        .as_any()
        .downcast_ref::<Parameter>()
        .unwrap();
    assert_eq!(parameter.constraints.range, None);
    assert_eq!(
        parameter.snapshot().constraints.range,
        RangeConstraint::uniform(Some(0.0), Some(5.0))
    );
    assert_eq!(parameter.get(), &ParamValue::Float(5.0));
    assert!(engine.redo().unwrap());
    let current = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded: Engine<FeedbackNode> = crate::app::from_sparse_project_json(&current).unwrap();
    loaded.settle_edit_notifications(16).unwrap();
    let owner = loaded.nodes.get(loaded.root).unwrap().node_data().first_child.unwrap();
    let FeedbackNode::Transient(owner) = loaded.nodes.get(owner).unwrap() else {
        panic!()
    };
    let id = owner.position.id();
    assert_eq!(live(&loaded, id), ParamValue::Float(0.0));
    assert_eq!(
        loaded
            .nodes
            .get(id)
            .unwrap()
            .engine_param_snapshot()
            .unwrap()
            .constraints
            .range,
        None
    );
}

#[test]
fn invalid_runtime_bounds_and_authored_writable_targets_leave_state_unchanged() {
    let (mut engine, id) = fixture();
    for range in [
        RangeConstraint::uniform(Some(6.0), Some(5.0)),
        RangeConstraint::uniform(Some(f64::NAN), None),
    ] {
        let before = engine.nodes.get(id).unwrap().engine_param_snapshot().unwrap();
        engine.edits.push(Edit::SetParamRuntimeRange {
            node: id,
            range,
            guard: None,
        });
        assert!(engine.apply_edits().is_err());
        assert_eq!(engine.nodes.get(id).unwrap().engine_param_snapshot().unwrap(), before);
    }
    engine
        .nodes
        .get_mut(id)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<Parameter>()
        .unwrap()
        .read_only = false;
    engine.edits.push(Edit::SetParamRuntimeRange {
        node: id,
        range: RangeConstraint::uniform(Some(0.0), Some(5.0)),
        guard: None,
    });
    assert!(engine.apply_edits().is_err());
    engine.edits.push(Edit::SetParamFeedback {
        node: id,
        value: Some(ParamValue::Float(3.0)),
        expected_input: None,
        guard: None,
    });
    assert!(engine.apply_edits().is_err());
    assert_eq!(live(&engine, id), ParamValue::Float(2.0));
}

#[test]
fn clearing_feedback_cannot_expose_an_authored_value_outside_runtime_bounds() {
    let (mut engine, id) = fixture();
    engine
        .nodes
        .get_mut(id)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<Parameter>()
        .unwrap()
        .constraints
        .policy = crate::parameter::ParameterConstraintPolicy::Reject;
    engine.edits.push(Edit::SetParamRuntimeRange {
        node: id,
        range: RangeConstraint::uniform(Some(0.0), Some(1.0)),
        guard: None,
    });
    engine.apply_edits().unwrap();
    assert_eq!(live(&engine, id), ParamValue::Float(1.0));
    engine.edits.push(Edit::SetParamFeedback {
        node: id,
        value: None,
        expected_input: None,
        guard: None,
    });
    assert!(engine.apply_edits().is_err());
    assert_eq!(live(&engine, id), ParamValue::Float(1.0));
    engine
        .nodes
        .get_mut(id)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<Parameter>()
        .unwrap()
        .constraints
        .policy = crate::parameter::ParameterConstraintPolicy::ClampAdapt;
    feedback(&mut engine, id, None);
    assert_eq!(live(&engine, id), ParamValue::Float(1.0));
    engine.edits.push(Edit::SetParamRuntimeRange {
        node: id,
        range: None,
        guard: None,
    });
    engine.apply_edits().unwrap();
    feedback(&mut engine, id, None);
    assert_eq!(live(&engine, id), ParamValue::Float(2.0));
}

fn transient_fixture() -> (Engine<FeedbackNode>, NodeId) {
    let (mut engine, id) = fixture();
    let parameter = engine
        .nodes
        .get_mut(id)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<Parameter>()
        .unwrap();
    parameter.read_only = false;
    parameter.transient_value = true;
    (engine, id)
}
fn input_time(engine: &Engine<FeedbackNode>, id: NodeId) -> Option<crate::engine::EngineTime> {
    engine
        .nodes
        .get(id)
        .unwrap()
        .as_any()
        .downcast_ref::<Parameter>()
        .unwrap()
        .runtime_input_time
}
fn begin_seek(engine: &mut Engine<FeedbackNode>) {
    engine.edits.push(Edit::BeginEditSession {
        origin: crate::edit::EditOrigin::Ui,
        label: None,
        client_edit_id: "seek".into(),
        ui_client_instance_id: Some("client".into()),
    });
    engine.apply_edits().unwrap();
}
fn end_seek(engine: &mut Engine<FeedbackNode>) {
    engine.edits.push(Edit::EndEditSession {
        client_edit_id: "seek".into(),
    });
    engine.settle_edit_notifications(16).unwrap();
}

#[test]
fn feedback_rejects_a_previous_input_even_before_owner_callbacks_run() {
    let (mut engine, id) = transient_fixture();
    write(&mut engine, id, 5.0);
    let previous = input_time(&engine, id);
    write(&mut engine, id, 6.0);
    engine.edits.push(Edit::SetParamFeedback {
        node: id,
        value: Some(ParamValue::Float(9.0)),
        expected_input: previous,
        guard: None,
    });
    engine.apply_edits().unwrap();
    assert_eq!(live(&engine, id), ParamValue::Float(6.0));
    feedback(&mut engine, id, Some(ParamValue::Float(7.0)));
    assert_eq!(live(&engine, id), ParamValue::Float(7.0));
}

#[test]
fn active_seek_defers_latest_feedback_without_history_and_drops_superseded_input() {
    let (mut engine, id) = transient_fixture();
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    begin_seek(&mut engine);
    write(&mut engine, id, 5.0);
    feedback(&mut engine, id, Some(ParamValue::Float(7.0)));
    assert_eq!(live(&engine, id), ParamValue::Float(5.0));
    write(&mut engine, id, 6.0);
    feedback(&mut engine, id, Some(ParamValue::Float(8.0)));
    feedback(&mut engine, id, Some(ParamValue::Float(9.0)));
    assert_eq!(live(&engine, id), ParamValue::Float(6.0));
    end_seek(&mut engine);
    assert_eq!(live(&engine, id), ParamValue::Float(9.0));
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
}

#[test]
fn producer_changes_fence_deferred_values_and_runtime_bounds() {
    let (mut engine, id) = transient_fixture();
    let producer = golden_io::RevisionSource::default();
    let reader = producer.reader();
    let old = reader.guard_at(reader.current());
    begin_seek(&mut engine);
    write(&mut engine, id, 5.0);
    engine.edits.push(Edit::SetParamFeedback {
        node: id,
        value: Some(ParamValue::Float(9.0)),
        expected_input: input_time(&engine, id),
        guard: Some(old.clone()),
    });
    engine.apply_edits().unwrap();
    producer.advance();
    engine.edits.push(Edit::SetParamRuntimeRange {
        node: id,
        range: RangeConstraint::uniform(Some(0.0), Some(1.0)),
        guard: Some(old),
    });
    end_seek(&mut engine);
    assert_eq!(live(&engine, id), ParamValue::Float(5.0));
    assert!(
        engine
            .nodes
            .get(id)
            .unwrap()
            .engine_param_snapshot()
            .unwrap()
            .constraints
            .range
            .is_none()
    );
    feedback(&mut engine, id, Some(ParamValue::Float(8.0)));
    assert_eq!(live(&engine, id), ParamValue::Float(8.0));
}

#[test]
fn unrelated_session_targets_do_not_suspend_a_transient_parameter() {
    let (mut engine, id) = transient_fixture();
    let mut other = Parameter::new("Other", ParamValue::Float(0.0), ParameterChangeCheck::ValueChange);
    other.transient_value = true;
    engine.add_node(other.into(), None);
    engine.apply_edits().unwrap();
    let other = engine
        .nodes
        .iter()
        .find(|(_, node)| node.node_data().meta.label == "Other")
        .unwrap()
        .0;
    begin_seek(&mut engine);
    write(&mut engine, other, 3.0);
    feedback(&mut engine, id, Some(ParamValue::Float(9.0)));
    assert_eq!(live(&engine, id), ParamValue::Float(9.0));
    end_seek(&mut engine);
}

#[test]
fn disconnected_client_resumes_feedback_and_removed_parameter_starts_a_fresh_lifetime() {
    let (mut engine, id) = transient_fixture();
    begin_seek(&mut engine);
    write(&mut engine, id, 5.0);
    let previous = input_time(&engine, id);
    feedback(&mut engine, id, Some(ParamValue::Float(8.0)));
    assert!(engine.cancel_active_ui_edit_session_for_client("client"));
    engine.settle_edit_notifications(16).unwrap();
    assert_eq!(live(&engine, id), ParamValue::Float(8.0));
    engine.edits.push(Edit::RemoveNode { node: id });
    engine.apply_edits().unwrap();
    assert!(engine.undo().unwrap());
    assert_eq!(live(&engine, id), ParamValue::Float(2.0));
    engine.edits.push(Edit::SetParamFeedback {
        node: id,
        value: Some(ParamValue::Float(9.0)),
        expected_input: previous,
        guard: None,
    });
    engine.apply_edits().unwrap();
    assert_eq!(live(&engine, id), ParamValue::Float(2.0));
}

#[test]
fn shared_runtime_bounds_prepare_writes_before_publishing_a_constraint_event() {
    use crate::parameter::RuntimeScalarRange;
    let (mut engine, id) = transient_fixture();
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    let range = RuntimeScalarRange::new(0.0, 0.0).unwrap();
    engine.edits.push(Edit::BindParamRuntimeRange {
        node: id,
        range: range.clone(),
    });
    engine.apply_edits().unwrap();
    engine.inbox.clear();
    range.set(0.0, 10.0).unwrap();
    assert_eq!(
        engine
            .nodes
            .get(id)
            .unwrap()
            .engine_param_snapshot()
            .unwrap()
            .constraints
            .range,
        RangeConstraint::uniform(Some(0.0), Some(10.0))
    );
    write(&mut engine, id, 8.0);
    assert_eq!(live(&engine, id), ParamValue::Float(8.0));
    engine
        .edits
        .push(Edit::RefreshParamRuntimeRange { node: id, guard: None });
    engine.apply_edits().unwrap();
    assert!(engine.inbox.events.iter().any(|event| matches!(
        &event.kind,
        EventKind::ParamConstraintsChanged { param, new_constraints, .. }
        if *param == id && new_constraints.range == RangeConstraint::uniform(Some(0.0), Some(10.0))
    )));
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
}

#[test]
fn shared_bounds_do_not_mutate_an_atomic_proposal_or_allow_partial_rejection() {
    use crate::parameter::RuntimeScalarRange;
    let (mut engine, id) = transient_fixture();
    let other = Parameter::new("Other", ParamValue::Float(0.0), ParameterChangeCheck::ValueChange);
    engine.add_node(other.into(), None);
    engine.apply_edits().unwrap();
    let other = engine
        .nodes
        .iter()
        .find(|(_, node)| node.node_data().meta.label == "Other")
        .unwrap()
        .0;
    let range = RuntimeScalarRange::new(0.0, 0.0).unwrap();
    engine.edits.push(Edit::BindParamRuntimeRange {
        node: id,
        range: range.clone(),
    });
    engine.apply_edits().unwrap();
    range.set(0.0, 10.0).unwrap();
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    engine.edits.push(Edit::SetParamsAtomic {
        values: vec![(id, ParamValue::Float(8.0)), (other, ParamValue::Float(f64::NAN))],
        behaviour: ParameterEventBehaviour::Append,
    });
    assert!(engine.apply_edits().is_err());
    assert_eq!(live(&engine, id), ParamValue::Float(0.0));
    assert_eq!(live(&engine, other), ParamValue::Float(0.0));
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    engine.edits.push(Edit::SetParamsAtomic {
        values: vec![(id, ParamValue::Float(8.0)), (other, ParamValue::Float(4.0))],
        behaviour: ParameterEventBehaviour::Append,
    });
    engine.apply_edits().unwrap();
    assert_eq!(live(&engine, id), ParamValue::Float(8.0));
    assert_eq!(live(&engine, other), ParamValue::Float(4.0));
}

#[test]
fn shared_range_validation_fixed_replacement_and_removal_fence_old_sources() {
    use crate::parameter::RuntimeScalarRange;
    let (mut engine, id) = transient_fixture();
    let range = RuntimeScalarRange::new(0.0, 10.0).unwrap();
    for (min, max) in [(3.0, 2.0), (f64::NAN, 3.0), (0.0, f64::INFINITY)] {
        assert!(range.set(min, max).is_err());
        assert_eq!(range.range(), RangeConstraint::uniform(Some(0.0), Some(10.0)).unwrap());
    }
    engine.edits.push(Edit::BindParamRuntimeRange {
        node: id,
        range: range.clone(),
    });
    engine.apply_edits().unwrap();
    write(&mut engine, id, 8.0);
    range.set(0.0, 1.0).unwrap();
    engine
        .edits
        .push(Edit::RefreshParamRuntimeRange { node: id, guard: None });
    engine.apply_edits().unwrap();
    assert_eq!(live(&engine, id), ParamValue::Float(1.0));
    engine.edits.push(Edit::SetParamRuntimeRange {
        node: id,
        range: RangeConstraint::uniform(Some(0.0), Some(3.0)),
        guard: None,
    });
    engine.apply_edits().unwrap();
    range.set(0.0, 100.0).unwrap();
    write(&mut engine, id, 8.0);
    assert_eq!(live(&engine, id), ParamValue::Float(3.0));
    engine.edits.push(Edit::BindParamRuntimeRange {
        node: id,
        range: range.clone(),
    });
    engine.apply_edits().unwrap();
    engine.edits.push(Edit::RemoveNode { node: id });
    engine.apply_edits().unwrap();
    assert!(engine.undo().unwrap());
    range.set(0.0, 1.0).unwrap();
    assert_eq!(live(&engine, id), ParamValue::Float(2.0));
    assert!(
        engine
            .nodes
            .get(id)
            .unwrap()
            .engine_param_snapshot()
            .unwrap()
            .constraints
            .range
            .is_none()
    );
}
