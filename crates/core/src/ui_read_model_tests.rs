use crate::app::ProjectFileSpec;
use crate::engine::Engine;
use crate::parameter::{ParamValue, Parameter, ParameterChangeCheck, ParameterEventBehaviour};
use crate::ui_read_model::UiReadModel;
use crate::ui_sync::{UiEditIntent, UiEventBatch, UiEventKind};

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

    assert_eq!(engine.ui_event_log().len(), 4, "retained ui log should compact to capacity");

    let compacted_batch = read_model.publish_engine_events_since(&engine, before_event_time, project_file);
    assert_eq!(batch_param_values(&compacted_batch), vec![4, 5, 6, 7]);
}