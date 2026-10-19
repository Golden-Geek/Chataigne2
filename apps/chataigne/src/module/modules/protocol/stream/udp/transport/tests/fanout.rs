use super::*;

fn receiver() -> UdpSocket {
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    socket.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    socket
}

fn target(socket: &UdpSocket) -> UdpOutputTarget {
    UdpOutputTarget {
        remote_host: "127.0.0.1".into(),
        remote_port: socket.local_addr().unwrap().port(),
    }
}

fn assert_empty(socket: &UdpSocket) {
    socket.set_read_timeout(Some(Duration::from_millis(50))).unwrap();
    let error = socket.recv(&mut [0; 16]).unwrap_err();
    assert!(matches!(
        error.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
    ));
}

#[test]
fn graceful_retirement_finishes_fanout_across_turns_before_the_next_command() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let mut handle = UdpStreamingTransportHandle::spawn_with_setup(config(), &pool, notifier(), move || {
        release_rx.recv().unwrap();
        UdpSocket::bind("127.0.0.1:0").map_err(|error| error.to_string())
    })
    .unwrap();
    let receivers: Vec<_> = (0..40).map(|_| receiver()).collect();
    for value in [3, 7] {
        let bytes = vec![value; 512];
        let mut plan = OutputPlanBuilder::default();
        for socket in &receivers {
            plan.push_target(target(socket)).unwrap();
        }
        handle
            .send(UdpOutboundSend::new(bytes, plan.finish().unwrap(), false).unwrap())
            .unwrap();
    }
    handle.finish_admission();
    release_tx.send(()).unwrap();
    assert!(pool.wait_for_idle(Duration::from_secs(3)));
    for socket in &receivers {
        for value in [3, 7] {
            let mut bytes = [0; 512];
            assert_eq!(socket.recv(&mut bytes).unwrap(), 512);
            assert_eq!(bytes, [value; 512]);
        }
    }
}

#[test]
fn full_queue_rejects_the_whole_fanout_without_sending_a_destination_prefix() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let mut handle = UdpStreamingTransportHandle::spawn_with_setup(config(), &pool, notifier(), move || {
        release_rx.recv().unwrap();
        UdpSocket::bind("127.0.0.1:0").map_err(|error| error.to_string())
    })
    .unwrap();
    let accepted = receiver();
    let mut accepted_plan = OutputPlanBuilder::default();
    accepted_plan.push_target(target(&accepted)).unwrap();
    let accepted_plan = accepted_plan.finish().unwrap();
    for _ in 0..256 {
        handle
            .send(UdpOutboundSend::new(vec![1], accepted_plan.clone(), false).unwrap())
            .unwrap();
    }
    let first = receiver();
    let last = receiver();
    let bytes = vec![9];
    let mut plan = OutputPlanBuilder::default();
    plan.push_target(target(&first)).unwrap();
    plan.push_target(target(&last)).unwrap();
    assert!(handle
        .send(UdpOutboundSend::new(bytes, plan.finish().unwrap(), false).unwrap())
        .unwrap_err()
        .contains("whole command was not admitted"));
    handle.finish_admission();
    release_tx.send(()).unwrap();
    assert!(pool.wait_for_idle(Duration::from_secs(3)));
    assert_empty(&first);
    assert_empty(&last);
}

#[test]
fn invalid_late_destination_rejects_the_command_before_the_valid_prefix_is_sent() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let mut handle = UdpStreamingTransportHandle::spawn_with_setup(config(), &pool, notifier(), || {
        UdpSocket::bind("127.0.0.1:0").map_err(|error| error.to_string())
    })
    .unwrap();
    let socket = receiver();
    let mut plan = OutputPlanBuilder::default();
    plan.push_target(target(&socket)).unwrap();
    assert!(plan
        .push_target(UdpOutputTarget {
            remote_host: "x".repeat(1025),
            remote_port: 1
        })
        .is_err());
    assert!(plan.finish().is_err());
    handle.finish_admission();
    assert!(pool.wait_for_idle(Duration::from_secs(3)));
    assert_empty(&socket);
}

#[test]
fn native_destination_failure_reports_once_and_preserves_the_remaining_order() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let mut handle = UdpStreamingTransportHandle::spawn_with_setup(config(), &pool, notifier(), || {
        UdpSocket::bind("127.0.0.1:0").map_err(|error| error.to_string())
    })
    .unwrap();
    let socket = receiver();
    let bytes = vec![7];
    let mut plan = OutputPlanBuilder::default();
    plan.push_target(target(&socket)).unwrap();
    plan.push_target(UdpOutputTarget {
        remote_host: "::1".into(),
        remote_port: 1,
    })
    .unwrap();
    plan.push_target(target(&socket)).unwrap();
    handle
        .send(UdpOutboundSend::new(bytes, plan.finish().unwrap(), true).unwrap())
        .unwrap();
    handle
        .send(packet_to(
            vec![8],
            "127.0.0.1".into(),
            socket.local_addr().unwrap().port(),
        ))
        .unwrap();
    handle.finish_admission();
    assert!(pool.wait_for_idle(Duration::from_secs(3)));
    for expected in [7, 7, 8] {
        let mut bytes = [0; 8];
        assert_eq!(socket.recv(&mut bytes).unwrap(), 1);
        assert_eq!(bytes[0], expected);
    }
    assert_empty(&socket);
    let mut events = Vec::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    loop {
        let turn = handle.drain_events(&mut events);
        assert!(turn.received <= UDP_EVENT_DRAIN_BUDGET.get());
        if turn.state == PendingDrainState::Disconnected {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "retired worker replies must drain to disconnection"
        );
    }
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, StreamingWorkerEvent::Error(_)))
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, StreamingWorkerEvent::OutgoingLog(_)))
            .count(),
        2
    );
}
