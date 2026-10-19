use super::*;
use crate::app::module::common::streaming::parser::StreamingSeparator;
use golden_io::RetainedPayload;

fn next_input(handle: &TcpStreamingTransportHandle) -> RetainedPayload<PreparedStreamInput> {
    let mut result = None;
    wait(|| {
        let mut events = Vec::new();
        handle.drain_events(&mut events);
        for event in events {
            match event {
                StreamingWorkerEvent::Input(input) => {
                    assert!(result.is_none(), "fixture sends one chunk at a time");
                    result = Some(input);
                }
                StreamingWorkerEvent::Error(error) => panic!("{error}"),
                _ => {}
            }
        }
        result.is_some()
    });
    result.unwrap()
}

fn receiving(pool: &RetirementPool) -> (TcpStreamingTransportHandle, TcpStream) {
    let (client, peer) = pair();
    let mut client = Some(client);
    let mut config = config();
    config.receive_enabled = true;
    let handle = TcpStreamingTransportHandle::spawn_with_connector(config, pool, notifier(), move |_| {
        client.take().ok_or("no replacement".into())
    })
    .unwrap();
    wait(|| handle.connection.load(Ordering::Acquire) == 1);
    (handle, peer)
}

#[test]
fn prepared_values_callbacks_and_logs_keep_their_native_input_configuration() {
    let pool = pool();
    let (mut handle, mut peer) = receiving(&pool);
    let mut input = handle.input.snapshot().config;
    input.log_incoming = true;
    handle.update_input(input.clone());
    peer.write_all(b"old 1\n").unwrap();
    let old = next_input(&handle);
    assert_eq!(old.values.as_ref().unwrap()[0].path_segments, ["old"]);
    assert_eq!(old.callbacks.len(), 3);
    assert_eq!(old.callbacks[0].payload["callback"], "dataReceived");
    assert_eq!(old.callbacks[2].payload["args"][0], "old 1\n");
    assert!(old.log_message.as_ref().unwrap().starts_with("Received TCP "));

    input.parse.mode = "json".into();
    input.log_incoming = false;
    handle.update_input(input);
    peer.write_all(br#"{"new":2}"#).unwrap();
    let new = next_input(&handle);
    assert_eq!(new.values.as_ref().unwrap()[0].path_segments, ["new"]);
    assert!(new.log_message.is_none());
    assert_eq!(old.values.as_ref().unwrap()[0].path_segments, ["old"]);
    assert!(old.log_message.is_some());
    handle.stop();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn grammar_and_processing_generations_reject_partial_frames_without_reconnecting() {
    let pool = pool();
    let (mut handle, mut peer) = receiving(&pool);
    peer.write_all(b"old ").unwrap();
    assert!(next_input(&handle).values.as_ref().unwrap().is_empty());
    let mut input = handle.input.snapshot().config;
    input.parse.name_separator = Some(StreamingSeparator::Colon);
    handle.update_input(input.clone());
    peer.write_all(b"new:2\n").unwrap();
    let new = next_input(&handle);
    assert_eq!(new.rejected_lines, 1);
    assert_eq!(new.values.as_ref().unwrap()[0].path_segments, ["new"]);

    peer.write_all(b"abandoned:").unwrap();
    assert!(next_input(&handle).values.as_ref().unwrap().is_empty());
    input.processing_enabled = false;
    handle.update_input(input.clone());
    // No bytes need arrive while disabled for the generation boundary to apply.
    input.processing_enabled = true;
    handle.update_input(input);
    peer.write_all(b"fresh:3\n").unwrap();
    let fresh = next_input(&handle);
    assert_eq!(fresh.rejected_lines, 1);
    assert_eq!(fresh.values.as_ref().unwrap()[0].path_segments, ["fresh"]);
    assert_eq!(handle.connection.load(Ordering::Acquire), 1);
    handle.stop();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn log_only_changes_preserve_a_partial_frame_and_disabled_processing_keeps_callbacks() {
    let pool = pool();
    let (mut handle, mut peer) = receiving(&pool);
    peer.write_all(b"kept ").unwrap();
    assert!(next_input(&handle).values.as_ref().unwrap().is_empty());
    let mut input = handle.input.snapshot().config;
    input.log_incoming = true;
    handle.update_input(input.clone());
    peer.write_all(b"7\n").unwrap();
    let kept = next_input(&handle);
    assert_eq!(kept.rejected_lines, 0);
    assert_eq!(kept.values.as_ref().unwrap()[0].path_segments, ["kept"]);
    input.processing_enabled = false;
    handle.update_input(input);
    peer.write_all(b"ignored 4\n").unwrap();
    let ignored = next_input(&handle);
    assert!(ignored.values.as_ref().unwrap().is_empty());
    assert_eq!(ignored.callbacks.len(), 3);
    assert!(ignored.log_message.as_ref().unwrap().ends_with("(processing disabled)"));
    handle.stop();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn reconnect_rejects_a_partial_frame_even_before_the_new_peer_sends() {
    let pool = pool();
    let (first, mut first_peer) = pair();
    let (second, mut second_peer) = pair();
    let mut sockets = std::collections::VecDeque::from([first, second]);
    let mut config = config();
    config.receive_enabled = true;
    let mut handle = TcpStreamingTransportHandle::spawn_with_connector(config, &pool, notifier(), move |_| {
        sockets.pop_front().ok_or("no replacement".into())
    })
    .unwrap();
    wait(|| handle.connection.load(Ordering::Acquire) == 1);
    first_peer.write_all(b"old ").unwrap();
    assert!(next_input(&handle).values.as_ref().unwrap().is_empty());
    drop(first_peer);
    wait(|| handle.connection.load(Ordering::Acquire) == 2);
    let mut events = Vec::new();
    handle.drain_events(&mut events);
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, StreamingWorkerEvent::RejectedInput(1)))
            .count(),
        1
    );
    second_peer.write_all(b"new 8\n").unwrap();
    let new = next_input(&handle);
    assert_eq!(new.values.as_ref().unwrap()[0].path_segments, ["new"]);
    assert_eq!(new.rejected_lines, 0);
    handle.stop();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn retirement_reports_a_partial_frame_once_after_saturated_accepted_output() {
    let pool = pool();
    let (mut handle, mut peer) = receiving(&pool);
    peer.write_all(b"partial ").unwrap();
    assert!(next_input(&handle).values.as_ref().unwrap().is_empty());
    let mut accepted = 0;
    for _ in 0..1000 {
        if handle.send(vec![7], true).is_err() {
            break;
        }
        accepted += 1;
    }
    wait(|| handle.event_metrics().rejected > 0);
    handle.finish_admission();
    let events = drain(&handle);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, StreamingWorkerEvent::RejectedInput(1)))
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, StreamingWorkerEvent::OutgoingLog(_)))
            .count(),
        accepted
    );
    let mut bytes = Vec::new();
    peer.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, vec![7; accepted]);
}
