use super::*;
use golden_core::node::Node;

#[test]
fn every_catalog_type_is_an_owned_child_with_atomic_history_and_project_identity() {
    let mut engine = fixture();
    let owner = create(&mut engine, MediaKind::Composition);
    let parent = engine.node_id_by_uuid(owner.media.0).unwrap();
    for choice in crate::media::creation::media_choices() {
        let before = engine.undo_len();
        let MediaEditResult::InstancesCreated { instances } = edit(
            &mut engine,
            owner,
            CompositionEdit::CreateMedia {
                media_kind: choice.media_kind,
                label: None,
                position: Point { x: 123., y: 234. },
            },
        ) else {
            panic!()
        };
        let uuid = instances[0].instance.0;
        let id = engine.node_id_by_uuid(uuid).unwrap();
        assert_eq!(engine.nodes.get(id).unwrap().node_data().parent, Some(parent));
        assert_eq!(engine.nodes.get(id).unwrap().node_data().meta.label, choice.label);
        assert_eq!(engine.undo_len(), before + 1);
        let snapshot = inspect(&mut engine, owner);
        let instance = snapshot.instances.last().unwrap();
        assert_eq!(instance.definition.transform.position, [123., 234.]);
        if choice.media_kind == MediaKind::Reference {
            let output = instance.definition.source.unwrap();
            assert_eq!(output, crate::media::resolve_source_node(&engine, uuid).unwrap());
            assert!(matches!(
                crate::media::channel(&engine, output).unwrap().definition.source,
                crate::media::protocol::ChannelSource::Reference { target: None }
            ));
        } else {
            assert_eq!(instance.definition.source.unwrap().media, MediaId(uuid));
        }
        assert!(engine.undo().unwrap());
        assert!(engine.node_id_by_uuid(uuid).is_none());
        assert!(engine.redo().unwrap());
        let saved = to_sparse_project_json_pretty(&engine).unwrap();
        let mut restored = from_sparse_project_json::<AppNode>(&saved).unwrap();
        restored.apply_edits().unwrap();
        assert_eq!(inspect(&mut restored, owner), snapshot);
    }
}

#[test]
fn invalid_creation_is_atomic_and_repeated_defaults_get_backend_unique_labels() {
    let mut engine = fixture();
    let owner = create(&mut engine, MediaKind::Composition);
    let before = engine.undo_len();
    for (label, position) in [
        (Some(" ".into()), Point { x: 0., y: 0. }),
        (None, Point { x: f32::NAN, y: 0. }),
    ] {
        assert!(
            !invoke(
                &mut engine,
                MediaEdit::Composition {
                    reference: owner,
                    edit: CompositionEdit::CreateMedia {
                        media_kind: MediaKind::SolidColor,
                        label,
                        position
                    }
                }
            )
            .success
        );
        assert_eq!(engine.undo_len(), before);
        assert!(inspect(&mut engine, owner).instances.is_empty());
    }
    for _ in 0..2 {
        edit(
            &mut engine,
            owner,
            CompositionEdit::CreateMedia {
                media_kind: MediaKind::SolidColor,
                label: None,
                position: Point { x: 0., y: 0. },
            },
        );
    }
    assert_eq!(
        inspect(&mut engine, owner)
            .instances
            .iter()
            .map(|i| i.label.as_str())
            .collect::<Vec<_>>(),
        ["Solid Color", "Solid Color 2"]
    );
}
