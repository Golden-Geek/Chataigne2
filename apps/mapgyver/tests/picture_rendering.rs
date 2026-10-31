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

#[test]
#[ignore = "requires explicit hardware D3D11 and native Spout output availability"]
fn matrix_timed_launches_share_native_outputs_and_retire_interrupted_sources() {
    use mapgyver::matrix::protocol::*;
    use mapgyver::media::capture_channels_at;
    let (done, finished) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        if finished.recv_timeout(Duration::from_secs(60)).is_err() {
            eprintln!("native Matrix test exceeded retirement deadline");
            std::process::exit(1);
        }
    });
    let adapter = adapters()
        .unwrap()
        .into_iter()
        .find(|adapter| !adapter.software)
        .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let mut engine = golden_core::app::create_new_project_engine::<AppNode>().unwrap();
    engine.apply_edits().unwrap();
    let mut sources = Vec::new();
    for rgba in [[1., 0., 0., 0.5], [0., 1., 0., 0.25], [0., 0., 1., 1.]] {
        let MediaEditResult::Created { reference } = edit(
            &mut engine,
            MediaEdit::CreateMedia {
                media_kind: MediaKind::SolidColor,
                label: None,
            },
        ) else {
            panic!()
        };
        let mut definition = channel(&engine, reference).unwrap().definition;
        definition.size = SizePolicy::Fixed {
            extent: Extent2D::new(2, 2).unwrap(),
        };
        definition.source = ChannelSource::SolidColor { rgba };
        edit(&mut engine, MediaEdit::ConfigureChannel { reference, definition });
        sources.push(reference);
    }
    let MediaEditResult::Created { reference: matrix } = edit(
        &mut engine,
        MediaEdit::CreateMedia {
            media_kind: MediaKind::Matrix,
            label: None,
        },
    ) else {
        panic!()
    };
    let matrix_edit = |engine: &mut AppEngine, operation| {
        edit(
            engine,
            MediaEdit::Matrix {
                media: matrix.media,
                edit: operation,
            },
        )
    };
    matrix_edit(
        &mut engine,
        MatrixEdit::Configure {
            definition: MatrixDefinition {
                canvas: Extent2D::new(2, 2).unwrap(),
                transition: TransitionDefinition {
                    duration_ms: 500,
                    curve: TransitionCurve::SmoothStep,
                },
                ..Default::default()
            },
        },
    );
    let MediaEditResult::MatrixCreated { items, .. } = matrix_edit(
        &mut engine,
        MatrixEdit::AddGroup {
            parent: None,
            label: None,
        },
    ) else {
        panic!()
    };
    let group = items[0];
    let MatrixItemId::Group { id: group_id } = group else {
        panic!()
    };
    matrix_edit(
        &mut engine,
        MatrixEdit::ConfigureItem {
            item: group,
            settings: CompositeSettings {
                opacity: 0.5,
                ..Default::default()
            },
        },
    );
    matrix_edit(
        &mut engine,
        MatrixEdit::AddLayers {
            parent: Some(group_id),
            count: 2,
        },
    );
    matrix_edit(&mut engine, MatrixEdit::AddColumns { count: 3 });
    let MediaEditResult::Matrix { snapshot } = matrix_edit(&mut engine, MatrixEdit::Inspect) else {
        panic!()
    };
    let rows = snapshot
        .items
        .iter()
        .filter_map(|item| {
            if let MatrixItemId::Layer { id } = item.id {
                Some((id, item.channel))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    matrix_edit(
        &mut engine,
        MatrixEdit::SetCells {
            assignments: rows
                .iter()
                .flat_map(|(layer, _)| {
                    sources.iter().enumerate().map(|(i, source)| CellAssignment {
                        layer: *layer,
                        column: snapshot.columns[i].id,
                        source: Some(*source),
                    })
                })
                .collect(),
        },
    );
    let roots = [matrix, rows[0].1];
    let mut owned = renderer(adapter.id, directory.path());
    let live = owned.0.as_mut().unwrap();
    let await_frame = |live: &mut ChannelRenderer| {
        let mut installed = None;
        wait("Matrix native generation/frame", || {
            live.refresh().unwrap();
            let mut outcomes = Vec::new();
            live.drain_outcomes(&mut outcomes, NonZeroUsize::new(16).unwrap());
            for outcome in outcomes {
                if Some(outcome.request) == live.latest_request() {
                    match outcome.result {
                        OutcomeKind::Installed { generation, .. } => installed = Some(generation.identity),
                        OutcomeKind::Rejected(failure) => panic!("Matrix publication rejected: {failure:?}"),
                        _ => {}
                    }
                }
            }
            let snapshot = live.snapshot();
            snapshot
                .last_frame
                .is_some_and(|frame| Some(frame.identity) == installed)
                && snapshot
                    .resources
                    .is_some_and(|resources| resources.active.is_some_and(|active| active.output_count == 2))
        });
    };
    let publish_timed = |live: &mut ChannelRenderer, engine: &AppEngine| {
        let time = live.read_time().unwrap();
        live.publish(capture_channels_at(engine, &roots, 16, time).unwrap(), routes(roots))
            .unwrap();
    };
    matrix_edit(
        &mut engine,
        MatrixEdit::LaunchColumn {
            column: snapshot.columns[0].id,
            mode: LaunchMode::Cut,
        },
    );
    publish_timed(live, &engine);
    await_frame(live);
    let targets = roots.map(|reference| {
        live.channels()
            .iter()
            .find(|channel| channel.reference == reference)
            .unwrap()
            .target
    });
    matrix_edit(
        &mut engine,
        MatrixEdit::LaunchColumn {
            column: snapshot.columns[1].id,
            mode: LaunchMode::Configured,
        },
    );
    publish_timed(live, &engine);
    await_frame(live);
    assert!(
        sources[..2]
            .iter()
            .all(|reference| live.channels().iter().any(|channel| channel.reference == *reference))
    );
    // Wait for real shared-time progress, then interrupt before the 500 ms endpoint.
    let progress = live.read_time().unwrap().monotonic();
    wait("Matrix transition progress", || {
        live.read_time().unwrap().monotonic().duration_since(progress).unwrap() >= Duration::from_millis(25)
    });
    matrix_edit(
        &mut engine,
        MatrixEdit::RemoveColumn {
            column: snapshot.columns[0].id,
        },
    );
    matrix_edit(
        &mut engine,
        MatrixEdit::SetCells {
            assignments: rows
                .iter()
                .map(|(layer, _)| CellAssignment {
                    layer: *layer,
                    column: snapshot.columns[1].id,
                    source: Some(sources[2]),
                })
                .collect(),
        },
    );
    matrix_edit(
        &mut engine,
        MatrixEdit::LaunchColumn {
            column: snapshot.columns[1].id,
            mode: LaunchMode::Configured,
        },
    );
    publish_timed(live, &engine);
    await_frame(live);
    assert!(
        sources
            .iter()
            .all(|reference| live.channels().iter().any(|channel| channel.reference == *reference))
    );
    let transitioning = live.latest_request().unwrap();
    let transitioning_identity = live.snapshot().resources.unwrap().active.unwrap().identity;
    wait("Matrix outgoing demand release", || {
        live.refresh().unwrap();
        let mut outcomes = Vec::new();
        live.drain_outcomes(&mut outcomes, NonZeroUsize::new(16).unwrap());
        assert!(
            !outcomes
                .iter()
                .any(|outcome| matches!(outcome.result, OutcomeKind::Rejected(_)))
        );
        !live
            .channels()
            .iter()
            .any(|channel| sources[..2].contains(&channel.reference))
    });
    wait("Matrix cleanup generation submitted", || {
        live.refresh().unwrap();
        let mut outcomes = Vec::new();
        live.drain_outcomes(&mut outcomes, NonZeroUsize::new(16).unwrap());
        assert!(
            !outcomes
                .iter()
                .any(|outcome| matches!(outcome.result, OutcomeKind::Rejected(_)))
        );
        let snapshot = live.snapshot();
        snapshot
            .resources
            .and_then(|resources| resources.active)
            .is_some_and(|active| {
                active.identity != transitioning_identity
                    && snapshot
                        .last_frame
                        .is_some_and(|frame| frame.identity == active.identity)
            })
    });
    assert_eq!(live.latest_request().unwrap().get(), transitioning.get() + 1);
    assert!(live.channels().iter().any(|channel| channel.reference == sources[2]));
    assert_eq!(
        roots.map(|reference| live
            .channels()
            .iter()
            .find(|channel| channel.reference == reference)
            .unwrap()
            .target),
        targets
    );
    let cleaned = live.latest_request().unwrap();
    assert!(live.refresh().unwrap().is_none());
    assert_eq!(live.latest_request(), Some(cleaned));
    let resources = live.snapshot().resources.unwrap();
    assert!(resources.bytes_used <= resources.byte_limit);
    assert!(resources.in_flight <= 2 && resources.retiring_generations <= 2);
    assert!(live.native_control().unwrap().sources().is_empty());
    drop(owned);
    done.send(()).unwrap();
}

#[test]
#[ignore = "requires explicit hardware D3D11 and native Spout output availability"]
fn nested_compositions_share_sources_and_native_outputs_across_edits_and_source_undo() {
    use mapgyver::media::composition::protocol::*;
    let (done, finished) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        if finished.recv_timeout(Duration::from_secs(60)).is_err() {
            eprintln!("native Composition test exceeded retirement deadline");
            std::process::exit(1);
        }
    });
    let adapter = adapters()
        .unwrap()
        .into_iter()
        .find(|adapter| !adapter.software)
        .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let mut engine = golden_core::app::create_new_project_engine::<AppNode>().unwrap();
    engine.apply_edits().unwrap();
    let mut references = Vec::new();
    for (kind, extent) in [
        (MediaKind::SolidColor, 4),
        (MediaKind::Composition, 4),
        (MediaKind::Composition, 8),
    ] {
        let MediaEditResult::Created { reference } = edit(
            &mut engine,
            MediaEdit::CreateMedia {
                media_kind: kind,
                label: None,
            },
        ) else {
            panic!()
        };
        let mut definition = channel(&engine, reference).unwrap().definition;
        definition.size = SizePolicy::Fixed {
            extent: Extent2D::new(extent, extent).unwrap(),
        };
        if kind == MediaKind::Composition {
            definition.source = ChannelSource::Composition {
                canvas: Extent2D::new(extent, extent).unwrap(),
            };
        }
        edit(&mut engine, MediaEdit::ConfigureChannel { reference, definition });
        references.push(reference);
    }
    edit(
        &mut engine,
        MediaEdit::Composition {
            reference: references[1],
            edit: CompositionEdit::AddInstances {
                sources: vec![references[0]],
            },
        },
    );
    let MediaEditResult::InstancesCreated { instances } = edit(
        &mut engine,
        MediaEdit::Composition {
            reference: references[2],
            edit: CompositionEdit::AddInstances {
                sources: vec![references[1], references[1]],
            },
        },
    ) else {
        panic!()
    };
    let mut owned = renderer(adapter.id, directory.path());
    let live = owned.0.as_mut().unwrap();
    let roots = [references[2]; 2];
    let await_frame = |live: &mut ChannelRenderer| {
        let mut installed = None;
        wait("Composition native frame", || {
            live.refresh().unwrap();
            let mut outcomes = Vec::new();
            live.drain_outcomes(&mut outcomes, NonZeroUsize::new(16).unwrap());
            for outcome in outcomes {
                if Some(outcome.request) == live.latest_request() {
                    match outcome.result {
                        OutcomeKind::Installed { generation, .. } => installed = Some(generation.identity),
                        OutcomeKind::Rejected(failure) => panic!("Composition publication rejected: {failure:?}"),
                        _ => {}
                    }
                }
            }
            let snapshot = live.snapshot();
            snapshot
                .resources
                .is_some_and(|resources| resources.active.is_some_and(|active| active.output_count == 2))
                && snapshot
                    .last_frame
                    .is_some_and(|frame| Some(frame.identity) == installed)
                && live
                    .channels()
                    .iter()
                    .find(|channel| channel.reference == roots[0])
                    .is_some_and(|channel| channel.resolution.availability == ChannelAvailability::Ready)
        });
    };
    publish(live, &engine, roots);
    await_frame(live);
    assert_eq!(live.channels().len(), 3);
    let stale = capture_channels(&engine, &roots, 16).unwrap();
    let definition = InstanceDefinition {
        source: Some(references[1]),
        transform: Transform2D {
            position: [3.0, 4.0],
            scale: [-1.0, 1.0],
            rotation: std::f32::consts::FRAC_PI_2,
            pivot: [0.25, 0.75],
        },
        opacity: 0.6,
        blend: BlendMode::Screen,
        ..Default::default()
    };
    edit(
        &mut engine,
        MediaEdit::Composition {
            reference: references[2],
            edit: CompositionEdit::ConfigureInstance {
                instance: instances[0].instance,
                definition,
            },
        },
    );
    assert!(!stale.is_current());
    assert!(live.publish(stale, routes(roots)).is_err());
    publish(live, &engine, roots);
    await_frame(live);
    edit(
        &mut engine,
        MediaEdit::RemoveMedia {
            media: references[0].media,
        },
    );
    publish(live, &engine, roots);
    await_frame(live);
    assert!(engine.undo().unwrap());
    publish(live, &engine, roots);
    await_frame(live);
    assert!(live.native_control().unwrap().sources().is_empty());
    drop(owned);
    done.send(()).unwrap();
}

struct OwnedRenderer(Option<ChannelRenderer>);
impl Drop for OwnedRenderer {
    fn drop(&mut self) {
        if let Some(mut renderer) = self.0.take() {
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
        before_share,
        "the retained shared payload should survive the plan edit without another upload"
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
