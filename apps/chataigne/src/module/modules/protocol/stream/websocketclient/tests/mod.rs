use std::{
    net::TcpListener,
    thread,
    time::{Duration, Instant},
};

use tokio::runtime::Builder;
use tokio_tungstenite::accept_async;
use golden_io::PendingDrainState;

use super::transport::{
    StreamingWorkerEvent, WebSocketClientConnectionStatus, WebSocketClientTransportConfig,
    WebSocketClientTransportHandle,
};

#[test]
fn websocket_transport_client_connects_to_server() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind websocket test server");
    listener
        .set_nonblocking(true)
        .expect("set websocket test server nonblocking");
    let port = listener.local_addr().expect("read websocket test server addr").port();

    let server_thread = thread::spawn(move || {
        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build websocket test server runtime");

        let listener = {
            let _guard = runtime.enter();
            tokio::net::TcpListener::from_std(listener).expect("convert websocket test listener")
        };

        runtime.block_on(async move {
            let (stream, _) = tokio::time::timeout(Duration::from_secs(2), listener.accept())
                .await
                .expect("timed out waiting for websocket test client")
                .expect("accept websocket test client");
            let mut websocket = accept_async(stream).await.expect("accept websocket handshake");
            let _ = tokio::time::timeout(Duration::from_secs(2), websocket.close(None)).await;
        });
    });

    let mut handle = WebSocketClientTransportHandle::spawn(WebSocketClientTransportConfig {
        remote_host: "127.0.0.1".to_string(),
        remote_port: port,
        path: "/transport-test".to_string(),
        receive_enabled: true,
        send_enabled: true,
    })
    .expect("spawn websocket client transport");

    let deadline = Instant::now() + Duration::from_secs(2);
    'waiting: loop {
        let mut events = Vec::new();
        let drain = handle.drain_events(&mut events);
        for event in events {
            match event {
                StreamingWorkerEvent::Status(WebSocketClientConnectionStatus::Connected { .. }) => {
                    break 'waiting;
                }
                StreamingWorkerEvent::Error(error) => {
                    panic!("websocket client transport emitted unexpected error: {error}");
                }
                _ => {}
            }
        }
        assert_ne!(
            drain.state,
            PendingDrainState::Disconnected,
            "websocket client transport event channel disconnected unexpectedly"
        );

        if Instant::now() >= deadline {
            panic!("timed out waiting for websocket client connected status");
        }

        thread::sleep(Duration::from_millis(10));
    }

    handle.stop();
    server_thread.join().expect("join websocket test server thread");
}
