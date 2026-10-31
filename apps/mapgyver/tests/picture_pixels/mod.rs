//! Fixed M04 Picture gate: real app preparation, native output pixels and asset lifecycle.
use super::*;
use golden_io::PayloadPool;
use golden_video::native::SpoutProbe;
use std::sync::mpsc;

const MIB: usize = 1024 * 1024;
const DEADLINE: Duration = Duration::from_secs(8);
const PATTERN: [[u8; 4]; 4] = [[255, 0, 0, 128], [0, 255, 0, 255], [0, 0, 255, 64], [0, 0, 0, 0]];

fn source_png(path: &Path, width: u32, height: u32, pixels: &[u8], bad_gamma: bool) {
    let mut encoder = png::Encoder::new(std::fs::File::create(path).unwrap(), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    if bad_gamma {
        encoder.set_source_gamma(png::ScaledFloat::new(1.0));
    }
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(pixels).unwrap();
    writer.finish().unwrap();
}

fn jpeg(path: &Path) {
    image::codecs::jpeg::JpegEncoder::new_with_quality(std::fs::File::create(path).unwrap(), 100)
        .write_image(&[0, 0, 255].repeat(5 * 3), 5, 3, image::ExtendedColorType::Rgb8)
        .unwrap();
}

struct Fixture {
    renderer: OwnedRenderer,
    names: [String; 3],
    extents: [Extent2D; 2],
}
impl Fixture {
    fn new(adapter: AdapterId, root: &Path, first: Extent2D) -> Self {
        let extents = [first, Extent2D::new(3, 2).unwrap()];
        let names = std::array::from_fn(|index| format!("MapGyverPicturePixels-{}-{index}", uuid::Uuid::new_v4()));
        let limits = PlanLimits {
            max_passes: 8,
            max_external_textures: 4,
            max_outputs: 3,
            max_dependencies: 64,
            max_texture_dimension: 8192,
            max_texture_bytes: 256 * MIB as u64,
        };
        let config = RuntimeConfig {
            plan_limits: limits,
            resource_limits: ResourceLimits {
                max_bytes: 512 * MIB as u64,
                max_in_flight: NonZeroUsize::new(2).unwrap(),
                max_retiring: 2,
            },
            frame_rate: golden_time::RationalRate::new(60, 1).unwrap(),
            max_outstanding: NonZeroUsize::new(8).unwrap(),
            max_input_bytes: NonZeroUsize::new(257 * MIB).unwrap(),
            max_retained_input_bytes: NonZeroUsize::new(260 * MIB).unwrap(),
            recovery_delay: Duration::from_millis(50),
            max_recovery_delay: Duration::from_secs(1),
            notifier: None,
        };
        let outputs = names
            .iter()
            .zip([extents[0], extents[0], extents[1]])
            .enumerate()
            .map(|(index, (name, extent))| OutputBinding::Spout {
                id: OutputId::new(index as u64 + 1).unwrap(),
                name: name.clone(),
                descriptor: TextureDescriptor {
                    extent,
                    format: PixelFormat::Bgra8Unorm,
                    color_encoding: ColorEncoding::Srgb,
                    alpha_mode: AlphaMode::Straight,
                },
            })
            .collect();
        let mut renderer = ChannelRenderer::new(
            VideoRuntime::spawn_d3d11(
                config,
                D3d11Config {
                    adapter,
                    diagnostics: None,
                    inputs: InputLimits {
                        max_sources: 0,
                        max_storage_bytes: 0,
                    },
                    outputs,
                },
            )
            .unwrap(),
            limits,
        );
        renderer
            .enable_pictures(
                root,
                PreparationLimits {
                    max_source_bytes: 64 * MIB as u64,
                    max_decode_bytes: NonZeroUsize::new(128 * MIB).unwrap(),
                    max_prepared_bytes: NonZeroUsize::new(256 * MIB).unwrap(),
                    ..Default::default()
                },
                None,
            )
            .unwrap();
        let deadline = Instant::now() + DEADLINE;
        while renderer.snapshot().resources.is_none() {
            assert!(Instant::now() < deadline, "Picture renderer startup timeout");
            std::thread::sleep(Duration::from_millis(5));
        }
        Self {
            renderer: OwnedRenderer(Some(renderer)),
            names,
            extents,
        }
    }
    fn live(&mut self) -> &mut ChannelRenderer {
        self.renderer.0.as_mut().unwrap()
    }
    fn publish(&mut self, engine: &AppEngine, references: [ChannelReference; 2]) {
        self.live()
            .publish(
                capture_channels(engine, &references, 8).unwrap(),
                [references[0], references[0], references[1]]
                    .into_iter()
                    .enumerate()
                    .map(|(index, channel)| ChannelRoute {
                        output: OutputId::new(index as u64 + 1).unwrap(),
                        channel,
                    })
                    .collect(),
            )
            .unwrap();
    }
    fn settle(
        &mut self,
        expected: [(ChannelAvailability, Extent2D); 2],
        healthy: Option<mapgyver::media::rendering::PreparedChannel>,
    ) {
        let deadline = Instant::now() + DEADLINE;
        let mut installed = None;
        let mut accepted = None;
        loop {
            let live = self.live();
            if let Err(error) = live.refresh() {
                assert!(error.contains("source observations"), "{error}");
            }
            let mut outcomes = Vec::new();
            live.drain_outcomes(&mut outcomes, NonZeroUsize::new(16).unwrap());
            for outcome in outcomes {
                match outcome.result {
                    OutcomeKind::Installed { generation, .. } => {
                        installed = Some((outcome.request, generation.identity));
                        accepted = None;
                    }
                    OutcomeKind::Rejected(error) => assert_eq!(
                        error.code(),
                        golden_video::runtime::FailureCode::StaleSource,
                        "{error:?}"
                    ),
                    _ => {}
                }
            }
            if let Some(healthy) = healthy {
                let current = live
                    .channels()
                    .iter()
                    .find(|channel| channel.reference == healthy.reference)
                    .unwrap();
                assert_eq!(
                    *current, healthy,
                    "independent healthy source must not reset during peer recovery"
                );
            }
            let current = live.channels().len() == 2
                && live
                    .channels()
                    .iter()
                    .zip(expected)
                    .all(|(channel, (availability, extent))| {
                        channel.resolution.availability == availability && channel.resolution.extent == Some(extent)
                    });
            if current
                && let Some((request, identity)) = installed
                && Some(request) == live.latest_request()
                && let Some(frame) = live.snapshot().last_frame
                && frame.identity == identity
            {
                let token = *accepted.get_or_insert(frame.completion);
                let outputs = live.native_control().unwrap().output_snapshots();
                if outputs.len() == 3
                    && outputs.iter().all(|output| {
                        output
                            .last_accepted
                            .is_some_and(|last| last.device == token.device && last.serial >= token.serial)
                    })
                {
                    return;
                }
            }
            assert!(
                Instant::now() < deadline,
                "Picture settlement timeout: {:?}",
                live.channels()
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    fn uploaded(&mut self) -> u64 {
        self.live().native_control().unwrap().snapshot().uploaded_images
    }
    fn pixels(&self, adapter: AdapterId, expected: [&[[u8; 4]]; 2], label: &str) {
        let budget = PayloadPool::new("picture-pixel-qualification", 9, 192 * MIB).unwrap();
        let mut probes = self
            .names
            .iter()
            .map(|name| SpoutProbe::new(name, adapter, budget.clone()).unwrap())
            .collect::<Vec<_>>();
        for (index, probe) in probes.iter_mut().enumerate() {
            let deadline = Instant::now() + DEADLINE;
            let image = loop {
                if let Some(image) = probe.capture_pixels().unwrap() {
                    break image;
                }
                assert!(
                    Instant::now() < deadline,
                    "Picture pixel capture timeout: {label}/{index}"
                );
                std::thread::sleep(Duration::from_millis(5));
            };
            let channel = usize::from(index == 2);
            assert_eq!(image.extent(), self.extents[channel]);
            assert_eq!(
                image.pixels().len(),
                (image.extent().width() * image.extent().height()) as usize
            );
            let oracle = expected[channel];
            assert!(oracle.len() == 1 || oracle.len() == image.pixels().len());
            for (offset, pixel) in image.pixels().iter().enumerate() {
                let expected = oracle[offset % oracle.len()];
                assert!(
                    pixel
                        .iter()
                        .zip(expected)
                        .all(|(actual, wanted)| actual.abs_diff(wanted) <= 2),
                    "Picture {label}/{index} texel {offset}: {pixel:?}, expected {expected:?} tolerance2/255"
                );
            }
            eprintln!(
                "Picture {label} output={index} extent={:?} pixels={} sample={:?} tolerance2/255",
                image.extent(),
                image.pixels().len(),
                image.pixels()[0]
            );
        }
        drop(probes);
        assert!(budget.wait_for_idle(Duration::from_secs(1)));
    }
    fn close(self) {
        let retired = self.renderer.0.as_ref().unwrap().picture_retirement().unwrap();
        drop(self);
        assert!(retired.wait(DEADLINE));
    }
}

fn ready(width: u32, height: u32) -> (ChannelAvailability, Extent2D) {
    (ChannelAvailability::Ready, Extent2D::new(width, height).unwrap())
}

#[test]
#[ignore = "explicit M04 Picture color/alpha, asset lifecycle and 4K/native sharing qualification"]
fn authored_picture_pixels_recover_share_relink_replace_reject_color_and_reload_4k() {
    let (done, completion) = mpsc::channel();
    std::thread::spawn(move || {
        if matches!(
            completion.recv_timeout(Duration::from_secs(120)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ) {
            eprintln!("Picture pixel qualification/retirement watchdog expired");
            std::process::exit(1);
        }
    });
    let hardware = adapters()
        .unwrap()
        .into_iter()
        .filter(|adapter| !adapter.software)
        .collect::<Vec<_>>();
    assert!(!hardware.is_empty());
    for adapter in hardware {
        let root = tempfile::tempdir().unwrap();
        let pattern = [[255, 0, 0, 128], [0, 255, 0, 255], [0, 0, 255, 64], [255, 0, 255, 0]].concat();
        source_png(&root.path().join("pattern.png"), 2, 2, &pattern, false);
        let mut engine = golden_core::app::create_new_project_engine::<AppNode>().unwrap();
        engine.apply_edits().unwrap();
        let first_asset = import(&mut engine, "pattern.png");
        let second_asset = import(&mut engine, "missing.jpg");
        let MediaEditResult::Created { reference: first } = edit(
            &mut engine,
            MediaEdit::CreateMedia {
                media_kind: MediaKind::Picture,
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
        configure(&mut engine, first, first_asset);
        configure(&mut engine, second, second_asset);
        let mut definition = channel(&engine, second).unwrap().definition;
        definition.size = SizePolicy::Fixed {
            extent: Extent2D::new(3, 2).unwrap(),
        };
        edit(
            &mut engine,
            MediaEdit::ConfigureChannel {
                reference: second,
                definition,
            },
        );
        let references = [first, second];
        let mut fixture = Fixture::new(adapter.id, root.path(), Extent2D::new(2, 2).unwrap());
        fixture.publish(&engine, references);
        let missing = (ChannelAvailability::Unavailable, Extent2D::new(3, 2).unwrap());
        fixture.settle([ready(2, 2), missing], None);
        fixture.pixels(adapter.id, [&PATTERN, &[[0; 4]]], "missing");
        assert_eq!(fixture.uploaded(), 1);
        let healthy = fixture.live().channels()[0];
        jpeg(&root.path().join("missing.jpg"));
        fixture.settle([ready(2, 2), ready(3, 2)], Some(healthy));
        fixture.pixels(
            adapter.id,
            [&PATTERN, &[[0, 0, 255, 255]]],
            "automatic-missing-recovery",
        );
        assert_eq!(fixture.uploaded(), 2);

        configure(&mut engine, first, second_asset);
        fixture.publish(&engine, references);
        fixture.settle([ready(5, 3), ready(3, 2)], None);
        let channels = fixture.live().channels();
        assert_eq!(channels[0].source, channels[1].source);
        fixture.pixels(adapter.id, [&[[0, 0, 255, 255]], &[[0, 0, 255, 255]]], "shared-jpeg");
        assert_eq!(
            fixture.uploaded(),
            2,
            "shared consumers must not reupload a retained image"
        );
        assert!(engine.undo().unwrap());
        fixture.publish(&engine, references);
        fixture.settle([ready(2, 2), ready(3, 2)], None);
        fixture.pixels(adapter.id, [&PATTERN, &[[0, 0, 255, 255]]], "undo-source");
        let healthy = fixture.live().channels()[0];
        let uploads = fixture.uploaded();
        let replacement = root.path().join("replacement.png");
        source_png(&replacement, 3, 2, &[0, 255, 0, 128].repeat(6), false);
        let stale = capture_channels(&engine, &references, 8).unwrap();
        edit(
            &mut engine,
            MediaEdit::RelinkAsset {
                asset: second_asset,
                locator: relative("replacement.png"),
            },
        );
        assert!(!stale.is_current());
        fixture.publish(&engine, references);
        fixture.settle([ready(2, 2), ready(3, 2)], Some(healthy));
        fixture.pixels(adapter.id, [&PATTERN, &[[0, 255, 0, 128]]], "relink-alpha");
        assert_eq!(fixture.uploaded(), uploads + 1);
        let history = engine.undo_len();
        source_png(&replacement, 4, 3, &[255, 0, 0, 64].repeat(12), false);
        fixture.live().retry_picture(second_asset).unwrap();
        fixture.settle([ready(2, 2), ready(3, 2)], Some(healthy));
        fixture.pixels(adapter.id, [&PATTERN, &[[255, 0, 0, 64]]], "in-place-replacement");
        assert_eq!(
            engine.undo_len(),
            history,
            "file change recovery is not authored history"
        );
        assert_eq!(fixture.uploaded(), uploads + 2);
        source_png(&root.path().join("unsupported.png"), 1, 1, &[0, 255, 0, 255], true);
        edit(
            &mut engine,
            MediaEdit::RelinkAsset {
                asset: second_asset,
                locator: relative("unsupported.png"),
            },
        );
        fixture.publish(&engine, references);
        fixture.settle(
            [
                ready(2, 2),
                (ChannelAvailability::Incompatible, Extent2D::new(3, 2).unwrap()),
            ],
            Some(healthy),
        );
        fixture.pixels(adapter.id, [&PATTERN, &[[0; 4]]], "unsupported-gamma");
        edit(
            &mut engine,
            MediaEdit::RelinkAsset {
                asset: second_asset,
                locator: relative("missing.jpg"),
            },
        );
        fixture.publish(&engine, references);
        fixture.settle([ready(2, 2), ready(3, 2)], Some(healthy));
        fixture.pixels(adapter.id, [&PATTERN, &[[0, 0, 255, 255]]], "recovered-jpeg");
        fixture.close();

        source_png(
            &root.path().join("large.png"),
            3840,
            2160,
            &[0, 255, 255, 128].repeat(3840 * 2160),
            false,
        );
        edit(
            &mut engine,
            MediaEdit::RelinkAsset {
                asset: first_asset,
                locator: relative("large.png"),
            },
        );
        let document = golden_core::app::to_sparse_project_json_pretty(&engine).unwrap();
        let mut restored = golden_core::app::from_sparse_project_json::<AppNode>(&document).unwrap();
        restored.apply_edits().unwrap();
        let mut recreated = Fixture::new(adapter.id, root.path(), Extent2D::new(3840, 2160).unwrap());
        recreated.publish(&restored, references);
        recreated.settle([ready(3840, 2160), ready(3, 2)], None);
        recreated.pixels(
            adapter.id,
            [&[[0, 255, 255, 128]], &[[0, 0, 255, 255]]],
            "4k-reload-recreate",
        );
        let frames = recreated.live().snapshot().frames_submitted;
        let uploads = recreated.uploaded();
        let deadline = Instant::now() + DEADLINE;
        while recreated.live().snapshot().frames_submitted < frames + 3 {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(
            recreated.uploaded(),
            uploads,
            "CPU image upload must not recur per frame"
        );
        assert!(recreated.live().native_control().unwrap().sources().is_empty());
        assert_eq!(mapgyver::media::asset(&restored, first_asset).unwrap().id, first_asset);
        assert_eq!(
            mapgyver::media::asset(&restored, second_asset).unwrap().id,
            second_asset
        );
        recreated.close();
        eprintln!(
            "M04 Picture qualified adapter={:?} description={}",
            adapter.id, adapter.description
        );
    }
    done.send(()).unwrap();
}
