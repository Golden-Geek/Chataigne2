use super::*;
use golden_core::node::Node;

fn drop(
    engine: &mut AppEngine,
    media: MediaId,
    row: LayerId,
    column: ColumnId,
    source: ChannelReference,
    copy: bool,
) -> (CellId, MediaId) {
    let MediaEditResult::MatrixMediaCreated { cell, media: owned } = edit(
        engine,
        media,
        MatrixEdit::DropSource {
            layer: row,
            column,
            source: source.media.0,
            copy,
        },
    ) else {
        panic!()
    };
    (cell, owned)
}
#[test]
fn typed_cells_own_reference_or_independent_variants_with_history_and_project_parity() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    apply(
        &mut engine,
        MediaEdit::AddChannel {
            media: source.media,
            label: None,
        },
    );
    let matrix = create(&mut engine, MediaKind::Matrix);
    let row = layer(layers(&mut engine, matrix.media, None, 1)[0]);
    let cols = columns(&mut engine, matrix.media, 2);
    let history = engine.undo_len();
    let (cell, reference) = drop(&mut engine, matrix.media, row, cols[0], source, false);
    let (_, copy) = drop(&mut engine, matrix.media, row, cols[1], source, true);
    assert_eq!(engine.undo_len(), history + 2);
    let id = engine.node_id_by_uuid(cell.0).unwrap();
    let AppNode::MatrixCell(node) = engine.nodes.get(id).unwrap() else {
        panic!()
    };
    assert_eq!(node.get_type(), super::super::cell_node::CELL_TYPE);
    assert!(node.project_encode_data().unwrap().is_null());
    let content = node.content(&engine).unwrap().unwrap();
    assert_eq!(engine.nodes.get(content).unwrap().node_data().meta.uuid, reference.0);
    assert!(matches!(engine.nodes.get(content), Some(AppNode::ReferenceMedia(_))));
    let MediaEditResult::Channels { page } = apply(
        &mut engine,
        MediaEdit::InspectChannels {
            query: MediaChannelsQuery {
                media: copy,
                start: 0,
                count: 32,
            },
        },
    ) else {
        panic!()
    };
    assert_eq!(page.total, 2);
    assert_ne!(page.channels[0].reference.channel, source.channel);
    let saved = inspect(&mut engine, matrix.media);
    let json = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&json).unwrap();
    loaded.apply_edits().unwrap();
    assert_eq!(inspect(&mut loaded, matrix.media), saved);
    assert!(engine.undo().unwrap());
    assert_eq!(inspect(&mut engine, matrix.media).cells.len(), 1);
    assert!(engine.redo().unwrap());
    assert_eq!(inspect(&mut engine, matrix.media), saved);
}
#[test]
fn in_use_owned_content_replacement_removal_and_clear_reject_atomically_until_unload() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let other = create(&mut engine, MediaKind::Picture);
    let matrix = create(&mut engine, MediaKind::Matrix);
    let row = layer(layers(&mut engine, matrix.media, None, 1)[0]);
    let column = columns(&mut engine, matrix.media, 1)[0];
    let (_, media) = drop(&mut engine, matrix.media, row, column, source, true);
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::LaunchColumn {
            column,
            mode: LaunchMode::Cut,
        },
    );
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    for operation in [
        MediaEdit::Matrix {
            media: matrix.media,
            edit: MatrixEdit::DropSource {
                layer: row,
                column,
                source: other.media.0,
                copy: false,
            },
        },
        MediaEdit::Matrix {
            media: matrix.media,
            edit: MatrixEdit::SetCells {
                assignments: vec![CellAssignment {
                    layer: row,
                    column,
                    source: None,
                }],
            },
        },
        MediaEdit::Matrix {
            media: matrix.media,
            edit: MatrixEdit::RemoveColumn { column },
        },
        MediaEdit::RemoveMedia { media },
    ] {
        let ack = invoke(&mut engine, operation);
        assert!(!ack.success, "{ack:?}");
        assert!(ack.error_message.unwrap().contains("still in use"));
        assert_eq!(engine.undo_len(), history);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    }
    edit(&mut engine, matrix.media, MatrixEdit::ClearPlayback);
    drop(&mut engine, matrix.media, row, column, other, false);
}
#[test]
fn owned_reference_targets_reject_matrix_feedback_through_generic_parameters() {
    use golden_core::parameter::{ParamValue, ParameterEventBehaviour};
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let matrix = create(&mut engine, MediaKind::Matrix);
    let row = layer(layers(&mut engine, matrix.media, None, 1)[0]);
    let column = columns(&mut engine, matrix.media, 1)[0];
    let (cell, owned) = drop(&mut engine, matrix.media, row, column, source, false);
    let AppNode::ReferenceMedia(node) = engine.nodes.get(engine.node_id_by_uuid(owned.0).unwrap()).unwrap() else {
        panic!()
    };
    let target = node.target_parameter();
    let AppNode::MatrixCell(node) = engine.nodes.get(engine.node_id_by_uuid(cell.0).unwrap()).unwrap() else {
        panic!()
    };
    let output = node.output_parameter();
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    engine.edits.push(golden_core::edit::Edit::SetParamsAtomic {
        values: vec![
            (
                target,
                ParamValue::Reference(golden_core::node::NodeReference::new(matrix.media.0)),
            ),
            (
                output,
                ParamValue::Reference(golden_core::node::NodeReference::new(matrix.channel.0)),
            ),
        ],
        behaviour: ParameterEventBehaviour::Append,
    });
    assert!(engine.apply_edits().is_err());
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
}
