use crate::{
    engine::Engine,
    events::{CustomEvent, EventKind},
    node::Folder,
    ui_read_model::UiReadModel,
    ui_sync::{UiEventKind, UiProjectFileSpec, UiSubscriptionScope},
};
use std::sync::Arc;

fn push(engine: &mut Engine<Folder>, topic: &str, bytes: usize, latest: bool) {
    let event = if latest {
        CustomEvent::latest(topic, Some(engine.root), serde_json::json!(topic))
    } else {
        CustomEvent::new(topic, Some(engine.root), serde_json::json!(topic))
    };
    engine.push_ui_event_kind(EventKind::Custom(event.with_payload_storage_weight(bytes)));
    engine.finish_ui_event_capture();
}

#[test]
fn byte_eviction_and_latest_replacement_keep_counters_indexes_and_clear_coherent() {
    let mut engine = Engine::new(Folder::new("root"));
    engine.clear_ui_event_log();
    engine.set_ui_event_log_custom_byte_capacity(3000);
    push(&mut engine, "latest", 1200, true);
    push(&mut engine, "first", 2000, false);
    assert_eq!(engine.ui_event_log_custom_bytes(), 2000);
    assert_eq!(engine.ui_event_index_sizes_for_tests().0, 0);
    push(&mut engine, "second", 2000, false);
    push(&mut engine, "latest", 500, true);
    push(&mut engine, "latest", 1000, true);
    assert_eq!(engine.ui_event_log().len(), 2);
    assert_eq!(engine.ui_event_log_custom_bytes(), 3000);
    assert_eq!(engine.ui_event_index_sizes_for_tests().0, 1);
    engine.clear_ui_event_log();
    assert_eq!(engine.ui_event_log_custom_bytes(), 0);
    assert_eq!(engine.ui_event_index_sizes_for_tests(), (0, 0));
}

#[test]
fn logical_byte_eviction_releases_payload_even_before_lazy_prefix_compaction() {
    let mut engine = Engine::new(Folder::new("root"));
    engine.clear_ui_event_log();
    engine.set_ui_event_log_custom_byte_capacity(2500);
    let payload = Arc::new(serde_json::json!("old"));
    let weak = Arc::downgrade(&payload);
    let event = CustomEvent::shared("first", None, payload, crate::events::CustomEventRetention::Replay)
        .with_payload_storage_weight(1500);
    engine.push_ui_event_kind(EventKind::Custom(event));
    engine.finish_ui_event_capture();
    for index in 0..10 {
        push(&mut engine, &format!("small-{index}"), 64, false);
    }
    push(&mut engine, "large", 1500, false);
    assert!(weak.upgrade().is_none(), "evicted prefix must not pin the old payload");
    assert!(
        engine.ui_event_log_start > 0,
        "fixture must leave an uncompacted prefix"
    );
    assert_eq!(engine.ui_event_log_custom_bytes(), 2140);
}

#[test]
fn read_model_eviction_uses_captured_weights_and_marks_replay_cursor_expired() {
    let mut engine = Engine::new(Folder::new("root"));
    engine.clear_ui_event_log();
    engine.set_ui_event_log_custom_byte_capacity(2500);
    let model = UiReadModel::from_engine(&engine, UiProjectFileSpec::default());
    let origin = engine.time;
    for topic in ["one", "two", "three", "four"] {
        let before = engine.ui_event_log().last().map(|event| event.time);
        push(&mut engine, topic, 1200, false);
        let capture = model.collect_event_batch(&mut engine, before);
        let live = model.apply_event_capture(capture);
        assert_eq!(
            live.events.len(),
            1,
            "live publication must not be lost to replay eviction"
        );
    }
    assert_eq!(model.replay(None, UiSubscriptionScope::WholeGraph).events.len(), 2);
    let expired = model.replay(Some(origin), UiSubscriptionScope::WholeGraph);
    assert!(expired.events.iter().any(
        |event| matches!(&event.kind, UiEventKind::Custom { topic, .. } if topic == "__transport.resync_required")
    ));
}

#[test]
fn oversized_single_entry_is_forwarded_then_evicted_by_the_next_publication() {
    let mut engine = Engine::new(Folder::new("root"));
    engine.clear_ui_event_log();
    engine.set_ui_event_log_custom_byte_capacity(100);
    let model = UiReadModel::from_engine(&engine, UiProjectFileSpec::default());
    push(&mut engine, "oversized", 1000, false);
    let live = model.publish_engine_events_since(&mut engine, None);
    assert_eq!(live.events.len(), 1);
    let before = engine.ui_event_log().last().map(|event| event.time);
    push(&mut engine, "next", 50, false);
    model.publish_engine_events_since(&mut engine, before);
    assert_eq!(engine.ui_event_log_custom_bytes(), 50);
    assert_eq!(model.replay(None, UiSubscriptionScope::WholeGraph).events.len(), 1);
    assert!(matches!(&engine.ui_event_log()[0].kind, EventKind::Custom(custom) if custom.topic == "next"));
}

#[test]
fn one_capture_forwards_the_entire_live_batch_before_byte_eviction() {
    let mut engine = Engine::new(Folder::new("root"));
    engine.clear_ui_event_log();
    engine.set_ui_event_log_custom_byte_capacity(2500);
    let model = UiReadModel::from_engine(&engine, UiProjectFileSpec::default());
    for index in 0..8 {
        engine.push_ui_event_kind(EventKind::Custom(
            CustomEvent::new("occurrence", None, serde_json::json!(index)).with_payload_storage_weight(1200),
        ));
    }
    assert_eq!(engine.ui_event_log_custom_bytes(), 9600);
    let capture = model.collect_event_batch(&mut engine, None);
    assert_eq!(engine.ui_event_log_custom_bytes(), 2400);
    let live = model.apply_event_capture(capture);
    let values: Vec<_> = live
        .events
        .iter()
        .filter_map(|event| match &event.kind {
            UiEventKind::Custom { payload, .. } => payload.as_u64(),
            _ => None,
        })
        .collect();
    assert_eq!(values, (0..8).collect::<Vec<_>>());
    assert_eq!(model.replay(None, UiSubscriptionScope::WholeGraph).events.len(), 2);
}

#[test]
fn lowering_byte_capacity_trims_read_model_even_without_new_events() {
    let mut engine = Engine::new(Folder::new("root"));
    engine.clear_ui_event_log();
    let model = UiReadModel::from_engine(&engine, UiProjectFileSpec::default());
    push(&mut engine, "one", 1200, false);
    push(&mut engine, "two", 1200, false);
    let batch = model.publish_engine_events_since(&mut engine, None);
    engine.set_ui_event_log_custom_byte_capacity(1500);
    let empty = model.publish_engine_events_since(&mut engine, batch.to);
    assert!(empty.events.is_empty());
    assert_eq!(model.replay(None, UiSubscriptionScope::WholeGraph).events.len(), 1);
    assert!(model.last_evicted_event_time().is_some());
}

#[test]
fn oversized_weights_do_not_overflow_accounting_and_latest_replacement_recovers_it() {
    let mut engine = Engine::new(Folder::new("root"));
    engine.clear_ui_event_log();
    let model = UiReadModel::from_engine(&engine, UiProjectFileSpec::default());
    for _ in 0..2 {
        engine.push_ui_event_kind(EventKind::Custom(
            CustomEvent::new("oversized", None, serde_json::Value::Null).with_payload_storage_weight(usize::MAX),
        ));
    }
    let live = model.publish_engine_events_since(&mut engine, None);
    assert_eq!(live.events.len(), 2);
    assert_eq!(engine.ui_event_log_custom_bytes(), usize::MAX);
    let before = live.to;
    push(&mut engine, "latest", 1200, true);
    let next = model.publish_engine_events_since(&mut engine, before);
    assert_eq!(engine.ui_event_log_custom_bytes(), 1200);
    push(&mut engine, "latest", 2400, true);
    model.publish_engine_events_since(&mut engine, next.to);
    assert_eq!(engine.ui_event_log_custom_bytes(), 2400);
    assert_eq!(model.replay(None, UiSubscriptionScope::WholeGraph).events.len(), 1);
}
