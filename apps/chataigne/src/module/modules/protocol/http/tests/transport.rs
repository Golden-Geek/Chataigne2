use super::super::transport::*;
use crate::app::module::common::http::{HttpAuthConfig, HttpMethod, HttpRequestBody, HttpRequestPayload};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

pub(super) fn request() -> HttpQueuedRequest {
    HttpQueuedRequest {
        origin: HttpRequestOrigin::Script { method: "get".into() },
        payload: HttpRequestPayload {
            method: HttpMethod::Get,
            path: "/".into(),
            query: String::new(),
            value_path: String::new(),
            headers: Vec::new(),
            body: HttpRequestBody::Empty,
            description: String::new(),
        },
    }
}

pub(super) fn config(address: String) -> HttpTransportConfig {
    HttpTransportConfig {
        base_address: address,
        default_headers: Vec::new(),
        auth: HttpAuthConfig::default(),
        timeout: Duration::from_secs(2),
        retry_count: 0,
        retry_delay: Duration::ZERO,
    }
}

pub(super) fn read_request(stream: &mut TcpStream) {
    // Windows can inherit the listener's nonblocking mode; headers may arrive after accept.
    stream.set_nonblocking(false).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    let mut request = Vec::new();
    while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
        let mut buffer = [0; 1024];
        let received = stream.read(&mut buffer).unwrap();
        assert!(received != 0, "request headers must complete before EOF");
        request.extend_from_slice(&buffer[..received]);
        assert!(request.len() <= 32 * 1024);
    }
}

#[test]
fn fixture_waits_for_delayed_headers_from_a_nonblocking_listener() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    let (accepted_tx, accepted_rx) = mpsc::sync_channel(1);
    let server = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    accepted_tx.send(()).unwrap();
                    read_request(&mut stream);
                    return;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline);
                    thread::sleep(Duration::from_millis(1));
                }
                Err(error) => panic!("loopback accept: {error}"),
            }
        }
    });
    let mut client = TcpStream::connect(address).unwrap();
    accepted_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    thread::sleep(Duration::from_millis(20));
    client.write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();
    server.join().unwrap();
}

#[test]
fn stop_does_not_wait_for_a_stalled_request_or_full_command_queue() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}/", listener.local_addr().unwrap());
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        read_request(&mut stream);
        entered_tx.send(()).unwrap();
        let _ = release_rx.recv_timeout(Duration::from_secs(5));
        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    });
    let mut handle = HttpTransportHandle::spawn(config(address)).unwrap();
    handle.send(request()).unwrap();
    entered_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    let mut accepted = 0;
    while handle.send(request()).is_ok() {
        accepted += 1;
        assert!(accepted <= 4096, "request admission must be bounded");
    }
    let (stopped_tx, stopped_rx) = mpsc::channel();
    let stopper = thread::spawn(move || {
        let start = Instant::now();
        handle.stop();
        stopped_tx.send(start.elapsed()).unwrap();
    });
    let stopped = stopped_rx.recv_timeout(Duration::from_millis(200));
    release_tx.send(()).unwrap();
    server.join().unwrap();
    assert!(
        stopped.is_ok(),
        "stop must not wait behind network I/O or queued requests"
    );
    stopper.join().unwrap();
}

fn collect(handle: &HttpTransportHandle, count: usize) -> Vec<HttpWorkerEvent> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut events = Vec::new();
    while events.len() < count && Instant::now() < deadline {
        handle.drain_events(
            &mut events,
            std::num::NonZeroUsize::new(HTTP_RESPONSE_CAPACITY).unwrap(),
        );
        thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(events.len(), count, "worker must complete every admitted request");
    events
}

#[test]
fn declared_and_streamed_oversized_responses_fail_then_the_next_request_succeeds() {
    for declared in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = format!("http://{}/", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            for first in [true, false] {
                let (mut stream, _) = listener.accept().unwrap();
                stream.set_write_timeout(Some(Duration::from_secs(3))).unwrap();
                read_request(&mut stream);
                if !first {
                    stream
                        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                        .unwrap();
                } else if declared {
                    write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        HTTP_RESPONSE_BODY_BYTES + 1
                    )
                    .unwrap();
                } else {
                    stream
                        .write_all(b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n")
                        .unwrap();
                    let _ = stream.write_all(&vec![b'x'; HTTP_RESPONSE_BODY_BYTES + 1]);
                }
            }
        });
        let mut handle = HttpTransportHandle::spawn(config(address)).unwrap();
        handle.send(request()).unwrap();
        handle.send(request()).unwrap();
        let events = collect(&handle, 2);
        assert!(matches!(&events[0], HttpWorkerEvent::Failure(failure) if failure.error.contains("body limit")));
        assert!(
            matches!(&events[1], HttpWorkerEvent::Response(response) if response.body == b"ok" && response.callback_args[1] == "ok")
        );
        handle.stop();
        server.join().unwrap();
    }
}

#[test]
fn slow_consumers_backpressure_replies_without_losing_order_or_occurrences() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}/", listener.local_addr().unwrap());
    let (served_tx, served_rx) = mpsc::channel();
    let server = thread::spawn(move || {
        for index in 0..6 {
            let (mut stream, _) = listener.accept().unwrap();
            read_request(&mut stream);
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: 1\r\nConnection: close\r\n\r\n{index}"
            )
            .unwrap();
            served_tx.send(index).unwrap();
        }
    });
    let handle = HttpTransportHandle::spawn(config(address)).unwrap();
    for _ in 0..6 {
        handle.send(request()).unwrap();
    }
    for index in 0..5 {
        assert_eq!(served_rx.recv_timeout(Duration::from_secs(3)).unwrap(), index);
    }
    assert!(
        served_rx.try_recv().is_err(),
        "one completed in flight plus four queued replies bounds service"
    );
    let events = collect(&handle, 6);
    for (index, event) in events.iter().enumerate() {
        assert!(matches!(event, HttpWorkerEvent::Response(response) if response.body == index.to_string().as_bytes()));
    }
    server.join().unwrap();
}

#[test]
fn cancellation_interrupts_http_retry_delay_and_releases_worker_capacity() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}/", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        read_request(&mut stream);
        stream
            .write_all(b"HTTP/1.1 503 Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .unwrap();
    });
    let lifetimes = golden_io::RetirementPool::new(std::num::NonZeroUsize::MIN);
    let mut settings = config(address);
    settings.retry_count = 10;
    settings.retry_delay = Duration::from_secs(60);
    let mut handle = HttpTransportHandle::spawn_with_lifetimes(settings, &lifetimes).unwrap();
    handle.send(request()).unwrap();
    server.join().unwrap();
    handle.stop();
    assert!(
        lifetimes.wait_for_idle(Duration::from_secs(2)),
        "retry must be woken without another network attempt"
    );
    assert!(handle.send(request()).unwrap_err().contains("no longer running"));
}

#[test]
fn request_admission_counts_owned_payload_bytes_and_json_complexity() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let mut handle = HttpTransportHandle::spawn(config(format!("http://{}/", listener.local_addr().unwrap()))).unwrap();
    let mut large = request();
    large.payload.body = HttpRequestBody::Text {
        text: "x".repeat(9 * 1024 * 1024),
        content_type: String::new(),
    };
    assert!(handle.send(large).unwrap_err().contains("capacity"));
    let mut complex = request();
    complex.payload.body = HttpRequestBody::Json {
        value: serde_json::Value::Array(vec![serde_json::Value::Null; 16_385]),
    };
    assert!(handle.send(complex).unwrap_err().contains("16384"));
    handle.stop();
}

#[test]
fn json_value_expansion_rejects_excess_nodes_and_repeated_large_paths() {
    use crate::app::module::common::http::response_json_received_values;
    let object = (0..4097)
        .map(|index| (index.to_string(), serde_json::json!(index)))
        .collect();
    let body = serde_json::to_vec(&serde_json::Value::Object(object)).unwrap();
    let error = response_json_received_values(&body, Some("application/json"), "/", "").unwrap_err();
    assert!(error.contains("4096"));

    let leaves = (0..3500)
        .map(|index| (index.to_string(), serde_json::json!(index)))
        .collect();
    let mut value = serde_json::Value::Object(leaves);
    for _ in 0..10 {
        let mut parent = serde_json::Map::new();
        parent.insert("x".repeat(256), value);
        value = serde_json::Value::Object(parent);
    }
    let body = serde_json::to_vec(&value).unwrap();
    assert!(body.len() < HTTP_RESPONSE_BODY_BYTES);
    let error = response_json_received_values(&body, Some("application/json"), "/", "").unwrap_err();
    assert!(error.contains("8 MiB"), "repeated path copies must be charged: {error}");
}

#[test]
fn json_value_paths_are_bounded_and_provenance_is_utf8_safe() {
    use crate::app::module::common::http::response_json_received_values;
    let mut object = serde_json::Map::new();
    object.insert("x".repeat(257), serde_json::json!(1));
    let body = serde_json::to_vec(&object).unwrap();
    assert!(response_json_received_values(&body, Some("application/json"), "/", "")
        .unwrap_err()
        .contains("256-byte"));
    let path = "é".repeat(3000);
    let values = response_json_received_values(b"1", Some("application/json"), &path, "value")
        .unwrap()
        .unwrap();
    assert!(values[0].source_description.len() <= 4096);
    assert!(values[0].source_description.ends_with("..."));
}
