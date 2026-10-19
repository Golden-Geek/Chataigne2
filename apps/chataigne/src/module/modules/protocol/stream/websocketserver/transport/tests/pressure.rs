use super::*;

#[test]
fn bounded_command_admission_rejects_whole_payloads_before_queue_overflow() {
    for (payload, accepted) in [(vec![7], 256), (vec![7; COMMAND_BYTES - size_of::<Command>()], 8)] {
        let pool = pool();
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let mut handle = spawn_using(config(0), &pool, move |_| {
            started_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            Err("delayed setup".into())
        })
        .unwrap();
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        handle.clients.store(1, Ordering::Release);
        for _ in 0..accepted {
            handle.send(StreamingSendFrameKind::Binary, &payload).unwrap();
        }
        assert!(handle
            .send(StreamingSendFrameKind::Binary, &payload)
            .unwrap_err()
            .contains("capacity exhausted"));
        handle.stop();
        assert!(handle.send(StreamingSendFrameKind::Binary, &payload).is_err());
        release_tx.send(()).unwrap();
        assert!(pool.wait_for_idle(Duration::from_secs(2)));
    }
}

#[test]
fn saturated_input_replies_leave_output_and_cancellation_independent() {
    let pool = pool();
    let mut handle = spawn(config(0), &pool);
    let address = listening(&handle);
    let mut client = connect(&handle, address, "/");
    for _ in 0..100 {
        client.send(Message::text("input")).unwrap();
    }
    wait(|| handle.worker.event_metrics().rejected > 0);
    let metrics = handle.worker.event_metrics();
    assert!(metrics.depth <= 64 && metrics.retained_weight <= EVENT_BYTES);
    handle.send(StreamingSendFrameKind::Text, b"independent").unwrap();
    assert_eq!(client.read().unwrap(), Message::text("independent"));
    let started = Instant::now();
    handle.stop();
    assert!(started.elapsed() < Duration::from_millis(100));
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn saturated_output_failures_retain_input_and_resume_without_replaying_native_effects() {
    let pool = pool();
    let handle = spawn(config(0), &pool);
    let address = listening(&handle);
    let mut client = connect(&handle, address, "/");
    for _ in 0..100 {
        handle.send(StreamingSendFrameKind::Text, &[255]).unwrap();
    }
    wait(|| handle.worker.event_metrics().rejected > 0);
    client.send(Message::text("independent input")).unwrap();
    let mut failures = 0;
    let mut inputs = 0;
    wait(|| {
        let mut events = Vec::new();
        handle.drain_events(&mut events);
        for event in events {
            match event {
                Event::Error(error) if error.contains("UTF-8") => failures += 1,
                Event::Input(input) => {
                    assert_eq!(input.callbacks[2].payload()["args"][0], "independent input");
                    inputs += 1;
                }
                other => panic!("unexpected pressure result: {other:?}"),
            }
        }
        failures == 100 && inputs == 1
    });
    handle.send(StreamingSendFrameKind::Text, b"fresh output").unwrap();
    assert_eq!(client.read().unwrap(), Message::text("fresh output"));
}

#[test]
fn invalid_configuration_capacity_is_rejected_before_native_reservation() {
    let pool = pool();
    let mut invalid = config(0);
    invalid.bind_host.reserve(1025);
    assert!(spawn_using(invalid, &pool, worker::bind).is_err());
    let mut invalid = config(0);
    invalid.path.reserve(8193);
    assert!(spawn_using(invalid, &pool, worker::bind).is_err());
    let mut invalid = config(0);
    invalid.input.parse.mode.reserve(17);
    assert!(spawn_using(invalid, &pool, worker::bind).is_err());
    assert_eq!(pool.metrics().active, 0);
}

#[test]
fn pending_handshakes_share_the_client_limit_and_free_capacity_when_cancelled() {
    let pool = pool();
    let mut handle = spawn(config(0), &pool);
    let address = listening(&handle);
    let _stalled: Vec<_> = (0..=MAX_CLIENTS).map(|_| raw(address)).collect();
    until(
        &handle,
        |event| matches!(event, Event::Error(error) if error.contains("client/handshake capacity")),
    );
    assert_eq!(handle.clients.load(Ordering::Acquire), 0);
    handle.stop();
    assert!(pool.wait_for_idle(Duration::from_secs(1)));
}
