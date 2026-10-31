use std::num::NonZeroUsize;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use golden_time::RationalRate;
use uuid::Uuid;
use windows::Win32::Graphics::Dxgi::{DXGI_ERROR_DEVICE_REMOVED, DXGI_ERROR_NOT_FOUND};

use super::super::*;
use crate::native::tests::initialization_gate::InitializationGate;
use crate::native::{AdapterId, D3d11Config, D3d11Control, InputLimits, OutputBinding, adapters};
use crate::plan::{PlanLimits, PlanOutput, RenderPass, RenderPlan};
use crate::runtime::{
    LatePolicy, OutcomeKind, PlanActivation, ResourceLimits, RuntimeConfig, RuntimeState, VideoRuntime,
};
use crate::{AlphaMode, ColorEncoding, Extent2D, PixelFormat};

#[test]
fn initialization_classification_distinguishes_permanent_contracts_from_retryable_native_unavailability() {
    for error in [
        DXGI_ERROR_UNSUPPORTED,
        DXGI_ERROR_INVALID_CALL,
        E_INVALIDARG,
        E_NOINTERFACE,
    ] {
        assert_eq!(initialization_error(error), BackendError::UnsupportedInitialization);
    }
    for error in [
        DXGI_ERROR_NOT_FOUND,
        DXGI_ERROR_DEVICE_REMOVED,
        HRESULT(0x8007000e_u32 as i32),
    ] {
        assert_eq!(initialization_error(error), BackendError::AllocationFailed);
    }
}

const WAIT: Duration = Duration::from_secs(6);

fn wait(label: &str, mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + WAIT;
    while !ready() {
        assert!(Instant::now() < deadline, "timed out: {label}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

pub(super) fn descriptor() -> TextureDescriptor {
    TextureDescriptor {
        extent: Extent2D::new(8, 8).unwrap(),
        format: PixelFormat::Rgba8Unorm,
        color_encoding: ColorEncoding::Linear,
        alpha_mode: AlphaMode::Premultiplied,
    }
}

pub(super) fn config() -> RuntimeConfig {
    RuntimeConfig {
        plan_limits: PlanLimits {
            max_passes: 2,
            max_external_textures: 0,
            max_outputs: 1,
            max_dependencies: 4,
            max_texture_dimension: 16,
            max_texture_bytes: 65536,
        },
        resource_limits: ResourceLimits {
            max_bytes: 65536,
            max_in_flight: NonZeroUsize::new(2).unwrap(),
            max_retiring: 2,
        },
        frame_rate: RationalRate::new(60, 1).unwrap(),
        max_outstanding: NonZeroUsize::new(4).unwrap(),
        max_input_bytes: NonZeroUsize::new(65536).unwrap(),
        max_retained_input_bytes: NonZeroUsize::new(262144).unwrap(),
        recovery_delay: Duration::from_millis(100),
        max_recovery_delay: Duration::from_millis(400),
        notifier: None,
    }
}

pub(super) fn scene() -> RenderPlan {
    RenderPlan {
        external_textures: vec![],
        passes: vec![RenderPass {
            id: PassId::new(1).unwrap(),
            output: TextureId::new(1).unwrap(),
            descriptor: descriptor(),
            operation: RenderOperation::Clear(LinearColor::from_straight(0.0, 1.0, 0.0, 1.0).unwrap()),
        }],
        outputs: vec![PlanOutput {
            id: OutputId::new(1).unwrap(),
            source: TextureId::new(1).unwrap(),
        }],
    }
}

struct RuntimeGuard(Option<VideoRuntime>);
impl Drop for RuntimeGuard {
    fn drop(&mut self) {
        if let Some(runtime) = self.0.take() {
            let retirement = runtime.retirement();
            runtime.close();
            drop(runtime);
            while !retirement.wait(Duration::from_millis(20)) {}
        }
    }
}

#[test]
#[ignore = "requires hardware D3D11 and an owned native Spout sender"]
fn hardware_initial_native_allocation_retries_real_unavailable_adapter_then_serves_first_device() {
    let (done, stop) = mpsc::channel();
    std::thread::spawn(move || {
        if matches!(
            stop.recv_timeout(Duration::from_secs(60)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ) {
            eprintln!("owned initial native allocation test exceeded 60 seconds");
            std::process::exit(1);
        }
    });
    let inventory = adapters().unwrap();
    let selected = inventory.iter().find(|adapter| !adapter.software).unwrap().id;
    let mut unavailable = AdapterId {
        low: u32::MAX,
        high: i32::MAX,
    };
    while inventory.iter().any(|adapter| adapter.id == unavailable) {
        unavailable.low -= 1;
    }
    let gate = InitializationGate::new(unavailable);
    let control = D3d11Control::default();
    *control.initialization_gate.lock().unwrap() = Some(gate.clone());
    let mut guard = RuntimeGuard(Some(
        VideoRuntime::spawn_d3d11_with_control(
            config(),
            D3d11Config {
                diagnostics: None,
                adapter: selected,
                inputs: InputLimits::default(),
                outputs: vec![OutputBinding::Spout {
                    id: OutputId::new(1).unwrap(),
                    name: format!("GoldenInitialRecovery-{}", Uuid::new_v4()),
                    descriptor: descriptor(),
                }],
            },
            control.clone(),
        )
        .unwrap(),
    ));
    let runtime = guard.0.as_mut().unwrap();
    wait("initial native allocation failure remains recoverable", || {
        control.snapshot().initialization_attempts >= 1
            && runtime.snapshot().state == RuntimeState::Faulted
            && runtime.snapshot().resources.is_some()
    });
    let pending = runtime.snapshot().resources.unwrap();
    let first_device = pending.last_submitted.device;
    let baseline = CONSTANT_BYTES + descriptor().byte_len().unwrap();
    assert_eq!(first_device.generation(), 1);
    assert_eq!(pending.bytes_used, baseline);
    assert_eq!(pending.peak_bytes, baseline);
    assert!(!pending.initialized);
    assert!(
        !pending.device_lost,
        "no native device ever existed, so no device loss occurred"
    );
    assert_eq!(pending.last_submitted.serial, 0);
    assert_eq!(pending.last_completed.serial, 0);
    assert_eq!(control.snapshot().hresult, Some(DXGI_ERROR_NOT_FOUND.0));
    assert!(!control.snapshot().initialized);
    assert!(control.snapshot().device.is_none());
    assert!(control.snapshot().adapter.is_none());
    assert!(control.output_snapshots().is_empty());
    assert!(control.sources().is_empty());
    let request = runtime
        .submit_plan(
            runtime.project_epoch(),
            scene(),
            PlanActivation::NextFrame(runtime.read_time().unwrap().epoch()),
            LatePolicy::Activate,
        )
        .unwrap();
    let before = control.snapshot().initialization_attempts;
    let until = Instant::now() + Duration::from_millis(550);
    while Instant::now() < until {
        let snapshot = runtime.snapshot();
        assert_eq!(snapshot.state, RuntimeState::Faulted);
        assert_eq!(snapshot.frames_submitted, 0);
        let resources = snapshot.resources.unwrap();
        assert!(!resources.initialized);
        assert!(!resources.device_lost);
        assert_eq!(resources.bytes_used, baseline);
        assert_eq!(resources.peak_bytes, baseline);
        assert_eq!(resources.last_submitted.device, first_device);
        assert!(!control.snapshot().initialized);
        assert!(control.snapshot().device.is_none());
        assert!(control.snapshot().adapter.is_none());
        assert!(control.output_snapshots().is_empty());
        std::thread::sleep(Duration::from_millis(10));
    }
    let attempts = control.snapshot().initialization_attempts;
    assert!(
        attempts > before && attempts <= before + 4,
        "actual factory attempts are backoff-bounded"
    );
    let mut outcomes = Vec::new();
    runtime.drain_outcomes(&mut outcomes, NonZeroUsize::new(4).unwrap());
    assert!(
        outcomes.is_empty(),
        "startup unavailability keeps the authored plan pending"
    );
    // Only the test factory gate changes. Production configuration still selects the same
    // hardware LUID; Device::new executes its real DXGI/D3D11 path on every attempt.
    gate.release();
    wait("first native device and submitted output", || {
        let native = control.snapshot();
        let snapshot = runtime.snapshot();
        native.initialized
            && native.adapter == Some(selected)
            && native.device == Some(first_device)
            && snapshot.state == RuntimeState::Running
            && snapshot.frames_submitted >= 2
            && control.output_snapshots().iter().any(|output| {
                output
                    .last_accepted
                    .is_some_and(|token| token.device == first_device && token.serial > 0)
            })
    });
    let recovered = runtime.snapshot().resources.unwrap();
    assert_eq!(
        runtime.snapshot().failure.unwrap().code(),
        crate::runtime::FailureCode::Backend,
        "serving state recovers while the latest startup error remains diagnostic history"
    );
    assert!(recovered.initialized);
    assert!(!recovered.device_lost);
    assert_eq!(recovered.last_submitted.device, first_device);
    assert_eq!(recovered.bytes_used, baseline + 2 * descriptor().byte_len().unwrap());
    assert_eq!(recovered.peak_bytes, recovered.bytes_used);
    assert_eq!(control.snapshot().hresult, None);
    runtime.drain_outcomes(&mut outcomes, NonZeroUsize::new(4).unwrap());
    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].request, request);
    assert!(matches!(outcomes[0].result, OutcomeKind::Installed { .. }));
    eprintln!(
        "native initialization attempts={} baseline={} first_device={first_device:?} submitted={}",
        control.snapshot().initialization_attempts,
        baseline,
        runtime.snapshot().frames_submitted
    );
    drop(guard);
    assert!(!control.snapshot().initialized);
    assert!(control.snapshot().device.is_none());
    assert!(control.snapshot().adapter.is_none());
    assert!(control.output_snapshots().is_empty());
    done.send(()).unwrap();
}
