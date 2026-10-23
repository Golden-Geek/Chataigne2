use super::super::tests::fixture::frame;
use super::*;
use std::{
    collections::VecDeque,
    sync::{atomic::AtomicUsize, mpsc},
    thread,
    time::Instant,
};
#[derive(Default)]
struct Control {
    rows: Mutex<VecDeque<KinectRuntimePoll>>,
    polls: AtomicUsize,
    panic: AtomicBool,
    gate: Mutex<Option<mpsc::Receiver<()>>>,
    cleanup: Mutex<Option<mpsc::Receiver<()>>>,
}
struct Fake(Arc<Control>);
impl Provider for Fake {
    fn poll(&mut self) -> Result<Vec<KinectRuntimePoll>, String> {
        self.0.polls.fetch_add(1, Ordering::AcqRel);
        if let Some(rx) = self.0.gate.lock().unwrap().take() {
            rx.recv().unwrap();
        }
        assert!(!self.0.panic.swap(false, Ordering::AcqRel), "controlled Kinect panic");
        let mut rows = self.0.rows.lock().unwrap();
        let count = rows.len().min(16);
        Ok(rows.drain(..count).collect())
    }
}
impl Drop for Fake {
    fn drop(&mut self) {
        if let Some(rx) = self.0.cleanup.lock().unwrap().take() {
            let _ = rx.recv();
        }
    }
}
fn slots() -> RetirementPool {
    RetirementPool::new(NonZeroUsize::MIN)
}
fn pool(items: usize) -> PayloadPool {
    PayloadPool::new("kinect-runtime-test", items, items * EVENT_WEIGHT).unwrap()
}
fn until(mut predicate: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(4);
    while !predicate() {
        assert!(Instant::now() < end, "Kinect runtime fixture deadline");
        thread::sleep(Duration::from_millis(1));
    }
}
fn spawn(control: Arc<Control>, pool: PayloadPool) -> (Kinect2Runtime, RetirementPool) {
    let owner = slots();
    let native = slots();
    let runtime = Kinect2Runtime::spawn_using(
        &owner,
        &native,
        pool,
        WorkerNotifier::new(|| {}),
        KinectReferenceSpace::Absolute,
        move || Ok(Fake(control)),
    )
    .unwrap();
    (runtime, native)
}
#[test]
fn native_sensor_cannot_open_in_test_builds() {
    assert!(native::create().is_err());
}
#[test]
fn blocked_setup_retains_both_lifetime_slots_without_blocking_actor_retirement() {
    let owner = slots();
    let native = slots();
    let (release, rx) = mpsc::channel();
    let (entered, ready) = mpsc::channel();
    let control = Arc::new(Control::default());
    let before = Instant::now();
    let runtime = Kinect2Runtime::spawn_using(
        &owner,
        &native,
        pool(8),
        WorkerNotifier::new(|| {}),
        KinectReferenceSpace::Absolute,
        move || {
            entered.send(()).unwrap();
            rx.recv().unwrap();
            Ok(Fake(control))
        },
    )
    .unwrap();
    assert!(before.elapsed() < Duration::from_millis(250));
    ready.recv_timeout(Duration::from_secs(1)).unwrap();
    drop(runtime);
    assert_eq!(owner.metrics().active, 1);
    assert_eq!(native.metrics().active, 1);
    release.send(()).unwrap();
    assert!(native.wait_for_idle(Duration::from_secs(2)));
    assert!(owner.wait_for_idle(Duration::from_secs(2)));
}
#[test]
fn blocked_poll_does_not_hold_actor_retirement_or_release_native_capacity_early() {
    let control = Arc::new(Control::default());
    let (release, rx) = mpsc::channel();
    *control.gate.lock().unwrap() = Some(rx);
    let (runtime, native) = spawn(control.clone(), pool(8));
    until(|| control.polls.load(Ordering::Acquire) > 0);
    let before = Instant::now();
    drop(runtime);
    assert!(before.elapsed() < Duration::from_millis(250));
    assert_eq!(native.metrics().active, 1);
    release.send(()).unwrap();
    assert!(native.wait_for_idle(Duration::from_secs(2)));
}
#[test]
fn full_mailbox_retains_64_ordered_tracking_boundaries_in_bounded_actor_turns() {
    let control = Arc::new(Control::default());
    control.rows.lock().unwrap().extend((0..64).map(frame));
    let (mut runtime, native) = spawn(control.clone(), pool(96));
    until(|| runtime.queued() == 32 && runtime.pressure());
    let polls = control.polls.load(Ordering::Acquire);
    thread::sleep(Duration::from_millis(25));
    assert_eq!(control.polls.load(Ordering::Acquire), polls);
    let mut count = 0;
    until(|| {
        let rows = runtime.poll().unwrap();
        assert!(rows.len() <= 16);
        for row in rows {
            assert_eq!(row.prepared.tracking_id, (count + 1).to_string());
            count += 1;
        }
        count == 64
    });
    drop(runtime);
    assert!(native.wait_for_idle(Duration::from_secs(2)));
}
#[test]
fn exported_publication_keeps_original_quota_after_native_retirement() {
    let control = Arc::new(Control::default());
    control.rows.lock().unwrap().push_back(frame(0));
    let sources = pool(1);
    let (mut runtime, native) = spawn(control, sources.clone());
    until(|| runtime.has_pending());
    let row = runtime.poll().unwrap().pop().unwrap();
    let exported = row.clone();
    drop((runtime, row));
    assert!(native.wait_for_idle(Duration::from_secs(2)));
    assert!(!exported.current());
    assert!(sources.try_reserve(1).is_err());
    drop(exported);
    assert!(sources.wait_for_idle(Duration::from_secs(2)));
}
#[test]
fn eight_reference_revisions_reprepare_held_raw_frame_without_reopening_native() {
    let control = Arc::new(Control::default());
    control.rows.lock().unwrap().push_back(frame(0));
    let (mut runtime, native) = spawn(control, pool(32));
    until(|| runtime.has_pending());
    let mut rows = runtime.poll().unwrap();
    for i in 0..8 {
        let space = if i % 2 == 0 {
            KinectReferenceSpace::Torso
        } else {
            KinectReferenceSpace::Absolute
        };
        runtime.configure(space);
        assert!(rows.iter().all(|r| !r.current()));
        until(|| runtime.has_pending());
        rows = runtime.poll().unwrap();
        assert_eq!(rows[0].prepared.space, space);
        assert_eq!(rows[0].prepared.speed, KinectVec3::ZERO);
        assert!(!rows[0].prepared.incoming);
        assert_eq!(native.metrics().active, 1);
    }
    drop((rows, runtime));
    assert!(native.wait_for_idle(Duration::from_secs(2)));
}
#[test]
fn source_capacity_pauses_polling_and_resumes_the_same_pending_frame() {
    let control = Arc::new(Control::default());
    control.rows.lock().unwrap().extend([frame(0), frame(1)]);
    let sources = pool(1);
    let (mut runtime, native) = spawn(control.clone(), sources.clone());
    until(|| runtime.queued() == 1 && runtime.pressure());
    let first = runtime.poll().unwrap();
    assert_eq!(first[0].prepared.tracking_id, "1");
    thread::sleep(Duration::from_millis(25));
    assert!(!runtime.has_pending());
    drop(first);
    until(|| runtime.has_pending());
    let last = runtime.poll().unwrap();
    assert_eq!(last[0].prepared.tracking_id, "2");
    drop((last, runtime));
    assert!(native.wait_for_idle(Duration::from_secs(2)));
    assert!(sources.wait_for_idle(Duration::from_secs(2)));
}
#[test]
fn stopped_worker_invalidates_and_notifies_before_blocked_provider_cleanup() {
    let control = Arc::new(Control::default());
    control.rows.lock().unwrap().push_back(frame(0));
    let (release, rx) = mpsc::channel();
    *control.cleanup.lock().unwrap() = Some(rx);
    let (mut runtime, native) = spawn(control.clone(), pool(16));
    until(|| runtime.has_pending());
    let rows = runtime.poll().unwrap();
    control.panic.store(true, Ordering::Release);
    until(|| !runtime.is_running());
    assert!(!rows[0].current());
    assert_eq!(native.metrics().active, 1);
    release.send(()).unwrap();
    drop((rows, runtime));
    assert!(native.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn reference_change_cancels_full_mailbox_prefix_and_reprepares_current_state() {
    let control = Arc::new(Control::default());
    control.rows.lock().unwrap().extend((0..64).map(frame));
    let (mut runtime, native) = spawn(control, pool(96));
    until(|| runtime.queued() == 32 && runtime.pressure());
    runtime.configure(KinectReferenceSpace::Torso);
    let old = runtime.poll().unwrap();
    assert!(old.iter().all(|row| !row.current()));
    let mut current = None;
    until(|| {
        for row in runtime.poll().unwrap() {
            if row.current() && !row.prepared.incoming {
                current = Some(row);
            }
        }
        current.is_some()
    });
    let row = current.unwrap();
    assert_eq!(row.prepared.space, KinectReferenceSpace::Torso);
    assert!((row.prepared.joints[KinectJoint::Head.index()].x - 0.2).abs() < 1e-6);
    assert_eq!(native.metrics().active, 1);
    drop((old, row, runtime));
    assert!(native.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn rapid_reference_round_trip_preserves_current_tracking_status_under_mailbox_pressure() {
    let control = Arc::new(Control::default());
    control.rows.lock().unwrap().extend((0..64).map(frame));
    let (mut runtime, native) = spawn(control, pool(96));
    until(|| runtime.queued() == 32 && runtime.pressure());
    runtime.configure(KinectReferenceSpace::Torso);
    runtime.configure(KinectReferenceSpace::Absolute);
    let old = runtime.poll().unwrap();
    assert!(old.iter().all(|row| !row.current()));
    let mut current = None;
    until(|| {
        for row in runtime.poll().unwrap() {
            if row.current() && !row.prepared.incoming {
                current = Some(row);
            }
        }
        current.is_some()
    });
    let row = current.unwrap();
    assert_eq!(row.prepared.space, KinectReferenceSpace::Absolute);
    assert_eq!(
        row.prepared.last_event,
        Some(format!("Tracking body {}", row.prepared.tracking_id))
    );
    assert_eq!(native.metrics().active, 1);
    drop((row, old, runtime));
    assert!(native.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn cancelled_loss_snapshot_republishes_current_tracking_status_in_the_new_revision() {
    let control = Arc::new(Control::default());
    control.rows.lock().unwrap().push_back(frame(0));
    let (mut runtime, native) = spawn(control.clone(), pool(16));
    until(|| runtime.has_pending());
    let acquired = runtime.poll().unwrap();
    control.rows.lock().unwrap().push_back(KinectRuntimePoll {
        sensor_available: false,
        frame: None,
    });
    until(|| runtime.queued() == 1);
    runtime.configure(KinectReferenceSpace::Torso);
    let cancelled = runtime.poll().unwrap();
    assert!(cancelled.iter().all(|row| !row.current()));
    until(|| runtime.has_pending());
    let current = runtime.poll().unwrap();
    assert_eq!(current[0].prepared.space, KinectReferenceSpace::Torso);
    assert!(!current[0].prepared.sensor_available);
    assert_eq!(current[0].prepared.last_event.as_deref(), Some("Lost tracked body"));
    drop((acquired, cancelled, current, runtime));
    assert!(native.wait_for_idle(Duration::from_secs(2)));
}
