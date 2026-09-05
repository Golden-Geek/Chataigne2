use std::{net::TcpListener, thread, time::{Duration, Instant}};

use tokio::runtime::Builder;
use tokio_tungstenite::connect_async;
use golden_io::PendingDrainState;

use super::transport::{
    WebSocketServerTransportConfig, WebSocketServerTransportHandle, WebSocketServerWorkerEvent,
};

#[test]
fn websocket_transport_server_accepts_websocket_connection() {
    let reserved = TcpListener::bind("127.0.0.1:0").expect("reserve test port");
    let port = reserved.local_addr().expect("read reserved local addr").port();
    drop(reserved);

    let mut handle = WebSocketServerTransportHandle::spawn(WebSocketServerTransportConfig {
        bind_host: "127.0.0.1".to_string(),
        bind_port: port,
        path: String::new(),
        receive_enabled: true,
        send_enabled: true,
    })
    .expect("spawn websocket server transport");

    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");

    let (_socket, _response) = runtime
        .block_on(connect_async(format!("ws://127.0.0.1:{port}/transport-test")))
        .expect("connect websocket test client");

    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let mut events = Vec::new();
        let drain = handle.drain_events(&mut events);
        for event in events {
            match event {
                WebSocketServerWorkerEvent::ClientConnected { .. } => {
                    handle.stop();
                    return;
                }
                WebSocketServerWorkerEvent::Stopped(error) => {
                    panic!("websocket server transport stopped unexpectedly: {error}");
                }
                _ => {}
            }
        }
        assert_ne!(
            drain.state,
            PendingDrainState::Disconnected,
            "websocket server transport event channel disconnected unexpectedly"
        );

        if Instant::now() >= deadline {
            panic!("timed out waiting for websocket server client connection event");
        }

        thread::sleep(Duration::from_millis(10));
    }

}
