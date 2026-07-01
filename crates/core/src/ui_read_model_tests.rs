use crate::edit::{Edit, EditOrigin};
use crate::engine::Engine;
use crate::parameter::{ParamValue, Parameter, ParameterChangeCheck, ParameterEventBehaviour};
use crate::ui_read_model::UiReadModel;
use crate::ui_sync::{UiEditIntent, UiEventBatch, UiEventKind, UiNodeDataDto, UiProjectFileSpec, UiSubscriptionScope};

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

fn event_param_values(events: &[crate::ui_sync::UiEventDto]) -> Vec<i32> {
    events
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

    let read_model = UiReadModel::from_engine(&engine, crate::ui_sync::UiProjectFileSpec::default());

    for value in 1..=3 {
        apply_param_change(&mut engine, value);
    }

    let initial_batch = read_model.publish_engine_events_since(&engine, None);
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

    let compacted_batch = read_model.publish_engine_events_since(&engine, before_event_time);
    assert_eq!(batch_param_values(&compacted_batch), vec![4, 5, 6, 7]);
}

#[test]
fn ui_feedback_coalescing_keeps_latest_only_for_coalescable_value_params() {
    let mut engine = Engine::new(Parameter::new("root", ParamValue::Int(0), ParameterChangeCheck::None));
    let read_model = UiReadModel::from_engine(&engine, UiProjectFileSpec::default());

    for value in 1..=3 {
        apply_param_change(&mut engine, value);
    }

    let batch = read_model.publish_engine_events_since(&engine, None);
    let coalesced = read_model.coalesce_ui_feedback_events(batch.events.clone());
    assert_eq!(event_param_values(&coalesced), vec![3]);

    let mut append_param = Parameter::new("root", ParamValue::Int(0), ParameterChangeCheck::None);
    append_param.event_behaviour = ParameterEventBehaviour::Append;
    let mut append_engine = Engine::new(append_param);
    let append_read_model = UiReadModel::from_engine(&append_engine, UiProjectFileSpec::default());
    for value in 1..=3 {
        apply_param_change(&mut append_engine, value);
    }
    let append_batch = append_read_model.publish_engine_events_since(&append_engine, None);
    let append_coalesced = append_read_model.coalesce_ui_feedback_events(append_batch.events.clone());
    assert_eq!(event_param_values(&append_coalesced), vec![1, 2, 3]);

    let mut trigger_engine = Engine::new(Parameter::new(
        "root",
        ParamValue::Trigger(),
        ParameterChangeCheck::None,
    ));
    let trigger_read_model = UiReadModel::from_engine(&trigger_engine, UiProjectFileSpec::default());
    for _ in 0..2 {
        let ack = trigger_engine.apply_ui_intent(UiEditIntent::SetParam {
            node: trigger_engine.root,
            value: ParamValue::Trigger(),
            behaviour: ParameterEventBehaviour::Coalesce,
        });
        assert!(ack.success, "trigger should apply: {:?}", ack.error_message);
    }
    let trigger_batch = trigger_read_model.publish_engine_events_since(&trigger_engine, None);
    let trigger_coalesced = trigger_read_model.coalesce_ui_feedback_events(trigger_batch.events.clone());
    let trigger_count = trigger_coalesced
        .iter()
        .filter(|event| {
            matches!(
                event.kind,
                UiEventKind::ParamChanged {
                    new_value: ParamValue::Trigger(),
                    ..
                }
            )
        })
        .count();
    assert_eq!(trigger_count, 2);
}

#[test]
fn first_param_change_survives_the_next_structural_rebuild() {
    let mut engine = Engine::new(Parameter::new("root", ParamValue::Int(0), ParameterChangeCheck::None));
    let read_model = UiReadModel::from_engine(&engine, crate::ui_sync::UiProjectFileSpec::default());

    let param_cursor = read_model.current_event_time();
    apply_param_change(&mut engine, 7);
    let param_batch = read_model.publish_engine_events_since(&engine, param_cursor);
    assert_eq!(batch_param_values(&param_batch), vec![7]);

    let structural_cursor = read_model.current_event_time();
    let root = engine.root;
    engine.add_node(
        Parameter::new("child", ParamValue::Int(0), ParameterChangeCheck::None),
        Some(root),
    );
    engine.apply_edits().expect("child should attach");
    read_model.publish_engine_events_since(&engine, structural_cursor);

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

#[test]
fn project_file_path_survives_structural_rebuild() {
    let mut engine = Engine::new(Parameter::new("root", ParamValue::Int(0), ParameterChangeCheck::None));
    let project_file = UiProjectFileSpec {
        display_name: "Noisette".to_string(),
        extension: "noisette".to_string(),
        current_path: Some("D:/Projects/example.noisette".to_string()),
    };
    let read_model = UiReadModel::from_engine(&engine, project_file.clone());

    let structural_cursor = read_model.current_event_time();
    let root = engine.root;
    engine.add_node(
        Parameter::new("child", ParamValue::Int(1), ParameterChangeCheck::None),
        Some(root),
    );
    engine.apply_edits().expect("child should attach");
    read_model.publish_engine_events_since(&engine, structural_cursor);

    assert_eq!(read_model.current_snapshot().project_file, project_file);
}

#[test]
fn eventless_end_edit_updates_snapshot_history_after_structural_edit() {
    let mut engine = Engine::new(Parameter::new("root", ParamValue::Int(0), ParameterChangeCheck::None));
    let read_model = UiReadModel::from_engine(&engine, UiProjectFileSpec::default());
    let session_id = "duplicate-anode";

    engine.edits.push(Edit::BeginEditSession {
        origin: EditOrigin::Ui,
        label: Some("Duplicate ANode".to_string()),
        client_edit_id: session_id.to_string(),
        ui_client_instance_id: None,
    });
    engine.apply_edits().expect("edit session should begin");
    read_model.publish_engine_events_since(&engine, read_model.current_event_time());

    let structural_cursor = read_model.current_event_time();
    let root = engine.root;
    engine.add_node(
        Parameter::new("child", ParamValue::Int(1), ParameterChangeCheck::None),
        Some(root),
    );
    engine
        .apply_edits()
        .expect("structural edit should apply inside edit session");
    read_model.publish_engine_events_since(&engine, structural_cursor);

    assert!(
        read_model
            .snapshot_for_scope(UiSubscriptionScope::WholeGraph)
            .history
            .active_edit_session,
        "structural edit capture should expose the active edit session"
    );

    let end_cursor = read_model.current_event_time();
    engine.edits.push(Edit::EndEditSession {
        client_edit_id: session_id.to_string(),
    });
    engine.apply_edits().expect("edit session should end");
    let end_batch = read_model.publish_engine_events_since(&engine, end_cursor);

    assert!(
        end_batch.events.is_empty(),
        "ending an edit session should not need graph events to update history"
    );
    assert!(
        !read_model
            .snapshot_for_scope(UiSubscriptionScope::WholeGraph)
            .history
            .active_edit_session,
        "eventless end edit capture should clear snapshot history"
    );
}
