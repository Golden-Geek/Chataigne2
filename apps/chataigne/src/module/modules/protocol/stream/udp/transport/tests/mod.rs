use super::super::outbound::OutputPlanBuilder;
use super::*;
mod backpressure;
mod fanout;
mod preparation;
mod replies;
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
        input: input_config(),
    }
}

#[test]
fn starting_a_transport_does_not_wait_for_endpoint_setup() {
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let (handle_tx, handle_rx) = mpsc::sync_channel(1);
    let caller = thread::spawn(move || {
        let handle = UdpStreamingTransportHandle::spawn_using(config(), notifier(), move || {
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
    let plan = OutputPlanBuilder::default().finish().unwrap();
    assert!(UdpOutboundSend::new(vec![0; 65_536], plan, false).is_err());
}

#[test]
fn byte_admission_and_cancellation_remain_bounded_during_blocked_setup() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let mut handle = UdpStreamingTransportHandle::spawn_with_setup(config(), &pool, notifier(), move || {
        entered_tx.send(()).unwrap();
        release_rx.recv().unwrap();
        UdpSocket::bind("127.0.0.1:0").map_err(|error| error.to_string())
    })
    .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let mut accepted = 0;
    for _ in 0..64 {
        if handle
            .send(packet_to(vec![0; 60_000], "127.0.0.1".into(), 9000))
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
    let rejected = UdpStreamingTransportHandle::spawn_with_setup(config(), &pool, notifier(), || {
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
    let mut handle = UdpStreamingTransportHandle::spawn_with_setup(config(), &pool, notifier(), move || {
        release_rx.recv().unwrap();
        UdpSocket::bind("127.0.0.1:0").map_err(|error| error.to_string())
    })
    .unwrap();
    for value in 0..32u8 {
        handle.send(packet_to(vec![value], "127.0.0.1".into(), port)).unwrap();
    }
    handle.finish_admission();
    assert!(handle.send(packet_to(vec![99], "127.0.0.1".into(), port)).is_err());
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
    let handle = UdpStreamingTransportHandle::spawn_with_setup(config(), &pool, notifier(), move || {
        release_rx.recv().unwrap();
        Err("injected bind failure".into())
    })
    .unwrap();
    for value in 0..3 {
        handle.send(packet_to(vec![value], "127.0.0.1".into(), 9000)).unwrap();
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
    assert!(handle.send(packet_to(vec![9], "127.0.0.1".into(), 9000)).is_err());
}

#[test]
fn continuous_input_keeps_bounded_owner_turns_and_does_not_starve_sends_or_cancellation() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    let address = socket.local_addr().unwrap();
    let mut input = config();
    input.receive_enabled = true;
    let mut handle =
        UdpStreamingTransportHandle::spawn_with_setup(input, &pool, notifier(), move || Ok(socket)).unwrap();
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
        .send(packet_to(
            vec![99],
            "127.0.0.1".into(),
            receiver.local_addr().unwrap().port(),
        ))
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
            .filter(|event| matches!(event, StreamingWorkerEvent::Input(_)))
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

fn notifier() -> golden_io::WorkerNotifier {
    golden_io::WorkerNotifier::new(|| {})
}

fn input_config() -> crate::app::module::common::streaming::input::StreamInputConfig {
    crate::app::module::common::streaming::input::StreamInputConfig {
        module: golden_core::node::NodeId(1),
        parse: Default::default(),
        processing_enabled: false,
        log_incoming: false,
    }
}

fn next_input(handle: &UdpStreamingTransportHandle) -> RetainedPayload<PreparedStreamInput> {
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    loop {
        let mut events = Vec::new();
        handle.drain_events(&mut events);
        for event in events {
            match event {
                StreamingWorkerEvent::Input(input) => return input,
                StreamingWorkerEvent::Failed(error) | StreamingWorkerEvent::Error(error) => panic!("{error}"),
                _ => {}
            }
        }
        assert!(std::time::Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn input_is_prepared_on_its_worker_and_cannot_be_reinterpreted_after_reconfiguration() {
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    let address = socket.local_addr().unwrap();
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let mut config = config();
    config.receive_enabled = true;
    config.input.processing_enabled = true;
    let mut changed = config.input.clone();
    let mut handle =
        UdpStreamingTransportHandle::spawn_with_setup(config, &pool, notifier(), move || Ok(socket)).unwrap();
    let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
    sender.send_to(b"old 1\n", address).unwrap();
    let old = next_input(&handle);
    assert_eq!(old.values.as_ref().unwrap()[0].path_segments, ["old"]);
    assert_eq!(old.callbacks.len(), 3);
    assert_eq!(old.callbacks[0].payload()["callback"], "dataReceived");
    assert_eq!(old.callbacks[2].payload()["args"][0], "old 1\n");
    changed.parse.mode = "json".into();
    handle.update_input(changed);
    sender.send_to(br#"{"new":2}"#, address).unwrap();
    let new = next_input(&handle);
    assert_eq!(new.values.as_ref().unwrap()[0].path_segments, ["new"]);
    assert_eq!(old.values.as_ref().unwrap()[0].path_segments, ["old"]);
    handle.stop();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

fn packet_to(bytes: Vec<u8>, remote_host: String, remote_port: u16) -> UdpOutboundSend {
    let mut plan = OutputPlanBuilder::default();
    plan.push_target(UdpOutputTarget {
        remote_host,
        remote_port,
    })
    .unwrap();
    UdpOutboundSend::new(bytes, plan.finish().unwrap(), false).unwrap()
}

#[test]
fn processing_generations_reject_old_partial_lines_and_preserve_new_data() {
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    let address = socket.local_addr().unwrap();
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let mut config = config();
    config.receive_enabled = true;
    config.input.processing_enabled = true;
    let mut disabled = config.input.clone();
    disabled.processing_enabled = false;
    let enabled = config.input.clone();
    let mut handle =
        UdpStreamingTransportHandle::spawn_with_setup(config, &pool, notifier(), move || Ok(socket)).unwrap();
    let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
    sender.send_to(b"partial ", address).unwrap();
    assert!(next_input(&handle).values.as_ref().unwrap().is_empty());
    handle.update_input(disabled);
    sender.send_to(b"ignored\n", address).unwrap();
    let ignored = next_input(&handle);
    assert!(ignored.values.as_ref().unwrap().is_empty());
    assert_eq!(ignored.rejected_lines, 1);
    assert_eq!(ignored.callbacks.len(), 3);
    handle.update_input(enabled);
    sender.send_to(b"next 7\n", address).unwrap();
    let next = next_input(&handle);
    assert_eq!(next.values.as_ref().unwrap()[0].path_segments, ["next"]);
    handle.stop();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn maximum_raw_datagram_fits_worker_reply_and_payload_lifetime_admission() {
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    let address = socket.local_addr().unwrap();
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let mut config = config();
    config.receive_enabled = true;
    config.input.processing_enabled = true;
    config.input.parse.mode = "raw".into();
    let mut handle =
        UdpStreamingTransportHandle::spawn_with_setup(config, &pool, notifier(), move || Ok(socket)).unwrap();
    UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .send_to(&vec![7; UDP_MAX_PAYLOAD], address)
        .unwrap();
    let input = next_input(&handle);
    assert_eq!(input.values.as_ref().unwrap().len(), UDP_MAX_PAYLOAD);
    assert!(input.weight <= 32 * 1024 * 1024);
    assert_eq!(input.callbacks.len(), 3);
    handle.stop();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}
