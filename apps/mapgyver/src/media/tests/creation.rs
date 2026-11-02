use super::*;
use crate::media::{ChannelNode, channel};
use golden_core::{
    app::{from_sparse_project_json, to_sparse_project_json_pretty},
    node::Node,
};

#[test]
fn old_media_schemas_are_rejected_instead_of_reinterpreting_channel_settings() {
    let mut engine = fixture();
    let reference = create(&mut engine, MediaKind::SolidColor);
    let json = to_sparse_project_json_pretty(&engine).unwrap();
    assert!(json.contains("mapgyver.media.v5"));
    for old_type in [
        "mapgyver.media",
        "mapgyver.media.v2",
        "mapgyver.media.v3",
        "mapgyver.media.v4",
        "mapgyver.media.v999",
    ] {
        let old = json.replace("mapgyver.media.v5", old_type);
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
        let baseline = owner.create_user_item("mapgyver.media.channel").unwrap();
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
                node_type: "mapgyver.media.channel".into(),
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
