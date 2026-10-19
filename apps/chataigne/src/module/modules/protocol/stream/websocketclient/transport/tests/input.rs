use super::*;

fn next_input(handle: &WebSocketClientTransportHandle) -> RetainedPayload<PreparedStreamInput> {
    let mut packet = None;
    wait(|| {
        let mut events = Vec::new();
        handle.drain_events(&mut events);
        for event in events {
            match event {
                StreamingWorkerEvent::Input(input) => packet = Some(input),
                StreamingWorkerEvent::Error(error) => panic!("{error}"),
                _ => {}
            }
        }
        packet.is_some()
    });
    packet.unwrap()
}

#[test]
fn text_and_binary_frames_keep_distinct_callback_contracts_and_worker_prepared_values() {
    let pool = pool();
    let (mut handle, mut peer) = connected(&pool);
    peer.send(Message::text("level 7\n")).unwrap();
    let text = next_input(&handle);
    assert_eq!(text.values.as_ref().unwrap()[0].path_segments, ["level"]);
    assert_eq!(text.callbacks.len(), 3);
    assert_eq!(text.callbacks[2].payload()["callback"], "textReceived");
    peer.send(Message::binary(b"other 8\n".to_vec())).unwrap();
    let binary = next_input(&handle);
    assert_eq!(binary.values.as_ref().unwrap()[0].path_segments, ["other"]);
    assert_eq!(binary.callbacks.len(), 2);
    assert_eq!(binary.callbacks[0].payload()["args"][1], serde_json::Value::Null);
    handle.stop();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn grammar_and_processing_updates_fence_partial_frames_without_reconnecting() {
    let pool = pool();
    let (mut handle, mut peer) = connected(&pool);
    peer.send(Message::text("old ")).unwrap();
    assert!(next_input(&handle).values.as_ref().unwrap().is_empty());
    let mut input = handle.input.snapshot().config;
    input.parse.name_separator = Some(crate::app::module::common::streaming::parser::StreamingSeparator::Colon);
    input.log_incoming = true;
    handle.update_input(input.clone()).unwrap();
    peer.send(Message::text("new:2\n")).unwrap();
    let new = next_input(&handle);
    assert_eq!(new.rejected_lines, 1);
    assert_eq!(new.values.as_ref().unwrap()[0].path_segments, ["new"]);
    assert!(new.log_message.as_ref().unwrap().starts_with("Received WebSocket "));
    peer.send(Message::text("abandoned:")).unwrap();
    next_input(&handle);
    input.processing_enabled = false;
    handle.update_input(input.clone()).unwrap();
    input.processing_enabled = true;
    handle.update_input(input.clone()).unwrap();
    peer.send(Message::text("fresh:3\n")).unwrap();
    assert_eq!(next_input(&handle).rejected_lines, 1);
    let generation = handle.input.snapshot().generation;
    input.parse.mode.reserve(17);
    assert!(handle.update_input(input).is_err());
    assert_eq!(handle.input.snapshot().generation, generation);
    assert_eq!(handle.connection.load(Ordering::Acquire), 1);
    handle.stop();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn saturated_input_reply_publication_leaves_native_output_and_shutdown_runnable() {
    let pool = pool();
    let (mut handle, mut peer) = connected(&pool);
    let mut reader = WebSocket::from_raw_socket(peer.get_ref().try_clone().unwrap(), Role::Server, None);
    let producer = thread::spawn(move || {
        for _ in 0..256 {
            if peer.send(Message::text("value 7\n")).is_err() {
                break;
            }
        }
    });
    wait(|| handle.worker.event_metrics().rejected > 0);
    let metrics = handle.worker.event_metrics();
    assert!(metrics.depth <= 64 && metrics.retained_weight <= EVENT_BYTES);
    handle
        .send(StreamingSendFrameKind::Text, b"independent", false)
        .unwrap();
    assert_eq!(reader.read().unwrap(), Message::text("independent"));
    let started = Instant::now();
    handle.stop();
    assert!(started.elapsed() < Duration::from_millis(100));
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    producer.join().unwrap();
}

#[test]
fn ping_and_close_control_frames_are_serviced_when_application_receive_is_disabled() {
    let pool = pool();
    let (client, mut peer) = pair();
    let mut client = Some(client);
    let mut settings = config();
    settings.receive_enabled = false;
    let mut handle = WebSocketClientTransportHandle::spawn_with_connector(settings, &pool, notifier(), move |_, _| {
        client.take().ok_or("no replacement".into())
    })
    .unwrap();
    wait(|| handle.connection.load(Ordering::Acquire) == 1);
    peer.send(Message::Ping(vec![1, 2].into())).unwrap();
    assert_eq!(peer.read().unwrap(), Message::Pong(vec![1, 2].into()));
    peer.close(None).unwrap();
    assert!(matches!(peer.read().unwrap(), Message::Close(_)));
    wait(|| handle.connection.load(Ordering::Acquire) == 0);
    handle.stop();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn callback_complexity_rejection_discards_partial_input_and_the_next_frame_recovers() {
    let pool = pool();
    let (mut handle, mut peer) = connected(&pool);
    peer.send(Message::text("old ")).unwrap();
    assert!(next_input(&handle).values.as_ref().unwrap().is_empty());
    peer.send(Message::binary(vec![0; 300_000])).unwrap();
    let mut events = Vec::new();
    wait(|| {
        handle.drain_events(&mut events);
        events
            .iter()
            .any(|event| matches!(event, StreamingWorkerEvent::Error(error) if error.contains("callback")))
    });
    assert!(!events
        .iter()
        .any(|event| matches!(event, StreamingWorkerEvent::Input(_))));
    assert_eq!(handle.connection.load(Ordering::Acquire), 1);
    peer.send(Message::text("fresh 7\n")).unwrap();
    let fresh = next_input(&handle);
    assert_eq!(fresh.rejected_lines, 1);
    assert_eq!(fresh.values.as_ref().unwrap()[0].path_segments, ["fresh"]);
    handle.stop();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn oversized_wire_frames_close_the_original_connection_before_application_delivery() {
    let pool = pool();
    let (mut handle, mut peer) = connected(&pool);
    let _ = peer.send(Message::binary(vec![0; 1024 * 1024 + 1]));
    let mut events = Vec::new();
    wait(|| {
        handle.drain_events(&mut events);
        events.iter().any(|event| matches!(event, StreamingWorkerEvent::Status(WebSocketClientConnectionStatus::Recovering { message }) if message.contains("too long")))
    });
    assert!(!events
        .iter()
        .any(|event| matches!(event, StreamingWorkerEvent::Input(_))));
    assert_eq!(handle.connection.load(Ordering::Acquire), 0);
    handle.stop();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}
