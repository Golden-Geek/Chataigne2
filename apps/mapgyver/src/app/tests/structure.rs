use super::*;
use crate::{app::AppEngine, media::protocol::*, screens::protocol::*};
use golden_core::{
    app::ProjectLifecycle,
    application::apply_project_intent,
    node::{NodeId, NodeUuid, UserNodeRole},
    ui_sync::UiEditIntent,
};

fn media(engine: &mut AppEngine, edit: MediaEdit) -> MediaEditResult {
    let ack = apply_project_intent(
        engine,
        UiEditIntent::InvokeAppOperation {
            namespace: MEDIA_NAMESPACE.into(),
            operation: MEDIA_OPERATION.into(),
            payload: serde_json::to_value(edit).unwrap(),
        },
    );
    assert!(ack.success, "{ack:?}");
    serde_json::from_value(ack.result.unwrap().value).unwrap()
}
fn source(engine: &mut AppEngine, kind: MediaKind) -> ChannelReference {
    let MediaEditResult::Created { reference } = media(
        engine,
        MediaEdit::CreateMedia {
            media_kind: kind,
            label: None,
        },
    ) else {
        panic!()
    };
    reference
}
fn screen(engine: &mut AppEngine) -> (ScreenId, SurfaceReference) {
    let ack = apply_project_intent(
        engine,
        UiEditIntent::InvokeAppOperation {
            namespace: SCREEN_NAMESPACE.into(),
            operation: SCREEN_OPERATION.into(),
            payload: serde_json::to_value(ScreenEdit::CreateScreen { label: None }).unwrap(),
        },
    );
    assert!(ack.success, "{ack:?}");
    let ScreenEditResult::ScreenCreated { screen, surface } =
        serde_json::from_value(ack.result.unwrap().value).unwrap()
    else {
        panic!()
    };
    (screen, surface)
}
fn move_item(engine: &mut AppEngine, uuid: NodeUuid, parent: NodeId, after: Option<NodeId>) {
    let before = to_sparse_project_json_pretty(engine).unwrap();
    let history = engine.undo_len();
    let node = engine.node_id_by_uuid(uuid).unwrap();
    assert_eq!(
        engine.nodes.get(node).unwrap().node_data().user_role,
        UserNodeRole::ItemRoot
    );
    let ack = apply_project_intent(
        engine,
        UiEditIntent::MoveNode {
            node,
            new_parent: parent,
            new_prev_sibling: after,
        },
    );
    assert!(ack.success, "{ack:?}");
    assert_eq!(engine.undo_len(), history + 1);
    let after = to_sparse_project_json_pretty(engine).unwrap();
    assert!(engine.undo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(engine).unwrap(), before);
    assert!(engine.redo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(engine).unwrap(), after);
}

#[test]
fn candidate_preparation_keeps_declared_matrix_app_controls_intrinsic_across_reload_and_live_group_moves() {
    use crate::matrix::protocol::*;
    let mut engine = create_new_project_engine::<AppNode>().unwrap();
    let matrix = source(&mut engine, MediaKind::Matrix);
    let MediaEditResult::Matrix { snapshot } = media(
        &mut engine,
        MediaEdit::Matrix {
            media: matrix.media,
            edit: MatrixEdit::Inspect,
        },
    ) else {
        panic!()
    };
    let columns = snapshot.columns.iter().map(|column| column.id).collect::<Vec<_>>();
    let MatrixItemId::Layer { id: layer } = snapshot.items[0].id else {
        panic!()
    };
    media(
        &mut engine,
        MediaEdit::Matrix {
            media: matrix.media,
            edit: MatrixEdit::ConfigureCell {
                layer,
                column: columns[0],
                settings: CellSettings::default(),
            },
        },
    );
    let MediaEditResult::MatrixColumnGroupCreated { group } = media(
        &mut engine,
        MediaEdit::Matrix {
            media: matrix.media,
            edit: MatrixEdit::AddColumnGroup {
                parent: None,
                label: None,
            },
        },
    ) else {
        panic!()
    };
    let MediaEditResult::MatrixColumnGroupCreated { group: nested } = media(
        &mut engine,
        MediaEdit::Matrix {
            media: matrix.media,
            edit: MatrixEdit::AddColumnGroup {
                parent: Some(group),
                label: None,
            },
        },
    ) else {
        panic!()
    };
    let controls = engine
        .nodes
        .iter()
        .filter_map(|(_, node)| {
            matches!(
                node,
                AppNode::MatrixColumn(_) | AppNode::MatrixColumnGroup(_) | AppNode::MatrixCell(_)
            )
            .then_some(node.node_data().meta.uuid)
        })
        .collect::<Vec<_>>();
    assert_eq!(controls.len(), 6);
    let encoded = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&encoded).unwrap();
    let controls = controls
        .into_iter()
        .map(|owner| {
            let owner_id = loaded.node_id_by_uuid(owner).unwrap();
            let control = match loaded.nodes.get(owner_id).unwrap() {
                AppNode::MatrixColumn(node) => node.application_node(),
                AppNode::MatrixColumnGroup(node) => node.application_node(),
                AppNode::MatrixCell(node) => node.application_node(),
                _ => None,
            }
            .unwrap();
            (control, owner)
        })
        .collect::<Vec<_>>();
    for corrupt in [false, true] {
        if corrupt {
            for (control, _) in &controls {
                let data = loaded.nodes.get_mut(*control).unwrap().node_data_mut();
                data.user_role = UserNodeRole::ItemRoot;
                data.meta.user_permissions.can_edit_name = true;
                data.meta.user_permissions.can_remove_and_duplicate = true;
            }
        }
        AppNode::prepare_project_candidate(&mut loaded).unwrap();
        for (control, owner) in &controls {
            let data = loaded.nodes.get(*control).unwrap().node_data();
            assert_eq!(
                data.user_role,
                UserNodeRole::Regular,
                "declared App Control {control:?}"
            );
            assert_eq!(data.meta.user_permissions, Default::default());
            assert_eq!(data.parent, loaded.node_id_by_uuid(*owner));
            assert_eq!(data.meta.decl_id.0, "app_control");
        }
    }
    golden_core::app::prepare_engine_for_runtime(&mut loaded).unwrap();
    for (column, parent) in [(columns[0], group), (columns[1], nested)] {
        media(
            &mut loaded,
            MediaEdit::Matrix {
                media: matrix.media,
                edit: MatrixEdit::MoveColumnItem {
                    item: ColumnItemId::Column { id: column },
                    parent: Some(parent),
                    after: None,
                },
            },
        );
    }
    let saved = to_sparse_project_json_pretty(&loaded).unwrap();
    assert!(loaded.undo().unwrap());
    assert!(loaded.redo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(&loaded).unwrap(), saved);
}

#[test]
fn shared_tree_sorts_screens_surfaces_media_channels_and_whole_matrix_owners_with_exact_history() {
    let mut engine = create_new_project_engine::<AppNode>().unwrap();
    engine.apply_edits().unwrap();
    let a = source(&mut engine, MediaKind::SolidColor);
    let b = source(&mut engine, MediaKind::Picture);
    let matrix = source(&mut engine, MediaKind::Matrix);
    let owner = engine
        .nodes
        .get(engine.node_id_by_uuid(a.media.0).unwrap())
        .unwrap()
        .node_data()
        .parent
        .unwrap();
    move_item(&mut engine, b.media.0, owner, None);
    move_item(&mut engine, matrix.media.0, owner, None);
    let MediaEditResult::Created { reference: channel } = media(
        &mut engine,
        MediaEdit::AddChannel {
            media: a.media,
            label: None,
        },
    ) else {
        panic!()
    };
    let parent = engine.node_id_by_uuid(a.media.0).unwrap();
    move_item(&mut engine, channel.channel.0, parent, None);
    let (first, surface) = screen(&mut engine);
    let (second, _) = screen(&mut engine);
    let screen_parent = engine
        .nodes
        .get(engine.node_id_by_uuid(first.0).unwrap())
        .unwrap()
        .node_data()
        .parent
        .unwrap();
    move_item(&mut engine, second.0, screen_parent, None);
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::InvokeAppOperation {
            namespace: SCREEN_NAMESPACE.into(),
            operation: SCREEN_OPERATION.into(),
            payload: serde_json::to_value(ScreenEdit::AddSurface {
                screen: first,
                label: None,
            })
            .unwrap(),
        },
    );
    assert!(ack.success, "{ack:?}");
    let ScreenEditResult::SurfaceCreated { reference } = serde_json::from_value(ack.result.unwrap().value).unwrap()
    else {
        panic!()
    };
    let parent = engine.node_id_by_uuid(first.0).unwrap();
    move_item(&mut engine, reference.surface.0, parent, None);
    assert!(crate::media::channel(&engine, a).is_ok());
    assert!(engine.node_id_by_uuid(surface.surface.0).is_some());
    AppNode::prepare_project_candidate(&mut engine).unwrap();
}

#[test]
fn shared_catalog_creation_builds_complete_typed_media_and_screen_forests() {
    let mut engine = create_new_project_engine::<AppNode>().unwrap();
    engine.apply_edits().unwrap();
    let manager = crate::media::authoring::manager(&engine).unwrap();
    let catalog = engine.catalog_creatable_items(manager);
    for item in catalog
        .into_iter()
        .filter(|item| item.item_kind == "mapgyver.media.v8" || item.node_type == "mapgyver.media.folder")
    {
        let history = engine.undo_len();
        let ack = apply_project_intent(
            &mut engine,
            UiEditIntent::CreateUserItem {
                parent: manager,
                node_type: item.node_type,
                label: None,
                initial_params: vec![],
            },
        );
        assert!(ack.success, "{ack:?}");
        assert_eq!(engine.undo_len(), history + 1);
    }
    let MediaEditResult::Manager { snapshot } = media(&mut engine, MediaEdit::InspectManager) else {
        panic!()
    };
    assert_eq!(snapshot.media.len(), 6);
    assert!(snapshot.media.iter().all(|media| media.channels.len() == 1));
    let screen_manager = engine
        .nodes
        .iter()
        .find_map(|(id, node)| matches!(node, AppNode::ScreenManager(_)).then_some(id))
        .unwrap();
    let item = engine
        .catalog_creatable_items(screen_manager)
        .into_iter()
        .find(|item| item.node_type == "mapgyver.screen")
        .unwrap();
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::CreateUserItem {
            parent: screen_manager,
            node_type: item.node_type,
            label: None,
            initial_params: vec![],
        },
    );
    assert!(ack.success, "{ack:?}");
    let created = crate::screens::inspect(&engine).unwrap();
    assert_eq!(created.screens.len(), 2);
    assert_eq!(created.screens[0].surfaces.len(), 1);
    let json = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&json).unwrap();
    loaded.apply_edits().unwrap();
    AppNode::prepare_project_candidate(&mut loaded).unwrap();
    assert_eq!(crate::screens::inspect(&loaded).unwrap(), created);
}

#[test]
fn shared_tree_rejects_owner_breakage_and_last_channel_deletion_without_mutation() {
    let mut engine = create_new_project_engine::<AppNode>().unwrap();
    engine.apply_edits().unwrap();
    let a = source(&mut engine, MediaKind::SolidColor);
    let b = source(&mut engine, MediaKind::SolidColor);
    let (screen, surface) = screen(&mut engine);
    let channel = engine.node_id_by_uuid(a.channel.0).unwrap();
    let media = engine.node_id_by_uuid(a.media.0).unwrap();
    let other = engine.node_id_by_uuid(b.media.0).unwrap();
    let screen = engine.node_id_by_uuid(screen.0).unwrap();
    let surface = engine.node_id_by_uuid(surface.surface.0).unwrap();
    let manager = crate::media::authoring::manager(&engine).unwrap();
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    for intent in [
        UiEditIntent::MoveNode {
            node: channel,
            new_parent: other,
            new_prev_sibling: None,
        },
        UiEditIntent::MoveNode {
            node: media,
            new_parent: screen,
            new_prev_sibling: None,
        },
        UiEditIntent::MoveNode {
            node: surface,
            new_parent: manager,
            new_prev_sibling: None,
        },
        UiEditIntent::RemoveNode { node: channel },
        UiEditIntent::RemoveNodes {
            nodes: vec![channel, channel],
        },
    ] {
        let ack = apply_project_intent(&mut engine, intent);
        assert!(!ack.success, "{ack:?}");
        assert_eq!(engine.undo_len(), history);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    }
}

#[test]
fn candidate_preparation_classifies_existing_authored_items_without_changing_parameters_or_identity() {
    let mut engine = create_new_project_engine::<AppNode>().unwrap();
    engine.apply_edits().unwrap();
    let source = source(&mut engine, MediaKind::Picture);
    let (screen, surface) = screen(&mut engine);
    let ids = [source.media.0, source.channel.0, screen.0, surface.surface.0]
        .map(|uuid| engine.node_id_by_uuid(uuid).unwrap());
    for id in ids {
        engine.nodes.get_mut(id).unwrap().node_data_mut().user_role = UserNodeRole::Regular;
    }
    let values = engine
        .nodes
        .iter()
        .filter_map(|(id, node)| {
            node.engine_param_snapshot()
                .map(|value| (id, node.node_data().meta.uuid, value))
        })
        .collect::<Vec<_>>();
    let history = engine.undo_len();
    AppNode::prepare_project_candidate(&mut engine).unwrap();
    for id in ids {
        assert_eq!(
            engine.nodes.get(id).unwrap().node_data().user_role,
            UserNodeRole::ItemRoot
        );
    }
    for (id, uuid, snapshot) in values {
        let node = engine.nodes.get(id).unwrap();
        assert_eq!(node.node_data().meta.uuid, uuid);
        assert_eq!(node.engine_param_snapshot().unwrap(), snapshot);
    }
    assert_eq!(engine.undo_len(), history);
}
