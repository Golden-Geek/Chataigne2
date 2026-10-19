use super::*;

#[test]
fn full_completion_mailbox_keeps_fifo_logs_independent_input_and_the_retiring_prefix() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let mut handle = spawn_using(config(0), &pool, worker::bind).unwrap();
    let address = listening(&handle);
    let mut peer = connect(&handle, address);
    for byte in 0..100 {
        handle.send(&[byte], true).unwrap();
    }
    wait(|| handle.worker.event_metrics().rejected > 0);
    assert_eq!(handle.worker.event_metrics().depth, 64);
    peer.write_all(b"independent\n").unwrap();
    let mut logs = Vec::new();
    let mut received = 0;
    wait(|| {
        let mut events = Vec::new();
        handle.drain_events(&mut events);
        collect(events, &mut logs, &mut received);
        received == 1
    });
    handle.finish_admission();
    assert!(handle.send(b"late", true).is_err());
    wait(|| {
        let mut events = Vec::new();
        let drain = handle.drain_events(&mut events);
        collect(events, &mut logs, &mut received);
        drain.state == golden_io::PendingDrainState::Disconnected
    });
    let mut bytes = Vec::new();
    peer.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, (0..100).collect::<Vec<u8>>());
    let expected = (0..100)
        .map(|byte| crate::app::module::common::streaming::outgoing::broadcast_log("TCP server", &[byte], 1, 1))
        .collect::<Vec<_>>();
    assert_eq!(logs, expected);
    assert_eq!(received, 1);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}
fn collect(events: Vec<TcpServerWorkerEvent>, logs: &mut Vec<String>, received: &mut usize) {
    for event in events {
        match event {
            TcpServerWorkerEvent::OutgoingLog(message) => logs.push(message),
            TcpServerWorkerEvent::Input(_) => *received += 1,
            TcpServerWorkerEvent::Error(message) => panic!("{message}"),
            _ => {}
        }
    }
}
