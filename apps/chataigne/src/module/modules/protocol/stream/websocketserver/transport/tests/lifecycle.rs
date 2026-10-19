use super::*;
use std::io::Read;

#[test]
fn blocked_native_setup_retains_its_slot_and_never_blocks_stop_or_drop() {
    let pool = pool();
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let mut handle = spawn_using(config(0), &pool, move |_| {
        started_tx.send(()).unwrap();
        release_rx.recv().unwrap();
        Err("delayed bind".into())
    })
    .unwrap();
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let started = Instant::now();
    handle.stop();
    drop(handle);
    assert!(started.elapsed() < Duration::from_millis(100));
    assert_eq!(pool.metrics().active, 1);
    assert!(spawn_using(config(0), &pool, worker::bind).is_err());
    release_tx.send(()).unwrap();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn occupied_bind_recovers_in_the_same_native_lifetime() {
    let pool = pool();
    let occupied = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = occupied.local_addr().unwrap();
    let handle = spawn(config(address.port()), &pool);
    until(&handle, |event| matches!(event, Event::Recovering(_)));
    assert!(handle
        .send(StreamingSendFrameKind::Text, b"not admitted", false)
        .is_err());
    drop(occupied);
    assert_eq!(listening(&handle), address);
    let _client = connect(&handle, address, "/");
    assert_eq!(pool.metrics().active, 1);
}

#[test]
fn stalled_http_peer_does_not_block_valid_upgrades_or_cancellation() {
    let pool = pool();
    let mut handle = spawn(config(0), &pool);
    let address = listening(&handle);
    let _stalled = raw(address);
    let started = Instant::now();
    let mut valid = connect(&handle, address, "/");
    handle.send(StreamingSendFrameKind::Text, b"live", false).unwrap();
    assert_eq!(valid.read().unwrap(), Message::text("live"));
    assert!(started.elapsed() < Duration::from_secs(1));
    handle.stop();
    assert!(pool.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn exact_path_rejects_other_paths_and_empty_path_preserves_actual_path_info() {
    let pool = pool();
    let mut selected = config(0);
    selected.path = "/selected".into();
    let handle = spawn(selected, &pool);
    let address = listening(&handle);
    let result = client(format!("ws://{address}/other"), raw(address));
    assert!(
        matches!(result, Err(tokio_tungstenite::tungstenite::handshake::HandshakeError::Failure(tokio_tungstenite::tungstenite::Error::Http(response))) if response.status().as_u16() == 404)
    );
    let _valid = connect(&handle, address, "/selected?query=allowed");
    drop(handle);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    let handle = spawn(config(0), &pool);
    let address = listening(&handle);
    let _socket = client(format!("ws://{address}/arbitrary"), raw(address)).unwrap().0;
    until(
        &handle,
        |event| matches!(event, Event::ClientConnected { info, .. } if info.ends_with("Path: /arbitrary")),
    );
}

#[test]
fn receive_disabled_still_services_ping_and_close() {
    let pool = pool();
    let mut selected = config(0);
    selected.receive_enabled = false;
    let handle = spawn(selected, &pool);
    let address = listening(&handle);
    let mut socket = connect(&handle, address, "/");
    socket.send(Message::Ping(vec![1, 2].into())).unwrap();
    assert_eq!(socket.read().unwrap(), Message::Pong(vec![1, 2].into()));
    socket.close(None).unwrap();
    assert!(matches!(socket.read().unwrap(), Message::Close(_)));
    until(&handle, |event| {
        matches!(event, Event::ClientDisconnected { reason: None, .. })
    });
    assert_eq!(handle.clients.load(Ordering::Acquire), 0);
}

#[test]
fn oversized_wire_frame_closes_only_its_peer_and_recovers_for_new_connections() {
    let pool = pool();
    let handle = spawn(config(0), &pool);
    let address = listening(&handle);
    let mut oversized = connect(&handle, address, "/");
    let _ = oversized.send(Message::binary(vec![0; COMMAND_BYTES + 1]));
    until(
        &handle,
        |event| matches!(event, Event::ClientDisconnected { reason: Some(reason), .. } if reason.contains("input failed")),
    );
    assert_eq!(handle.clients.load(Ordering::Acquire), 0);
    let mut replacement = connect(&handle, address, "/");
    handle.send(StreamingSendFrameKind::Text, b"fresh", false).unwrap();
    assert_eq!(replacement.read().unwrap(), Message::text("fresh"));
}

#[test]
fn native_graceful_admission_close_drains_frames_and_aborts_pending_handshakes() {
    let pool = pool();
    let mut handle = spawn(config(0), &pool);
    let address = listening(&handle);
    let mut client = connect(&handle, address, "/");
    let mut stalled = raw(address);
    let payload = vec![5; 200_000];
    handle.send(StreamingSendFrameKind::Binary, &payload, false).unwrap();
    handle.send(StreamingSendFrameKind::Text, b"tail", false).unwrap();
    handle.worker.finish_admission();
    assert!(handle.send(StreamingSendFrameKind::Text, b"late", false).is_err());
    assert_eq!(client.read().unwrap().into_data().as_ref(), payload);
    assert_eq!(client.read().unwrap(), Message::text("tail"));
    assert!(matches!(client.read().unwrap(), Message::Close(_)));
    let mut events = Vec::new();
    wait(|| handle.drain_events(&mut events).state == golden_io::PendingDrainState::Disconnected);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    assert!(matches!(stalled.read(&mut [0; 1]), Ok(0) | Err(_)));
}
