use super::*;
use crate::app::module::script_api::NativeCallbackDraft;
use std::{
    collections::VecDeque,
    sync::{atomic::AtomicUsize, mpsc},
    thread,
    time::Instant,
};
#[derive(Default)]
struct Control {
    rows: Mutex<VecDeque<Record>>,
    polls: AtomicUsize,
    panic: AtomicBool,
    cleanup: Mutex<Option<mpsc::Receiver<()>>>,
}
struct Fake(Arc<Control>);
impl Provider for Fake {
    fn poll(&mut self) -> Result<Vec<Record>, String> {
        self.0.polls.fetch_add(1, Ordering::AcqRel);
        assert!(!self.0.panic.swap(false, Ordering::AcqRel), "controlled Gamepad panic");
        let mut rows = self.0.rows.lock().unwrap();
        let count = rows.len().min(16);
        Ok(rows.drain(..count).collect())
    }
    fn state(&mut self, _selection: &str) -> Result<Option<GamepadStateSnapshot>, String> {
        Ok(None)
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
fn pool(count: usize) -> PayloadPool {
    PayloadPool::new("gamepad-runtime-test", count + 8, 32 * 1024 * 1024).unwrap()
}
fn device() -> DiscoveredGamepad {
    DiscoveredGamepad {
        index: 0,
        variant_id: "gamepad:0|Pad".into(),
        label: "Pad".into(),
        details: "fixture".into(),
    }
}
fn edge(pressed: bool) -> Record {
    Record::Event(if pressed {
        GamepadRuntimeEvent::ButtonPressed {
            device: device(),
            button: GamepadButton::South,
        }
    } else {
        GamepadRuntimeEvent::ButtonReleased {
            device: device(),
            button: GamepadButton::South,
        }
    })
}
fn until(mut predicate: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(4);
    while !predicate() {
        assert!(Instant::now() < end, "Gamepad runtime fixture deadline");
        thread::sleep(Duration::from_millis(1));
    }
}
fn spawn(
    control: Arc<Control>,
    sources: PayloadPool,
    resources: Option<Resources>,
) -> (GamepadRuntime, RetirementPool) {
    let native = slots();
    let owner = slots();
    let runtime = GamepadRuntime::spawn_using(
        &owner,
        &native,
        sources,
        WorkerNotifier::new(|| {}),
        Settings::default(),
        resources,
        move || Ok(Fake(control)),
    )
    .unwrap();
    (runtime, native)
}
#[test]
fn native_gamepad_endpoint_cannot_open_in_test_builds() {
    assert!(native::create().is_err());
}
#[test]
fn blocked_open_retains_both_lifetime_slots_without_blocking_the_actor() {
    let owner = slots();
    let native = slots();
    let (release, rx) = mpsc::channel();
    let (entered, ready) = mpsc::channel();
    let control = Arc::new(Control::default());
    let before = Instant::now();
    let runtime = GamepadRuntime::spawn_using(
        &owner,
        &native,
        pool(8),
        WorkerNotifier::new(|| {}),
        Settings::default(),
        None,
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
fn full_mailbox_preserves_sixty_four_button_occurrences_in_bounded_turns() {
    let control = Arc::new(Control::default());
    control.rows.lock().unwrap().push_back(Record::Catalog(vec![device()]));
    control.rows.lock().unwrap().extend((0..64).map(|i| edge(i % 2 == 0)));
    let (mut runtime, native) = spawn(control.clone(), pool(96), None);
    until(|| runtime.queued() == 32 && runtime.pressure());
    let polls = control.polls.load(Ordering::Acquire);
    thread::sleep(Duration::from_millis(25));
    assert_eq!(polls, control.polls.load(Ordering::Acquire));
    let mut seen = Vec::new();
    until(|| {
        let rows = runtime.poll_events().unwrap();
        assert!(rows.len() <= 16);
        for row in rows {
            if row.prepared.incoming {
                seen.push(row.prepared.buttons[0]);
            }
        }
        seen.len() == 64
    });
    assert_eq!(seen, (0..64).map(|i| i % 2 == 0).collect::<Vec<_>>());
    drop(runtime);
    assert!(native.wait_for_idle(Duration::from_secs(2)));
}
#[test]
fn retained_publications_keep_original_source_capacity_through_retirement() {
    let control = Arc::new(Control::default());
    control
        .rows
        .lock()
        .unwrap()
        .extend([Record::Catalog(vec![device()]), edge(true)]);
    let sources = pool(2);
    let (mut runtime, native) = spawn(control, sources.clone(), None);
    until(|| runtime.queued() == 2);
    let rows = runtime.poll_events().unwrap();
    let exported = rows[1].clone();
    drop((rows, runtime));
    assert!(native.wait_for_idle(Duration::from_secs(2)));
    assert!(!exported.current());
    assert!(!sources.wait_for_idle(Duration::from_millis(10)));
    drop(exported);
    assert!(sources.wait_for_idle(Duration::from_secs(2)));
}
#[test]
fn configuration_cancels_callback_pressure_without_free_capacity_or_native_reopen() {
    let callbacks_pool = PayloadPool::new("gamepad-held-callback", 1, 1024 * 1024).unwrap();
    let callbacks = NativeCallbackPreparer::using(callbacks_pool.clone());
    let draft = NativeCallbackDraft::new("held", vec![]).unwrap();
    let held = callbacks.try_retain(&draft).unwrap();
    let control = Arc::new(Control::default());
    control
        .rows
        .lock()
        .unwrap()
        .extend([Record::Catalog(vec![device()]), edge(true)]);
    let resources = Resources {
        domains: PayloadPool::new("gamepad-domains", 16, 16 * 1024 * 1024).unwrap(),
        callbacks,
    };
    let (mut runtime, native) = spawn(control.clone(), pool(16), Some(resources));
    until(|| runtime.queued() == 1 && runtime.pressure());
    let old = runtime.poll_events().unwrap();
    runtime.configure("none", Settings::default().axes).unwrap();
    assert!(old.iter().all(|r| !r.current()));
    until(|| runtime.has_pending());
    let rows = runtime.poll_events().unwrap();
    assert_eq!(rows[0].prepared.catalog.selection, "none");
    assert!(!rows[0].prepared.buttons[0]);
    assert_eq!(native.metrics().active, 1);
    assert!(callbacks_pool.try_reserve(1).is_err());
    drop((held, draft, rows, old, runtime));
    assert!(native.wait_for_idle(Duration::from_secs(2)));
    assert!(callbacks_pool.wait_for_idle(Duration::from_secs(2)));
}
#[test]
fn eight_selection_revisions_preserve_domains_and_reject_invalid_configuration_atomically() {
    let control = Arc::new(Control::default());
    control.rows.lock().unwrap().push_back(Record::Catalog(vec![device()]));
    let (mut runtime, native) = spawn(control, pool(32), None);
    until(|| runtime.has_pending());
    let mut previous = runtime.poll_events().unwrap();
    for i in 0..8 {
        runtime
            .configure(if i % 2 == 0 { "none" } else { "auto" }, Settings::default().axes)
            .unwrap();
        assert!(previous.iter().all(|r| !r.current()));
        until(|| runtime.has_pending());
        let next = runtime.poll_events().unwrap();
        assert!(previous[0]
            .prepared
            .catalog
            .options
            .same_storage(&next[0].prepared.catalog.options));
        previous = next;
    }
    let mut axes = Settings::default().axes;
    axes[0].offset = f64::NAN;
    assert!(runtime.configure("auto", axes).is_err());
    assert!(previous[0].current());
    drop((previous, runtime));
    assert!(native.wait_for_idle(Duration::from_secs(2)));
}
#[test]
fn panic_retires_source_and_notifies_before_blocked_provider_cleanup() {
    let control = Arc::new(Control::default());
    control.rows.lock().unwrap().push_back(Record::Catalog(vec![device()]));
    let (release, rx) = mpsc::channel();
    *control.cleanup.lock().unwrap() = Some(rx);
    let (mut runtime, native) = spawn(control.clone(), pool(16), None);
    until(|| runtime.has_pending());
    let rows = runtime.poll_events().unwrap();
    control.panic.store(true, Ordering::Release);
    until(|| !runtime.is_running());
    assert!(!rows[0].current());
    assert_eq!(native.metrics().active, 1);
    release.send(()).unwrap();
    drop((rows, runtime));
    assert!(native.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn full_choice_domain_capacity_retains_catalog_until_capacity_recovers() {
    let domains = PayloadPool::new("gamepad-held-domain", 1, 4 * 1024 * 1024).unwrap();
    let held = domains.try_retain(0u8, 4 * 1024 * 1024).unwrap();
    let control = Arc::new(Control::default());
    control
        .rows
        .lock()
        .unwrap()
        .extend([Record::Catalog(vec![device()]), edge(true)]);
    let resources = Resources {
        domains: domains.clone(),
        callbacks: NativeCallbackPreparer::new().unwrap(),
    };
    let (mut runtime, native) = spawn(control.clone(), pool(16), Some(resources));
    until(|| runtime.pressure());
    assert!(!runtime.has_pending());
    let polls = control.polls.load(Ordering::Acquire);
    thread::sleep(Duration::from_millis(25));
    assert_eq!(polls, control.polls.load(Ordering::Acquire));
    drop(held);
    let mut rows = Vec::new();
    until(|| {
        rows.extend(runtime.poll_events().unwrap());
        rows.len() == 2
    });
    assert!(rows[1].prepared.buttons[0]);
    let options = rows[0].prepared.catalog.options.clone();
    drop((rows, runtime));
    assert!(native.wait_for_idle(Duration::from_secs(2)));
    assert!(!domains.wait_for_idle(Duration::from_millis(10)));
    drop(options);
    assert!(domains.wait_for_idle(Duration::from_secs(2)));
}
