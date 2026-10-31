use super::protocol::*;
use crate::{
    app::{AppEngine, AppNode},
    media::protocol::*,
};
use golden_core::{
    app::{create_new_project_engine, from_sparse_project_json, to_sparse_project_json_pretty},
    application::apply_project_intent,
    ui_sync::UiEditIntent,
};

mod copying;
mod playback;
mod timeline;
mod validation;

fn fixture() -> AppEngine {
    let mut engine = create_new_project_engine::<AppNode>().unwrap();
    engine.apply_edits().unwrap();
    engine
}
fn invoke(engine: &mut AppEngine, edit: MediaEdit) -> golden_core::ui_sync::UiAck {
    apply_project_intent(
        engine,
        UiEditIntent::InvokeAppOperation {
            namespace: MEDIA_NAMESPACE.into(),
            operation: MEDIA_OPERATION.into(),
            payload: serde_json::to_value(edit).unwrap(),
        },
    )
}
fn apply(engine: &mut AppEngine, edit: MediaEdit) -> MediaEditResult {
    let ack = invoke(engine, edit);
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
fn edit(engine: &mut AppEngine, media: MediaId, edit: MatrixEdit) -> MediaEditResult {
    apply(engine, MediaEdit::Matrix { media, edit })
}
fn inspect(engine: &mut AppEngine, media: MediaId) -> MatrixSnapshot {
    let MediaEditResult::Matrix { snapshot } = edit(engine, media, MatrixEdit::Inspect) else {
        panic!()
    };
    snapshot
}
fn layers(engine: &mut AppEngine, media: MediaId, parent: Option<GroupId>, count: u16) -> Vec<MatrixItemId> {
    let MediaEditResult::MatrixCreated { items, .. } = edit(engine, media, MatrixEdit::AddLayers { parent, count })
    else {
        panic!()
    };
    items
}
fn columns(engine: &mut AppEngine, media: MediaId, count: u16) -> Vec<ColumnId> {
    let MediaEditResult::MatrixCreated { columns, .. } = edit(engine, media, MatrixEdit::AddColumns { count }) else {
        panic!()
    };
    columns
}
fn layer(item: MatrixItemId) -> LayerId {
    let MatrixItemId::Layer { id } = item else { panic!() };
    id
}

#[test]
fn sparse_authoring_exports_identity_roundtrip_and_undo() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let matrix = create(&mut engine, MediaKind::Matrix);
    let before = engine.undo_len();
    let items = layers(&mut engine, matrix.media, None, 3);
    assert_eq!(engine.undo_len(), before + 1);
    let columns = columns(&mut engine, matrix.media, 1000);
    let initial = inspect(&mut engine, matrix.media);
    assert_eq!(initial.items.len(), 3);
    assert_eq!(initial.columns.len(), 1000);
    assert!(initial.cells.is_empty());
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::SetCells {
            assignments: vec![CellAssignment {
                layer: layer(items[0]),
                column: columns[999],
                source: Some(source),
            }],
        },
    );
    let authored = inspect(&mut engine, matrix.media);
    assert_eq!(authored.cells.len(), 1);
    let encoded = to_sparse_project_json_pretty(&engine).unwrap();
    let mut restored = from_sparse_project_json::<AppNode>(&encoded).unwrap();
    assert_eq!(inspect(&mut restored, matrix.media).cells.len(), 1);
    assert_eq!(inspect(&mut restored, matrix.media), authored);
    assert!(engine.undo().unwrap());
    assert_eq!(inspect(&mut engine, matrix.media), initial);
    assert!(engine.redo().unwrap());
    assert_eq!(inspect(&mut engine, matrix.media), authored);
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::ReorderColumn {
            column: columns[999],
            after: None,
        },
    );
    let reordered = inspect(&mut engine, matrix.media);
    assert_eq!(reordered.columns[0].id, columns[999]);
    assert_eq!(reordered.cells, authored.cells);
    assert_eq!(reordered.items, authored.items);
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::MoveItem {
            item: items[0],
            parent: None,
            after: Some(items[2]),
        },
    );
    let moved = inspect(&mut engine, matrix.media);
    assert_eq!(
        moved.items.iter().map(|item| item.id).collect::<Vec<_>>(),
        vec![items[1], items[2], items[0]]
    );
    for item in &moved.items {
        assert_eq!(
            item.channel,
            authored.items.iter().find(|old| old.id == item.id).unwrap().channel
        );
    }
}

#[test]
fn source_assignment_rejects_direct_and_composition_matrix_feedback_atomically() {
    use crate::media::composition::protocol::CompositionEdit;
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let composition = create(&mut engine, MediaKind::Composition);
    let items = layers(&mut engine, matrix.media, None, 1);
    let columns = columns(&mut engine, matrix.media, 1);
    apply(
        &mut engine,
        MediaEdit::Composition {
            reference: composition,
            edit: CompositionEdit::AddInstances { sources: vec![matrix] },
        },
    );
    let history = engine.undo_len();
    for source in [matrix, composition] {
        let ack = invoke(
            &mut engine,
            MediaEdit::Matrix {
                media: matrix.media,
                edit: MatrixEdit::SetCells {
                    assignments: vec![CellAssignment {
                        layer: layer(items[0]),
                        column: columns[0],
                        source: Some(source),
                    }],
                },
            },
        );
        assert!(!ack.success);
        assert!(inspect(&mut engine, matrix.media).cells.is_empty());
        assert_eq!(engine.undo_len(), history);
    }
}

#[test]
fn batch_layer_column_creation_is_sparse_and_one_history_step_each() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let history = engine.undo_len();
    let started = std::time::Instant::now();
    layers(&mut engine, matrix.media, None, 1000);
    columns(&mut engine, matrix.media, 1000);
    eprintln!(
        "Matrix 1000 layers/1000 columns, no eager cells: {:?}",
        started.elapsed()
    );
    assert_eq!(engine.undo_len(), history + 2);
    let snapshot = inspect(&mut engine, matrix.media);
    assert_eq!(snapshot.items.len(), 1000);
    assert_eq!(snapshot.columns.len(), 1000);
    assert!(snapshot.cells.is_empty());
    let MediaEditResult::Manager { snapshot } = apply(&mut engine, MediaEdit::InspectManager) else {
        panic!()
    };
    assert_eq!(snapshot.media[0].channels.len(), 1001);
}
