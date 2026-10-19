use super::*;

#[test]
fn blocked_open_retains_its_lifetime_without_blocking_stop_or_creating_replacements() {
    let pool = pool();
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let mut handle = spawn_using(config(), &pool, move |_| {
        started_tx.send(()).unwrap();
        release_rx.recv().unwrap();
        Err("delayed open".into())
    })
    .unwrap();
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let published = handle.connection.clone();
    let started = Instant::now();
    handle.stop();
    drop(handle);
    assert!(started.elapsed() < Duration::from_millis(100));
    assert_eq!(pool.metrics().active, 1);
    assert!(spawn_using(config(), &pool, |_| panic!("no replacement slot")).is_err());
    release_tx.send(()).unwrap();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    assert_eq!(published.load(Ordering::Acquire), 0);
}

#[test]
fn blocked_native_cleanup_keeps_its_slot_and_never_joins_the_owner() {
    let pool = pool();
    let (mut port, _) = wire();
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    port.drop_gate = Some((started_tx, release_rx));
    let mut port = Some(port);
    let mut handle = spawn_using(config(), &pool, move |_| port.take().ok_or("no replacement".into())).unwrap();
    wait(|| handle.connection.load(Ordering::Acquire) == 1);
    let started = Instant::now();
    handle.stop();
    drop(handle);
    assert!(started.elapsed() < Duration::from_millis(100));
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(pool.metrics().active, 1);
    assert!(spawn_using(config(), &pool, |_| panic!("no replacement slot")).is_err());
    release_tx.send(()).unwrap();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn partial_failure_and_old_queued_commands_never_move_to_a_replacement_connection() {
    let pool = pool();
    let (first, old) = wire();
    let (second, new) = wire();
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    old.lock().unwrap().actions = VecDeque::from([
        Action::Block(started_tx, release_rx, 2),
        Action::Fail(io::ErrorKind::TimedOut),
    ]);
    let mut ports = VecDeque::from([first, second]);
    let handle = spawn_using(config(), &pool, move |_| {
        ports.pop_front().ok_or("no replacement".into())
    })
    .unwrap();
    wait(|| handle.connection.load(Ordering::Acquire) == 1);
    handle.send(b"old payload").unwrap();
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    for _ in 0..3 {
        handle.send(b"stale queued suffix").unwrap();
    }
    release_tx.send(()).unwrap();
    let mut failures = 0;
    wait(|| {
        let mut events = Vec::new();
        handle.drain_events(&mut events);
        failures += events
            .iter()
            .filter(|event| matches!(event, Event::Warning(message) if message.contains("will not be replayed")))
            .count();
        failures == 4 && handle.connection.load(Ordering::Acquire) == 2
    });
    handle.send(b"fresh").unwrap();
    wait(|| new.lock().unwrap().output == b"fresh");
    assert_eq!(old.lock().unwrap().output, b"ol");
}

#[test]
fn graceful_admission_close_finishes_the_accepted_fifo_without_opening_another_port() {
    let pool = pool();
    let (mut handle, state) = connected(&pool);
    let payload = vec![8; 100_000];
    handle.send(&payload).unwrap();
    handle.send(b"tail").unwrap();
    handle.worker.finish_admission();
    assert!(handle.send(b"late").is_err());
    let mut events = Vec::new();
    wait(|| handle.drain_events(&mut events).state == golden_io::PendingDrainState::Disconnected);
    assert_eq!(state.lock().unwrap().output, [payload.as_slice(), b"tail"].concat());
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}
