use super::*;
use std::{io::Write, net::TcpListener};

#[test]
fn failed_worker_is_detected_before_blocked_native_cleanup_releases_its_slot_or_payload() {
    let endpoint = TcpListener::bind("127.0.0.1:0").unwrap();
    let (mut engine, id) = create_mqtt_module();
    let lifetimes = slots();
    attach(
        &mut engine,
        id,
        Control::default(),
        &lifetimes,
        endpoint.local_addr().unwrap().port(),
    );
    module(&mut engine, id).stop_transport();
    assert!(lifetimes.wait_for_idle(Duration::from_secs(1)));
    let control = Control::default();
    let worker_control = control.clone();
    let (entered, cleanup) = mpsc::sync_channel(1);
    let (release, gate) = mpsc::sync_channel(1);
    let output = pool(2);
    let notifier = module(&mut engine, id).wake.as_ref().unwrap().notifier.clone();
    let handle =
        MqttTransportHandle::spawn_using(config(), id, notifier, &lifetimes, output.clone(), pool(8), move |_| {
            let mut broker = FakeBroker::new(worker_control);
            broker.cleanup = Some((entered, gate));
            broker
        })
        .unwrap();
    until(|| handle.is_connected());
    module(&mut engine, id).transport = Some(handle);
    module(&mut engine, id).incoming.activate();
    publish(&mut engine, id, "owned through cleanup");
    until(|| control.published.load(Ordering::Acquire) == 1);
    let receipt = module(&mut engine, id).pending_publishes.front().unwrap().0.clone();
    control.panic.store(true, Ordering::Release);
    cleanup.recv_timeout(Duration::from_secs(3)).unwrap();
    until(|| {
        turn(&mut engine);
        module(&mut engine, id).retry_at.is_some()
    });
    assert!(module(&mut engine, id).transport.is_none());
    assert_eq!(lifetimes.metrics().active, 1);
    assert!(output.try_reserve(1).is_err());
    assert_eq!(receipt.status(), PublishStatus::Pending);
    module(&mut engine, id).retry_at = None;
    release.send(()).unwrap();
    assert!(lifetimes.wait_for_idle(Duration::from_secs(1)));
    until(|| {
        turn(&mut engine);
        receipt.status() == PublishStatus::FailedUnconfirmed && module(&mut engine, id).pending_publishes.is_empty()
    });
    assert!(output.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn worker_failure_wakes_the_engine_and_coarse_retry_recovers_on_the_authored_loopback_endpoint() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();
    let (sent, received) = mpsc::sync_channel(1);
    let (release, gate) = mpsc::sync_channel(1);
    let broker = std::thread::spawn(move || {
        let mut connection = None;
        until(|| match listener.accept() {
            Ok((stream, _)) => {
                connection = Some(stream);
                true
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => false,
            Err(error) => panic!("{error}"),
        });
        let mut connection = connection.unwrap();
        connection.set_nonblocking(false).unwrap();
        connection.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        assert_eq!(super::super::wire::packet(&mut connection).0, 0x10);
        connection.write_all(&[0x20, 0x02, 0x00, 0x00]).unwrap();
        loop {
            let (kind, bytes) = super::super::wire::packet(&mut connection);
            match kind & 0xf0 {
                0x80 => connection.write_all(&[0x90, 0x03, bytes[0], bytes[1], 0x00]).unwrap(),
                0x30 => {
                    let topic = usize::from(u16::from_be_bytes([bytes[0], bytes[1]]));
                    assert_eq!(&bytes[4 + topic..], b"recovered");
                    connection
                        .write_all(&[0x40, 0x02, bytes[2 + topic], bytes[3 + topic]])
                        .unwrap();
                    sent.send(()).unwrap();
                    gate.recv_timeout(Duration::from_secs(3)).unwrap();
                    break;
                }
                0xc0 => connection.write_all(&[0xd0, 0]).unwrap(),
                other => panic!("unexpected recovery packet {other:x}"),
            }
        }
    });
    let (mut engine, id) = create_mqtt_module();
    let control = Control::default();
    let lifetimes = slots();
    attach(&mut engine, id, control.clone(), &lifetimes, port);
    publish(&mut engine, id, "must-not-replay");
    until(|| control.published.load(Ordering::Acquire) == 1);
    let original = module(&mut engine, id).pending_publishes.front().unwrap().0.clone();
    control.panic.store(true, Ordering::Release);
    until(|| {
        turn(&mut engine);
        module(&mut engine, id).retry_at.is_some()
    });
    assert_eq!(original.status(), PublishStatus::FailedUnconfirmed);
    assert!(module(&mut engine, id).transport.is_none());
    assert!(module(&mut engine, id).retry_at.unwrap() > Instant::now());
    quiet(&mut engine);
    module(&mut engine, id).retry_at = Some(Instant::now() - Duration::from_secs(1));
    module(&mut engine, id).request_service();
    until(|| {
        turn(&mut engine);
        module(&mut engine, id)
            .transport
            .as_ref()
            .is_some_and(MqttTransportHandle::is_connected)
    });
    publish(&mut engine, id, "recovered");
    received.recv_timeout(Duration::from_secs(3)).unwrap();
    until(|| {
        turn(&mut engine);
        module(&mut engine, id).pending_publishes.is_empty()
    });
    assert_eq!(
        control.published.load(Ordering::Acquire),
        1,
        "failed work must never replay"
    );
    module(&mut engine, id).stop_transport();
    release.send(()).unwrap();
    broker.join().unwrap();
    assert!(lifetimes.wait_for_idle(Duration::from_secs(1)));
}
