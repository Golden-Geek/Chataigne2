use super::*;
use golden_core::{
    application::apply_project_intent,
    node::{Node, NodeMetaPatch},
    ui_sync::UiEditIntent,
};

#[test]
fn composition_configuration_and_metadata_share_one_enable_state_and_history() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let owner = create(&mut engine, MediaKind::Composition);
    let instance = add(&mut engine, owner, vec![source])[0];
    let node = engine.node_id_by_uuid(instance.instance.0).unwrap();
    let original = inspect(&mut engine, owner);
    let mut definition = original.instances[0].definition.clone();
    definition.enabled = false;
    definition.opacity = 0.25;
    let history = engine.undo_len();
    edit(
        &mut engine,
        owner,
        CompositionEdit::ConfigureInstance {
            instance: instance.instance,
            definition: definition.clone(),
        },
    );
    assert!(!engine.nodes.get(node).unwrap().node_data().meta.enabled);
    assert!(!engine.nodes.get(node).unwrap().node_data().effective_enabled);
    assert_eq!(inspect(&mut engine, owner).instances[0].definition, definition);
    assert_eq!(engine.undo_len(), history + 1);
    let captured = crate::media::capture_channels(&engine, &[owner], 16).unwrap();
    assert_eq!(captured.channels.len(), 1, "disabled children must not demand sources");
    let document = to_sparse_project_json_pretty(&engine).unwrap();
    assert!(!document.contains("render_enabled"));
    let mut loaded = from_sparse_project_json::<AppNode>(&document).unwrap();
    loaded.apply_edits().unwrap();
    crate::media::validate_project(&loaded).unwrap();
    assert_eq!(inspect(&mut loaded, owner), inspect(&mut engine, owner));
    assert!(engine.undo().unwrap());
    assert_eq!(inspect(&mut engine, owner), original);
    assert!(engine.redo().unwrap());
    assert_eq!(inspect(&mut engine, owner).instances[0].definition, definition);
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::PatchMeta {
            node,
            patch: NodeMetaPatch {
                enabled: Some(true),
                ..Default::default()
            }
            .into(),
        },
    );
    assert!(ack.success, "{ack:?}");
    assert!(inspect(&mut engine, owner).instances[0].definition.enabled);
    assert!(!captured.is_current());
    assert!(engine.undo().unwrap());
    assert!(!inspect(&mut engine, owner).instances[0].definition.enabled);
    let MediaEditResult::InstancesCreated { instances } = edit(
        &mut engine,
        owner,
        CompositionEdit::DuplicateInstances {
            instances: vec![instance.instance],
        },
    ) else {
        panic!()
    };
    let copied = inspect(&mut engine, owner)
        .instances
        .into_iter()
        .find(|item| item.reference == instances[0])
        .unwrap();
    assert!(!copied.definition.enabled);
    let copied_node = engine.node_id_by_uuid(copied.reference.instance.0).unwrap();
    assert!(!engine.nodes.get(copied_node).unwrap().node_data().meta.enabled);
}

#[test]
fn disabled_reference_creation_uses_media_metadata_and_previous_parameter_schema_is_rejected() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let owner = create(&mut engine, MediaKind::Composition);
    let owner_node = engine.node_id_by_uuid(owner.media.0).unwrap();
    let (tree, instance) = super::super::children::reference_tree(
        "Disabled".into(),
        Some(crate::media::MediaSource {
            media: source.media,
            channel: Some(source.channel),
        }),
        InstanceDefinition {
            enabled: false,
            ..Default::default()
        },
    );
    engine.edits.push(golden_core::edit::Edit::AddNodeTree {
        parent: owner_node,
        prev_sibling: None,
        tree,
    });
    engine.apply_edits().unwrap();
    assert!(!inspect(&mut engine, owner).instances[0].definition.enabled);
    let id = engine.node_id_by_uuid(instance.0).unwrap();
    assert!(!engine.nodes.get(id).unwrap().node_data().meta.enabled);
    let document = to_sparse_project_json_pretty(&engine).unwrap();
    assert!(!document.contains("render_enabled"));
    let old = document.replace("mapgyver.composition.placement.v3", "mapgyver.composition.placement.v2");
    let error = match from_sparse_project_json::<AppNode>(&old) {
        Ok(_) => panic!("old duplicate-enabled schema must not be silently reinterpreted"),
        Err(error) => error,
    };
    assert!(
        error.to_string().contains("Unsupported Composition Placement schema"),
        "{error}"
    );
}
