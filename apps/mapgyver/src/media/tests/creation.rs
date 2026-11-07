use super::*;
use crate::media::{ChannelNode, channel};
use golden_core::{
    app::{from_sparse_project_json, to_sparse_project_json_pretty},
    node::Node,
};

#[test]
fn manager_catalog_and_api_share_defaults_unique_identity_and_atomic_history() {
    let mut engine = fixture();
    let manager = crate::media::authoring::manager(&engine).unwrap();
    for (kind, label) in [
        (MediaKind::SolidColor, "Solid Color"),
        (MediaKind::Picture, "Picture"),
        (MediaKind::SharedTexture, "Shared Texture"),
        (MediaKind::Composition, "Composition"),
        (MediaKind::Matrix, "Matrix"),
    ] {
        let item = engine
            .catalog_creatable_items(manager)
            .into_iter()
            .find(|item| item.label == label)
            .unwrap();
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
        let catalog = crate::media::catalog::inspect(&engine).unwrap();
        let generic = catalog.media.iter().find(|media| media.label == label).unwrap();
        assert_eq!(generic.media_kind, kind);
        assert_eq!(generic.channels.len(), 1);
        assert_eq!(generic.channels[0].definition, ChannelDefinition::for_media(kind));
        let reference = generic.channels[0].reference;
        let receipt = ack
            .created_node
            .as_ref()
            .expect("catalog alias must return its exact committed root");
        assert_eq!(receipt.uuid, reference.media.0);
        assert_eq!(engine.node_id_by_uuid(receipt.uuid), Some(receipt.node_id));
        let created = create(&mut engine, kind);
        assert_ne!(created.media, reference.media);
        assert_ne!(created.channel, reference.channel);
        for reference in [reference, created] {
            let node = engine
                .nodes
                .get(engine.node_id_by_uuid(reference.channel.0).unwrap())
                .unwrap();
            let permissions = &node.node_data().meta.user_permissions;
            assert_eq!(
                permissions.can_edit_name,
                kind.has_authored_channel_variants(),
                "fresh catalog/API variant permission must not depend on reload: {kind:?}"
            );
            assert_eq!(
                permissions.can_remove_and_duplicate,
                kind.has_authored_channel_variants()
            );
        }
        assert_eq!(engine.undo_len(), history + 2);
        let catalog = crate::media::catalog::inspect(&engine).unwrap();
        let api = catalog.media.iter().find(|media| media.id == created.media).unwrap();
        assert_ne!(api.label, label);
        assert_eq!(api.channels[0].definition, generic.channels[0].definition);
        assert_eq!(api.channels[0].label, generic.channels[0].label);
        assert!(engine.undo().unwrap());
        assert!(channel(&engine, created).is_err());
        assert!(channel(&engine, reference).is_ok());
        assert!(engine.redo().unwrap());
        assert_eq!(crate::media::catalog::inspect(&engine).unwrap(), catalog);

        let before = to_sparse_project_json_pretty(&engine).unwrap();
        let history = engine.undo_len();
        assert!(
            !invoke(
                &mut engine,
                MediaEdit::CreateMedia {
                    media_kind: kind,
                    label: Some("  ".into())
                }
            )
            .success
        );
        if !kind.has_authored_channel_variants() {
            let parent = engine.node_id_by_uuid(created.media.0).unwrap();
            assert!(engine.catalog_creatable_items(parent).is_empty());
            let ack = apply_project_intent(
                &mut engine,
                UiEditIntent::CreateUserItem {
                    parent,
                    node_type: "mapgyver.media.channel.v2".into(),
                    label: None,
                    initial_params: vec![],
                },
            );
            assert!(!ack.success, "derived output creation must reject: {ack:?}");
            assert!(ack.created_node.is_none());
        }
        assert_eq!(engine.undo_len(), history);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    }
    let before = crate::media::catalog::inspect(&engine).unwrap();
    let json = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<crate::app::AppNode>(&json).unwrap();
    loaded.apply_edits().unwrap();
    crate::media::validate_project(&loaded).unwrap();
    assert_eq!(crate::media::catalog::inspect(&loaded).unwrap(), before);
}

#[test]
fn old_media_schemas_are_rejected_instead_of_reinterpreting_channel_settings() {
    let mut engine = fixture();
    let reference = create(&mut engine, MediaKind::SolidColor);
    let json = to_sparse_project_json_pretty(&engine).unwrap();
    assert!(json.contains("mapgyver.media.v8"));
    for old_type in [
        "mapgyver.media",
        "mapgyver.media.v2",
        "mapgyver.media.v3",
        "mapgyver.media.v4",
        "mapgyver.media.v5",
        "mapgyver.media.v6",
        "mapgyver.media.v7",
        "mapgyver.media.v999",
    ] {
        let old = json.replace("mapgyver.media.v8", old_type);
        let error = from_sparse_project_json::<crate::app::AppNode>(&old)
            .err()
            .expect("old media schema must not load")
            .to_string();
        assert!(error.contains("mapgyver.media"), "{error}");
        assert!(error.contains("Unsupported MapGyver media schema"), "{error}");
    }
    assert!(channel(&engine, reference).is_ok());
}

#[test]
fn intrinsic_channel_baselines_are_kind_independent_and_live_catalog_defaults_survive_ordered_reload() {
    let mut engine = fixture();
    let mut references = Vec::new();
    for kind in [
        MediaKind::Picture,
        MediaKind::SolidColor,
        MediaKind::SharedTexture,
        MediaKind::Composition,
        MediaKind::Matrix,
    ] {
        let reference = create(&mut engine, kind);
        let parent = engine.node_id_by_uuid(reference.media.0).unwrap();
        let owner = engine.nodes.get(parent).unwrap();
        let baseline = owner.create_user_item("mapgyver.media.channel.v2").unwrap();
        assert_eq!(
            baseline.as_any().downcast_ref::<ChannelNode>().unwrap().kind(),
            MediaKind::SolidColor
        );
        references.push(reference);
        if !kind.has_authored_channel_variants() {
            continue;
        }
        let ack = apply_project_intent(
            &mut engine,
            UiEditIntent::CreateUserItem {
                parent,
                node_type: "mapgyver.media.channel.v2".into(),
                label: Some("Contextual channel".into()),
                initial_params: vec![],
            },
        );
        assert!(ack.success, "{ack:?}");
        let new = crate::parameters::domain_children(&engine, parent)
            .unwrap()
            .into_iter()
            .find(|id| engine.nodes.get(*id).unwrap().node_data().meta.label == "Contextual channel")
            .unwrap();
        let new_reference = ChannelReference {
            media: reference.media,
            channel: ChannelId(engine.nodes.get(new).unwrap().node_data().meta.uuid),
        };
        assert_eq!(
            channel(&engine, new_reference).unwrap().definition,
            ChannelDefinition::for_media(kind)
        );
        let ack = apply_project_intent(
            &mut engine,
            UiEditIntent::MoveNode {
                node: new,
                new_parent: parent,
                new_prev_sibling: None,
            },
        );
        assert!(ack.success, "{ack:?}");
        references.push(new_reference);
    }
    let snapshots = references
        .into_iter()
        .map(|reference| channel(&engine, reference).unwrap())
        .collect::<Vec<_>>();
    let json = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<crate::app::AppNode>(&json).unwrap();
    loaded.apply_edits().unwrap();
    crate::media::validate_project(&loaded).unwrap();
    for snapshot in snapshots {
        assert_eq!(channel(&loaded, snapshot.reference).unwrap(), snapshot);
    }
}
