use super::*;
use std::{collections::VecDeque, sync::mpsc, thread, time::Instant};
#[derive(Default)]
struct Control {
    input: Mutex<VecDeque<MouseRuntimeEvent>>,
    polls: std::sync::atomic::AtomicUsize,
    panic: AtomicBool,
}
struct Fake {
    control: Arc<Control>,
    cleanup: Option<mpsc::Receiver<()>>,
}
impl Provider for Fake {
    fn poll(&mut self) -> Result<Vec<MouseRuntimeEvent>, String> {
        self.control.polls.fetch_add(1, Ordering::AcqRel);
        assert!(
            !self.control.panic.swap(false, Ordering::AcqRel),
            "controlled mouse failure"
        );
        Ok(self.control.input.lock().unwrap().pop_front().into_iter().collect())
    }
}
impl Drop for Fake {
    fn drop(&mut self) {
        if let Some(cleanup) = self.cleanup.take() {
            let _ = cleanup.recv();
        }
    }
}
fn slots(count: usize) -> RetirementPool {
    RetirementPool::new(NonZeroUsize::new(count).unwrap())
}
fn pool(count: usize) -> PayloadPool {
    PayloadPool::new(
        "mouse-fixture",
        count + 1,
        CATALOG_WEIGHT + count * preparation::FRAME_WEIGHT,
    )
    .unwrap()
}
fn edge(pressed: bool) -> MouseRuntimeEvent {
    MouseRuntimeEvent::Input {
        device: "fake:mouse".into(),
        event: MouseInputEvent::ButtonChanged {
            button: MouseButtonKind::Left,
            pressed,
        },
    }
}
fn until(mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !predicate() {
        assert!(Instant::now() < deadline, "mouse fixture deadline");
        thread::sleep(Duration::from_millis(1));
    }
}
fn spawn(
    control: Arc<Control>,
    cleanup: Option<mpsc::Receiver<()>>,
    input: PayloadPool,
) -> (MouseInputRuntime, RetirementPool, RetirementPool) {
    let owner = slots(1);
    let global = slots(1);
    let pending = std::mem::take(&mut *control.input.lock().unwrap());
    control.input.lock().unwrap().push_back(catalog());
    let next = control.clone();
    let mut handle = MouseInputRuntime::spawn_using(
        &owner,
        &global,
        input,
        WorkerNotifier::new(|| {}),
        MouseInputConfig::default(),
        move || Ok(Fake { control, cleanup }),
    )
    .unwrap();
    until(|| handle.has_pending());
    drop(handle.poll_events().unwrap());
    next.input.lock().unwrap().extend(pending);
    (handle, owner, global)
}
#[test]
fn retained_source_capacity_pauses_polling_until_the_original_is_disposed() {
    let control = Arc::new(Control::default());
    control.input.lock().unwrap().extend([edge(true), edge(false)]);
    let input = pool(1);
    let (mut handle, owner, global) = spawn(control.clone(), None, input.clone());
    until(|| handle.has_pending() && handle.pressure());
    let polls = control.polls.load(Ordering::Acquire);
    let events = handle.poll_events().unwrap();
    let retained = events[0].clone();
    drop(events);
    assert!(input.try_reserve(INPUT_WEIGHT).is_err());
    thread::sleep(Duration::from_millis(25));
    assert_eq!(polls, control.polls.load(Ordering::Acquire));
    drop(retained);
    until(|| handle.has_pending());
    assert!(
        matches!(&handle.poll_events().unwrap()[0].prepared.row,preparation::Row::Input(frame) if !frame.buttons[0])
    );
    drop(handle);
    assert!(global.wait_for_idle(Duration::from_secs(1)));
    assert!(owner.wait_for_idle(Duration::from_secs(1)));
    assert!(input.wait_for_idle(Duration::from_secs(1)));
}
#[test]
fn blocked_startup_and_drop_never_wait_and_keep_both_lifetime_slots() {
    let owner = slots(1);
    let global = slots(1);
    let (release, gate) = mpsc::channel();
    let start = Instant::now();
    let handle = MouseInputRuntime::spawn_using(
        &owner,
        &global,
        pool(1),
        WorkerNotifier::new(|| {}),
        MouseInputConfig::default(),
        move || {
            gate.recv().unwrap();
            Ok(Fake {
                control: Arc::new(Control::default()),
                cleanup: None,
            })
        },
    )
    .unwrap();
    assert!(start.elapsed() < Duration::from_millis(100));
    drop(handle);
    assert_eq!(owner.metrics().active, 1);
    assert_eq!(global.metrics().active, 1);
    assert!(MouseInputRuntime::spawn_using(
        &owner,
        &slots(1),
        pool(1),
        WorkerNotifier::new(|| {}),
        MouseInputConfig::default(),
        || -> Result<Fake, String> { panic!("owner must reject before open") }
    )
    .is_err());
    release.send(()).unwrap();
    assert!(global.wait_for_idle(Duration::from_secs(1)));
    assert!(owner.wait_for_idle(Duration::from_secs(1)));
}
#[test]
fn failure_is_visible_and_retires_retained_sources_before_blocked_cleanup() {
    let control = Arc::new(Control::default());
    control.input.lock().unwrap().push_back(edge(true));
    let (release, cleanup) = mpsc::channel();
    let (mut handle, owner, global) = spawn(control.clone(), Some(cleanup), pool(2));
    until(|| handle.has_pending());
    let events = handle.poll_events().unwrap();
    assert!(events[0].current());
    control.panic.store(true, Ordering::Release);
    until(|| !handle.is_running());
    assert!(!events[0].current());
    assert!(handle.poll_events().is_err());
    let start = Instant::now();
    drop(handle);
    assert!(start.elapsed() < Duration::from_millis(100));
    assert_eq!(owner.metrics().active, 1);
    assert_eq!(global.metrics().active, 1);
    release.send(()).unwrap();
    assert!(global.wait_for_idle(Duration::from_secs(1)));
    assert!(owner.wait_for_idle(Duration::from_secs(1)));
}
#[test]
fn actor_turn_is_sixteen_and_ordered_edges_survive_continuous_input() {
    let control = Arc::new(Control::default());
    control.input.lock().unwrap().extend((0..80).map(|i| edge(i % 2 == 0)));
    let (mut handle, owner, global) = spawn(control, None, pool(128));
    until(|| handle.worker.event_metrics().depth == 32 && handle.pressure());
    let mut actual = Vec::new();
    while actual.len() < 80 {
        let batch = handle.poll_events().unwrap();
        assert!(batch.len() <= 16);
        actual.extend(batch.into_iter().filter_map(|row| match &row.prepared.row {
            preparation::Row::Input(frame) => Some(frame.buttons[0]),
            _ => None,
        }));
        thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(actual, (0..80).map(|i| i % 2 == 0).collect::<Vec<_>>());
    drop(handle);
    assert!(global.wait_for_idle(Duration::from_secs(1)));
    assert!(owner.wait_for_idle(Duration::from_secs(1)));
}
#[test]
fn cancellation_under_full_output_retires_retained_publications_and_recovers_slots() {
    let control = Arc::new(Control::default());
    control.input.lock().unwrap().extend((0..80).map(|i| edge(i % 2 == 0)));
    let (mut handle, owner, global) = spawn(control, None, pool(128));
    until(|| handle.worker.event_metrics().depth == 32 && handle.pressure());
    let events = handle.poll_events().unwrap();
    drop(handle);
    assert!(events.iter().all(|row| !row.current()));
    assert!(global.wait_for_idle(Duration::from_secs(1)));
    assert!(owner.wait_for_idle(Duration::from_secs(1)));
}
#[test]
fn repeated_failed_startup_and_reconfigure_do_not_grow_worker_lifetimes() {
    let owner = slots(1);
    let global = slots(1);
    for _ in 0..8 {
        let mut handle = MouseInputRuntime::spawn_using::<Fake>(
            &owner,
            &global,
            pool(1),
            WorkerNotifier::new(|| {}),
            MouseInputConfig::default(),
            || Err("controlled startup failure".into()),
        )
        .unwrap();
        until(|| !handle.is_running());
        assert!(handle.poll_events().err().unwrap().contains("startup failure"));
        drop(handle);
        assert!(global.wait_for_idle(Duration::from_secs(1)));
        assert!(owner.wait_for_idle(Duration::from_secs(1)));
    }
}
#[test]
fn metadata_admission_counts_backing_capacity_and_provider_turns() {
    let mut id = String::with_capacity(1025);
    id.push('x');
    assert!(validate(&MouseRuntimeEvent::Input {
        device: id,
        event: MouseInputEvent::Moved {
            x: 0,
            y: 0,
            dx: 0,
            dy: 0
        }
    })
    .is_err());
    let devices = Vec::with_capacity(65);
    assert!(validate(&MouseRuntimeEvent::DevicesChanged(devices)).is_err());
    struct TooMany;
    impl Provider for TooMany {
        fn poll(&mut self) -> Result<Vec<MouseRuntimeEvent>, String> {
            Ok((0..17).map(|_| edge(true)).collect())
        }
    }
    let owner = slots(1);
    let global = slots(1);
    let mut handle = MouseInputRuntime::spawn_using(
        &owner,
        &global,
        pool(32),
        WorkerNotifier::new(|| {}),
        MouseInputConfig::default(),
        || Ok(TooMany),
    )
    .unwrap();
    until(|| !handle.is_running());
    assert!(handle.poll_events().err().unwrap().contains("16 records"));
    drop(handle);
    assert!(global.wait_for_idle(Duration::from_secs(1)));
}
#[test]
fn ordinary_motion_keeps_its_delta_and_button_occurrences_in_source_order() {
    let control = Arc::new(Control::default());
    let expected = vec![
        MouseRuntimeEvent::Input {
            device: "fake:mouse".into(),
            event: MouseInputEvent::Moved {
                x: 10,
                y: 20,
                dx: 3,
                dy: -2,
            },
        },
        edge(true),
        edge(false),
    ];
    control.input.lock().unwrap().extend(expected.clone());
    let (mut handle, _, global) = spawn(control, None, pool(4));
    let mut states = Vec::new();
    until(|| {
        states.extend(
            handle
                .poll_events()
                .unwrap()
                .into_iter()
                .filter_map(|row| match &row.prepared.row {
                    preparation::Row::Input(frame) => Some((frame.position, frame.buttons[0])),
                    _ => None,
                }),
        );
        states.len() == 3
    });
    assert_eq!(states, vec![((10, 20), false), ((10, 20), true), ((10, 20), false)]);
    drop(handle);
    assert!(global.wait_for_idle(Duration::from_secs(1)));
}

fn catalog() -> MouseRuntimeEvent {
    MouseRuntimeEvent::DevicesChanged(vec![DiscoveredMouseDevice {
        index: 0,
        variant_id: "fake:mouse".into(),
        label: "Fake Mouse".into(),
        details: "fixture".into(),
    }])
}
mod publication;
