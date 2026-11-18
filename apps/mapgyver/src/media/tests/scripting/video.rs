use super::*;

#[test]
fn hosted_video_subtitles_edit_and_query_return_the_same_typed_settings() {
    let mut engine = fixture();
    let reference = create(&mut engine, MediaKind::Video);
    let MediaEditResult::Created { reference: sibling } = apply(
        &mut engine,
        MediaEdit::AddChannel {
            media: reference.media,
            label: None,
        },
    ) else {
        panic!("expected a second Video channel");
    };
    script(
        &mut engine,
        format!(
            r#"
            function init() {{
                local.listen({{level:1}});
                const media = '{}';
                const channel = '{}';
                const sibling = '{}';
                local.configureVideoSubtitles('captions', media, channel, true, 'File', 'clips/one.srt', 6.5, 'Arial', [0.4, 0.8]);
                local.inspectVideoSubtitles('inspect', media, channel);
                local.inspectVideoSubtitles('sibling', media, sibling);
            }}
            function onAppOperationResult(reply) {{ gc.emit('media-method-reply', reply); }}
            "#,
            reference.media.0.0, reference.channel.0.0, sibling.channel.0.0
        ),
    );
    let history = engine.undo_len();
    service_script_app_operations(&mut engine).unwrap();
    let results = replies(&engine);
    assert_eq!(results.len(), 3);
    assert_eq!(engine.undo_len(), history + 1);
    assert!(
        results.iter().all(|result| result["status"] == "applied"),
        "{results:?}"
    );
    assert_eq!(results[0]["value"], results[1]["value"]);
    let value = &results[0]["value"];
    assert_eq!(value["kind"], "VideoSubtitles");
    assert_eq!(
        serde_json::from_value::<ChannelReference>(value["reference"].clone()).unwrap(),
        reference
    );
    assert_eq!(value["settings"]["enabled"], true);
    assert_eq!(value["settings"]["track"]["kind"], "File");
    assert_eq!(value["settings"]["file"], "clips/one.srt");
    assert_eq!(value["settings"]["size"], 6.5);
    assert_eq!(value["settings"]["font"], "Arial");
    assert_eq!(value["settings"]["position"], serde_json::json!([0.4, 0.8]));
    assert_eq!(value["tracks"], serde_json::json!([]));
    let sibling_value = &results[2]["value"];
    assert_eq!(
        serde_json::from_value::<ChannelReference>(sibling_value["reference"].clone()).unwrap(),
        sibling
    );
    assert_eq!(sibling_value["settings"]["enabled"], false);
    assert_eq!(sibling_value["settings"]["track"]["kind"], "None");
}

#[test]
fn hosted_matrix_cut_activates_video_after_commit_without_render_or_playback_query() {
    use crate::matrix::protocol::*;
    let mut engine = fixture();
    let video = create(&mut engine, MediaKind::Video);
    let id = engine.node_id_by_uuid(video.media.0).unwrap();
    let AppNode::Media(node) = engine.nodes.get(id).unwrap() else {
        panic!()
    };
    let control = node.video_control().unwrap();
    control.prepare(std::time::Duration::from_secs(10)).unwrap();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let snapshot = crate::matrix::inspect(&engine, matrix.media).unwrap();
    let row = snapshot
        .items
        .iter()
        .find_map(|item| match item.id {
            MatrixItemId::Layer { id } => Some(id),
            _ => None,
        })
        .unwrap();
    let column = snapshot.columns[0].id;
    apply(
        &mut engine,
        MediaEdit::Matrix {
            media: matrix.media,
            edit: MatrixEdit::DropSource {
                layer: row,
                column,
                source: video.media.0,
                copy: false,
            },
        },
    );
    script(
        &mut engine,
        format!(
            "function init(){{local.listen({{level:1}});local.launchMatrixColumn('cut','{}','{}','Cut');}} function onAppOperationResult(reply){{gc.emit('media-method-reply',reply);}}",
            matrix.media.0.0, column.0.0
        ),
    );
    service_script_app_operations(&mut engine).unwrap();
    // The named method commits a typed trigger; its Matrix control intent is serviced next.
    service_script_app_operations(&mut engine).unwrap();
    engine.run_tick(std::time::Duration::from_millis(1)).unwrap();
    let replies = replies(&engine);
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0]["status"], "applied", "{replies:?}");
    assert_eq!(
        control.settings().unwrap().mode,
        crate::media::video::protocol::VideoPlaybackMode::Playing
    );
    assert!(control.sample().unwrap().playing);
}

#[test]
fn hosted_video_methods_commit_once_and_report_actual_readiness_and_rejection() {
    let mut engine = fixture();
    let reference = create(&mut engine, MediaKind::Video);
    let media = reference.media.0.0.to_string();
    let channel = reference.channel.0.0.to_string();
    script(
        &mut engine,
        format!(
            r#"
            function init() {{
                local.listen({{level:1}});
                const media = '{media}', channel = '{channel}';
                local.configureVideoFile('file', media, channel, 'clips/example.mov');
                local.seekVideo('seek', media, 1250.5);
                local.setVideoSpeed('speed', media, -0.5);
                local.setVideoLoop('loop', media, true);
                local.setVideoAutoSmoothLoop('overlap', media, 750.25);
                local.setVideoPlayAtLoad('load', media, true);
                local.playVideo('play', media);
                local.pauseVideo('pause', media);
                local.stepVideo('step', media, channel, 'Next');
                local.stopVideo('stop', media);
                local.inspectVideo('inspect', media);
                local.inspectVideoFrame('frame', media, channel);
                local.inspectVideoSource('source', media, channel);
                local.setVideoSpeed('reject', media, 17);
            }}
            function onAppOperationResult(reply) {{
                gc.emit('media-method-reply', reply);
                if (reply.request_id === 'loop') local.setVideoLoopStart('loop-start', '{media}', 2150.5);
            }}
            "#
        ),
    );
    // The script service deliberately limits requests per engine turn.
    service_script_app_operations(&mut engine).unwrap();
    service_script_app_operations(&mut engine).unwrap();
    service_script_app_operations(&mut engine).unwrap();
    let results = replies(&engine);
    assert_eq!(results.len(), 15, "each actual QuickJS call has one callback");
    for request in [
        "file",
        "seek",
        "speed",
        "loop",
        "loop-start",
        "overlap",
        "load",
        "play",
        "pause",
        "stop",
        "inspect",
        "frame",
        "source",
    ] {
        let reply = results.iter().find(|reply| reply["request_id"] == request).unwrap();
        assert_eq!(reply["status"], "applied", "{reply}");
    }
    let result =
        |request: &str| &results.iter().find(|reply| reply["request_id"] == request).unwrap()["value"]["snapshot"];
    assert_eq!(
        result("seek")["settings"]["position_ms"],
        0.0,
        "unprepared script seeks clamp to zero"
    );
    assert_eq!(result("play")["settings"]["mode"], "Playing");
    assert_eq!(result("play")["ready"], false);
    assert_eq!(result("play")["playing"], false);
    let unavailable = results.iter().find(|reply| reply["request_id"] == "step").unwrap();
    assert_eq!(unavailable["status"], "rejected");
    assert!(unavailable.get("value").is_none());
    assert_eq!(result("stop")["position_ms"], 0.0);
    assert_eq!(result("inspect")["settings"]["speed"], -0.5);
    assert_eq!(result("inspect")["settings"]["looping"], true);
    assert_eq!(result("loop-start")["settings"]["loop_start_ms"], 2150.5);
    assert_eq!(result("inspect")["settings"]["auto_smooth_loop_ms"], 750.25);
    assert_eq!(result("inspect")["settings"]["play_at_load"], true);
    assert!(results.iter().find(|reply| reply["request_id"] == "frame").unwrap()["value"]["frame"].is_null());
    let source = &results.iter().find(|reply| reply["request_id"] == "source").unwrap()["value"]["status"];
    assert_eq!(
        source["state"],
        if cfg!(feature = "video-files") {
            "Loading"
        } else {
            "Unavailable"
        }
    );
    assert_eq!(source["retrying"], false);
    let rejection = results.iter().find(|reply| reply["request_id"] == "reject").unwrap();
    assert_eq!(rejection["status"], "rejected");
    assert!(rejection.get("value").is_none());
    let after = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    service_script_app_operations(&mut engine).unwrap();
    assert_eq!(replies(&engine).len(), 15);
    assert_eq!(
        engine.undo_len(),
        history,
        "callbacks and read-only inspection add no history"
    );
    assert!(engine.undo().unwrap());
    assert!(engine.redo().unwrap());
    service_script_app_operations(&mut engine).unwrap();
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), after);
    assert_eq!(replies(&engine).len(), 15, "history does not replay requests");
}

#[test]
fn hosted_video_crop_accepts_a_typed_open_end_and_commits_each_call_once() {
    let mut engine = fixture();
    let media = create(&mut engine, MediaKind::Video).media.0.0;
    script(
        &mut engine,
        format!(
            r#"
            function init() {{
                local.listen({{level:1}});
                local.setVideoCrop('crop', '{media}', 250.75, 700);
                local.setVideoCrop('through-end', '{media}', 300, null);
            }}
            function onAppOperationResult(reply) {{ gc.emit('media-method-reply', reply); }}
            "#
        ),
    );
    let history = engine.undo_len();
    service_script_app_operations(&mut engine).unwrap();
    let results = replies(&engine);
    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|reply| reply["status"] == "applied"), "{results:?}");
    assert_eq!(engine.undo_len(), history + 2);
    let settings = |request: &str| {
        &results.iter().find(|reply| reply["request_id"] == request).unwrap()["value"]["snapshot"]["settings"]
    };
    assert_eq!(settings("crop")["start_ms"], 250.75);
    assert_eq!(settings("crop")["end_ms"], 700.0);
    assert_eq!(settings("through-end")["start_ms"], 300.0);
    assert!(settings("through-end")["end_ms"].is_null());
}
