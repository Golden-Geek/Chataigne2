use super::*;
use crate::plan::{PlanLimits, PlanOutput, RenderOperation, RenderPass, RenderPlan};
use crate::runtime::identity::{PlanRevision, ProjectEpoch};
use crate::runtime::null::{NullBackend, NullConfig, NullControl};
use crate::{
    AlphaMode, ColorEncoding, Extent2D, LinearColor, OutputId, PassId, PixelFormat, TextureDescriptor, TextureId,
};
use golden_time::{PlaybackRate, ShowPosition, SystemClock, TimeAuthority};
use uuid::Uuid;

struct Fixture {
    core: RenderCore<NullBackend>,
    control: NullControl,
    runtime: Uuid,
}

impl Fixture {
    fn new(max_bytes: u64, max_retiring: usize) -> Self {
        let runtime = Uuid::new_v4();
        let device = DeviceGeneration::new(runtime, 0);
        let control = NullControl::new();
        let backend = NullBackend::new(
            NullConfig {
                auto_complete: false,
                ..Default::default()
            },
            device,
            control.clone(),
        )
        .unwrap();
        let core = RenderCore::new(
            backend,
            device,
            ResourceLimits {
                max_bytes,
                max_in_flight: NonZeroUsize::new(2).unwrap(),
                max_retiring,
            },
        )
        .unwrap();
        Self { core, control, runtime }
    }

    fn identity(&self, revision: u64) -> PlanIdentity {
        PlanIdentity {
            project: ProjectEpoch::new(self.runtime, 0),
            revision: PlanRevision::new(revision),
        }
    }

    fn install(&mut self, revision: u64) -> GenerationInfo {
        let prepared = self.core.prepare(self.identity(revision), plan(1)).unwrap();
        self.core.install(prepared).unwrap()
    }

    fn submit(&mut self) -> CompletionToken {
        self.core.submit(time()).unwrap().unwrap().token
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // A failed assertion must not leave a manual fake GPU blocked in the safety drain.
        self.control.lose_device();
        let _ = self.core.poll_completion();
    }
}

fn time() -> TimeSnapshot {
    TimeAuthority::new(SystemClock::new(), ShowPosition::from_nanos(0), PlaybackRate::NORMAL)
        .unwrap()
        .sample()
        .unwrap()
}

fn plan(width: u32) -> CompiledPlan {
    RenderPlan {
        passes: vec![RenderPass {
            id: PassId::new(1).unwrap(),
            output: TextureId::new(1).unwrap(),
            descriptor: TextureDescriptor {
                extent: Extent2D::new(width, 1).unwrap(),
                format: PixelFormat::Rgba8Unorm,
                color_encoding: ColorEncoding::Linear,
                alpha_mode: AlphaMode::Premultiplied,
            },
            operation: RenderOperation::Clear(LinearColor::TRANSPARENT),
        }],
        outputs: vec![PlanOutput {
            id: OutputId::new(1).unwrap(),
            source: TextureId::new(1).unwrap(),
        }],
        ..Default::default()
    }
    .compile(&PlanLimits {
        max_passes: 2,
        max_external_textures: 2,
        max_outputs: 2,
        max_dependencies: 4,
        max_texture_dimension: 1024,
        max_texture_bytes: 4096,
    })
    .unwrap()
}

fn two_output_plan() -> CompiledPlan {
    let descriptor = plan(1).passes()[0].descriptor;
    RenderPlan {
        passes: (1..=2)
            .map(|id| RenderPass {
                id: PassId::new(id).unwrap(),
                output: TextureId::new(id).unwrap(),
                descriptor,
                operation: RenderOperation::Clear(LinearColor::TRANSPARENT),
            })
            .collect(),
        outputs: (1..=2)
            .map(|id| PlanOutput {
                id: OutputId::new(id).unwrap(),
                source: TextureId::new(id).unwrap(),
            })
            .collect(),
        ..Default::default()
    }
    .compile(&PlanLimits {
        max_passes: 2,
        max_external_textures: 2,
        max_outputs: 2,
        max_dependencies: 4,
        max_texture_dimension: 1024,
        max_texture_bytes: 4096,
    })
    .unwrap()
}

#[test]
fn budget_rejection_precedes_allocation_and_failed_prepare_preserves_the_live_generation() {
    let mut fixture = Fixture::new(12, 2);
    let original = fixture.install(1);
    let created = fixture.control.snapshot().created;
    assert!(matches!(
        fixture.core.prepare(fixture.identity(2), plan(3)),
        Err(CoreError::ResourceBudgetExceeded { .. })
    ));
    assert_eq!(fixture.control.snapshot().created, created);
    fixture.control.fail_next_prepare_after(1);
    assert!(matches!(
        fixture.core.prepare(fixture.identity(2), two_output_plan()),
        Err(CoreError::Backend(BackendError::AllocationFailed))
    ));
    let snapshot = fixture.core.snapshot();
    assert_eq!(snapshot.active, Some(original));
    assert_eq!(snapshot.bytes_used, 4);
    assert_eq!(snapshot.prepared_generations, 0);
    assert_eq!(fixture.control.snapshot().live_bytes, 4);
    assert_eq!(fixture.control.snapshot().created, created + 1);
    assert_eq!(fixture.control.snapshot().dropped, 1);
}

#[test]
fn prepared_live_and_two_retiring_generations_all_remain_charged_until_last_use() {
    let mut fixture = Fixture::new(16, 2);
    fixture.install(1);
    let first = fixture.submit();
    fixture.install(2);
    let second = fixture.submit();
    fixture.install(3);
    let prepared = fixture.core.prepare(fixture.identity(4), plan(1)).unwrap();
    let snapshot = fixture.core.snapshot();
    assert_eq!((snapshot.bytes_used, snapshot.peak_bytes), (16, 16));
    assert_eq!(
        (
            snapshot.prepared_generations,
            snapshot.retiring_generations,
            snapshot.in_flight
        ),
        (1, 2, 2)
    );
    assert_eq!(fixture.control.snapshot().live_bytes, 16);
    assert!(matches!(
        fixture.core.prepare(fixture.identity(5), plan(1)),
        Err(CoreError::PreparationBusy)
    ));
    drop(prepared);
    assert_eq!(fixture.core.snapshot().bytes_used, 12);
    fixture.control.complete_through(first);
    fixture.core.poll_completion().unwrap();
    assert_eq!(fixture.core.snapshot().bytes_used, 8);
    fixture.control.complete_through(second);
    fixture.core.poll_completion().unwrap();
    assert_eq!(fixture.core.snapshot().bytes_used, 4);
    fixture.core.clear_live().unwrap();
    assert_eq!(fixture.core.snapshot().bytes_used, 0);
    assert_eq!(fixture.control.snapshot().live, 0);
    assert_eq!(
        fixture.control.snapshot().last_drop_thread,
        Some(std::thread::current().id())
    );
}

#[test]
fn full_in_flight_window_and_failed_submission_do_not_create_untracked_uses() {
    let mut fixture = Fixture::new(32, 2);
    fixture.install(1);
    let first = fixture.submit();
    let second = fixture.submit();
    assert_eq!(fixture.core.submit(time()), Err(CoreError::InFlightFull));
    fixture.control.complete_through(first);
    fixture.core.poll_completion().unwrap();
    fixture.control.fail_next_submit();
    assert_eq!(
        fixture.core.submit(time()),
        Err(CoreError::Backend(BackendError::SubmissionFailed))
    );
    assert_eq!(fixture.core.snapshot().last_submitted, second);
    assert_eq!(fixture.core.snapshot().in_flight, 1);
    let third = fixture.submit();
    assert_eq!(third.serial, 3);
    fixture.control.complete_through(third);
    fixture.core.poll_completion().unwrap();
    assert!(!fixture.core.has_pending_uses());
}

#[test]
fn retirement_pressure_preserves_both_candidate_and_live_until_capacity_returns() {
    let mut fixture = Fixture::new(32, 0);
    let original = fixture.install(1);
    let use_token = fixture.submit();
    let prepared = fixture.core.prepare(fixture.identity(2), plan(1)).unwrap();
    let candidate = prepared.info();
    let error = fixture.core.install(prepared).unwrap_err();
    assert_eq!(error.reason(), InstallFailure::RetirementFull);
    assert_eq!(fixture.core.snapshot().active, Some(original));
    assert_eq!(fixture.core.snapshot().bytes_used, 8);
    assert_eq!(fixture.core.clear_live(), Err(CoreError::RetirementFull));
    let prepared = error.into_prepared();
    assert_eq!(prepared.info(), candidate);
    fixture.control.complete_through(use_token);
    fixture.core.poll_completion().unwrap();
    assert_eq!(fixture.core.install(prepared).unwrap(), candidate);
    assert_eq!(fixture.core.snapshot().bytes_used, 4);
}

#[test]
fn foreign_future_and_old_completions_cannot_release_a_generation() {
    let mut fixture = Fixture::new(32, 2);
    fixture.install(1);
    let token = fixture.submit();
    fixture.install(2);
    let foreign = CompletionToken {
        device: DeviceGeneration::new(Uuid::new_v4(), 0),
        serial: token.serial,
    };
    assert!(!fixture.core.complete(foreign).unwrap());
    assert!(matches!(
        fixture.core.complete(CompletionToken { serial: 2, ..token }),
        Err(CoreError::FutureCompletion { .. })
    ));
    assert_eq!(fixture.core.snapshot().bytes_used, 8);
    assert_eq!(fixture.core.snapshot().retiring_generations, 1);
    fixture.control.complete_through(token);
    fixture.core.poll_completion().unwrap();
    assert_eq!(fixture.core.snapshot().bytes_used, 4);
    assert!(!fixture.core.complete(CompletionToken { serial: 0, ..token }).unwrap());
}

#[test]
fn device_recreation_requires_loss_and_fences_old_serials_from_new_resources() {
    let mut fixture = Fixture::new(32, 2);
    let original = fixture.install(1);
    let old = fixture.submit();
    let next = old.device.next().unwrap();
    assert_eq!(
        fixture.core.reset_after_loss(next).unwrap_err(),
        CoreError::DeviceNotLost
    );
    fixture.control.lose_device();
    assert_eq!(
        fixture.core.poll_completion(),
        Err(CoreError::Backend(BackendError::DeviceLost))
    );
    let (identity, plan) = fixture.core.reset_after_loss(next).unwrap().unwrap();
    assert_eq!(identity, original.identity);
    assert_eq!(fixture.core.snapshot().bytes_used, 0);
    assert_eq!(fixture.control.snapshot().live, 0);
    let prepared = fixture.core.prepare(identity, plan).unwrap();
    assert!(prepared.info().generation > original.generation);
    fixture.core.install(prepared).unwrap();
    let new = fixture.submit();
    assert_eq!(new.serial, old.serial);
    assert_ne!(new.device, old.device);
    assert!(
        !fixture
            .core
            .complete(CompletionToken {
                serial: u64::MAX,
                ..old
            })
            .unwrap()
    );
    assert!(!fixture.control.complete_through(old));
    assert_eq!(fixture.core.snapshot().in_flight, 1);
    fixture.control.complete_through(new);
    fixture.core.poll_completion().unwrap();
}

#[test]
fn recreation_requires_discarding_the_callers_prepared_candidate() {
    let mut fixture = Fixture::new(32, 2);
    fixture.install(1);
    let prepared = fixture.core.prepare(fixture.identity(2), plan(1)).unwrap();
    fixture.control.lose_device();
    assert!(fixture.core.poll_completion().is_err());
    let next = fixture.core.snapshot().last_submitted.device.next().unwrap();
    assert_eq!(
        fixture.core.reset_after_loss(next).unwrap_err(),
        CoreError::PreparedStillLive
    );
    assert_eq!(fixture.core.snapshot().bytes_used, 8);
    drop(prepared);
    fixture.core.reset_after_loss(next).unwrap();
    assert_eq!(fixture.core.snapshot().bytes_used, 0);
}

#[test]
fn empty_plan_has_a_generation_but_submits_no_batches() {
    let mut fixture = Fixture::new(0, 0);
    let empty = RenderPlan::default()
        .compile(&PlanLimits {
            max_passes: 0,
            max_external_textures: 0,
            max_outputs: 0,
            max_dependencies: 0,
            max_texture_dimension: 0,
            max_texture_bytes: 0,
        })
        .unwrap();
    let prepared = fixture.core.prepare(fixture.identity(1), empty).unwrap();
    assert_eq!(fixture.core.install(prepared).unwrap().output_count, 0);
    assert_eq!(fixture.core.submit(time()).unwrap(), None);
    assert_eq!(fixture.core.snapshot().bytes_used, 0);
}

#[test]
fn actual_resource_destruction_precedes_budget_release() {
    use std::cell::Cell;
    struct ObservedResource {
        budget: Rc<RefCell<BudgetState>>,
        used_at_drop: Rc<Cell<u64>>,
    }
    impl Drop for ObservedResource {
        fn drop(&mut self) {
            self.used_at_drop.set(self.budget.borrow().used);
        }
    }
    struct ObservedBackend {
        device: DeviceGeneration,
        budget: Option<Rc<RefCell<BudgetState>>>,
        used_at_drop: Rc<Cell<u64>>,
    }
    impl Backend for ObservedBackend {
        type Resources = ObservedResource;
        fn resource_bytes(&self, _plan: &CompiledPlan) -> Result<u64, BackendError> {
            Ok(4)
        }
        fn prepare(
            &mut self,
            _plan: &CompiledPlan,
            _device: DeviceGeneration,
        ) -> Result<Self::Resources, BackendError> {
            Ok(ObservedResource {
                budget: self.budget.clone().unwrap(),
                used_at_drop: self.used_at_drop.clone(),
            })
        }
        fn submit(
            &mut self,
            _resources: &Self::Resources,
            _plan: &CompiledPlan,
            _token: CompletionToken,
            _time: TimeSnapshot,
        ) -> Result<(), BackendError> {
            Ok(())
        }
        fn completion(&mut self) -> Result<CompletionToken, BackendError> {
            Ok(CompletionToken {
                device: self.device,
                serial: 0,
            })
        }
        fn wait_for_completion(&mut self, _timeout: Duration) -> Result<(), BackendError> {
            Ok(())
        }
        fn reset(&mut self, _device: DeviceGeneration) -> Result<(), BackendError> {
            Err(BackendError::DeviceLost)
        }
    }
    let runtime = Uuid::new_v4();
    let device = DeviceGeneration::new(runtime, 0);
    let used_at_drop = Rc::new(Cell::new(0));
    let backend = ObservedBackend {
        device,
        budget: None,
        used_at_drop: used_at_drop.clone(),
    };
    let mut core = RenderCore::new(
        backend,
        device,
        ResourceLimits {
            max_bytes: 4,
            max_in_flight: NonZeroUsize::MIN,
            max_retiring: 0,
        },
    )
    .unwrap();
    core.backend.budget = Some(core.budget.clone());
    let identity = PlanIdentity {
        project: ProjectEpoch::new(runtime, 0),
        revision: PlanRevision::new(1),
    };
    let prepared = core.prepare(identity, plan(1)).unwrap();
    drop(prepared);
    assert_eq!(used_at_drop.get(), 4);
    assert_eq!(core.snapshot().bytes_used, 0);
}
