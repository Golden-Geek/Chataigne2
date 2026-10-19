use super::*;
mod input;
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

fn config() -> TcpStreamingTransportConfig {
    TcpStreamingTransportConfig {
        remote_host: "127.0.0.1".into(),
        remote_port: 9002,
        receive_enabled: false,
        send_enabled: true,
        input: StreamInputConfig {
            module: golden_core::node::NodeId(1),
            parse: Default::default(),
            processing_enabled: true,
            log_incoming: false,
        },
    }
}
fn pool() -> RetirementPool {
    RetirementPool::new(NonZeroUsize::MIN)
}
fn notifier() -> WorkerNotifier {
    WorkerNotifier::new(|| {})
}
fn pair() -> (TcpStream, TcpStream) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let peer = listener.accept().unwrap().0;
    peer.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    peer.set_write_timeout(Some(Duration::from_secs(3))).unwrap();
    (client, peer)
}
fn wait(mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !ready() {
        assert!(Instant::now() < deadline, "TCP fixture did not reach expected state");
        thread::sleep(Duration::from_millis(1));
    }
}
fn drain(handle: &TcpStreamingTransportHandle) -> Vec<StreamingWorkerEvent> {
    let mut events = Vec::new();
    wait(|| handle.drain_events(&mut events).state == golden_io::PendingDrainState::Disconnected);
    events
}

#[test]
fn blocked_endpoint_setup_returns_early_and_cancellation_retains_its_single_native_slot() {
    let pool = pool();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let mut handle = TcpStreamingTransportHandle::spawn_with_connector(config(), &pool, notifier(), move |_| {
        entered_tx.send(()).unwrap();
        release_rx.recv().unwrap();
        Err("injected resolution failure".into())
    })
    .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(handle.send(&[1], false).unwrap_err().contains("not connected"));
    handle.stop();
    assert_eq!(pool.metrics().active, 1);
    assert!(
        TcpStreamingTransportHandle::spawn_with_connector(config(), &pool, notifier(), |_| Err("unavailable".into()))
            .is_err()
    );
    release_tx.send(()).unwrap();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn full_command_and_reply_queues_retire_every_accepted_send_and_log_once() {
    let pool = pool();
    let (client, mut peer) = pair();
    let mut connection = Some(client);
    let mut handle = TcpStreamingTransportHandle::spawn_with_connector(config(), &pool, notifier(), move |_| {
        connection.take().ok_or("no replacement".into())
    })
    .unwrap();
    wait(|| handle.connection.load(Ordering::Acquire) != 0);
    let mut expected = Vec::new();
    for index in 0..1000 {
        let value = index as u8;
        match handle.send(&[value], true) {
            Ok(()) => expected.push(value),
            Err(error) => {
                assert!(error.contains("capacity exhausted"));
                break;
            }
        }
    }
    assert!(!expected.is_empty() && expected.len() <= 321);
    handle.finish_admission();
    assert!(handle.send(&[99], false).is_err());
    wait(|| handle.worker.event_metrics().rejected > 0);
    assert!(!pool.wait_for_idle(Duration::from_millis(20)));
    let events = drain(&handle);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    let mut actual = Vec::new();
    peer.read_to_end(&mut actual).unwrap();
    assert_eq!(actual, expected);
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, StreamingWorkerEvent::OutgoingLog(_)))
            .count(),
        expected.len()
    );
    assert!(!events
        .iter()
        .any(|event| matches!(event, StreamingWorkerEvent::Error(_))));
}

#[test]
fn an_old_generation_admission_race_cannot_send_on_a_replacement_connection() {
    let pool = pool();
    let (first, first_peer) = pair();
    let (second, mut second_peer) = pair();
    let mut sockets = std::collections::VecDeque::from([first, second]);
    let mut config = config();
    config.receive_enabled = true;
    let mut handle = TcpStreamingTransportHandle::spawn_with_connector(config, &pool, notifier(), move |_| {
        sockets.pop_front().ok_or("no replacement".into())
    })
    .unwrap();
    wait(|| handle.connection.load(Ordering::Acquire) == 1);
    drop(first_peer);
    wait(|| handle.connection.load(Ordering::Acquire) == 2);
    // Models a producer paused between loading generation 1 and its mailbox admission.
    let old = Outbound::new(b"old".to_vec(), 1, false).unwrap();
    let weight = old.weight();
    handle.worker.send_weighted(old, weight).unwrap();
    handle.send(b"new", false).unwrap();
    handle.finish_admission();
    let events = drain(&handle);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    let mut bytes = Vec::new();
    second_peer.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"new");
    assert_eq!(events.iter().filter(|event| matches!(event, StreamingWorkerEvent::Error(error) if error.contains("original connection") && error.contains("0/3"))).count(), 1);
}

#[test]
fn incoming_reply_pressure_does_not_gate_a_successful_unlogged_send() {
    let pool = pool();
    let (client, mut peer) = pair();
    let mut client = Some(client);
    let mut config = config();
    config.receive_enabled = true;
    let mut handle = TcpStreamingTransportHandle::spawn_with_connector(config, &pool, notifier(), move |_| {
        client.take().ok_or("no replacement".into())
    })
    .unwrap();
    wait(|| handle.connection.load(Ordering::Acquire) != 0);
    let mut sender = peer.try_clone().unwrap();
    let producer = thread::spawn(move || sender.write_all(&vec![1; 8192 * 80]));
    wait(|| handle.worker.event_metrics().rejected > 0);
    handle.send(b"out", false).unwrap();
    let mut bytes = [0; 3];
    peer.read_exact(&mut bytes).unwrap();
    handle.stop();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    let _ = producer.join().unwrap();
    assert_eq!(&bytes, b"out");
}
