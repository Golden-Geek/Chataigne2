use super::*;
use crate::matrix::protocol::{MatrixEdit, MatrixItemId};
use golden_core::{
    app::{from_sparse_project_json, to_sparse_project_json_pretty},
    node::Node,
    parameter::{ParamValue, ParameterEventBehaviour},
};
use std::collections::HashSet;

#[test]
fn every_matrix_and_application_duration_declares_a_clock_without_rescaling_stored_milliseconds() {
    let mut engine = fixture();
    let reference = create(&mut engine, MediaKind::Matrix);
    let MediaEditResult::Matrix { snapshot } = apply(
        &mut engine,
        MediaEdit::Matrix {
            media: reference.media,
            edit: MatrixEdit::Inspect,
        },
    ) else {
        panic!("Matrix snapshot");
    };
    let layer = snapshot
        .items
        .iter()
        .find_map(|item| match item.id {
            MatrixItemId::Layer { id } => Some(id),
            _ => None,
        })
        .unwrap();
    apply(
        &mut engine,
        MediaEdit::Matrix {
            media: reference.media,
            edit: MatrixEdit::CreateCellMedia {
                layer,
                column: snapshot.columns[0].id,
                media_kind: MediaKind::SolidColor,
            },
        },
    );
    let mut declarations = HashSet::new();
    let mut layer_parameter = None;
    for (id, node) in engine.nodes.iter() {
        let field = node.node_data().meta.decl_id.0.rsplit('/').next().unwrap();
        if !field.ends_with("_ms") {
            continue;
        }
        let parameter = node.engine_param_snapshot().unwrap();
        assert_eq!(parameter.ui_hints.widget.as_deref(), Some("time"));
        assert_eq!(parameter.ui_hints.unit.as_deref(), Some("ms"));
        let folder = engine.nodes.get(node.node_data().parent.unwrap()).unwrap();
        let owner = engine.nodes.get(folder.node_data().parent.unwrap()).unwrap();
        declarations.insert((owner.get_type().to_owned(), field.to_owned()));
        if owner.node_data().meta.uuid == layer.0 && field == "duration_ms" {
            layer_parameter = Some(id);
        }
    }
    assert_eq!(
        declarations.len(),
        8,
        "three Matrix owners and five app-control duration fields"
    );
    let parameter = layer_parameter.unwrap();
    let uuid = engine.nodes.get(parameter).unwrap().node_data().meta.uuid;
    let ack = apply_project_intent(
        &mut engine,
        UiEditIntent::SetParam {
            node: parameter,
            value: ParamValue::Float(1234.0),
            behaviour: ParameterEventBehaviour::Append,
        },
    );
    assert!(ack.success, "{ack:?}");
    assert_eq!(
        engine
            .nodes
            .get(parameter)
            .unwrap()
            .engine_param_snapshot()
            .unwrap()
            .value,
        ParamValue::Float(1234.0)
    );
    assert!(engine.undo().unwrap());
    assert_eq!(
        engine
            .nodes
            .get(parameter)
            .unwrap()
            .engine_param_snapshot()
            .unwrap()
            .value,
        ParamValue::Float(250.0)
    );
    assert!(engine.redo().unwrap());
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<crate::app::AppNode>(&saved).unwrap();
    loaded.apply_edits().unwrap();
    let parameter = loaded.node_id_by_uuid(uuid).unwrap();
    let value = loaded.nodes.get(parameter).unwrap().engine_param_snapshot().unwrap();
    assert_eq!(value.value, ParamValue::Float(1234.0));
    assert_eq!(value.ui_hints.unit.as_deref(), Some("ms"));
}
