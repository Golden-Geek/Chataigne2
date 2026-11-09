use super::*;
use golden_io::PayloadPool;
use golden_video::{
    AlphaMode, ColorEncoding, PixelFormat, TextureDescriptor,
    native::{AdapterId, D3d11Config, GpuFrameBudget, InputLimits, OutputBinding, SpoutProbe, adapters},
    runtime::DeviceGeneration,
};

const NAMES: [&str; 8] = [
    "h264-color.mp4",
    "hevc-main.mp4",
    "hevc-main10.mp4",
    "prores.mov",
    "prores-alpha.mov",
    "hap.mov",
    "hap-alpha.mov",
    "hap-q.mov",
];

struct Owner {
    engine: AppEngine,
    renderer: Option<ChannelRenderer>,
    adapter: AdapterId,
    references: Vec<ChannelReference>,
    senders: Vec<String>,
    budget: GpuFrameBudget,
    original: DeviceGeneration,
    zero: Vec<[u8; 4]>,
    fourth: Vec<[u8; 4]>,
    authored: String,
    history: usize,
}

impl Owner {
    fn new(adapter: AdapterId) -> Self {
        let mut engine = engine();
        let references: Vec<_> = NAMES.iter().map(|name| source(&mut engine, name)).collect();
        for reference in &references {
            edit(
                &mut engine,
                MediaEdit::ConfigureMediaResolution {
                    media: reference.media,
                    size: SizePolicy::Fixed {
                        extent: Extent2D::new(16, 16).unwrap(),
                    },
                },
            );
        }
        let senders: Vec<_> = NAMES
            .iter()
            .enumerate()
            .map(|(index, _)| format!("MapGyverVideoReset-{}-{index}", uuid::Uuid::new_v4()))
            .collect();
        let config = config();
        let descriptor = TextureDescriptor {
            extent: Extent2D::new(16, 16).unwrap(),
            format: PixelFormat::Bgra8Unorm,
            color_encoding: ColorEncoding::Srgb,
            alpha_mode: AlphaMode::Straight,
        };
        let mut renderer = ChannelRenderer::new(
            VideoRuntime::spawn_d3d11(
                config.clone(),
                D3d11Config {
                    adapter,
                    diagnostics: None,
                    inputs: InputLimits {
                        max_sources: 0,
                        max_storage_bytes: 0,
                    },
                    output_rates: Vec::new(),
                    outputs: senders
                        .iter()
                        .enumerate()
                        .map(|(index, name)| OutputBinding::Spout {
                            id: OutputId::new(index as u64 + 1).unwrap(),
                            name: name.clone(),
                            descriptor,
                        })
                        .collect(),
                },
            )
            .unwrap(),
            config.plan_limits,
        );
        let budget = GpuFrameBudget::new(512 * 1024 * 1024);
        renderer
            .enable_hardware_videos(&fixture_root(), None, adapter, budget.clone())
            .unwrap();
        wait(|| renderer.snapshot().resources.is_some());
        let original = renderer.snapshot().resources.unwrap().last_submitted.device;
        renderer
            .publish(
                capture_channels(&engine, &references, 16).unwrap(),
                references
                    .iter()
                    .enumerate()
                    .map(|(index, channel)| ChannelRoute {
                        output: OutputId::new(index as u64 + 1).unwrap(),
                        channel: *channel,
                    })
                    .collect(),
            )
            .unwrap();
        let mut owner = Self {
            engine,
            renderer: Some(renderer),
            adapter,
            references,
            senders,
            budget,
            original,
            zero: Vec::new(),
            fourth: Vec::new(),
            authored: String::new(),
            history: 0,
        };
        owner.settle();
        owner.seek(4);
        owner.fourth = owner.pixels(None);
        owner.seek(0);
        owner.zero = owner.pixels(None);
        assert!(
            owner
                .zero
                .iter()
                .zip(&owner.fourth)
                .all(|(zero, fourth)| zero != fourth)
        );
        owner.assert_routes();
        assert_eq!(owner.live().video_retirement().unwrap().active_workers(), 8);
        owner
    }

    fn live(&self) -> &ChannelRenderer {
        self.renderer.as_ref().unwrap()
    }

    fn assert_routes(&self) {
        for (index, reference) in self.references.iter().enumerate() {
            let (_, source) = self.live().videos.as_ref().unwrap().source(*reference).unwrap();
            assert_eq!(
                matches!(source, golden_video::plan::ExternalSource::GpuFrames(_)),
                !NAMES[index].starts_with("prores"),
                "{} on {:?}: hardware/BC route must survive recovery",
                NAMES[index],
                self.adapter
            );
        }
    }

    fn play(&mut self) {
        for reference in &self.references {
            for operation in [VideoEdit::Loop { looping: true }, VideoEdit::Play] {
                edit(
                    &mut self.engine,
                    MediaEdit::Video {
                        media: reference.media,
                        edit: operation,
                    },
                );
            }
        }
        self.authored = golden_core::app::to_sparse_project_json_pretty(&self.engine).unwrap();
        self.history = self.engine.undo_len();
    }

    fn unchanged(&mut self) {
        assert_eq!(
            golden_core::app::to_sparse_project_json_pretty(&self.engine).unwrap(),
            self.authored,
            "device recovery must preserve all authored files/transport/geometry"
        );
        assert_eq!(self.engine.undo_len(), self.history);
        for reference in &self.references {
            let MediaEditResult::Video { snapshot } =
                edit(&mut self.engine, MediaEdit::InspectVideo { media: reference.media })
            else {
                panic!()
            };
            assert!(snapshot.playing && snapshot.settings.looping);
            assert_eq!(
                snapshot.settings.mode,
                crate::media::video::protocol::VideoPlaybackMode::Playing
            );
        }
    }

    fn pump(&mut self) {
        let renderer = self.renderer.as_mut().unwrap();
        let mut outcomes = Vec::new();
        renderer.drain_outcomes(&mut outcomes, NonZeroUsize::new(16).unwrap());
        if let Err(error) = renderer.refresh() {
            assert!(error.contains("source observations"), "{error}");
        }
    }

    fn settle(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            self.pump();
            let runtime = self.live().snapshot();
            if runtime.state == golden_video::runtime::RuntimeState::Running
                && self.live().channels().len() == 8
                && self.live().channels().iter().all(|channel| {
                    channel.resolution.availability == crate::media::resolution::ChannelAvailability::Ready
                })
                && runtime.resources.is_some_and(|resources| resources.active.is_some())
                && self.live().native_control().unwrap().output_snapshots().len() == 8
                && self
                    .live()
                    .native_control()
                    .unwrap()
                    .output_snapshots()
                    .iter()
                    .all(|output| {
                        output.last_accepted.is_some_and(|accepted| {
                            Some(accepted.device) == runtime.resources.map(|resources| resources.last_submitted.device)
                        })
                    })
            {
                return;
            }
            assert!(Instant::now() < deadline, "Video reset preparation: {runtime:?}");
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    fn seek(&mut self, index: u32) {
        for (channel, reference) in self.references.iter().enumerate() {
            let rate = match NAMES[channel] {
                "hevc-main.mp4" | "hevc-main10.mp4" | "hap-q.mov" => 30000.0 / 1001.0,
                "prores.mov" | "prores-alpha.mov" | "hap-alpha.mov" => 25.0,
                _ => 24.0,
            };
            let MediaEditResult::Video { snapshot } = edit(
                &mut self.engine,
                MediaEdit::Video {
                    media: reference.media,
                    edit: VideoEdit::Pause,
                },
            ) else {
                panic!()
            };
            assert!(!snapshot.playing, "static pixel checks require an explicit Pause");
            edit(
                &mut self.engine,
                MediaEdit::Video {
                    media: reference.media,
                    edit: VideoEdit::Seek {
                        position_ms: f64::from(index) * 1000.0 / rate,
                    },
                },
            );
            steps::at(&mut self.engine, *reference, f64::from(index) * 1000.0 / rate);
        }
        self.settle();
        let submitted = self.live().snapshot().frames_submitted;
        wait(|| {
            let snapshot = self.live().snapshot();
            snapshot.frames_submitted >= submitted + 3 && snapshot.last_frame.is_some()
        });
    }

    fn pixels(&mut self, expected: Option<&[[u8; 4]]>) -> Vec<[u8; 4]> {
        let budget = PayloadPool::new("Video reset output probes", 8, 1024 * 1024).unwrap();
        let mut values = Vec::new();
        for (index, sender) in self.senders.clone().iter().enumerate() {
            let mut probe = SpoutProbe::new(sender, self.adapter, budget.clone()).unwrap();
            let deadline = Instant::now() + Duration::from_secs(8);
            let mut last = None;
            loop {
                self.pump();
                if let Some(image) = probe.capture_pixels().unwrap() {
                    let pixel = image.pixels()[8 * 16 + 8];
                    last = Some(pixel);
                    if expected.is_none_or(|expected| {
                        pixel
                            .iter()
                            .zip(expected[index])
                            .all(|(actual, wanted)| actual.abs_diff(wanted) <= 2)
                    }) {
                        values.push(pixel);
                        break;
                    }
                }
                if Instant::now() >= deadline {
                    let observed = edit(
                        &mut self.engine,
                        MediaEdit::InspectVideoFrame {
                            reference: self.references[index],
                        },
                    );
                    panic!(
                        "Video reset output {index}: actual={last:?}, expected={expected:?}, frame={observed:?}, runtime={:?}",
                        self.live().snapshot()
                    );
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        assert!(budget.wait_for_idle(Duration::from_secs(1)));
        values
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        if let Some(mut renderer) = self.renderer.take() {
            let video = renderer.video_retirement().unwrap();
            let retirement = renderer.retirement();
            let deadline = Instant::now() + Duration::from_secs(5);
            renderer.close();
            drop(renderer);
            assert!(retirement.wait(deadline.saturating_duration_since(Instant::now())));
            assert!(video.wait(deadline.saturating_duration_since(Instant::now())));
            assert_eq!(video.active_workers(), 0);
            assert_eq!(video.retained_bytes(), 0);
            assert_eq!(self.budget.retained_bytes(), 0);
        }
    }
}

fn run(genuine: bool) {
    if genuine {
        assert_eq!(
            std::env::var("GOLDEN_VIDEO_EXTERNAL_DEVICE_REMOVAL").as_deref(),
            Ok("1")
        );
    }
    let mut owners: Vec<_> = adapters()
        .unwrap()
        .into_iter()
        .filter(|adapter| !adapter.software)
        .map(|adapter| Owner::new(adapter.id))
        .collect();
    assert_eq!(owners.len(), 2, "both local GPUs required");
    for owner in &mut owners {
        owner.play();
    }
    eprintln!(
        "READY_EXTERNAL_DEVICE_REMOVAL pid={}: eight Video sources/output pixels on each GPU",
        std::process::id()
    );
    if genuine {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            for owner in &mut owners {
                owner.pump();
            }
            if owners.iter().any(|owner| {
                owner
                    .live()
                    .snapshot()
                    .resources
                    .is_some_and(|resources| resources.last_submitted.device != owner.original)
            }) {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "no genuine renderer device-generation change"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    for owner in &mut owners {
        owner.settle();
        owner.unchanged();
        let current = owner.live().snapshot().resources.unwrap().last_submitted.device;
        let affected = current != owner.original;
        if affected {
            assert!(current.generation() > owner.original.generation());
            assert!(owner.live().snapshot().failure.is_some());
        }
        owner.seek(4);
        let fourth = owner.fourth.clone();
        let actual = owner.pixels(Some(&fourth));
        assert!(actual.iter().zip(&owner.zero).all(|(pixel, zero)| pixel != zero));
        owner.seek(0);
        let zero = owner.zero.clone();
        owner.pixels(Some(&zero));
        owner.assert_routes();
        assert_eq!(owner.live().video_retirement().unwrap().active_workers(), 8);
        assert_eq!(owner.live().snapshot().resources.unwrap().retiring_generations, 0);
        eprintln!(
            "Video device recovery {:?}: affected={affected}, old={:?}, current={current:?}, eight fresh outputs={actual:?}, GPU bytes={}",
            owner.adapter,
            owner.original,
            owner.budget.retained_bytes()
        );
    }
    let closing = Instant::now();
    drop(owners);
    assert!(closing.elapsed() < Duration::from_secs(5));
    eprintln!(
        "Video device recovery: both renderer/file owners fully retired, zero CPU/GPU admission, shutdown={:?}",
        closing.elapsed()
    );
}

#[test]
#[ignore = "explicit both-GPU Video/native output preflight"]
fn hardware_video_recovery_preflight_without_device_removal() {
    run(false);
}

#[test]
#[ignore = "requires an explicitly authorized external driver reset after readiness"]
fn hardware_video_recovery_after_genuine_device_removal() {
    run(true);
}
