use super::*;
use crate::{app::AppNode, media::channel};
use golden_core::{
    app::{from_sparse_project_json, to_sparse_project_json_pretty},
    node::Node,
};

fn sender(engine: &AppEngine, reference: ChannelReference) -> Option<String> {
    let ChannelSource::Spout { sender } = channel(engine, reference).unwrap().definition.source else {
        panic!()
    };
    sender
}

#[test]
fn exact_missing_senders_survive_media_mode_switch_history_and_project_roundtrip() {
    let mut engine = fixture();
    let first = create(&mut engine, MediaKind::SharedTexture);
    let MediaEditResult::Created { reference: second } = apply(
        &mut engine,
        MediaEdit::AddChannel {
            media: first.media,
            label: None,
        },
    ) else {
        panic!()
    };
    for (reference, name) in [(first, "Missing exact α"), (second, "Independent sender")] {
        apply(
            &mut engine,
            MediaEdit::ConfigureChannel {
                reference,
                source: ChannelSource::Spout {
                    sender: Some(name.into()),
                },
            },
        );
    }
    let before = engine.undo_len();
    apply(
        &mut engine,
        MediaEdit::ConfigureMediaSenderSelection {
            media: first.media,
            mode: SenderSelectionMode::Auto,
        },
    );
    assert_eq!(engine.undo_len(), before + 1);
    assert_eq!(sender(&engine, first).as_deref(), Some("Missing exact α"));
    assert_eq!(sender(&engine, second).as_deref(), Some("Independent sender"));
    apply(
        &mut engine,
        MediaEdit::ConfigureChannel {
            reference: first,
            source: ChannelSource::Spout {
                sender: Some("Auto replacement".into()),
            },
        },
    );
    apply(
        &mut engine,
        MediaEdit::ConfigureMediaSenderSelection {
            media: first.media,
            mode: SenderSelectionMode::Manual,
        },
    );
    assert_eq!(sender(&engine, first).as_deref(), Some("Auto replacement"));
    assert!(engine.undo().unwrap());
    assert_eq!(sender(&engine, first).as_deref(), Some("Auto replacement"));
    assert!(engine.undo().unwrap());
    assert_eq!(sender(&engine, first).as_deref(), Some("Missing exact α"));
    assert!(engine.undo().unwrap());
    assert_eq!(sender(&engine, second).as_deref(), Some("Independent sender"));
    assert!(engine.redo().unwrap());
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    loaded.apply_edits().unwrap();
    assert_eq!(sender(&loaded, first).as_deref(), Some("Missing exact α"));
    let media = loaded
        .nodes
        .get(loaded.node_id_by_uuid(first.media.0).unwrap())
        .unwrap();
    let AppNode::Media(media) = media else { panic!() };
    assert_eq!(media.sender_selection(), Some(SenderSelectionMode::Auto));
    assert!(!media.node_data().meta.uuid.0.is_nil());
}

#[test]
fn mode_intent_rejects_non_shared_media_without_history() {
    let mut engine = fixture();
    let reference = create(&mut engine, MediaKind::SolidColor);
    let before = engine.undo_len();
    assert!(
        !invoke(
            &mut engine,
            MediaEdit::ConfigureMediaSenderSelection {
                media: reference.media,
                mode: SenderSelectionMode::Auto
            }
        )
        .success
    );
    assert_eq!(engine.undo_len(), before);
}
