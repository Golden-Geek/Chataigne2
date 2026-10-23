use super::*;
use crate::app::module::script_api::{NativeCallbackDraft, NativeCallbackPreparer};

fn resources(domains: PayloadPool, callbacks: PayloadPool) -> Resources {
    Resources {
        domains,
        callbacks: NativeCallbackPreparer::using(callbacks),
    }
}
fn large_pool(name: &'static str, count: usize) -> PayloadPool {
    PayloadPool::new(name, count, 8 * 1024 * 1024).unwrap()
}
fn spawn_with(control: Arc<Control>, resources: Resources) -> (KeyboardInputRuntime, RetirementPool) {
    let owner = slots(1);
    let native = slots(1);
    let runtime = KeyboardInputRuntime::spawn_prepared_using(
        &owner,
        &native,
        large_pool("keyboard-resource-source", 16),
        WorkerNotifier::new(|| {}),
        "auto",
        Some(resources),
        move || Ok(Fake { control, cleanup: None }),
    )
    .unwrap();
    (runtime, native)
}

#[test]
fn callback_pressure_stages_one_original_edge_and_resumes_in_order_without_polling() {
    let control = Arc::new(Control::default());
    control
        .input
        .lock()
        .unwrap()
        .extend([catalog(), event(true), event(false)]);
    let callbacks = large_pool("keyboard-worker-callbacks", 4);
    let preparer = NativeCallbackPreparer::using(callbacks.clone());
    let draft = NativeCallbackDraft::new("held", vec![]).unwrap();
    let held = preparer.try_retain(&draft).unwrap();
    let (mut runtime, native) = spawn_with(
        control.clone(),
        resources(large_pool("keyboard-worker-domains", 8), callbacks.clone()),
    );
    until(|| runtime.queued() == 1 && runtime.pressure());
    drop(runtime.poll_events().unwrap());
    let polls = control.polls.load(Ordering::Acquire);
    thread::sleep(Duration::from_millis(25));
    assert_eq!(control.polls.load(Ordering::Acquire), polls);
    drop((held, draft));
    until(|| runtime.queued() == 1);
    let first = runtime.poll_events().unwrap();
    assert!(matches!(
        first[0].prepared.row,
        super::super::preparation::Row::Input(super::super::preparation::Frame { pressed: true, .. })
    ));
    assert_eq!(runtime.queued(), 0);
    drop(first);
    until(|| runtime.queued() == 1);
    let second = runtime.poll_events().unwrap();
    assert!(matches!(
        second[0].prepared.row,
        super::super::preparation::Row::Input(super::super::preparation::Frame { pressed: false, .. })
    ));
    drop(runtime);
    assert!(native.wait_for_idle(Duration::from_secs(2)));
    assert!(!callbacks.wait_for_idle(Duration::from_millis(10)));
    drop(second);
    assert!(callbacks.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn choice_pressure_pauses_the_provider_and_exported_domain_keeps_its_canonical_reservation() {
    let control = Arc::new(Control::default());
    control.input.lock().unwrap().push_back(catalog());
    let domains = large_pool("keyboard-worker-choice-pressure", 1);
    let blocked = Arc::new(1u8);
    domains.try_retain_arc(blocked.clone(), 1).unwrap();
    let (mut runtime, native) = spawn_with(
        control.clone(),
        resources(domains.clone(), large_pool("keyboard-worker-choice-callbacks", 16)),
    );
    until(|| runtime.pressure());
    assert_eq!(runtime.queued(), 0);
    let polls = control.polls.load(Ordering::Acquire);
    thread::sleep(Duration::from_millis(25));
    assert_eq!(control.polls.load(Ordering::Acquire), polls);
    drop(blocked);
    until(|| runtime.queued() == 1);
    let publications = runtime.poll_events().unwrap();
    let exported = publications[0].prepared.catalog.options.clone();
    drop((publications, runtime));
    assert!(native.wait_for_idle(Duration::from_secs(2)));
    assert!(!domains.wait_for_idle(Duration::from_millis(10)));
    assert!(domains.try_reserve(1).is_err());
    drop(exported);
    assert!(domains.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn eight_selection_reconfigurations_invalidate_queued_sources_without_reopening_native_input() {
    let control = Arc::new(Control::default());
    control.input.lock().unwrap().push_back(catalog());
    let opens = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = opens.clone();
    let native = slots(1);
    let owner = slots(1);
    let mut runtime = KeyboardInputRuntime::spawn_using(
        &owner,
        &native,
        large_pool("keyboard-selection-sources", 32),
        WorkerNotifier::new(|| {}),
        "auto",
        move || {
            observed.fetch_add(1, Ordering::AcqRel);
            Ok(Fake { control, cleanup: None })
        },
    )
    .unwrap();
    until(|| runtime.queued() == 1);
    let mut previous = runtime.poll_events().unwrap();
    for index in 0..8 {
        let selection = if index % 2 == 0 { "fake:keyboard" } else { "auto" };
        runtime.configure(selection).unwrap();
        assert!(previous.iter().all(|row| !row.current()));
        until(|| runtime.queued() == 1);
        let options = previous[0].prepared.catalog.options.clone();
        previous = runtime.poll_events().unwrap();
        assert!(options.same_storage(&previous[0].prepared.catalog.options));
        assert!(previous.iter().all(|row| row.current()));
        assert_eq!(previous[0].prepared.catalog.selection, selection);
        assert_eq!(native.metrics().active, 1);
    }
    assert!(runtime.configure(&"x".repeat(1025)).is_err());
    assert!(previous.iter().all(|row| row.current()));
    assert_eq!(opens.load(Ordering::Acquire), 1);
    drop((previous, runtime));
    assert!(native.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn shared_frame_metadata_keeps_its_original_source_quota_after_both_envelopes_and_owner_are_gone() {
    let control = Arc::new(Control::default());
    control.input.lock().unwrap().extend([catalog(), event(true)]);
    let sources = large_pool("keyboard-shared-metadata", 8);
    let owner = slots(1);
    let native = slots(1);
    let mut runtime = KeyboardInputRuntime::spawn_using(
        &owner,
        &native,
        sources.clone(),
        WorkerNotifier::new(|| {}),
        "auto",
        move || Ok(Fake { control, cleanup: None }),
    )
    .unwrap();
    until(|| runtime.queued() == 2);
    let publications = runtime.poll_events().unwrap();
    assert!(matches!(
        publications[1].prepared.row,
        super::super::preparation::Row::Input(_)
    ));
    let metadata = publications[1].prepared.catalog.clone();
    drop((publications, runtime));
    assert!(native.wait_for_idle(Duration::from_secs(2)));
    assert!(!sources.wait_for_idle(Duration::from_millis(10)));
    drop(metadata);
    assert!(sources.wait_for_idle(Duration::from_secs(2)));
}
