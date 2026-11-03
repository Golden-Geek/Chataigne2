use super::*;
use crate::applications::protocol::{ApplicationDefinition, ApplicationId};

#[test]
fn thousand_columns_bind_typed_controls_without_per_control_lifecycle_snapshots() {
    let mut engine = fixture();
    let MediaEditResult::Created { reference } = apply(
        &mut engine,
        MediaEdit::CreateMedia {
            media_kind: MediaKind::Matrix,
            label: None,
        },
    ) else {
        panic!()
    };
    let initial = inspect(&mut engine, reference.media);
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    let snapshots = engine.tick_stats().lifecycle_snapshot_builds;
    let started = std::time::Instant::now();
    let created = columns(&mut engine, reference.media, 997);
    let elapsed = started.elapsed();
    let snapshots = engine.tick_stats().lifecycle_snapshot_builds - snapshots;
    assert!(
        snapshots <= 8,
        "bulk creation must share lifecycle snapshots instead of cloning the document per control: {snapshots}"
    );
    let authored = inspect(&mut engine, reference.media);
    assert_eq!(authored.columns.len(), 1000);
    assert_eq!(authored.items, initial.items);
    assert!(authored.cells.is_empty());
    assert_eq!(engine.undo_len(), history + 1);
    for column in &authored.columns {
        let control = crate::applications::inspect(&engine, ApplicationId(column.id.0)).unwrap();
        assert_eq!(control.definition, ApplicationDefinition::default());
    }
    assert_eq!(authored.columns[3].id, created[0]);
    assert_eq!(authored.columns.last().unwrap().id, *created.last().unwrap());
    let saved = to_sparse_project_json_pretty(&engine).unwrap();
    assert!(engine.undo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    assert!(engine.redo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
    assert_eq!(inspect(&mut loaded, reference.media), authored);
    for column in [authored.columns[0].id, *created.last().unwrap()] {
        assert_eq!(
            crate::applications::inspect(&loaded, ApplicationId(column.0))
                .unwrap()
                .definition,
            ApplicationDefinition::default()
        );
    }
    eprintln!(
        "Matrix bulk creation columns=997 total=1000 lifecycle_snapshots={snapshots} elapsed_ms={}",
        elapsed.as_millis()
    );
}
