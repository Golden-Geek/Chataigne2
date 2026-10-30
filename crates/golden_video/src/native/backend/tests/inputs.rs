use std::io::{BufRead, BufReader, Write};
use std::num::NonZeroUsize;
use std::os::windows::process::CommandExt;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};
use windows::Win32::System::Threading::CREATE_NO_WINDOW;

use golden_time::{PlaybackRate, RationalRate, ShowPosition, SystemClock, TimeAuthority};
use uuid::Uuid;

use super::super::*;
use crate::native::{AdapterId, InputLimits, OutputBinding, SourceState, adapters};
use crate::plan::SpoutSource;
use crate::plan::{ExternalTexture, Layer, PlanLimits, PlanOutput, RenderPass, RenderPlan};
use crate::runtime::{LatePolicy, PlanActivation, ResourceLimits, RuntimeConfig, RuntimeState, VideoRuntime};
use crate::{AlphaMode, ColorEncoding, Extent2D, Opacity, PixelFormat};

const WAIT: Duration = Duration::from_secs(5);

struct Producer {
    child: Arc<Mutex<Child>>,
    input: ChildStdin,
    replies: mpsc::Receiver<String>,
}

impl Producer {
    fn new(adapter: usize, name: &str, registry: &Mutex<Vec<Weak<Mutex<Child>>>>) -> Self {
        let executable =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/native_source_fixture.exe");
        assert!(
            executable.exists(),
            "build native_source_fixture before this explicit hardware test"
        );
        let mut child = Command::new(executable)
            .creation_flags(CREATE_NO_WINDOW.0)
            .args(["--adapter", &adapter.to_string(), "--sender", name])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let output = child.stdout.take().unwrap();
        let (tx, replies) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(output).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        let child = Arc::new(Mutex::new(child));
        registry.lock().unwrap().push(Arc::downgrade(&child));
        let producer = Self { child, input, replies };
        producer.reply();
        producer
    }

    fn reply(&self) {
        let deadline = Instant::now() + WAIT;
        loop {
            let line = self
                .replies
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .unwrap();
            if line.starts_with("OK ") {
                break;
            }
        }
    }

    fn command(&mut self, command: &str) {
        writeln!(self.input, "{command}").unwrap();
        self.input.flush().unwrap();
        self.reply();
    }
}

impl Drop for Producer {
    fn drop(&mut self) {
        // Owned fixture processes only; a failed assertion must not leave producers running.
        let mut child = self.child.lock().unwrap_or_else(|error| error.into_inner());
        let _ = child.kill();
        let _ = child.wait();
    }
}

fn external(width: u32, height: u32) -> TextureDescriptor {
    TextureDescriptor {
        extent: Extent2D::new(width, height).unwrap(),
        format: PixelFormat::Bgra8Unorm,
        color_encoding: ColorEncoding::Srgb,
        alpha_mode: AlphaMode::Opaque,
    }
}

fn internal() -> TextureDescriptor {
    TextureDescriptor {
        extent: Extent2D::new(1, 1).unwrap(),
        format: PixelFormat::Rgba8Unorm,
        color_encoding: ColorEncoding::Linear,
        alpha_mode: AlphaMode::Premultiplied,
    }
}

fn native_config(adapter: AdapterId, output_name: String) -> D3d11Config {
    D3d11Config {
        diagnostics: None,
        adapter,
        inputs: InputLimits {
            max_sources: 4,
            max_storage_bytes: 5 * InputLimits::source_storage_bytes(external(4, 4)).unwrap(),
        },
        outputs: vec![OutputBinding::Spout {
            id: OutputId::new(1).unwrap(),
            name: output_name,
            descriptor: internal(),
        }],
    }
}

fn runtime_config() -> RuntimeConfig {
    RuntimeConfig {
        plan_limits: PlanLimits {
            max_passes: 8,
            max_external_textures: 2,
            max_outputs: 1,
            max_dependencies: 16,
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

fn scene(names: &[String; 2], width: u32, height: u32) -> RenderPlan {
    RenderPlan {
        external_textures: vec![
            ExternalTexture {
                id: TextureId::new(1).unwrap(),
                descriptor: external(width, height),
                source: Some(
                    SpoutSource {
                        sender: names[0].clone(),
                        descriptor: external(4, 4),
                        poll_interval: Duration::from_millis(5),
                        frame_count: crate::plan::SpoutFrameCount::VersionedV1,
                    }
                    .into(),
                ),
            },
            ExternalTexture {
                id: TextureId::new(2).unwrap(),
                descriptor: external(2, 2),
                source: Some(
                    SpoutSource {
                        sender: names[1].clone(),
                        descriptor: external(4, 4),
                        poll_interval: Duration::from_millis(5),
                        frame_count: crate::plan::SpoutFrameCount::VersionedV1,
                    }
                    .into(),
                ),
            },
        ],
        passes: vec![RenderPass {
            id: PassId::new(1).unwrap(),
            output: TextureId::new(3).unwrap(),
            descriptor: internal(),
            operation: RenderOperation::Composite {
                layers: vec![
                    Layer {
                        source: TextureId::new(1).unwrap().into(),
                        opacity: Opacity::OPAQUE,
                        blend: BlendMode::Normal,
                        filter: SampleFilter::Nearest,
                        mapping: None,
                    },
                    Layer {
                        source: TextureId::new(2).unwrap().into(),
                        opacity: Opacity::new(0.5).unwrap(),
                        blend: BlendMode::Normal,
                        filter: SampleFilter::Nearest,
                        mapping: None,
                    },
                ],
            },
        }],
        outputs: vec![PlanOutput {
            id: OutputId::new(1).unwrap(),
            source: TextureId::new(3).unwrap(),
        }],
    }
}

fn wait(label: &str, mut check: impl FnMut() -> bool) {
    let deadline = Instant::now() + WAIT;
    while !check() {
        assert!(Instant::now() < deadline, "timed out: {label}");
        std::thread::sleep(Duration::from_millis(5));
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

struct Renderer {
    names: [String; 2],
    resources: Option<D3d11Resources>,
    backend: D3d11Backend,
    plan: CompiledPlan,
    submitted: u64,
    generation: DeviceGeneration,
}

impl Renderer {
    fn new(config: D3d11Config, names: [String; 2]) -> Self {
        let generation = DeviceGeneration::new(Uuid::new_v4(), 1);
        let mut backend = D3d11Backend::new(config, Default::default(), generation);
        backend.initialize().unwrap();
        let plan = scene(&names, 2, 2).compile(&runtime_config().plan_limits).unwrap();
        let resources = backend
            .prepare(&plan, super::generation(&backend, &plan, generation))
            .unwrap();
        Self {
            names,
            resources: Some(resources),
            backend,
            plan,
            submitted: 0,
            generation,
        }
    }

    fn submit(&mut self) {
        let serial = self.submitted + 1;
        self.submitted = serial;
        let time = TimeAuthority::new(SystemClock::new(), ShowPosition::from_nanos(0), PlaybackRate::NORMAL)
            .unwrap()
            .sample()
            .unwrap();
        if let Err(error) = self.backend.submit(
            self.resources.as_ref().unwrap(),
            &self.plan,
            CompletionToken {
                device: self.generation,
                serial,
            },
            time,
        ) {
            self.submitted -= 1;
            panic!("native submission failed before use: {error}");
        }
    }

    fn drain(&mut self) {
        wait("renderer completion", || {
            self.backend.completion().unwrap().serial >= self.submitted
        });
    }

    fn retain_input_reads(&mut self) {
        self.submitted += 1;
        let native = self.backend.native.as_mut().unwrap();
        let resources = self.resources.as_ref().unwrap();
        let _submission = native.completion.begin(
            &native.device,
            CompletionToken {
                device: self.generation,
                serial: self.submitted,
            },
        );
        for external in self.plan.external_textures() {
            let previous = resources.input_sequences[&external.id].get().unwrap_or_default();
            wait("actual source read-lease acquisition", || {
                let copied = native
                    .inputs
                    .copy(
                        external.id,
                        &native.device,
                        &resources.textures[&external.id],
                        self.submitted,
                        previous,
                    )
                    .unwrap();
                if let Some(copied) = copied.filter(|copied| copied.sequence != previous.sequence) {
                    resources.input_sequences[&external.id].set(Some(copied));
                    true
                } else {
                    false
                }
            });
        }
    }

    fn tick(&mut self) {
        self.drain();
        self.submit();
        self.drain();
    }

    fn pixel(&mut self) -> [u8; 4] {
        self.tick();
        crate::native::tests::pixels::readback(
            &self.backend.native.as_ref().unwrap().device,
            &self.resources.as_ref().unwrap().outputs[&OutputId::new(1).unwrap()],
        )[0]
    }

    fn expect_pixel(&mut self, expected: [u8; 4]) {
        let mut last = [0; 4];
        wait("composed source pixels", || {
            last = self.pixel();
            last.iter()
                .zip(expected)
                .all(|(a, b)| (*a as i32 - b as i32).abs() <= 2)
        });
        eprintln!("native input pixel {last:?}, expected {expected:?}, tolerance2/255");
    }

    fn replace(&mut self, width: u32, height: u32) {
        self.drain();
        let plan = scene(&self.names, width, height)
            .compile(&runtime_config().plan_limits)
            .unwrap();
        let resources = self
            .backend
            .prepare(&plan, super::generation(&self.backend, &plan, self.generation))
            .unwrap();
        self.resources = Some(resources);
        self.plan = plan;
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        self.backend.quiesce();
        while !self
            .backend
            .native
            .as_ref()
            .is_none_or(|native| native.device.removed())
        {
            if self
                .backend
                .completion()
                .is_ok_and(|done| done.serial >= self.submitted)
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        self.resources.take();
    }
}

#[test]
#[ignore = "requires hardware D3D11 and the built native_source_fixture executable"]
fn hardware_inputs_receive_bound_resize_reconnect_and_retire_across_independent_processes() {
    let (done, stop) = mpsc::channel();
    let children = Arc::new(Mutex::new(Vec::<Weak<Mutex<Child>>>::new()));
    let watchdog_children = children.clone();
    std::thread::spawn(move || {
        if matches!(
            stop.recv_timeout(Duration::from_secs(60)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ) {
            for child in watchdog_children.lock().unwrap().iter().filter_map(Weak::upgrade) {
                let mut child = child.lock().unwrap_or_else(|error| error.into_inner());
                let _ = child.kill();
                let _ = child.wait();
            }
            eprintln!("native input test exceeded60seconds");
            std::process::exit(1);
        }
    });
    let inventory = adapters().unwrap();
    let (index, adapter) = inventory
        .iter()
        .enumerate()
        .find(|(_, adapter)| !adapter.software)
        .unwrap();
    let identity = Uuid::new_v4();
    let names = [format!("GoldenInputA-{identity}"), format!("GoldenInputB-{identity}")];
    let mut red = Producer::new(index, &names[0], &children);
    red.command("start 2 2 1 0 0");
    let mut blue = Producer::new(index, &names[1], &children);
    blue.command("start 2 2 0 0 1");
    {
        let mut guard = RuntimeGuard(Some(
            VideoRuntime::spawn_d3d11(
                runtime_config(),
                native_config(adapter.id, format!("GoldenInputPublic-{identity}")),
            )
            .unwrap(),
        ));
        let runtime = guard.0.as_mut().unwrap();
        runtime
            .submit_plan(
                runtime.project_epoch(),
                scene(&names, 2, 2),
                PlanActivation::NextFrame(runtime.read_time().unwrap().epoch()),
                LatePolicy::Activate,
            )
            .unwrap();
        wait("public runtime source integration", || {
            let snapshot = runtime.snapshot();
            assert!(snapshot.failure.is_none(), "{:?}", snapshot.failure);
            runtime.d3d11_control().unwrap().sources().iter().all(|source| {
                source.state == SourceState::Streaming && source.renderer_copies > 0 && source.bridge_slots == 3
            }) && snapshot.frames_submitted >= 4
        });
        assert!(runtime.null_control().is_none());
        eprintln!(
            "public input runtime frames={} sources={:?}",
            runtime.snapshot().frames_submitted,
            runtime.d3d11_control().unwrap().sources()
        );
        let active = runtime.snapshot().resources.unwrap().active.unwrap();
        let before = runtime.snapshot().frames_submitted;
        let mut excessive = scene(&names, 2, 2);
        let crate::plan::ExternalSource::Spout(source) = excessive.external_textures[0].source.as_mut().unwrap() else {
            unreachable!()
        };
        source.descriptor = external(16, 16);
        let rejected = runtime
            .submit_plan(
                runtime.project_epoch(),
                excessive,
                PlanActivation::NextFrame(runtime.read_time().unwrap().epoch()),
                LatePolicy::Activate,
            )
            .unwrap();
        wait("public source-capacity rejection", || {
            let mut outcomes = Vec::new();
            runtime.drain_outcomes(&mut outcomes, NonZeroUsize::new(4).unwrap());
            outcomes.iter().any(|outcome| outcome.request == rejected && matches!(&outcome.result,
                crate::runtime::OutcomeKind::Rejected(failure) if failure.code() == crate::runtime::FailureCode::ResourcePressure))
        });
        wait("live frames continue after source pressure", || {
            runtime.snapshot().frames_submitted > before
        });
        assert_eq!(runtime.snapshot().state, RuntimeState::Running);
        assert!(runtime.snapshot().failure.is_none());
        assert_eq!(runtime.snapshot().resources.unwrap().active.unwrap(), active);
        eprintln!("public source pressure preserves Running state, live generation and continued frames");
        runtime.close();
        assert!(runtime.retirement().wait(WAIT));
        assert_eq!(runtime.snapshot().state, RuntimeState::Stopped);
        assert_eq!(runtime.snapshot().resources.unwrap().bytes_used, 0);
        assert!(
            runtime
                .d3d11_control()
                .unwrap()
                .sources()
                .iter()
                .all(|source| source.state == SourceState::Stopped)
        );
    }
    let mut renderer = Renderer::new(
        native_config(adapter.id, format!("GoldenInputPixels-{identity}")),
        names.clone(),
    );
    wait("prepared inputs warm before render activation", || {
        renderer
            .backend
            .control
            .sources()
            .iter()
            .all(|source| source.state == SourceState::Streaming && source.renderer_copies == 0)
    });
    renderer.resources.take();
    wait("discarded preparation releases input demand", || {
        renderer
            .backend
            .control
            .sources()
            .iter()
            .all(|source| source.state == SourceState::Idle)
    });
    let idle_calls = renderer
        .backend
        .control
        .sources()
        .iter()
        .map(|source| source.sdk_calls)
        .collect::<Vec<_>>();
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(
        idle_calls,
        renderer
            .backend
            .control
            .sources()
            .iter()
            .map(|source| source.sdk_calls)
            .collect::<Vec<_>>()
    );
    renderer.resources = Some(
        renderer
            .backend
            .prepare(
                &renderer.plan,
                super::generation(&renderer.backend, &renderer.plan, renderer.generation),
            )
            .unwrap(),
    );
    renderer.expect_pixel([128, 0, 128, 255]);
    // Retain two renderer-use leases without observing completion. Three source slots must
    // coalesce instead of allocating a fourth, even though GPU commands may already be done.
    renderer.drain();
    // Publication precedes actual source GPU completion. Acquire each lease through the
    // real copy path instead of assuming that two immediate submissions acquired both.
    renderer.retain_input_reads();
    renderer.retain_input_reads();
    wait("bounded source bridge pressure", || {
        renderer
            .backend
            .control
            .sources()
            .iter()
            .all(|source| source.bridge_busy > 0 && source.bridge_slots == 3)
    });
    let busy_calls = renderer
        .backend
        .control
        .sources()
        .iter()
        .map(|source| source.sdk_calls)
        .collect::<Vec<_>>();
    std::thread::sleep(Duration::from_millis(25));
    assert_eq!(
        busy_calls,
        renderer
            .backend
            .control
            .sources()
            .iter()
            .map(|source| source.sdk_calls)
            .collect::<Vec<_>>()
    );
    renderer.drain();
    red.command("pause");
    blue.command("pause");
    std::thread::sleep(Duration::from_millis(50));
    renderer.replace(2, 2);
    renderer.expect_pixel([128, 0, 128, 255]);
    blue.command("start 2 2 0 0 1");
    red.command("stop");
    wait("sender disconnect", || {
        renderer.tick();
        renderer.backend.control.sources()[0].state == SourceState::WaitingForSender
    });
    renderer.expect_pixel([0, 0, 128, 255]);
    red.command("start 3 2 1 0 0");
    wait("bounded source resize", || {
        renderer.tick();
        renderer.backend.control.sources()[0].observed_descriptor == Some(external(3, 2))
    });
    renderer.replace(3, 2);
    renderer.expect_pixel([128, 0, 128, 255]);
    let healthy = renderer.backend.control.sources()[1].publications;
    let admitted_pool = renderer.backend.control.sources()[0].pool_generation;
    red.command("start 8 8 1 0 0");
    wait("oversized sender rejection", || {
        renderer.tick();
        renderer.backend.control.sources()[0].state == SourceState::DescriptorMismatch
    });
    assert_eq!(renderer.backend.control.sources()[0].pool_generation, admitted_pool);
    assert_eq!(
        renderer.backend.control.sources()[0].observed_descriptor,
        Some(external(3, 2))
    );
    wait("healthy source continues", || {
        renderer.tick();
        renderer.backend.control.sources()[1].publications > healthy
    });
    renderer.expect_pixel([0, 0, 128, 255]);
    red.command("start 2 2 1 0 0");
    renderer.replace(2, 2);
    renderer.expect_pixel([128, 0, 128, 255]);
    let sources = renderer.backend.control.sources();
    assert!(
        sources
            .iter()
            .all(|source| source.generation == 1 && source.bridge_slots == 3)
    );
    assert!(sources[0].pool_generation >= 3);
    eprintln!(
        "native input recovery passed LUID{:08x}:{:08x}; final sources={sources:?}",
        adapter.id.high as u32, adapter.id.low
    );
    // A new sender subscription is prepared while the old generation still owns its
    // source. Changing the meaning of an existing binding is rejected atomically.
    let conflicting = scene(&[names[1].clone(), names[1].clone()], 2, 2)
        .compile(&runtime_config().plan_limits)
        .unwrap();
    assert!(matches!(
        renderer.backend.prepare(
            &conflicting,
            super::generation(&renderer.backend, &conflicting, renderer.generation)
        ),
        Err(crate::runtime::backend::BackendError::UnsupportedPlan)
    ));
    renderer.expect_pixel([128, 0, 128, 255]);
    let original_binding = renderer.backend.control.sources()[0].binding;
    let mut remapped = scene(&[names[1].clone(), names[1].clone()], 2, 2);
    remapped.external_textures[0].id = TextureId::new(4).unwrap();
    let RenderOperation::Composite { layers } = &mut remapped.passes[0].operation else {
        unreachable!()
    };
    layers[0].source = TextureId::new(4).unwrap().into();
    let remapped = remapped.compile(&runtime_config().plan_limits).unwrap();
    let next = renderer
        .backend
        .prepare(
            &remapped,
            super::generation(&renderer.backend, &remapped, renderer.generation),
        )
        .unwrap();
    let previous = renderer.resources.replace(next).unwrap();
    renderer.plan = remapped.clone();
    renderer.expect_pixel([0, 0, 255, 255]);
    assert_eq!(renderer.backend.control.sources().len(), 3);
    assert_eq!(renderer.backend.control.sources()[0].binding, original_binding);
    assert_ne!(renderer.backend.control.sources()[2].binding, original_binding);
    let mut extra = scene(&names, 2, 2);
    extra.external_textures[0].id = TextureId::new(5).unwrap();
    let RenderOperation::Composite { layers } = &mut extra.passes[0].operation else {
        unreachable!()
    };
    layers[0].source = TextureId::new(5).unwrap().into();
    let extra_plan = extra.compile(&runtime_config().plan_limits).unwrap();
    let extra = renderer
        .backend
        .prepare(
            &extra_plan,
            super::generation(&renderer.backend, &extra_plan, renderer.generation),
        )
        .unwrap();
    let mut excess = scene(&names, 2, 2);
    excess.external_textures[0].id = TextureId::new(6).unwrap();
    let RenderOperation::Composite { layers } = &mut excess.passes[0].operation else {
        unreachable!()
    };
    layers[0].source = TextureId::new(6).unwrap().into();
    let excess = excess.compile(&runtime_config().plan_limits).unwrap();
    assert!(matches!(
        renderer.backend.prepare(
            &excess,
            super::generation(&renderer.backend, &excess, renderer.generation)
        ),
        Err(crate::runtime::backend::BackendError::SourceCapacity)
    ));
    renderer.expect_pixel([0, 0, 255, 255]);
    drop(extra);
    drop(previous);
    wait("retired source capacity becomes reusable", || {
        match renderer.backend.prepare(
            &excess,
            super::generation(&renderer.backend, &excess, renderer.generation),
        ) {
            Ok(resources) => {
                drop(resources);
                true
            }
            Err(crate::runtime::backend::BackendError::SourceCapacity) => false,
            Err(error) => panic!("source replacement: {error}"),
        }
    });
    assert_eq!(renderer.backend.control.snapshot().device, Some(renderer.generation));
    eprintln!("dynamic subscription replacement, old-binding fence and four-source overlap bound passed");
    drop(renderer);
    let mut bytes_config = native_config(adapter.id, format!("GoldenInputBytes-{identity}"));
    bytes_config.inputs.max_storage_bytes = 2 * InputLimits::source_storage_bytes(external(4, 4)).unwrap();
    let mut bytes_renderer = Renderer::new(bytes_config, names.clone());
    bytes_renderer.expect_pixel([128, 0, 128, 255]);
    assert!(matches!(
        bytes_renderer.backend.prepare(
            &remapped,
            super::generation(&bytes_renderer.backend, &remapped, bytes_renderer.generation)
        ),
        Err(crate::runtime::backend::BackendError::SourceCapacity)
    ));
    assert_eq!(bytes_renderer.backend.control.sources().len(), 2);
    bytes_renderer.expect_pixel([128, 0, 128, 255]);
    eprintln!("source storage ceiling rejects a third subscription despite four available worker slots");
    drop(bytes_renderer);
    red.command("exit");
    blue.command("exit");
    let _ = done.send(());
}
