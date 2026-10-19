use super::*;

#[test]
fn full_completion_mailbox_preserves_logs_duplex_input_and_retiring_frames_without_replay() {
    let pool = pool();
    let mut handle = spawn(config(0), &pool);
    let address = listening(&handle);
    let mut peer = connect(&handle, address, "/logging");
    for byte in 0..100 {
        handle.send(StreamingSendFrameKind::Binary, &[byte], true).unwrap();
    }
    wait(|| handle.worker.event_metrics().rejected > 0);
    assert_eq!(handle.worker.event_metrics().depth, 64);
    peer.send(Message::text("independent")).unwrap();
    let mut logs = Vec::new();
    let mut received = 0;
    wait(|| {
        let mut events = Vec::new();
        handle.drain_events(&mut events);
        collect(events, &mut logs, &mut received);
        received == 1
    });
    handle.finish_admission();
    assert!(handle.send(StreamingSendFrameKind::Text, b"late", true).is_err());
    wait(|| {
        let mut events = Vec::new();
        let drain = handle.drain_events(&mut events);
        collect(events, &mut logs, &mut received);
        drain.state == golden_io::PendingDrainState::Disconnected
    });
    for byte in 0..100 {
        assert_eq!(peer.read().unwrap().into_data().as_ref(), &[byte]);
    }
    assert!(
        matches!(peer.read().unwrap(), Message::Close(_)),
        "no replayed or unadmitted frames"
    );
    let expected = (0..100)
        .map(|byte| crate::app::module::common::streaming::outgoing::broadcast_log("WebSocket server", &[byte], 1, 1))
        .collect::<Vec<_>>();
    assert_eq!(logs, expected);
    assert_eq!(received, 1);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}
fn collect(events: Vec<Event>, logs: &mut Vec<String>, received: &mut usize) {
    for event in events {
        match event {
            Event::OutgoingLog(message) => logs.push(message),
            Event::Input(_) => *received += 1,
            Event::Error(message) => panic!("{message}"),
            _ => {}
        }
    }
}
