use super::super::super::transport::{
    HttpOutcome, HttpResponse, HttpTransportConfig, HttpTransportHandle, HttpWorkerEvent,
};
use super::*;
use crate::app::module::common::http::HttpAuthConfig;
use golden_io::{RetirementPool, WorkerNotifier};
use std::{
    num::NonZeroUsize,
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

fn config() -> HttpTransportConfig {
    HttpTransportConfig {
        base_address: "http://127.0.0.1/".into(),
        default_headers: vec![],
        auth: HttpAuthConfig::default(),
        timeout: Duration::from_secs(1),
        retry_count: 0,
        retry_delay: Duration::ZERO,
    }
}
fn response(request: &HttpQueuedRequest) -> HttpOutcome {
    HttpOutcome::Response(HttpResponse {
        origin: request.origin.clone(),
        request: request.payload.clone(),
        url: "http://127.0.0.1/".into(),
        status: 200,
        status_text: "OK".into(),
        headers: vec![],
        content_type: None,
        body: vec![],
        received_values: Ok(None),
        elapsed_ms: 0,
        attempts: 1,
    })
}

#[test]
fn failed_conversion_is_an_ordered_rejection_and_does_not_disconnect_or_block_later_requests() {
    let calls = Arc::new(AtomicUsize::new(0));
    let native_calls = calls.clone();
    let actor = thread::current().id();
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let worker = HttpTransportHandle::spawn_using(
        config(),
        &pool,
        None,
        WorkerNotifier::new(|| {}),
        move |_, _, request, _, progress| {
            assert_ne!(thread::current().id(), actor);
            native_calls.fetch_add(1, Ordering::Relaxed);
            progress.attempts = 1;
            Some(response(request))
        },
    )
    .unwrap();
    worker
        .send_input(script("request", vec![ParamValue::Str("invalid".into())]))
        .unwrap();
    let next = script("get", vec![ParamValue::Str("/after".into())]);
    worker.send_input(next.clone()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut events = vec![];
    while events.len() < 2 {
        worker.drain_events(&mut events, NonZeroUsize::new(4).unwrap());
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
    assert!(matches!(&events[0], HttpWorkerEvent::Rejected(event) if event.attempts == 0 && event.error.len() <= 4096));
    assert!(matches!(&events[1], HttpWorkerEvent::Response(event) if event.request.path == "/after"));
    worker.send_input(next).unwrap();
    while calls.load(Ordering::Relaxed) < 2 {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(calls.load(Ordering::Relaxed), 2);
}

#[test]
fn full_native_queue_rejects_shallow_inputs_while_an_accepted_prefix_is_blocked() {
    let (started_tx, started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let mut first = true;
    let worker = HttpTransportHandle::spawn_using(
        config(),
        &pool,
        None,
        WorkerNotifier::new(|| {}),
        move |_, _, request, _, progress| {
            if first {
                first = false;
                started_tx.send(()).unwrap();
                release_rx.recv().unwrap();
            }
            progress.attempts = 1;
            Some(response(request))
        },
    )
    .unwrap();
    let input = script(
        "post",
        vec![
            ParamValue::Str("/blocked".into()),
            ParamValue::Str("x".repeat(128 * 1024)),
        ],
    );
    worker.send_input(input.clone()).unwrap();
    started_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    let mut admitted = 0;
    for _ in 0..257 {
        match worker.send_input(input.clone()) {
            Ok(()) => admitted += 1,
            Err(error) => {
                assert!(error.contains("capacity"));
                break;
            }
        }
    }
    assert!((1..256).contains(&admitted));
    drop(worker);
    release_tx.send(()).unwrap();
}
