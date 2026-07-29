use crate::edit::{Edit, EditOrigin};
use crate::engine::Engine;
use crate::events::CustomEvent;
use crate::node::{Node, NodeMetaPatch};
use crate::parameter::{ParamValue, Parameter, ParameterChangeCheck, ParameterEnumOption, ParameterEventBehaviour};
use crate::ui_read_model::UiReadModel;
use crate::ui_sync::{UiEditIntent, UiEventBatch, UiEventKind, UiNodeDataDto, UiProjectFileSpec, UiSubscriptionScope};
use std::collections::HashSet;
use std::sync::{Arc, Barrier};

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
    let mut root = Parameter::new("root", ParamValue::Int(0), ParameterChangeCheck::None);
    root.event_behaviour = ParameterEventBehaviour::Append;
    let mut engine = Engine::new(root);
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
fn retained_ui_event_logs_keep_latest_only_for_coalescable_value_params() {
    let mut engine = Engine::new(Parameter::new("root", ParamValue::Int(0), ParameterChangeCheck::None));
    let read_model = UiReadModel::from_engine(&engine, UiProjectFileSpec::default());

    for value in 1..=200 {
        apply_param_change(&mut engine, value);
    }

    assert_eq!(
        engine.ui_event_log().len(),
        1,
        "engine replay log should retain only the latest coalescable value per parameter"
    );

    let batch = read_model.publish_engine_events_since(&engine, None);
    assert_eq!(batch_param_values(&batch), vec![200]);

    let replay = read_model.replay(None, UiSubscriptionScope::WholeGraph);
    assert_eq!(batch_param_values(&replay), vec![200]);
}

#[test]
fn engine_ui_retention_indexes_preserve_barriers_and_recover_after_trim() {
    let mut engine = Engine::new(Parameter::new("root", ParamValue::Int(0), ParameterChangeCheck::None));

    apply_param_change(&mut engine, 1);
    engine.edits.push(Edit::EmitCustomEvent {
        event: CustomEvent::new("test.barrier", None, serde_json::Value::Null),
    });
    engine.apply_edits().expect("replay barrier should apply");
    apply_param_change(&mut engine, 2);
    assert_eq!(
        engine
            .ui_event_log()
            .iter()
            .filter_map(|event| match &event.kind {
                crate::events::EventKind::ParamChanged {
                    new_value: ParamValue::Int(value),
                    ..
                } => Some(*value),
                _ => None,
            })
            .collect::<Vec<_>>(),
        vec![1, 2],
        "a reliable barrier must end the coalescable parameter run"
    );

    engine.clear_ui_event_log();
    engine.set_ui_event_log_capacity(1);
    engine.edits.push(Edit::EmitCustomEvent {
        event: CustomEvent::latest("test.preview", None, serde_json::json!({ "generation": 1 })),
    });
    engine.apply_edits().expect("latest event should apply");
    engine.edits.push(Edit::EmitCustomEvent {
        event: CustomEvent::new("test.reliable", None, serde_json::Value::Null),
    });
    engine.apply_edits().expect("reliable event should evict the preview");
    engine.edits.push(Edit::EmitCustomEvent {
        event: CustomEvent::latest("test.preview", None, serde_json::json!({ "generation": 2 })),
    });
    engine
        .apply_edits()
        .expect("a stale latest-event index should recover lazily");

    let [retained] = engine.ui_event_log() else {
        panic!("capacity-one log should retain exactly one event");
    };
    let crate::events::EventKind::Custom(retained) = &retained.kind else {
        panic!("latest preview should be retained");
    };
    assert_eq!(retained.topic, "test.preview");
    assert_eq!(
        retained.payload.get("generation").and_then(serde_json::Value::as_i64),
        Some(2)
    );

    for generation in 3..=32 {
        engine.edits.push(Edit::EmitCustomEvent {
            event: CustomEvent::latest(
                format!("test.preview.{generation}"),
                None,
                serde_json::json!({ "generation": generation }),
            ),
        });
        engine.apply_edits().expect("dynamic latest event should apply");
    }
    assert_eq!(
        engine.ui_event_index_sizes_for_tests(),
        (1, 0),
        "retention indexes must discard dynamic keys with their evicted events"
    );
}

#[test]
fn read_model_retains_only_latest_explicit_latest_custom_event() {
    let mut engine = Engine::new(Parameter::new("root", ParamValue::Int(0), ParameterChangeCheck::None));
    let root = engine.root;
    let read_model = UiReadModel::from_engine(&engine, UiProjectFileSpec::default());

    engine.edits.push(Edit::EmitCustomEvent {
        event: CustomEvent::latest("test.runtime_frame", Some(root), serde_json::json!({ "generation": 1 })),
    });
    engine.apply_edits().expect("first latest event should apply");
    let first = read_model.publish_engine_events_since(&engine, None);

    engine.edits.push(Edit::EmitCustomEvent {
        event: CustomEvent::latest("test.runtime_frame", Some(root), serde_json::json!({ "generation": 2 })),
    });
    engine.apply_edits().expect("second latest event should apply");
    read_model.publish_engine_events_since(&engine, first.to);

    let replay = read_model.replay(None, UiSubscriptionScope::WholeGraph);
    assert_eq!(replay.events.len(), 1);
    assert!(matches!(
        &replay.events[0].kind,
        UiEventKind::Custom { topic, payload, .. }
            if topic == "test.runtime_frame" && payload["generation"] == 2
    ));

    // Ordinary custom events remain lossless even when their topic contains "preview".
    for sequence in 1..=2 {
        engine.edits.push(Edit::EmitCustomEvent {
            event: CustomEvent::new(
                "test.runtime_preview",
                Some(root),
                serde_json::json!({ "sequence": sequence }),
            ),
        });
        engine.apply_edits().expect("replayed custom event should apply");
    }
    read_model.publish_engine_events_since(&engine, replay.to);
    let replay = read_model.replay(None, UiSubscriptionScope::WholeGraph);
    assert_eq!(
        replay
            .events
            .iter()
            .filter(|event| matches!(&event.kind, UiEventKind::Custom { topic, .. } if topic == "test.runtime_preview"))
            .count(),
        2,
    );
}

#[test]
fn replay_before_the_first_new_event_does_not_require_resync_without_eviction() {
    let mut root = Parameter::new("root", ParamValue::Int(0), ParameterChangeCheck::None);
    root.event_behaviour = ParameterEventBehaviour::Append;
    let mut engine = Engine::new(root);
    let read_model = UiReadModel::from_engine(&engine, UiProjectFileSpec::default());
    let snapshot_time = read_model.current_snapshot().at;

    engine.time.tick += 1;
    apply_param_change(&mut engine, 1);
    read_model.publish_engine_events_since(&engine, Some(snapshot_time));
    let replay = read_model.replay(Some(snapshot_time), UiSubscriptionScope::WholeGraph);

    assert_eq!(batch_param_values(&replay), vec![1]);
    assert!(!replay.events.iter().any(|event| {
        matches!(
            &event.kind,
            UiEventKind::Custom { topic, .. } if topic == "__transport.resync_required"
        )
    }));
}

#[test]
fn replay_seeks_directly_to_the_suffix_after_a_near_tail_cursor() {
    let mut root = Parameter::new("root", ParamValue::Int(0), ParameterChangeCheck::None);
    root.event_behaviour = ParameterEventBehaviour::Append;
    let mut engine = Engine::new(root);
    let read_model = UiReadModel::from_engine(&engine, UiProjectFileSpec::default());

    for value in 1..=256 {
        apply_param_change(&mut engine, value);
    }
    let published = read_model.publish_engine_events_since(&engine, None);
    let cursor = published.events[254].time;

    let replay = read_model.replay(Some(cursor), UiSubscriptionScope::WholeGraph);
    assert_eq!(batch_param_values(&replay), vec![256]);
    assert_eq!(replay.to, published.to);
}

#[test]
fn replay_requires_resync_only_after_the_cursor_falls_behind_an_eviction() {
    let mut root = Parameter::new("root", ParamValue::Int(0), ParameterChangeCheck::None);
    root.event_behaviour = ParameterEventBehaviour::Append;
    let mut engine = Engine::new(root);
    let mut read_model = UiReadModel::from_engine(&engine, UiProjectFileSpec::default());
    read_model.set_event_capacity_for_tests(2);
    let snapshot_time = read_model.current_snapshot().at;

    engine.time.tick += 1;
    for value in 1..=3 {
        apply_param_change(&mut engine, value);
    }
    read_model.publish_engine_events_since(&engine, Some(snapshot_time));
    let replay = read_model.replay(Some(snapshot_time), UiSubscriptionScope::WholeGraph);

    assert!(replay.events.iter().any(|event| {
        matches!(
            &event.kind,
            UiEventKind::Custom { topic, payload, .. }
                if topic == "__transport.resync_required"
                    && payload.get("reason").and_then(serde_json::Value::as_str)
                        == Some("cursor_out_of_retention_window")
        )
    }));
}

#[test]
fn whole_graph_snapshot_survives_corrupt_sibling_cycle() {
    let mut engine = Engine::new(Parameter::new("root", ParamValue::Int(0), ParameterChangeCheck::None));
    engine.add_node(
        Parameter::new("first", ParamValue::Int(1), ParameterChangeCheck::None),
        None,
    );
    engine.add_node(
        Parameter::new("second", ParamValue::Int(2), ParameterChangeCheck::None),
        None,
    );
    engine.apply_edits().expect("children should be inserted");

    let root = engine.root;
    let first_child = engine
        .nodes
        .get(root)
        .and_then(|node| node.node_data().first_child)
        .expect("root should have a first child");
    let second_child = engine
        .nodes
        .get(first_child)
        .and_then(|node| node.node_data().next_sibling)
        .expect("first child should have a sibling before corruption");
    assert_ne!(first_child, second_child);

    engine
        .nodes
        .get_mut(first_child)
        .expect("first child should exist")
        .node_data_mut()
        .next_sibling = Some(first_child);

    let read_model = UiReadModel::from_engine(&engine, UiProjectFileSpec::default());
    let snapshot = read_model.current_snapshot();

    assert!(snapshot.nodes.iter().any(|node| node.node_id == root));
    assert!(snapshot.nodes.iter().any(|node| node.node_id == first_child));
    assert!(snapshot.nodes.len() <= engine.nodes.len());
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
fn scoped_replay_filters_unrelated_graph_transactions() {
    let mut engine = Engine::new(Parameter::new("root", ParamValue::Int(0), ParameterChangeCheck::None));
    let root = engine.root;
    engine.add_node(
        Parameter::new("left", ParamValue::Int(0), ParameterChangeCheck::None),
        Some(root),
    );
    engine.add_node(
        Parameter::new("right", ParamValue::Int(0), ParameterChangeCheck::None),
        Some(root),
    );
    engine.apply_edits().expect("children should attach");

    let read_model = UiReadModel::from_engine(&engine, UiProjectFileSpec::default());
    let snapshot = read_model.current_snapshot();
    let root_snapshot = snapshot
        .nodes
        .iter()
        .find(|node| node.node_id == root)
        .expect("root should exist");
    let left = root_snapshot.children[0];
    let right = root_snapshot.children[1];

    let ack = engine.apply_ui_intent(UiEditIntent::PatchMeta {
        node: right,
        patch: NodeMetaPatch {
            label: Some("right renamed".to_string()),
            ..Default::default()
        },
    });
    assert!(ack.success, "rename should apply: {:?}", ack.error_message);
    read_model.publish_engine_events_since(&engine, Some(snapshot.at));

    let left_replay = read_model.replay(
        None,
        UiSubscriptionScope::Subtree {
            root: left,
            max_depth: u32::MAX,
        },
    );
    assert!(
        left_replay.events.is_empty(),
        "unrelated sibling transaction should not fan out to left subtree"
    );
    assert_eq!(
        left_replay.to,
        read_model.current_event_time(),
        "an out-of-scope suffix must still advance the subscription cursor"
    );

    let right_replay = read_model.replay(
        None,
        UiSubscriptionScope::Subtree {
            root: right,
            max_depth: u32::MAX,
        },
    );
    assert_eq!(right_replay.events.len(), 1);
    let UiEventKind::GraphTransaction { transaction } = &right_replay.events[0].kind else {
        panic!("right subtree should receive a graph transaction");
    };
    assert_eq!(transaction.ops.len(), 1);

    let caught_up_left = read_model.replay(
        left_replay.to,
        UiSubscriptionScope::Subtree {
            root: left,
            max_depth: u32::MAX,
        },
    );
    assert!(caught_up_left.events.is_empty());
    assert_eq!(
        caught_up_left.to, None,
        "a caught-up scoped replay must not repeatedly scan the same retained suffix"
    );
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
fn snapshot_materializes_parameter_updates_without_a_structural_rebuild() {
    let mut engine = Engine::new(Parameter::new(
        "root",
        ParamValue::Enum("wasapi".to_string()),
        ParameterChangeCheck::None,
    ));
    let root = engine.root;
    let read_model = UiReadModel::from_engine(&engine, UiProjectFileSpec::default());
    let cursor = read_model.current_event_time();

    let mut constraints = engine.nodes.get(root).expect("root parameter").constraints.clone();
    constraints.enum_options = ["wasapi", "asio"]
        .into_iter()
        .enumerate()
        .map(|(ordering, backend)| ParameterEnumOption {
            variant_id: backend.to_string(),
            value: ParamValue::Enum(backend.to_string()),
            label: backend.to_uppercase(),
            tags: Vec::new(),
            ordering: i32::try_from(ordering).ok(),
        })
        .collect();
    engine.edits.push(Edit::SetParamConstraints {
        node: root,
        constraints: constraints.clone(),
    });
    engine.apply_edits().expect("parameter constraints should apply");
    let ack = engine.apply_ui_intent(UiEditIntent::SetParam {
        node: root,
        value: ParamValue::Enum("asio".to_string()),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    assert!(ack.success, "enum value should apply: {:?}", ack.error_message);
    let batch = read_model.publish_engine_events_since(&engine, cursor);

    let snapshot = read_model.snapshot_for_scope(UiSubscriptionScope::WholeGraph);
    assert_eq!(snapshot.at, batch.to.expect("parameter updates should advance time"));
    let root = snapshot
        .nodes
        .iter()
        .find(|node| node.node_id == root)
        .expect("root should remain in the snapshot");
    let UiNodeDataDto::Parameter { param } = &root.data else {
        panic!("root should remain a parameter");
    };
    assert_eq!(param.value, ParamValue::Enum("asio".to_string()));
    assert_eq!(param.constraints, constraints);
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

#[test]
fn user_context_custom_events_refresh_the_read_model_snapshot() {
    let mut engine = Engine::new(Parameter::new("root", ParamValue::Int(0), ParameterChangeCheck::None));
    let read_model = UiReadModel::from_engine(&engine, UiProjectFileSpec::default());
    let cursor = read_model.current_event_time();

    let ack = engine.apply_ui_intent(UiEditIntent::EnsureUserContextScope { owner: engine.root });
    assert!(ack.success, "scope creation should apply: {:?}", ack.error_message);

    let batch = read_model.publish_engine_events_since(&engine, cursor);
    assert!(matches!(
        batch.events.as_slice(),
        [event]
            if matches!(
                &event.kind,
                UiEventKind::Custom { topic, .. }
                    if topic == "__user_context.scope_changed"
            )
    ));

    let snapshot = read_model.current_snapshot();
    assert_eq!(snapshot.user_contexts.scopes.len(), 1);
    assert_eq!(snapshot.user_contexts.scopes[0].owner, engine.root);
}

#[test]
fn structural_publication_defers_whole_graph_snapshot_materialization() {
    let mut engine = Engine::new(Parameter::new("root", ParamValue::Int(0), ParameterChangeCheck::None));
    let read_model = UiReadModel::from_engine(&engine, UiProjectFileSpec::default());
    assert!(!read_model.snapshot_cache_is_dirty_for_tests());

    let cursor = read_model.current_event_time();
    let root = engine.root;
    engine.add_node(
        Parameter::new("child", ParamValue::Int(1), ParameterChangeCheck::None),
        Some(root),
    );
    engine.apply_edits().expect("child should attach");
    let batch = read_model.publish_engine_events_since(&engine, cursor);

    assert!(!batch.events.is_empty());
    assert!(
        read_model.snapshot_cache_is_dirty_for_tests(),
        "structural publication must only invalidate the immutable snapshot cache"
    );

    let replay = read_model.replay(cursor, UiSubscriptionScope::WholeGraph);
    assert!(!replay.events.is_empty());
    assert!(
        read_model.snapshot_cache_is_dirty_for_tests(),
        "normal replay must not materialize a whole-graph snapshot"
    );

    let scoped = read_model.snapshot_for_scope(UiSubscriptionScope::Subtree { root, max_depth: 0 });
    assert_eq!(scoped.nodes.len(), 1);
    assert!(
        read_model.snapshot_cache_is_dirty_for_tests(),
        "a bounded subtree snapshot must scale with its scope"
    );

    let whole = read_model.snapshot_for_scope(UiSubscriptionScope::WholeGraph);
    assert_eq!(whole.nodes.len(), 2);
    assert!(!read_model.snapshot_cache_is_dirty_for_tests());
}

#[test]
fn concurrent_snapshot_consumers_share_one_lazy_materialization() {
    let mut engine = Engine::new(Parameter::new("root", ParamValue::Int(0), ParameterChangeCheck::None));
    let read_model = Arc::new(UiReadModel::from_engine(&engine, UiProjectFileSpec::default()));
    let cursor = read_model.current_event_time();
    let root = engine.root;
    engine.add_node(
        Parameter::new("child", ParamValue::Int(1), ParameterChangeCheck::None),
        Some(root),
    );
    engine.apply_edits().expect("child should attach");
    read_model.publish_engine_events_since(&engine, cursor);
    assert!(read_model.snapshot_cache_is_dirty_for_tests());

    let worker_count = 8;
    let barrier = Arc::new(Barrier::new(worker_count));
    let workers: Vec<_> = (0..worker_count)
        .map(|_| {
            let read_model = read_model.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                read_model.current_snapshot()
            })
        })
        .collect();
    let snapshots: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().expect("snapshot worker should finish"))
        .collect();

    assert!(snapshots.iter().all(|snapshot| snapshot.nodes.len() == 2));
    assert!(
        snapshots
            .iter()
            .skip(1)
            .all(|snapshot| Arc::ptr_eq(&snapshots[0], snapshot)),
        "concurrent readers should reuse the same cached immutable snapshot"
    );
}

#[test]
fn scoped_replay_parent_index_tracks_node_moves() {
    let mut engine = Engine::new(Parameter::new("root", ParamValue::Int(0), ParameterChangeCheck::None));
    let root = engine.root;
    engine.add_node(
        Parameter::new("left", ParamValue::Int(0), ParameterChangeCheck::None),
        Some(root),
    );
    engine.add_node(
        Parameter::new("right", ParamValue::Int(0), ParameterChangeCheck::None),
        Some(root),
    );
    engine.apply_edits().expect("branches should attach");

    let initial = engine.ui_snapshot(UiSubscriptionScope::WholeGraph);
    let root_node = initial
        .nodes
        .iter()
        .find(|node| node.node_id == root)
        .expect("root should exist");
    let left = root_node.children[0];
    let right = root_node.children[1];
    engine.add_node(
        Parameter::new("leaf", ParamValue::Int(0), ParameterChangeCheck::None),
        Some(left),
    );
    engine.apply_edits().expect("leaf should attach");
    let leaf = engine
        .ui_snapshot(UiSubscriptionScope::Subtree {
            root: left,
            max_depth: 1,
        })
        .nodes
        .into_iter()
        .find(|node| node.node_id != left)
        .expect("leaf should exist")
        .node_id;

    let read_model = UiReadModel::from_engine(&engine, UiProjectFileSpec::default());
    let move_cursor = read_model.current_event_time();
    let move_ack = engine.apply_ui_intent(UiEditIntent::MoveNode {
        node: leaf,
        new_parent: right,
        new_prev_sibling: None,
    });
    assert!(move_ack.success, "leaf move should apply: {:?}", move_ack.error_message);
    read_model.publish_engine_events_since(&engine, move_cursor);

    let value_cursor = read_model.current_event_time();
    let value_ack = engine.apply_ui_intent(UiEditIntent::SetParam {
        node: leaf,
        value: ParamValue::Int(9),
        behaviour: ParameterEventBehaviour::Coalesce,
    });
    assert!(
        value_ack.success,
        "leaf value should apply: {:?}",
        value_ack.error_message
    );
    read_model.publish_engine_events_since(&engine, value_cursor);

    let left_replay = read_model.replay(
        value_cursor,
        UiSubscriptionScope::Subtree {
            root: left,
            max_depth: u32::MAX,
        },
    );
    assert!(batch_param_values(&left_replay).is_empty());

    let right_replay = read_model.replay(
        value_cursor,
        UiSubscriptionScope::Subtree {
            root: right,
            max_depth: u32::MAX,
        },
    );
    assert_eq!(batch_param_values(&right_replay), vec![9]);

    let right_snapshot = read_model.snapshot_for_scope(UiSubscriptionScope::Subtree {
        root: right,
        max_depth: 1,
    });
    assert_eq!(
        right_snapshot
            .nodes
            .iter()
            .map(|node| node.node_id)
            .collect::<HashSet<_>>(),
        HashSet::from([right, leaf])
    );
}
