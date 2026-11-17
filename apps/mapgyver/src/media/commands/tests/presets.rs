use super::*;
use crate::matrix::protocol::{MatrixEdit, MatrixPresetEdit};

pub(super) fn prepare(engine: &mut AppEngine, command: NodeUuid, kind: MediaCommandKind, media: MediaId) {
    if !matches!(
        kind,
        MediaCommandKind::UpdateMatrixPreset
            | MediaCommandKind::RecallMatrixPreset
            | MediaCommandKind::RenameMatrixPreset
            | MediaCommandKind::DeleteMatrixPreset
    ) {
        return;
    }
    let MediaEditResult::MatrixPresetCreated { preset } = apply(
        engine,
        MediaEdit::Matrix {
            media,
            edit: MatrixEdit::Preset {
                edit: MatrixPresetEdit::Capture {
                    label: Some("Before".into()),
                },
            },
        },
    ) else {
        panic!()
    };
    set(engine, command, "matrix_preset", reference(preset));
    if matches!(
        kind,
        MediaCommandKind::UpdateMatrixPreset | MediaCommandKind::RecallMatrixPreset
    ) {
        let snapshot = crate::matrix::inspect(engine, media).unwrap();
        let node = engine.node_id_by_uuid(snapshot.columns[0].id.0).unwrap();
        let ack = apply_project_intent(
            engine,
            UiEditIntent::PatchMeta {
                node,
                patch: golden_core::ui_sync::NodeMetaPatch {
                    enabled: Some(false),
                    ..Default::default()
                },
            },
        );
        assert!(ack.success);
    }
}

#[test]
fn preset_commands_reject_missing_and_foreign_members_without_mutation() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix).media;
    let other = create(&mut engine, MediaKind::Matrix).media;
    for kind in [
        MediaCommandKind::UpdateMatrixPreset,
        MediaCommandKind::RecallMatrixPreset,
        MediaCommandKind::RenameMatrixPreset,
        MediaCommandKind::DeleteMatrixPreset,
    ] {
        let command = command(&mut engine, kind);
        set(&mut engine, command, "media", reference(matrix.0));
        let before = to_sparse_project_json_pretty(&engine).unwrap();
        assert!(!invoke(&mut engine, MediaEdit::ExecuteCommand { command }).success);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
        prepare(&mut engine, command, kind, other);
        let before = to_sparse_project_json_pretty(&engine).unwrap();
        assert!(!invoke(&mut engine, MediaEdit::ExecuteCommand { command }).success);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    }
}
