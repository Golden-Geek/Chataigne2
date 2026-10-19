use super::*;
use golden_io::RetirementPool;
use std::{net::UdpSocket, num::NonZeroUsize, sync::mpsc, thread};

fn packet(port: u16, byte: u8) -> UdpStreamingOutboundPacket {
    UdpStreamingOutboundPacket {
        bytes: vec![byte],
        remote_host: "127.0.0.1".into(),
        remote_port: port,
    }
}

#[test]
fn reconfiguration_waits_for_old_sends_and_socket_release_before_rebinding() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let old_socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    let bind_port = old_socket.local_addr().unwrap().port();
    let receiver = UdpSocket::bind("127.0.0.1:0").unwrap();
    receiver.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    let port = receiver.local_addr().unwrap().port();
    let config = UdpStreamingTransportConfig {
        bind_interface_host: "127.0.0.1".into(),
        bind_port,
        receive_enabled: false,
    };
    let mut next = config.clone();
    next.receive_enabled = true;
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let mut transports = UdpTransports::default();
    transports
        .configure_using(config, |config| {
            UdpStreamingTransportHandle::spawn_with_setup(config, &pool, move || {
                release_rx.recv().unwrap();
                Ok(old_socket)
            })
        })
        .unwrap();
    transports.send(packet(port, 1)).unwrap();
    assert!(!transports
        .configure_using(next.clone(), |_| panic!("old generation still owns its socket"))
        .unwrap());
    assert!(
        transports.send(packet(port, 2)).is_err(),
        "pending configuration must fence new send admission"
    );
    release_tx.send(()).unwrap();
    let mut buffer = [0u8; 8];
    assert_eq!(receiver.recv(&mut buffer).unwrap(), 1);
    assert_eq!(buffer[0], 1);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    assert!(transports.poll().disconnected);
    assert!(transports
        .configure_using(next, |config| {
            let port = config.bind_port;
            UdpStreamingTransportHandle::spawn_with_setup(config, &pool, move || {
                UdpSocket::bind(("127.0.0.1", port)).map_err(|error| error.to_string())
            })
        })
        .unwrap());
    let deadline = Instant::now() + Duration::from_secs(2);
    while !transports.connected() {
        let poll = transports.poll();
        assert!(
            poll.failure.is_none(),
            "the old socket must be released before binding the replacement"
        );
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
    transports.cancel_all();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    assert_eq!(pool.metrics().peak, 1);
}

#[test]
fn native_failure_closes_admission_and_recovery_obeys_backoff() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let config = UdpStreamingTransportConfig {
        bind_interface_host: "127.0.0.1".into(),
        bind_port: 0,
        receive_enabled: false,
    };
    let mut transports = UdpTransports::default();
    transports
        .configure_using(config.clone(), |config| {
            UdpStreamingTransportHandle::spawn_with_setup(config, &pool, || Err("controlled bind failure".into()))
        })
        .unwrap();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    let poll = transports.poll();
    assert_eq!(poll.failure.as_deref(), Some("controlled bind failure"));
    assert!(poll.disconnected);
    assert!(!transports
        .configure_using(config.clone(), |_| panic!("retry deadline has not elapsed"))
        .unwrap());
    // Exercise the same deadline check without a timing-sensitive sleep.
    transports.retry_at = Some(Instant::now() - Duration::from_millis(1));
    assert!(transports
        .configure_using(config, |config| {
            UdpStreamingTransportHandle::spawn_with_setup(config, &pool, || {
                UdpSocket::bind("127.0.0.1:0").map_err(|error| error.to_string())
            })
        })
        .unwrap());
    let deadline = Instant::now() + Duration::from_secs(2);
    while !transports.connected() {
        assert!(transports.poll().failure.is_none());
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
    transports.cancel_all();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}
