use super::*;

#[test]
fn queued_frames_from_a_failed_connection_are_never_moved_to_its_replacement() {
    let pool = pool();
    let (first, first_peer) = pair();
    let (second, mut second_peer) = pair();
    let mut sockets = std::collections::VecDeque::from([first, second]);
    let mut handle = WebSocketClientTransportHandle::spawn_with_connector(config(), &pool, notifier(), move |_, _| {
        sockets.pop_front().ok_or("no replacement".into())
    })
    .unwrap();
    wait(|| handle.connection.load(Ordering::Acquire) == 1);
    let mut accepted = 0;
    for index in 0..1000 {
        match handle.send(StreamingSendFrameKind::Binary, &[index as u8], true) {
            Ok(()) => accepted += 1,
            Err(error) => {
                assert!(error.contains("capacity exhausted"));
                break;
            }
        }
    }
    wait(|| handle.worker.event_metrics().rejected > 0);
    first_peer.get_ref().shutdown(std::net::Shutdown::Both).unwrap();
    drop(first_peer);
    let mut events = Vec::new();
    wait(|| {
        handle.drain_events(&mut events);
        handle.connection.load(Ordering::Acquire) == 2
            && events
                .iter()
                .filter(|event| {
                    matches!(
                        event,
                        StreamingWorkerEvent::OutgoingLog(_) | StreamingWorkerEvent::Error(_)
                    )
                })
                .count()
                == accepted
    });
    assert!(events
        .iter()
        .any(|event| matches!(event, StreamingWorkerEvent::Error(error) if error.contains("original connection"))));
    second_peer.get_mut().set_nonblocking(true).unwrap();
    assert!(matches!(second_peer.get_mut().peek(&mut [0; 1]), Err(error) if error.kind() == io::ErrorKind::WouldBlock));
    second_peer.get_mut().set_nonblocking(false).unwrap();
    handle.send(StreamingSendFrameKind::Text, b"fresh", false).unwrap();
    assert_eq!(second_peer.read().unwrap(), Message::text("fresh"));
    handle.stop();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn a_stalled_http_handshake_cancels_without_waiting_for_its_deadline() {
    let pool = pool();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let mut settings = config();
    settings.remote_port = listener.local_addr().unwrap().port();
    let endpoint = settings.clone();
    let mut handle =
        WebSocketClientTransportHandle::spawn_with_connector(settings, &pool, notifier(), move |cancel, stopped| {
            socket::connect(&endpoint, cancel, stopped)
        })
        .unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut accepted = None;
    wait(|| match listener.accept() {
        Ok((peer, _)) => {
            accepted = Some(peer);
            true
        }
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => false,
        Err(error) => panic!("{error}"),
    });
    let started = Instant::now();
    handle.stop();
    assert!(pool.wait_for_idle(Duration::from_millis(250)));
    assert!(started.elapsed() < Duration::from_secs(1));
    let mut peer = accepted.unwrap();
    peer.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
    peer.read_to_end(&mut Vec::new()).unwrap();
}

#[test]
fn protocol_socket_enforces_per_turn_io_byte_and_call_limits() {
    use std::io::Write;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    client.set_nonblocking(true).unwrap();
    let mut peer = listener.accept().unwrap().0;
    peer.write_all(&vec![7; 100_000]).unwrap();
    let mut budget = BudgetSocket::new(client);
    wait(|| budget.stream.peek(&mut [0; 1]).is_ok_and(|count| count > 0));
    let mut read = 0;
    for _ in 0..5 {
        match budget.read(&mut [0; 100_000]) {
            Ok(count) => read += count,
            Err(error) => {
                assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
                break;
            }
        }
    }
    assert!(read > 0 && read <= 64 * 1024);
    budget.reset();
    // Sender completion does not mean every segment has reached this socket.
    wait(|| budget.stream.peek(&mut [0; 1]).is_ok_and(|count| count > 0));
    assert!(budget.read(&mut [0; 100_000]).unwrap() > 0);
    let written = budget.write(&vec![8; 100_000]).unwrap();
    assert!(written > 0 && written <= 64 * 1024);
    peer.write_all(&[9; 10]).unwrap();
    wait(|| budget.stream.peek(&mut [0; 10]).is_ok_and(|count| count >= 10));
    budget.reset();
    for _ in 0..4 {
        assert_eq!(budget.read(&mut [0; 1]).unwrap(), 1);
    }
    assert_eq!(budget.read(&mut [0; 1]).unwrap_err().kind(), io::ErrorKind::WouldBlock);
}

#[test]
fn graceful_admission_close_interrupts_a_handshake_that_has_no_admitted_frame_prefix() {
    let pool = pool();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let mut settings = config();
    settings.remote_port = listener.local_addr().unwrap().port();
    let endpoint = settings.clone();
    let mut handle =
        WebSocketClientTransportHandle::spawn_with_connector(settings, &pool, notifier(), move |cancel, stopped| {
            socket::connect(&endpoint, cancel, stopped)
        })
        .unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut accepted = None;
    wait(|| match listener.accept() {
        Ok((peer, _)) => {
            accepted = Some(peer);
            true
        }
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => false,
        Err(error) => panic!("{error}"),
    });
    handle.finish_admission();
    drain(&handle);
    assert!(pool.wait_for_idle(Duration::from_millis(250)));
    assert!(!handle.worker.is_accepting_commands());
    drop(accepted);
}

#[test]
fn an_unavailable_endpoint_recovers_with_backoff_without_another_owner_edit() {
    let pool = pool();
    let (client, mut peer) = pair();
    let mut client = Some(client);
    let attempts = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let native = attempts.clone();
    let mut handle = WebSocketClientTransportHandle::spawn_with_connector(config(), &pool, notifier(), move |_, _| {
        if native.fetch_add(1, Ordering::Relaxed) == 0 {
            Err("injected unavailable endpoint".into())
        } else {
            client.take().ok_or("no replacement".into())
        }
    })
    .unwrap();
    wait(|| handle.connection.load(Ordering::Acquire) == 1);
    assert_eq!(attempts.load(Ordering::Relaxed), 2);
    handle.send(StreamingSendFrameKind::Text, b"recovered", false).unwrap();
    assert_eq!(peer.read().unwrap(), Message::text("recovered"));
    handle.stop();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn reconnect_discards_a_partial_frame_before_the_new_peer_sends_any_data() {
    let pool = pool();
    let (first, mut peer) = pair();
    let (second, mut next) = pair();
    let mut sockets = std::collections::VecDeque::from([first, second]);
    let mut handle = WebSocketClientTransportHandle::spawn_with_connector(config(), &pool, notifier(), move |_, _| {
        sockets.pop_front().ok_or("no replacement".into())
    })
    .unwrap();
    wait(|| handle.connection.load(Ordering::Acquire) == 1);
    peer.send(Message::text("old ")).unwrap();
    let mut events = Vec::new();
    wait(|| {
        handle.drain_events(&mut events);
        events
            .iter()
            .any(|event| matches!(event, StreamingWorkerEvent::Input(_)))
    });
    drop(peer);
    wait(|| {
        handle.drain_events(&mut events);
        handle.connection.load(Ordering::Acquire) == 2
    });
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, StreamingWorkerEvent::RejectedInput(1)))
            .count(),
        1
    );
    next.send(Message::text("fresh 4\n")).unwrap();
    let mut fresh = None;
    wait(|| {
        let mut events = Vec::new();
        handle.drain_events(&mut events);
        for event in events {
            if let StreamingWorkerEvent::Input(input) = event {
                fresh = Some(input);
            }
        }
        fresh.is_some()
    });
    assert_eq!(fresh.unwrap().values.as_ref().unwrap()[0].path_segments, ["fresh"]);
    handle.stop();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}
