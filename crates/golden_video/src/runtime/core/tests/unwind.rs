use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc;

use super::*;

struct PanickingSubmit {
    inner: NullBackend,
    entered: mpsc::SyncSender<CompletionToken>,
}

impl Backend for PanickingSubmit {
    type Resources = <NullBackend as Backend>::Resources;

    fn initialize(&mut self) -> Result<(), BackendError> {
        self.inner.initialize()
    }

    fn resource_bytes(&self, plan: &CompiledPlan) -> Result<u64, BackendError> {
        self.inner.resource_bytes(plan)
    }

    fn prepare(&mut self, plan: &CompiledPlan, device: DeviceGeneration) -> Result<Self::Resources, BackendError> {
        self.inner.prepare(plan, device)
    }

    fn submit(
        &mut self,
        resources: &Self::Resources,
        plan: &CompiledPlan,
        token: CompletionToken,
        time: TimeSnapshot,
    ) -> Result<(), BackendError> {
        self.inner.submit(resources, plan, token, time)?;
        self.entered.send(token).unwrap();
        panic!("injected unwind after establishing GPU use");
    }

    fn completion(&mut self) -> Result<CompletionToken, BackendError> {
        self.inner.completion()
    }

    fn wait_for_completion(&mut self, timeout: Duration) -> Result<(), BackendError> {
        self.inner.wait_for_completion(timeout)
    }

    fn reset(&mut self, device: DeviceGeneration) -> Result<(), BackendError> {
        self.inner.reset(device)
    }
}

struct LossOnExit(NullControl);

impl Drop for LossOnExit {
    fn drop(&mut self) {
        self.0.lose_device();
    }
}

#[test]
fn unwinding_submission_keeps_its_generation_until_the_claimed_use_completes() {
    let control = NullControl::new();
    let _loss_guard = LossOnExit(control.clone());
    let worker_control = control.clone();
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (done_tx, done_rx) = mpsc::sync_channel(1);
    let worker = std::thread::spawn(move || {
        let runtime = Uuid::new_v4();
        let device = DeviceGeneration::new(runtime, 0);
        let backend = PanickingSubmit {
            inner: NullBackend::new(
                NullConfig {
                    auto_complete: false,
                    ..Default::default()
                },
                device,
                worker_control,
            )
            .unwrap(),
            entered: entered_tx,
        };
        let mut core = RenderCore::admit(
            backend,
            device,
            ResourceLimits {
                max_bytes: 64,
                max_in_flight: NonZeroUsize::new(2).unwrap(),
                max_retiring: 2,
            },
        )
        .unwrap();
        core.initialize().unwrap();
        let prepared = core
            .prepare(
                PlanIdentity {
                    project: ProjectEpoch::new(runtime, 0),
                    revision: PlanRevision::new(1),
                },
                plan(1),
            )
            .unwrap();
        core.install(prepared).unwrap();
        assert!(catch_unwind(AssertUnwindSafe(|| core.submit(time()))).is_err());
        drop(core);
        done_tx.send(()).unwrap();
    });
    let token = entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let ended_early = done_rx.recv_timeout(Duration::from_millis(20)).is_ok();
    let live_before_completion = control.snapshot().live;
    assert!(control.complete_through(token));
    if !ended_early {
        done_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    }
    worker.join().unwrap();
    assert!(!ended_early, "unwind must not bypass the completion drain");
    assert_eq!(live_before_completion, 1);
    assert_eq!(control.snapshot().live, 0);
}
