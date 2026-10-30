#![cfg(all(windows, feature = "native-video-smoke"))]

use std::io::{BufRead, BufReader, Write};
use std::num::NonZeroUsize;
use std::os::windows::process::CommandExt;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex, Weak, mpsc};
use std::time::{Duration, Instant};

use golden_core::application::apply_project_intent;
use golden_core::ui_sync::UiEditIntent;
use golden_video::native::{D3d11Config, InputLimits, OutputBinding, adapters};
use golden_video::plan::PlanLimits;
use golden_video::runtime::{OutcomeKind, ResourceLimits, RuntimeConfig, VideoRuntime};
use golden_video::{AlphaMode, ColorEncoding, Extent2D, OutputId, PixelFormat, TextureDescriptor};
use mapgyver::app::{AppEngine, AppNode};
use mapgyver::media::protocol::*;
use mapgyver::media::rendering::{ChannelRenderer, ChannelRoute};
use mapgyver::media::resolution::ChannelAvailability;
use mapgyver::media::{capture_channels, channel};
use windows::Win32::System::Threading::CREATE_NO_WINDOW;

const WAIT: Duration = Duration::from_secs(8);

struct Producer {
    child: Arc<Mutex<Child>>,
    stdin: ChildStdin,
    replies: mpsc::Receiver<String>,
}
impl Producer {
    fn new(adapter: usize, sender: &str, registry: &Mutex<Vec<Weak<Mutex<Child>>>>) -> Self {
        let executable =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/native_source_fixture.exe");
        assert!(
            executable.exists(),
            "build native_source_fixture before the explicit native channel test"
        );
        let mut child = Command::new(executable)
            .creation_flags(CREATE_NO_WINDOW.0)
            .args(["--adapter", &adapter.to_string(), "--sender", sender])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, replies) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        let child = Arc::new(Mutex::new(child));
        registry.lock().unwrap().push(Arc::downgrade(&child));
        let producer = Self { child, stdin, replies };
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
        writeln!(self.stdin, "{command}").unwrap();
        self.stdin.flush().unwrap();
        self.reply();
    }
}
impl Drop for Producer {
    fn drop(&mut self) {
        let mut child = self.child.lock().unwrap_or_else(|error| error.into_inner());
        let _ = child.kill();
        let _ = child.wait();
    }
}

struct OwnedRenderer(Option<ChannelRenderer>);
impl Drop for OwnedRenderer {
    fn drop(&mut self) {
        if let Some(mut renderer) = self.0.take() {
            let retirement = renderer.retirement();
            renderer.close();
            drop(renderer);
            while !retirement.wait(Duration::from_millis(20)) {}
        }
    }
}

fn wait(label: &str, mut check: impl FnMut() -> bool) {
    let deadline = Instant::now() + WAIT;
    while !check() {
        assert!(Instant::now() < deadline, "native channel deadline: {label}");
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn config() -> RuntimeConfig {
    RuntimeConfig {
        plan_limits: PlanLimits {
            max_passes: 16,
            max_external_textures: 16,
            max_outputs: 4,
            max_dependencies: 64,
            max_texture_dimension: 16,
            max_texture_bytes: 65536,
        },
        resource_limits: ResourceLimits {
            max_bytes: 65536,
            max_in_flight: NonZeroUsize::new(2).unwrap(),
            max_retiring: 2,
        },
        frame_rate: golden_time::RationalRate::new(60, 1).unwrap(),
        max_outstanding: NonZeroUsize::new(16).unwrap(),
        max_input_bytes: NonZeroUsize::new(65536).unwrap(),
        max_retained_input_bytes: NonZeroUsize::new(262144).unwrap(),
        recovery_delay: Duration::from_millis(50),
        max_recovery_delay: Duration::from_secs(1),
        notifier: None,
    }
}
fn descriptor(width: u32, height: u32) -> TextureDescriptor {
    TextureDescriptor {
        extent: Extent2D::new(width, height).unwrap(),
        format: PixelFormat::Bgra8Unorm,
        color_encoding: ColorEncoding::Srgb,
        alpha_mode: AlphaMode::Opaque,
    }
}
fn renderer(adapter: golden_video::native::AdapterId, name: &str) -> OwnedRenderer {
    let config = config();
    let runtime = VideoRuntime::spawn_d3d11(
        config.clone(),
        D3d11Config {
            diagnostics: None,
            adapter,
            inputs: InputLimits {
                max_sources: 4,
                max_storage_bytes: 4 * InputLimits::source_storage_bytes(descriptor(4, 4)).unwrap(),
            },
            outputs: (1..=2)
                .map(|id| OutputBinding::Spout {
                    id: OutputId::new(id).unwrap(),
                    name: format!("{name}-{id}"),
                    descriptor: descriptor(2, 2),
                })
                .collect(),
        },
    )
    .unwrap();
    let renderer = ChannelRenderer::new(runtime, config.plan_limits);
    wait("native startup", || renderer.snapshot().resources.is_some());
    OwnedRenderer(Some(renderer))
}
fn edit(engine: &mut AppEngine, edit: MediaEdit) -> MediaEditResult {
    let ack = apply_project_intent(
        engine,
        UiEditIntent::InvokeAppOperation {
            namespace: MEDIA_NAMESPACE.into(),
            operation: MEDIA_OPERATION.into(),
            payload: serde_json::to_value(edit).unwrap(),
        },
    );
    assert!(ack.success, "{ack:?}");
    serde_json::from_value(ack.result.unwrap().value).unwrap()
}
fn configure(engine: &mut AppEngine, reference: ChannelReference, sender: &str) {
    let mut definition = channel(engine, reference).unwrap().definition;
    definition.source = ChannelSource::Spout {
        sender: Some(sender.into()),
        maximum: descriptor(4, 4),
    };
    edit(engine, MediaEdit::ConfigureChannel { reference, definition });
}
fn routes(references: [ChannelReference; 2]) -> Vec<ChannelRoute> {
    references
        .into_iter()
        .enumerate()
        .map(|(index, channel)| ChannelRoute {
            output: OutputId::new(index as u64 + 1).unwrap(),
            channel,
        })
        .collect()
}
fn publish(renderer: &mut ChannelRenderer, engine: &AppEngine, references: [ChannelReference; 2]) {
    renderer
        .publish(capture_channels(engine, &references, 16).unwrap(), routes(references))
        .unwrap();
}
fn ready(renderer: &mut ChannelRenderer, extents: [Extent2D; 2]) {
    let mut installed = None;
    wait("resolved channel resources", || {
        if let Err(error) = renderer.refresh() {
            assert!(error.contains("source observations"), "{error}");
        }
        let mut outcomes = Vec::new();
        renderer.drain_outcomes(&mut outcomes, NonZeroUsize::new(16).unwrap());
        for outcome in outcomes {
            match outcome.result {
                OutcomeKind::Installed { generation, .. } => installed = Some((outcome.request, generation.identity)),
                OutcomeKind::Rejected(failure) => assert!(
                    matches!(failure.code(), golden_video::runtime::FailureCode::StaleSource),
                    "{failure:?}"
                ),
                _ => {}
            }
        }
        renderer.channels().len() == 2
            && renderer.channels().iter().zip(extents).all(|(channel, extent)| {
                channel.resolution.availability == ChannelAvailability::Ready
                    && channel.resolution.extent == Some(extent)
            })
            && installed.is_some_and(|(request, identity)| {
                Some(request) == renderer.latest_request()
                    && renderer
                        .snapshot()
                        .last_frame
                        .is_some_and(|frame| frame.identity == identity)
            })
    });
}

#[test]
#[ignore = "requires hardware D3D11 and built native_source_fixture"]
fn authored_channels_publish_resize_reconnect_reject_stale_observations_and_recreate_resources() {
    let children = Arc::new(Mutex::new(Vec::<Weak<Mutex<Child>>>::new()));
    let watched = children.clone();
    let (done, finished) = mpsc::channel();
    std::thread::spawn(move || {
        if finished.recv_timeout(Duration::from_secs(60)).is_err() {
            for child in watched.lock().unwrap().iter().filter_map(Weak::upgrade) {
                let _ = child.lock().unwrap().kill();
            }
            eprintln!("native channel watchdog expired");
            std::process::exit(1);
        }
    });
    let (index, adapter) = adapters()
        .unwrap()
        .into_iter()
        .enumerate()
        .find(|(_, adapter)| !adapter.software)
        .unwrap();
    let identity = uuid::Uuid::new_v4();
    let names = [
        format!("MapGyverChannelA-{identity}"),
        format!("MapGyverChannelB-{identity}"),
    ];
    let mut a = Producer::new(index, &names[0], &children);
    let mut b = Producer::new(index, &names[1], &children);
    a.command("start 2 2 1 0 0");
    b.command("start 3 2 0 0 1");
    let mut engine = golden_core::app::create_new_project_engine::<AppNode>().unwrap();
    engine.apply_edits().unwrap();
    let MediaEditResult::Created { reference: first } = edit(
        &mut engine,
        MediaEdit::CreateMedia {
            media_kind: MediaKind::SharedTexture,
            label: None,
        },
    ) else {
        panic!()
    };
    let MediaEditResult::Created { reference: second } = edit(
        &mut engine,
        MediaEdit::AddChannel {
            media: first.media,
            label: None,
        },
    ) else {
        panic!()
    };
    let references = [first, second];
    configure(&mut engine, first, &names[0]);
    configure(&mut engine, second, &names[1]);
    let mut owned = renderer(adapter.id, &format!("MapGyverOutput-{identity}"));
    let live = owned.0.as_mut().unwrap();
    publish(live, &engine, references);
    ready(live, [Extent2D::new(2, 2).unwrap(), Extent2D::new(3, 2).unwrap()]);
    let original = live.channels().to_vec();
    let device = live.snapshot().resources.unwrap().last_submitted.device;
    let old = live.source_observations();
    configure(&mut engine, first, &names[1]);
    publish(live, &engine, references);
    ready(live, [Extent2D::new(3, 2).unwrap(); 2]);
    assert_ne!(live.channels()[0].source, original[0].source);
    assert_eq!(live.channels()[0].target, original[0].target);
    assert!(
        live.publish_observed(
            capture_channels(&engine, &references, 16).unwrap(),
            routes(references),
            old
        )
        .is_err()
    );
    let before_resize = live.source_observations();
    b.command("start 4 3 0 0 1");
    ready(live, [Extent2D::new(4, 3).unwrap(); 2]);
    assert!(
        live.publish_observed(
            capture_channels(&engine, &references, 16).unwrap(),
            routes(references),
            before_resize
        )
        .is_err()
    );
    engine.undo().unwrap();
    publish(live, &engine, references);
    ready(live, [Extent2D::new(2, 2).unwrap(), Extent2D::new(4, 3).unwrap()]);
    b.command("stop");
    wait("missing sender", || {
        let _ = live.refresh();
        live.channels()[1].resolution.availability == ChannelAvailability::Unavailable
    });
    b.command("start 3 2 0 0 1");
    ready(live, [Extent2D::new(2, 2).unwrap(), Extent2D::new(3, 2).unwrap()]);
    assert_eq!(live.snapshot().resources.unwrap().last_submitted.device, device);
    assert!(
        live.channels()
            .iter()
            .zip(references)
            .all(|(channel, reference)| channel.reference == reference)
    );
    let old_project = live.snapshot().project;
    let old_observations = live.source_observations();
    let json = golden_core::app::to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = golden_core::app::from_sparse_project_json::<AppNode>(&json).unwrap();
    loaded.apply_edits().unwrap();
    publish(live, &loaded, references);
    ready(live, [Extent2D::new(2, 2).unwrap(), Extent2D::new(3, 2).unwrap()]);
    assert_ne!(live.snapshot().project, old_project);
    assert!(
        live.publish_observed(
            capture_channels(&loaded, &references, 16).unwrap(),
            routes(references),
            old_observations.clone()
        )
        .is_err()
    );
    let final_frames = live.snapshot().frames_submitted;
    eprintln!(
        "app channel publication passed LUID{:08x}:{:08x}, frames={final_frames}, channels={:?}",
        adapter.id.high as u32,
        adapter.id.low,
        live.channels()
    );
    drop(owned);
    let mut recreated = renderer(adapter.id, &format!("MapGyverRecreated-{identity}"));
    let live = recreated.0.as_mut().unwrap();
    publish(live, &loaded, references);
    ready(live, [Extent2D::new(2, 2).unwrap(), Extent2D::new(3, 2).unwrap()]);
    assert!(
        live.publish_observed(
            capture_channels(&loaded, &references, 16).unwrap(),
            routes(references),
            old_observations
        )
        .is_err()
    );
    assert!(
        live.channels()
            .iter()
            .zip(references)
            .all(|(channel, reference)| channel.reference == reference)
    );
    drop(recreated);
    a.command("exit");
    b.command("exit");
    let _ = done.send(());
}
