use super::*;
use crate::media::rendering::tests::{
    retention::drop_copy,
    transitions::{fixture, launch, matrix_edit, time},
};
use crate::{
    app::AppNode,
    matrix::protocol::{LaunchMode, MatrixEdit},
    media::capture_channels_at,
};

#[test]
fn removed_owned_video_keeps_live_frames_until_its_matrix_clip_releases_the_source() {
    let (mut engine, matrix, solids, layer, columns) = fixture();
    let video = source(&mut engine, "h264.mp4");
    let owned = drop_copy(&mut engine, matrix, layer, columns[0], video);
    edit(
        &mut engine,
        MediaEdit::Video {
            media: owned.media,
            edit: VideoEdit::Loop { looping: true },
        },
    );
    edit(
        &mut engine,
        MediaEdit::Video {
            media: owned.media,
            edit: VideoEdit::Play,
        },
    );
    let Some(AppNode::Media(node)) = engine.nodes.get(engine.node_id_by_uuid(owned.media.0).unwrap()) else {
        panic!()
    };
    let control = node.video_control().unwrap();
    let time = time();
    launch(&mut engine, matrix.media, columns[0], LaunchMode::Cut);
    let mut renderer = renderer();
    renderer.enable_videos(&fixture_root(), None).unwrap();
    renderer
        .publish(
            capture_channels_at(&engine, &[matrix], 32, time).unwrap(),
            routes(matrix),
        )
        .unwrap();
    wait(|| pixel(&renderer, owned).is_some());
    drop_copy(&mut engine, matrix, layer, columns[0], solids[0]);
    assert!(engine.node_id_by_uuid(owned.media.0).is_none());
    let capture = capture_channels_at(&engine, &[matrix], 32, time).unwrap();
    assert!(
        capture
            .channels
            .iter()
            .any(|channel| channel.reference == owned && channel.owned && channel.definition.is_some())
    );
    renderer.publish(capture, routes(matrix)).unwrap();
    let before = pixel(&renderer, owned).unwrap();
    wait(|| pixel(&renderer, owned).is_some_and(|pixels| pixels != before));
    assert!(control.sample().unwrap().playing);
    assert!(control.snapshot(owned.media).unwrap().ready);
    matrix_edit(&mut engine, matrix.media, MatrixEdit::ClearPlayback);
    renderer
        .publish(
            capture_channels_at(&engine, &[matrix], 32, time).unwrap(),
            routes(matrix),
        )
        .unwrap();
    wait(|| pixel(&renderer, owned).is_none());
    let retirement = renderer.video_retirement().unwrap();
    close(renderer);
    assert!(retirement.wait(Duration::from_secs(5)));
    assert_eq!(retirement.active_workers(), 0);
    assert_eq!(retirement.retained_bytes(), 0);
}
