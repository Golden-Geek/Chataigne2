use super::*;
use std::{
    io::{self, Read},
    net::{TcpListener, TcpStream},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
use tokio_tungstenite::tungstenite::{protocol::Role, Message, WebSocket};

mod input;
mod lifecycle;

fn config() -> WebSocketClientTransportConfig {
    WebSocketClientTransportConfig {
        remote_host: "127.0.0.1".into(),
        remote_port: 9002,
        path: "/".into(),
        receive_enabled: true,
        send_enabled: true,
        input: StreamInputConfig {
            module: golden_core::node::NodeId(1),
            parse: Default::default(),
            processing_enabled: true,
            log_incoming: false,
        },
    }
}
fn notifier() -> WorkerNotifier {
    WorkerNotifier::new(|| {})
}
fn pool() -> RetirementPool {
    RetirementPool::new(NonZeroUsize::MIN)
}
fn wait(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !condition() {
        assert!(Instant::now() < deadline, "WebSocket fixture did not settle");
        thread::sleep(Duration::from_millis(1));
    }
}
fn pair() -> (WireSocket, WebSocket<TcpStream>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    client.set_nonblocking(true).unwrap();
    let peer = listener.accept().unwrap().0;
    peer.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    peer.set_write_timeout(Some(Duration::from_secs(3))).unwrap();
    let limits = crate::app::module::common::streaming::websocket::websocket_config()
        .read_buffer_size(8192)
        .write_buffer_size(0)
        .max_write_buffer_size(2 * 1024 * 1024 + 1024);
    (
        WebSocket::from_raw_socket(socket::BudgetSocket::new(client), Role::Client, Some(limits)),
        WebSocket::from_raw_socket(peer, Role::Server, None),
    )
}
fn connected(pool: &RetirementPool) -> (WebSocketClientTransportHandle, WebSocket<TcpStream>) {
    let (client, peer) = pair();
    let mut client = Some(client);
    let handle = WebSocketClientTransportHandle::spawn_with_connector(config(), pool, notifier(), move |_, _| {
        client.take().ok_or("no replacement".into())
    })
    .unwrap();
    wait(|| handle.connection.load(Ordering::Acquire) == 1);
    (handle, peer)
}
fn drain(handle: &WebSocketClientTransportHandle) -> Vec<StreamingWorkerEvent> {
    let mut events = Vec::new();
    wait(|| handle.drain_events(&mut events).state == golden_io::PendingDrainState::Disconnected);
    events
}

#[test]
fn blocked_endpoint_setup_retains_its_native_slot_without_blocking_stop_or_creating_replacements() {
    let pool = pool();
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let mut handle = WebSocketClientTransportHandle::spawn_with_connector(config(), &pool, notifier(), move |_, _| {
        started_tx.send(()).unwrap();
        release_rx.recv().unwrap();
        Err("delayed resolution".into())
    })
    .unwrap();
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let started = Instant::now();
    handle.stop();
    drop(handle);
    assert!(started.elapsed() < Duration::from_millis(100));
    assert_eq!(pool.metrics().active, 1);
    assert!(
        WebSocketClientTransportHandle::spawn_with_connector(config(), &pool, notifier(), |_, _| panic!(
            "no replacement slot"
        ))
        .is_err()
    );
    release_tx.send(()).unwrap();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn admission_checks_host_path_items_and_payload_bytes_before_native_expansion() {
    let pool = pool();
    for field in [0, 1] {
        let mut invalid = config();
        if field == 0 {
            invalid.remote_host.reserve(1025);
        } else {
            invalid.path.reserve(8193);
        }
        assert!(
            WebSocketClientTransportHandle::spawn_with_connector(invalid, &pool, notifier(), |_, _| panic!(
                "invalid configuration must not connect"
            ))
            .is_err()
        );
        assert_eq!(pool.metrics().active, 0);
    }
    for (payload, accepted) in [
        (vec![7], 256),
        (vec![7; COMMAND_BYTES - std::mem::size_of::<Outbound>()], 8),
    ] {
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let mut handle =
            WebSocketClientTransportHandle::spawn_with_connector(config(), &pool, notifier(), move |_, _| {
                started_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                Err("gated setup".into())
            })
            .unwrap();
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        // Isolate queue admission from connection discovery while setup is gated.
        handle.connection.store(1, Ordering::Release);
        for _ in 0..accepted {
            handle.send(StreamingSendFrameKind::Binary, &payload, false).unwrap();
        }
        assert!(handle
            .send(StreamingSendFrameKind::Binary, &payload, false)
            .unwrap_err()
            .contains("capacity exhausted"));
        assert!(handle
            .send(StreamingSendFrameKind::Binary, &vec![0; COMMAND_BYTES], false)
            .unwrap_err()
            .contains("1 MiB"));
        handle.stop();
        release_tx.send(()).unwrap();
        drop(handle);
        assert!(pool.wait_for_idle(Duration::from_secs(2)));
    }
}

#[test]
fn full_reply_pressure_retires_every_accepted_frame_and_log_in_fifo_order() {
    let pool = pool();
    let (mut handle, mut peer) = connected(&pool);
    let mut expected = Vec::new();
    for index in 0..1000 {
        let byte = index as u8;
        match handle.send(StreamingSendFrameKind::Binary, &[byte], true) {
            Ok(()) => expected.push(byte),
            Err(error) => {
                assert!(error.contains("capacity exhausted"));
                break;
            }
        }
    }
    assert!(expected.len() > 64);
    handle.finish_admission();
    wait(|| handle.worker.event_metrics().rejected > 0);
    assert!(!pool.wait_for_idle(Duration::from_millis(20)));
    let events = drain(&handle);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    let mut actual = Vec::new();
    loop {
        match peer.read().unwrap() {
            Message::Binary(bytes) => actual.push(bytes[0]),
            Message::Close(_) => break,
            message => panic!("{message:?}"),
        }
    }
    assert_eq!(actual, expected);
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, StreamingWorkerEvent::OutgoingLog(_)))
            .count(),
        expected.len()
    );
    assert!(!events
        .iter()
        .any(|event| matches!(event, StreamingWorkerEvent::Error(_))));
}

#[test]
fn invalid_text_is_rejected_without_disconnect_and_the_next_valid_frame_succeeds() {
    let pool = pool();
    let (mut handle, mut peer) = connected(&pool);
    handle.send(StreamingSendFrameKind::Text, &[255], false).unwrap();
    handle.send(StreamingSendFrameKind::Text, b"valid", false).unwrap();
    assert_eq!(peer.read().unwrap(), Message::text("valid"));
    let mut events = Vec::new();
    wait(|| {
        handle.drain_events(&mut events);
        events
            .iter()
            .any(|event| matches!(event, StreamingWorkerEvent::Error(error) if error.contains("UTF-8")))
    });
    assert_eq!(handle.connection.load(Ordering::Acquire), 1);
    handle.stop();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}
