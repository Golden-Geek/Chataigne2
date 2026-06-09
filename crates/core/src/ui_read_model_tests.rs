use crate::app::ProjectFileSpec;
use crate::engine::Engine;
use crate::parameter::{ParamValue, Parameter, ParameterChangeCheck, ParameterEventBehaviour};
use crate::ui_read_model::UiReadModel;
use crate::ui_sync::{UiEditIntent, UiEventBatch, UiEventKind, UiNodeDataDto};

fn apply_param_change(engine: &mut Engine<Parameter>, value: i32) {
    let ack = engine.apply_ui_intent(UiEditIntent::SetParam {
        node: engine.root,
        value: ParamValue::Int(value),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    assert!(ack.success, "set param should apply: {:?}", ack.error_message);
}

fn batch_param_values(batch: &UiEventBatch) -> Vec<i32> {
    batch
        .events
        .iter()
        .filter_map(|event| match &event.kind {
            UiEventKind::ParamChanged {
                new_value: ParamValue::Int(value),
                ..
            } => Some(*value),
            _ => None,
        })
        .collect()
}

#[test]
fn publish_engine_events_since_survives_event_log_compaction() {
    let mut engine = Engine::new(Parameter::new("root", ParamValue::Int(0), ParameterChangeCheck::None));
    engine.set_ui_event_log_capacity(4);

    let project_file = ProjectFileSpec::default();
    let read_model = UiReadModel::from_engine(&engine, project_file.clone());

    for value in 1..=3 {
        apply_param_change(&mut engine, value);
    }

    let initial_batch = read_model.publish_engine_events_since(&engine, None, project_file.clone());
    assert_eq!(batch_param_values(&initial_batch), vec![1, 2, 3]);

    let before_event_time = engine.ui_event_log().last().map(|event| event.time);
    for value in 4..=7 {
        apply_param_change(&mut engine, value);
    }

    assert_eq!(
        engine.ui_event_log().len(),
        4,
        "retained ui log should compact to capacity"
    );

    let compacted_batch = read_model.publish_engine_events_since(&engine, before_event_time, project_file);
    assert_eq!(batch_param_values(&compacted_batch), vec![4, 5, 6, 7]);
}

#[test]
fn first_param_change_survives_the_next_structural_rebuild() {
    let mut engine = Engine::new(Parameter::new("root", ParamValue::Int(0), ParameterChangeCheck::None));
    let project_file = ProjectFileSpec::default();
    let read_model = UiReadModel::from_engine(&engine, project_file.clone());

    let param_cursor = read_model.current_event_time();
    apply_param_change(&mut engine, 7);
    let param_batch = read_model.publish_engine_events_since(&engine, param_cursor, project_file.clone());
    assert_eq!(batch_param_values(&param_batch), vec![7]);

    let structural_cursor = read_model.current_event_time();
    let root = engine.root;
    engine.add_node(
        Parameter::new("child", ParamValue::Int(0), ParameterChangeCheck::None),
        Some(root),
    );
    engine.apply_edits().expect("child should attach");
    read_model.publish_engine_events_since(&engine, structural_cursor, project_file);

    let snapshot = read_model.current_snapshot();
    let root = snapshot
        .nodes
        .iter()
        .find(|node| node.node_id == root)
        .expect("root should remain in the rebuilt snapshot");
    let UiNodeDataDto::Parameter { param } = &root.data else {
        panic!("root should remain a parameter");
    };
    assert_eq!(param.value, ParamValue::Int(7));
    assert_eq!(param.default_value, Some(ParamValue::Int(0)));
}
