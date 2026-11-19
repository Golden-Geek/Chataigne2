use super::*;
use golden_core::node::Node;

#[test]
fn file_drop_places_an_owned_child_alongside_existing_media_with_one_undo() {
    let mut engine = fixture();
    let owner = create(&mut engine, MediaKind::Composition);
    let existing = create(&mut engine, MediaKind::SolidColor);
    add(&mut engine, owner, vec![existing]);
    let before = inspect(&mut engine, owner);
    let history = engine.undo_len();
    let directory = tempfile::tempdir().unwrap();
    let path = |name: &str| {
        golden_assets::AbsoluteAssetPath::try_from(directory.path().join(name).to_string_lossy().into_owned()).unwrap()
    };
    let file = path("Composition still.png");
    let position = Point { x: 123.0, y: 234.0 };
    let drop_file = |path| CompositionEdit::DropMediaFile { path, position };
    assert!(
        !invoke(
            &mut engine,
            MediaEdit::Composition {
                reference: owner,
                edit: drop_file(path("unsupported.txt")),
            },
        )
        .success
    );
    assert_eq!(engine.undo_len(), history);
    assert_eq!(inspect(&mut engine, owner), before);

    let MediaEditResult::InstancesCreated { instances } = edit(&mut engine, owner, drop_file(file.clone())) else {
        panic!("file drop should create a child");
    };
    let uuid = instances[0].instance.0;
    let child = engine.node_id_by_uuid(uuid).unwrap();
    assert_eq!(
        engine.nodes.get(child).unwrap().node_data().parent,
        engine.node_id_by_uuid(owner.media.0)
    );
    let after = inspect(&mut engine, owner);
    assert_eq!(after.instances.len(), 2);
    assert_eq!(after.instances[0], before.instances[0]);
    let instance = &after.instances[1];
    assert_eq!(instance.reference, instances[0]);
    assert_eq!(instance.label, "Composition still");
    assert_eq!(instance.definition.transform.position, [position.x, position.y]);
    let source = instance.definition.source.unwrap();
    assert_eq!(source.media, MediaId(uuid));
    assert_eq!(
        crate::media::channel(&engine, source).unwrap().definition.source,
        ChannelSource::Picture {
            file: Some(AssetLocator::Absolute { path: file })
        }
    );
    assert_eq!(engine.undo_len(), history + 1);
    assert!(engine.undo().unwrap());
    assert!(engine.node_id_by_uuid(uuid).is_none());
    assert_eq!(inspect(&mut engine, owner), before);
    assert!(engine.redo().unwrap());
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let mut restored = from_sparse_project_json::<AppNode>(&saved).unwrap();
    restored.apply_edits().unwrap();
    golden_core::app::validate_engine_project_candidate(&restored).unwrap();
    assert_eq!(inspect(&mut restored, owner), after);
}

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
