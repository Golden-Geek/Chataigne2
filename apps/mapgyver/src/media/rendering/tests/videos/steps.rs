use super::*;
use crate::media::video::protocol::{VideoFrameState, VideoStepDirection};
use golden_core::{
    app::to_sparse_project_json_pretty,
    edit::Edit,
    events::EventKind,
    node::{Node, NodeId},
    parameter::{ParamValue, ParameterEventBehaviour},
    script::{ScriptNode, ScriptNodeConfig, ScriptSource},
};

fn frame(engine: &mut AppEngine, reference: ChannelReference) -> Option<VideoFrameState> {
    let MediaEditResult::VideoFrame { frame } = edit(engine, MediaEdit::InspectVideoFrame { reference }) else {
        panic!()
    };
    frame
}
fn at(engine: &mut AppEngine, reference: ChannelReference, position: f64) -> VideoFrameState {
    let mut result = None;
    wait(|| {
        result = frame(engine, reference);
        result
            .as_ref()
            .is_some_and(|frame| (frame.position_ms - position).abs() < 0.000001 && frame.previous_ready)
    });
    result.unwrap()
}
fn step(engine: &mut AppEngine, reference: ChannelReference, direction: VideoStepDirection) -> f64 {
    let MediaEditResult::Video { snapshot } = edit(
        engine,
        MediaEdit::Video {
            media: reference.media,
            edit: VideoEdit::Step {
                channel: reference.channel,
                direction,
            },
        },
    ) else {
        panic!()
    };
    assert_eq!(
        snapshot.settings.mode,
        crate::media::video::protocol::VideoPlaybackMode::Paused
    );
    snapshot.settings.position_ms
}
fn reference_pixels(name: &str) -> Vec<Vec<u8>> {
    use golden_video::decode::{DecodeLimits, SoftwareDecoder, SoftwareImageConverter};
    let mut decoder = SoftwareDecoder::open(
        &fixture_root().join(name),
        DecodeLimits {
            max_pixels: 4096 * 2160,
            max_packet_bytes: 1024 * 1024,
            max_packets_per_frame: 256,
        },
    )
    .unwrap();
    let mut converter = SoftwareImageConverter::default();
    let mut pixels = Vec::new();
    while let Some(frame) = decoder.next_frame().unwrap() {
        pixels.push(converter.convert(&frame).unwrap().pixels()[..4].to_vec());
    }
    pixels
}

#[test]
fn exact_mixed_rate_steps_commit_once_roundtrip_history_and_cross_keyframe_seeks() {
    for name in ["h264.mp4", "hevc.mp4", "prores.mov", "h264-keyframes.mp4"] {
        let pixels = reference_pixels(name);
        let mut engine = engine();
        let reference = source(&mut engine, name);
        let mut renderer = renderer();
        renderer.enable_videos(&fixture_root(), None).unwrap();
        renderer
            .publish(capture_channels(&engine, &[reference], 8).unwrap(), routes(reference))
            .unwrap();
        let mut current = at(&mut engine, reference, 0.0);
        let mut timestamps = vec![current.position_ms];
        assert_eq!(pixel(&renderer, reference).unwrap(), pixels[0]);
        while let Some(next) = current.next_ms {
            assert!(timestamps.len() < 20);
            let before = to_sparse_project_json_pretty(&engine).unwrap();
            let history = engine.undo_len();
            let actual = step(&mut engine, reference, VideoStepDirection::Next);
            assert_eq!(actual, next);
            assert_eq!(engine.undo_len(), history + 1);
            let after = to_sparse_project_json_pretty(&engine).unwrap();
            assert!(engine.undo().unwrap());
            assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
            assert!(engine.redo().unwrap());
            assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), after);
            current = at(&mut engine, reference, next);
            assert_eq!(
                pixel(&renderer, reference).unwrap(),
                pixels[timestamps.len()],
                "{name}: forward pixels"
            );
            timestamps.push(next);
        }
        assert_eq!(timestamps.len(), if name == "h264-keyframes.mp4" { 12 } else { 6 });
        let before = to_sparse_project_json_pretty(&engine).unwrap();
        let history = engine.undo_len();
        step(&mut engine, reference, VideoStepDirection::Next);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
        assert_eq!(engine.undo_len(), history, "EOF is an exact no-op while paused");
        for expected in timestamps.iter().rev().skip(1) {
            let previous = current.previous_ms.unwrap();
            assert_eq!(previous, *expected);
            assert_eq!(step(&mut engine, reference, VideoStepDirection::Previous), *expected);
            current = at(&mut engine, reference, *expected);
            let index = timestamps.iter().position(|time| time == expected).unwrap();
            assert_eq!(
                pixel(&renderer, reference).unwrap(),
                pixels[index],
                "{name}: backward pixels"
            );
        }
        assert!(current.previous_ms.is_none());
        let history = engine.undo_len();
        step(&mut engine, reference, VideoStepDirection::Previous);
        assert_eq!(engine.undo_len(), history);
        let retirement = renderer.video_retirement().unwrap();
        close(renderer);
        assert!(retirement.wait(Duration::from_secs(5)));
    }
}

fn command_parameter(engine: &AppEngine, command: golden_core::node::NodeUuid, field: &str) -> NodeId {
    let owner = engine.node_id_by_uuid(command).unwrap();
    let folder = engine.nodes.get(owner).unwrap().node_data().first_child.unwrap();
    engine
        .nodes
        .iter()
        .find_map(|(id, node)| {
            (node.node_data().parent == Some(folder)
                && node.node_data().meta.decl_id.0 == format!("parameters/{field}"))
            .then_some(id)
        })
        .unwrap()
}
#[test]
fn prepared_steps_match_named_quickjs_commands_headless_parameters_and_committed_callbacks() {
    use crate::media::commands::MediaCommandKind;
    let mut engine = engine();
    let reference = source(&mut engine, "hevc.mp4");
    let mut renderer = renderer();
    renderer.enable_videos(&fixture_root(), None).unwrap();
    renderer
        .publish(capture_channels(&engine, &[reference], 8).unwrap(), routes(reference))
        .unwrap();
    let first = at(&mut engine, reference, 0.0);
    let next = first.next_ms.unwrap();
    let MediaEditResult::CommandCreated { command } = edit(
        &mut engine,
        MediaEdit::CreateCommand {
            command_kind: MediaCommandKind::VideoStep,
            label: None,
        },
    ) else {
        panic!()
    };
    for (field, value) in [
        (
            "media",
            ParamValue::Reference(golden_core::node::NodeReference::new(reference.media.0)),
        ),
        (
            "channel",
            ParamValue::Reference(golden_core::node::NodeReference::new(reference.channel.0)),
        ),
    ] {
        engine.edits.push(Edit::SetParam {
            node: command_parameter(&engine, command, field),
            value,
            behaviour: ParameterEventBehaviour::Append,
        });
    }
    engine.apply_edits().unwrap();
    let history = engine.undo_len();
    edit(&mut engine, MediaEdit::ExecuteCommand { command });
    assert_eq!(engine.undo_len(), history + 1);
    at(&mut engine, reference, next);
    let parent = crate::media::authoring::manager(&engine).unwrap();
    let mut script = ScriptNode::new(
        "Exact step caller",
        ScriptNodeConfig {
            source: ScriptSource::Inline {
                text: format!(
                    "function init(){{local.listen({{level:1}});local.stepVideo('previous','{}','{}','Previous');}} function onAppOperationResult(reply){{gc.emit('exact-video-step',reply);}}",
                    reference.media.0.0, reference.channel.0.0
                ),
            },
        },
    );
    script.budgets.max_wall_time_us_per_load = 1_000_000;
    script.budgets.max_wall_time_us_per_callback = 1_000_000;
    engine.add_node(script.into(), Some(parent));
    engine.apply_edits().unwrap();
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    golden_core::application::service_script_app_operations(&mut engine).unwrap();
    assert_eq!(engine.undo_len(), history + 1);
    let replies = engine
        .ui_event_log()
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::Custom(event) if event.topic == "exact-video-step" => Some(&event.payload),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0]["status"], "applied");
    assert_eq!(replies[0]["value"]["snapshot"]["settings"]["position_ms"], 0.0);
    at(&mut engine, reference, 0.0);
    assert!(engine.undo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    assert!(engine.redo().unwrap());
    at(&mut engine, reference, 0.0);
    let retirement = renderer.video_retirement().unwrap();
    close(renderer);
    assert!(retirement.wait(Duration::from_secs(5)));
}
