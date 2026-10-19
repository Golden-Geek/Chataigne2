use super::*;

#[test]
fn invalid_configurations_are_rejected_before_native_lifetime_reservation() {
    let pool = pool();
    let mut spare_capacity = config();
    spare_capacity.port_name.reserve(1025);
    for invalid in [
        spare_capacity,
        SerialConnectionConfig {
            port_name: "bad\0name".into(),
            ..config()
        },
        SerialConnectionConfig {
            baud_rate: 0,
            ..config()
        },
    ] {
        assert!(spawn_using(invalid, &pool, |_| panic!("invalid config cannot open")).is_err());
    }
    assert_eq!(pool.metrics().active, 0);
}

#[test]
fn output_reply_pressure_retains_each_failure_and_independent_replacement_input() {
    let pool = pool();
    let (first, old) = wire();
    let (second, new) = wire();
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    old.lock().unwrap().actions = VecDeque::from([
        Action::Block(started_tx, release_rx, 1),
        Action::Fail(io::ErrorKind::TimedOut),
    ]);
    let mut ports = VecDeque::from([first, second]);
    let handle = spawn_using(config(), &pool, move |_| {
        ports.pop_front().ok_or("no replacement".into())
    })
    .unwrap();
    wait(|| handle.connection.load(Ordering::Acquire) == 1);
    handle.send(b"original", false).unwrap();
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    for _ in 0..100 {
        handle.send(b"stale", false).unwrap();
    }
    release_tx.send(()).unwrap();
    wait(|| handle.worker.event_metrics().rejected > 0 && handle.connection.load(Ordering::Acquire) == 2);
    new.lock().unwrap().input.push_back(b"independent input".to_vec());
    handle.send(b"fresh", false).unwrap();
    let mut failures = 0;
    let mut input = Vec::new();
    wait(|| {
        let mut events = Vec::new();
        handle.drain_events(&mut events);
        for event in events {
            match event {
                Event::Warning(message) if message.contains("will not be replayed") => failures += 1,
                Event::Input(prepared) => input.extend(input_bytes(&prepared)),
                _ => {}
            }
        }
        failures == 101 && input == b"independent input" && new.lock().unwrap().output == b"fresh"
    });
    assert_eq!(old.lock().unwrap().output, b"o");
}

#[test]
fn native_write_pressure_rejects_whole_commands_at_item_and_retained_byte_limits() {
    for (payload, accepted) in [
        (vec![7], 256),
        (vec![7; COMMAND_BYTES - std::mem::size_of::<Outbound>()], 1),
    ] {
        let pool = pool();
        let (mut handle, state) = connected(&pool);
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        state
            .lock()
            .unwrap()
            .actions
            .push_back(Action::Block(started_tx, release_rx, 1));
        handle.send(b"first", false).unwrap();
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        for _ in 0..accepted {
            handle.send(&payload, false).unwrap();
        }
        assert!(handle.send(&payload, false).unwrap_err().contains("capacity exhausted"));
        let started = Instant::now();
        handle.stop();
        assert!(started.elapsed() < Duration::from_millis(100));
        assert!(handle.send(&payload, false).is_err());
        release_tx.send(()).unwrap();
        assert!(pool.wait_for_idle(Duration::from_secs(2)));
        assert_eq!(state.lock().unwrap().output, b"f");
    }
}

#[test]
fn full_input_replies_retain_fifo_while_output_and_cancellation_remain_independent() {
    let pool = pool();
    let (mut handle, state) = connected(&pool);
    state.lock().unwrap().input = (0..100).map(|i| vec![i as u8]).collect();
    wait(|| handle.worker.event_metrics().rejected > 0);
    let metrics = handle.worker.event_metrics();
    assert!(metrics.depth <= 64 && metrics.retained_weight <= EVENT_BYTES);
    handle.send(b"independent", false).unwrap();
    wait(|| state.lock().unwrap().output == b"independent");
    let mut received = Vec::new();
    wait(|| {
        let mut events = Vec::new();
        handle.drain_events(&mut events);
        for event in events {
            if let Event::Input(prepared) = event {
                received.extend(input_bytes(&prepared));
            }
        }
        received.len() == 100
    });
    assert_eq!(received, (0..100).map(|i| i as u8).collect::<Vec<_>>());
    state.lock().unwrap().input = (0..100).map(|_| vec![1]).collect();
    wait(|| handle.worker.event_metrics().depth == 64);
    handle.stop();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}
