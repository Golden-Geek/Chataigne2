use super::super::lifecycle::HttpTransports;
use super::super::transport::{HttpTransportHandle, HttpWorkerEvent};
use super::transport::{config, request};
use super::*;
use golden_io::RetirementPool;
use std::num::NonZeroUsize;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Instant;

fn named(path: &str) -> super::super::transport::HttpQueuedRequest {
    let mut next = request();
    next.payload.path = path.into();
    next
}

fn succeeded(next: &super::super::transport::HttpQueuedRequest) -> HttpWorkerEvent {
    HttpWorkerEvent::Response(test_response(&next.payload.path, None, Vec::new()))
}

fn settings(index: usize) -> super::super::transport::HttpTransportConfig {
    // Executors are injected: no request is sent to this placeholder address.
    let mut next = config("http://127.0.0.1/".into());
    next.retry_count = index as u32;
    next
}

fn success_factory(
    config: super::super::transport::HttpTransportConfig,
    pool: &RetirementPool,
    previous: Option<super::super::transport::GenerationFence>,
) -> Result<HttpTransportHandle, String> {
    HttpTransportHandle::spawn_using(config, pool, previous, |_, _, next, _, _| Some(succeeded(next)))
}

fn collect(manager: &mut HttpTransports, count: usize) -> Vec<HttpWorkerEvent> {
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut events = Vec::new();
    while events.len() < count && Instant::now() < deadline {
        let polled = manager.poll();
        assert!(polled.events.len() <= 4, "aggregate generation service budget");
        events.extend(polled.events);
        thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(events.len(), count);
    events
}

fn paths(events: &[HttpWorkerEvent]) -> Vec<&str> {
    events
        .iter()
        .map(|event| match event {
            HttpWorkerEvent::Response(response) => response.request.path.as_str(),
            HttpWorkerEvent::Failure(failure) => failure.request.path.as_str(),
        })
        .collect()
}

#[test]
fn http_full_lifetime_capacity_preserves_the_old_owner_and_recovers() {
    let pool = RetirementPool::new(NonZeroUsize::MIN);
    let mut manager = HttpTransports::with_lifetimes(pool.clone());
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    manager
        .configure_using(settings(0), move |config, pool, previous| {
            let mut first = true;
            HttpTransportHandle::spawn_using(config, pool, previous, move |_, _, next, _, _| {
                if first {
                    first = false;
                    entered_tx.send(()).unwrap();
                    let _ = release_rx.recv_timeout(Duration::from_secs(3));
                }
                Some(succeeded(next))
            })
        })
        .unwrap();
    manager.send(named("old first")).unwrap();
    manager.send(named("old second")).unwrap();
    entered_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    let mut expected = vec!["old first".to_string(), "old second".to_string()];
    for index in 0..257 {
        let path = format!("old queued {index}");
        match manager.send(named(&path)) {
            Ok(()) => expected.push(path),
            Err(error) => {
                assert!(error.contains("capacity"));
                break;
            }
        }
    }
    assert_eq!(expected.len(), 257, "one in flight plus a full 256-command mailbox");
    assert!(manager
        .configure_using(settings(1), success_factory)
        .unwrap_err()
        .contains("capacity"));
    assert!(manager
        .send(named("not admitted"))
        .unwrap_err()
        .contains("not admitted"));
    release_tx.send(()).unwrap();
    assert_eq!(paths(&collect(&mut manager, expected.len())), expected);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    assert!(manager.poll().events.is_empty());
    manager.configure_using(settings(1), success_factory).unwrap();
    manager.send(named("recovered")).unwrap();
    assert_eq!(paths(&collect(&mut manager, 1)), ["recovered"]);
    manager.pause_admission();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    assert_eq!(pool.metrics().peak, 1);
}

#[test]
fn http_repeated_reconfiguration_has_a_fixed_owner_limit_and_coalesces_identical_settings() {
    let pool = RetirementPool::new(NonZeroUsize::new(8).unwrap());
    let mut manager = HttpTransports::with_lifetimes(pool.clone());
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let executed = Arc::new(Mutex::new(Vec::<String>::new()));
    let old_executed = executed.clone();
    manager
        .configure_using(settings(0), move |config, pool, previous| {
            HttpTransportHandle::spawn_using(config, pool, previous, move |_, _, next, _, _| {
                old_executed.lock().unwrap().push(next.payload.path.clone());
                entered_tx.send(()).unwrap();
                let _ = release_rx.recv_timeout(Duration::from_secs(3));
                Some(succeeded(next))
            })
        })
        .unwrap();
    manager.send(named("first")).unwrap();
    entered_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    for index in 1..=2 {
        let executed = executed.clone();
        manager
            .configure_using(settings(index), move |config, pool, previous| {
                HttpTransportHandle::spawn_using(config, pool, previous, move |_, _, next, _, _| {
                    executed.lock().unwrap().push(next.payload.path.clone());
                    Some(succeeded(next))
                })
            })
            .unwrap();
        for _ in 0..20 {
            manager.block_admission();
            manager
                .configure_using(settings(index), |_, _, _| {
                    panic!("identical settings must reuse admission")
                })
                .unwrap();
        }
        manager.send(named(&format!("generation {index}"))).unwrap();
    }
    for _ in 0..20 {
        assert!(manager
            .configure_using(settings(3), success_factory)
            .unwrap_err()
            .contains("older admitted requests"));
        assert!(manager.send(named("rejected")).is_err());
    }
    assert_eq!(*executed.lock().unwrap(), ["first"]);
    release_tx.send(()).unwrap();
    assert_eq!(
        paths(&collect(&mut manager, 3)),
        ["first", "generation 1", "generation 2"]
    );
    assert_eq!(*executed.lock().unwrap(), ["first", "generation 1", "generation 2"]);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    manager.poll();
    manager.configure_using(settings(3), success_factory).unwrap();
    manager.send(named("after drain")).unwrap();
    assert_eq!(paths(&collect(&mut manager, 1)), ["after drain"]);
    manager.pause_admission();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    assert!(
        pool.metrics().peak <= 3,
        "consumer ownership, not available pool capacity, bounds churn"
    );
}

#[test]
fn http_worker_failure_resolves_queued_requests_without_sending_them_and_then_recovers() {
    let pool = RetirementPool::new(NonZeroUsize::new(2).unwrap());
    let mut manager = HttpTransports::with_lifetimes(pool.clone());
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    manager
        .configure_using(settings(0), move |config, pool, previous| {
            HttpTransportHandle::spawn_using(config, pool, previous, move |_, _, _, _, progress| {
                progress.attempts = 1;
                entered_tx.send(()).unwrap();
                let _ = release_rx.recv_timeout(Duration::from_secs(3));
                panic!("injected request worker failure");
            })
        })
        .unwrap();
    manager.send(named("started")).unwrap();
    entered_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    manager.send(named("queued")).unwrap();
    release_tx.send(()).unwrap();
    let events = collect(&mut manager, 2);
    assert_eq!(paths(&events), ["started", "queued"]);
    assert!(
        matches!(&events[0], HttpWorkerEvent::Failure(failure) if failure.attempts == 1 && failure.error.contains("may have reached"))
    );
    assert!(
        matches!(&events[1], HttpWorkerEvent::Failure(failure) if failure.attempts == 0 && failure.error.contains("not sent"))
    );
    assert!(manager.send(named("rejected after failure")).is_err());
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
    manager.poll();
    manager.configure_using(settings(1), success_factory).unwrap();
    manager.send(named("recovered")).unwrap();
    assert_eq!(paths(&collect(&mut manager, 1)), ["recovered"]);
    manager.pause_admission();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn stopping_a_gated_generation_wakes_it_without_issuing_its_requests() {
    let pool = RetirementPool::new(NonZeroUsize::new(2).unwrap());
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let mut old = HttpTransportHandle::spawn_using(settings(0), &pool, None, move |_, _, next, _, _| {
        entered_tx.send(()).unwrap();
        let _ = release_rx.recv_timeout(Duration::from_secs(3));
        Some(succeeded(next))
    })
    .unwrap();
    old.send(named("old")).unwrap();
    entered_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    let mut next =
        HttpTransportHandle::spawn_using(settings(1), &pool, Some(old.completion_fence()), |_, _, _, _, _| {
            panic!("gated request must never execute")
        })
        .unwrap();
    next.send(named("cancelled destination")).unwrap();
    next.stop();
    drop(next);
    let deadline = Instant::now() + Duration::from_secs(2);
    while pool.metrics().active != 1 && Instant::now() < deadline {
        thread::yield_now();
    }
    assert_eq!(pool.metrics().active, 1, "stop must wake the predecessor wait");
    old.finish_admission();
    release_tx.send(()).unwrap();
    let mut events = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(2);
    while events.is_empty() && Instant::now() < deadline {
        old.drain_events(&mut events, NonZeroUsize::MIN);
        thread::yield_now();
    }
    assert_eq!(paths(&events), ["old"]);
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn http_native_retirement_cannot_block_resolved_results_or_new_generations() {
    let pool = RetirementPool::new(NonZeroUsize::new(2).unwrap());
    let mut manager = HttpTransports::with_lifetimes(pool.clone());
    let (retiring_tx, retiring_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    manager
        .configure_using(settings(0), move |config, pool, previous| {
            HttpTransportHandle::spawn_with_retirement_hook(
                config,
                pool,
                previous,
                |_, _, next, _, _| Some(succeeded(next)),
                move || {
                    retiring_tx.send(()).unwrap();
                    let _ = release_rx.recv_timeout(Duration::from_secs(3));
                },
            )
        })
        .unwrap();
    manager.send(named("old")).unwrap();
    manager.configure_using(settings(1), success_factory).unwrap();
    manager.send(named("new")).unwrap();
    retiring_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    let events = collect(&mut manager, 2);
    assert_eq!(
        paths(&events),
        ["old", "new"],
        "cleanup must not hold completion ordering hostage"
    );
    assert!(
        pool.metrics().active >= 1,
        "the native resource still owns its lifetime slot"
    );
    release_tx.send(()).unwrap();
    manager.pause_admission();
    assert!(pool.wait_for_idle(Duration::from_secs(2)));
}
