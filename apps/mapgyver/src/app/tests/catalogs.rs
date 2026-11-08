use super::*;
use golden_core::{application::apply_project_intent, ui_sync::UiEditIntent};

#[test]
fn manager_catalogs_expose_document_objects_and_reject_automation_wrappers() {
    let mut engine = create_new_project_engine::<AppNode>().unwrap();
    engine.apply_edits().unwrap();
    let media = crate::media::authoring::manager(&engine).unwrap();
    let screens = engine
        .nodes
        .iter()
        .find_map(|(id, node)| matches!(node, AppNode::ScreenManager(_)).then_some(id))
        .unwrap();
    for (parent, expected) in [
        (
            media,
            vec![
                "Folder",
                "Reference",
                "Solid Color",
                "Picture",
                "Video",
                "Shared Texture",
                "Composition",
                "Matrix",
            ],
        ),
        (screens, vec!["Screen"]),
    ] {
        let catalog = engine.catalog_creatable_items(parent);
        assert_eq!(
            catalog.iter().map(|item| item.label.as_str()).collect::<Vec<_>>(),
            expected
        );
        let before = to_sparse_project_json_pretty(&engine).unwrap();
        let history = engine.undo_len();
        for node_type in ["mapgyver_media_command", "mapgyver_screen_command", "script"] {
            let node = engine.nodes.get(parent).unwrap();
            assert!(node.create_user_item(node_type).is_none());
            assert!(node.create_user_item_tree(node_type).is_none());
            let ack = apply_project_intent(
                &mut engine,
                UiEditIntent::CreateUserItem {
                    parent,
                    node_type: node_type.into(),
                    label: None,
                    initial_params: vec![],
                },
            );
            assert!(!ack.success, "unexpected creation of {node_type}: {ack:?}");
            assert_eq!(engine.undo_len(), history);
            assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
        }
    }
}

#[test]
fn automation_wrapper_types_are_not_registered() {
    for node_type in ["mapgyver_media_command", "mapgyver_screen_command"] {
        assert!(AppNode::project_create(node_type).is_none());
    }
}
