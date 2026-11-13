use super::{video::*, *};
use crate::media::video::protocol::*;
use golden_core::application::service_script_app_operations;
use std::time::Duration;

fn service(engine: &mut AppEngine) {
    for _ in 0..3 {
        tick(engine);
        service_script_app_operations(engine).unwrap();
    }
    tick(engine);
}
fn reference(engine: &mut AppEngine, target: MediaId) -> MediaId {
    let MediaEditResult::ReferenceCreated { media } = apply(
        engine,
        MediaEdit::CreateReference {
            target: Some(target),
            label: None,
        },
    ) else {
        panic!()
    };
    media
}
fn retarget(engine: &mut AppEngine, media: MediaId, target: MediaId) {
    apply(
        engine,
        MediaEdit::ConfigureReference {
            media,
            target: Some(target),
        },
    );
    service(engine);
}
fn enabled(engine: &mut AppEngine, uuid: golden_core::node::NodeUuid, enabled: bool) {
    let node = engine.node_id_by_uuid(uuid).unwrap();
    assert!(
        apply_project_intent(
            engine,
            UiEditIntent::PatchMeta {
                node,
                patch: golden_core::ui_sync::NodeMetaPatch {
                    enabled: Some(enabled),
                    ..Default::default()
                }
            }
        )
        .success
    );
    service(engine);
}

#[test]
fn live_nested_reference_retarget_applies_all_nine_policies_without_a_renderer() {
    for activation in [
        VideoOnActivate::PlayFromStart,
        VideoOnActivate::Resume,
        VideoOnActivate::Nothing,
    ] {
        for deactivation in [
            VideoOnDeactivate::Stop,
            VideoOnDeactivate::Pause,
            VideoOnDeactivate::Nothing,
        ] {
            let mut engine = fixture();
            let sources = [
                create(&mut engine, MediaKind::Video),
                create(&mut engine, MediaKind::Video),
            ];
            let clocks = sources.map(|source| clock(&engine, source.media));
            for (source, clock) in sources.iter().zip(&clocks) {
                clock.prepare(Duration::from_secs(10)).unwrap();
                for edit in [
                    VideoEdit::Speed { speed: 0.0 },
                    VideoEdit::Seek { position_ms: 2500.0 },
                    VideoEdit::OnActivate { policy: activation },
                    VideoEdit::OnDeactivate { policy: deactivation },
                ] {
                    policy(&mut engine, source.media, edit);
                }
            }
            let inner = reference(&mut engine, sources[0].media);
            let outer = reference(&mut engine, inner);
            let matrix = create(&mut engine, MediaKind::Matrix);
            let row = layer(layers(&mut engine, matrix.media, None, 1)[0]);
            let column = columns(&mut engine, matrix.media, 1)[0];
            assign(&mut engine, matrix.media, row, column, outer);
            cut(&mut engine, matrix.media, column);
            let history = engine.undo_len();
            retarget(&mut engine, inner, sources[1].media);
            assert_eq!(
                engine.undo_len(),
                history + 1,
                "only the authored retarget enters history"
            );
            let expected = if activation == VideoOnActivate::PlayFromStart {
                Duration::ZERO
            } else {
                Duration::from_millis(2500)
            };
            assert_eq!(clocks[1].sample().unwrap().position, expected);
            assert_eq!(
                clocks[1].sample().unwrap().playing,
                activation != VideoOnActivate::Nothing
            );
            assert_eq!(
                clocks[0].sample().unwrap().position,
                if deactivation == VideoOnDeactivate::Stop {
                    Duration::ZERO
                } else {
                    expected
                }
            );
            assert_eq!(
                clocks[0].sample().unwrap().playing,
                deactivation == VideoOnDeactivate::Nothing && activation != VideoOnActivate::Nothing
            );
        }
    }
}

#[test]
fn selected_video_media_channel_and_reference_enable_reconcile_without_hidden_activation() {
    let mut engine = fixture();
    let video = create(&mut engine, MediaKind::Video);
    let control = clock(&engine, video.media);
    control.prepare(Duration::from_secs(10)).unwrap();
    let reference = reference(&mut engine, video.media);
    let matrix = create(&mut engine, MediaKind::Matrix);
    let row = layer(layers(&mut engine, matrix.media, None, 1)[0]);
    let column = columns(&mut engine, matrix.media, 1)[0];
    assign(&mut engine, matrix.media, row, column, reference);
    cut(&mut engine, matrix.media, column);
    for owner in [video.media.0, video.channel.0, reference.0] {
        enabled(&mut engine, owner, false);
        assert_eq!(control.settings().unwrap().mode, VideoPlaybackMode::Stopped);
        enabled(&mut engine, owner, true);
        assert!(control.sample().unwrap().playing);
    }
}

#[test]
fn source_retarget_retains_another_matrix_and_undo_redo_follows_the_same_live_route() {
    let mut engine = fixture();
    let sources = [
        create(&mut engine, MediaKind::Video),
        create(&mut engine, MediaKind::Video),
    ];
    let clocks = sources.map(|source| clock(&engine, source.media));
    for control in &clocks {
        control.prepare(Duration::from_secs(10)).unwrap();
    }
    let references = [
        reference(&mut engine, sources[0].media),
        reference(&mut engine, sources[0].media),
    ];
    for source in references {
        let matrix = create(&mut engine, MediaKind::Matrix);
        let row = layer(layers(&mut engine, matrix.media, None, 1)[0]);
        let column = columns(&mut engine, matrix.media, 1)[0];
        assign(&mut engine, matrix.media, row, column, source);
        cut(&mut engine, matrix.media, column);
    }
    retarget(&mut engine, references[0], sources[1].media);
    assert!(clocks.iter().all(|control| control.sample().unwrap().playing));
    retarget(&mut engine, references[1], sources[1].media);
    assert_eq!(clocks[0].settings().unwrap().mode, VideoPlaybackMode::Stopped);
    assert!(engine.undo().unwrap());
    service(&mut engine);
    assert!(clocks.iter().all(|control| control.sample().unwrap().playing));
    assert!(engine.redo().unwrap());
    service(&mut engine);
    assert_eq!(clocks[0].settings().unwrap().mode, VideoPlaybackMode::Stopped);
    assert!(clocks[1].sample().unwrap().playing);
}

#[test]
fn retargeting_an_outgoing_reference_moves_its_video_use_but_keeps_the_original_fade_deadline() {
    use super::cell_playback::{at, capture, time};
    let mut engine = fixture();
    let sources = [
        create(&mut engine, MediaKind::Video),
        create(&mut engine, MediaKind::Video),
    ];
    let clocks = sources.map(|source| clock(&engine, source.media));
    for control in &clocks {
        control.prepare(Duration::from_secs(10)).unwrap();
    }
    let source = reference(&mut engine, sources[0].media);
    let matrix = create(&mut engine, MediaKind::Matrix);
    let row = layer(layers(&mut engine, matrix.media, None, 1)[0]);
    let columns = columns(&mut engine, matrix.media, 2);
    assign(&mut engine, matrix.media, row, columns[0], source);
    cut(&mut engine, matrix.media, columns[0]);
    let origin = time();
    capture(&engine, matrix.media, row, at(origin, 1000));
    configured(&mut engine, matrix.media, columns[1], 30_000);
    retarget(&mut engine, source, sources[1].media);
    assert_eq!(clocks[0].settings().unwrap().mode, VideoPlaybackMode::Stopped);
    assert!(
        clocks[1].sample().unwrap().playing,
        "new canonical source stays moving for the outgoing clip"
    );
    capture(&engine, matrix.media, row, at(origin, 31_100));
    assert_eq!(clocks[1].settings().unwrap().mode, VideoPlaybackMode::Stopped);
}

#[test]
fn replacing_cell_source_preserves_the_launched_route_until_explicit_relaunch() {
    use super::cell_playback::{capture, time};
    let mut engine = fixture();
    let sources = [
        create(&mut engine, MediaKind::Video),
        create(&mut engine, MediaKind::Video),
    ];
    let clocks = sources.map(|source| clock(&engine, source.media));
    for control in &clocks {
        control.prepare(Duration::from_secs(10)).unwrap();
    }
    let matrix = create(&mut engine, MediaKind::Matrix);
    let row = layer(layers(&mut engine, matrix.media, None, 1)[0]);
    let column = columns(&mut engine, matrix.media, 1)[0];
    assign(&mut engine, matrix.media, row, column, sources[0].media);
    cut(&mut engine, matrix.media, column);
    capture(&engine, matrix.media, row, time());
    assign(&mut engine, matrix.media, row, column, sources[1].media);
    service(&mut engine);
    assert_eq!(
        clocks[1].settings().unwrap().mode,
        VideoPlaybackMode::Paused,
        "authoring a replacement never activates it in the old clip"
    );
    assert!(
        clocks[0].sample().unwrap().playing,
        "retained source keeps moving after its owned Reference is replaced"
    );
    cut(&mut engine, matrix.media, column);
    assert!(clocks[1].sample().unwrap().playing);
    assert_eq!(clocks[0].settings().unwrap().mode, VideoPlaybackMode::Stopped);
}

#[test]
fn deleting_and_restoring_a_video_or_layer_cannot_reuse_an_old_generation_or_hidden_scope() {
    for delete_layer in [false, true] {
        let mut engine = fixture();
        let source = create(&mut engine, MediaKind::Video);
        let control = clock(&engine, source.media);
        control.prepare(Duration::from_secs(10)).unwrap();
        let matrix = create(&mut engine, MediaKind::Matrix);
        let row = layer(layers(&mut engine, matrix.media, None, 1)[0]);
        let column = columns(&mut engine, matrix.media, 1)[0];
        assign(&mut engine, matrix.media, row, column, source.media);
        cut(&mut engine, matrix.media, column);
        if delete_layer {
            edit(
                &mut engine,
                matrix.media,
                MatrixEdit::RemoveItem {
                    item: MatrixItemId::Layer { id: row },
                },
            );
        } else {
            apply(&mut engine, MediaEdit::RemoveMedia { media: source.media });
        }
        service(&mut engine);
        if delete_layer {
            assert!(!control.sample().unwrap().playing);
        } else {
            assert!(
                control.command(VideoPlaybackMode::Paused).is_err(),
                "retired media rejects old intents"
            );
        }
        assert!(engine.undo().unwrap());
        service(&mut engine);
        let restored = clock(&engine, source.media);
        restored.prepare(Duration::from_secs(10)).unwrap();
        assert!(
            restored.sample().unwrap().playing,
            "restored source needs a fresh current use"
        );
        edit(&mut engine, matrix.media, MatrixEdit::ClearPlayback);
        service(&mut engine);
        assert_eq!(restored.settings().unwrap().mode, VideoPlaybackMode::Stopped);
    }
}

#[test]
fn launching_an_unrelated_layer_does_not_adopt_a_replacement_in_the_old_launch() {
    let mut engine = fixture();
    let sources = [
        create(&mut engine, MediaKind::Video),
        create(&mut engine, MediaKind::Video),
    ];
    let clocks = sources.map(|source| clock(&engine, source.media));
    for control in &clocks {
        control.prepare(Duration::from_secs(10)).unwrap();
    }
    let matrix = create(&mut engine, MediaKind::Matrix);
    let rows = layers(&mut engine, matrix.media, None, 2)
        .into_iter()
        .map(layer)
        .collect::<Vec<_>>();
    let columns = columns(&mut engine, matrix.media, 2);
    assign(&mut engine, matrix.media, rows[0], columns[0], sources[0].media);
    assign(&mut engine, matrix.media, rows[1], columns[1], sources[0].media);
    cut(&mut engine, matrix.media, columns[0]);
    assign(&mut engine, matrix.media, rows[0], columns[0], sources[1].media);
    service(&mut engine);
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::LaunchCell {
            layer: rows[1],
            column: columns[1],
            mode: LaunchMode::Cut,
        },
    );
    service(&mut engine);
    assert_eq!(clocks[1].settings().unwrap().mode, VideoPlaybackMode::Paused);
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::LaunchCell {
            layer: rows[0],
            column: columns[0],
            mode: LaunchMode::Cut,
        },
    );
    service(&mut engine);
    assert!(clocks[1].sample().unwrap().playing);
}

#[test]
fn route_notifications_ignore_manual_transport_color_and_resolution_edits() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    for kind in [MediaKind::Video, MediaKind::SolidColor] {
        let mut engine = fixture();
        let source = create(&mut engine, kind);
        let mut guards = Vec::new();
        crate::media::MediaSource {
            media: source.media,
            channel: Some(source.channel),
        }
        .video_owner(&engine, &mut guards)
        .unwrap();
        let notifications = Arc::new(AtomicUsize::new(0));
        let count = notifications.clone();
        let notifier = golden_io::WorkerNotifier::new(move || {
            count.fetch_add(1, Ordering::Relaxed);
        });
        let _subscriptions = guards
            .iter()
            .map(|guard| guard.subscribe(notifier.clone()))
            .collect::<Vec<_>>();
        if kind == MediaKind::Video {
            clock(&engine, source.media).prepare(Duration::from_secs(10)).unwrap();
            for edit in [
                VideoEdit::Seek { position_ms: 3000.0 },
                VideoEdit::Speed { speed: 0.5 },
                VideoEdit::Play,
            ] {
                policy(&mut engine, source.media, edit);
            }
        } else {
            apply(
                &mut engine,
                MediaEdit::ConfigureChannel {
                    reference: source,
                    source: ChannelSource::SolidColor {
                        rgba: [0.2, 0.3, 0.4, 1.0],
                    },
                },
            );
        }
        apply(
            &mut engine,
            MediaEdit::ConfigureMediaResolution {
                media: source.media,
                size: SizePolicy::Fixed {
                    extent: golden_video::Extent2D::new(320, 240).unwrap(),
                },
            },
        );
        assert_eq!(
            notifications.load(Ordering::Relaxed),
            0,
            "content/transport feedback must not recapture Matrix routes"
        );
        enabled(&mut engine, source.channel.0, false);
        assert_eq!(notifications.load(Ordering::Relaxed), 1);
    }
}

#[test]
fn a_reactivation_with_stale_routes_still_runs_its_new_identity_once() {
    use golden_core::{
        edit::Edit,
        node::NodeReference,
        parameter::{ParamValue, ParameterEventBehaviour},
    };
    let mut engine = fixture();
    let sources = [
        create(&mut engine, MediaKind::Video),
        create(&mut engine, MediaKind::Video),
    ];
    let control = clock(&engine, sources[0].media);
    control.prepare(Duration::from_secs(10)).unwrap();
    policy(&mut engine, sources[0].media, VideoEdit::Speed { speed: 0.0 });
    let route = reference(&mut engine, sources[0].media);
    let MediaEditResult::Created { reference: variant } = apply(
        &mut engine,
        MediaEdit::AddChannel {
            media: sources[0].media,
            label: None,
        },
    ) else {
        panic!()
    };
    apply(
        &mut engine,
        MediaEdit::SelectReferenceChannel {
            media: route,
            channel: None,
        },
    );
    let matrix = create(&mut engine, MediaKind::Matrix);
    let row = layer(layers(&mut engine, matrix.media, None, 1)[0]);
    let column = columns(&mut engine, matrix.media, 1)[0];
    assign(&mut engine, matrix.media, row, column, route);
    cut(&mut engine, matrix.media, column);
    policy(&mut engine, sources[0].media, VideoEdit::Seek { position_ms: 2500.0 });
    super::super::playback::launch(&mut engine, matrix.media, column, LaunchMode::Cut).unwrap();
    let Some(AppNode::ReferenceMedia(node)) = engine.node_id_by_uuid(route.0).and_then(|id| engine.nodes.get(id))
    else {
        panic!()
    };
    let target = node.channel_parameter();
    // Route changes after preparation invalidate the receipt's captured recipe, even when
    // the final target returns to the same owner. The new Matrix activation must still run.
    for channel in [variant.channel, sources[0].channel] {
        engine.edits.push(Edit::SetParam {
            node: target,
            value: ParamValue::Reference(NodeReference::new(channel.0)),
            behaviour: ParameterEventBehaviour::Append,
        });
    }
    engine.apply_edits().unwrap();
    service(&mut engine);
    assert_eq!(control.sample().unwrap().position, Duration::ZERO);
    policy(&mut engine, sources[0].media, VideoEdit::Seek { position_ms: 1700.0 });
    service(&mut engine);
    assert_eq!(
        control.sample().unwrap().position,
        Duration::from_millis(1700),
        "route callbacks cannot replay the activation"
    );
}

#[test]
fn matrix_layer_visibility_does_not_stop_or_reactivate_its_launched_video() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::Video);
    let control = clock(&engine, source.media);
    control.prepare(Duration::from_secs(10)).unwrap();
    policy(&mut engine, source.media, VideoEdit::Speed { speed: 0.0 });
    let matrix = create(&mut engine, MediaKind::Matrix);
    let row = layer(layers(&mut engine, matrix.media, None, 1)[0]);
    let column = columns(&mut engine, matrix.media, 1)[0];
    assign(&mut engine, matrix.media, row, column, source.media);
    cut(&mut engine, matrix.media, column);
    policy(&mut engine, source.media, VideoEdit::Seek { position_ms: 2500.0 });
    for visible in [false, true] {
        enabled(&mut engine, row.0, visible);
        assert!(control.sample().unwrap().playing);
        assert_eq!(control.sample().unwrap().position, Duration::from_millis(2500));
    }
}
