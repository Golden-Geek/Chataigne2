use super::*;
use crate::app::AppNode;
use golden_core::app::{from_sparse_project_json, to_sparse_project_json_pretty};
use golden_core::node::{Node, NodeUuid};

fn folder(engine: &mut AppEngine, parent: Option<NodeUuid>) -> NodeUuid {
    let MediaEditResult::FolderCreated { folder } = apply(engine, MediaEdit::CreateFolder { parent, label: None })
    else {
        panic!("folder result")
    };
    folder
}

#[test]
fn nested_media_catalog_capture_history_and_project_roundtrip_keep_identities() {
    let mut engine = fixture();
    let outer = folder(&mut engine, None);
    let inner = folder(&mut engine, Some(outer));
    let before = engine.undo_len();
    let MediaEditResult::Created { reference } = apply(
        &mut engine,
        MediaEdit::CreateMediaInFolder {
            folder: inner,
            media_kind: MediaKind::SolidColor,
            label: None,
        },
    ) else {
        panic!()
    };
    assert_eq!(engine.undo_len(), before + 1);
    assert!(super::super::channel(&engine, reference).is_ok());
    let MediaEditResult::Manager { snapshot } = apply(&mut engine, MediaEdit::InspectManager) else {
        panic!()
    };
    assert_eq!(snapshot.media[0].id, reference.media);
    assert_eq!(snapshot.media[0].channels[0].reference, reference);
    assert!(super::super::capture_channels(&engine, &[reference], 64).is_ok());
    assert!(engine.undo().unwrap());
    assert!(super::super::channel(&engine, reference).is_err());
    assert!(engine.redo().unwrap());
    assert!(super::super::channel(&engine, reference).is_ok());
    super::super::validate_project(&engine).unwrap();
    let json = to_sparse_project_json_pretty(&engine).unwrap();
    let restored = from_sparse_project_json::<AppNode>(&json).unwrap();
    super::super::validate_project(&restored).unwrap();
    let id = restored.node_id_by_uuid(reference.media.0).unwrap();
    let parent = restored.nodes.get(id).unwrap().node_data().parent.unwrap();
    assert_eq!(restored.nodes.get(parent).unwrap().node_data().meta.uuid, inner);
    assert!(super::super::channel(&restored, reference).is_ok());
}

#[test]
fn invalid_folder_creation_is_atomic_and_generic_move_preserves_channel_references() {
    let mut engine = fixture();
    let reference = create(&mut engine, MediaKind::Picture);
    let before = engine.undo_len();
    let ack = invoke(
        &mut engine,
        MediaEdit::CreateFolder {
            parent: Some(reference.media.0),
            label: None,
        },
    );
    assert!(!ack.success);
    assert_eq!(engine.undo_len(), before);
    let destination = folder(&mut engine, None);
    let media = engine.node_id_by_uuid(reference.media.0).unwrap();
    let parent = engine.node_id_by_uuid(destination).unwrap();
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::MoveNode {
            node: media,
            new_parent: parent,
            new_prev_sibling: None,
        },
    );
    assert!(ack.success, "{ack:?}");
    assert!(super::super::channel(&engine, reference).is_ok());
    super::super::validate_project(&engine).unwrap();
    assert!(engine.undo().unwrap());
    assert!(super::super::channel(&engine, reference).is_ok());
}

#[test]
fn folder_organization_rejects_cycles_and_restores_nested_contents_on_undo() {
    let mut engine = fixture();
    let outer = folder(&mut engine, None);
    let inner = folder(&mut engine, Some(outer));
    let reference = create(&mut engine, MediaKind::Composition);
    apply(
        &mut engine,
        MediaEdit::MoveEntry {
            entry: reference.media.0,
            parent: Some(inner),
        },
    );
    apply(
        &mut engine,
        MediaEdit::RenameFolder {
            folder: inner,
            label: "Clips".into(),
        },
    );
    let before = engine.undo_len();
    for edit in [
        MediaEdit::MoveEntry {
            entry: outer,
            parent: Some(inner),
        },
        MediaEdit::MoveEntry {
            entry: reference.channel.0,
            parent: Some(inner),
        },
        MediaEdit::RenameFolder {
            folder: inner,
            label: " ".into(),
        },
    ] {
        assert!(!invoke(&mut engine, edit).success);
        assert_eq!(engine.undo_len(), before);
    }
    let id = engine.node_id_by_uuid(inner).unwrap();
    assert_eq!(engine.nodes.get(id).unwrap().node_data().meta.label, "Clips");
    apply(&mut engine, MediaEdit::RemoveFolder { folder: outer });
    assert!(super::super::channel(&engine, reference).is_err());
    assert!(engine.undo().unwrap());
    assert!(super::super::channel(&engine, reference).is_ok());
    super::super::validate_project(&engine).unwrap();
}
