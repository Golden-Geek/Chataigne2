use super::*;
use golden_engine::app::{ProjectLifecycle, create_new_project_engine};
use std::sync::atomic::{AtomicBool, Ordering};

golden_engine::define_node_enum!(
    enum StopNode {}
);
impl ProjectLifecycle for StopNode {}

struct Service(Arc<AtomicBool>);
impl Drop for Service {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

#[test]
fn shared_stop_wakes_idle_listeners_and_releases_every_host_service() {
    let shutdown = UiServerShutdown::default();
    let mut completions = Vec::new();
    for _ in 0..2 {
        let engine = create_new_project_engine::<StopNode>().unwrap();
        let shutdown = shutdown.clone();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let (done_tx, done_rx) = mpsc::sync_channel(1);
        let dropped = Arc::new(AtomicBool::new(false));
        let service = Service(dropped.clone());
        thread::spawn(move || {
            let result = crate::run_with_ui_server_config_and_ready(
                engine,
                UiServerConfig {
                    bind_addr: "127.0.0.1:0".into(),
                    shutdown: Some(shutdown),
                    ..Default::default()
                },
                |_| {
                    ready_tx.send(()).unwrap();
                    Ok(service)
                },
            );
            done_tx.send(result).unwrap();
        });
        ready_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        completions.push((done_rx, dropped));
    }
    shutdown.request();
    for (done, dropped) in completions {
        done.recv_timeout(Duration::from_secs(2)).unwrap().unwrap();
        assert!(dropped.load(Ordering::Acquire));
    }
}

#[test]
fn stop_closes_a_blocked_http_connection_and_releases_the_listening_port() {
    let reservation = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = reservation.local_addr().unwrap();
    drop(reservation);
    let shutdown = UiServerShutdown::default();
    let stop = shutdown.clone();
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    let (done_tx, done_rx) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let result = crate::run_with_ui_server_config_and_ready(
            create_new_project_engine::<StopNode>().unwrap(),
            UiServerConfig {
                bind_addr: address.to_string(),
                shutdown: Some(stop),
                ..Default::default()
            },
            |_| {
                ready_tx.send(()).unwrap();
                Ok(())
            },
        );
        done_tx.send(result).unwrap();
    });
    ready_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    let mut client = TcpStream::connect(address).unwrap();
    client
        .write_all(b"GET /api/ui/health HTTP/1.1\r\nHost: localhost\r\n")
        .unwrap();
    client.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
    shutdown.request();
    done_rx.recv_timeout(Duration::from_secs(2)).unwrap().unwrap();
    let mut byte = [0];
    assert!(match client.read(&mut byte) {
        Ok(0) => true,
        Err(error) => matches!(error.kind(), ErrorKind::ConnectionReset | ErrorKind::ConnectionAborted),
        _ => false,
    });
    TcpListener::bind(address).expect("stopped server must release the listener");
}

#[test]
fn already_requested_stop_never_starts_host_services() {
    let shutdown = UiServerShutdown::default();
    shutdown.request();
    let called = AtomicBool::new(false);
    crate::run_with_ui_server_config_and_ready(
        create_new_project_engine::<StopNode>().unwrap(),
        UiServerConfig {
            shutdown: Some(shutdown),
            ..Default::default()
        },
        |_| {
            called.store(true, Ordering::Release);
            Ok(())
        },
    )
    .unwrap();
    assert!(!called.load(Ordering::Acquire));
}

#[test]
fn a_blocked_worker_reports_timeout_instead_of_clean_shutdown_and_can_finish_later() {
    let mut workers = ServerWorkers::new(UiServerShutdown::default());
    let (release, held) = mpsc::channel();
    workers
        .spawn("held-test-worker", move || {
            held.recv().unwrap();
        })
        .unwrap();
    let error = workers.finish(Duration::from_millis(10)).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::TimedOut);
    release.send(()).unwrap();
    workers.finish(Duration::from_secs(1)).unwrap();
}

#[test]
fn a_long_runtime_deadline_does_not_delay_cancellation() {
    let mut workers = ServerWorkers::new(UiServerShutdown::default());
    let stop = workers.shutdown.clone();
    let (entered, waiting) = mpsc::sync_channel(1);
    workers
        .spawn("long-deadline-test", move || {
            let mut pacer = RuntimeLoopPacer::new();
            entered.send(()).unwrap();
            pacer.wait_for_next_tick(Instant::now(), Duration::from_secs(60), &stop);
        })
        .unwrap();
    waiting.recv_timeout(Duration::from_secs(1)).unwrap();
    thread::sleep(Duration::from_millis(20));
    workers.finish(Duration::from_millis(500)).unwrap();
}
