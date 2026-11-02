use super::*;
use crate::media::channel;
use golden_core::{app::to_sparse_project_json_pretty, node::Node};

#[test]
fn generic_channel_copy_rejects_derived_and_foreign_owners_before_any_batch_mutation() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let destination = create(&mut engine, MediaKind::SolidColor);
    let composition = create(&mut engine, MediaKind::Composition);
    let source_id = engine.node_id_by_uuid(source.channel.0).unwrap();
    let composition_id = engine.node_id_by_uuid(composition.channel.0).unwrap();
    let source_parent = engine.node_id_by_uuid(source.media.0).unwrap();
    let composition_parent = engine.node_id_by_uuid(composition.media.0).unwrap();
    let destination_parent = engine.node_id_by_uuid(destination.media.0).unwrap();
    // A loaded or scripted permission grant must not override document ownership.
    let data = engine.nodes.get_mut(composition_id).unwrap().node_data_mut();
    data.user_role = golden_core::node::UserNodeRole::ItemRoot;
    data.meta.user_permissions = golden_core::node::NodeUserPermissions::all();
    let spec = |source, new_parent| golden_core::ui_sync::UiDuplicateNodeSpec {
        source,
        new_parent,
        new_prev_sibling: None,
        initial_params: vec![],
    };
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    for intent in [
        UiEditIntent::DuplicateNode {
            source: composition_id,
            new_parent: composition_parent,
            new_prev_sibling: None,
            initial_params: vec![],
        },
        UiEditIntent::DuplicateNode {
            source: source_id,
            new_parent: destination_parent,
            new_prev_sibling: None,
            initial_params: vec![],
        },
        UiEditIntent::DuplicateNodes {
            nodes: vec![spec(source_id, source_parent), spec(composition_id, composition_parent)],
            created_items: vec![],
            dependent_items: vec![],
        },
    ] {
        let ack = apply_project_intent(&mut engine, intent);
        assert!(!ack.success);
        assert!(ack.error_message.unwrap().contains("only source channel variants"));
        assert_eq!(engine.undo_len(), history);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    }
}

#[test]
fn channel_moves_skip_declared_folders_and_boundaries_are_history_free() {
    let mut engine = fixture();
    let first = create(&mut engine, MediaKind::SolidColor);
    let MediaEditResult::Created { reference: second } = apply(
        &mut engine,
        MediaEdit::AddChannel {
            media: first.media,
            label: None,
        },
    ) else {
        panic!()
    };
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    apply(
        &mut engine,
        MediaEdit::MoveChannel {
            reference: first,
            direction: ChannelMoveDirection::Earlier,
        },
    );
    assert_eq!(engine.undo_len(), history);
    apply(
        &mut engine,
        MediaEdit::MoveChannel {
            reference: first,
            direction: ChannelMoveDirection::Later,
        },
    );
    assert_eq!(engine.undo_len(), history + 1);
    let MediaEditResult::Manager { snapshot } = apply(&mut engine, MediaEdit::InspectManager) else {
        panic!()
    };
    assert_eq!(
        snapshot.media[0]
            .channels
            .iter()
            .map(|channel| channel.reference)
            .collect::<Vec<_>>(),
        [second, first]
    );
    apply(
        &mut engine,
        MediaEdit::MoveChannel {
            reference: first,
            direction: ChannelMoveDirection::Later,
        },
    );
    assert_eq!(engine.undo_len(), history + 1);
    assert!(engine.undo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
}

#[test]
fn source_channel_catalogs_keep_selection_and_parameter_folders_never_inherit_creation() {
    let mut engine = fixture();
    for kind in [
        MediaKind::SolidColor,
        MediaKind::Picture,
        MediaKind::SharedTexture,
        MediaKind::Composition,
        MediaKind::Matrix,
    ] {
        let reference = create(&mut engine, kind);
        let media = engine.node_id_by_uuid(reference.media.0).unwrap();
        let channel = engine.node_id_by_uuid(reference.channel.0).unwrap();
        let items = engine.catalog_creatable_items(media);
        if kind.has_authored_channel_variants() {
            assert_eq!(items.len(), 1);
            assert!(!items[0].select_when_created);
        } else {
            assert!(items.is_empty());
            let before = to_sparse_project_json_pretty(&engine).unwrap();
            let history = engine.undo_len();
            assert!(
                !invoke(
                    &mut engine,
                    MediaEdit::AddChannel {
                        media: reference.media,
                        label: None
                    }
                )
                .success
            );
            assert_eq!(engine.undo_len(), history);
            assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
        }
        for root in [media, channel] {
            let mut child = engine.nodes.get(root).unwrap().node_data().first_child;
            let folder = loop {
                let id = child.expect("declared Parameters folder");
                let node = engine.nodes.get(id).unwrap();
                if crate::parameters::is_parameter_folder(node) {
                    break id;
                }
                child = node.node_data().next_sibling;
            };
            assert!(crate::parameters::is_parameter_folder(
                engine.nodes.get(folder).unwrap()
            ));
            assert!(engine.catalog_creatable_items(folder).is_empty());
            assert!(
                engine
                    .nodes
                    .get(folder)
                    .unwrap()
                    .user_container_rules()
                    .unwrap()
                    .accepts_item_kinds
                    .is_empty()
            );
        }
    }
}

#[test]
fn channel_generic_duplication_keeps_typed_content_independent_with_history_and_project_identity() {
    let mut engine = fixture();
    let original = create(&mut engine, MediaKind::SolidColor);
    let root = engine.node_id_by_uuid(original.channel.0).unwrap();
    let before = engine.undo_len();
    let parent = engine.node_id_by_uuid(original.media.0).unwrap();
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::DuplicateNode {
            source: root,
            new_parent: parent,
            new_prev_sibling: Some(root),
            initial_params: vec![],
        },
    );
    assert!(ack.success, "{ack:?}");
    assert_eq!(engine.undo_len(), before + 1);
    let MediaEditResult::Manager { snapshot } = apply(&mut engine, MediaEdit::InspectManager) else {
        panic!()
    };
    let copy = snapshot.media[0].channels[1].reference;
    assert_ne!(copy, original);
    assert_eq!(
        channel(&engine, copy).unwrap().definition,
        channel(&engine, original).unwrap().definition
    );
    assert!(engine.undo().unwrap());
    assert!(channel(&engine, copy).is_err());
    assert!(engine.redo().unwrap());
    let json = to_sparse_project_json_pretty(&engine).unwrap();
    let loaded = golden_core::app::from_sparse_project_json::<crate::app::AppNode>(&json).unwrap();
    assert_eq!(channel(&loaded, copy).unwrap(), channel(&engine, copy).unwrap());
}
