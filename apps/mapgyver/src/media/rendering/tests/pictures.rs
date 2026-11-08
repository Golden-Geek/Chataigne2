use super::*;
use crate::media::resolution::ChannelAvailability;
use golden_assets::{AssetRetirement, PreparationLimits};
use golden_video::runtime::{FailureCode, OutcomeKind};
use std::path::Path;

fn relative(path: &str) -> AssetLocator {
    AssetLocator::ProjectRelative {
        path: path.to_owned().try_into().unwrap(),
    }
}

fn import(engine: &mut AppEngine, path: &str) -> AssetId {
    let MediaEditResult::AssetCreated { asset } = edit(
        engine,
        MediaEdit::ImportAsset {
            locator: relative(path),
            label: None,
        },
    ) else {
        panic!("asset import result")
    };
    asset
}

fn select(engine: &mut AppEngine, reference: ChannelReference, asset: AssetId) {
    let mut definition = channel(engine, reference).unwrap().definition;
    definition.source = ChannelSource::Picture {
        file: Some(crate::media::asset(engine, asset).unwrap().locator),
    };
    edit(
        engine,
        MediaEdit::ConfigureChannel {
            reference,
            source: definition.source,
        },
    );
}

fn png(root: &Path, name: &str, width: u32, height: u32, rgba: [u8; 4]) {
    let file = std::fs::File::create(root.join(name)).unwrap();
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer
        .write_image_data(&rgba.repeat((width * height) as usize))
        .unwrap();
    writer.finish().unwrap();
}

fn picture_renderer(root: &Path) -> ChannelRenderer {
    let mut config = config();
    config.max_input_bytes = NonZeroUsize::new(1024 * 1024).unwrap();
    config.max_retained_input_bytes = NonZeroUsize::new(4 * 1024 * 1024).unwrap();
    let mut renderer = ChannelRenderer::new(
        VideoRuntime::spawn_null(config.clone(), NullConfig::default()).unwrap(),
        config.plan_limits,
    );
    wait(|| renderer.snapshot().resources.is_some());
    renderer
        .enable_pictures(
            root,
            PreparationLimits {
                max_source_bytes: 64 * 1024,
                // Match the native app's decode allowance with its 1 MiB plan bound above.
                max_decode_bytes: NonZeroUsize::new(128 * 1024 * 1024).unwrap(),
                max_prepared_bytes: NonZeroUsize::new(256 * 1024 * 1024).unwrap(),
                ..Default::default()
            },
            None,
        )
        .unwrap();
    renderer
}

#[cfg(all(windows, feature = "native-video"))]
#[test]
fn production_preview_budget_admits_a_4k_picture_shared_by_differently_sized_channels() {
    let root = tempfile::tempdir().unwrap();
    png(root.path(), "large.png", 3840, 2160, [255, 0, 0, 255]);
    let mut engine = engine();
    let asset = import(&mut engine, "large.png");
    let first = create(&mut engine, MediaKind::Picture);
    let second = create(&mut engine, MediaKind::Picture);
    select(&mut engine, first, asset);
    select(&mut engine, second, asset);
    let size = SizePolicy::Fixed {
        extent: Extent2D::new(640, 360).unwrap(),
    };
    edit(
        &mut engine,
        MediaEdit::ConfigureMediaResolution {
            media: second.media,
            size,
        },
    );
    let config = crate::preview::config::configuration(512 * 1024 * 1024, None);
    let mut renderer = ChannelRenderer::new(
        VideoRuntime::spawn_null(config.video.clone(), NullConfig::default()).unwrap(),
        config.video.plan_limits,
    );
    renderer.enable_pictures(root.path(), config.pictures, None).unwrap();
    publish(&mut renderer, &engine, [first, second]);
    settled(&mut renderer, [ready(3840, 2160), ready(640, 360)]);
    finish(renderer);
}

#[test]
fn close_releases_picture_leases_before_waiting_for_retirement_without_dropping_the_renderer() {
    let root = tempfile::tempdir().unwrap();
    png(root.path(), "source.png", 2, 2, [255, 0, 0, 255]);
    let mut engine = engine();
    let asset = import(&mut engine, "source.png");
    let reference = create(&mut engine, MediaKind::Picture);
    select(&mut engine, reference, asset);
    let other = create(&mut engine, MediaKind::Picture);
    select(&mut engine, other, asset);
    let mut renderer = picture_renderer(root.path());
    publish(&mut renderer, &engine, [reference, other]);
    settled(
        &mut renderer,
        [(ChannelAvailability::Ready, Some(Extent2D::new(2, 2).unwrap())); 2],
    );
    let pictures = renderer.picture_retirement().unwrap();
    renderer.close();
    assert!(renderer.retirement().wait(Duration::from_secs(5)));
    assert!(pictures.wait(Duration::from_secs(5)));
}

#[test]
fn equal_owned_files_share_one_admitted_source_slot_and_distinct_files_still_obey_the_bound() {
    let root = tempfile::tempdir().unwrap();
    png(root.path(), "shared.png", 2, 2, [255, 0, 0, 255]);
    png(root.path(), "other.png", 2, 2, [0, 255, 0, 255]);
    let mut engine = engine();
    let references = [
        create(&mut engine, MediaKind::Picture),
        create(&mut engine, MediaKind::Picture),
    ];
    for reference in references {
        edit(
            &mut engine,
            MediaEdit::ConfigurePictureFile {
                reference,
                file: "shared.png".into(),
            },
        );
    }
    let mut config = config();
    config.plan_limits.max_external_textures = 1;
    let mut renderer = ChannelRenderer::new(
        VideoRuntime::spawn_null(config.clone(), NullConfig::default()).unwrap(),
        config.plan_limits,
    );
    renderer
        .enable_pictures(root.path(), PreparationLimits::default(), None)
        .unwrap();
    publish(&mut renderer, &engine, references);
    settled(&mut renderer, [ready(2, 2); 2]);
    assert_eq!(renderer.channels()[0].source, renderer.channels()[1].source);
    let request = renderer.latest_request();
    edit(
        &mut engine,
        MediaEdit::ConfigurePictureFile {
            reference: references[1],
            file: "other.png".into(),
        },
    );
    assert!(
        renderer
            .publish(
                capture_channels(&engine, &references, 16).unwrap(),
                two_routes(references)
            )
            .unwrap_err()
            .contains("source count")
    );
    assert_eq!(
        renderer.latest_request(),
        request,
        "rejected demand never installs a partial plan"
    );
    assert!(engine.undo().unwrap());
    publish(&mut renderer, &engine, references);
    settled(&mut renderer, [ready(2, 2); 2]);
    finish(renderer);
}

#[test]
fn relocating_the_picture_root_reprepares_relative_sources_without_reauthoring_channels() {
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    png(first.path(), "source.png", 2, 3, [255, 0, 0, 255]);
    png(second.path(), "source.png", 5, 4, [0, 255, 0, 255]);
    let mut engine = engine();
    let asset = import(&mut engine, "source.png");
    let a = create(&mut engine, MediaKind::Picture);
    let b = create(&mut engine, MediaKind::Picture);
    for reference in [a, b] {
        select(&mut engine, reference, asset);
    }
    let mut renderer = picture_renderer(first.path());
    publish(&mut renderer, &engine, [a, b]);
    settled(
        &mut renderer,
        [(ChannelAvailability::Ready, Some(Extent2D::new(2, 3).unwrap())); 2],
    );
    assert!(renderer.set_file_root(second.path()).unwrap());
    publish(&mut renderer, &engine, [a, b]);
    settled(
        &mut renderer,
        [(ChannelAvailability::Ready, Some(Extent2D::new(5, 4).unwrap())); 2],
    );
    assert!(!renderer.set_file_root(second.path()).unwrap());
    close(renderer);
}

fn two_routes(references: [ChannelReference; 2]) -> Vec<ChannelRoute> {
    references
        .into_iter()
        .enumerate()
        .map(|(index, channel)| ChannelRoute {
            output: OutputId::new(index as u64 + 1).unwrap(),
            channel,
        })
        .collect()
}

#[test]
fn picture_variants_keep_native_extents_and_share_media_resolution_through_disable_and_undo() {
    let root = tempfile::tempdir().unwrap();
    png(root.path(), "small.png", 2, 3, [255, 0, 0, 255]);
    png(root.path(), "wide.png", 5, 4, [0, 255, 0, 255]);
    let mut engine = engine();
    let first = create(&mut engine, MediaKind::Picture);
    let MediaEditResult::Created { reference: second } = edit(
        &mut engine,
        MediaEdit::AddChannel {
            media: first.media,
            label: None,
        },
    ) else {
        panic!()
    };
    for (reference, path) in [(first, "small.png"), (second, "wide.png")] {
        let asset = import(&mut engine, path);
        select(&mut engine, reference, asset);
    }
    let mut renderer = picture_renderer(root.path());
    publish(&mut renderer, &engine, [first, second]);
    settled(&mut renderer, [ready(2, 3), ready(5, 4)]);
    let sources = [first, second].map(|reference| channel(&engine, reference).unwrap().definition.source);
    edit(
        &mut engine,
        MediaEdit::ConfigureMediaResolution {
            media: first.media,
            size: SizePolicy::Fixed {
                extent: Extent2D::new(7, 6).unwrap(),
            },
        },
    );
    publish(&mut renderer, &engine, [first, second]);
    settled(&mut renderer, [ready(7, 6); 2]);
    assert_eq!(
        [first, second].map(|reference| channel(&engine, reference).unwrap().definition.source),
        sources
    );
    edit(
        &mut engine,
        MediaEdit::ConfigureMediaResolution {
            media: first.media,
            size: SizePolicy::Automatic,
        },
    );
    publish(&mut renderer, &engine, [first, second]);
    settled(&mut renderer, [ready(2, 3), ready(5, 4)]);
    assert!(engine.undo().unwrap());
    engine
        .dispatch_inbox(golden_core::process_ctx::ExecutionPhase::EndOfTickStabilization)
        .unwrap();
    publish(&mut renderer, &engine, [first, second]);
    settled(&mut renderer, [ready(7, 6); 2]);
    finish(renderer);
}

fn publish(renderer: &mut ChannelRenderer, engine: &AppEngine, references: [ChannelReference; 2]) {
    renderer
        .publish(
            capture_channels(engine, &references, 16).unwrap(),
            two_routes(references),
        )
        .unwrap();
}

fn settled(renderer: &mut ChannelRenderer, expected: [(ChannelAvailability, Option<Extent2D>); 2]) {
    settled_preserving(renderer, expected, None);
}

fn settled_preserving(
    renderer: &mut ChannelRenderer,
    expected: [(ChannelAvailability, Option<Extent2D>); 2],
    healthy: Option<PreparedChannel>,
) {
    let mut installed = None;
    wait(|| {
        renderer.refresh().unwrap();
        if let Some(healthy) = healthy {
            assert_eq!(
                renderer.channels().first(),
                Some(&healthy),
                "retry must preserve the unrelated healthy channel throughout preparation"
            );
        }
        let mut outcomes = Vec::new();
        renderer.drain_outcomes(&mut outcomes, NonZeroUsize::new(16).unwrap());
        for outcome in outcomes {
            match outcome.result {
                OutcomeKind::Installed { generation, .. } => installed = Some((outcome.request, generation.identity)),
                OutcomeKind::Rejected(failure) => assert!(
                    matches!(failure.code(), FailureCode::StaleSource | FailureCode::Superseded),
                    "{failure:?}"
                ),
                _ => {}
            }
        }
        renderer.channels().len() == expected.len()
            && renderer
                .channels()
                .iter()
                .zip(expected)
                .all(|(channel, (availability, extent))| {
                    channel.resolution.availability == availability && channel.resolution.extent == extent
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

fn ready(width: u32, height: u32) -> (ChannelAvailability, Option<Extent2D>) {
    (ChannelAvailability::Ready, Some(Extent2D::new(width, height).unwrap()))
}

fn finish(mut renderer: ChannelRenderer) {
    let pictures: AssetRetirement = renderer.picture_retirement().expect("picture owner");
    let video = renderer.retirement();
    renderer.close();
    drop(renderer);
    assert!(video.wait(Duration::from_secs(5)));
    assert!(pictures.wait(Duration::from_secs(5)));
}

#[test]
fn automatic_picture_extent_is_shared_with_backend_and_rejects_stale_reload_close_and_loaded_project() {
    let root = tempfile::tempdir().unwrap();
    png(root.path(), "extent.png", 8, 16, [255, 0, 0, 255]);
    let mut engine = engine();
    let asset = import(&mut engine, "extent.png");
    let reference = create(&mut engine, MediaKind::Picture);
    select(&mut engine, reference, asset);
    assert!(crate::media::resolution::channel_extent(&engine, reference).is_err());
    let mut renderer = picture_renderer(root.path());
    renderer
        .publish(capture_channels(&engine, &[reference], 16).unwrap(), routes(reference))
        .unwrap();
    wait(|| {
        renderer.refresh().unwrap();
        crate::media::resolution::channel_extent(&engine, reference) == Ok(Extent2D::new(8, 16).unwrap())
    });
    let saved = golden_core::app::to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = golden_core::app::from_sparse_project_json::<crate::app::AppNode>(&saved).unwrap();
    loaded.apply_edits().unwrap();
    assert!(crate::media::resolution::channel_extent(&loaded, reference).is_err());
    edit(&mut engine, MediaEdit::ReloadPicture { reference });
    assert!(crate::media::resolution::channel_extent(&engine, reference).is_err());
    renderer
        .publish(capture_channels(&engine, &[reference], 16).unwrap(), routes(reference))
        .unwrap();
    wait(|| {
        renderer.refresh().unwrap();
        crate::media::resolution::channel_extent(&engine, reference).is_ok()
    });
    let moved = tempfile::tempdir().unwrap();
    png(moved.path(), "extent.png", 4, 4, [0, 255, 0, 255]);
    assert!(renderer.set_file_root(moved.path()).unwrap());
    assert!(crate::media::resolution::channel_extent(&engine, reference).is_err());
    renderer
        .publish(capture_channels(&engine, &[reference], 16).unwrap(), routes(reference))
        .unwrap();
    wait(|| {
        renderer.refresh().unwrap();
        crate::media::resolution::channel_extent(&engine, reference) == Ok(Extent2D::new(4, 4).unwrap())
    });
    let pictures = renderer.picture_retirement().unwrap();
    let video = renderer.retirement();
    renderer.close();
    assert!(crate::media::resolution::channel_extent(&engine, reference).is_err());
    drop(renderer);
    assert!(video.wait(Duration::from_secs(5)));
    assert!(pictures.wait(Duration::from_secs(5)));
}

#[test]
fn independent_picture_channels_resolve_share_resize_undo_relink_and_reload_without_identity_changes() {
    let project = tempfile::tempdir().unwrap();
    png(project.path(), "first.png", 2, 2, [255, 0, 0, 128]);
    png(project.path(), "second.png", 3, 2, [0, 0, 255, 255]);
    let mut engine = engine();
    let first_asset = import(&mut engine, "first.png");
    let second_asset = import(&mut engine, "second.png");
    let first = create(&mut engine, MediaKind::Picture);
    let MediaEditResult::Created { reference: second } = edit(
        &mut engine,
        MediaEdit::AddChannel {
            media: first.media,
            label: None,
        },
    ) else {
        panic!()
    };
    select(&mut engine, first, first_asset);
    select(&mut engine, second, second_asset);
    let references = [first, second];
    let mut renderer = picture_renderer(project.path());
    publish(&mut renderer, &engine, references);
    settled(&mut renderer, [ready(2, 2), ready(3, 2)]);
    let original = renderer.channels().to_vec();
    assert_ne!(original[0].source, original[1].source);

    select(&mut engine, second, first_asset);
    publish(&mut renderer, &engine, references);
    settled(&mut renderer, [ready(2, 2); 2]);
    assert_eq!(renderer.channels()[0].reference, first);
    assert_eq!(renderer.channels()[1].reference, second);
    let shared_source = renderer.channels()[0].source;
    assert_eq!(shared_source, renderer.channels()[1].source);
    let binding = AssetId::from_uuid(first.channel.0.0);
    let fingerprint = renderer.pictures.as_ref().unwrap().get(binding).unwrap().fingerprint();
    png(project.path(), "first.png", 2, 2, [0, 255, 0, 255]);
    let history = engine.ui_history_state();
    edit(&mut engine, MediaEdit::ReloadPicture { reference: first });
    assert_eq!(engine.ui_history_state(), history);
    publish(&mut renderer, &engine, references);
    settled(&mut renderer, [ready(2, 2); 2]);
    assert_ne!(renderer.channels()[0].source, shared_source);
    assert_eq!(renderer.channels()[0].source, renderer.channels()[1].source);
    assert_ne!(
        renderer.pictures.as_ref().unwrap().get(binding).unwrap().fingerprint(),
        fingerprint
    );
    let shared_source = renderer.channels()[0].source;
    // The first owner may leave a shared File while its peer keeps the old
    // decoded image. Neither source identity nor cache lifetime may collide.
    select(&mut engine, first, second_asset);
    publish(&mut renderer, &engine, references);
    settled(&mut renderer, [ready(3, 2), ready(2, 2)]);
    assert_ne!(renderer.channels()[0].source, shared_source);
    assert_eq!(renderer.channels()[1].source, shared_source);
    assert!(engine.undo().unwrap());
    publish(&mut renderer, &engine, references);
    settled(&mut renderer, [ready(2, 2); 2]);
    assert!(engine.undo().unwrap());
    publish(&mut renderer, &engine, references);
    settled(&mut renderer, [ready(2, 2), ready(3, 2)]);

    resize(&mut engine, second, 5);
    publish(&mut renderer, &engine, references);
    settled(&mut renderer, [ready(5, 2); 2]);
    assert_eq!(renderer.channels()[1].target, original[1].target);
    assert!(engine.undo().unwrap());
    png(project.path(), "replacement.png", 4, 3, [0, 255, 0, 255]);
    edit(
        &mut engine,
        MediaEdit::ConfigurePictureFile {
            reference: second,
            file: "replacement.png".into(),
        },
    );
    publish(&mut renderer, &engine, references);
    settled(&mut renderer, [ready(2, 2), ready(4, 3)]);
    assert_eq!(crate::media::asset(&engine, second_asset).unwrap().id, second_asset);

    let json = golden_core::app::to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = golden_core::app::from_sparse_project_json::<crate::app::AppNode>(&json).unwrap();
    loaded.apply_edits().unwrap();
    let old_project = renderer.snapshot().project;
    publish(&mut renderer, &loaded, references);
    settled(&mut renderer, [ready(2, 2), ready(4, 3)]);
    assert_ne!(renderer.snapshot().project, old_project);
    assert_eq!(
        renderer
            .channels()
            .iter()
            .map(|channel| channel.reference)
            .collect::<Vec<_>>(),
        references
    );
    finish(renderer);
}

#[test]
fn missing_picture_recovery_replacement_and_project_relocation_keep_the_healthy_channel_live() {
    let project = tempfile::tempdir().unwrap();
    png(project.path(), "healthy.png", 2, 2, [255, 0, 0, 255]);
    let mut engine = engine();
    let first_asset = import(&mut engine, "healthy.png");
    let second_asset = import(&mut engine, "missing.png");
    let first = create(&mut engine, MediaKind::Picture);
    let second = create(&mut engine, MediaKind::Picture);
    select(&mut engine, first, first_asset);
    select(&mut engine, second, second_asset);
    let references = [first, second];
    let mut renderer = picture_renderer(project.path());
    publish(&mut renderer, &engine, references);
    settled(&mut renderer, [ready(2, 2), (ChannelAvailability::Unavailable, None)]);
    assert!(renderer.snapshot().frames_submitted > 0);
    let healthy = renderer.channels()[0];

    png(project.path(), "missing.png", 3, 2, [0, 0, 255, 255]);
    let stale = capture_channels(&engine, &references, 16).unwrap();
    let document = golden_core::app::to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    edit(&mut engine, MediaEdit::ReloadPicture { reference: second });
    assert!(
        !stale.is_current(),
        "reload fences captured plans without an authored edit"
    );
    assert_eq!(engine.undo_len(), history);
    assert_eq!(
        golden_core::app::to_sparse_project_json_pretty(&engine).unwrap(),
        document
    );
    publish(&mut renderer, &engine, references);
    settled_preserving(&mut renderer, [ready(2, 2), ready(3, 2)], Some(healthy));
    png(project.path(), "missing.png", 4, 3, [0, 255, 0, 255]);
    edit(&mut engine, MediaEdit::ReloadPicture { reference: second });
    publish(&mut renderer, &engine, references);
    settled_preserving(&mut renderer, [ready(2, 2), ready(4, 3)], Some(healthy));
    assert_eq!(crate::media::asset(&engine, second_asset).unwrap().id, second_asset);

    let relocated = tempfile::tempdir().unwrap();
    png(relocated.path(), "healthy.png", 5, 2, [255, 0, 0, 255]);
    png(relocated.path(), "missing.png", 6, 3, [0, 0, 255, 255]);
    renderer.relocate_pictures(relocated.path()).unwrap();
    settled(&mut renderer, [ready(5, 2), ready(6, 3)]);
    assert_eq!(
        renderer
            .channels()
            .iter()
            .map(|channel| channel.reference)
            .collect::<Vec<_>>(),
        references
    );
    finish(renderer);
}

#[test]
fn removed_playing_picture_copy_keeps_its_prepared_lease_until_cut_and_renderer_retirement() {
    use super::transitions::{fixture, launch};
    use crate::{matrix::protocol::LaunchMode, media::capture_channels_at};
    let project = tempfile::tempdir().unwrap();
    png(project.path(), "owned.png", 2, 2, [255, 0, 0, 255]);
    let (mut engine, matrix, _, layer, columns) = fixture();
    let source = create(&mut engine, MediaKind::Picture);
    let asset = import(&mut engine, "owned.png");
    select(&mut engine, source, asset);
    let old = super::retention::drop_copy(&mut engine, matrix, layer, columns[0], source);
    let mut renderer = picture_renderer(project.path());
    launch(&mut engine, matrix.media, columns[0], LaunchMode::Cut);
    let time = renderer.read_time().unwrap();
    renderer
        .publish(
            capture_channels_at(&engine, &[matrix], 32, time).unwrap(),
            routes(matrix),
        )
        .unwrap();
    wait(|| {
        renderer.refresh().unwrap();
        let mut outcomes = Vec::new();
        renderer.drain_outcomes(&mut outcomes, NonZeroUsize::new(16).unwrap());
        renderer
            .channels()
            .iter()
            .any(|channel| channel.reference == old && channel.resolution.availability == ChannelAvailability::Ready)
    });
    let binding = AssetId::from_uuid(old.channel.0.0);
    let fingerprint = renderer.pictures.as_ref().unwrap().get(binding).unwrap().fingerprint();
    edit(&mut engine, MediaEdit::RemoveMedia { media: old.media });
    let time = renderer.read_time().unwrap();
    let request = renderer
        .publish(
            capture_channels_at(&engine, &[matrix], 32, time).unwrap(),
            routes(matrix),
        )
        .unwrap();
    installed(&renderer, request);
    assert_eq!(
        renderer.pictures.as_ref().unwrap().get(binding).unwrap().fingerprint(),
        fingerprint
    );
    assert!(
        renderer
            .channels()
            .iter()
            .any(|channel| channel.reference == old && channel.resolution.availability == ChannelAvailability::Ready)
    );
    launch(&mut engine, matrix.media, columns[1], LaunchMode::Cut);
    let time = renderer.read_time().unwrap();
    let request = renderer
        .publish(
            capture_channels_at(&engine, &[matrix], 32, time).unwrap(),
            routes(matrix),
        )
        .unwrap();
    installed(&renderer, request);
    assert!(renderer.pictures.as_ref().unwrap().get(binding).is_none());
    assert!(!renderer.channels().iter().any(|channel| channel.reference == old));
    finish(renderer);
}
