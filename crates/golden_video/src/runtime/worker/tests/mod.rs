use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

use golden_io::WorkerLimits;
use golden_time::{PlaybackRate, RationalRate, ShowPosition, SystemClock};
use uuid::Uuid;

use super::*;
use crate::plan::{CompiledPlan, PlanLimits};
use crate::runtime::backend::BackendError;
use crate::runtime::{CompletionToken, ResourceLimits};

const BASELINE_BYTES: u64 = 13;
const DEADLINE: Duration = Duration::from_secs(5);

#[derive(Default)]
struct Observations {
    attempts: AtomicUsize,
    charged_at_initialization: AtomicU64,
    dropped: AtomicBool,
}

struct UnsupportedBackend {
    shared: Arc<Shared>,
    observed: Arc<Observations>,
}

impl Backend for UnsupportedBackend {
    type Resources = ();

    fn base_resource_bytes(&self) -> Result<u64, BackendError> {
        Ok(BASELINE_BYTES)
    }

    fn initialize(&mut self) -> Result<(), BackendError> {
        self.observed.attempts.fetch_add(1, Ordering::AcqRel);
        let resources = self.shared.snapshot().resources.expect("baseline was admitted");
        self.observed
            .charged_at_initialization
            .store(resources.bytes_used, Ordering::Release);
        Err(BackendError::AlreadyBound)
    }

    fn resource_bytes(&self, _: &CompiledPlan) -> Result<u64, BackendError> {
        panic!("uninitialized backend cannot admit plan resources")
    }

    fn prepare(&mut self, _: &CompiledPlan, _: DeviceGeneration) -> Result<(), BackendError> {
        panic!("uninitialized backend cannot prepare resources")
    }

    fn submit(&mut self, _: &(), _: &CompiledPlan, _: CompletionToken, _: TimeSnapshot) -> Result<(), BackendError> {
        panic!("uninitialized backend cannot submit GPU work")
    }

    fn completion(&mut self) -> Result<CompletionToken, BackendError> {
        panic!("uninitialized backend cannot query native completion")
    }

    fn wait_for_completion(&mut self, _: Duration) -> Result<(), BackendError> {
        panic!("uninitialized backend cannot wait for native completion")
    }

    fn reset(&mut self, _: DeviceGeneration) -> Result<(), BackendError> {
        panic!("initialization failure cannot fabricate device loss")
    }
}

impl Drop for UnsupportedBackend {
    fn drop(&mut self) {
        self.observed.dropped.store(true, Ordering::Release);
    }
}

#[test]
fn terminal_initialization_retires_without_an_installed_shared_waker() {
    let runtime = Uuid::new_v4();
    let device = DeviceGeneration::new(runtime, 1);
    let clock: Arc<dyn Clock> = Arc::new(SystemClock::new());
    let mut authority = TimeAuthority::new(clock.clone(), ShowPosition::from_nanos(0), PlaybackRate::NORMAL).unwrap();
    let shared = Arc::new(Shared::new(
        ProjectEpoch::new(runtime, 0),
        authority.sample().unwrap(),
        clock,
        1,
        None,
    ));
    let observed = Arc::new(Observations::default());
    let render_lifetimes = RetirementPool::new(NonZeroUsize::MIN);
    let preparation_lifetimes = RetirementPool::new(NonZeroUsize::MIN);
    let run_preparation = preparation_lifetimes.clone();
    let run_shared = shared.clone();
    let run_observed = observed.clone();
    // Intentionally never install Shared's waker: initialization can finish before spawn
    // returns to the normal caller that installs it. Cancellation still owns a direct wake.
    let worker = BoundedWorker::<(), ()>::spawn(
        "golden-video-terminal-startup-test",
        &render_lifetimes,
        WorkerLimits {
            command_items: 1,
            command_weight: 1,
            event_items: 1,
            event_weight: 1,
        },
        move |context| {
            let backend_shared = run_shared.clone();
            run_backend(
                context,
                run_shared,
                config(),
                run_preparation,
                device,
                authority,
                Duration::ZERO,
                move || {
                    Ok(UnsupportedBackend {
                        shared: backend_shared,
                        observed: run_observed,
                    })
                },
            );
        },
    )
    .unwrap();
    let retired = render_lifetimes.wait_for_idle(DEADLINE);
    if !retired {
        worker.cancel();
        assert!(
            render_lifetimes.wait_for_idle(DEADLINE),
            "failed test cleanup did not retire"
        );
    }
    assert!(retired, "terminal startup must not park in the one-hour idle wait");
    assert!(preparation_lifetimes.wait_for_idle(DEADLINE));
    assert_eq!(observed.attempts.load(Ordering::Acquire), 1);
    assert_eq!(
        observed.charged_at_initialization.load(Ordering::Acquire),
        BASELINE_BYTES
    );
    assert!(observed.dropped.load(Ordering::Acquire));
    let snapshot = shared.snapshot();
    assert_eq!(snapshot.state, RuntimeState::Stopped);
    assert_eq!(snapshot.failure.unwrap().code(), FailureCode::Backend);
    let resources = snapshot.resources.expect("retirement retains zero-charge metadata");
    assert!(!resources.initialized);
    assert!(!resources.device_lost);
    assert_eq!(resources.bytes_used, 0);
    assert_eq!(resources.peak_bytes, BASELINE_BYTES);
    assert_eq!(resources.in_flight, 0);
    assert!(resources.active.is_none());
    assert_eq!(resources.last_submitted.device, device);
    assert_eq!(resources.last_submitted.serial, 0);
}

fn config() -> RuntimeConfig {
    RuntimeConfig {
        plan_limits: PlanLimits {
            max_passes: 1,
            max_external_textures: 1,
            max_outputs: 1,
            max_dependencies: 1,
            max_texture_dimension: 1,
            max_texture_bytes: 4,
        },
        resource_limits: ResourceLimits {
            max_bytes: BASELINE_BYTES,
            max_in_flight: NonZeroUsize::MIN,
            max_retiring: 0,
        },
        frame_rate: RationalRate::new(60, 1).unwrap(),
        max_outstanding: NonZeroUsize::MIN,
        max_input_bytes: NonZeroUsize::MIN,
        max_retained_input_bytes: NonZeroUsize::MIN,
        recovery_delay: Duration::from_millis(10),
        max_recovery_delay: Duration::from_millis(100),
        notifier: None,
    }
}
