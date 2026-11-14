use super::protocol::*;
use crate::{
    app::{AppEngine, AppNode},
    matrix::protocol::MatrixEdit,
    media::protocol::*,
};
use golden_core::{
    app::{ProjectLifecycle, create_new_project_engine, from_sparse_project_json, to_sparse_project_json_pretty},
    application::apply_project_intent,
    node::Node,
    ui_sync::UiEditIntent,
};

fn fixture() -> AppEngine {
    let mut engine = create_new_project_engine::<AppNode>().unwrap();
    engine.apply_edits().unwrap();
    engine
}
fn apply(engine: &mut AppEngine, edit: MediaEdit) -> MediaEditResult {
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
fn create(engine: &mut AppEngine, kind: MediaKind) -> ChannelReference {
    let MediaEditResult::Created { reference } = apply(
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
fn request(selected: golden_core::node::NodeUuid) -> SpatialEditorQuery {
    SpatialEditorQuery {
        selected: Some(selected),
        document: None,
        locked: None,
        retained: None,
    }
}
fn read(engine: &mut AppEngine, query: SpatialEditorQuery) -> SpatialEditorBinding {
    let payload = serde_json::to_value(MediaEdit::InspectSpatialEditor { query }).unwrap();
    let direct = AppNode::query_app_operation(engine, MEDIA_NAMESPACE, MEDIA_OPERATION, &payload)
        .unwrap()
        .unwrap();
    let MediaEditResult::SpatialEditor { binding } = apply(engine, MediaEdit::InspectSpatialEditor { query }) else {
        panic!()
    };
    assert_eq!(
        direct,
        serde_json::to_value(MediaEditResult::SpatialEditor {
            binding: binding.clone()
        })
        .unwrap()
    );
    binding
}

#[test]
fn relevant_descendants_switch_domains_unrelated_selection_retains_and_locks_are_independent() {
    let mut engine = fixture();
    let composition = create(&mut engine, MediaKind::Composition);
    let matrix = create(&mut engine, MediaKind::Matrix);
    let source = create(&mut engine, MediaKind::SolidColor);
    let MediaEditResult::Matrix { snapshot } = apply(
        &mut engine,
        MediaEdit::Matrix {
            media: matrix.media,
            edit: MatrixEdit::Inspect,
        },
    ) else {
        panic!()
    };
    let mut query = request(composition.media.0);
    let initial = read(&mut engine, query);
    assert_eq!(
        initial.target,
        Some(SpatialEditorTarget::Composition { reference: composition })
    );
    query.document = Some(initial.document);
    query.retained = initial.target;
    query.selected = Some(source.channel.0);
    assert_eq!(read(&mut engine, query).target, initial.target);
    let layer = snapshot.items[0].id;
    let uuid = match layer {
        crate::matrix::protocol::MatrixItemId::Layer { id } => id.0,
        _ => panic!(),
    };
    let id = engine.node_id_by_uuid(uuid).unwrap();
    let child = engine.nodes.get(id).unwrap().node_data().first_child.unwrap();
    let parameter = engine.nodes.get(child).unwrap().node_data().meta.uuid;
    for selected in [matrix.media.0, matrix.channel.0, uuid, parameter] {
        query.selected = Some(selected);
        let binding = read(&mut engine, query);
        assert_eq!(
            binding.target,
            Some(SpatialEditorTarget::Matrix { media: matrix.media })
        );
        assert!(binding.present);
    }
    query.locked = initial.target;
    assert_eq!(read(&mut engine, query).target, initial.target);
    query.locked = Some(SpatialEditorTarget::Matrix { media: matrix.media });
    query.selected = Some(composition.channel.0);
    assert_eq!(read(&mut engine, query).target, query.locked);
}

#[test]
fn locked_missing_bindings_recover_with_undo_load_and_rename_without_crossing_documents() {
    for kind in [MediaKind::Composition, MediaKind::Matrix] {
        let mut engine = fixture();
        let reference = create(&mut engine, kind);
        let initial = read(&mut engine, request(reference.media.0));
        let mut query = SpatialEditorQuery {
            document: Some(initial.document),
            locked: initial.target,
            retained: initial.target,
            ..request(reference.media.0)
        };
        apply(
            &mut engine,
            MediaEdit::RenameMedia {
                media: reference.media,
                label: "Bound renamed".into(),
            },
        );
        assert_eq!(read(&mut engine, query).label.as_deref(), Some("Bound renamed"));
        apply(&mut engine, MediaEdit::RemoveMedia { media: reference.media });
        let missing = read(&mut engine, query);
        assert_eq!(missing.target, initial.target);
        assert!(!missing.present);
        assert!(engine.undo().unwrap());
        assert!(read(&mut engine, query).present);
        let before = to_sparse_project_json_pretty(&engine).unwrap();
        let history = engine.ui_history_state();
        for _ in 0..10 {
            assert!(read(&mut engine, query).present);
        }
        assert_eq!(engine.ui_history_state(), history);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
        let mut loaded = from_sparse_project_json::<AppNode>(&before).unwrap();
        loaded.apply_edits().unwrap();
        assert!(read(&mut loaded, query).present);
        let mut other = fixture();
        let new = create(&mut other, kind);
        query.selected = Some(new.media.0);
        assert!(!read(&mut other, query).document_matches);
        query.locked = None;
        assert!(read(&mut other, query).present);
    }
}

#[test]
fn nested_edited_owner_selection_retains_its_domain_without_locking_or_authoring_changes() {
    use crate::media::composition::protocol::CompositionEdit;
    for kind in [MediaKind::Matrix, MediaKind::Composition] {
        let mut engine = fixture();
        let outer = create(&mut engine, MediaKind::Composition);
        let MediaEditResult::InstancesCreated { instances } = apply(
            &mut engine,
            MediaEdit::Composition {
                reference: outer,
                edit: CompositionEdit::CreateMedia {
                    media_kind: kind,
                    label: None,
                    position: golden_video::plan::Point { x: 0.0, y: 0.0 },
                },
            },
        ) else {
            panic!()
        };
        let media = MediaId(instances[0].instance.0);
        let MediaEditResult::Channels { page } = apply(
            &mut engine,
            MediaEdit::InspectChannels {
                query: MediaChannelsQuery {
                    media,
                    start: 0,
                    count: 1,
                },
            },
        ) else {
            panic!()
        };
        let initial = read(&mut engine, request(page.channels[0].reference.channel.0));
        let mut query = SpatialEditorQuery {
            document: Some(initial.document),
            retained: initial.target,
            ..request(media.0)
        };
        let history = engine.ui_history_state();
        let shown = read(&mut engine, query);
        assert_eq!(shown.target, initial.target);
        assert!(shown.present && shown.document_matches);
        query.selected = Some(outer.media.0);
        assert_eq!(
            read(&mut engine, query).target,
            Some(SpatialEditorTarget::Composition { reference: outer })
        );
        assert_eq!(engine.ui_history_state(), history);
    }
}

#[test]
fn composition_instances_keep_their_owner_but_their_channels_can_enter_nested_spatial_content() {
    use crate::media::composition::protocol::CompositionEdit;
    let mut engine = fixture();
    let outer = create(&mut engine, MediaKind::Composition);
    let MediaEditResult::InstancesCreated { instances } = apply(
        &mut engine,
        MediaEdit::Composition {
            reference: outer,
            edit: CompositionEdit::CreateMedia {
                media_kind: MediaKind::Matrix,
                label: None,
                position: golden_video::plan::Point { x: 0.0, y: 0.0 },
            },
        },
    ) else {
        panic!()
    };
    let instance = instances[0];
    assert_eq!(
        read(&mut engine, request(instance.instance.0)).target,
        Some(SpatialEditorTarget::Composition { reference: outer })
    );
    let MediaEditResult::Matrix { snapshot } = apply(
        &mut engine,
        MediaEdit::Matrix {
            media: MediaId(instance.instance.0),
            edit: MatrixEdit::Inspect,
        },
    ) else {
        panic!()
    };
    assert_eq!(
        read(&mut engine, request(snapshot.final_channel.channel.0)).target,
        Some(SpatialEditorTarget::Matrix { media: snapshot.media })
    );
}

#[test]
fn unscoped_and_wrong_kind_explicit_targets_reject_without_authored_changes() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let mut query = request(source.media.0);
    query.locked = Some(SpatialEditorTarget::Matrix { media: source.media });
    assert!(super::query::inspect(&engine, query).is_err());
    query.document = Some(engine.nodes.get(engine.root).unwrap().node_data().meta.uuid);
    assert!(super::query::inspect(&engine, query).is_err());
    query.locked = Some(SpatialEditorTarget::Composition { reference: source });
    assert!(super::query::inspect(&engine, query).is_err());
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
}
