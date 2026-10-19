use super::*;

fn duplex_fixture(pool: &RetirementPool) -> (UdpStreamingTransportHandle, SocketAddr) {
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    let address = socket.local_addr().unwrap();
    let input = InputPlan::new(input_config());
    let worker_input = input.clone();
    let worker = BoundedWorker::spawn_notified(
        "udp-duplex-pressure",
        pool,
        WorkerLimits {
            command_items: 256,
            command_weight: UDP_COMMAND_BYTES,
            event_items: 1,
            event_weight: 32 * 1024 * 1024,
        },
        notifier(),
        move |context| worker_loop(context, true, worker_input, || Ok(socket)),
    )
    .unwrap();
    (UdpStreamingTransportHandle { worker, input }, address)
}

fn wait_until(mut condition: impl FnMut() -> bool) {
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while !condition() {
        assert!(
            std::time::Instant::now() < deadline,
            "controlled UDP worker did not reach the expected state"
        );
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn full_input_replies_do_not_strand_a_cold_unlogged_send() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let (mut handle, address) = duplex_fixture(&pool);
    // Let initial discovery finish before deliberately filling the only reply slot.
    wait_until(|| {
        let mut events = Vec::new();
        handle.drain_events(&mut events);
        events
            .iter()
            .any(|event| matches!(event, StreamingWorkerEvent::Interfaces(_)))
    });
    let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
    sender.send_to(&[1], address).unwrap();
    wait_until(|| handle.worker.event_metrics().depth == 1);
    let rejected = handle.worker.event_metrics().rejected;
    sender.send_to(&[2], address).unwrap();
    wait_until(|| handle.worker.event_metrics().rejected > rejected);

    let receiver = UdpSocket::bind("127.0.0.1:0").unwrap();
    receiver.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    let snapshot = Arc::new(super::super::super::tests::snapshot::nested_snapshot(
        20_000,
        "127.0.0.1",
        receiver.local_addr().unwrap().port(),
    ));
    let (command, _) =
        super::super::super::output_plan::command(&mut None, &snapshot, golden_core::node::NodeId(1), vec![9], false)
            .unwrap();
    handle.send(command).unwrap();
    let delivered = receiver.recv(&mut [0; 8]);
    if delivered.is_ok() {
        let mut inputs = Vec::new();
        wait_until(|| {
            let mut events = Vec::new();
            handle.drain_events(&mut events);
            for event in events {
                if let StreamingWorkerEvent::Input(input) = event {
                    inputs.push(input.callbacks[0].payload()["args"][0][0].as_u64().unwrap());
                }
            }
            inputs.len() == 2
        });
        assert_eq!(inputs, [1, 2], "pending input must resume once, in order");
    }
    handle.stop();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    assert_eq!(
        delivered.unwrap(),
        1,
        "reply pressure must not gate cold preparation or an unlogged native send"
    );
}

fn receiver() -> UdpSocket {
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    socket.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    socket
}

fn logged_fanout(first: &UdpSocket, second: &UdpSocket) -> UdpOutboundSend {
    let mut plan = OutputPlanBuilder::default();
    for socket in [first, second] {
        plan.push_target(UdpOutputTarget {
            remote_host: "127.0.0.1".into(),
            remote_port: socket.local_addr().unwrap().port(),
        })
        .unwrap();
    }
    UdpOutboundSend::new(vec![7], plan.finish().unwrap(), true).unwrap()
}

#[test]
fn output_reply_pressure_resumes_the_next_destination_once_and_retirement_drains_logs() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let (mut handle, _) = duplex_fixture(&pool);
    // Ready occupies the only slot; interface discovery is now pending behind it.
    wait_until(|| handle.worker.event_metrics().rejected > 0);
    let first = receiver();
    let second = receiver();
    handle.send(logged_fanout(&first, &second)).unwrap();
    assert_eq!(first.recv(&mut [0; 8]).unwrap(), 1);
    second.set_read_timeout(Some(Duration::from_millis(50))).unwrap();
    assert!(
        second.recv(&mut [0; 8]).is_err(),
        "a pending output outcome pauses its own cursor"
    );
    handle.finish_admission();
    assert!(
        !pool.wait_for_idle(Duration::from_millis(20)),
        "retirement must retain its pending outcome and suffix"
    );
    let mut events = Vec::new();
    wait_until(|| handle.drain_events(&mut events).state == PendingDrainState::Disconnected);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    second.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    assert_eq!(second.recv(&mut [0; 8]).unwrap(), 1);
    let logs: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            StreamingWorkerEvent::OutgoingLog(message) => Some(message),
            _ => None,
        })
        .collect();
    assert_eq!(logs.len(), 2);
    assert!(logs[0].ends_with(&format!(":{}", first.local_addr().unwrap().port())));
    assert!(logs[1].ends_with(&format!(":{}", second.local_addr().unwrap().port())));
    for socket in [&first, &second] {
        socket.set_read_timeout(Some(Duration::from_millis(50))).unwrap();
        assert!(
            socket.recv(&mut [0; 8]).is_err(),
            "reply retries must never resend a destination"
        );
    }
}

#[test]
fn cancelling_a_worker_with_full_reply_slots_abandons_its_unsent_suffix_without_joining() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let (mut handle, _) = duplex_fixture(&pool);
    wait_until(|| handle.worker.event_metrics().rejected > 0);
    let first = receiver();
    let second = receiver();
    handle.send(logged_fanout(&first, &second)).unwrap();
    assert_eq!(first.recv(&mut [0; 8]).unwrap(), 1);
    handle.stop();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    second.set_read_timeout(Some(Duration::from_millis(50))).unwrap();
    assert!(second.recv(&mut [0; 8]).is_err());
}
