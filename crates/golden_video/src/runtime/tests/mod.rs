use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use golden_time::{
    ActivationStamp, Clock, MonotonicTime, PlaybackRate, RationalRate, ShowPosition, TimeAuthority, TimeError,
    TimeSnapshot,
};

use crate::plan::{PlanLimits, PlanOutput, RenderOperation, RenderPass, RenderPlan};
use crate::{
    AlphaMode, ColorEncoding, Extent2D, LinearColor, OutputId, PassId, PixelFormat, TextureDescriptor, TextureId,
};

use super::{
    AdmissionError, FailureCode, GenerationInfo, LatePolicy, NullConfig, NullControl, OutcomeKind, PlanActivation,
    RequestId, ResourceLimits, RuntimeConfig, RuntimeOutcome, RuntimeRetirement, RuntimeState, VideoRuntime,
};

mod admission;
mod ownership;
mod scheduling;

const DEADLINE: Duration = Duration::from_secs(5);

#[derive(Default)]
struct TestClock(AtomicU64);

impl TestClock {
    fn at(nanos: u64) -> Arc<Self> {
        Arc::new(Self(AtomicU64::new(nanos)))
    }
    fn set(&self, nanos: u64) {
        self.0.store(nanos, Ordering::Release);
    }
}

impl Clock for TestClock {
    fn now(&self) -> Result<MonotonicTime, TimeError> {
        Ok(MonotonicTime::from_nanos(self.0.load(Ordering::Acquire)))
    }
}

fn config() -> RuntimeConfig {
    RuntimeConfig {
        plan_limits: PlanLimits {
            max_passes: 64,
            max_external_textures: 64,
            max_outputs: 64,
            max_dependencies: 256,
            max_texture_dimension: 16_384,
            max_texture_bytes: 1024 * 1024,
        },
        resource_limits: ResourceLimits {
            max_bytes: 1024 * 1024,
            max_in_flight: NonZeroUsize::new(2).unwrap(),
            max_retiring: 2,
        },
        frame_rate: RationalRate::new(60, 1).unwrap(),
        max_outstanding: NonZeroUsize::new(16).unwrap(),
        max_input_bytes: NonZeroUsize::new(256 * 1024).unwrap(),
        max_retained_input_bytes: NonZeroUsize::new(1024 * 1024).unwrap(),
        recovery_delay: Duration::from_millis(10),
        max_recovery_delay: Duration::from_millis(100),
        notifier: None,
    }
}

fn plan(value: u8, extent: u32, outputs: u64) -> RenderPlan {
    let texture = TextureId::new(1).unwrap();
    RenderPlan {
        external_textures: Vec::new(),
        passes: vec![RenderPass {
            id: PassId::new(1).unwrap(),
            output: texture,
            descriptor: TextureDescriptor {
                extent: Extent2D::new(extent, extent).unwrap(),
                format: PixelFormat::Bgra8Unorm,
                color_encoding: ColorEncoding::Linear,
                alpha_mode: AlphaMode::Premultiplied,
            },
            operation: RenderOperation::Clear(
                LinearColor::from_straight(f32::from(value) / 255.0, 0.0, 0.0, 1.0).unwrap(),
            ),
        }],
        outputs: (1..=outputs)
            .map(|id| PlanOutput {
                id: OutputId::new(id).unwrap(),
                source: texture,
            })
            .collect(),
    }
}

struct Harness {
    runtime: Option<VideoRuntime>,
    clock: Arc<TestClock>,
    null: NullControl,
    retirement: RuntimeRetirement,
    outcomes: BTreeMap<RequestId, RuntimeOutcome>,
}

impl Harness {
    fn new(config: RuntimeConfig, null: NullConfig) -> Self {
        let clock = TestClock::at(0);
        let runtime = VideoRuntime::spawn_null_with_clock(config, null, clock.clone()).unwrap();
        let null = runtime.null_control();
        let retirement = runtime.retirement();
        let mut harness = Self {
            runtime: Some(runtime),
            clock,
            null,
            retirement,
            outcomes: BTreeMap::new(),
        };
        harness.wait("runtime startup", |h| {
            h.runtime().snapshot().state == RuntimeState::Running
        });
        harness
    }

    fn runtime(&self) -> &VideoRuntime {
        self.runtime.as_ref().unwrap()
    }
    fn runtime_mut(&mut self) -> &mut VideoRuntime {
        self.runtime.as_mut().unwrap()
    }

    fn advance(&self, nanos: u64) {
        self.clock.set(nanos);
        self.runtime().wake();
    }

    fn submit(&mut self, plan: RenderPlan) -> RequestId {
        let project = self.runtime().project_epoch();
        let epoch = self.runtime().read_time().unwrap().epoch();
        self.runtime_mut()
            .submit_plan(project, plan, PlanActivation::NextFrame(epoch), LatePolicy::Activate)
            .unwrap()
    }

    fn collect(&mut self) {
        let mut collected = Vec::new();
        self.runtime()
            .drain_outcomes(&mut collected, NonZeroUsize::new(64).unwrap());
        for outcome in collected {
            assert!(
                self.outcomes.insert(outcome.request, outcome).is_none(),
                "duplicate reliable outcome"
            );
        }
    }

    fn outcome(&mut self, request: RequestId) -> RuntimeOutcome {
        self.wait("reliable request outcome", |h| {
            h.collect();
            h.outcomes.contains_key(&request)
        });
        self.outcomes.remove(&request).unwrap()
    }

    fn installed(&mut self, request: RequestId) -> (GenerationInfo, TimeSnapshot) {
        match self.outcome(request).result {
            OutcomeKind::Installed { generation, at } => (generation, at),
            result => panic!("expected installation, got {result:?}"),
        }
    }

    fn wait(&mut self, description: &str, mut ready: impl FnMut(&mut Self) -> bool) {
        let until = Instant::now() + DEADLINE;
        loop {
            if ready(self) {
                return;
            }
            assert!(
                Instant::now() < until,
                "timed out waiting for {description}; runtime={:?}; null={:?}",
                self.runtime().snapshot(),
                self.null.snapshot()
            );
            thread::sleep(Duration::from_millis(2));
        }
    }

    fn no_outcome(&mut self, request: RequestId, duration: Duration) {
        let until = Instant::now() + duration;
        loop {
            self.collect();
            assert!(
                !self.outcomes.contains_key(&request),
                "retirement acknowledged before matching completion"
            );
            if Instant::now() >= until {
                break;
            }
            thread::sleep(Duration::from_millis(2));
        }
    }

    fn drive(&mut self, description: &str, mut ready: impl FnMut(&mut Self) -> bool) {
        self.wait(description, |h| {
            if ready(h) {
                return true;
            }
            let next = h.clock.0.load(Ordering::Acquire).checked_add(20_000_000).unwrap();
            h.advance(next);
            false
        });
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.close();
            self.null.lose_device();
            drop(runtime);
        } else {
            self.null.lose_device();
        }
        let retired = self.retirement.wait(DEADLINE);
        if !thread::panicking() {
            assert!(retired, "test runtime failed to retire owned workers/resources");
        }
    }
}

fn rejected_code(outcome: RuntimeOutcome) -> FailureCode {
    match outcome.result {
        OutcomeKind::Rejected(failure) => failure.code(),
        result => panic!("expected rejection, got {result:?}"),
    }
}
