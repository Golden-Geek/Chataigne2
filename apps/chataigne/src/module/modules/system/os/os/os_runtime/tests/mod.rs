use super::*;
use golden_core::{
    node::{Folder, Node},
    script::ScriptMethodCall,
};
use std::{
    sync::{atomic::AtomicUsize, mpsc},
    time::{Duration, Instant},
};
mod engine;

type Gate = (mpsc::SyncSender<()>, mpsc::Receiver<()>);
#[derive(Clone, Default)]
struct Control {
    actions: Arc<Mutex<Vec<Request>>>,
    samples: Arc<AtomicUsize>,
    fail_sample: Arc<AtomicBool>,
    panic_sample: Arc<AtomicBool>,
    fail_execute: Arc<AtomicBool>,
}
struct Fake {
    control: Control,
    execute_gate: Option<Gate>,
    cleanup: Option<Gate>,
}
impl Provider for Fake {
    fn sample(&mut self) -> Result<OsMetricsSnapshot, String> {
        self.control.samples.fetch_add(1, Ordering::AcqRel);
        assert!(
            !self.control.panic_sample.load(Ordering::Acquire),
            "injected OS sample panic"
        );
        if self.control.fail_sample.load(Ordering::Acquire) {
            return Err("injected OS metrics failure".into());
        }
        Ok(OsMetricsSnapshot {
            architecture: "fake-arch".into(),
            os_version: "fixture".into(),
            global_cpu_ratio: 0.5,
            ..Default::default()
        })
    }
    fn execute(&mut self, request: &Request) -> Result<(), String> {
        if let Some((entered, blocked)) = self.execute_gate.take() {
            let _ = entered.send(());
            let _ = blocked.recv();
        }
        self.control.actions.lock().unwrap().push(request.clone());
        if self.control.fail_execute.load(Ordering::Acquire) {
            Err("injected OS native command failure".into())
        } else {
            Ok(())
        }
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
fn slots() -> RetirementPool {
    RetirementPool::new(NonZeroUsize::MIN)
}
fn payloads(items: usize) -> PayloadPool {
    PayloadPool::new("os-test", items, 16 * 1024 * 1024).unwrap()
}
fn until(mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !ready() {
        assert!(Instant::now() < deadline, "OS worker did not progress");
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn spawn(
    control: Control,
    slots: &RetirementPool,
    input: PayloadPool,
    output: PayloadPool,
    execute_gate: Option<Gate>,
    cleanup: Option<Gate>,
    wake: WorkerNotifier,
) -> OsRuntime {
    let runtime = OsRuntime::spawn_using(wake, slots, input, output, move || {
        Ok(Fake {
            control,
            execute_gate,
            cleanup,
        })
    })
    .unwrap();
    until(|| runtime.has_metrics() || !runtime.is_running());
    runtime
}
fn engine() -> crate::app::AppEngine {
    crate::app::AppEngine::new(Folder::new("source").into())
}
fn script(
    runtime: &OsRuntime,
    engine: &crate::app::AppEngine,
    method: &str,
    args: Vec<golden_core::parameter::ParamValue>,
) -> Result<Receipt, String> {
    let method = Method::parse(method).unwrap();
    let call = ScriptMethodCall::prepare(method.name(), args).unwrap();
    runtime.send(
        Input::Script {
            method,
            args: call.args().clone(),
        },
        engine.node_waker(engine.root).unwrap(),
        method,
    )
}

#[test]
fn blocked_setup_reserves_lifetime_and_cancelled_setup_cannot_execute_commands() {
    let slots = slots();
    let control = Control::default();
    let observed = control.clone();
    let (entered, entry) = mpsc::sync_channel(1);
    let (release, blocked) = mpsc::sync_channel(1);
    let mut runtime = OsRuntime::spawn_using(
        WorkerNotifier::new(|| {}),
        &slots,
        payloads(8),
        payloads(8),
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
    let receipt = script(&runtime, &engine, "shutdown", vec![]).unwrap();
    runtime.stop();
    assert!(OsRuntime::spawn_using(
        WorkerNotifier::new(|| {}),
        &slots,
        payloads(8),
        payloads(8),
        || -> Result<Fake, String> { panic!("slot released during opening") }
    )
    .is_err());
    release.send(()).unwrap();
    drop(runtime);
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
    assert!(receipt.complete());
    assert!(receipt.take().unwrap().is_err());
    assert!(observed.actions.lock().unwrap().is_empty());
    assert_eq!(observed.samples.load(Ordering::Acquire), 0);
}

#[test]
fn blocked_native_command_fills_64_slots_and_stop_cancels_every_accepted_suffix() {
    let slots = slots();
    let control = Control::default();
    let output = payloads(256);
    let (entered, entry) = mpsc::sync_channel(1);
    let (release, blocked) = mpsc::sync_channel(1);
    let wakes = Arc::new(AtomicUsize::new(0));
    let counter = wakes.clone();
    let mut runtime = spawn(
        control.clone(),
        &slots,
        payloads(8),
        output.clone(),
        Some((entered, blocked)),
        None,
        WorkerNotifier::new(move || {
            counter.fetch_add(1, Ordering::AcqRel);
        }),
    );
    let engine = engine();
    let first = script(&runtime, &engine, "shutdown", vec![]).unwrap();
    entry.recv_timeout(Duration::from_secs(1)).unwrap();
    let receipts = (0..64)
        .map(|_| script(&runtime, &engine, "reboot", vec![]).unwrap())
        .collect::<Vec<_>>();
    let before = wakes.load(Ordering::Acquire);
    assert!(script(&runtime, &engine, "logout", vec![]).is_err());
    assert_eq!(
        wakes.load(Ordering::Acquire),
        before,
        "unadmitted work cannot wake via cancellation"
    );
    runtime.stop();
    release.send(()).unwrap();
    drop(runtime);
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
    assert_eq!(control.actions.lock().unwrap().len(), 1);
    assert!(first.complete());
    assert!(first.take().unwrap().unwrap().attempted);
    assert!(receipts.iter().all(Receipt::complete));
    for receipt in receipts {
        assert!(receipt.take().unwrap().is_err());
    }
    drop(first);
    assert!(output.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn stopped_notification_and_fences_precede_blocked_native_cleanup() {
    let slots = slots();
    let control = Control::default();
    let (entered, entry) = mpsc::sync_channel(1);
    let (release, blocked) = mpsc::sync_channel(1);
    let mut runtime = spawn(
        control,
        &slots,
        payloads(8),
        payloads(8),
        None,
        Some((entered, blocked)),
        WorkerNotifier::new(|| {}),
    );
    let frame = runtime.take_metrics().unwrap();
    assert!(frame.fence.current());
    runtime.stop();
    entry.recv_timeout(Duration::from_secs(1)).unwrap();
    assert!(!runtime.is_running());
    assert!(!frame.fence.current());
    assert_eq!(slots.metrics().active, 1);
    assert!(slots.try_reserve().is_err());
    release.send(()).unwrap();
    drop(runtime);
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
    let mut recovered = spawn(
        Control::default(),
        &slots,
        payloads(8),
        payloads(8),
        None,
        None,
        WorkerNotifier::new(|| {}),
    );
    recovered.stop();
    drop(recovered);
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn fake_provider_preserves_all_four_methods_and_reports_native_failure_without_restarting() {
    use golden_core::parameter::ParamValue;
    let slots = slots();
    let control = Control::default();
    let mut runtime = spawn(
        control.clone(),
        &slots,
        payloads(8),
        payloads(64),
        None,
        None,
        WorkerNotifier::new(|| {}),
    );
    let engine = engine();
    let receipts = ["shutdown", "reboot", "logout"].map(|method| script(&runtime, &engine, method, vec![]).unwrap());
    let wol = script(
        &runtime,
        &engine,
        "wakeOnLan",
        vec![
            ParamValue::Str("AA:BB:CC:DD:EE:FF".into()),
            ParamValue::Str("fixture.invalid".into()),
            ParamValue::Int(9),
        ],
    )
    .unwrap();
    until(|| wol.complete());
    assert_eq!(control.actions.lock().unwrap().len(), 4);
    for receipt in receipts {
        let outcome = receipt.take().unwrap().unwrap();
        assert!(outcome.attempted && outcome.error.is_none() && outcome.requested.is_some());
    }
    assert!(wol.take().unwrap().unwrap().error.is_none());
    control.fail_execute.store(true, Ordering::Release);
    let failed = script(&runtime, &engine, "logout", vec![]).unwrap();
    until(|| failed.complete());
    let outcome = failed.take().unwrap().unwrap();
    assert!(outcome.attempted && outcome.requested.is_some() && outcome.failed.is_some());
    assert!(runtime.is_running());
    runtime.stop();
}

#[test]
fn retired_producer_is_rejected_before_native_dispatch() {
    let slots = slots();
    let control = Control::default();
    let (entered, entry) = mpsc::sync_channel(1);
    let (release, blocked) = mpsc::sync_channel(1);
    let mut runtime = spawn(
        control.clone(),
        &slots,
        payloads(8),
        payloads(32),
        Some((entered, blocked)),
        None,
        WorkerNotifier::new(|| {}),
    );
    let engine = engine();
    let first = script(&runtime, &engine, "shutdown", vec![]).unwrap();
    entry.recv_timeout(Duration::from_secs(1)).unwrap();
    let later = script(&runtime, &engine, "reboot", vec![]).unwrap();
    drop(engine);
    release.send(()).unwrap();
    until(|| later.complete());
    let outcome = later.take().unwrap().unwrap();
    assert!(!outcome.attempted);
    assert!(outcome.error.as_deref().unwrap().contains("not sent"));
    assert_eq!(control.actions.lock().unwrap().len(), 1);
    assert!(first.complete());
    runtime.stop();
}

#[test]
fn metrics_pressure_withholds_samples_without_losing_provider_and_recovers_quota() {
    let slots = slots();
    let control = Control::default();
    let input = payloads(1);
    let held = input.try_reserve(WEIGHT).unwrap();
    let mut runtime = OsRuntime::spawn_using(WorkerNotifier::new(|| {}), &slots, input.clone(), payloads(8), {
        let control = control.clone();
        move || {
            Ok(Fake {
                control,
                execute_gate: None,
                cleanup: None,
            })
        }
    })
    .unwrap();
    until(|| matches!(runtime.take_status(), Some(Status::Warning(_))));
    assert!(runtime.is_running());
    assert_eq!(control.samples.load(Ordering::Acquire), 0);
    drop(held);
    until(|| runtime.has_metrics());
    let metrics = runtime.take_metrics().unwrap();
    assert!(input.try_reserve(1).is_err());
    runtime.stop();
    assert!(!metrics.fence.current());
    drop(runtime);
    assert!(slots.wait_for_idle(Duration::from_secs(1)));
    assert!(!input.wait_for_idle(Duration::from_millis(5)));
    drop(metrics);
    assert!(input.wait_for_idle(Duration::from_secs(1)));
}

#[test]
fn callback_admission_failure_preserves_the_known_native_result_without_replay() {
    let slots = slots();
    let control = Control::default();
    let output = payloads(16);
    let mut runtime = spawn(
        control.clone(),
        &slots,
        payloads(8),
        output.clone(),
        None,
        None,
        WorkerNotifier::new(|| {}),
    );
    let engine = engine();
    let call = ScriptMethodCall::prepare("logout", vec![]).unwrap();
    let source = runtime
        .capture(
            Input::Script {
                method: Method::Logout,
                args: call.args().clone(),
            },
            engine.node_waker(engine.root).unwrap(),
            Method::Logout,
            golden_core::engine::EngineTime {
                tick: 0,
                micro: 0,
                seq: 0,
            },
            None,
        )
        .unwrap();
    let mut provider = Fake {
        control: control.clone(),
        execute_gate: None,
        cleanup: None,
    };
    let started = AtomicBool::new(false);
    let outcome = outcomes::execute_with(&mut provider, source, &output, &started, |_, _| {
        Err("injected callback quota pressure".into())
    })
    .unwrap();
    assert!(started.load(Ordering::Acquire));
    assert!(outcome.attempted && outcome.error.is_none());
    assert!(outcome.callback_error.is_some());
    assert!(outcome.requested.is_none());
    assert_eq!(control.actions.lock().unwrap().len(), 1);
    runtime.stop();
}

#[test]
fn failed_disposer_startup_can_retry_and_successful_initialization_keeps_one_quota_owner() {
    let cell = Mutex::new(None);
    assert!(initialize_pool(&cell, || Err("injected disposer startup failure".into())).is_err());
    let first = initialize_pool(&cell, || Ok(payloads(1))).unwrap();
    let second = initialize_pool(&cell, || panic!("live pool must not be initialized twice")).unwrap();
    let held = first.try_reserve(WEIGHT).unwrap();
    assert!(
        second.try_reserve(1).is_err(),
        "both accesses share the original admission owner"
    );
    drop(held);
    assert!(second.try_reserve(WEIGHT).is_ok());
}
