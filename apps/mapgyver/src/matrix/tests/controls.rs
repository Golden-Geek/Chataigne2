use super::*;
use golden_core::{
    application::service_script_app_operations,
    edit::Edit,
    node::{Node, NodeId, NodeUuid},
    parameter::{ParamValue, ParameterEventBehaviour},
    script::ScriptPropertyWrite,
};

fn parameter(engine: &AppEngine, owner: NodeUuid, field: &str) -> NodeId {
    let owner = engine.node_id_by_uuid(owner).unwrap();
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
fn pulse(engine: &mut AppEngine, owner: NodeUuid, field: &str) {
    let node = parameter(engine, owner, field);
    assert_eq!(
        engine.nodes.get(node).unwrap().engine_param_snapshot().unwrap().value,
        ParamValue::Trigger()
    );
    let ack = apply_project_intent(
        engine,
        UiEditIntent::SetParam {
            node,
            value: ParamValue::Trigger(),
            behaviour: ParameterEventBehaviour::Append,
        },
    );
    assert!(ack.success, "{field}: {ack:?}");
}
fn current(engine: &AppEngine, media: MediaId) -> MatrixPlaybackSnapshot {
    let MediaEditResult::MatrixPlayback { snapshot } = super::super::playback::inspect(engine, media).unwrap() else {
        panic!()
    };
    snapshot
}
fn cell(engine: &mut AppEngine, media: MediaId, layer: LayerId, column: ColumnId) -> CellId {
    edit(
        engine,
        media,
        MatrixEdit::ConfigureCell {
            layer,
            column,
            settings: CellSettings::default(),
        },
    );
    inspect(engine, media)
        .cells
        .iter()
        .find(|cell| cell.layer == layer && cell.definition.column == column)
        .unwrap()
        .id
}

#[test]
fn typed_column_cell_and_stop_all_pulses_preserve_anchor_history_and_project() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let row = layer(layers(&mut engine, matrix.media, None, 1)[0]);
    let columns = columns(&mut engine, matrix.media, 2);
    let cell = cell(&mut engine, matrix.media, row, columns[1]);
    let authored = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    pulse(&mut engine, columns[0].0, "play");
    let anchor = current(&engine, matrix.media);
    assert_eq!(anchor.desired_column, Some(columns[0]));
    assert_eq!(anchor.transition.duration_ms, 250);
    pulse(&mut engine, cell.0, "cut");
    let independent = current(&engine, matrix.media);
    assert_eq!(independent.launch_id, anchor.launch_id);
    assert_eq!(independent.desired_column, anchor.desired_column);
    assert_eq!(independent.layers.len(), 1);
    assert!(independent.layers[0].individual);
    assert_eq!(independent.layers[0].column, Some(columns[1]));
    assert_eq!(independent.layers[0].transition.duration_ms, 0);
    pulse(&mut engine, cell.0, "play");
    assert_eq!(current(&engine, matrix.media).layers[0].transition.duration_ms, 250);
    pulse(&mut engine, matrix.media.0, "stop_all");
    let stopped = current(&engine, matrix.media);
    assert_eq!(stopped.desired_column, None);
    assert!(stopped.layers.is_empty());
    assert_eq!(stopped.transition.duration_ms, 0);
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), authored);
    let mut loaded = from_sparse_project_json::<AppNode>(&authored).unwrap();
    golden_core::app::prepare_engine_for_runtime(&mut loaded).unwrap();
    service_script_app_operations(&mut loaded).unwrap();
    assert_eq!(current(&loaded, matrix.media), MatrixPlaybackSnapshot::default());
    pulse(&mut loaded, columns[1].0, "cut");
    assert_eq!(current(&loaded, matrix.media).desired_column, Some(columns[1]));
}

#[test]
fn script_parameter_write_uses_the_same_operation_and_needs_no_prior_matrix_inspection() {
    let mut engine = fixture();
    golden_core::app::prepare_engine_for_runtime(&mut engine).unwrap();
    // Keep product defaults: no Document inspection or RuntimeBinding is required to invoke.
    let MediaEditResult::Created { reference } = apply(
        &mut engine,
        MediaEdit::CreateMedia {
            media_kind: MediaKind::Matrix,
            label: None,
        },
    ) else {
        panic!()
    };
    let column = engine
        .nodes
        .iter()
        .find_map(|(_, node)| matches!(node, AppNode::MatrixColumn(_)).then_some(node.node_data().meta.uuid))
        .unwrap();
    let node = parameter(&engine, column, "cut");
    engine.edits.push(Edit::SetNodeScriptProperty {
        node,
        write: ScriptPropertyWrite::prepare("value", ParamValue::Trigger()).unwrap(),
    });
    engine.apply_edits().unwrap();
    engine.apply_edits().unwrap();
    engine
        .dispatch_inbox(golden_core::process_ctx::ExecutionPhase::EndOfTickStabilization)
        .unwrap();
    engine.apply_edits().unwrap();
    engine
        .dispatch_inbox(golden_core::process_ctx::ExecutionPhase::EndOfTickStabilization)
        .unwrap();
    service_script_app_operations(&mut engine).unwrap();
    assert_eq!(current(&engine, reference.media).desired_column, Some(ColumnId(column)));
    assert_eq!(current(&engine, reference.media).transition.duration_ms, 0);
}

#[test]
fn queued_controls_cancel_on_owner_disable_or_removal_and_history_does_not_replay() {
    for remove in [false, true] {
        let mut engine = fixture();
        let matrix = create(&mut engine, MediaKind::Matrix);
        let column = columns(&mut engine, matrix.media, 1)[0];
        let owner = engine.node_id_by_uuid(column.0).unwrap();
        let trigger = parameter(&engine, column.0, "cut");
        engine.edits.push(Edit::SetParam {
            node: trigger,
            value: ParamValue::Trigger(),
            behaviour: ParameterEventBehaviour::Append,
        });
        engine.apply_edits().unwrap();
        engine.edits.push(if remove {
            Edit::RemoveNode { node: owner }
        } else {
            Edit::PatchMeta {
                node: owner,
                patch: golden_core::node::NodeMetaPatch {
                    enabled: Some(false),
                    ..Default::default()
                },
            }
        });
        engine.apply_edits().unwrap();
        service_script_app_operations(&mut engine).unwrap();
        assert_eq!(current(&engine, matrix.media), MatrixPlaybackSnapshot::default());
    }
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let column = columns(&mut engine, matrix.media, 1)[0];
    pulse(&mut engine, column.0, "play");
    pulse(&mut engine, matrix.media.0, "stop_all");
    let stopped = current(&engine, matrix.media);
    assert!(engine.undo().unwrap());
    assert!(engine.redo().unwrap());
    service_script_app_operations(&mut engine).unwrap();
    assert_eq!(current(&engine, matrix.media), stopped);
}

#[test]
fn stop_all_removes_independent_render_and_application_demand_while_old_capture_remains_owned() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let row = layer(layers(&mut engine, matrix.media, None, 1)[0]);
    let column = columns(&mut engine, matrix.media, 1)[0];
    let source = create(&mut engine, MediaKind::SolidColor);
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::DropSource {
            layer: row,
            column,
            source: source.media.0,
            copy: false,
        },
    );
    let MediaEditResult::MatrixCellPrepared { cell, application } = edit(
        &mut engine,
        matrix.media,
        MatrixEdit::ConfigureCellApplication {
            layer: row,
            column,
            definition: crate::applications::protocol::ApplicationDefinition {
                executable: Some("C:\\missing\\not-launched.exe".into()),
                ..Default::default()
            },
            pre: 0,
            post: 0,
        },
    ) else {
        panic!()
    };
    pulse(&mut engine, cell.0, "cut");
    let time = super::cell_playback::time();
    let old = super::cell_playback::capture(&engine, matrix.media, row, time);
    assert_eq!(old.inputs.len(), 1);
    assert!(old.applications.references.contains(&application));
    pulse(&mut engine, matrix.media.0, "stop_all");
    let new = super::cell_playback::capture(&engine, matrix.media, row, time);
    assert!(new.inputs.is_empty());
    assert!(new.applications.references.is_empty());
    assert_eq!(old.inputs.len(), 1);
    assert!(old.applications.references.contains(&application));
    assert!(
        crate::applications::logical::capture_logical_applications(&engine)
            .unwrap()
            .demand
            .is_empty()
    );
}
