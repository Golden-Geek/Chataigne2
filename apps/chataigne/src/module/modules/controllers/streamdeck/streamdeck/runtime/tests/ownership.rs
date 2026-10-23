use super::*;
use std::{
    sync::{
        atomic::{AtomicBool, AtomicUsize},
        mpsc,
    },
    time::{Duration, Instant},
};
#[derive(Default)]
struct Control {
    input: Mutex<VecDeque<Input>>,
    polls: AtomicUsize,
    rendered: Mutex<Vec<KeyVisual>>,
    fail: AtomicBool,
    block_poll: Mutex<Option<mpsc::Receiver<()>>>,
    entered_poll: AtomicBool,
    asset: AtomicBool,
    cleanup: Mutex<Option<mpsc::Receiver<()>>>,
    threads: Mutex<Vec<ThreadId>>,
}
struct Fake(Arc<Control>);
fn info() -> DeviceInfo {
    DeviceInfo {
        serial: "fixture".into(),
        product: "Fixture".into(),
        key_count: 6,
        feedback: true,
    }
}
impl Fake {
    fn record(&self) {
        let mut ids = self.0.threads.lock().unwrap();
        let id = thread::current().id();
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
}
impl Driver for Fake {
    fn discover(&mut self) -> Result<Vec<DeviceInfo>, String> {
        self.record();
        Ok(vec![info()])
    }
    fn connect(&mut self, _serial: &str) -> Result<DeviceInfo, String> {
        self.record();
        Ok(info())
    }
    fn disconnect(&mut self) -> Result<(), String> {
        self.record();
        Ok(())
    }
    fn poll(&mut self) -> Result<Vec<Input>, String> {
        self.record();
        self.0.polls.fetch_add(1, Ordering::AcqRel);
        if let Some(rx) = self.0.block_poll.lock().unwrap().take() {
            self.0.entered_poll.store(true, Ordering::Release);
            let _ = rx.recv();
        }
        if self.0.fail.load(Ordering::Acquire) {
            return Err("controlled native input failure".into());
        }
        let mut input = self.0.input.lock().unwrap();
        let count = input.len().min(16);
        Ok(input.drain(..count).collect())
    }
    fn render(&mut self, index: usize, visual: &KeyVisual) -> Result<(), RenderError> {
        self.record();
        if self.0.fail.load(Ordering::Acquire) {
            return Err(RenderError::Device("controlled render failure".into()));
        }
        if self.0.asset.load(Ordering::Acquire) {
            return Err(RenderError::Asset("controlled asset failure".into()));
        }
        let mut rendered = self.0.rendered.lock().unwrap();
        rendered.resize(6, KeyVisual::default());
        rendered[index] = visual.clone();
        Ok(())
    }
    fn brightness(&mut self, _value: u8) -> Result<(), String> {
        self.record();
        Ok(())
    }
}
impl Drop for Fake {
    fn drop(&mut self) {
        self.record();
        if let Some(rx) = self.0.cleanup.lock().unwrap().take() {
            let _ = rx.recv();
        }
    }
}
fn slots() -> RetirementPool {
    RetirementPool::new(NonZeroUsize::MIN)
}
fn pool(items: usize) -> PayloadPool {
    PayloadPool::new("streamdeck-runtime-fixture", items, 32 * 1024 * 1024).unwrap()
}
struct Frames {
    selection: RevisionSource,
    draw: RevisionSource,
    revision: u64,
    pool: PayloadPool,
}
impl Frames {
    fn new() -> Self {
        Self {
            selection: RevisionSource::default(),
            draw: RevisionSource::default(),
            revision: 0,
            pool: pool(16),
        }
    }
    fn capture(&self, color: (f64, f64, f64, f64)) -> RetainedPayload<Frame> {
        self.pool
            .try_retain(
                Frame {
                    selection: super::super::super::AUTO_DEVICE_VARIANT.into(),
                    selection_guard: self.selection.reader().guard_at(0),
                    draw_guard: self.draw.reader().guard_at(self.revision),
                    brightness: 80,
                    visuals: vec![
                        KeyVisual {
                            color,
                            ..Default::default()
                        };
                        6
                    ],
                    routes: vec![Route::fixture(); MAX_KEYS],
                },
                FRAME_WEIGHT,
            )
            .unwrap()
    }
    fn change(&mut self) -> RetainedPayload<Frame> {
        self.revision = self.draw.advance();
        self.capture((0.0, 0.0, 1.0, 1.0))
    }
}
fn until(mut condition: impl FnMut() -> bool) {
    let until = Instant::now() + Duration::from_secs(4);
    while !condition() {
        assert!(Instant::now() < until, "Stream Deck runtime fixture deadline");
        thread::sleep(Duration::from_millis(1));
    }
}
fn spawn(
    control: Arc<Control>,
    frame: RetainedPayload<Frame>,
    sources: PayloadPool,
    domains: PayloadPool,
) -> (Runtime, RetirementPool, RetirementPool) {
    let owner = slots();
    let native = slots();
    let runtime = Runtime::spawn_using(
        &owner,
        &native,
        Resources::new(sources, domains).unwrap(),
        WorkerNotifier::new(|| {}),
        frame,
        move || Ok(Fake(control)),
    )
    .unwrap();
    (runtime, owner, native)
}
#[test]
fn blocked_setup_is_worker_owned_and_keeps_admission_through_retirement() {
    let frames = Frames::new();
    let owner = slots();
    let native = slots();
    let control = Arc::new(Control::default());
    let (release, rx) = mpsc::channel();
    let (entered, ready) = mpsc::channel();
    let start = Instant::now();
    let runtime = Runtime::spawn_using(
        &owner,
        &native,
        Resources::new(pool(64), pool(64)).unwrap(),
        WorkerNotifier::new(|| {}),
        frames.capture((0.0, 0.0, 0.0, 1.0)),
        move || {
            entered.send(thread::current().id()).unwrap();
            rx.recv().unwrap();
            Ok(Fake(control))
        },
    )
    .unwrap();
    assert!(start.elapsed() < Duration::from_millis(250));
    assert_ne!(
        ready.recv_timeout(Duration::from_secs(1)).unwrap(),
        thread::current().id()
    );
    drop(runtime);
    assert_eq!(owner.metrics().active, 1);
    assert_eq!(native.metrics().active, 1);
    assert!(owner.try_reserve().is_err());
    release.send(()).unwrap();
    assert!(owner.wait_for_idle(Duration::from_secs(2)));
    assert!(native.wait_for_idle(Duration::from_secs(2)));
}
#[test]
fn five_hundred_twelve_edges_survive_full_mailbox_while_latest_feedback_advances() {
    let control = Arc::new(Control::default());
    control
        .input
        .lock()
        .unwrap()
        .extend((0..512).map(|index| if index % 2 == 0 { Input::Down(0) } else { Input::Up(0) }));
    let mut frames = Frames::new();
    let (mut runtime, owner, native) = spawn(
        control.clone(),
        frames.capture((1.0, 0.0, 0.0, 1.0)),
        pool(128),
        pool(128),
    );
    until(|| runtime.worker.event_metrics().depth == 32 && runtime.pressure());
    let polls = control.polls.load(Ordering::Acquire);
    runtime.configure(frames.change());
    until(|| {
        control
            .rendered
            .lock()
            .unwrap()
            .first()
            .is_some_and(|v| v.color == (0.0, 0.0, 1.0, 1.0))
    });
    assert_eq!(control.polls.load(Ordering::Acquire), polls);
    let mut seen = Vec::new();
    until(|| {
        for row in runtime.poll().unwrap() {
            assert!(row.current());
            if let Event::Input(input) = &row.event {
                seen.push(input.input.parts().1);
            }
        }
        seen.len() == 512
    });
    assert_eq!(seen, (0..512).map(|index| index % 2 == 0).collect::<Vec<_>>());
    assert_eq!(control.threads.lock().unwrap().len(), 1);
    assert_ne!(control.threads.lock().unwrap()[0], thread::current().id());
    drop(runtime);
    assert!(owner.wait_for_idle(Duration::from_secs(2)));
    assert!(native.wait_for_idle(Duration::from_secs(2)));
}
#[test]
fn native_failure_invalidates_and_notifies_before_blocked_cleanup() {
    let control = Arc::new(Control::default());
    let frames = Frames::new();
    let (mut runtime, owner, native) = spawn(
        control.clone(),
        frames.capture((1.0, 0.0, 0.0, 1.0)),
        pool(64),
        pool(64),
    );
    until(|| control.polls.load(Ordering::Acquire) > 0);
    let retained = runtime.poll().unwrap();
    assert!(!retained.is_empty());
    let (release, rx) = mpsc::channel();
    *control.cleanup.lock().unwrap() = Some(rx);
    control.fail.store(true, Ordering::Release);
    until(|| !runtime.running());
    assert!(retained.iter().all(|row| !row.current()));
    assert_eq!(owner.metrics().active, 1);
    assert_eq!(native.metrics().active, 1);
    assert!(owner.try_reserve().is_err());
    release.send(()).unwrap();
    drop(runtime);
    assert!(owner.wait_for_idle(Duration::from_secs(2)));
    assert!(native.wait_for_idle(Duration::from_secs(2)));
}
#[test]
fn source_capacity_pauses_input_and_recovers_without_stalling_feedback() {
    let control = Arc::new(Control::default());
    let mut frames = Frames::new();
    let sources = pool(64);
    let (mut runtime, owner, native) = spawn(
        control.clone(),
        frames.capture((1.0, 0.0, 0.0, 1.0)),
        sources.clone(),
        pool(64),
    );
    until(|| control.polls.load(Ordering::Acquire) > 0);
    drop(runtime.poll().unwrap());
    thread::sleep(Duration::from_millis(120));
    let mut held = Vec::new();
    while let Ok(value) = sources.try_retain((), INPUT_WEIGHT) {
        held.push(value);
    }
    control.input.lock().unwrap().push_back(Input::Down(0));
    until(|| runtime.pressure());
    assert!(!runtime.pending());
    runtime.configure(frames.change());
    until(|| {
        control
            .rendered
            .lock()
            .unwrap()
            .first()
            .is_some_and(|v| v.color == (0.0, 0.0, 1.0, 1.0))
    });
    drop(held);
    let mut seen = false;
    until(|| {
        for row in runtime.poll().unwrap() {
            if matches!(&row.event, Event::Input(_)) {
                seen = true;
            }
        }
        seen
    });
    drop(runtime);
    assert!(owner.wait_for_idle(Duration::from_secs(2)));
    assert!(native.wait_for_idle(Duration::from_secs(2)));
}
#[test]
fn asset_error_is_recoverable_and_does_not_disconnect_input() {
    let control = Arc::new(Control::default());
    control.asset.store(true, Ordering::Release);
    let frames = Frames::new();
    let (mut runtime, owner, native) = spawn(
        control.clone(),
        frames.capture((1.0, 0.0, 0.0, 1.0)),
        pool(64),
        pool(64),
    );
    until(|| control.polls.load(Ordering::Acquire) > 0);
    control.input.lock().unwrap().push_back(Input::Down(0));
    let mut seen = false;
    until(|| {
        for row in runtime.poll().unwrap() {
            if matches!(row.event, Event::Input(_)) {
                seen = true;
            }
        }
        seen
    });
    assert!(runtime.running());
    drop(runtime);
    assert!(owner.wait_for_idle(Duration::from_secs(2)));
    assert!(native.wait_for_idle(Duration::from_secs(2)));
}
#[cfg(feature = "streamdeck-hid")]
#[test]
fn native_endpoints_are_disabled_in_test_builds() {
    assert!(native::create().is_err());
}

#[test]
fn callback_pressure_keeps_order_and_latest_feedback_then_recovers() {
    let control = Arc::new(Control::default());
    let mut frames = Frames::new();
    let callback_pool = pool(1);
    let owner = slots();
    let native = slots();
    let provider = control.clone();
    let resources = Resources {
        sources: pool(128),
        domains: pool(128),
        callbacks: NativeCallbackPreparer::using(callback_pool.clone()),
    };
    let mut runtime = Runtime::spawn_using(
        &owner,
        &native,
        resources,
        WorkerNotifier::new(|| {}),
        frames.capture((1.0, 0.0, 0.0, 1.0)),
        move || Ok(Fake(provider)),
    )
    .unwrap();
    until(|| control.polls.load(Ordering::Acquire) > 0);
    drop(runtime.poll().unwrap());
    control.input.lock().unwrap().extend([Input::Down(0), Input::Up(0)]);
    let mut first = None;
    until(|| {
        for row in runtime.poll().unwrap() {
            if let Event::Input(input) = &row.event {
                assert_eq!(input.input, Input::Down(0));
                first = Some(row);
            }
        }
        first.is_some()
    });
    until(|| runtime.pressure());
    runtime.configure(frames.change());
    until(|| {
        control
            .rendered
            .lock()
            .unwrap()
            .first()
            .is_some_and(|v| v.color == (0.0, 0.0, 1.0, 1.0))
    });
    assert!(!callback_pool.wait_for_idle(Duration::from_millis(5)));
    drop(first);
    let mut released = false;
    until(|| {
        for row in runtime.poll().unwrap() {
            if let Event::Input(input) = &row.event {
                assert_eq!(input.input, Input::Up(0));
                released = true;
            }
        }
        released
    });
    drop(runtime);
    assert!(native.wait_for_idle(Duration::from_secs(2)));
    assert!(owner.wait_for_idle(Duration::from_secs(2)));
    assert!(callback_pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn domain_pressure_pauses_new_state_transitions_and_recovers() {
    let control = Arc::new(Control::default());
    let frames = Frames::new();
    let domains = pool(8);
    let mut held = Vec::new();
    for _ in 0..8 {
        held.push(domains.try_retain((), 1).unwrap());
    }
    let (mut runtime, owner, native) = spawn(control.clone(), frames.capture((1.0, 0.0, 0.0, 1.0)), pool(64), domains);
    until(|| runtime.pressure());
    assert_eq!(control.polls.load(Ordering::Acquire), 0);
    assert!(!runtime.pending());
    drop(held);
    until(|| {
        drop(runtime.poll().unwrap());
        control.polls.load(Ordering::Acquire) > 0
    });
    drop(runtime);
    assert!(native.wait_for_idle(Duration::from_secs(2)));
    assert!(owner.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn blocked_poll_does_not_block_reconfiguration_or_retirement() {
    let control = Arc::new(Control::default());
    let (release, rx) = mpsc::channel();
    *control.block_poll.lock().unwrap() = Some(rx);
    let mut frames = Frames::new();
    let (mut runtime, owner, native) = spawn(
        control.clone(),
        frames.capture((1.0, 0.0, 0.0, 1.0)),
        pool(64),
        pool(64),
    );
    until(|| control.entered_poll.load(Ordering::Acquire));
    let start = Instant::now();
    runtime.configure(frames.change());
    drop(runtime);
    assert!(start.elapsed() < Duration::from_millis(250));
    assert_eq!(owner.metrics().active, 1);
    release.send(()).unwrap();
    assert!(native.wait_for_idle(Duration::from_secs(2)));
    assert!(owner.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn repeated_visual_reconfiguration_keeps_one_native_owner() {
    let control = Arc::new(Control::default());
    let mut frames = Frames::new();
    let (mut runtime, owner, native) = spawn(
        control.clone(),
        frames.capture((1.0, 0.0, 0.0, 1.0)),
        pool(128),
        pool(128),
    );
    until(|| control.polls.load(Ordering::Acquire) > 0);
    for _ in 0..8 {
        runtime.configure(frames.change());
        thread::sleep(Duration::from_millis(10));
        drop(runtime.poll().unwrap());
        assert_eq!(native.metrics().active, 1);
        assert_eq!(owner.metrics().active, 1);
    }
    assert_eq!(control.threads.lock().unwrap().len(), 1);
    drop(runtime);
    assert!(native.wait_for_idle(Duration::from_secs(2)));
    assert!(owner.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn accepted_input_keeps_its_original_frame_admission_after_native_retirement() {
    let control = Arc::new(Control::default());
    let mut frames = Frames::new();
    frames.pool = pool(1);
    let frame_pool = frames.pool.clone();
    let (mut runtime, owner, native) = spawn(
        control.clone(),
        frames.capture((1.0, 0.0, 0.0, 1.0)),
        pool(128),
        pool(128),
    );
    until(|| control.polls.load(Ordering::Acquire) > 0);
    drop(runtime.poll().unwrap());
    control.input.lock().unwrap().push_back(Input::Down(0));
    let mut input = None;
    until(|| {
        for row in runtime.poll().unwrap() {
            if matches!(row.event, Event::Input(_)) {
                input = Some(row);
            }
        }
        input.is_some()
    });
    drop(runtime);
    assert!(native.wait_for_idle(Duration::from_secs(2)));
    assert!(owner.wait_for_idle(Duration::from_secs(2)));
    assert!(frame_pool.try_reserve(FRAME_WEIGHT).is_err());
    assert!(!frame_pool.wait_for_idle(Duration::from_millis(10)));
    drop(input);
    assert!(frame_pool.wait_for_idle(Duration::from_secs(2)));
}

#[test]
fn retained_byte_pressure_pauses_before_mailbox_capacity_and_recovers() {
    let control = Arc::new(Control::default());
    let sources = pool(128);
    let mut frames = Frames::new();
    let (mut runtime, owner, native) = spawn(
        control.clone(),
        frames.capture((1.0, 0.0, 0.0, 1.0)),
        sources.clone(),
        pool(128),
    );
    until(|| control.polls.load(Ordering::Acquire) > 0);
    drop(runtime.poll().unwrap());
    thread::sleep(Duration::from_millis(120));
    let held = sources
        .try_retain((), 32 * 1024 * 1024 - STATUS_WEIGHT - 2 * INPUT_WEIGHT)
        .unwrap();
    control
        .input
        .lock()
        .unwrap()
        .extend((0..64).map(|index| if index % 2 == 0 { Input::Down(0) } else { Input::Up(0) }));
    until(|| runtime.pressure());
    assert!(runtime.worker.event_metrics().depth <= 2);
    runtime.configure(frames.change());
    until(|| {
        control
            .rendered
            .lock()
            .unwrap()
            .first()
            .is_some_and(|v| v.color == (0.0, 0.0, 1.0, 1.0))
    });
    drop(held);
    let mut seen = Vec::new();
    until(|| {
        for row in runtime.poll().unwrap() {
            if let Event::Input(input) = &row.event {
                seen.push(input.input.parts().1);
            }
        }
        seen.len() == 64
    });
    assert_eq!(seen, (0..64).map(|index| index % 2 == 0).collect::<Vec<_>>());
    drop(runtime);
    assert!(native.wait_for_idle(Duration::from_secs(2)));
    assert!(owner.wait_for_idle(Duration::from_secs(2)));
}
