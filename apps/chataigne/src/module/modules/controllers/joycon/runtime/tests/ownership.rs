use super::super::*;
use std::{
    collections::VecDeque,
    sync::{atomic::AtomicUsize, mpsc},
    thread,
};

#[derive(Default)]
struct Control {
    actions: Mutex<Vec<JoyConWorkerCommand>>,
    input: Mutex<VecDeque<JoyConWorkerEvent>>,
    polls: AtomicUsize,
    maintenance: AtomicUsize,
    panic_action: AtomicBool,
}
struct Fake {
    control: Arc<Control>,
    action: Option<mpsc::Receiver<()>>,
    cleanup: Option<mpsc::Receiver<()>>,
}
impl Provider for Fake {
    fn execute(&mut self, command: &JoyConWorkerCommand) -> Result<String, String> {
        self.control.actions.lock().unwrap().push(command.clone());
        if let Some(gate) = self.action.take() {
            gate.recv().unwrap();
        }
        assert!(
            !self.control.panic_action.swap(false, Ordering::AcqRel),
            "controlled Joy-Con failure"
        );
        Ok("sent".into())
    }
    fn poll(&mut self) -> Vec<JoyConWorkerEvent> {
        self.control.polls.fetch_add(1, Ordering::AcqRel);
        self.control.input.lock().unwrap().pop_front().into_iter().collect()
    }
    fn maintain(&mut self) {
        self.control.maintenance.fetch_add(1, Ordering::AcqRel);
    }
}
impl Drop for Fake {
    fn drop(&mut self) {
        if let Some(gate) = self.cleanup.take() {
            let _ = gate.recv();
        }
    }
}
fn slots() -> RetirementPool {
    RetirementPool::new(NonZeroUsize::MIN)
}
fn pool(input: bool, count: usize) -> PayloadPool {
    PayloadPool::new(
        "joycon-controlled",
        count,
        count * if input { EVENT_WEIGHT } else { COMMAND_WEIGHT },
    )
    .unwrap()
}
fn spawn(
    control: Arc<Control>,
    action: Option<mpsc::Receiver<()>>,
    cleanup: Option<mpsc::Receiver<()>>,
    input: PayloadPool,
) -> (JoyConRuntimeHandle, RetirementPool, PayloadPool) {
    let slots = slots();
    let commands = pool(false, 128);
    let handle =
        JoyConRuntimeHandle::spawn_using(WorkerNotifier::new(|| {}), &slots, commands.clone(), input, move || {
            Ok(Fake {
                control,
                action,
                cleanup,
            })
        })
        .unwrap();
    (handle, slots, commands)
}
fn until(mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !predicate() {
        assert!(Instant::now() < deadline, "Joy-Con fixture deadline");
        thread::sleep(Duration::from_millis(1));
    }
}
fn command(index: usize) -> JoyConWorkerCommand {
    JoyConWorkerCommand::Vibrate(JoyConVibrateRequest {
        target: "both".into(),
        frequency_hz: 300.0,
        amplitude: 0.5,
        duration_ms: 60,
        description: index.to_string(),
    })
}
fn state(pressed: bool) -> JoyConWorkerEvent {
    let mut state = JoyConRuntimeState::disconnected();
    state.left.connected = true;
    if pressed {
        state.left.left_buttons.push(Buttons::Down);
    }
    JoyConWorkerEvent::State(Box::new(state))
}

#[test]
fn sixty_four_sources_keep_original_capacity_through_receipt_and_disposal() {
    let control = Arc::new(Control::default());
    let (release, gate) = mpsc::channel();
    let (mut handle, slots, commands) = spawn(control.clone(), Some(gate), None, pool(true, 64));
    let mut receipts = vec![handle.send(command(0)).unwrap()];
    until(|| control.actions.lock().unwrap().len() == 1);
    for index in 1..64 {
        receipts.push(handle.send(command(index)).unwrap());
    }
    assert!(handle.send(command(64)).is_err());
    release.send(()).unwrap();
    until(|| receipts.iter().all(Receipt::complete));
    assert_eq!(
        *control.actions.lock().unwrap(),
        (0..64).map(command).collect::<Vec<_>>()
    );
    assert!(handle.send(command(65)).is_err());
    let retained = receipts.remove(0).source();
    drop(receipts);
    until(|| handle.admission.metrics().active == 1);
    assert!(commands.try_reserve(128 * COMMAND_WEIGHT).is_err());
    drop(retained);
    assert!(commands.wait_for_idle(Duration::from_secs(1)));
    let next = handle.send(command(66)).unwrap();
    until(|| next.complete());
    assert_eq!(next.take(), Some(Outcome::Completed(Ok("sent".into()))));
    handle.stop();
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn retirement_returns_before_action_and_cleanup_and_resolves_every_accepted_suffix() {
    let control = Arc::new(Control::default());
    let (release, action) = mpsc::channel();
    let (finish, cleanup) = mpsc::channel();
    let (mut handle, slots, _) = spawn(control.clone(), Some(action), Some(cleanup), pool(true, 64));
    let first = handle.send(command(0)).unwrap();
    until(|| control.actions.lock().unwrap().len() == 1);
    let suffix: Vec<_> = (1..64).map(|index| handle.send(command(index)).unwrap()).collect();
    let start = Instant::now();
    handle.stop();
    assert!(start.elapsed() < Duration::from_millis(100));
    assert_eq!(slots.metrics().active, 1);
    release.send(()).unwrap();
    until(|| !handle.is_running() && suffix.iter().all(Receipt::complete));
    assert_eq!(first.take(), Some(Outcome::Completed(Ok("sent".into()))));
    assert!(suffix.iter().all(|receipt| receipt.take() == Some(Outcome::NotSent)));
    assert_eq!(control.actions.lock().unwrap().len(), 1);
    assert_eq!(slots.metrics().active, 1);
    finish.send(()).unwrap();
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn panic_marks_started_work_unconfirmed_and_cancels_suffix_before_blocked_cleanup() {
    let control = Arc::new(Control::default());
    control.panic_action.store(true, Ordering::Release);
    let (release, action) = mpsc::channel();
    let (finish, cleanup) = mpsc::channel();
    let (handle, slots, _) = spawn(control.clone(), Some(action), Some(cleanup), pool(true, 64));
    let first = handle.send(command(0)).unwrap();
    until(|| control.actions.lock().unwrap().len() == 1);
    let suffix = handle.send(command(1)).unwrap();
    release.send(()).unwrap();
    until(|| !handle.is_running() && first.complete() && suffix.complete());
    assert_eq!(first.take(), Some(Outcome::Unconfirmed));
    assert_eq!(suffix.take(), Some(Outcome::NotSent));
    assert_eq!(slots.metrics().active, 1);
    finish.send(()).unwrap();
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn full_input_mailbox_pauses_reads_but_preserves_commands_and_native_maintenance() {
    let control = Arc::new(Control::default());
    control
        .input
        .lock()
        .unwrap()
        .extend((0..512).map(|index| state(index % 2 == 0)));
    let (mut handle, slots, _) = spawn(control.clone(), None, None, pool(true, 64));
    until(|| handle.pressure());
    let polls = control.polls.load(Ordering::Acquire);
    let maintained = control.maintenance.load(Ordering::Acquire);
    let receipt = handle.send(command(0)).unwrap();
    until(|| receipt.complete());
    until(|| control.maintenance.load(Ordering::Acquire) > maintained + 2);
    assert_eq!(control.polls.load(Ordering::Acquire), polls);
    assert_eq!(receipt.take(), Some(Outcome::Completed(Ok("sent".into()))));
    let mut observed = Vec::new();
    until(|| {
        while let Ok(event) = handle.try_recv() {
            if let JoyConWorkerEvent::State(state) = &event.event {
                observed.push(!state.left.left_buttons.is_empty());
            }
        }
        observed.len() == 512
    });
    assert_eq!(observed, (0..512).map(|index| index % 2 == 0).collect::<Vec<_>>());
    handle.stop();
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn retained_publication_keeps_original_pool_quota_and_retirement_invalidates_queued_input() {
    let control = Arc::new(Control::default());
    control.input.lock().unwrap().extend([state(true), state(false)]);
    let input = pool(true, 1);
    let (mut handle, slots, _) = spawn(control.clone(), None, None, input.clone());
    let mut publication = None;
    until(|| {
        publication = handle.try_recv().ok();
        publication.is_some()
    });
    let publication = publication.unwrap();
    assert!(publication.guards.iter().all(RevisionGuard::is_current));
    until(|| handle.pressure());
    assert!(input.try_reserve(EVENT_WEIGHT).is_err());
    handle.stop();
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
    assert!(!publication.guards.iter().all(RevisionGuard::is_current));
    assert!(input.try_reserve(EVENT_WEIGHT).is_err());
    drop(publication);
    assert!(input.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn startup_reserves_lifetime_before_blocked_open_and_cancellation_never_joins() {
    let slots = slots();
    let (started, ready) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let mut handle = JoyConRuntimeHandle::spawn_using(
        WorkerNotifier::new(|| {}),
        &slots,
        pool(false, 64),
        pool(true, 64),
        move || {
            started.send(()).unwrap();
            gate.recv().unwrap();
            Ok(Fake {
                control: Arc::new(Control::default()),
                action: None,
                cleanup: None,
            })
        },
    )
    .unwrap();
    ready.recv_timeout(Duration::from_secs(1)).unwrap();
    assert_eq!(slots.metrics().active, 1);
    let calls = Arc::new(AtomicBool::new(false));
    let observed = calls.clone();
    assert!(JoyConRuntimeHandle::spawn_using(
        WorkerNotifier::new(|| {}),
        &slots,
        pool(false, 64),
        pool(true, 64),
        move || {
            observed.store(true, Ordering::Release);
            Ok(Fake {
                control: Arc::new(Control::default()),
                action: None,
                cleanup: None,
            })
        }
    )
    .is_err());
    assert!(!calls.load(Ordering::Acquire));
    let before = Instant::now();
    handle.stop();
    assert!(before.elapsed() < Duration::from_millis(100));
    assert_eq!(slots.metrics().active, 1);
    release.send(()).unwrap();
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn oversized_and_nonfinite_commands_reject_without_native_actions() {
    let control = Arc::new(Control::default());
    let (mut handle, slots, _) = spawn(control.clone(), None, None, pool(true, 64));
    let JoyConWorkerCommand::Vibrate(mut request) = command(0) else {
        unreachable!()
    };
    request.target = "x".repeat(257);
    assert!(handle.send(JoyConWorkerCommand::Vibrate(request.clone())).is_err());
    request.target = "both".into();
    request.frequency_hz = f32::NAN;
    assert!(handle.send(JoyConWorkerCommand::Vibrate(request)).is_err());
    assert!(control.actions.lock().unwrap().is_empty());
    handle.stop();
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn native_endpoints_are_disabled_in_test_builds() {
    assert!(native::Native::open().is_err());
}

#[test]
fn startup_failure_diagnostic_survives_native_termination_until_actor_retirement() {
    let slots = slots();
    let mut handle = JoyConRuntimeHandle::spawn_using(
        WorkerNotifier::new(|| {}),
        &slots,
        pool(false, 64),
        pool(true, 64),
        || Err::<Fake, String>("controlled setup failure".into()),
    )
    .unwrap();
    until(|| !handle.is_running());
    let event = handle.try_recv().unwrap();
    assert!(matches!(&event.event, JoyConWorkerEvent::Error(message) if message == "controlled setup failure"));
    assert!(event.guards.iter().all(RevisionGuard::is_current));
    handle.stop();
    assert!(!event.guards.iter().all(RevisionGuard::is_current));
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
}
