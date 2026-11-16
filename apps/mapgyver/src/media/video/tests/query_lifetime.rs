use crate::{
    app::AppEngine,
    media::{
        protocol::*,
        tests::{apply, create, fixture},
    },
};
use golden_core::app::{from_sparse_project_json, to_sparse_project_json_pretty};

fn read(engine: &mut AppEngine, reference: ChannelReference) -> Option<super::super::protocol::VideoSourceStatus> {
    let before = to_sparse_project_json_pretty(engine).unwrap();
    let history = engine.ui_history_state();
    let payload = serde_json::to_value(MediaEdit::InspectVideoSource { reference }).unwrap();
    let response = crate::media::queries::query_operation(engine, MEDIA_OPERATION, &payload)
        .unwrap()
        .unwrap();
    let MediaEditResult::VideoSource { status } = serde_json::from_value(response).unwrap() else {
        panic!()
    };
    let MediaEditResult::VideoSource { status: ordinary } = apply(engine, MediaEdit::InspectVideoSource { reference })
    else {
        panic!()
    };
    assert_eq!(status, ordinary);
    assert_eq!(engine.ui_history_state(), history);
    assert_eq!(to_sparse_project_json_pretty(engine).unwrap(), before);
    status
}

#[test]
fn video_source_status_lifetime_handles_removal_undo_reload_and_new_without_reviving_old_targets() {
    let mut engine = fixture();
    let reference = create(&mut engine, MediaKind::Video);
    apply(
        &mut engine,
        MediaEdit::AddChannel {
            media: reference.media,
            label: None,
        },
    );
    assert!(read(&mut engine, reference).is_some());
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    apply(&mut engine, MediaEdit::RemoveChannel { reference });
    assert!(read(&mut engine, reference).is_none());
    assert!(engine.undo().unwrap());
    assert!(read(&mut engine, reference).is_some());
    apply(&mut engine, MediaEdit::RemoveMedia { media: reference.media });
    assert!(read(&mut engine, reference).is_none());
    assert!(engine.undo().unwrap());
    assert!(read(&mut engine, reference).is_some());
    let mut replacement = fixture();
    assert!(read(&mut replacement, reference).is_none());
    let mut loaded = from_sparse_project_json::<crate::app::AppNode>(&saved).unwrap();
    loaded.apply_edits().unwrap();
    assert!(read(&mut loaded, reference).is_some());
}

#[test]
fn video_source_status_lifetime_keeps_live_wrong_kind_and_foreign_channel_rejections() {
    let mut engine = fixture();
    let first = create(&mut engine, MediaKind::Video);
    let other = create(&mut engine, MediaKind::Video);
    let solid = create(&mut engine, MediaKind::SolidColor);
    for reference in [
        solid,
        ChannelReference {
            media: first.media,
            channel: other.channel,
        },
    ] {
        let before = to_sparse_project_json_pretty(&engine).unwrap();
        let payload = serde_json::to_value(MediaEdit::InspectVideoSource { reference }).unwrap();
        assert!(
            crate::media::queries::query_operation(&engine, MEDIA_OPERATION, &payload)
                .unwrap()
                .is_err()
        );
        assert!(!crate::media::tests::invoke(&mut engine, MediaEdit::InspectVideoSource { reference }).success);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    }
}
