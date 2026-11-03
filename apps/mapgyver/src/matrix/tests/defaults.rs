use super::*;
use golden_core::node::Node;

fn create_actual(engine: &mut AppEngine) -> ChannelReference {
    let MediaEditResult::Created { reference } = apply(
        engine,
        MediaEdit::CreateMedia {
            media_kind: MediaKind::Matrix,
            label: None,
        },
    ) else {
        panic!()
    };
    reference
}
fn verify(engine: &mut AppEngine, reference: ChannelReference) -> MatrixSnapshot {
    let snapshot = inspect(engine, reference.media);
    assert_eq!(snapshot.final_channel, reference);
    assert_eq!(snapshot.items.len(), 3);
    assert_eq!(snapshot.columns.len(), 3);
    assert!(snapshot.cells.is_empty(), "the nine empty addresses remain sparse");
    for (index, row) in snapshot.items.iter().enumerate() {
        assert_eq!(row.label, format!("Layer {}", index + 1));
        assert!(matches!(row.id, MatrixItemId::Layer { .. }));
        assert_eq!(row.parent, None);
        assert_eq!(row.settings, CompositeSettings::default());
        assert_eq!(row.layer, Some(LayerSettings::default()));
        assert_ne!(row.channel, reference);
    }
    for (index, column) in snapshot.columns.iter().enumerate() {
        assert_eq!(column.label, format!("Column {}", index + 1));
        assert_eq!(column.definition, ColumnDefinition::default());
    }
    crate::media::validate_project(engine).unwrap();
    snapshot
}

#[test]
fn backend_creation_is_one_history_step_and_defaults_survive_exact_undo_copy_and_project_without_regeneration() {
    let mut engine = fixture();
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    let reference = create_actual(&mut engine);
    assert_eq!(engine.undo_len(), history + 1);
    let original = verify(&mut engine, reference);
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    assert!(engine.undo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    assert!(engine.redo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    assert_eq!(verify(&mut loaded, reference), original);
    let owner = engine.node_id_by_uuid(reference.media.0).unwrap();
    let parent = engine.nodes.get(owner).unwrap().node_data().parent.unwrap();
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::DuplicateNode {
            source: owner,
            new_parent: parent,
            new_prev_sibling: Some(owner),
            initial_params: vec![],
        },
    );
    assert!(ack.success, "{ack:?}");
    let duplicate = engine
        .nodes
        .iter()
        .find(|(_, node)| {
            node.node_data().parent == Some(parent)
                && node.node_data().meta.uuid != reference.media.0
                && matches!(node, AppNode::Media(media) if media.kind() == MediaKind::Matrix)
        })
        .unwrap()
        .1
        .node_data()
        .meta
        .uuid;
    let copied = inspect(&mut engine, MediaId(duplicate));
    verify(&mut engine, copied.final_channel);
    assert_eq!(copied.items.len(), 3);
    assert_eq!(copied.columns.len(), 3);
    assert!(copied.cells.is_empty());
    assert_ne!(copied.final_channel, reference);
    assert!(copied.items.iter().all(|row| {
        original
            .items
            .iter()
            .all(|old| old.id != row.id && old.channel != row.channel)
    }));
    assert!(
        copied
            .columns
            .iter()
            .all(|column| original.columns.iter().all(|old| old.id != column.id))
    );
    clear_scene(&mut engine, reference.media);
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    let empty = inspect(&mut loaded, reference.media);
    assert!(empty.items.is_empty() && empty.columns.is_empty() && empty.cells.is_empty());
    assert_eq!(empty.final_channel, reference);
}

#[test]
fn manager_and_folder_catalog_creation_have_the_same_three_by_three_defaults() {
    let mut engine = fixture();
    let manager = crate::media::authoring::manager(&engine).unwrap();
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::CreateUserItem {
            parent: manager,
            node_type: "mapgyver.media.folder".into(),
            label: Some("Defaults folder".into()),
            initial_params: vec![],
        },
    );
    assert!(ack.success, "{ack:?}");
    let folder = engine
        .nodes
        .iter()
        .find(|(_, node)| node.node_data().meta.label == "Defaults folder")
        .unwrap()
        .0;
    for parent in [manager, folder] {
        let history = engine.undo_len();
        let ack = apply_project_intent(
            &mut engine,
            UiEditIntent::CreateUserItem {
                parent,
                node_type: "mapgyver.media.matrix".into(),
                label: None,
                initial_params: vec![],
            },
        );
        assert!(ack.success, "{ack:?}");
        assert_eq!(engine.undo_len(), history + 1);
        let media = engine
            .nodes
            .iter()
            .find(|(_, node)| {
                node.node_data().parent == Some(parent)
                    && matches!(node, AppNode::Media(media) if media.kind() == MediaKind::Matrix)
            })
            .unwrap()
            .1
            .node_data()
            .meta
            .uuid;
        let reference = inspect(&mut engine, MediaId(media)).final_channel;
        verify(&mut engine, reference);
    }
}

#[test]
fn atomic_multi_owner_copy_preserves_all_paired_exports_with_independent_identities() {
    let mut engine = fixture();
    let references = [create_actual(&mut engine), create_actual(&mut engine)];
    let manager = crate::media::authoring::manager(&engine).unwrap();
    let nodes = references
        .iter()
        .map(|reference| golden_core::ui_sync::UiDuplicateNodeSpec {
            source: engine.node_id_by_uuid(reference.media.0).unwrap(),
            new_parent: manager,
            new_prev_sibling: None,
            initial_params: vec![],
        })
        .collect();
    let history = engine.undo_len();
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::DuplicateNodes {
            nodes,
            created_items: vec![],
            dependent_items: vec![],
        },
    );
    assert!(ack.success, "{ack:?}");
    assert_eq!(engine.undo_len(), history + 1);
    let MediaEditResult::Manager { snapshot } = apply(&mut engine, MediaEdit::InspectManager) else {
        panic!()
    };
    let media = snapshot
        .media
        .into_iter()
        .filter(|media| media.media_kind == MediaKind::Matrix)
        .collect::<Vec<_>>();
    assert_eq!(media.len(), 4);
    let mut identities = std::collections::HashSet::new();
    for media in media {
        let reference = inspect(&mut engine, media.id).final_channel;
        let snapshot = verify(&mut engine, reference);
        assert!(identities.insert(snapshot.final_channel.channel.0));
        for row in snapshot.items {
            assert!(identities.insert(row.channel.channel.0));
        }
    }
    assert_eq!(identities.len(), 16);
}
