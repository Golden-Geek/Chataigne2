use super::*;
use std::{
    net::TcpStream,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
use tokio_tungstenite::tungstenite::{client::client, Message, WebSocket};

mod input;
mod lifecycle;
mod pressure;

fn config(port: u16) -> WebSocketServerTransportConfig {
    WebSocketServerTransportConfig {
        bind_host: "127.0.0.1".into(),
        bind_port: port,
        path: String::new(),
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
fn spawn_using(
    config: WebSocketServerTransportConfig,
    pool: &RetirementPool,
    binder: impl FnMut(&WebSocketServerTransportConfig) -> Result<TcpListener, String> + Send + 'static,
) -> Result<WebSocketServerTransportHandle, String> {
    WebSocketServerTransportHandle::spawn_using(config, pool, binder, notifier())
}
fn pool() -> RetirementPool {
    RetirementPool::new(NonZeroUsize::MIN)
}
fn spawn(config: WebSocketServerTransportConfig, pool: &RetirementPool) -> WebSocketServerTransportHandle {
    spawn_using(config, pool, worker::bind).unwrap()
}
fn wait(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !condition() {
        assert!(Instant::now() < deadline, "WebSocket server fixture did not settle");
        thread::sleep(Duration::from_millis(1));
    }
}
fn until(handle: &WebSocketServerTransportHandle, mut condition: impl FnMut(&Event) -> bool) {
    wait(|| {
        let mut events = Vec::new();
        let state = handle.drain_events(&mut events).state;
        let found = events.iter().any(&mut condition);
        assert!(found || state != golden_io::PendingDrainState::Disconnected);
        found
    });
}
fn listening(handle: &WebSocketServerTransportHandle) -> SocketAddr {
    let mut address = None;
    until(handle, |event| {
        if let Event::Listening(value) = event {
            address = Some(*value);
            true
        } else {
            false
        }
    });
    address.unwrap()
}
fn raw(address: SocketAddr) -> TcpStream {
    let stream = TcpStream::connect(address).unwrap();
    stream.set_nonblocking(false).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    stream.set_write_timeout(Some(Duration::from_secs(3))).unwrap();
    stream
}
fn connect(handle: &WebSocketServerTransportHandle, address: SocketAddr, path: &str) -> WebSocket<TcpStream> {
    let socket = client(format!("ws://{address}{path}"), raw(address)).unwrap().0;
    until(handle, |event| matches!(event, Event::ClientConnected { .. }));
    socket
}

#[test]
fn broadcasts_preserve_frame_kinds_and_fifo_for_multiple_peers() {
    let pool = pool();
    let handle = spawn(config(0), &pool);
    let address = listening(&handle);
    let mut first = connect(&handle, address, "/one");
    let mut second = connect(&handle, address, "/two");
    assert!(handle
        .send(StreamingSendFrameKind::Binary, &vec![0; COMMAND_BYTES])
        .is_err());
    let payload = vec![7; 200_000];
    handle.send(StreamingSendFrameKind::Binary, &payload).unwrap();
    handle.send(StreamingSendFrameKind::Text, b"tail").unwrap();
    for client in [&mut first, &mut second] {
        assert_eq!(client.read().unwrap(), Message::Binary(payload.clone().into()));
        assert_eq!(client.read().unwrap(), Message::text("tail"));
    }
    drop(handle);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn invalid_text_fails_one_broadcast_without_disconnecting_or_replaying_it() {
    let pool = pool();
    let handle = spawn(config(0), &pool);
    let address = listening(&handle);
    let mut client = connect(&handle, address, "/");
    handle.send(StreamingSendFrameKind::Text, &[255]).unwrap();
    until(
        &handle,
        |event| matches!(event, Event::Error(error) if error.contains("UTF-8")),
    );
    handle.send(StreamingSendFrameKind::Text, b"valid").unwrap();
    assert_eq!(client.read().unwrap(), Message::text("valid"));
    assert_eq!(handle.clients.load(Ordering::Acquire), 1);
}

#[test]
fn inbound_frames_keep_their_peer_and_text_binary_identity() {
    let pool = pool();
    let handle = spawn(config(0), &pool);
    let address = listening(&handle);
    let mut client = connect(&handle, address, "/input");
    let label = client.get_ref().local_addr().unwrap().to_string();
    client.send(Message::text("text")).unwrap();
    until(&handle, |event| {
        if let Event::Input(input) = event {
            assert_eq!(input.callbacks.len(), 3);
            assert_eq!(input.callbacks[2].payload()["args"][0], "text");
            assert_eq!(input.callbacks[2].payload()["args"][1], label);
            true
        } else {
            false
        }
    });
    client.send(Message::binary(vec![0, 255])).unwrap();
    until(&handle, |event| {
        if let Event::Input(input) = event {
            assert_eq!(input.callbacks.len(), 2);
            assert_eq!(input.callbacks[0].payload()["args"][0], serde_json::json!([0, 255]));
            assert_eq!(input.callbacks[0].payload()["args"][1], label);
            true
        } else {
            false
        }
    });
}
