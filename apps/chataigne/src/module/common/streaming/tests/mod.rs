use golden_core::parameter::ParamValue;

use crate::app::module::common::{
    received_values::ReceivedValuePayload,
    streaming::{module_helpers::StreamingIncomingQueue, parser::StreamingIncomingMessage},
};

#[test]
fn incoming_stream_queue_applies_bounded_backpressure_without_reordering() {
    let mut queue = StreamingIncomingQueue::with_limits(2, 4);
    queue.push_messages(vec![message("first"), message("second"), message("third")]);

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

fn message(path: &str) -> StreamingIncomingMessage {
    StreamingIncomingMessage {
        path_segments: vec![path.to_string()],
        payload: ReceivedValuePayload::Single(ParamValue::Int(1)),
        source_description: "bounded queue fixture".to_string(),
    }
}
