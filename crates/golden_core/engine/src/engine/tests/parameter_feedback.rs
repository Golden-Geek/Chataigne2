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

define_node_enum!(
    enum FeedbackNode {}
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
    engine
        .edits
        .push_from(engine.root, Edit::SetParamFeedback { node, value });
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
    });
    assert!(engine.apply_edits().is_err());
    assert_eq!(live(&engine, id), ParamValue::Float(8.0));
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    assert_eq!(engine.undo_len(), history);
}
