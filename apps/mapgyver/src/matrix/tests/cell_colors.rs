use super::*;
use golden_core::{color::Color, node::Node};

#[test]
fn sparse_cell_color_is_atomic_and_reset_preserves_authored_cell() {
    let mut engine = fixture();
    let media = create(&mut engine, MediaKind::Matrix).media;
    let layer = layer(layers(&mut engine, media, None, 1)[0]);
    let column = columns(&mut engine, media, 1)[0];
    let color = Color {
        red: 0.8,
        green: 0.2,
        blue: 0.4,
        alpha: 1.0,
    };
    let history = engine.undo_len();
    let result = edit(
        &mut engine,
        media,
        MatrixEdit::SetCellColor {
            layer,
            column,
            color: None,
        },
    );
    assert!(matches!(result, MediaEditResult::MatrixCellColorSet { cell: None }));
    assert_eq!(engine.undo_len(), history);
    let result = edit(
        &mut engine,
        media,
        MatrixEdit::SetCellColor {
            layer,
            column,
            color: Some(color),
        },
    );
    let MediaEditResult::MatrixCellColorSet { cell: Some(cell) } = result else {
        panic!()
    };
    assert_eq!(engine.undo_len(), history + 1);
    let id = engine.node_id_by_uuid(cell.0).unwrap();
    assert_eq!(
        engine.nodes.get(id).unwrap().node_data().meta.presentation.color,
        Some(color)
    );
    assert!(engine.undo().unwrap());
    assert!(engine.node_id_by_uuid(cell.0).is_none());
    assert!(engine.redo().unwrap());
    let id = engine.node_id_by_uuid(cell.0).unwrap();
    edit(
        &mut engine,
        media,
        MatrixEdit::SetCellColor {
            layer,
            column,
            color: None,
        },
    );
    assert_eq!(engine.nodes.get(id).unwrap().node_data().meta.presentation.color, None);
    assert!(engine.undo().unwrap());
    assert_eq!(
        engine.nodes.get(id).unwrap().node_data().meta.presentation.color,
        Some(color)
    );
}

#[test]
fn cell_color_rejects_foreign_addresses_and_invalid_values_without_creating_nodes() {
    let mut engine = fixture();
    let media = create(&mut engine, MediaKind::Matrix).media;
    let other = create(&mut engine, MediaKind::Matrix).media;
    let layer = layer(layers(&mut engine, media, None, 1)[0]);
    let column = columns(&mut engine, media, 1)[0];
    let foreign = columns(&mut engine, other, 1)[0];
    let color = Color {
        red: 0.8,
        green: 0.2,
        blue: 0.4,
        alpha: 1.0,
    };
    let history = engine.undo_len();
    for (column, color) in [(foreign, color), (column, Color { red: 1.5, ..color })] {
        let ack = invoke(
            &mut engine,
            MediaEdit::Matrix {
                media,
                edit: MatrixEdit::SetCellColor {
                    layer,
                    column,
                    color: Some(color),
                },
            },
        );
        assert!(!ack.success, "{ack:?}");
        assert_eq!(engine.undo_len(), history);
    }
}
