use super::*;

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
            function onAppOperationResult(reply) {{ gc.emit('media-method-reply', reply); }}
            "#
        ),
    );
    // The script service deliberately limits requests per engine turn.
    service_script_app_operations(&mut engine).unwrap();
    service_script_app_operations(&mut engine).unwrap();
    let results = replies(&engine);
    assert_eq!(results.len(), 13, "each actual QuickJS call has one callback");
    for request in [
        "file", "seek", "speed", "loop", "load", "play", "pause", "stop", "inspect", "frame", "source",
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
    assert_eq!(replies(&engine).len(), 13);
    assert_eq!(
        engine.undo_len(),
        history,
        "callbacks and read-only inspection add no history"
    );
    assert!(engine.undo().unwrap());
    assert!(engine.redo().unwrap());
    service_script_app_operations(&mut engine).unwrap();
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), after);
    assert_eq!(replies(&engine).len(), 13, "history does not replay requests");
}
