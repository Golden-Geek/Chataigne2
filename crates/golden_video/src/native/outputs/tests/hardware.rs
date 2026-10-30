use std::num::NonZeroUsize;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use golden_time::RationalRate;
use uuid::Uuid;

use super::super::Delivery;
use super::super::spout::SpoutOutput;
use crate::native::completion::EventCompletion;
use crate::native::device::Device;
use crate::native::texture::Texture;
use crate::native::{AdapterId, D3d11Config, D3d11Control, InputLimits, OutputBinding, OutputState, adapters};
use crate::plan::{PlanLimits, PlanOutput, RenderOperation, RenderPass, RenderPlan};
use crate::runtime::{
    CompletionToken, DeviceGeneration, LatePolicy, PlanActivation, ResourceLimits, RuntimeConfig, RuntimeState,
    VideoRuntime,
};
use crate::{
    AlphaMode, ColorEncoding, Extent2D, LinearColor, OutputId, PassId, PixelFormat, TextureDescriptor, TextureId,
};

const WAIT: Duration = Duration::from_secs(6);

fn descriptor() -> TextureDescriptor {
    TextureDescriptor {
        extent: Extent2D::new(8, 8).unwrap(),
        format: PixelFormat::Rgba8Unorm,
        color_encoding: ColorEncoding::Linear,
        alpha_mode: AlphaMode::Premultiplied,
    }
}

fn wait(label: &str, mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + WAIT;
    while !ready() {
        assert!(Instant::now() < deadline, "timed out: {label}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Its separate native device/sender is owned by this test, with the same completion discipline
/// as production. Destruction cannot release SDK storage while its copy is still queued.
struct Collider {
    output: Option<SpoutOutput>,
    texture: Texture,
    device: Device,
    completion: EventCompletion,
    token: CompletionToken,
}

impl Collider {
    fn new(adapter: AdapterId, name: String) -> Self {
        let device = Device::new(adapter).unwrap();
        let texture = Texture::new(&device.device, descriptor()).unwrap();
        let output = SpoutOutput::new(&device.device, name, descriptor()).unwrap();
        let completion = EventCompletion::new(&device).unwrap();
        let token = CompletionToken {
            device: DeviceGeneration::new(Uuid::new_v4(), 1),
            serial: 1,
        };
        let mut collider = Self {
            output: Some(output),
            texture,
            device,
            completion,
            token,
        };
        let delivered;
        {
            let _submission = collider.completion.begin(&collider.device, token);
            // SAFETY: this test owns the texture, device and context on this thread.
            unsafe {
                collider
                    .device
                    .context
                    .ClearRenderTargetView(&collider.texture.rtv, &[1.0, 0.0, 0.0, 1.0]);
            }
            delivered = collider
                .output
                .as_mut()
                .unwrap()
                .deliver(&collider.texture, 0, None, &mut None);
        }
        collider.drain();
        assert_eq!(delivered, Delivery::Accepted);
        collider
    }

    fn drain(&mut self) {
        wait("owned collider GPU completion", || {
            self.completion
                .completed(&self.device, self.token.device)
                .unwrap()
                .serial
                >= self.token.serial
        });
    }
}

impl Drop for Collider {
    fn drop(&mut self) {
        while !self.device.removed()
            && !self
                .completion
                .completed(&self.device, self.token.device)
                .is_ok_and(|done| done.serial >= self.token.serial)
        {
            std::thread::sleep(Duration::from_millis(1));
        }
        self.output.take();
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

fn config() -> RuntimeConfig {
    RuntimeConfig {
        plan_limits: PlanLimits {
            max_passes: 2,
            max_external_textures: 0,
            max_outputs: 2,
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
        max_recovery_delay: Duration::from_secs(1),
        notifier: None,
    }
}

fn scene() -> RenderPlan {
    RenderPlan {
        external_textures: vec![],
        passes: vec![RenderPass {
            id: PassId::new(1).unwrap(),
            output: TextureId::new(1).unwrap(),
            descriptor: descriptor(),
            operation: RenderOperation::Clear(LinearColor::from_straight(0.0, 1.0, 0.0, 1.0).unwrap()),
        }],
        outputs: (1..=2)
            .map(|id| PlanOutput {
                id: OutputId::new(id).unwrap(),
                source: TextureId::new(1).unwrap(),
            })
            .collect(),
    }
}

fn endpoint(control: &D3d11Control, id: u64) -> crate::native::OutputSnapshot {
    control
        .output_snapshots()
        .into_iter()
        .find(|output| output.id == OutputId::new(id).unwrap())
        .unwrap()
}

#[test]
#[ignore = "requires hardware D3D11 and owned native Spout senders"]
fn hardware_output_collision_recovers_through_public_runtime_without_stopping_healthy_output() {
    let (done, stop) = mpsc::channel();
    std::thread::spawn(move || {
        if matches!(
            stop.recv_timeout(Duration::from_secs(60)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ) {
            eprintln!("owned output recovery test exceeded 60 seconds");
            std::process::exit(1);
        }
    });
    let adapter = adapters()
        .unwrap()
        .into_iter()
        .find(|adapter| !adapter.software)
        .unwrap();
    let identity = Uuid::new_v4();
    let collided_name = format!("GoldenRecoveryCollision-{identity}");
    let collider = Collider::new(adapter.id, collided_name.clone());
    let mut guard = RuntimeGuard(Some(
        VideoRuntime::spawn_d3d11(
            config(),
            D3d11Config {
                diagnostics: None,
                adapter: adapter.id,
                inputs: InputLimits::default(),
                outputs: vec![
                    OutputBinding::Spout {
                        id: OutputId::new(1).unwrap(),
                        name: collided_name,
                        descriptor: descriptor(),
                    },
                    OutputBinding::Spout {
                        id: OutputId::new(2).unwrap(),
                        name: format!("GoldenRecoveryHealthy-{identity}"),
                        descriptor: descriptor(),
                    },
                ],
            },
        )
        .unwrap(),
    ));
    let runtime = guard.0.as_mut().unwrap();
    let control = runtime.d3d11_control().unwrap();
    runtime
        .submit_plan(
            runtime.project_epoch(),
            scene(),
            PlanActivation::NextFrame(runtime.read_time().unwrap().epoch()),
            LatePolicy::Activate,
        )
        .unwrap();
    wait("initial output isolation", || {
        let snapshot = runtime.snapshot();
        assert!(snapshot.failure.is_none(), "{:?}", snapshot.failure);
        control.snapshot().initialized
            && snapshot.state == RuntimeState::Running
            && control.output_snapshots().len() == 2
            && endpoint(&control, 1).state == OutputState::Recovering
            && endpoint(&control, 2).last_accepted.is_some()
    });
    let failed = endpoint(&control, 1);
    let healthy = endpoint(&control, 2);
    let native_device = control.snapshot().device;
    let frames = runtime.snapshot().frames_submitted;
    let until = Instant::now() + Duration::from_millis(550);
    while Instant::now() < until {
        assert_eq!(runtime.snapshot().state, RuntimeState::Running);
        assert!(runtime.snapshot().failure.is_none());
        std::thread::sleep(Duration::from_millis(10));
    }
    let retried = endpoint(&control, 1);
    assert!(
        retried.attempts <= failed.attempts + 3,
        "retry attempts must be capped: {failed:?} -> {retried:?}"
    );
    assert!(retried.hresult.is_some());
    assert!(runtime.snapshot().frames_submitted > frames);
    assert!(endpoint(&control, 2).last_accepted.unwrap().serial > healthy.last_accepted.unwrap().serial);
    assert_eq!(endpoint(&control, 2).generation, healthy.generation);
    // Collider's creation already observed its real event-query completion; Drop checks again
    // before freeing the name and its device. No external sender or setting is touched.
    drop(collider);
    wait("exact-name output recovery", || {
        assert!(runtime.snapshot().failure.is_none());
        let recovered = endpoint(&control, 1);
        recovered.state == OutputState::Ready
            && recovered.last_accepted.is_some()
            && recovered.generation > failed.generation
            && recovered.attempts > failed.attempts
    });
    assert_eq!(
        control.snapshot().device,
        native_device,
        "ordinary output recovery keeps the device"
    );
    assert!(control.snapshot().initialized);
    assert_eq!(endpoint(&control, 2).generation, healthy.generation);
    eprintln!(
        "public output recovery failed={failed:?} retried={retried:?} recovered={:?} healthy={:?} frames={}",
        endpoint(&control, 1),
        endpoint(&control, 2),
        runtime.snapshot().frames_submitted
    );
    drop(guard); // RuntimeGuard waits for actual worker/resource retirement.
    assert!(control.output_snapshots().is_empty());
    done.send(()).unwrap();
}
