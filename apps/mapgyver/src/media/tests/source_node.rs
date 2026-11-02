use super::*;
use crate::media::resolve_source_node;
use golden_core::node::{Node, NodeUuid};

#[test]
fn source_nodes_resolve_actual_media_and_channels_in_authored_channel_order() {
    let mut engine = fixture();
    for kind in [
        MediaKind::Picture,
        MediaKind::SolidColor,
        MediaKind::SharedTexture,
        MediaKind::Composition,
        MediaKind::Matrix,
    ] {
        let reference = create(&mut engine, kind);
        assert_eq!(resolve_source_node(&engine, reference.media.0).unwrap(), reference);
        assert_eq!(resolve_source_node(&engine, reference.channel.0).unwrap(), reference);
    }
    let MediaEditResult::FolderCreated { folder } = apply(
        &mut engine,
        MediaEdit::CreateFolder {
            parent: None,
            label: None,
        },
    ) else {
        panic!()
    };
    let MediaEditResult::Created { reference: first } = apply(
        &mut engine,
        MediaEdit::CreateMediaInFolder {
            folder,
            media_kind: MediaKind::Picture,
            label: None,
        },
    ) else {
        panic!()
    };
    let MediaEditResult::Created { reference: second } = apply(
        &mut engine,
        MediaEdit::AddChannel {
            media: first.media,
            label: None,
        },
    ) else {
        panic!()
    };
    apply(
        &mut engine,
        MediaEdit::ReorderChannel {
            reference: second,
            after: None,
        },
    );
    let history = engine.undo_len();
    assert_eq!(resolve_source_node(&engine, first.media.0).unwrap(), second);
    assert_eq!(resolve_source_node(&engine, first.channel.0).unwrap(), first);
    for invalid in [
        folder,
        engine.nodes.get(engine.root).unwrap().node_data().meta.uuid,
        NodeUuid::nil(),
    ] {
        assert!(resolve_source_node(&engine, invalid).is_err());
    }
    assert_eq!(engine.undo_len(), history, "source resolution must be read-only");
    apply(&mut engine, MediaEdit::RemoveChannel { reference: second });
    assert!(resolve_source_node(&engine, second.channel.0).is_err());
    assert_eq!(resolve_source_node(&engine, first.media.0).unwrap(), first);
}
