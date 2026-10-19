use super::*;
use crate::app::AppEngine;
use golden_core::{
    edit::{Edit, EditRequest},
    events::EventKind,
    node::{Folder, NodeId},
    parameter::{Parameter, ParameterChangeCheck, ParameterEventBehaviour},
    process_ctx::{ExecutionPhase, ProcessCtx},
};

fn engine_with_parameter(label: &str, value: ParamValue) -> (AppEngine, NodeId) {
    let mut engine = AppEngine::new(Folder::new("root").into());
    engine.add_node(
        Parameter::new(label, value, ParameterChangeCheck::ValueChange).into(),
        None,
    );
    engine.apply_edits().unwrap();
    let id = engine.process_tree_snapshot().find_child(engine.root, label).unwrap();
    (engine, id)
}

fn plan(queue: &mut StreamingIncomingQueue, engine: &AppEngine) -> ProcessCtx {
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    queue.process_pending(&mut ctx, &engine.process_tree_snapshot(), Some(engine.root), true);
    ctx
}

fn apply(engine: &mut AppEngine, mut ctx: ProcessCtx) {
    engine.absorb_edits(&mut ctx).unwrap();
    engine.apply_edits().unwrap();
}

#[test]
fn stream_admission_rejects_an_entire_batch_without_losing_the_accepted_head() {
    let mut queue = StreamingIncomingQueue::with_limits(2, 4096);
    queue.push_messages(vec![message("head")].try_into().unwrap());
    queue.push_messages(vec![message("reject1"), message("reject2")].try_into().unwrap());
    assert_eq!(queue.pending_message_count(), 1);
    assert_eq!(queue.take_dropped_message_count(), 2);
    assert_eq!(queue.take_pending_messages_for_test(), [message("head")]);
}

#[test]
fn stream_admission_counts_inflight_receipts_until_completion() {
    let mut engine = AppEngine::new(Folder::new("root").into());
    let mut queue = StreamingIncomingQueue::with_limits(1, 4096);
    queue.push_messages(vec![message("head")].try_into().unwrap());
    let ctx = plan(&mut queue, &engine);
    queue.push_messages(vec![message("too_early")].try_into().unwrap());
    assert_eq!(queue.pending_message_count(), 1);
    assert_eq!(queue.take_dropped_message_count(), 1);
    assert!(plan(&mut queue, &engine).edits.pending.is_empty());
    apply(&mut engine, ctx);
    assert!(plan(&mut queue, &engine).edits.pending.is_empty());
    assert!(!queue.has_pending_messages());
    queue.push_messages(vec![message("next")].try_into().unwrap());
    assert_eq!(queue.pending_message_count(), 1);
    assert_eq!(queue.take_dropped_message_count(), 0);
}

#[test]
fn stream_cursor_preserves_repeated_existing_values_within_one_turn() {
    let (mut engine, id) = engine_with_parameter("level", ParamValue::Int(0));
    let mut queue = StreamingIncomingQueue::new();
    let first = message("level");
    let mut second = first.clone();
    second.payload = ReceivedValuePayload::Single(ParamValue::Int(0));
    queue.push_messages(vec![first, second].try_into().unwrap());
    let ctx = plan(&mut queue, &engine);
    assert_eq!(ctx.edits.pending.len(), 2);
    apply(&mut engine, ctx);
    assert_eq!(
        engine.process_tree_snapshot().node(id).unwrap().param_value,
        Some(ParamValue::Int(0))
    );
    assert!(plan(&mut queue, &engine).edits.pending.is_empty());
    assert!(!queue.has_pending_messages());
}

#[test]
fn stream_cursor_allows_one_admitted_scalar_above_the_turn_byte_target() {
    let mut engine = AppEngine::new(Folder::new("root").into());
    let mut queue = StreamingIncomingQueue::new();
    let mut input = message("large");
    input.payload = ReceivedValuePayload::Single(ParamValue::Str("x".repeat(256 * 1024)));
    queue.push_messages(vec![input].try_into().unwrap());
    let ctx = plan(&mut queue, &engine);
    assert_eq!(
        ctx.edits.pending.len(),
        1,
        "bounded admission must not strand a large scalar"
    );
    apply(&mut engine, ctx);
    plan(&mut queue, &engine);
    assert!(!queue.has_pending_messages());
    let snapshot = engine.process_tree_snapshot();
    let id = snapshot.find_child(engine.root, "large").unwrap();
    assert!(
        matches!(&snapshot.node(id).unwrap().param_value, Some(ParamValue::Str(value)) if value.len() == 256 * 1024)
    );
}

#[test]
fn stream_cursor_materializes_large_lists_in_bounded_turns() {
    let mut engine = AppEngine::new(Folder::new("root").into());
    let mut queue = StreamingIncomingQueue::new();
    let mut input = message("list");
    input.payload = ReceivedValuePayload::Multi((0..1000).map(ParamValue::Int).collect());
    queue.push_messages(vec![input].try_into().unwrap());
    let mut turns = 0;
    while queue.has_pending_messages() {
        let ctx = plan(&mut queue, &engine);
        assert!(ctx.edits.pending.len() <= 256);
        apply(&mut engine, ctx);
        turns += 1;
        assert!(turns < 100, "continuation must make progress");
    }
    assert!(turns > 1);
    let snapshot = engine.process_tree_snapshot();
    let list = snapshot.find_child(engine.root, "list").unwrap();
    assert_eq!(snapshot.child_ids(list).len(), 1000);
}

#[test]
fn maximum_udp_raw_batch_is_admitted_and_applied_in_order_without_truncation() {
    use crate::app::module::common::streaming::parser::{
        StreamingParseConfig, StreamingParser, STREAMING_INPUT_MODE_RAW,
    };
    let (mut engine, id) = engine_with_parameter("byte", ParamValue::Int(-1));
    let bytes: Vec<_> = (0..65_507).map(|index| (index % 256) as u8).collect();
    let messages = StreamingParser::default()
        .push_bytes(
            &bytes,
            &StreamingParseConfig {
                mode: STREAMING_INPUT_MODE_RAW.into(),
                ..Default::default()
            },
        )
        .unwrap();
    let mut queue = StreamingIncomingQueue::new();
    queue.push_messages(messages);
    assert_eq!(queue.pending_message_count(), bytes.len());
    assert_eq!(queue.take_dropped_message_count(), 0);
    let mut applied = Vec::new();
    let mut turns = 0;
    while queue.has_pending_messages() {
        let ctx = plan(&mut queue, &engine);
        assert!(ctx.edits.pending.len() <= 256);
        for request in &ctx.edits.pending {
            if let Edit::SetParam {
                node,
                value: ParamValue::Int(value),
                ..
            } = request.edit
            {
                assert_eq!(node, id);
                applied.push(value);
            }
        }
        apply(&mut engine, ctx);
        turns += 1;
        assert!(turns < 2000);
    }
    assert_eq!(applied, bytes.iter().map(|byte| i32::from(*byte)).collect::<Vec<_>>());
}

#[test]
fn stream_graph_failure_does_not_replay_an_applied_trigger_prefix() {
    let (mut engine, id) = engine_with_parameter("pulse", ParamValue::Trigger());
    let mut queue = StreamingIncomingQueue::new();
    let mut pulse = message("pulse");
    pulse.payload = ReceivedValuePayload::Single(ParamValue::Trigger());
    queue.push_messages(vec![pulse.clone(), pulse].try_into().unwrap());
    let mut ctx = plan(&mut queue, &engine);
    assert_eq!(ctx.edits.pending.len(), 2);
    let before = engine
        .ui_event_log()
        .iter()
        .filter(|event| matches!(event.kind, EventKind::ParamChanged { param, .. } if param == id))
        .count();
    ctx.edits.pending.insert(
        1,
        EditRequest::new(Edit::SetParam {
            node: NodeId(u64::MAX),
            value: ParamValue::Int(1),
            behaviour: ParameterEventBehaviour::Append,
        }),
    );
    engine.absorb_edits(&mut ctx).unwrap();
    assert!(engine.apply_edits().is_err());
    assert!(plan(&mut queue, &engine).edits.pending.is_empty());
    assert_eq!(queue.take_failed_batch_count(), 1);
    assert_eq!(queue.take_failed_batch_count(), 0);
    assert!(!queue.has_pending_messages());
    let after = engine
        .ui_event_log()
        .iter()
        .filter(|event| matches!(event.kind, EventKind::ParamChanged { param, .. } if param == id))
        .count();
    assert_eq!(after, before + 1);
    queue.push_messages(vec![message("next")].try_into().unwrap());
    let ctx = plan(&mut queue, &engine);
    apply(&mut engine, ctx);
    plan(&mut queue, &engine);
    assert!(!queue.has_pending_messages(), "later work recovers after failure");
}

#[test]
fn stream_dropped_edit_context_fails_receipt_without_retry() {
    let mut engine = AppEngine::new(Folder::new("root").into());
    let mut queue = StreamingIncomingQueue::new();
    queue.push_messages(vec![message("discarded")].try_into().unwrap());
    drop(plan(&mut queue, &engine));
    let ctx = plan(&mut queue, &engine);
    assert!(ctx.edits.pending.is_empty());
    assert_eq!(queue.take_failed_batch_count(), 1);
    assert!(!queue.has_pending_messages());
    apply(&mut engine, ctx);
    assert!(engine
        .process_tree_snapshot()
        .find_child(engine.root, "discarded")
        .is_none());
}
