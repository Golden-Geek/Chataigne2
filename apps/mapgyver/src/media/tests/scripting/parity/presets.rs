use super::*;
use crate::matrix::protocol::{MatrixEdit, MatrixPresetEdit};

pub(super) fn authored(engine: &AppEngine) -> Vec<(golden_core::node::NodeUuid, serde_json::Value)> {
    use golden_core::node::Node;
    let mut entries: Vec<_> = engine
        .nodes
        .iter()
        .filter_map(|(_, node)| match node {
            AppNode::MatrixPreset(node) => Some((
                node.node_data().meta.uuid,
                serde_json::json!(node.node_data().meta.label),
            )),
            AppNode::MatrixPresetEntry(node) => Some((node.node_data().meta.uuid, serde_json::json!(node.values()))),
            AppNode::MatrixColumn(node) => Some((
                node.node_data().meta.uuid,
                serde_json::json!(node.node_data().meta.enabled),
            )),
            AppNode::MatrixElement(node) => Some((
                node.node_data().meta.uuid,
                serde_json::json!(node.node_data().meta.enabled),
            )),
            _ => None,
        })
        .collect();
    entries.sort_by_key(|entry| entry.0.0);
    entries
}

#[test]
fn every_preset_operation_and_member_query_match_ui_quickjs_hosted_and_named_scripts() {
    let mut baseline = fixture();
    let matrix = create(&mut baseline, MediaKind::Matrix).media;
    let MediaEditResult::MatrixPresetCreated { preset } = apply(
        &mut baseline,
        MediaEdit::Matrix {
            media: matrix,
            edit: MatrixEdit::Preset {
                edit: MatrixPresetEdit::Capture {
                    label: Some("Show".into()),
                },
            },
        },
    ) else {
        panic!()
    };
    let column = crate::matrix::inspect(&baseline, matrix).unwrap().columns[0].id;
    let node = baseline.node_id_by_uuid(column.0).unwrap();
    assert!(
        golden_core::application::apply_project_intent(
            &mut baseline,
            UiEditIntent::PatchMeta {
                node,
                patch: golden_core::ui_sync::NodeMetaPatch {
                    enabled: Some(false),
                    ..Default::default()
                }
            }
        )
        .success
    );
    let document = to_sparse_project_json_pretty(&baseline).unwrap();
    for edit in [
        MatrixPresetEdit::Capture {
            label: Some("New".into()),
        },
        MatrixPresetEdit::Update { preset },
        MatrixPresetEdit::Recall { preset },
        MatrixPresetEdit::Rename {
            preset,
            label: "Renamed".into(),
        },
        MatrixPresetEdit::Delete { preset },
        MatrixPresetEdit::InspectMembers {
            preset,
            after: None,
            count: 2,
        },
    ] {
        for caller in CALLERS {
            let mut engine = loaded(&document);
            let value = run(
                &mut engine,
                caller,
                MediaEdit::Matrix {
                    media: matrix,
                    edit: MatrixEdit::Preset { edit: edit.clone() },
                },
            );
            assert!(value.is_some(), "{caller:?}: {edit:?}");
            assert_eq!(
                crate::matrix::inspect(&engine, matrix).unwrap().cells,
                crate::matrix::inspect(&baseline, matrix).unwrap().cells
            );
            let presets = crate::matrix::presets::summaries(&engine, matrix).unwrap();
            match &edit {
                MatrixPresetEdit::Capture { .. } => assert_eq!(presets.len(), 2),
                MatrixPresetEdit::Delete { .. } => assert!(presets.is_empty()),
                MatrixPresetEdit::Rename { label, .. } => assert_eq!(&presets[0].label, label),
                MatrixPresetEdit::Recall { .. } => assert!(
                    engine
                        .nodes
                        .get(engine.node_id_by_uuid(column.0).unwrap())
                        .unwrap()
                        .node_data()
                        .meta
                        .enabled
                ),
                _ => {}
            }
        }
    }
}
