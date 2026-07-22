use std::{net::TcpListener, thread, time::{Duration, Instant}};

use tokio::runtime::Builder;
use tokio_tungstenite::connect_async;

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
        match handle.try_recv() {
            Ok(WebSocketServerWorkerEvent::ClientConnected { .. }) => break,
            Ok(WebSocketServerWorkerEvent::Stopped(error)) => {
                panic!("websocket server transport stopped unexpectedly: {error}");
            }
            Ok(_) => {}
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                panic!("websocket server transport event channel disconnected unexpectedly");
            }
        }

        if Instant::now() >= deadline {
            panic!("timed out waiting for websocket server client connection event");
        }

        thread::sleep(Duration::from_millis(10));
    }

    handle.stop();
}
