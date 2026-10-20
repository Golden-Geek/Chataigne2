use super::super::{worker::OscSocket, OSC_EVENT_DRAIN_BUDGET};
use super::*;
use golden_io::{RetirementPool, WorkerNotifier};
use std::{
    collections::VecDeque,
    num::NonZeroUsize,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc, Mutex,
    },
    time::Instant,
};

#[derive(Default)]
struct FakeSocket {
    sent: Arc<Mutex<Vec<Vec<u8>>>>,
    incoming: VecDeque<Vec<u8>>,
    continuous: Option<Vec<u8>>,
    reads: Arc<AtomicUsize>,
    cleanup: Option<(mpsc::Sender<()>, mpsc::Receiver<()>)>,
    wait_failure: Arc<AtomicBool>,
    failed_wait: Arc<AtomicBool>,
}

impl FakeSocket {
    fn with_cleanup(mut self, entered: mpsc::Sender<()>, gate: mpsc::Receiver<()>) -> Self {
        self.cleanup = Some((entered, gate));
        self
    }

    fn with_continuous_input(mut self, bytes: Vec<u8>) -> Self {
        self.continuous = Some(bytes);
        self
    }
}

impl OscSocket for FakeSocket {
    fn address(&self) -> std::net::SocketAddr {
        "127.0.0.1:45000".parse().unwrap()
    }
    fn receive(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let packet = self
            .incoming
            .pop_front()
            .or_else(|| self.continuous.clone())
            .ok_or(io::ErrorKind::WouldBlock)?;
        bytes[..packet.len()].copy_from_slice(&packet);
        self.reads.fetch_add(1, Ordering::AcqRel);
        Ok(packet.len())
    }
    fn send(&mut self, bytes: &[u8], _: std::net::SocketAddr) -> io::Result<usize> {
        self.sent.lock().unwrap().push(bytes.to_vec());
        Ok(bytes.len())
    }
    fn wait(&mut self, _: Duration) -> io::Result<()> {
        std::thread::sleep(Duration::from_millis(1));
        if self.wait_failure.load(Ordering::Acquire) {
            self.failed_wait.store(true, Ordering::Release);
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "injected poll failure"));
        }
        Ok(())
    }
}

impl Drop for FakeSocket {
    fn drop(&mut self) {
        if let Some((entered, gate)) = self.cleanup.take() {
            entered.send(()).unwrap();
            gate.recv().unwrap();
        }
    }
}

fn config() -> OscTransportConfig {
    OscTransportConfig {
        bind_interface_host: "127.0.0.1".into(),
        bind_port: 0,
        receive_enabled: true,
    }
}

fn message(index: usize) -> OscOutboundMessage {
    OscOutboundMessage {
        address: Arc::from(format!("/ordered/{index}")),
        payload: OscValuePayload::Single(ParamValue::Trigger()),
        remote_address: "127.0.0.1:45001".parse().unwrap(),
    }
}

fn start(pool: &RetirementPool, socket: FakeSocket) -> OscTransportHandle {
    OscTransportHandle::spawn_using(config(), pool, WorkerNotifier::new(|| {}), move |_, _| Ok(socket)).unwrap()
}

fn drain_to_exit(handle: &OscTransportHandle) -> Vec<super::super::OscWorkerEvent> {
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut events = Vec::new();
    loop {
        let before = events.len();
        let turn = handle.drain_events(&mut events);
        assert!(events.len() - before <= OSC_EVENT_DRAIN_BUDGET.get());
        if turn.state == PendingDrainState::Disconnected {
            return events;
        }
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
}

#[test]
fn captured_requests_reserve_whole_command_capacity_before_worker_preparation() {
    use super::super::super::osc_outbound::{OutboundRequest, QueueError};
    use golden_core::node::Folder;
    let engine = crate::app::AppEngine::new(Folder::new("producer").into());
    let snapshot = engine.process_tree_snapshot();
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let (release, gate) = mpsc::channel();
    let mut handle =
        OscTransportHandle::spawn_using::<FakeSocket>(config(), &pool, WorkerNotifier::new(|| {}), move |_, _| {
            gate.recv().unwrap();
            Err("injected setup failure".into())
        })
        .unwrap();
    for _ in 0..4 {
        handle
            .enqueue(
                OutboundRequest::values(
                    &snapshot,
                    engine.root,
                    engine.root,
                    engine.root,
                    engine.node_waker(engine.root).unwrap(),
                    false,
                )
                .unwrap(),
            )
            .unwrap();
    }
    let rejected = handle.enqueue(
        OutboundRequest::values(
            &snapshot,
            engine.root,
            engine.root,
            engine.root,
            engine.node_waker(engine.root).unwrap(),
            false,
        )
        .unwrap(),
    );
    assert!(matches!(rejected, Err(QueueError::Full)));
    handle.finish_admission();
    release.send(()).unwrap();
    let events = drain_to_exit(&handle);
    assert_eq!(events.len(), 5);
    assert!(events
        .iter()
        .skip(1)
        .all(|event| matches!(event, super::super::OscWorkerEvent::Error(_))));
    assert!(pool.wait_for_idle(Duration::from_secs(3)));
}

#[test]
fn startup_failure_finishes_every_accepted_command_without_replay() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let (release, gate) = mpsc::channel();
    let mut handle =
        OscTransportHandle::spawn_using::<FakeSocket>(config(), &pool, WorkerNotifier::new(|| {}), move |_, _| {
            gate.recv().unwrap();
            Err("injected bind failure".into())
        })
        .unwrap();
    assert!(!handle.is_ready());
    for index in 0..256 {
        handle.send(message(index)).unwrap();
    }
    assert!(handle.send(message(256)).unwrap_err().contains("capacity"));
    handle.finish_admission();
    release.send(()).unwrap();
    let events = drain_to_exit(&handle);
    assert_eq!(events.len(), 257);
    for (index, event) in events.iter().skip(1).enumerate() {
        assert!(
            matches!(event, super::super::OscWorkerEvent::Error(error) if error.contains(&format!("/ordered/{index} failed before send")))
        );
    }
    assert!(pool.wait_for_idle(Duration::from_secs(3)));
}

#[test]
fn outgoing_byte_quota_rejects_before_the_item_capacity_and_keeps_the_prefix() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let (release, gate) = mpsc::channel();
    let mut handle =
        OscTransportHandle::spawn_using::<FakeSocket>(config(), &pool, WorkerNotifier::new(|| {}), move |_, _| {
            gate.recv().unwrap();
            Err("injected setup failure".into())
        })
        .unwrap();
    let mut accepted = 0;
    loop {
        let mut command = message(accepted);
        command.payload = OscValuePayload::Single(ParamValue::Str("x".repeat(64 * 1024)));
        match handle.send(command) {
            Ok(()) => accepted += 1,
            Err(error) => {
                assert!(error.contains("capacity"));
                break;
            }
        }
        assert!(accepted < 256);
    }
    assert!(accepted > 0 && accepted < 256);
    handle.finish_admission();
    release.send(()).unwrap();
    let events = drain_to_exit(&handle);
    assert_eq!(events.len(), accepted + 1);
    assert!(pool.wait_for_idle(Duration::from_secs(3)));
}

#[test]
fn oversized_encoded_datagram_reports_failure_without_attempting_native_send() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let socket = FakeSocket::default();
    let sent = socket.sent.clone();
    let mut handle = start(&pool, socket);
    wait_ready(&handle);
    let mut command = message(1);
    command.payload = OscValuePayload::Single(ParamValue::Str("x".repeat(70 * 1024)));
    handle.send(command).unwrap();
    handle.finish_admission();
    let events = drain_to_exit(&handle);
    assert!(sent.lock().unwrap().is_empty());
    assert!(events
        .iter()
        .any(|event| matches!(event, super::super::OscWorkerEvent::Error(error) if error.contains("65507 bytes"))));
    assert!(pool.wait_for_idle(Duration::from_secs(3)));
}

#[test]
fn startup_cancellation_is_nonblocking_and_holds_capacity_until_setup_returns() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let (entered, ready) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let handle = OscTransportHandle::spawn_using(config(), &pool, WorkerNotifier::new(|| {}), move |_, _| {
        entered.send(()).unwrap();
        gate.recv().unwrap();
        Ok(FakeSocket::default())
    })
    .unwrap();
    ready.recv_timeout(Duration::from_secs(3)).unwrap();
    let (done, dropped) = mpsc::channel();
    let owner = std::thread::spawn(move || {
        drop(handle);
        done.send(()).unwrap();
    });
    let nonblocking = dropped.recv_timeout(Duration::from_secs(3)).is_ok();
    let rejected = OscTransportHandle::spawn_using(config(), &pool, WorkerNotifier::new(|| {}), |_, _| {
        Ok(FakeSocket::default())
    });
    release.send(()).unwrap();
    owner.join().unwrap();
    assert!(nonblocking);
    assert!(rejected.is_err());
    assert!(pool.wait_for_idle(Duration::from_secs(3)));
    let mut recovered = start(&pool, FakeSocket::default());
    wait_ready(&recovered);
    recovered.stop();
    assert!(pool.wait_for_idle(Duration::from_secs(3)));
}

#[test]
fn blocked_native_cleanup_keeps_the_lifetime_without_joining_its_owner() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let (entered, cleanup) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let mut handle = start(&pool, FakeSocket::default().with_cleanup(entered, gate));
    wait_ready(&handle);
    handle.finish_admission();
    cleanup.recv_timeout(Duration::from_secs(3)).unwrap();
    let (done, dropped) = mpsc::channel();
    let owner = std::thread::spawn(move || {
        drop(handle);
        done.send(()).unwrap();
    });
    let nonblocking = dropped.recv_timeout(Duration::from_secs(3)).is_ok();
    let rejected = OscTransportHandle::spawn_using(config(), &pool, WorkerNotifier::new(|| {}), |_, _| {
        Ok(FakeSocket::default())
    });
    release.send(()).unwrap();
    owner.join().unwrap();
    assert!(nonblocking);
    assert!(rejected.is_err());
    assert!(pool.wait_for_idle(Duration::from_secs(3)));
}

#[test]
fn native_wake_ownership_is_cleared_before_the_lifetime_slot_is_released() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let mut handle = OscTransportHandle::spawn_using(
        config(),
        &pool,
        WorkerNotifier::new(|| {}),
        super::super::worker::NativeSocket::open,
    )
    .unwrap();
    wait_ready(&handle);
    assert!(handle.wake.source.lock().unwrap().is_some());
    handle.stop();
    assert!(pool.wait_for_idle(Duration::from_secs(3)));
    assert!(handle.wake.source.lock().unwrap().is_none());
    assert!(!handle.is_ready());
}

#[test]
fn full_input_preserves_occurrences_and_services_independent_output() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let bytes = encoder::encode(&OscPacket::Message(OscMessage {
        addr: "/trigger".into(),
        args: vec![],
    }))
    .unwrap();
    let socket = FakeSocket::default().with_continuous_input(bytes);
    let sent = socket.sent.clone();
    let reads = socket.reads.clone();
    let mut handle = start(&pool, socket);
    let deadline = Instant::now() + Duration::from_secs(3);
    while handle.worker.event_metrics().depth < 128 {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    handle.send(message(1)).unwrap();
    while sent.lock().unwrap().is_empty() {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    assert!(reads.load(Ordering::Acquire) <= 129);
    handle.finish_admission();
    let events = drain_to_exit(&handle);
    let delivered = events
        .iter()
        .filter(|event| matches!(event, super::super::OscWorkerEvent::Message(_)))
        .count();
    assert_eq!(delivered, reads.load(Ordering::Acquire));
    assert_eq!(sent.lock().unwrap().len(), 1);
    assert!(pool.wait_for_idle(Duration::from_secs(3)));
}

#[test]
fn native_wake_failure_does_not_turn_an_accepted_send_into_a_retry() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let socket = FakeSocket::default();
    let sent = socket.sent.clone();
    let mut handle = start(&pool, socket);
    wait_ready(&handle);
    handle.wake.failed.store(true, Ordering::Release);
    handle.send(message(1)).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut events = Vec::new();
    while !events.iter().any(
        |event| matches!(event, super::super::OscWorkerEvent::Error(error) if error.contains("native wake failed")),
    ) {
        handle.drain_events(&mut events);
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    handle.finish_admission();
    events.extend(drain_to_exit(&handle));
    assert_eq!(sent.lock().unwrap().len(), 1);
    assert!(events
        .iter()
        .all(|event| !matches!(event, super::super::OscWorkerEvent::Message(_))));
    assert!(pool.wait_for_idle(Duration::from_secs(3)));
}

#[test]
fn native_failure_preserves_the_received_bundle_prefix_behind_a_full_mailbox() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let content = (0..200)
        .map(|index| {
            OscPacket::Message(OscMessage {
                addr: format!("/prefix/{index}"),
                args: vec![],
            })
        })
        .collect();
    let bytes = encoder::encode(&OscPacket::Bundle(rosc::OscBundle {
        timetag: (0, 1).into(),
        content,
    }))
    .unwrap();
    let mut socket = FakeSocket::default();
    socket.incoming.push_back(bytes);
    let wait_failure = socket.wait_failure.clone();
    let failed_wait = socket.failed_wait.clone();
    let handle = start(&pool, socket);
    let deadline = Instant::now() + Duration::from_secs(3);
    while handle.worker.event_metrics().depth < 128 {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    wait_failure.store(true, Ordering::Release);
    while !failed_wait.load(Ordering::Acquire) {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    let events = drain_to_exit(&handle);
    let addresses = events
        .iter()
        .filter_map(|event| match event {
            super::super::OscWorkerEvent::Message(message) => Some(message.address.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        addresses,
        (0..200).map(|index| format!("/prefix/{index}")).collect::<Vec<_>>()
    );
    assert!(events
        .iter()
        .any(|event| matches!(event, super::super::OscWorkerEvent::Failed(error) if error.contains("poll failed"))));
    assert!(pool.wait_for_idle(Duration::from_secs(3)));
}
