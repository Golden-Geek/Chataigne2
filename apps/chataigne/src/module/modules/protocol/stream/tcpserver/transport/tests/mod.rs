use super::*;
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

mod input;
mod logging;

fn config(port: u16) -> TcpServerTransportConfig {
    TcpServerTransportConfig {
        bind_host: "127.0.0.1".into(),
        bind_port: port,
        receive_enabled: true,
        send_enabled: true,
        input: StreamInputConfig {
            module: golden_core::node::NodeId(1),
            parse: Default::default(),
            processing_enabled: true,
            log_incoming: false,
        },
    }
}
fn spawn(config: TcpServerTransportConfig) -> Result<TcpServerTransportHandle, String> {
    TcpServerTransportHandle::spawn(config, WorkerNotifier::new(|| {}))
}
fn spawn_using(
    config: TcpServerTransportConfig,
    pool: &RetirementPool,
    binder: impl FnMut(&TcpServerTransportConfig) -> Result<TcpListener, String> + Send + 'static,
) -> Result<TcpServerTransportHandle, String> {
    TcpServerTransportHandle::spawn_using(config, pool, binder, WorkerNotifier::new(|| {}))
}
fn wait(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(4);
    while !condition() {
        assert!(Instant::now() < deadline, "TCP server condition did not settle");
        thread::sleep(Duration::from_millis(1));
    }
}
fn until(handle: &TcpServerTransportHandle, mut condition: impl FnMut(&TcpServerWorkerEvent) -> bool) {
    wait(|| {
        let mut events = Vec::new();
        let drain = handle.drain_events(&mut events);
        let found = events.iter().any(&mut condition);
        assert!(found || drain.state != golden_io::PendingDrainState::Disconnected);
        found
    });
}
fn listening(handle: &TcpServerTransportHandle) -> SocketAddr {
    let mut address = None;
    until(handle, |event| match event {
        TcpServerWorkerEvent::Listening(value) => {
            address = Some(*value);
            true
        }
        _ => false,
    });
    address.unwrap()
}
fn connect(handle: &TcpServerTransportHandle, address: SocketAddr) -> TcpStream {
    let stream = TcpStream::connect(address).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    until(handle, |event| {
        matches!(event, TcpServerWorkerEvent::ClientConnected { .. })
    });
    stream
}

#[test]
fn blocked_native_setup_and_cleanup_keep_their_lifetime_without_blocking_stop() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let started = Instant::now();
    let mut handle = spawn_using(config(0), &pool, move |_| {
        started_tx.send(()).unwrap();
        release_rx.recv().unwrap();
        Err("injected delayed bind failure".into())
    })
    .unwrap();
    assert!(started.elapsed() < Duration::from_millis(100));
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let started = Instant::now();
    handle.stop();
    drop(handle);
    assert!(started.elapsed() < Duration::from_millis(100));
    assert_eq!(pool.metrics().active, 1);
    assert!(spawn_using(config(0), &pool, worker::bind).is_err());
    release_tx.send(()).unwrap();
    wait(|| pool.metrics().active == 0);
    let recovered = spawn_using(config(0), &pool, worker::bind).unwrap();
    listening(&recovered);
    drop(recovered);
    wait(|| pool.metrics().active == 0);
}

#[test]
fn occupied_endpoint_recovers_without_another_owner_edit() {
    let occupied = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = occupied.local_addr().unwrap();
    let handle = spawn(config(address.port())).unwrap();
    until(&handle, |event| matches!(event, TcpServerWorkerEvent::Recovering(_)));
    assert!(handle.send(b"not connected", false).is_err());
    drop(occupied);
    assert_eq!(listening(&handle), address);
    let mut client = connect(&handle, address);
    handle.send(b"recovered", false).unwrap();
    let mut bytes = [0; 9];
    client.read_exact(&mut bytes).unwrap();
    assert_eq!(&bytes, b"recovered");
}

#[test]
fn whole_broadcasts_are_fifo_for_multiple_clients_and_oversize_admission_is_explicit() {
    let handle = spawn(config(0)).unwrap();
    let address = listening(&handle);
    let mut first = connect(&handle, address);
    let mut second = connect(&handle, address);
    assert!(handle
        .send(&vec![0; COMMAND_BYTES], false)
        .unwrap_err()
        .contains("1 MiB"));
    let payload: Vec<_> = (0..200_000).map(|index| index as u8).collect();
    handle.send(&payload, false).unwrap();
    handle.send(b"tail", false).unwrap();
    let expected = [payload.as_slice(), b"tail"].concat();
    for client in [&mut first, &mut second] {
        let mut received = vec![0; expected.len()];
        client.read_exact(&mut received).unwrap();
        assert_eq!(received, expected);
    }
}

#[test]
fn saturated_input_replies_do_not_block_outgoing_native_commands_or_shutdown() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let mut handle = spawn_using(config(0), &pool, worker::bind).unwrap();
    let address = listening(&handle);
    let client = connect(&handle, address);
    let mut producer = client.try_clone().unwrap();
    producer.set_write_timeout(Some(Duration::from_secs(2))).unwrap();
    let sender = thread::spawn(move || {
        let _ = producer.write_all(&vec![4; 2 * 1024 * 1024]);
    });
    wait(|| handle.worker.event_metrics().depth > 0 && handle.worker.event_metrics().rejected > 0);
    let metrics = handle.worker.event_metrics();
    assert!(metrics.retained_weight <= EVENT_BYTES);
    handle.send(b"independent", false).unwrap();
    let mut client = client;
    let mut reply = [0; 11];
    client.read_exact(&mut reply).unwrap();
    assert_eq!(&reply, b"independent");
    let started = Instant::now();
    handle.stop();
    assert!(started.elapsed() < Duration::from_millis(100));
    wait(|| pool.metrics().active == 0);
    sender.join().unwrap();
}

#[test]
fn invalid_retained_host_capacity_is_rejected_before_native_lifetime_reservation() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let mut invalid = config(0);
    invalid.bind_host.reserve(1025);
    assert!(spawn_using(invalid, &pool, worker::bind).is_err());
    let mut invalid = config(0);
    invalid.input.parse.mode.reserve(17);
    assert!(spawn_using(invalid, &pool, worker::bind).is_err());
    assert_eq!(pool.metrics().active, 0);
}

#[test]
fn command_admission_enforces_item_and_retained_byte_limits_while_native_setup_is_blocked() {
    for (payload, accepted) in [(vec![7], 256), (vec![7; COMMAND_BYTES - size_of::<Command>()], 8)] {
        let pool = RetirementPool::new(NonZeroUsize::MIN);
        let (release_tx, release_rx) = mpsc::channel();
        let (started_tx, started_rx) = mpsc::channel();
        let mut handle = spawn_using(config(0), &pool, move |_| {
            started_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            Err("delayed setup".into())
        })
        .unwrap();
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        // Isolate queue admission from peer discovery while the native owner is gated.
        handle.clients.store(1, Ordering::Release);
        for _ in 0..accepted {
            handle.send(&payload, false).unwrap();
        }
        assert!(handle.send(&payload, false).unwrap_err().contains("capacity exhausted"));
        handle.stop();
        assert!(handle.send(&payload, false).is_err());
        release_tx.send(()).unwrap();
        drop(handle);
        wait(|| pool.metrics().active == 0);
    }
}

#[test]
fn client_limit_closes_overflow_and_recovers_capacity_after_a_disconnect() {
    let handle = spawn(config(0)).unwrap();
    let address = listening(&handle);
    let mut clients = Vec::new();
    for _ in 0..MAX_CLIENTS {
        clients.push(connect(&handle, address));
    }
    assert_eq!(handle.clients.load(Ordering::Acquire), MAX_CLIENTS);
    let mut overflow = TcpStream::connect(address).unwrap();
    overflow.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    assert_eq!(overflow.read(&mut [0; 1]).unwrap(), 0);
    assert_eq!(handle.clients.load(Ordering::Acquire), MAX_CLIENTS);
    drop(clients.pop());
    until(&handle, |event| {
        matches!(event, TcpServerWorkerEvent::ClientDisconnected { .. })
    });
    let _recovered = connect(&handle, address);
    assert_eq!(handle.clients.load(Ordering::Acquire), MAX_CLIENTS);
}

#[test]
fn graceful_native_admission_close_finishes_the_accepted_fifo_without_reaccepting_clients() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let mut handle = spawn_using(config(0), &pool, worker::bind).unwrap();
    let address = listening(&handle);
    let mut client = connect(&handle, address);
    let bytes = vec![5; 200_000];
    handle.send(&bytes, false).unwrap();
    handle.send(b"tail", false).unwrap();
    handle.worker.finish_admission();
    assert!(handle.send(b"late", false).is_err());
    let mut received = vec![0; bytes.len() + 4];
    client.read_exact(&mut received).unwrap();
    assert_eq!(received, [bytes.as_slice(), b"tail"].concat());
    wait(|| pool.metrics().active == 0);
    assert_eq!(client.read(&mut [0; 1]).unwrap(), 0);
}
