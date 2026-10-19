use golden_core::parameter::ParamValue;

mod application;

use crate::app::module::common::{
    received_values::{ReceivedValueMessage, ReceivedValuePayload},
    streaming::module_helpers::StreamingIncomingQueue,
};

#[test]
fn incoming_stream_queue_applies_bounded_backpressure_without_reordering() {
    let mut queue = StreamingIncomingQueue::with_limits(2, 4096);
    queue.push_messages(vec![message("first")]);
    queue.push_messages(vec![message("second")]);
    queue.push_messages(vec![message("third")]);

    assert_eq!(queue.pending_message_count(), 2);
    assert_eq!(queue.take_dropped_message_count(), 1);
    assert_eq!(queue.take_dropped_message_count(), 0);
    let paths = queue
        .take_pending_messages_for_test()
        .into_iter()
        .map(|message| message.path_segments[0].clone())
        .collect::<Vec<_>>();
    assert_eq!(paths, vec!["first", "second"]);
}

fn message(path: &str) -> ReceivedValueMessage {
    ReceivedValueMessage {
        path_segments: vec![path.to_string()],
        payload: ReceivedValuePayload::Single(ParamValue::Int(1)),
        source_description: "bounded queue fixture".to_string(),
    }
}

#[test]
fn incoming_stream_queue_accounts_for_retained_string_capacity() {
    let mut queue = StreamingIncomingQueue::with_limits(2, 1024);
    let mut text = String::with_capacity(4096);
    text.push('x');
    let mut input = message("large");
    input.payload = ReceivedValuePayload::Single(ParamValue::Str(text));
    queue.push_messages(vec![input]);
    assert_eq!(
        queue.pending_message_count(),
        0,
        "retained allocation must count even when string length is small"
    );
    assert_eq!(queue.take_dropped_message_count(), 1);
}

#[test]
fn incoming_stream_queue_retains_work_until_graph_application_completes() {
    use golden_core::{
        node::{Folder, Node},
        process_ctx::{ExecutionPhase, ProcessCtx},
    };
    let mut engine = crate::app::AppEngine::new(Folder::new("root").into());
    let mut queue = StreamingIncomingQueue::new();
    queue.push_messages(vec![message("created")]);
    let snapshot = engine.process_tree_snapshot();
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    queue.process_pending(&mut ctx, &snapshot, Some(engine.root), true);
    assert!(
        queue.has_pending_messages(),
        "a snapshot/planned edit does not acknowledge application"
    );
    engine.absorb_edits(&mut ctx).unwrap();
    engine.apply_edits().unwrap();
    let snapshot = engine.process_tree_snapshot();
    assert!(snapshot.find_child(engine.root, "created").is_some());
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    queue.process_pending(&mut ctx, &snapshot, Some(engine.root), true);
    assert!(!queue.has_pending_messages());
    assert!(
        ctx.edits.pending.is_empty(),
        "completion must not replay the applied value"
    );
    assert_eq!(engine.nodes.get(engine.root).unwrap().get_type(), "folder");
}

#[test]
fn incoming_stream_batch_preserves_an_ordered_return_to_the_snapshot_value() {
    use crate::app::module::common::received_values::{
        apply_received_value_batch, ReceivedValueBatchMessage, ReceivedValueBatchOptions,
    };
    use golden_core::{
        node::Folder,
        parameter::{Parameter, ParameterChangeCheck, ParameterEventBehaviour},
        process_ctx::{ExecutionPhase, ProcessCtx},
    };
    let mut engine = crate::app::AppEngine::new(Folder::new("root").into());
    engine.add_node(
        Parameter::new("level", ParamValue::Int(0), ParameterChangeCheck::ValueChange).into(),
        None,
    );
    engine.apply_edits().unwrap();
    let snapshot = engine.process_tree_snapshot();
    let level = snapshot.find_child(engine.root, "level").unwrap();
    let path = vec!["level".to_string()];
    let values = [
        ReceivedValuePayload::Single(ParamValue::Int(1)),
        ReceivedValuePayload::Single(ParamValue::Int(0)),
    ];
    let mut ctx = ProcessCtx::new(ExecutionPhase::EngineTick, engine.time);
    apply_received_value_batch(
        &mut ctx,
        &snapshot,
        engine.root,
        values.iter().map(|payload| ReceivedValueBatchMessage {
            path_segments: &path,
            payload,
            source_description: None,
        }),
        ReceivedValueBatchOptions {
            auto_add: true,
            event_behaviour: ParameterEventBehaviour::Append,
        },
    );
    engine.absorb_edits(&mut ctx).unwrap();
    engine.apply_edits().unwrap();
    assert_eq!(
        engine.process_tree_snapshot().node(level).unwrap().param_value,
        Some(ParamValue::Int(0))
    );
}
