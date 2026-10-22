use super::*;

#[test]
fn blocked_native_cleanup_keeps_its_payload_and_worker_admission_without_blocking_stop() {
    let lifetimes = slots();
    let output = pool(2);
    let control = Control::default();
    let (entered, cleanup) = mpsc::sync_channel(1);
    let (release, gate) = mpsc::sync_channel(1);
    let worker_control = control.clone();
    let mut handle = MqttTransportHandle::spawn_using(
        config(),
        NodeId(1),
        notify(),
        &lifetimes,
        output.clone(),
        pool(8),
        move |_| {
            let mut broker = FakeBroker::new(worker_control);
            broker.cleanup = Some((entered, gate));
            broker
        },
    )
    .unwrap();
    until(|| handle.connected.load(Ordering::Acquire));
    let receipt = handle.send(request(), false).unwrap();
    until(|| control.published.load(Ordering::Acquire) == 1);
    handle.stop();
    cleanup.recv_timeout(Duration::from_secs(1)).unwrap();
    assert!(
        output.try_reserve(1).is_err(),
        "library-held copy retains its source reservation during cleanup"
    );
    assert_eq!(receipt.status(), PublishStatus::Pending);
    for _ in 0..3 {
        assert!(MqttTransportHandle::spawn_using(
            config(),
            NodeId(1),
            notify(),
            &lifetimes,
            pool(8),
            pool(8),
            |_| -> FakeBroker { panic!("retiring worker released its slot early") }
        )
        .is_err());
    }
    release.send(()).unwrap();
    until(|| receipt.status() == PublishStatus::FailedUnconfirmed);
    drop(handle);
    assert!(lifetimes.wait_for_idle(Duration::from_secs(1)));
    assert!(output.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn full_command_mailbox_rejects_admission_and_every_accepted_command_gets_a_cancelled_outcome() {
    let lifetimes = slots();
    let output = pool(256);
    let control = Control::default();
    control.busy.store(true, Ordering::Release);
    let mut handle = fake(control, &lifetimes, output.clone(), pool(8));
    let mut receipts = Vec::new();
    for _ in 0..256 {
        match handle.send(request(), false) {
            Ok(receipt) => receipts.push(receipt),
            Err(_) => break,
        }
    }
    assert!(!receipts.is_empty() && receipts.len() <= COMMAND_ITEMS + 8);
    assert!(handle.send(request(), false).is_err());
    handle.stop();
    drop(handle);
    until(|| {
        receipts
            .iter()
            .all(|receipt| receipt.status() == PublishStatus::FailedUnconfirmed)
    });
    assert!(lifetimes.wait_for_idle(Duration::from_secs(1)));
    assert!(output.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn drained_incoming_payloads_keep_their_quota_and_cancel_capacity_waits() {
    let lifetimes = slots();
    let input = pool(1);
    let control = Control::default();
    let mut handle = fake(control.clone(), &lifetimes, pool(8), input.clone());
    control.event(Event::Incoming(Packet::Publish(rumqttc::Publish::new(
        "incoming",
        rumqttc::QoS::AtMostOnce,
        vec![1; 1024],
    ))));
    control.event(Event::Incoming(Packet::Publish(rumqttc::Publish::new(
        "incoming",
        rumqttc::QoS::AtMostOnce,
        vec![2; 1024],
    ))));
    let mut received = Vec::new();
    until(|| {
        handle.drain_events(&mut received);
        received
            .iter()
            .any(|event| matches!(event, MqttWorkerEvent::Publish(_)))
    });
    assert!(
        input.try_reserve(1).is_err(),
        "drain must not refund a consumer-owned payload"
    );
    handle.stop();
    drop(handle);
    assert!(
        lifetimes.wait_for_idle(Duration::from_secs(1)),
        "full input capacity must be cancellable"
    );
    assert!(!input.wait_for_idle(Duration::from_millis(10)));
    drop(received);
    assert!(input.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn native_worker_panic_has_an_explicit_unconfirmed_result_for_admitted_publications() {
    let lifetimes = slots();
    let output = pool(8);
    let control = Control::default();
    let handle = fake(control.clone(), &lifetimes, output.clone(), pool(8));
    let receipt = handle.send(request(), false).unwrap();
    until(|| control.published.load(Ordering::Acquire) == 1);
    control.panic.store(true, Ordering::Release);
    until(|| !handle.is_connected());
    until(|| receipt.status() == PublishStatus::FailedUnconfirmed);
    drop(handle);
    assert!(lifetimes.wait_for_idle(Duration::from_secs(1)));
    assert!(output.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn stalled_dns_retains_the_worker_slot_and_cancellation_prevents_a_late_connect() {
    use std::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let mut cfg = config();
    cfg.remote_port = address.port();
    let lifetimes = slots();
    let calls = Arc::new(AtomicUsize::new(0));
    let called = calls.clone();
    let (entered, resolving) = mpsc::sync_channel(1);
    let (release, gate) = mpsc::sync_channel(1);
    let mut handle =
        MqttTransportHandle::spawn_using(cfg, NodeId(1), notify(), &lifetimes, pool(8), pool(8), move |config| {
            worker::NativeBroker::with_resolver(config, move |_, _| {
                called.fetch_add(1, Ordering::AcqRel);
                let _ = entered.send(());
                let _ = gate.recv();
                Ok(vec![address])
            })
        })
        .unwrap();
    resolving.recv_timeout(Duration::from_secs(1)).unwrap();
    handle.stop();
    for _ in 0..3 {
        assert!(MqttTransportHandle::spawn_using(
            config(),
            NodeId(1),
            notify(),
            &lifetimes,
            pool(8),
            pool(8),
            |_| -> FakeBroker { panic!("blocked resolver released its lifetime slot") }
        )
        .is_err());
    }
    assert_eq!(calls.load(Ordering::Acquire), 1);
    release.send(()).unwrap();
    drop(handle);
    assert!(lifetimes.wait_for_idle(Duration::from_secs(1)));
    assert_eq!(listener.accept().unwrap_err().kind(), std::io::ErrorKind::WouldBlock);
}
