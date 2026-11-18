use super::*;
use golden_core::node::{Node, NodeMetaPatch};

fn group(engine: &mut AppEngine, media: MediaId, parent: Option<GroupId>) -> GroupId {
    let MediaEditResult::MatrixCreated { items, .. } =
        edit(engine, media, MatrixEdit::AddGroup { parent, label: None })
    else {
        panic!()
    };
    let MatrixItemId::Group { id } = items[0] else { panic!() };
    id
}

fn window(
    engine: &mut AppEngine,
    media: MediaId,
    offset: f64,
    selection: Option<MatrixSelection>,
) -> MatrixViewportSnapshot {
    let MediaEditResult::MatrixViewport { snapshot } = edit(
        engine,
        media,
        MatrixEdit::InspectViewport {
            viewport: MatrixViewport {
                row_layout: Some(MatrixRowLayout {
                    offset,
                    layer_height: 7.5,
                    group_height: 2.1,
                }),
                row_start: 0,
                row_count: 8,
                axis_start: 0,
                axis_count: 4,
            },
            selection,
        },
    ) else {
        panic!()
    };
    snapshot
}

#[test]
fn compact_nested_group_rows_collapse_without_changing_render_configuration_or_hidden_inspector_selection() {
    let mut engine = fixture();
    let media = create(&mut engine, MediaKind::Matrix).media;
    let outer = group(&mut engine, media, None);
    let inner = group(&mut engine, media, Some(outer));
    let layer = layer(layers(&mut engine, media, Some(inner), 1)[0]);
    group(&mut engine, media, None);
    let selected = Some(MatrixSelection {
        item: Some(MatrixItemId::Layer { id: layer }),
        column: None,
    });
    let before = window(&mut engine, media, 0.0, selected);
    assert_eq!((before.row_total, before.group_total), (4, 3));
    assert_eq!(
        before
            .rows
            .iter()
            .map(|row| (row.visible_index, row.groups_before))
            .collect::<Vec<_>>(),
        [(Some(0), 0), (Some(1), 1), (Some(2), 2), (Some(3), 2)]
    );
    assert!(before.rows[0].has_children && before.rows[1].has_children);
    assert!(!before.rows[3].has_children);
    let configuration = serde_json::to_value(inspect(&mut engine, media)).unwrap();
    let id = engine.node_id_by_uuid(outer.0).unwrap();
    let mut presentation = engine.nodes.get(id).unwrap().node_data().meta.presentation.clone();
    presentation.collapsed = true;
    assert!(
        apply_project_intent(
            &mut engine,
            UiEditIntent::PatchMeta {
                node: id,
                patch: NodeMetaPatch {
                    presentation: Some(presentation),
                    ..Default::default()
                }
                .into()
            }
        )
        .success
    );
    let collapsed = window(&mut engine, media, 0.0, selected);
    assert_eq!((collapsed.row_total, collapsed.group_total), (2, 2));
    assert!(collapsed.rows[0].collapsed);
    assert_eq!(
        collapsed.selected.unwrap().row.unwrap().item.id,
        MatrixItemId::Layer { id: layer }
    );
    assert_eq!(
        serde_json::to_value(inspect(&mut engine, media)).unwrap(),
        configuration
    );
    assert!(engine.undo().unwrap());
    assert_eq!(window(&mut engine, media, 0.0, selected).row_total, 4);
    assert!(engine.redo().unwrap());
    assert_eq!(window(&mut engine, media, 0.0, selected).row_total, 2);
    let json = to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = from_sparse_project_json::<AppNode>(&json).unwrap();
    loaded.apply_edits().unwrap();
    assert_eq!(window(&mut loaded, media, 0.0, selected).row_total, 2);
}

#[test]
fn physical_offset_queries_find_mixed_rows_and_reject_invalid_dimensions_without_history() {
    let mut engine = fixture();
    let media = create(&mut engine, MediaKind::Matrix).media;
    for _ in 0..12 {
        group(&mut engine, media, None);
    }
    layers(&mut engine, media, None, 12);
    let before = engine.undo_len();
    let page = window(&mut engine, media, 12.0 * 2.1 + 5.0 * 7.5 + 0.1, None);
    assert_eq!(page.row_start, 15);
    assert_eq!(page.rows.len(), 8);
    assert_eq!(page.rows[0].groups_before, 12);
    assert_eq!(page.rows[0].visible_index, Some(15));
    assert_eq!(engine.undo_len(), before);
    let ack = invoke(
        &mut engine,
        MediaEdit::Matrix {
            media,
            edit: MatrixEdit::InspectViewport {
                viewport: MatrixViewport {
                    row_layout: Some(MatrixRowLayout {
                        offset: 0.0,
                        layer_height: 0.0,
                        group_height: 2.1,
                    }),
                    row_start: 0,
                    row_count: 8,
                    axis_start: 0,
                    axis_count: 4,
                },
                selection: None,
            },
        },
    );
    assert!(!ack.success);
    assert_eq!(engine.undo_len(), before);
}
