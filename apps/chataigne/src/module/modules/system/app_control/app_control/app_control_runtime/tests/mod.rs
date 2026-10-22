use super::*;
use golden_core::{node::Folder, parameter::ParamValue, script::ScriptMethodCall};
use std::{
    sync::{atomic::AtomicUsize, mpsc},
    time::{Duration, Instant},
};

type Gate = (mpsc::SyncSender<()>, mpsc::Receiver<()>);
#[derive(Clone, Default)]
struct Control {
    actions: Arc<Mutex<Vec<Request>>>,
    samples: Arc<AtomicUsize>,
    panic_sample: Arc<AtomicBool>,
}
struct Fake {
    control: Control,
    execute_gate: Option<Gate>,
    cleanup: Option<Gate>,
}
impl Provider for Fake {
    fn sample(&mut self, config: &Config) -> Result<Observation, String> {
        self.control.samples.fetch_add(1, Ordering::AcqRel);
        assert!(
            !self.control.panic_sample.load(Ordering::Acquire),
            "injected App Control sample panic"
        );
        Ok(Observation {
            apps: config
                .apps
                .iter()
                .map(|entry| AppObservation {
                    metrics: WatchedAppMetrics::idle(&entry.target_path),
                    kind: if entry.target_path.is_empty() {
                        PathKind::Empty
                    } else {
                        PathKind::Missing
                    },
                })
                .collect(),
            folders: config
                .folders
                .iter()
                .map(|entry| FolderObservation {
                    update: FolderWatchUpdate {
                        path: entry.target_path.clone(),
                        ..Default::default()
                    },
                    kind: PathKind::Empty,
                })
                .collect(),
        })
    }
    fn execute(&mut self, request: &Request) -> Result<Execution, String> {
        if let Some((entered, blocked)) = self.execute_gate.take() {
            let _ = entered.send(());
            let _ = blocked.recv();
        }
        self.control.actions.lock().unwrap().push(request.clone());
        Ok(Execution {
            attempted: true,
            method: "launch",
            details: serde_json::json!({"fixture":true}),
        })
    }
}
impl Drop for Fake {
    fn drop(&mut self) {
        if let Some((entered, blocked)) = self.cleanup.take() {
            let _ = entered.send(());
            let _ = blocked.recv();
        }
    }
}
fn payloads(items: usize) -> PayloadPool {
    PayloadPool::new("app-control-test", items, 64 * 1024 * 1024).unwrap()
}
fn slots() -> RetirementPool {
    RetirementPool::new(NonZeroUsize::MIN)
}
fn until(mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !ready() {
        assert!(Instant::now() < deadline, "App Control worker did not progress");
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn spawn(
    control: Control,
    slots: &RetirementPool,
    commands: PayloadPool,
    execute_gate: Option<Gate>,
    cleanup: Option<Gate>,
) -> AppControlRuntime {
    AppControlRuntime::spawn_using(WorkerNotifier::new(|| {}), slots, payloads(32), commands, move || {
        Ok(Fake {
            control,
            execute_gate,
            cleanup,
        })
    })
    .unwrap()
}
pub(in super::super) fn fixture(wake: WorkerNotifier) -> AppControlRuntime {
    AppControlRuntime::spawn_using(
        wake,
        &RetirementPool::new(NonZeroUsize::new(32).unwrap()),
        payloads(32),
        payloads(256),
        || {
            Ok(Fake {
                control: Control::default(),
                execute_gate: None,
                cleanup: None,
            })
        },
    )
    .unwrap()
}
fn engine() -> crate::app::AppEngine {
    crate::app::AppEngine::new(Folder::new("source").into())
}
fn source(
    runtime: &AppControlRuntime,
    engine: &crate::app::AppEngine,
    admission: &RetirementPool,
) -> RetainedPayload<Source> {
    source_at(runtime, engine, admission, &engine.process_tree_snapshot())
}
fn source_at(
    runtime: &AppControlRuntime,
    engine: &crate::app::AppEngine,
    admission: &RetirementPool,
    snapshot: &Arc<golden_core::process_ctx::ProcessTreeSnapshot>,
) -> RetainedPayload<Source> {
    let args = ScriptMethodCall::prepare("launchApp", vec![ParamValue::Str("fixture-only.exe".into())]).unwrap();
    runtime
        .capture(
            Input::Script {
                module: engine.root,
                method: "launchApp",
                args: args.args().clone(),
                snapshot: snapshot.try_lease().unwrap(),
            },
            engine.node_waker(engine.root).unwrap(),
            golden_core::engine::EngineTime {
                tick: 0,
                micro: 0,
                seq: 0,
            },
            admission.try_reserve().unwrap(),
        )
        .unwrap()
}
#[test]
fn opening_and_retiring_workers_keep_their_slot_without_blocking_the_actor() {
    let slots = slots();
    let control = Control::default();
    let observed = control.clone();
    let (entered, entry) = mpsc::sync_channel(1);
    let (release, blocked) = mpsc::sync_channel(1);
    let mut runtime = AppControlRuntime::spawn_using(
        WorkerNotifier::new(|| {}),
        &slots,
        payloads(32),
        payloads(256),
        move || {
            entered.send(()).unwrap();
            blocked.recv().unwrap();
            Ok(Fake {
                control,
                execute_gate: None,
                cleanup: None,
            })
        },
    )
    .unwrap();
    entry.recv_timeout(Duration::from_secs(1)).unwrap();
    let engine = engine();
    let admission = RetirementPool::new(NonZeroUsize::new(128).unwrap());
    let receipt = runtime.submit(source(&runtime, &engine, &admission)).ok().unwrap();
    let before = Instant::now();
    runtime.stop();
    assert!(before.elapsed() < Duration::from_millis(100));
    assert!(AppControlRuntime::spawn_using(
        WorkerNotifier::new(|| {}),
        &slots,
        payloads(1),
        payloads(1),
        || -> Result<Fake, String> { panic!("opening slot was released") }
    )
    .is_err());
    release.send(()).unwrap();
    drop(runtime);
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
    assert!(receipt.complete());
    assert!(receipt.take().unwrap().is_err());
    assert!(observed.actions.lock().unwrap().is_empty());
}
#[test]
fn native_command_pressure_preserves_first_effect_and_cancels_all_64_queued_sources() {
    let slots = slots();
    let control = Control::default();
    let commands = payloads(256);
    let (entered, entry) = mpsc::sync_channel(1);
    let (release, blocked) = mpsc::sync_channel(1);
    let mut runtime = spawn(control.clone(), &slots, commands, Some((entered, blocked)), None);
    let engine = engine();
    let snapshot = engine.process_tree_snapshot();
    let admission = RetirementPool::new(NonZeroUsize::new(128).unwrap());
    let first = runtime
        .submit(source_at(&runtime, &engine, &admission, &snapshot))
        .ok()
        .unwrap();
    entry.recv_timeout(Duration::from_secs(1)).unwrap();
    let receipts = (0..64)
        .map(|_| {
            runtime
                .submit(source_at(&runtime, &engine, &admission, &snapshot))
                .ok()
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert!(runtime
        .submit(source_at(&runtime, &engine, &admission, &snapshot))
        .is_err());
    runtime.stop();
    release.send(()).unwrap();
    drop(runtime);
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
    assert_eq!(control.actions.lock().unwrap().len(), 1);
    assert!(first.take().unwrap().unwrap().attempted);
    assert!(receipts.iter().all(Receipt::complete));
    for receipt in receipts {
        assert!(receipt.take().unwrap().is_err());
    }
}
#[test]
fn report_generation_changes_cancel_old_captured_actions_before_execution() {
    let slots = slots();
    let control = Control::default();
    let mut runtime = spawn(control.clone(), &slots, payloads(32), None, None);
    let engine = engine();
    let admission = RetirementPool::new(NonZeroUsize::MIN);
    let captured = source(&runtime, &engine, &admission);
    runtime
        .configure(Config {
            apps: vec![],
            folders: vec![],
        })
        .unwrap();
    let receipt = runtime.submit(captured).ok().unwrap();
    until(|| receipt.complete());
    let outcome = receipt.take().unwrap().unwrap();
    assert!(!outcome.attempted);
    assert!(outcome.error.as_ref().unwrap().contains("retired"));
    assert!(control.actions.lock().unwrap().is_empty());
}
#[test]
fn completed_outcomes_keep_source_admission_until_the_payload_is_disposed() {
    let slots = slots();
    let runtime = spawn(Control::default(), &slots, payloads(32), None, None);
    let engine = engine();
    let admission = RetirementPool::new(NonZeroUsize::MIN);
    let receipt = runtime.submit(source(&runtime, &engine, &admission)).ok().unwrap();
    until(|| receipt.complete());
    let outcome = receipt.take().unwrap().unwrap();
    assert!(admission.try_reserve().is_err());
    drop(outcome);
    assert!(admission.wait_for_idle(Duration::from_secs(1)));
}
#[test]
fn panic_is_observable_before_blocked_provider_cleanup_and_slot_is_retained() {
    let slots = slots();
    let control = Control::default();
    control.panic_sample.store(true, Ordering::Release);
    let (entered, entry) = mpsc::sync_channel(1);
    let (release, blocked) = mpsc::sync_channel(1);
    let mut runtime = spawn(control, &slots, payloads(32), None, Some((entered, blocked)));
    runtime
        .configure(Config {
            apps: vec![],
            folders: vec![],
        })
        .unwrap();
    entry.recv_timeout(Duration::from_secs(1)).unwrap();
    assert!(!runtime.running());
    assert!(runtime.pending());
    assert!(AppControlRuntime::spawn_using(
        WorkerNotifier::new(|| {}),
        &slots,
        payloads(1),
        payloads(1),
        || -> Result<Fake, String> { panic!("retirement released early") }
    )
    .is_err());
    release.send(()).unwrap();
    drop(runtime);
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
}
#[test]
fn input_capacity_failure_never_reaches_provider() {
    let slots = slots();
    let control = Control::default();
    let runtime = spawn(control.clone(), &slots, payloads(1), None, None);
    let engine = engine();
    let admission = RetirementPool::new(NonZeroUsize::MIN);
    let captured = source(&runtime, &engine, &admission);
    let receipt = runtime.submit(captured).ok().unwrap();
    until(|| receipt.complete());
    assert!(receipt.take().unwrap().err().unwrap().contains("outcome capacity"));
    assert!(control.actions.lock().unwrap().is_empty());
}

#[test]
fn initial_empty_report_retries_quota_pressure_and_then_stops_sampling() {
    let slots = slots();
    let control = Control::default();
    let observed = control.clone();
    let reports = payloads(1);
    let held = reports.try_retain((), REPORT_WEIGHT).unwrap();
    let mut runtime =
        AppControlRuntime::spawn_using(WorkerNotifier::new(|| {}), &slots, reports, payloads(32), move || {
            Ok(Fake {
                control,
                execute_gate: None,
                cleanup: None,
            })
        })
        .unwrap();
    runtime
        .configure(Config {
            apps: vec![],
            folders: vec![],
        })
        .unwrap();
    until(|| matches!(runtime.event(), Some(Event::Warning(_))));
    assert_eq!(observed.samples.load(Ordering::Acquire), 0);
    drop(held);
    until(|| observed.samples.load(Ordering::Acquire) == 1);
    until(|| matches!(runtime.event(), Some(Event::Report(_))));
    std::thread::sleep(Duration::from_millis(550));
    assert_eq!(observed.samples.load(Ordering::Acquire), 1);
}
