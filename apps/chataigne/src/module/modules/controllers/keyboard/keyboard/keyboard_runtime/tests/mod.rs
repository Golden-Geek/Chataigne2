use super::*;
use std::{collections::VecDeque, sync::mpsc, thread, time::Instant};

#[derive(Default)]
struct Control {
    input: Mutex<VecDeque<KeyboardRuntimeEvent>>,
    polls: std::sync::atomic::AtomicUsize,
    panic: AtomicBool,
}
struct Fake {
    control: Arc<Control>,
    cleanup: Option<mpsc::Receiver<()>>,
}
impl Provider for Fake {
    fn poll(&mut self) -> Result<Vec<KeyboardRuntimeEvent>, String> {
        self.control.polls.fetch_add(1, Ordering::AcqRel);
        assert!(
            !self.control.panic.swap(false, Ordering::AcqRel),
            "controlled keyboard failure"
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
    PayloadPool::new("keyboard-fake-input", count, count * INPUT_WEIGHT).unwrap()
}
fn event(pressed: bool) -> KeyboardRuntimeEvent {
    KeyboardRuntimeEvent::Input {
        device: "fake:keyboard".into(),
        event: KeyboardInputEvent::KeyChanged {
            key: KeyboardKey::A,
            pressed,
        },
    }
}
fn until(mut predicate: impl FnMut() -> bool) {
    let limit = Instant::now() + Duration::from_secs(3);
    while !predicate() {
        assert!(Instant::now() < limit, "keyboard fixture deadline");
        thread::sleep(Duration::from_millis(1));
    }
}
fn spawn(
    control: Arc<Control>,
    cleanup: Option<mpsc::Receiver<()>>,
    input: PayloadPool,
) -> (KeyboardInputRuntime, RetirementPool, RetirementPool) {
    let owner = slots(1);
    let global = slots(1);
    let handle = KeyboardInputRuntime::spawn_using(&owner, &global, input, WorkerNotifier::new(|| {}), move || {
        Ok(Fake { control, cleanup })
    })
    .unwrap();
    (handle, owner, global)
}
#[test]
fn input_capacity_follows_the_original_publication_until_disposal_and_pauses_polling() {
    let control = Arc::new(Control::default());
    control.input.lock().unwrap().extend([event(true), event(false)]);
    let input = pool(1);
    let (mut handle, owner, global) = spawn(control.clone(), None, input.clone());
    until(|| handle.worker.event_metrics().depth == 1 && handle.pressure());
    let polls = control.polls.load(Ordering::Acquire);
    let events = handle.poll_events().unwrap();
    assert_eq!(events.len(), 1);
    assert!(input.try_reserve(INPUT_WEIGHT).is_err());
    thread::sleep(Duration::from_millis(25));
    assert_eq!(control.polls.load(Ordering::Acquire), polls);
    drop(events);
    until(|| handle.worker.event_metrics().depth == 1);
    assert!(matches!(
        handle.poll_events().unwrap()[0].event,
        KeyboardRuntimeEvent::Input {
            event: KeyboardInputEvent::KeyChanged { pressed: false, .. },
            ..
        }
    ));
    drop(handle);
    assert!(global.wait_for_idle(Duration::from_secs(1)));
    assert!(owner.wait_for_idle(Duration::from_secs(1)));
    assert!(input.wait_for_idle(Duration::from_secs(1)));
}
#[test]
fn blocked_startup_returns_immediately_and_holds_both_lifetime_slots_until_cleanup() {
    let owner = slots(1);
    let global = slots(1);
    let (release, gate) = mpsc::channel();
    let control = Arc::new(Control::default());
    let start = Instant::now();
    let handle = KeyboardInputRuntime::spawn_using(&owner, &global, pool(1), WorkerNotifier::new(|| {}), move || {
        gate.recv().unwrap();
        Ok(Fake { control, cleanup: None })
    })
    .unwrap();
    assert!(start.elapsed() < Duration::from_millis(100));
    drop(handle);
    assert_eq!(owner.metrics().active, 1);
    assert_eq!(global.metrics().active, 1);
    release.send(()).unwrap();
    assert!(global.wait_for_idle(Duration::from_secs(1)));
    assert!(owner.wait_for_idle(Duration::from_secs(1)));
}
#[test]
fn native_panic_is_observable_and_invalidates_publications_before_blocked_cleanup() {
    let control = Arc::new(Control::default());
    control.input.lock().unwrap().push_back(event(true));
    let (release, cleanup) = mpsc::channel();
    let (mut handle, owner, global) = spawn(control.clone(), Some(cleanup), pool(2));
    until(|| handle.worker.event_metrics().depth == 1);
    let events = handle.poll_events().unwrap();
    assert!(events[0].current());
    control.panic.store(true, Ordering::Release);
    until(|| !handle.shared.running.load(Ordering::Acquire));
    assert!(!events[0].current());
    assert!(handle.poll_events().is_err());
    assert_eq!(global.metrics().active, 1);
    let start = Instant::now();
    drop(handle);
    assert!(start.elapsed() < Duration::from_millis(100));
    assert_eq!(owner.metrics().active, 1);
    release.send(()).unwrap();
    assert!(global.wait_for_idle(Duration::from_secs(1)));
    assert!(owner.wait_for_idle(Duration::from_secs(1)));
}
#[test]
fn actor_drain_is_capped_at_sixteen_and_retirement_rejects_retained_sources() {
    let control = Arc::new(Control::default());
    control.input.lock().unwrap().extend((0..40).map(|i| event(i % 2 == 0)));
    let (mut handle, owner, global) = spawn(control, None, pool(64));
    until(|| handle.worker.event_metrics().depth == 32 && handle.pressure());
    let events = handle.poll_events().unwrap();
    assert_eq!(events.len(), 16);
    assert!(events.iter().all(|event| event.current()));
    drop(handle);
    assert!(events.iter().all(|event| !event.current()));
    assert!(global.wait_for_idle(Duration::from_secs(1)));
    assert!(owner.wait_for_idle(Duration::from_secs(1)));
}
#[test]
fn catalog_and_id_admission_rejects_overlarge_native_records() {
    assert!(validate(&KeyboardRuntimeEvent::Input {
        device: "a".repeat(1025),
        event: KeyboardInputEvent::KeyChanged {
            key: KeyboardKey::A,
            pressed: true
        }
    })
    .is_err());
    let device = DiscoveredKeyboardDevice {
        index: 0,
        variant_id: "fake".into(),
        label: "Fake".into(),
        details: "".into(),
    };
    assert!(validate(&KeyboardRuntimeEvent::DevicesChanged(vec![device.clone(); 65])).is_err());
    assert_eq!(
        validate(&KeyboardRuntimeEvent::DevicesChanged(vec![device; 64])).unwrap(),
        CATALOG_WEIGHT
    );
}
