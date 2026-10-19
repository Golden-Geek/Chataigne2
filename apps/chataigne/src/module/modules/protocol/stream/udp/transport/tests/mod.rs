use super::*;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::{sync::mpsc, thread};

fn config() -> UdpStreamingTransportConfig {
    UdpStreamingTransportConfig {
        bind_interface_host: "127.0.0.1".into(),
        bind_port: 0,
        receive_enabled: false,
    }
}

#[test]
fn starting_a_transport_does_not_wait_for_endpoint_setup() {
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let (handle_tx, handle_rx) = mpsc::sync_channel(1);
    let caller = thread::spawn(move || {
        let handle = UdpStreamingTransportHandle::spawn_using(config(), move || {
            entered_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            UdpSocket::bind("127.0.0.1:0").map_err(|error| error.to_string())
        })
        .unwrap();
        handle_tx.send(handle).unwrap();
    });
    entered_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    let early = handle_rx.recv_timeout(Duration::from_millis(100));
    let returned_before_setup = early.is_ok();
    release_tx.send(()).unwrap();
    let mut handle = early.unwrap_or_else(|_| handle_rx.recv_timeout(Duration::from_secs(3)).unwrap());
    handle.stop();
    caller.join().unwrap();
    assert!(returned_before_setup, "endpoint setup must not block the engine caller");
}

#[test]
fn impossible_datagram_is_rejected_before_transport_admission() {
    let mut handle = UdpStreamingTransportHandle::spawn(config()).unwrap();
    let result = handle.send(UdpStreamingOutboundPacket {
        bytes: vec![0; 65_536],
        remote_host: "127.0.0.1".into(),
        remote_port: 9000,
    });
    handle.stop();
    assert!(
        result.is_err(),
        "oversized datagrams must fail admission rather than enter the worker queue"
    );
}

#[test]
fn byte_admission_and_cancellation_remain_bounded_during_blocked_setup() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let mut handle = UdpStreamingTransportHandle::spawn_with_setup(config(), &pool, move || {
        entered_tx.send(()).unwrap();
        release_rx.recv().unwrap();
        UdpSocket::bind("127.0.0.1:0").map_err(|error| error.to_string())
    })
    .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let mut accepted = 0;
    for _ in 0..64 {
        if handle
            .send(UdpStreamingOutboundPacket {
                bytes: vec![0; 60_000],
                remote_host: "127.0.0.1".into(),
                remote_port: 9000,
            })
            .is_err()
        {
            break;
        }
        accepted += 1;
    }
    assert!(accepted > 0 && accepted <= UDP_COMMAND_BYTES / 60_000);
    let (stopped_tx, stopped_rx) = mpsc::sync_channel(1);
    let owner = thread::spawn(move || {
        handle.stop();
        stopped_tx.send(()).unwrap();
    });
    let stopped = stopped_rx.recv_timeout(Duration::from_millis(100)).is_ok();
    assert_eq!(
        pool.metrics().active,
        1,
        "blocked native setup keeps its reserved lifetime"
    );
    let rejected = UdpStreamingTransportHandle::spawn_with_setup(config(), &pool, || {
        UdpSocket::bind("127.0.0.1:0").map_err(|e| e.to_string())
    });
    release_tx.send(()).unwrap();
    owner.join().unwrap();
    assert!(stopped, "cancellation must not join blocked setup");
    assert!(
        rejected.is_err(),
        "replacement must not escape the native lifetime bound"
    );
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn graceful_close_attempts_all_accepted_packets_in_order() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let receiver = UdpSocket::bind("127.0.0.1:0").unwrap();
    receiver.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    let port = receiver.local_addr().unwrap().port();
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let mut handle = UdpStreamingTransportHandle::spawn_with_setup(config(), &pool, move || {
        release_rx.recv().unwrap();
        UdpSocket::bind("127.0.0.1:0").map_err(|error| error.to_string())
    })
    .unwrap();
    for value in 0..32u8 {
        handle
            .send(UdpStreamingOutboundPacket {
                bytes: vec![value],
                remote_host: "127.0.0.1".into(),
                remote_port: port,
            })
            .unwrap();
    }
    handle.finish_admission();
    assert!(handle
        .send(UdpStreamingOutboundPacket {
            bytes: vec![99],
            remote_host: "127.0.0.1".into(),
            remote_port: port
        })
        .is_err());
    release_tx.send(()).unwrap();
    let mut buffer = [0u8; 8];
    for expected in 0..32u8 {
        let length = receiver.recv(&mut buffer).unwrap();
        assert_eq!(&buffer[..length], &[expected]);
    }
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn failed_setup_resolves_every_previously_admitted_send() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let handle = UdpStreamingTransportHandle::spawn_with_setup(config(), &pool, move || {
        release_rx.recv().unwrap();
        Err("injected bind failure".into())
    })
    .unwrap();
    for value in 0..3 {
        handle
            .send(UdpStreamingOutboundPacket {
                bytes: vec![value],
                remote_host: "127.0.0.1".into(),
                remote_port: 9000,
            })
            .unwrap();
    }
    release_tx.send(()).unwrap();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    let mut events = Vec::new();
    assert_eq!(handle.drain_events(&mut events).state, PendingDrainState::Disconnected);
    assert!(matches!(&events[0], StreamingWorkerEvent::Failed(error) if error == "injected bind failure"));
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, StreamingWorkerEvent::Error(_)))
            .count(),
        3
    );
    assert!(handle
        .send(UdpStreamingOutboundPacket {
            bytes: vec![9],
            remote_host: "127.0.0.1".into(),
            remote_port: 9000
        })
        .is_err());
}

#[test]
fn continuous_input_keeps_bounded_owner_turns_and_does_not_starve_sends_or_cancellation() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    let address = socket.local_addr().unwrap();
    let mut input = config();
    input.receive_enabled = true;
    let mut handle = UdpStreamingTransportHandle::spawn_with_setup(input, &pool, move || Ok(socket)).unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    loop {
        let mut events = Vec::new();
        handle.drain_events(&mut events);
        if events
            .iter()
            .any(|event| matches!(event, StreamingWorkerEvent::Ready(_)))
        {
            break;
        }
        assert!(std::time::Instant::now() < deadline);
        thread::yield_now();
    }
    let running = Arc::new(AtomicBool::new(true));
    let produce = running.clone();
    let producer = thread::spawn(move || {
        let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
        sender.set_nonblocking(true).unwrap();
        let payload = vec![7; 60_000];
        while produce.load(Ordering::Relaxed) {
            let _ = sender.send_to(&payload, address);
            thread::yield_now();
        }
    });
    let receiver = UdpSocket::bind("127.0.0.1:0").unwrap();
    receiver.set_nonblocking(true).unwrap();
    handle
        .send(UdpStreamingOutboundPacket {
            bytes: vec![99],
            remote_host: "127.0.0.1".into(),
            remote_port: receiver.local_addr().unwrap().port(),
        })
        .unwrap();
    let mut delivered = false;
    let mut received = 0;
    let mut maximum_turn = 0;
    let mut buffer = [0u8; 8];
    while std::time::Instant::now() < deadline && (!delivered || received < 12) {
        let mut events = Vec::new();
        let turn = handle.drain_events(&mut events);
        maximum_turn = maximum_turn.max(turn.received);
        received += events
            .iter()
            .filter(|event| matches!(event, StreamingWorkerEvent::Bytes(_)))
            .count();
        if let Ok(length) = receiver.recv(&mut buffer) {
            delivered |= buffer[..length] == [99];
        }
        thread::yield_now();
    }
    running.store(false, Ordering::Relaxed);
    handle.stop();
    producer.join().unwrap();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    assert!(maximum_turn <= UDP_EVENT_DRAIN_BUDGET.get());
    assert!(received >= 12, "continuous native input must reach bounded owner turns");
    assert!(delivered, "receive traffic must not starve an admitted outbound packet");
}
