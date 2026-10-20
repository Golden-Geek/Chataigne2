use super::super::{GenerationFence, HttpQueuedRequest, HttpTransportConfig};
use super::*;
use crate::app::module::common::http::{HttpAuthConfig, HttpMethod, HttpRequestBody, HttpRequestPayload};
use golden_io::{BoundedWorker, PendingDrainState, RetirementPool, WorkerLimits};
use std::{
    num::NonZeroUsize,
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc, Arc,
    },
    thread,
    time::{Duration, Instant},
};

fn request(path: &str) -> HttpQueuedRequest {
    HttpQueuedRequest {
        origin: HttpRequestOrigin::Script { method: "get".into() },
        payload: HttpRequestPayload {
            method: HttpMethod::Get,
            path: path.into(),
            query: String::new(),
            value_path: String::new(),
            headers: vec![],
            body: HttpRequestBody::Empty,
            description: String::new(),
        },
    }
}

fn response(path: &str, body: Vec<u8>) -> HttpOutcome {
    let request = request(path);
    HttpOutcome::Response(HttpResponse {
        origin: request.origin,
        request: request.payload,
        url: "http://127.0.0.1/".into(),
        status: 200,
        status_text: "OK".into(),
        headers: vec![],
        content_type: None,
        body,
        received_values: Ok(None),
        elapsed_ms: 1,
        attempts: 1,
    })
}

fn pool(items: usize, bytes: usize) -> PayloadPool {
    PayloadPool::new("http-results-test", items, bytes).unwrap()
}

#[test]
fn alternate_callback_admission_failure_cannot_publish_a_partial_response() {
    let pool = pool(1, 1024 * 1024);
    let mut calls = 0;
    let event = prepare_in(response("/alternate", b"body".to_vec()), &pool, |name, args| {
        calls += 1;
        if calls == 2 {
            Err("alternate callback capacity exhausted".into())
        } else {
            NativeCallbackPayload::prepare(name, args)
        }
    });
    assert_eq!(calls, 2);
    assert!(
        matches!(event, HttpWorkerEvent::Rejected(rejection) if rejection.error.contains("alternate callback") && rejection.attempts == 1)
    );
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn response_and_graph_failure_callbacks_are_prepared_before_owner_binding() {
    let pool = pool(1, 1024 * 1024);
    let HttpWorkerEvent::Response(response) = prepare_in(
        response("/value", b"body".to_vec()),
        &pool,
        NativeCallbackPayload::prepare,
    ) else {
        panic!("response")
    };
    let first = response.response_callback(NodeId(42));
    let second = response.response_callback(NodeId(42));
    assert!(Arc::ptr_eq(&first.payload, &second.payload));
    assert_eq!(first.origin, Some(NodeId(42)));
    assert_eq!(first.payload["callback"], "responseReceived");
    assert_eq!(first.payload["args"][1], "body");
    let failure = response.application_failure_callback(NodeId(42));
    assert_eq!(failure.payload["callback"], "requestFailed");
    assert_eq!(failure.payload["args"][1]["path"], "/value");
    assert_eq!(failure.payload["args"][1]["attempts"], 1);
    assert_eq!(
        failure.payload["args"][0],
        super::super::super::callbacks::APPLICATION_FAILURE
    );
}

#[test]
fn retained_response_clones_hold_capacity_until_owner_cleanup_and_callbacks_survive() {
    let pool = pool(1, 1024 * 1024);
    let HttpWorkerEvent::Response(response) = prepare_in(
        response("/value", b"body".to_vec()),
        &pool,
        NativeCallbackPayload::prepare,
    ) else {
        panic!("response")
    };
    let copy = response.clone();
    let callback = response.response_callback(NodeId(1));
    drop(response);
    assert!(pool.try_retain(1u8, 1).is_err());
    drop(copy);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    assert_eq!(callback.payload["args"][1], "body");
}

#[test]
fn result_byte_admission_counts_spare_body_and_parse_error_capacity() {
    let pool = pool(2, 32 * 1024);
    let mut body = Vec::with_capacity(128 * 1024);
    body.extend_from_slice(b"ok");
    let event = prepare_in(response("/spare", body), &pool, NativeCallbackPayload::prepare);
    assert!(
        matches!(event, HttpWorkerEvent::Rejected(rejection) if rejection.error.contains("lifetime capacity") && rejection.attempts == 1)
    );
    let HttpOutcome::Response(mut data) = response("/parse", vec![]) else {
        unreachable!()
    };
    let mut error = String::with_capacity(128 * 1024);
    error.push('x');
    data.received_values = Err(error);
    let event = prepare_in(HttpOutcome::Response(data), &pool, NativeCallbackPayload::prepare);
    assert!(matches!(event, HttpWorkerEvent::Rejected(rejection) if rejection.error.contains("lifetime capacity")));
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn oversized_callback_complexity_becomes_one_prepared_request_failure() {
    let pool = pool(1, 1024 * 1024);
    let event = prepare_in(
        response("/large", vec![0xff; 32 * 1024]),
        &pool,
        NativeCallbackPayload::prepare,
    );
    let HttpWorkerEvent::Failure(failure) = event else {
        panic!("bounded failure")
    };
    assert_eq!(failure.attempts, 1);
    assert!(failure.error.contains("JSON complexity"));
    let callback = failure.callback(NodeId(1));
    assert_eq!(callback.payload["callback"], "requestFailed");
    assert_eq!(callback.payload["args"][1]["path"], "/large");
}

#[test]
fn callback_admission_failure_returns_bounded_identity_and_attempts_without_data_transfer() {
    let pool = pool(1, 1024 * 1024);
    let HttpOutcome::Response(mut response) = response(&"é".repeat(10_000), vec![]) else {
        unreachable!()
    };
    response.origin = HttpRequestOrigin::Command {
        command_id: NodeId(42),
        command_type: "request".into(),
    };
    response.attempts = 7;
    let event = prepare_in(HttpOutcome::Response(response), &pool, |_, _| Err("é".repeat(10_000)));
    assert!(event.retained_weight() < 8 * 1024);
    let HttpWorkerEvent::Rejected(rejection) = event else {
        panic!("rejected callback")
    };
    assert_eq!(rejection.command_id, Some(NodeId(42)));
    assert_eq!(rejection.attempts, 7);
    assert!(rejection.description.chars().count() <= 260);
    assert_eq!(rejection.error.len(), 4096);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn native_result_rejection_closes_admission_and_resolves_the_queued_suffix_without_sends() {
    let lifetimes = RetirementPool::new(NonZeroUsize::MIN);
    let result_pool = pool(1, 1);
    let calls = Arc::new(AtomicUsize::new(0));
    let native_calls = calls.clone();
    let completion = GenerationFence::default();
    let native_completion = completion.clone();
    let (release_tx, release_rx) = mpsc::channel();
    let worker = BoundedWorker::<super::super::HttpInput, HttpWorkerEvent>::spawn(
        "http-rejection",
        &lifetimes,
        WorkerLimits {
            command_items: 4,
            command_weight: 4096,
            event_items: 1,
            event_weight: HTTP_EVENT_BYTES,
        },
        move |context| {
            release_rx.recv().unwrap();
            let config = HttpTransportConfig {
                base_address: "http://127.0.0.1/".into(),
                default_headers: vec![],
                auth: HttpAuthConfig::default(),
                timeout: Duration::from_millis(100),
                retry_count: 0,
                retry_delay: Duration::ZERO,
            };
            let mut guard = native_completion.guard(golden_io::WorkerNotifier::new(|| {}));
            super::super::worker_loop(
                config,
                context,
                true,
                &mut guard,
                Box::new(move |_, _, request, _, progress| {
                    native_calls.fetch_add(1, Ordering::Relaxed);
                    progress.attempts = 1;
                    Some(response(&request.payload.path, b"ok".to_vec()))
                }),
                None,
                move |outcome| prepare_in(outcome, &result_pool, NativeCallbackPayload::prepare),
            );
        },
    )
    .unwrap();
    for path in ["/first", "/second", "/third"] {
        worker
            .send_weighted(super::super::HttpInput::prepare(request(path)).unwrap(), 1024)
            .unwrap();
    }
    release_tx.send(()).unwrap();
    let mut events = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if worker.drain_events(&mut events, NonZeroUsize::MIN).state == PendingDrainState::Disconnected {
            break;
        }
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    assert!(worker
        .send_weighted(super::super::HttpInput::prepare(request("/late")).unwrap(), 1024)
        .is_err());
    let results: Vec<_> = events
        .iter()
        .map(|event| match event {
            HttpWorkerEvent::Rejected(rejection) => (rejection.description.as_str(), rejection.attempts),
            _ => panic!("expected explicit rejection"),
        })
        .collect();
    assert_eq!(results, [("GET /first", 1), ("GET /second", 0), ("GET /third", 0)]);
    assert!(completion.is_drained());
    assert!(lifetimes.wait_for_idle(Duration::from_secs(2)));
}
