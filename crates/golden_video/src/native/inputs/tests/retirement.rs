use std::sync::mpsc;
use std::time::Duration;

use windows::core::Error;

use super::*;

const WAIT: Duration = Duration::from_secs(2);
const QUERY_ERROR: i32 = 0x8000_4005_u32 as i32;

fn surface(descriptor: TextureDescriptor) -> Arc<Surface> {
    // An owned kernel handle stands in for the source allocation; no GPU work is submitted.
    let handle = unsafe { CreateEventW(None, true, false, None) }.unwrap();
    Arc::new(Surface {
        handle: unsafe { OwnedHandle::from_raw_handle(handle.0) },
        index: 0,
        descriptor,
    })
}

#[test]
fn withdrawal_precedes_a_blocked_failing_drain_without_certifying_retirement() {
    let (shared, descriptor) = fixture();
    shared.update(|status| {
        status.observed_descriptor = Some(descriptor);
        status.generation = 1;
        status.pool_generation = 1;
        status.bridge_slots = 3;
    });
    let _demand = shared.acquire_demand();
    let source_owner = surface(descriptor);
    let renderer_lease = source_owner.clone();
    let ownership = Arc::downgrade(&source_owner);
    assert!(shared.publish(Publication {
        surface: source_owner.clone(),
        sequence: 1,
        timing: None,
    }));
    let reader = shared.observation_revision.reader();
    let guard = reader.guard_at(reader.current());
    let (attempt, attempts) = mpsc::channel();
    let (resume, resumed) = mpsc::channel();
    let (finished, retirement) = mpsc::channel();
    let worker_shared = shared.clone();
    let worker = std::thread::spawn(move || {
        worker::retire_source(
            &worker_shared,
            || false,
            || {
                attempt.send(()).unwrap();
                if resumed.recv_timeout(WAIT).unwrap() {
                    Ok(true)
                } else {
                    Err(Error::from_hresult(HRESULT(QUERY_ERROR)))
                }
            },
        );
        drop(source_owner);
        finished.send(()).unwrap();
    });

    attempts.recv_timeout(WAIT).unwrap();
    assert!(!guard.is_current());
    assert!(shared.latest.lock().unwrap().is_none());
    assert!(shared.retiring.load(Ordering::Acquire));
    assert!(!shared.released.load(Ordering::Acquire));
    assert_eq!(shared.status.lock().unwrap().state, SourceState::Recovering);
    assert_eq!(ownership.strong_count(), 2, "source and renderer owners remain held");

    resume.send(false).unwrap();
    attempts.recv_timeout(WAIT).unwrap();
    assert_eq!(shared.status.lock().unwrap().hresult, Some(QUERY_ERROR));
    assert_eq!(shared.status.lock().unwrap().bridge_slots, 3);
    shared.quiesce();
    shared.shutdown.store(true, Ordering::Release);
    shared.released.store(true, Ordering::Release);
    assert!(matches!(retirement.try_recv(), Err(mpsc::TryRecvError::Empty)));
    assert!(!worker.is_finished());
    assert_eq!(
        ownership.strong_count(),
        2,
        "cancel and renderer release do not drain source work"
    );
    assert_eq!(shared.status.lock().unwrap().state, SourceState::Recovering);

    resume.send(true).unwrap();
    retirement.recv_timeout(WAIT).unwrap();
    worker.join().unwrap();
    assert_eq!(ownership.strong_count(), 1);
    assert_eq!(shared.status.lock().unwrap().state, SourceState::Recovering);
    drop(renderer_lease);
    assert!(ownership.upgrade().is_none());
}

#[test]
fn repeated_withdrawal_preserves_renderer_acknowledgment_and_rejects_late_publication() {
    let (shared, descriptor) = fixture();
    let _demand = shared.acquire_demand();
    let source = surface(descriptor);
    assert!(shared.publish(Publication {
        surface: source.clone(),
        sequence: 1,
        timing: None,
    }));
    let reader = shared.observation_revision.reader();
    let before = reader.guard_at(reader.current());
    shared.released.store(true, Ordering::Release);
    shared.withdraw();
    assert!(!before.is_current());
    assert!(!shared.released.load(Ordering::Acquire));
    let withdrawn = reader.guard_at(reader.current());
    shared.released.store(true, Ordering::Release);
    shared.withdraw();
    assert!(
        withdrawn.is_current(),
        "the same withdrawal is one observation boundary"
    );
    assert!(shared.released.load(Ordering::Acquire));
    assert!(!shared.publish(Publication {
        surface: source,
        sequence: 2,
        timing: None,
    }));
    assert!(shared.latest.lock().unwrap().is_none());
    assert_eq!(shared.status.lock().unwrap().publications, 1);
    assert_eq!(shared.status.lock().unwrap().state, SourceState::Recovering);
}
