#![cfg(all(windows, feature = "native-video-smoke"))]

use std::num::NonZeroUsize;
use std::path::Path;
use std::time::{Duration, Instant};

use golden_assets::PreparationLimits;
use golden_core::application::apply_project_intent;
use golden_core::ui_sync::UiEditIntent;
use golden_video::native::{AdapterId, D3d11Config, InputLimits, OutputBinding, adapters};
use golden_video::plan::PlanLimits;
use golden_video::runtime::{OutcomeKind, ResourceLimits, RuntimeConfig, VideoRuntime};
use golden_video::{AlphaMode, ColorEncoding, Extent2D, OutputId, PixelFormat, TextureDescriptor};
use image::ImageEncoder;
use mapgyver::app::{AppEngine, AppNode};
use mapgyver::media::protocol::*;
use mapgyver::media::rendering::{ChannelRenderer, ChannelRoute};
use mapgyver::media::resolution::ChannelAvailability;
use mapgyver::media::{capture_channels, channel};

const WAIT: Duration = Duration::from_secs(5);

struct OwnedRenderer(Option<ChannelRenderer>);
impl Drop for OwnedRenderer {
    fn drop(&mut self) {
        if let Some(renderer) = self.0.take() {
            let video = renderer.retirement();
            let pictures = renderer.picture_retirement();
            renderer.close();
            drop(renderer);
            while !video.wait(Duration::from_millis(20)) {}
            if let Some(pictures) = pictures {
                while !pictures.wait(Duration::from_millis(20)) {}
            }
        }
    }
}

fn wait(label: &str, mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + WAIT;
    while !ready() {
        assert!(Instant::now() < deadline, "native Picture test deadline: {label}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn runtime_config() -> RuntimeConfig {
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
        max_input_bytes: NonZeroUsize::new(1024 * 1024).unwrap(),
        max_retained_input_bytes: NonZeroUsize::new(4 * 1024 * 1024).unwrap(),
        recovery_delay: Duration::from_millis(50),
        max_recovery_delay: Duration::from_secs(1),
        notifier: None,
    }
}

fn renderer(adapter: AdapterId, directory: &Path) -> OwnedRenderer {
    let config = runtime_config();
    let identity = uuid::Uuid::new_v4();
    let runtime = VideoRuntime::spawn_d3d11(
        config.clone(),
        D3d11Config {
            diagnostics: None,
            adapter,
            inputs: InputLimits {
                max_sources: 0,
                max_storage_bytes: 0,
            },
            outputs: (1..=2)
                .map(|id| OutputBinding::Spout {
                    id: OutputId::new(id).unwrap(),
                    name: format!("MapGyverPicture-{identity}-{id}"),
                    descriptor: TextureDescriptor {
                        extent: Extent2D::new(2, 2).unwrap(),
                        format: PixelFormat::Bgra8Unorm,
                        color_encoding: ColorEncoding::Srgb,
                        alpha_mode: AlphaMode::Opaque,
                    },
                })
                .collect(),
        },
    )
    .unwrap();
    let mut renderer = ChannelRenderer::new(runtime, config.plan_limits);
    renderer
        .enable_pictures(
            directory,
            PreparationLimits {
                max_source_bytes: 64 * 1024,
                max_decode_bytes: NonZeroUsize::new(128 * 1024).unwrap(),
                max_prepared_items: NonZeroUsize::new(8).unwrap(),
                max_prepared_bytes: NonZeroUsize::new(4 * 1024 * 1024).unwrap(),
                ..Default::default()
            },
            None,
        )
        .unwrap();
    wait("native startup", || renderer.snapshot().resources.is_some());
    OwnedRenderer(Some(renderer))
}

fn png(path: &Path, width: u32, height: u32, pixel: [u8; 4]) {
    let pixels = pixel.repeat((width * height) as usize);
    image::codecs::png::PngEncoder::new(std::fs::File::create(path).unwrap())
        .write_image(&pixels, width, height, image::ExtendedColorType::Rgba8)
        .unwrap();
}

fn relative(path: &str) -> AssetLocator {
    AssetLocator::ProjectRelative {
        path: path.to_string().try_into().unwrap(),
    }
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

fn import(engine: &mut AppEngine, path: &str) -> AssetId {
    let MediaEditResult::AssetCreated { asset } = edit(
        engine,
        MediaEdit::ImportAsset {
            locator: relative(path),
            label: None,
        },
    ) else {
        panic!("expected created Picture asset")
    };
    asset
}

fn configure(engine: &mut AppEngine, reference: ChannelReference, asset: AssetId) {
    let mut definition = channel(engine, reference).unwrap().definition;
    definition.source = ChannelSource::Picture { asset: Some(asset) };
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
    wait("prepared Picture extent/latest installation/frame", || {
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
#[ignore = "requires explicit hardware D3D11 and native Spout output availability"]
fn authored_picture_assets_decode_upload_share_relink_reload_and_recreate_native_resources() {
    let (done, finished) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        if finished.recv_timeout(Duration::from_secs(60)).is_err() {
            eprintln!("native Picture test watchdog expired before verified retirement");
            std::process::exit(1);
        }
    });
    let adapter = adapters()
        .unwrap()
        .into_iter()
        .find(|adapter| !adapter.software)
        .unwrap();
    let directory = tempfile::tempdir().unwrap();
    png(&directory.path().join("red.png"), 2, 2, [255, 0, 0, 255]);
    png(&directory.path().join("blue.png"), 3, 2, [0, 0, 255, 255]);
    png(&directory.path().join("replacement.png"), 4, 3, [0, 255, 0, 128]);
    let mut engine = golden_core::app::create_new_project_engine::<AppNode>().unwrap();
    engine.apply_edits().unwrap();
    let first_asset = import(&mut engine, "red.png");
    let second_asset = import(&mut engine, "blue.png");
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
    let references = [first, second];
    configure(&mut engine, first, first_asset);
    configure(&mut engine, second, second_asset);
    let mut owned = renderer(adapter.id, directory.path());
    let live = owned.0.as_mut().unwrap();
    publish(live, &engine, references);
    ready(live, [Extent2D::new(2, 2).unwrap(), Extent2D::new(3, 2).unwrap()]);
    let native = live.native_control().unwrap();
    assert!(
        native.sources().is_empty(),
        "Picture inputs must not spawn Spout receivers"
    );
    assert!(native.snapshot().uploaded_images >= 2);
    let independent_sources = [live.channels()[0].source, live.channels()[1].source];
    assert_ne!(independent_sources[0], independent_sources[1]);
    let before_share = native.snapshot().uploaded_images;
    configure(&mut engine, second, first_asset);
    publish(live, &engine, references);
    ready(live, [Extent2D::new(2, 2).unwrap(); 2]);
    assert_eq!(live.channels()[0].source, live.channels()[1].source);
    assert_eq!(
        native.snapshot().uploaded_images,
        before_share + 1,
        "shared asset should upload once for this plan"
    );
    let before_frames = live.snapshot().frames_submitted;
    let uploads = native.snapshot().uploaded_images;
    wait("independent frame cadence without repeated Picture upload", || {
        live.snapshot().frames_submitted >= before_frames + 3
    });
    assert_eq!(native.snapshot().uploaded_images, uploads);
    let stale = capture_channels(&engine, &references, 16).unwrap();
    edit(
        &mut engine,
        MediaEdit::RelinkAsset {
            asset: first_asset,
            locator: relative("replacement.png"),
        },
    );
    assert!(!stale.is_current());
    publish(live, &engine, references);
    ready(live, [Extent2D::new(4, 3).unwrap(); 2]);
    let MediaEditResult::Asset { record } = edit(&mut engine, MediaEdit::InspectAsset { asset: first_asset }) else {
        panic!()
    };
    assert_eq!(record.id, first_asset);
    assert_eq!(record.locator, relative("replacement.png"));
    let json = golden_core::app::to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = golden_core::app::from_sparse_project_json::<AppNode>(&json).unwrap();
    loaded.apply_edits().unwrap();
    let old_observations = live.source_observations();
    eprintln!(
        "app Picture native passed LUID{:08x}:{:08x}, frames={}, uploads={}, channels={:?}",
        adapter.id.high as u32,
        adapter.id.low,
        live.snapshot().frames_submitted,
        native.snapshot().uploaded_images,
        live.channels()
    );
    drop(owned);
    let mut recreated = renderer(adapter.id, directory.path());
    let live = recreated.0.as_mut().unwrap();
    publish(live, &loaded, references);
    ready(live, [Extent2D::new(4, 3).unwrap(); 2]);
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
    assert_eq!(live.channels()[0].source, live.channels()[1].source);
    let MediaEditResult::Asset { record } = edit(&mut loaded, MediaEdit::InspectAsset { asset: first_asset }) else {
        panic!()
    };
    assert_eq!(record.id, first_asset);
    assert_eq!(record.locator, relative("replacement.png"));
    assert!(live.native_control().unwrap().sources().is_empty());
    drop(recreated);
    done.send(()).unwrap();
}
