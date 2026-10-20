use super::super::super::{
    osc_outbound_work::Current,
    osc_runtime::{worker::OscSocket, OscWorkerCommand},
};
use super::*;
use std::{
    io,
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Mutex,
    },
    time::Duration,
};

#[derive(Default)]
struct Socket {
    resolved: Vec<String>,
    sent: Arc<Mutex<Vec<Vec<u8>>>>,
    gate: Option<(mpsc::Sender<()>, mpsc::Receiver<()>)>,
    partial_send: bool,
}
impl OscSocket for Socket {
    fn address(&self) -> SocketAddr {
        "127.0.0.1:8000".parse().unwrap()
    }
    fn resolve(&mut self, host: &str, port: u16) -> Result<SocketAddr, String> {
        self.resolved.push(host.into());
        if let Some((entered, gate)) = self.gate.take() {
            entered.send(()).unwrap();
            gate.recv_timeout(Duration::from_secs(3)).unwrap();
        }
        if host == "fail.example" {
            return Err("injected DNS failure".into());
        }
        Ok(([127, 0, 0, 1], port).into())
    }
    fn send(&mut self, bytes: &[u8], _: SocketAddr) -> io::Result<usize> {
        self.sent.lock().unwrap().push(bytes.to_vec());
        Ok(if self.partial_send {
            bytes.len() - 1
        } else {
            bytes.len()
        })
    }
    fn receive(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::ErrorKind::WouldBlock.into())
    }
    fn wait(&mut self, _: Duration) -> io::Result<()> {
        Ok(())
    }
}

fn run(request: OutboundRequest, socket: &mut Socket) -> Result<Vec<Result<Option<String>, String>>, String> {
    let mut work = Current::new(OscWorkerCommand::Captured(request))?;
    let mut outcomes = Vec::new();
    while !work.complete() {
        if let Some(outcome) = work.service(socket, || false)? {
            outcomes.push(outcome);
        }
    }
    Ok(outcomes)
}

#[test]
fn late_invalid_host_and_target_count_reject_the_whole_plan_before_dns_or_send() {
    for count_failure in [false, true] {
        let mut fixture = Fixture::new();
        fixture.output(fixture.outputs, "valid.example", 8000);
        if count_failure {
            for _ in 0..256 {
                fixture.output(fixture.outputs, "later.example", 8000);
            }
        } else {
            fixture.output(fixture.outputs, &"x".repeat(1025), 8000);
        }
        let mut socket = Socket::default();
        let error = run(fixture.request(), &mut socket).unwrap_err();
        assert!(
            error.contains(if count_failure { "256 targets" } else { "1024 bytes" }),
            "{error}"
        );
        assert!(socket.resolved.is_empty());
        assert!(socket.sent.lock().unwrap().is_empty());
    }
}

#[test]
fn argument_expansion_and_wire_overflow_fail_before_any_destination_effect() {
    let mut fixture = Fixture::new();
    fixture.output(fixture.outputs, "valid.example", 8000);
    let mut expanded = vec![ParamValue::Str("/expanded".into())];
    expanded.extend((0..257).map(|_| ParamValue::Color(0.1, 0.2, 0.3, 1.0)));
    for args in [
        expanded,
        vec![ParamValue::Str("/wire".into()), ParamValue::Str("x".repeat(70 * 1024))],
    ] {
        let mut socket = Socket::default();
        assert!(run(fixture.script(args).unwrap(), &mut socket).is_err());
        assert!(socket.resolved.is_empty());
        assert!(socket.sent.lock().unwrap().is_empty());
    }
}

#[test]
fn dns_failure_preserves_later_destinations_without_replaying_successes() {
    let mut fixture = Fixture::new();
    for host in ["first.example", "fail.example", "last.example"] {
        fixture.output(fixture.outputs, host, 8000);
    }
    let mut socket = Socket::default();
    let outcomes = run(fixture.request(), &mut socket).unwrap();
    assert!(matches!(&outcomes[0], Ok(Some(log)) if log.contains("first.example")));
    assert!(matches!(&outcomes[1], Err(error) if error.contains("2/3 failed")));
    assert!(matches!(&outcomes[2], Ok(Some(log)) if log.contains("last.example")));
    assert_eq!(socket.resolved, ["first.example", "fail.example", "last.example"]);
    assert_eq!(socket.sent.lock().unwrap().len(), 2);
}

#[test]
fn partial_native_send_is_unconfirmed_and_never_retried_or_logged_as_success() {
    let mut fixture = Fixture::new();
    fixture.output(fixture.outputs, "valid.example", 8000);
    let mut socket = Socket {
        partial_send: true,
        ..Default::default()
    };
    let outcomes = run(fixture.request(), &mut socket).unwrap();
    assert_eq!(outcomes.len(), 1);
    assert!(outcomes[0].as_ref().unwrap_err().contains("partial datagram"));
    assert_eq!(socket.sent.lock().unwrap().len(), 1);
}

#[test]
fn retired_producer_rejects_captured_data_before_resolution() {
    let mut fixture = Fixture::new();
    fixture.output(fixture.outputs, "valid.example", 8000);
    let request = fixture.request();
    drop(fixture);
    let mut socket = Socket::default();
    assert!(run(request, &mut socket).unwrap_err().contains("retired"));
    assert!(socket.resolved.is_empty());
}

#[test]
fn source_retirement_during_blocked_dns_prevents_the_native_effect() {
    blocked_resolution_fence(false);
}

#[test]
fn transport_cancellation_during_blocked_dns_prevents_the_native_effect() {
    blocked_resolution_fence(true);
}

fn blocked_resolution_fence(cancel_transport: bool) {
    let mut fixture = Fixture::new();
    fixture.output(fixture.outputs, "blocked.example", 8000);
    let work = Current::new(OscWorkerCommand::Captured(fixture.request())).unwrap();
    let (entered, waiting) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let socket = Socket {
        gate: Some((entered, gate)),
        ..Default::default()
    };
    let sent = socket.sent.clone();
    let cancelled = Arc::new(AtomicBool::new(false));
    let worker_cancelled = cancelled.clone();
    let worker = std::thread::spawn(move || {
        let (mut work, mut socket) = (work, socket);
        work.service(&mut socket, || worker_cancelled.load(Ordering::Acquire))
    });
    waiting.recv_timeout(Duration::from_secs(3)).unwrap();
    if cancel_transport {
        cancelled.store(true, Ordering::Release);
    } else {
        drop(fixture);
    }
    release.send(()).unwrap();
    let error = worker.join().unwrap().unwrap_err();
    assert!(error.contains("retired during destination resolution"), "{error}");
    assert!(sent.lock().unwrap().is_empty());
}
