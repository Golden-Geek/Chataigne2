use std::cell::Cell;
use std::rc::Weak;

use super::*;

#[derive(Default)]
struct Observations {
    initialized: Cell<usize>,
    dropped: Cell<usize>,
    charged_at_drop: Cell<u64>,
    drop_thread: Cell<Option<std::thread::ThreadId>>,
    budget: RefCell<Option<Weak<RefCell<BudgetState>>>>,
}

struct BaselineResource(Rc<Observations>);

impl Drop for BaselineResource {
    fn drop(&mut self) {
        self.0.dropped.set(self.0.dropped.get() + 1);
        self.0.drop_thread.set(Some(std::thread::current().id()));
        if let Some(budget) = self.0.budget.borrow().as_ref().and_then(Weak::upgrade) {
            self.0.charged_at_drop.set(budget.borrow().used);
        }
    }
}

struct BaselineBackend {
    null: NullBackend,
    bytes: u64,
    observations: Rc<Observations>,
    baseline: Option<BaselineResource>,
    fail_initialize: bool,
    fail_next_reset: bool,
}

impl BaselineBackend {
    fn allocate_baseline(&mut self) {
        self.observations
            .initialized
            .set(self.observations.initialized.get() + 1);
        self.baseline = Some(BaselineResource(self.observations.clone()));
    }
}

impl Backend for BaselineBackend {
    type Resources = <NullBackend as Backend>::Resources;

    fn base_resource_bytes(&self) -> Result<u64, BackendError> {
        Ok(self.bytes)
    }

    fn initialize(&mut self) -> Result<(), BackendError> {
        self.allocate_baseline();
        let result = if self.fail_initialize {
            Err(BackendError::AllocationFailed)
        } else {
            self.null.initialize()
        };
        if result.is_err() {
            drop(self.baseline.take());
        }
        result
    }

    fn resource_bytes(&self, plan: &CompiledPlan) -> Result<u64, BackendError> {
        self.null.resource_bytes(plan)
    }

    fn prepare(&mut self, plan: &CompiledPlan, generation: GenerationInfo) -> Result<Self::Resources, BackendError> {
        self.null.prepare(plan, generation)
    }

    fn submit(
        &mut self,
        resources: &Self::Resources,
        plan: &CompiledPlan,
        token: CompletionToken,
        time: TimeSnapshot,
    ) -> Result<(), BackendError> {
        self.null.submit(resources, plan, token, time)
    }

    fn completion(&mut self) -> Result<CompletionToken, BackendError> {
        self.null.completion()
    }

    fn wait_for_completion(&mut self, timeout: Duration) -> Result<(), BackendError> {
        self.null.wait_for_completion(timeout)
    }

    fn reset(&mut self, device: DeviceGeneration) -> Result<(), BackendError> {
        drop(self.baseline.take());
        if std::mem::take(&mut self.fail_next_reset) {
            return Err(BackendError::AllocationFailed);
        }
        self.null.reset(device)?;
        self.allocate_baseline();
        Ok(())
    }
}

fn make_backend(bytes: u64, fail_initialize: bool) -> (BaselineBackend, NullControl, Rc<Observations>, Uuid) {
    let runtime = Uuid::new_v4();
    let device = DeviceGeneration::new(runtime, 0);
    let control = NullControl::new();
    let observations = Rc::new(Observations::default());
    let backend = BaselineBackend {
        null: NullBackend::new(NullConfig::default(), device, control.clone()).unwrap(),
        bytes,
        observations: observations.clone(),
        baseline: None,
        fail_initialize,
        fail_next_reset: false,
    };
    (backend, control, observations, runtime)
}

fn limits(max_bytes: u64) -> ResourceLimits {
    ResourceLimits {
        max_bytes,
        max_in_flight: NonZeroUsize::new(2).unwrap(),
        max_retiring: 2,
    }
}

fn identity(runtime: Uuid, revision: u64) -> PlanIdentity {
    PlanIdentity {
        project: ProjectEpoch::new(runtime, 0),
        revision: PlanRevision::new(revision),
    }
}

#[test]
fn baseline_rejection_precedes_initialization_and_partial_initialization_is_disposed() {
    let (backend, _, observations, runtime) = make_backend(9, false);
    let result = RenderCore::admit(backend, DeviceGeneration::new(runtime, 0), limits(8));
    assert!(matches!(
        result,
        Err(CoreError::ResourceBudgetExceeded {
            requested: 9,
            used: 0,
            limit: 8,
        })
    ));
    assert_eq!(observations.initialized.get(), 0);
    assert_eq!(observations.dropped.get(), 0);

    let (backend, _, observations, runtime) = make_backend(8, true);
    let mut core = RenderCore::admit(backend, DeviceGeneration::new(runtime, 0), limits(8)).unwrap();
    let result = core.initialize();
    assert!(matches!(
        result,
        Err(CoreError::Backend(BackendError::AllocationFailed))
    ));
    assert_eq!(observations.initialized.get(), 1);
    assert_eq!(observations.dropped.get(), 1);
    assert_eq!(observations.drop_thread.get(), Some(std::thread::current().id()));
}

#[test]
fn pending_initialization_retains_admission_blocks_use_and_retries_the_original_device() {
    let (backend, control, observations, runtime) = make_backend(8, true);
    let device = DeviceGeneration::new(runtime, 0);
    let mut core = RenderCore::admit(backend, device, limits(12)).unwrap();
    *observations.budget.borrow_mut() = Some(Rc::downgrade(&core.budget));
    let budget = core.budget.clone();
    let initial = core.snapshot();
    assert_eq!(initial.bytes_used, 8);
    assert!(!initial.initialized && !initial.device_lost);
    assert_eq!(
        observations.initialized.get(),
        0,
        "admission creates no baseline objects"
    );
    assert!(matches!(
        core.prepare(identity(runtime, 1), plan(1)),
        Err(CoreError::NotInitialized)
    ));
    assert_eq!(core.submit(time()), Err(CoreError::NotInitialized));
    assert_eq!(core.poll_completion(), Err(CoreError::NotInitialized));
    assert_eq!(
        core.complete(CompletionToken { device, serial: 0 }),
        Err(CoreError::NotInitialized)
    );
    assert_eq!(core.wait_for_completion(Duration::ZERO), Err(CoreError::NotInitialized));
    assert!(matches!(
        core.reset_after_loss(device.next().unwrap()),
        Err(CoreError::DeviceNotLost)
    ));
    assert_eq!(control.snapshot().created, 0);

    assert_eq!(
        core.initialize(),
        Err(CoreError::Backend(BackendError::AllocationFailed))
    );
    assert_eq!(
        core.snapshot(),
        initial,
        "failed first allocation is not a published device loss"
    );
    assert_eq!(observations.dropped.get(), 1);
    assert_eq!(observations.charged_at_drop.get(), 8);
    core.backend.fail_initialize = false;
    core.initialize().unwrap();
    assert!(core.snapshot().initialized && !core.snapshot().device_lost);
    assert_eq!(core.snapshot().last_submitted, initial.last_submitted);
    assert_eq!(core.snapshot().bytes_used, 8);
    core.initialize().unwrap();
    assert_eq!(
        observations.initialized.get(),
        2,
        "successful initialization is idempotent"
    );
    drop(core.prepare(identity(runtime, 1), plan(1)).unwrap());
    assert_eq!(control.snapshot().created, 1);
    assert_eq!(core.snapshot().bytes_used, 8);
    drop(core);
    assert_eq!(observations.dropped.get(), 2);
    assert_eq!(observations.charged_at_drop.get(), 8);
    assert_eq!(budget.borrow().used, 0);
}

#[test]
fn baseline_remains_visible_without_a_plan_and_reduces_generation_capacity() {
    let (backend, control, observations, runtime) = make_backend(8, false);
    let mut core = RenderCore::admit(backend, DeviceGeneration::new(runtime, 0), limits(12)).unwrap();
    core.initialize().unwrap();
    *observations.budget.borrow_mut() = Some(Rc::downgrade(&core.budget));
    assert_eq!(core.snapshot().bytes_used, 8);
    assert_eq!(core.snapshot().peak_bytes, 8);
    assert_eq!(core.snapshot().prepared_generations, 0);

    let prepared = core.prepare(identity(runtime, 1), plan(1)).unwrap();
    assert_eq!(core.snapshot().bytes_used, 12);
    core.install(prepared).unwrap();
    assert!(matches!(
        core.prepare(identity(runtime, 2), plan(1)),
        Err(CoreError::ResourceBudgetExceeded {
            requested: 4,
            used: 12,
            limit: 12,
        })
    ));
    assert_eq!(control.snapshot().created, 1);
    assert_eq!(core.snapshot().active.unwrap().identity, identity(runtime, 1));
    core.clear_live().unwrap();
    assert_eq!(core.snapshot().bytes_used, 8);
    assert_eq!(control.snapshot().live, 0);

    let budget = core.budget.clone();
    drop(core);
    assert_eq!(observations.charged_at_drop.get(), 8);
    assert_eq!(observations.dropped.get(), 1);
    assert_eq!(budget.borrow().used, 0);
}

#[test]
fn baseline_stays_reserved_through_loss_failed_reset_and_reconstruction() {
    let (backend, control, observations, runtime) = make_backend(8, false);
    let device = DeviceGeneration::new(runtime, 0);
    let mut core = RenderCore::admit(backend, device, limits(12)).unwrap();
    core.initialize().unwrap();
    *observations.budget.borrow_mut() = Some(Rc::downgrade(&core.budget));
    let prepared = core.prepare(identity(runtime, 1), plan(1)).unwrap();
    core.install(prepared).unwrap();
    core.submit(time()).unwrap();
    control.lose_device();
    assert_eq!(
        core.poll_completion(),
        Err(CoreError::Backend(BackendError::DeviceLost))
    );
    core.lose_device().unwrap();
    assert_eq!(core.snapshot().bytes_used, 8);
    assert_eq!(control.snapshot().live, 0);

    let next = device.next().unwrap();
    core.backend.fail_next_reset = true;
    assert!(matches!(
        core.reset_after_loss(next),
        Err(CoreError::Backend(BackendError::AllocationFailed))
    ));
    assert_eq!(core.snapshot().bytes_used, 8);
    assert!(core.snapshot().device_lost);
    assert_eq!(observations.charged_at_drop.get(), 8);
    assert_eq!(observations.dropped.get(), 1);

    let (rebuild_identity, rebuild_plan) = core.reset_after_loss(next).unwrap().unwrap();
    assert_eq!(rebuild_identity, identity(runtime, 1));
    assert_eq!(core.snapshot().bytes_used, 8);
    assert_eq!(observations.initialized.get(), 2);
    assert!(!core.snapshot().device_lost);
    let prepared = core.prepare(rebuild_identity, rebuild_plan).unwrap();
    core.install(prepared).unwrap();
    assert_eq!(core.snapshot().bytes_used, 12);
    core.clear_live().unwrap();
    assert_eq!(core.snapshot().bytes_used, 8);
    assert_eq!(core.snapshot().peak_bytes, 12);
}
