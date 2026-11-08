use super::*;
use crate::{app::AppNode, media::video::protocol::*};
use golden_core::{
    app::{from_sparse_project_json, to_sparse_project_json_pretty},
    application::apply_project_intent,
    node::{Node, NodeId},
    parameter::{ParamValue, ParameterEventBehaviour},
};
use std::time::Duration;

fn state(engine: &mut AppEngine, media: MediaId) -> VideoTransportSnapshot {
    let MediaEditResult::Video { snapshot } = apply(engine, MediaEdit::InspectVideo { media }) else {
        panic!()
    };
    snapshot
}
fn edit(engine: &mut AppEngine, media: MediaId, edit: VideoEdit) -> VideoTransportSnapshot {
    let MediaEditResult::Video { snapshot } = apply(engine, MediaEdit::Video { media, edit }) else {
        panic!()
    };
    snapshot
}
fn parameter(engine: &AppEngine, uuid: golden_core::node::NodeUuid, name: &str) -> NodeId {
    let owner = engine.node_id_by_uuid(uuid).unwrap();
    let mut folder = engine.nodes.get(owner).unwrap().node_data().first_child;
    while let Some(id) = folder {
        let node = engine.nodes.get(id).unwrap();
        if crate::parameters::is_parameter_folder(node) {
            let mut child = node.node_data().first_child;
            while let Some(id) = child {
                let node = engine.nodes.get(id).unwrap();
                if node.node_data().meta.decl_id.0 == format!("parameters/{name}") {
                    return id;
                }
                child = node.node_data().next_sibling;
            }
        }
        folder = node.node_data().next_sibling;
    }
    panic!("missing {name}")
}

#[test]
fn video_creation_has_one_transport_real_file_variants_and_independent_project_controls() {
    let mut engine = fixture();
    let reference = create(&mut engine, MediaKind::Video);
    assert_eq!(
        state(&mut engine, reference.media).settings,
        VideoTransportSettings::default()
    );
    assert!(!state(&mut engine, reference.media).ready);
    let MediaEditResult::Created { reference: second } = apply(
        &mut engine,
        MediaEdit::AddChannel {
            media: reference.media,
            label: None,
        },
    ) else {
        panic!()
    };
    apply(
        &mut engine,
        MediaEdit::ConfigureVideoFile {
            reference,
            file: "clips/a.mov".into(),
        },
    );
    apply(
        &mut engine,
        MediaEdit::ConfigureVideoFile {
            reference: second,
            file: "clips/b.mp4".into(),
        },
    );
    assert_ne!(
        parameter(&engine, reference.channel.0, "file"),
        parameter(&engine, second.channel.0, "file")
    );
    let id = engine.node_id_by_uuid(reference.media.0).unwrap();
    let Some(AppNode::Media(node)) = engine.nodes.get(id) else {
        panic!()
    };
    let control = node.video_control().unwrap();
    assert!(std::sync::Arc::ptr_eq(&control, &node.video_control().unwrap()));
    let json = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&json).unwrap();
    loaded.apply_edits().unwrap();
    crate::media::validate_project(&loaded).unwrap();
    assert_eq!(
        crate::media::channel(&loaded, second).unwrap(),
        crate::media::channel(&engine, second).unwrap()
    );
    let id = loaded.node_id_by_uuid(reference.media.0).unwrap();
    let Some(AppNode::Media(node)) = loaded.nodes.get(id) else {
        panic!()
    };
    assert!(
        !std::sync::Arc::ptr_eq(&control, &node.video_control().unwrap()),
        "project replacement creates a new runtime owner"
    );
}

#[test]
fn typed_transport_intents_share_parameters_history_and_truthful_runtime_readiness() {
    let mut engine = fixture();
    let reference = create(&mut engine, MediaKind::Video);
    edit(&mut engine, reference.media, VideoEdit::Rate { rate: 0.5 });
    edit(&mut engine, reference.media, VideoEdit::Loop { looping: true });
    edit(&mut engine, reference.media, VideoEdit::Seek { position_ms: 1250.25 });
    let playing = edit(&mut engine, reference.media, VideoEdit::Play);
    assert_eq!(playing.settings.mode, VideoPlaybackMode::Playing);
    assert!(
        !playing.ready && !playing.playing,
        "accepted intent is not a decoder readiness claim"
    );
    let id = engine.node_id_by_uuid(reference.media.0).unwrap();
    let Some(AppNode::Media(node)) = engine.nodes.get(id) else {
        panic!()
    };
    let control = node.video_control().unwrap();
    control.prepare(Duration::from_secs(3600)).unwrap();
    let ready = state(&mut engine, reference.media);
    assert!(ready.ready && ready.playing);
    assert!(ready.position_ms >= 1250.25);
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let stopped = edit(&mut engine, reference.media, VideoEdit::Stop);
    assert_eq!(stopped.settings.position_ms, 0.0);
    assert_eq!(stopped.position_ms, 0.0);
    assert!(!stopped.playing);
    assert!(engine.undo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    assert!(engine.redo().unwrap());
    assert_eq!(
        state(&mut engine, reference.media).settings.mode,
        VideoPlaybackMode::Stopped
    );
    let sought = edit(&mut engine, reference.media, VideoEdit::Seek { position_ms: 500.5 });
    assert_eq!(sought.settings.mode, VideoPlaybackMode::Paused);
    assert!((sought.position_ms - 500.5).abs() < 0.00001);
}

#[test]
fn generic_parameter_edits_enforce_stop_companions_and_invalid_writes_are_atomic() {
    let mut engine = fixture();
    let reference = create(&mut engine, MediaKind::Video);
    edit(&mut engine, reference.media, VideoEdit::Seek { position_ms: 5000.0 });
    let mode = parameter(&engine, reference.media.0, "video_mode");
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::SetParam {
            node: mode,
            value: ParamValue::Enum("Stopped".into()),
            behaviour: ParameterEventBehaviour::Append,
        },
    );
    assert!(ack.success, "{ack:?}");
    assert_eq!(state(&mut engine, reference.media).settings.position_ms, 0.0);
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    assert!(
        !invoke(
            &mut engine,
            MediaEdit::Video {
                media: reference.media,
                edit: VideoEdit::Rate { rate: 17.0 }
            }
        )
        .success
    );
    assert!(
        !invoke(
            &mut engine,
            MediaEdit::Video {
                media: reference.media,
                edit: VideoEdit::Seek { position_ms: -1.0 }
            }
        )
        .success
    );
    let file = parameter(&engine, reference.channel.0, "file");
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::SetParam {
            node: file,
            value: ParamValue::File("bad\0movie.mp4".into()),
            behaviour: ParameterEventBehaviour::Append,
        },
    );
    assert!(!ack.success);
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
}

#[test]
fn script_projection_and_coalesced_step_requests_use_the_exact_owner_and_reject_stale_results() {
    let mut engine = fixture();
    let reference = create(&mut engine, MediaKind::Video);
    let operation = crate::media::source_api::operation(
        "seekVideo",
        &[
            ParamValue::Str(reference.media.0.0.to_string()),
            ParamValue::Float(750.25),
        ],
    )
    .unwrap()
    .unwrap();
    apply(&mut engine, operation);
    assert_eq!(state(&mut engine, reference.media).settings.position_ms, 750.25);
    edit(
        &mut engine,
        reference.media,
        VideoEdit::Step {
            channel: reference.channel,
            direction: VideoStepDirection::Next,
        },
    );
    let id = engine.node_id_by_uuid(reference.media.0).unwrap();
    let Some(AppNode::Media(node)) = engine.nodes.get(id) else {
        panic!()
    };
    let control = node.video_control().unwrap();
    let old = control.pending_step().unwrap().unwrap();
    edit(
        &mut engine,
        reference.media,
        VideoEdit::Step {
            channel: reference.channel,
            direction: VideoStepDirection::Previous,
        },
    );
    assert!(!control.resolve_step(old.revision, Duration::ZERO).unwrap());
    edit(&mut engine, reference.media, VideoEdit::Seek { position_ms: 100.0 });
    assert!(control.pending_step().unwrap().is_none());
    let wrong = create(&mut engine, MediaKind::Picture);
    let before = engine.undo_len();
    assert!(
        !invoke(
            &mut engine,
            MediaEdit::Video {
                media: wrong.media,
                edit: VideoEdit::Play
            }
        )
        .success
    );
    assert!(
        !invoke(
            &mut engine,
            MediaEdit::Video {
                media: reference.media,
                edit: VideoEdit::Step {
                    channel: wrong.channel,
                    direction: VideoStepDirection::Next
                }
            }
        )
        .success
    );
    assert_eq!(engine.undo_len(), before);
}
