use super::*;
use golden_core::{
    node::{Node, NodeId},
    parameter::{ParamValue, ParameterEventBehaviour},
};

fn parameter(engine: &AppEngine, owner: MatrixItemId, field: &str) -> NodeId {
    let uuid = match owner {
        MatrixItemId::Layer { id } => id.0,
        MatrixItemId::Group { id } => id.0,
    };
    let owner = engine.node_id_by_uuid(uuid).unwrap();
    let folder = engine
        .nodes
        .iter()
        .find(|(_, node)| node.node_data().parent == Some(owner) && crate::parameters::is_parameter_folder(*node))
        .unwrap()
        .0;
    engine
        .nodes
        .iter()
        .find(|(_, node)| {
            node.node_data().parent == Some(folder) && node.node_data().meta.decl_id.0 == format!("parameters/{field}")
        })
        .unwrap()
        .0
}
fn write(engine: &mut AppEngine, owner: MatrixItemId, field: &str, value: ParamValue) -> golden_core::ui_sync::UiAck {
    let node = parameter(engine, owner, field);
    apply_project_intent(
        engine,
        UiEditIntent::SetParam {
            node,
            value,
            behaviour: ParameterEventBehaviour::Append,
        },
    )
}
fn group(engine: &mut AppEngine, media: MediaId) -> (MatrixItemId, GroupId) {
    let MediaEditResult::MatrixCreated { items, .. } = edit(
        engine,
        media,
        MatrixEdit::AddGroup {
            parent: None,
            label: None,
        },
    ) else {
        panic!()
    };
    let MatrixItemId::Group { id } = items[0] else { panic!() };
    (items[0], id)
}

#[test]
fn layer_and_group_configure_writes_real_parameters_atomically_and_roundtrips_exactly() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let (group, parent) = group(&mut engine, matrix.media);
    let row = layers(&mut engine, matrix.media, Some(parent), 1)[0];
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    for item in [group, row] {
        let history = engine.undo_len();
        let before = to_sparse_project_json_pretty(&engine).unwrap();
        let identities = ["opacity", "blend", "render_enabled"].map(|field| {
            engine
                .nodes
                .get(parameter(&engine, item, field))
                .unwrap()
                .node_data()
                .meta
                .uuid
        });
        edit(
            &mut engine,
            matrix.media,
            MatrixEdit::ConfigureItem {
                item,
                settings: CompositeSettings {
                    opacity: 0.25,
                    blend: golden_video::plan::BlendMode::Screen,
                    enabled: false,
                },
            },
        );
        assert_eq!(engine.undo_len(), history + 1);
        let state = inspect(&mut engine, matrix.media);
        assert_eq!(
            state.items.iter().find(|row| row.id == item).unwrap().settings.opacity,
            0.25
        );
        for (field, expected) in [
            ("opacity", ParamValue::Float(0.25)),
            ("blend", ParamValue::Enum("Screen".into())),
            ("render_enabled", ParamValue::Bool(false)),
        ] {
            assert_eq!(
                engine
                    .nodes
                    .get(parameter(&engine, item, field))
                    .unwrap()
                    .engine_param_snapshot()
                    .unwrap()
                    .value,
                expected
            );
        }
        let saved = to_sparse_project_json_pretty(&engine).unwrap();
        let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
        assert_eq!(inspect(&mut loaded, matrix.media), state);
        assert_eq!(
            ["opacity", "blend", "render_enabled"].map(|field| loaded
                .nodes
                .get(parameter(&loaded, item, field))
                .unwrap()
                .node_data()
                .meta
                .uuid),
            identities
        );
        let uuid = match item {
            MatrixItemId::Layer { id } => id.0,
            MatrixItemId::Group { id } => id.0,
        };
        let node = engine.nodes.get(engine.node_id_by_uuid(uuid).unwrap()).unwrap();
        assert_eq!(node.project_encode_data().unwrap(), serde_json::Value::Null);
        assert!(engine.undo().unwrap());
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
        assert!(engine.redo().unwrap());
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    }
    assert_ne!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    crate::media::validate_project(&engine).unwrap();
}

#[test]
fn direct_parameter_changes_invalidate_render_capture_and_invalid_proposals_leave_no_partial_values() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let row = layers(&mut engine, matrix.media, None, 1)[0];
    let column = columns(&mut engine, matrix.media, 1)[0];
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::LaunchColumn {
            column,
            mode: LaunchMode::Cut,
        },
    );
    let capture = crate::media::capture_channels(&engine, &[matrix], 16).unwrap();
    assert!(capture.is_current());
    assert!(write(&mut engine, row, "opacity", ParamValue::Float(0.75)).success);
    assert!(!capture.is_current());
    assert!(write(&mut engine, row, "blend", ParamValue::Enum("Add".into())).success);
    assert!(write(&mut engine, row, "render_enabled", ParamValue::Bool(false)).success);
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    for (field, value) in [
        ("opacity", ParamValue::Float(-0.1)),
        ("opacity", ParamValue::Float(1.1)),
        ("blend", ParamValue::Enum("Unknown".into())),
    ] {
        assert!(
            !write(&mut engine, row, field, value.clone()).success,
            "{field}: {value:?}"
        );
        assert_eq!(engine.undo_len(), history);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    }
    let opacity = parameter(&engine, row, "opacity");
    let blend = parameter(&engine, row, "blend");
    engine.edits.push(golden_core::edit::Edit::SetParamsAtomic {
        values: vec![
            (opacity, ParamValue::Float(0.125)),
            (blend, ParamValue::Enum("Invalid".into())),
        ],
        behaviour: ParameterEventBehaviour::Append,
    });
    assert!(engine.apply_edits().is_err());
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    assert_eq!(engine.undo_len(), history);
}

#[test]
fn typed_nested_row_copy_remaps_parameter_identities_and_old_row_schemas_reject_before_recovery() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let (group, parent) = group(&mut engine, matrix.media);
    let row = layers(&mut engine, matrix.media, Some(parent), 1)[0];
    assert!(write(&mut engine, group, "opacity", ParamValue::Float(0.375)).success);
    assert!(write(&mut engine, row, "blend", ParamValue::Enum("Multiply".into())).success);
    let before = inspect(&mut engine, matrix.media);
    let saved_before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    let MediaEditResult::MatrixCreated { items, .. } =
        edit(&mut engine, matrix.media, MatrixEdit::DuplicateItem { item: group })
    else {
        panic!()
    };
    assert_eq!(engine.undo_len(), history + 1);
    let copy = items[0];
    let MatrixItemId::Group { id: copied_parent } = copy else {
        panic!()
    };
    let after = inspect(&mut engine, matrix.media);
    let copied_row = after
        .items
        .iter()
        .find(|item| item.parent == Some(copied_parent))
        .unwrap()
        .id;
    for (a, b) in [(group, copy), (row, copied_row)] {
        assert_eq!(
            before.items.iter().find(|item| item.id == a).unwrap().settings,
            after.items.iter().find(|item| item.id == b).unwrap().settings
        );
        for field in ["opacity", "blend", "render_enabled"] {
            assert_ne!(
                engine
                    .nodes
                    .get(parameter(&engine, a, field))
                    .unwrap()
                    .node_data()
                    .meta
                    .uuid,
                engine
                    .nodes
                    .get(parameter(&engine, b, field))
                    .unwrap()
                    .node_data()
                    .meta
                    .uuid
            );
        }
    }
    assert!(write(&mut engine, copied_row, "blend", ParamValue::Enum("Screen".into())).success);
    assert_eq!(
        inspect(&mut engine, matrix.media)
            .items
            .iter()
            .find(|item| item.id == row)
            .unwrap()
            .settings
            .blend,
        golden_video::plan::BlendMode::Multiply
    );
    assert!(engine.undo().unwrap());
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    assert_eq!(inspect(&mut loaded, matrix.media), after);
    for schema in [crate::matrix::node::LAYER_TYPE, crate::matrix::node::GROUP_TYPE] {
        assert!(from_sparse_project_json::<AppNode>(&saved.replace(schema, schema.trim_end_matches(".v2"))).is_err());
        let mut file = engine.to_project_file_with(Node::project_encode_data).unwrap();
        let mut pending = vec![&mut file.root];
        let mut changed = false;
        while let Some(record) = pending.pop() {
            if record.node_type == schema {
                record.data = Some(serde_json::json!({ "opacity": 0.1 }));
                changed = true;
                break;
            }
            pending.extend(record.children.iter_mut());
        }
        assert!(changed);
        assert!(crate::media::validate_project_file(&file).is_err());
        assert!(from_sparse_project_json::<AppNode>(&serde_json::to_string(&file).unwrap()).is_err());
    }
    assert!(engine.undo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved_before);
    assert!(engine.redo().unwrap());
    assert_eq!(inspect(&mut engine, matrix.media), after);
}
