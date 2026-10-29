use super::*;

#[test]
fn disabling_a_channel_invalidates_its_capture() {
    use golden_core::{edit::Edit, node::NodeMetaPatch};
    let mut engine = engine();
    let reference = create(&mut engine, MediaKind::SharedTexture);
    let captured = capture_channels(&engine, &[reference], 16).unwrap();
    engine.edits.push(Edit::PatchMeta {
        node: engine.node_id_by_uuid(reference.channel.0).unwrap(),
        patch: NodeMetaPatch {
            enabled: Some(false),
            ..Default::default()
        },
    });
    engine.apply_edits().unwrap();
    assert!(!captured.is_current());
    let current = capture_channels(&engine, &[reference], 16).unwrap();
    assert!(!current.channels[0].enabled);
}

#[test]
fn captures_fence_configuration_history_membership_and_project_reload() {
    let mut engine = engine();
    let reference = create(&mut engine, MediaKind::SolidColor);
    let capture = capture_channels(&engine, &[reference], 16).unwrap();
    assert!(capture.is_current());
    resize(&mut engine, reference, 2);
    assert!(!capture.is_current());
    let changed = capture_channels(&engine, &[reference], 16).unwrap();
    engine.undo().unwrap();
    assert!(!changed.is_current());
    let MediaEditResult::Created { reference: another } = edit(
        &mut engine,
        MediaEdit::AddChannel {
            media: reference.media,
            label: None,
        },
    ) else {
        panic!()
    };
    edit(&mut engine, MediaEdit::RemoveChannel { reference: another });
    let missing = capture_channels(&engine, &[another], 16).unwrap();
    assert!(missing.channels[0].definition.is_none());
    engine.undo().unwrap();
    assert!(!missing.is_current(), "undo must invalidate a captured missing channel");
    let captured = capture_channels(&engine, &[reference], 16).unwrap();
    edit(&mut engine, MediaEdit::RemoveMedia { media: reference.media });
    assert!(
        !captured.is_current(),
        "history-retained node payloads must not keep removed sources current"
    );
    engine.undo().unwrap();
    let captured = capture_channels(&engine, &[reference], 16).unwrap();
    let json = golden_core::app::to_sparse_project_json_pretty(&engine).unwrap();
    let mut restored = golden_core::app::from_sparse_project_json::<crate::app::AppNode>(&json).unwrap();
    restored.apply_edits().unwrap();
    assert_ne!(
        captured.project,
        capture_channels(&restored, &[reference], 16).unwrap().project
    );
    drop(engine);
    assert!(!captured.is_current());
}

#[test]
fn capture_is_bounded_and_wrong_owner_stays_missing_without_fallback() {
    let mut engine = engine();
    let a = create(&mut engine, MediaKind::SolidColor);
    let b = create(&mut engine, MediaKind::SolidColor);
    assert!(capture_channels(&engine, &[a, b], 1).is_err());
    let reference = ChannelReference {
        media: a.media,
        channel: b.channel,
    };
    let capture = capture_channels(&engine, &[reference, reference], 2).unwrap();
    assert_eq!(capture.channels.len(), 1);
    assert!(capture.channels[0].definition.is_none());
}
