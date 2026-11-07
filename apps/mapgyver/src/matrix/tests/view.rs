use super::*;
use std::sync::Arc;

#[test]
fn preview_sharing_preserves_reference_resolution_and_enablement() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let matrix = create(&mut engine, MediaKind::Matrix);
    let row = layer(layers(&mut engine, matrix.media, None, 1)[0]);
    let cols = columns(&mut engine, matrix.media, 2);
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::SetCells {
            assignments: cols
                .iter()
                .map(|column| CellAssignment {
                    layer: row,
                    column: *column,
                    source: Some(source),
                })
                .collect(),
        },
    );
    let initial = viewport(&mut engine, matrix.media, 0, 0);
    let alias = initial.cells[0].cell.definition.source.unwrap();
    assert!(initial.cells.iter().all(|cell| cell.source_preview == Some(source)));
    apply(
        &mut engine,
        MediaEdit::ConfigureMediaResolution {
            media: alias.media,
            size: SizePolicy::Fixed {
                extent: golden_video::Extent2D::new(32, 16).unwrap(),
            },
        },
    );
    let fixed = viewport(&mut engine, matrix.media, 0, 0);
    assert_eq!(fixed.cells[0].source_preview, Some(alias));
    assert_eq!(fixed.cells[1].source_preview, Some(source));
    let mut loaded = from_sparse_project_json::<AppNode>(&to_sparse_project_json_pretty(&engine).unwrap()).unwrap();
    loaded.apply_edits().unwrap();
    assert_eq!(
        viewport(&mut loaded, matrix.media, 0, 0).cells[0].source_preview,
        Some(alias)
    );
    assert!(engine.undo().unwrap());
    assert!(
        viewport(&mut engine, matrix.media, 0, 0)
            .cells
            .iter()
            .all(|cell| cell.source_preview == Some(source))
    );
    let owner = engine.node_id_by_uuid(alias.media.0).unwrap();
    assert!(
        apply_project_intent(
            &mut engine,
            UiEditIntent::PatchMeta {
                node: owner,
                patch: golden_core::node::NodeMetaPatch {
                    enabled: Some(false),
                    ..Default::default()
                }
                .into(),
            }
        )
        .success
    );
    let disabled = viewport(&mut engine, matrix.media, 0, 0);
    assert_eq!(disabled.cells[0].source_preview, Some(alias));
    assert_eq!(disabled.cells[1].source_preview, Some(source));
    assert!(engine.undo().unwrap());
    assert!(
        viewport(&mut engine, matrix.media, 0, 0)
            .cells
            .iter()
            .all(|cell| cell.source_preview == Some(source))
    );
}

#[test]
fn deleted_viewport_reports_typed_absence_without_history_and_recovers_on_undo() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix);
    layers(&mut engine, matrix.media, None, 1);
    let original = viewport(&mut engine, matrix.media, 0, 0);
    let owner = engine.node_id_by_uuid(matrix.media.0).unwrap();
    assert!(apply_project_intent(&mut engine, UiEditIntent::RemoveNode { node: owner }).success);
    let history = engine.undo_len();
    let result = edit(
        &mut engine,
        matrix.media,
        MatrixEdit::InspectViewport {
            viewport: MatrixViewport {
                row_start: 0,
                row_count: 16,
                axis_start: 0,
                axis_count: 8,
            },
            selection: None,
        },
    );
    assert!(matches!(result, MediaEditResult::MatrixMissing { media } if media == matrix.media));
    assert_eq!(engine.undo_len(), history);
    assert!(engine.undo().unwrap());
    let recovered = viewport(&mut engine, matrix.media, 0, 0);
    assert_eq!(recovered.rows[0].item.id, original.rows[0].item.id);
    assert_eq!(recovered.final_output.reference, original.final_output.reference);
}

fn viewport(engine: &mut AppEngine, media: MediaId, row: u32, column: u32) -> MatrixViewportSnapshot {
    let MediaEditResult::MatrixViewport { snapshot } = edit(
        engine,
        media,
        MatrixEdit::InspectViewport {
            selection: None,
            viewport: MatrixViewport {
                row_start: row,
                row_count: 16,
                axis_start: column,
                axis_count: 8,
            },
        },
    ) else {
        panic!()
    };
    snapshot
}

#[test]
fn cell_labels_and_previews_follow_effective_sources_without_duplicating_reference_identities() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    apply(
        &mut engine,
        MediaEdit::RenameMedia {
            media: source.media,
            label: "Red".into(),
        },
    );
    apply(
        &mut engine,
        MediaEdit::RenameChannel {
            reference: source,
            label: "Main".into(),
        },
    );
    let matrix = create(&mut engine, MediaKind::Matrix);
    let row = layer(layers(&mut engine, matrix.media, None, 1)[0]);
    let cols = columns(&mut engine, matrix.media, 3);
    for (index, column) in cols.iter().enumerate() {
        edit(
            &mut engine,
            matrix.media,
            MatrixEdit::DropSource {
                layer: row,
                column: *column,
                source: source.media.0,
                copy: index == 2,
            },
        );
    }
    let initial = viewport(&mut engine, matrix.media, 0, 0);
    let at = |snapshot: &MatrixViewportSnapshot, column| {
        snapshot
            .cells
            .iter()
            .find(|cell| cell.cell.definition.column == column)
            .unwrap()
            .clone()
    };
    let first = at(&initial, cols[0]);
    let second = at(&initial, cols[1]);
    let copy = at(&initial, cols[2]);
    assert_eq!(first.source_label.as_deref(), Some("Red [Main]"));
    assert_eq!(first.source_preview, Some(source));
    assert_eq!(second.source_preview, Some(source));
    assert_ne!(first.cell.definition.source, second.cell.definition.source);
    let copied_source = copy.source_preview.unwrap();
    assert_ne!(copied_source.media, source.media);
    assert_eq!(copy.source_label.as_deref(), Some("Red [Main]"));
    apply(
        &mut engine,
        MediaEdit::RenameMedia {
            media: source.media,
            label: "Live".into(),
        },
    );
    apply(
        &mut engine,
        MediaEdit::RenameChannel {
            reference: source,
            label: "Fill".into(),
        },
    );
    let history = engine.undo_len();
    let renamed = viewport(&mut engine, matrix.media, 0, 0);
    assert_eq!(at(&renamed, cols[0]).source_label.as_deref(), Some("Live [Fill]"));
    assert_eq!(at(&renamed, cols[1]).source_preview, Some(source));
    assert_eq!(at(&renamed, cols[2]).source_label.as_deref(), Some("Red [Main]"));
    assert_eq!(engine.undo_len(), history);
    let mut loaded = from_sparse_project_json::<AppNode>(&to_sparse_project_json_pretty(&engine).unwrap()).unwrap();
    loaded.apply_edits().unwrap();
    let restored = viewport(&mut loaded, matrix.media, 0, 0);
    assert_eq!(at(&restored, cols[0]).source_preview, Some(source));
    assert_eq!(at(&restored, cols[2]).source_preview, Some(copied_source));
    apply(&mut engine, MediaEdit::RemoveMedia { media: source.media });
    let missing = viewport(&mut engine, matrix.media, 0, 0);
    assert!(at(&missing, cols[0]).source_missing);
    assert!(at(&missing, cols[0]).source_preview.is_none());
    assert!(at(&missing, cols[1]).source_label.is_none());
    assert_eq!(at(&missing, cols[2]).source_preview, Some(copied_source));
    assert!(engine.undo().unwrap());
    assert_eq!(
        at(&viewport(&mut engine, matrix.media, 0, 0), cols[0]).source_preview,
        Some(source)
    );
}

#[test]
fn viewport_reads_are_bounded_sparse_cached_and_do_not_launch_or_change_authored_history() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let matrix = create(&mut engine, MediaKind::Matrix);
    let items = layers(&mut engine, matrix.media, None, 1000);
    let columns = columns(&mut engine, matrix.media, 1000);
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::SetCells {
            assignments: vec![CellAssignment {
                layer: layer(items[999]),
                column: columns[999],
                source: Some(source),
            }],
        },
    );
    let history = engine.undo_len();
    let authored = to_sparse_project_json_pretty(&engine).unwrap();
    let first = viewport(&mut engine, matrix.media, 0, 0);
    assert_eq!((first.row_total, first.column_total), (1000, 1000));
    assert_eq!((first.rows.len(), first.columns.len(), first.cells.len()), (16, 8, 0));
    let root = engine.node_id_by_uuid(matrix.channel.0).unwrap();
    let AppNode::MediaChannel(node) = engine.nodes.get(root).unwrap() else {
        panic!()
    };
    let cached = node.matrix_playback().unwrap().view().unwrap();
    let last = viewport(&mut engine, matrix.media, 990, 996);
    assert_eq!((last.rows.len(), last.columns.len(), last.cells.len()), (10, 4, 1));
    assert_ne!(last.cells[0].cell.definition.source.unwrap().media, source.media);
    let output = last.cells[0].cell.definition.source.unwrap();
    assert_eq!(
        output,
        crate::media::resolve_source_node(&engine, output.media.0).unwrap()
    );
    assert!(!last.cells[0].source_missing);
    assert_eq!(last.cells[0].source_preview, Some(source));
    assert_eq!(last.playback.desired_column, None);
    let AppNode::MediaChannel(node) = engine.nodes.get(root).unwrap() else {
        panic!()
    };
    assert!(Arc::ptr_eq(&cached, &node.matrix_playback().unwrap().view().unwrap()));
    assert!(serde_json::to_vec(&last).unwrap().len() < 20_000);
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), authored);
    assert!(viewport(&mut engine, matrix.media, u32::MAX, u32::MAX).rows.is_empty());
    let MediaEditResult::Choices { page } = apply(
        &mut engine,
        MediaEdit::InspectChoices {
            query: ChannelChoiceQuery {
                scope: ChannelChoiceScope::MatrixFinal,
                search: String::new(),
                start: 0,
                count: 64,
            },
        },
    ) else {
        panic!()
    };
    assert_eq!(page.total, 1);
    assert_eq!(page.choices[0].reference, matrix);
}

#[test]
fn viewport_refreshes_structure_payload_labels_history_and_missing_source_without_persisting_indices() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let matrix = create(&mut engine, MediaKind::Matrix);
    let items = layers(&mut engine, matrix.media, None, 2);
    let columns = columns(&mut engine, matrix.media, 2);
    viewport(&mut engine, matrix.media, 0, 0);
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::RenameItem {
            item: items[0],
            label: "Renamed".into(),
        },
    );
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::SetCells {
            assignments: vec![CellAssignment {
                layer: layer(items[0]),
                column: columns[0],
                source: Some(source),
            }],
        },
    );
    let current = viewport(&mut engine, matrix.media, 0, 0);
    assert_eq!(current.rows[0].item.label, "Renamed");
    assert_ne!(current.cells[0].cell.definition.source.unwrap().media, source.media);
    let output = current.cells[0].cell.definition.source.unwrap();
    assert_eq!(
        output,
        crate::media::resolve_source_node(&engine, output.media.0).unwrap()
    );
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::MoveItem {
            item: items[0],
            parent: None,
            after: Some(items[1]),
        },
    );
    assert_eq!(viewport(&mut engine, matrix.media, 0, 0).rows[1].item.id, items[0]);
    assert!(engine.undo().unwrap());
    assert_eq!(viewport(&mut engine, matrix.media, 0, 0).rows[0].item.id, items[0]);
    apply(&mut engine, MediaEdit::RemoveMedia { media: source.media });
    let missing = viewport(&mut engine, matrix.media, 0, 0);
    assert!(missing.cells[0].source_missing);
    assert!(missing.cells[0].source_label.is_none());
    assert!(missing.cells[0].source_preview.is_none());
    assert!(engine.undo().unwrap());
    assert!(!viewport(&mut engine, matrix.media, 0, 0).cells[0].source_missing);
    let mut restored = from_sparse_project_json::<AppNode>(&to_sparse_project_json_pretty(&engine).unwrap()).unwrap();
    assert_eq!(
        viewport(&mut restored, matrix.media, 0, 0).cells[0].cell.id,
        current.cells[0].cell.id
    );
}

#[test]
fn selected_inspector_is_bounded_outside_the_viewport_and_tracks_history_and_source_loss() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let matrix = create(&mut engine, MediaKind::Matrix);
    let items = layers(&mut engine, matrix.media, None, 128);
    let columns = columns(&mut engine, matrix.media, 128);
    let selection = MatrixSelection {
        item: Some(items[127]),
        column: Some(columns[127]),
    };
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::SetCells {
            assignments: vec![CellAssignment {
                layer: layer(items[127]),
                column: columns[127],
                source: Some(source),
            }],
        },
    );
    let read = |engine: &mut AppEngine, selection| {
        let MediaEditResult::MatrixViewport { snapshot } = edit(
            engine,
            matrix.media,
            MatrixEdit::InspectViewport {
                viewport: MatrixViewport {
                    row_start: 0,
                    row_count: 1,
                    axis_start: 0,
                    axis_count: 1,
                },
                selection: Some(selection),
            },
        ) else {
            panic!()
        };
        assert_eq!(
            (snapshot.rows.len(), snapshot.columns.len(), snapshot.cells.len()),
            (1, 1, 0)
        );
        assert!(snapshot.playback.desired_column.is_none());
        snapshot.selected.unwrap()
    };
    let history = engine.undo_len();
    let authored = to_sparse_project_json_pretty(&engine).unwrap();
    let selected = read(&mut engine, selection);
    assert_eq!(selected.selection, selection);
    assert_eq!(selected.row_index, Some(127));
    assert_eq!(selected.column_index, Some(127));
    assert_eq!(selected.row.as_ref().unwrap().item.id, items[127]);
    assert_eq!(selected.column.as_ref().unwrap().id, columns[127]);
    assert_ne!(
        selected.cell.as_ref().unwrap().cell.definition.source.unwrap().media,
        source.media
    );
    assert_eq!(
        selected.cell.as_ref().unwrap().cell.definition.source.unwrap().channel,
        crate::media::resolve_source_node(
            &engine,
            selected.cell.as_ref().unwrap().cell.definition.source.unwrap().media.0
        )
        .unwrap()
        .channel
    );
    assert!(!selected.cell.as_ref().unwrap().source_missing);
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), authored);

    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::RenameItem {
            item: items[127],
            label: "Offscreen".into(),
        },
    );
    assert_eq!(read(&mut engine, selection).row.unwrap().item.label, "Offscreen");
    assert!(engine.undo().unwrap());
    assert_eq!(
        read(&mut engine, selection).row.unwrap().item.label,
        selected.row.unwrap().item.label
    );
    assert!(engine.redo().unwrap());
    assert_eq!(read(&mut engine, selection).row.unwrap().item.label, "Offscreen");
    apply(&mut engine, MediaEdit::RemoveMedia { media: source.media });
    assert!(read(&mut engine, selection).cell.unwrap().source_missing);
    assert!(engine.undo().unwrap());
    assert!(!read(&mut engine, selection).cell.unwrap().source_missing);
    edit(&mut engine, matrix.media, MatrixEdit::RemoveItem { item: items[127] });
    let missing = read(&mut engine, selection);
    assert!(missing.row.is_none() && missing.cell.is_none());
    assert_eq!(missing.column.unwrap().id, columns[127]);
    assert!(engine.undo().unwrap());
    assert_eq!(read(&mut engine, selection).row.unwrap().item.id, items[127]);
    let missing = read(
        &mut engine,
        MatrixSelection {
            item: Some(MatrixItemId::Layer {
                id: LayerId(matrix.media.0),
            }),
            column: Some(ColumnId(matrix.media.0)),
        },
    );
    assert!(missing.row.is_none() && missing.column.is_none() && missing.cell.is_none());
}

#[test]
fn oversized_viewports_and_choice_queries_reject_without_changes() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let history = engine.undo_len();
    for (rows, cols) in [(0, 1), (1, 0), (129, 1), (1, 65), (128, 64)] {
        assert!(
            !invoke(
                &mut engine,
                MediaEdit::Matrix {
                    media: matrix.media,
                    edit: MatrixEdit::InspectViewport {
                        selection: None,
                        viewport: MatrixViewport {
                            row_start: 0,
                            row_count: rows,
                            axis_start: 0,
                            axis_count: cols
                        }
                    }
                }
            )
            .success
        );
    }
    for count in [0, 129] {
        assert!(
            !invoke(
                &mut engine,
                MediaEdit::InspectChoices {
                    query: ChannelChoiceQuery {
                        scope: ChannelChoiceScope::All,
                        search: String::new(),
                        start: 0,
                        count
                    }
                }
            )
            .success
        );
    }
    assert_eq!(engine.undo_len(), history);
}

fn targets(
    engine: &mut AppEngine,
    media: MediaId,
    scope: MatrixTargetScope,
    search: &str,
    start: u32,
    count: u16,
) -> MatrixTargetPage {
    let MediaEditResult::MatrixTargets { page } = edit(
        engine,
        media,
        MatrixEdit::InspectTargets {
            query: MatrixTargetQuery {
                scope,
                search: search.into(),
                start,
                count,
            },
        },
    ) else {
        panic!()
    };
    page
}

#[test]
fn target_queries_page_all_destinations_and_follow_grouping_rename_history_and_roundtrip() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let items = layers(&mut engine, matrix.media, None, 200);
    let columns = columns(&mut engine, matrix.media, 200);
    let MediaEditResult::MatrixCreated { items: groups, .. } = edit(
        &mut engine,
        matrix.media,
        MatrixEdit::AddGroup {
            parent: None,
            label: Some("Target group".into()),
        },
    ) else {
        panic!()
    };
    let MatrixItemId::Group { id: group } = groups[0] else {
        panic!()
    };
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::AddGroup {
            parent: Some(group),
            label: Some("Nested".into()),
        },
    );
    let before = engine.undo_len();
    let far = targets(
        &mut engine,
        matrix.media,
        MatrixTargetScope::Items { parent: None },
        "",
        190,
        8,
    );
    assert_eq!((far.total, far.choices.len()), (201, 8));
    assert_eq!(far.choices[0].id, MatrixTargetId::Item { item: items[190] });
    let all_groups = targets(&mut engine, matrix.media, MatrixTargetScope::Groups, "TARGET", 0, 64);
    assert_eq!(all_groups.total, 2);
    assert_eq!(all_groups.choices[1].label, "Target group / Nested");
    assert_eq!(engine.undo_len(), before);
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::MoveItem {
            item: items[199],
            parent: Some(group),
            after: None,
        },
    );
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::RenameItem {
            item: items[199],
            label: "Moved row".into(),
        },
    );
    let moved = targets(
        &mut engine,
        matrix.media,
        MatrixTargetScope::Items { parent: Some(group) },
        "moved",
        0,
        64,
    );
    assert_eq!(moved.choices[0].id, MatrixTargetId::Item { item: items[199] });
    assert_eq!(moved.choices[0].label, "Moved row");
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::ReorderColumn {
            column: columns[199],
            after: None,
        },
    );
    assert_eq!(
        targets(&mut engine, matrix.media, MatrixTargetScope::Columns, "", 0, 1).choices[0].id,
        MatrixTargetId::Column { column: columns[199] }
    );
    assert!(engine.undo().unwrap());
    assert_eq!(
        targets(&mut engine, matrix.media, MatrixTargetScope::Columns, "", 0, 1).choices[0].id,
        MatrixTargetId::Column { column: columns[0] }
    );
    let mut restored = from_sparse_project_json::<AppNode>(&to_sparse_project_json_pretty(&engine).unwrap()).unwrap();
    assert_eq!(
        targets(
            &mut restored,
            matrix.media,
            MatrixTargetScope::Items { parent: Some(group) },
            "",
            0,
            64
        )
        .choices[0]
            .id,
        MatrixTargetId::Item { item: items[199] }
    );
    let before = engine.undo_len();
    for (count, search, scope) in [
        (0, "".into(), MatrixTargetScope::Columns),
        (129, "".into(), MatrixTargetScope::Groups),
        (1, "x".repeat(257), MatrixTargetScope::Groups),
        (
            1,
            "".into(),
            MatrixTargetScope::Items {
                parent: Some(GroupId(golden_core::model::NodeUuid(uuid::Uuid::new_v4()))),
            },
        ),
    ] {
        assert!(
            !invoke(
                &mut engine,
                MediaEdit::Matrix {
                    media: matrix.media,
                    edit: MatrixEdit::InspectTargets {
                        query: MatrixTargetQuery {
                            scope,
                            search,
                            start: 0,
                            count
                        }
                    }
                }
            )
            .success
        );
    }
    assert_eq!(engine.undo_len(), before);
}
