use super::*;
use crate::{
    applications::protocol::ApplicationEdit,
    media::{composition::protocol::CompositionEdit, protocol::*},
    screens::protocol::{SCREEN_NAMESPACE, SCREEN_OPERATION, ScreenEdit, ScreenEditResult},
};
use golden_core::{
    app::validate_engine_project_candidate, application::apply_project_intent, node::NodeUuid, ui_sync::UiEditIntent,
};

fn media_edit(engine: &mut super::super::AppEngine, edit: MediaEdit) -> MediaEditResult {
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

fn remove_and_restore(engine: &mut super::super::AppEngine, uuid: NodeUuid) {
    let id = engine.node_id_by_uuid(uuid).unwrap();
    let node = engine.nodes.get(id).unwrap();
    assert!(
        node.node_data().meta.user_permissions.can_remove_and_duplicate,
        "{}",
        node.get_type()
    );
    let before = to_sparse_project_json_pretty(engine).unwrap();
    let history = engine.undo_len();
    let ack = apply_project_intent(engine, UiEditIntent::RemoveNode { node: id });
    assert!(ack.success, "{ack:?}");
    assert!(engine.node_id_by_uuid(uuid).is_none());
    assert_eq!(engine.undo_len(), history + 1);
    validate_engine_project_candidate(engine).unwrap();
    assert!(engine.undo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(engine).unwrap(), before);
    assert!(engine.redo().unwrap());
    assert!(engine.node_id_by_uuid(uuid).is_none());
    assert!(engine.undo().unwrap());
}

#[test]
fn every_authored_object_kind_can_be_deleted_from_the_shared_tree_with_history_and_reload() {
    let mut engine = create_new_project_engine::<AppNode>().unwrap();
    engine.apply_edits().unwrap();
    let mut objects = Vec::new();
    let mut composition = None;
    let mut source = None;
    for kind in [
        MediaKind::Picture,
        MediaKind::SolidColor,
        MediaKind::SharedTexture,
        MediaKind::Composition,
        MediaKind::Matrix,
    ] {
        let MediaEditResult::Created { reference } = media_edit(
            &mut engine,
            MediaEdit::CreateMedia {
                media_kind: kind,
                label: None,
            },
        ) else {
            panic!()
        };
        objects.push(reference.media.0);
        if kind == MediaKind::Composition {
            composition = Some(reference);
        }
        if kind == MediaKind::SolidColor {
            source = Some(reference);
        }
    }
    let MediaEditResult::FolderCreated { folder } = media_edit(
        &mut engine,
        MediaEdit::CreateFolder {
            parent: None,
            label: None,
        },
    ) else {
        panic!()
    };
    objects.push(folder);
    let MediaEditResult::AssetCreated { asset } = media_edit(
        &mut engine,
        MediaEdit::ImportAsset {
            locator: AssetLocator::ProjectRelative {
                path: "placeholder.png".to_owned().try_into().unwrap(),
            },
            label: None,
        },
    ) else {
        panic!()
    };
    objects.push(NodeUuid(asset.get()));
    let MediaEditResult::ApplicationCreated { application } = media_edit(
        &mut engine,
        MediaEdit::Applications {
            edit: ApplicationEdit::Create { label: None },
        },
    ) else {
        panic!()
    };
    objects.push(application.0);
    let MediaEditResult::InstancesCreated { instances } = media_edit(
        &mut engine,
        MediaEdit::Composition {
            reference: composition.unwrap(),
            edit: CompositionEdit::AddInstances {
                sources: vec![source.unwrap()],
            },
        },
    ) else {
        panic!()
    };
    objects.extend(instances.into_iter().map(|instance| instance.instance.0));
    let ack = apply_project_intent(
        &mut engine,
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
    objects.extend([surface.surface.0, screen.0]);

    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    loaded.apply_edits().unwrap();
    for uuid in objects {
        let original = engine
            .nodes
            .get(engine.node_id_by_uuid(uuid).expect("original authored object"))
            .unwrap();
        assert!(
            loaded.node_id_by_uuid(uuid).is_some(),
            "loaded {} {} missing {uuid:?}",
            original.get_type(),
            original.node_data().meta.label
        );
        remove_and_restore(&mut engine, uuid);
        remove_and_restore(&mut loaded, uuid);
    }
}
