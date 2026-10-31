use super::*;

fn group(engine: &mut AppEngine, media: MediaId, parent: Option<GroupId>) -> (MatrixItemId, GroupId) {
    let MediaEditResult::MatrixCreated { items, .. } =
        edit(engine, media, MatrixEdit::AddGroup { parent, label: None })
    else {
        panic!()
    };
    let MatrixItemId::Group { id } = items[0] else { panic!() };
    (items[0], id)
}

#[test]
fn group_copy_remaps_internal_exports_and_keeps_sources_independent_through_reload_remove_undo() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let matrix = create(&mut engine, MediaKind::Matrix);
    let (original, group) = group(&mut engine, matrix.media, None);
    let items = layers(&mut engine, matrix.media, Some(group), 2);
    let columns = columns(&mut engine, matrix.media, 1);
    let initial = inspect(&mut engine, matrix.media);
    let b = initial.items.iter().find(|item| item.id == items[1]).unwrap().channel;
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::SetCells {
            assignments: vec![
                CellAssignment {
                    layer: layer(items[0]),
                    column: columns[0],
                    source: Some(b),
                },
                CellAssignment {
                    layer: layer(items[1]),
                    column: columns[0],
                    source: Some(source),
                },
            ],
        },
    );
    let history = engine.undo_len();
    let MediaEditResult::MatrixCreated { items: copied, .. } =
        edit(&mut engine, matrix.media, MatrixEdit::DuplicateItem { item: original })
    else {
        panic!()
    };
    assert_eq!(engine.undo_len(), history + 1);
    let MatrixItemId::Group { id: copy_group } = copied[0] else {
        panic!()
    };
    let snapshot = inspect(&mut engine, matrix.media);
    let copies = snapshot
        .items
        .iter()
        .filter(|item| item.parent == Some(copy_group))
        .collect::<Vec<_>>();
    assert_eq!(copies.len(), 2);
    assert!(copies.iter().all(|copy| {
        !initial
            .items
            .iter()
            .any(|old| old.id == copy.id || old.channel == copy.channel)
    }));
    let a_copy = snapshot
        .cells
        .iter()
        .find(|cell| cell.layer == layer(copies[0].id))
        .unwrap();
    assert_eq!(a_copy.definition.source, Some(copies[1].channel));
    assert_eq!(
        snapshot
            .cells
            .iter()
            .find(|cell| cell.layer == layer(copies[1].id))
            .unwrap()
            .definition
            .source,
        Some(source)
    );
    let encoded = to_sparse_project_json_pretty(&engine).unwrap();
    let mut restored = from_sparse_project_json::<AppNode>(&encoded).unwrap();
    assert_eq!(inspect(&mut restored, matrix.media), snapshot);
    edit(&mut engine, matrix.media, MatrixEdit::RemoveItem { item: original });
    let remaining = inspect(&mut engine, matrix.media);
    assert_eq!(remaining.items.len(), 3);
    assert_eq!(remaining.cells.len(), 2);
    assert!(engine.undo().unwrap());
    assert_eq!(inspect(&mut engine, matrix.media), snapshot);
}

#[test]
fn column_copy_clear_and_removal_preserve_other_cells_and_undo_identity() {
    let mut engine = fixture();
    let source = create(&mut engine, MediaKind::SolidColor);
    let matrix = create(&mut engine, MediaKind::Matrix);
    let items = layers(&mut engine, matrix.media, None, 2);
    let columns = columns(&mut engine, matrix.media, 1);
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::SetCells {
            assignments: items
                .iter()
                .map(|item| CellAssignment {
                    layer: layer(*item),
                    column: columns[0],
                    source: Some(source),
                })
                .collect(),
        },
    );
    let original = inspect(&mut engine, matrix.media);
    let MediaEditResult::MatrixCreated { columns: copies, .. } = edit(
        &mut engine,
        matrix.media,
        MatrixEdit::DuplicateColumn { column: columns[0] },
    ) else {
        panic!()
    };
    let copied = inspect(&mut engine, matrix.media);
    assert_eq!(copied.cells.len(), 4);
    assert!(
        copied
            .cells
            .iter()
            .filter(|cell| cell.definition.column == copies[0])
            .all(|cell| original.cells.iter().all(|old| old.id != cell.id))
    );
    let address = CellAssignment {
        layer: layer(items[0]),
        column: columns[0],
        source: None,
    };
    let id = copied
        .cells
        .iter()
        .find(|cell| cell.layer == address.layer && cell.definition.column == address.column)
        .unwrap()
        .id;
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::SetCells {
            assignments: vec![address],
        },
    );
    let cleared = inspect(&mut engine, matrix.media);
    assert_eq!(
        cleared
            .cells
            .iter()
            .find(|cell| cell.id == id)
            .unwrap()
            .definition
            .source,
        None
    );
    let encoded = to_sparse_project_json_pretty(&engine).unwrap();
    let mut restored = from_sparse_project_json::<AppNode>(&encoded).unwrap();
    assert_eq!(inspect(&mut restored, matrix.media), cleared);
    let before = engine.undo_len();
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::RemoveColumn { column: columns[0] },
    );
    let remaining = inspect(&mut engine, matrix.media);
    assert_eq!(remaining.columns.len(), 1);
    assert_eq!(remaining.cells.len(), 2);
    assert_eq!(engine.undo_len(), before + 1);
    assert!(engine.undo().unwrap());
    assert_eq!(inspect(&mut engine, matrix.media), cleared);
    apply(&mut engine, MediaEdit::RemoveMedia { media: source.media });
    assert_eq!(inspect(&mut engine, matrix.media), cleared);
    let encoded = to_sparse_project_json_pretty(&engine).unwrap();
    let mut restored = from_sparse_project_json::<AppNode>(&encoded).unwrap();
    assert_eq!(inspect(&mut restored, matrix.media), cleared);
    assert!(engine.undo().unwrap());
    assert!(crate::media::channel(&engine, source).is_ok());
}

#[test]
fn document_item_and_column_settings_rename_move_and_export_ids_survive_reload() {
    let mut engine = fixture();
    let matrix = create(&mut engine, MediaKind::Matrix);
    let (item, group) = group(&mut engine, matrix.media, None);
    let items = layers(&mut engine, matrix.media, None, 1);
    let columns = columns(&mut engine, matrix.media, 2);
    let before = inspect(&mut engine, matrix.media);
    let export = before.items.iter().find(|item| item.id == items[0]).unwrap().channel;
    let mut definition = before.definition;
    definition.canvas = golden_video::Extent2D::new(800, 600).unwrap();
    definition.transition = TransitionDefinition {
        duration_ms: 450,
        curve: TransitionCurve::SmoothStep,
    };
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::Configure {
            definition: definition.clone(),
        },
    );
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::ConfigureItem {
            item,
            settings: CompositeSettings {
                opacity: 0.25,
                blend: golden_video::plan::BlendMode::Screen,
                enabled: false,
            },
        },
    );
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::RenameItem {
            item: items[0],
            label: "Foreground".into(),
        },
    );
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::MoveItem {
            item: items[0],
            parent: Some(group),
            after: None,
        },
    );
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::RenameColumn {
            column: columns[0],
            label: "Scene A".into(),
        },
    );
    edit(
        &mut engine,
        matrix.media,
        MatrixEdit::ConfigureColumn {
            column: columns[0],
            definition: ColumnDefinition {
                transition: Some(TransitionDefinition {
                    duration_ms: 0,
                    curve: TransitionCurve::Linear,
                }),
                ..Default::default()
            },
        },
    );
    let final_state = inspect(&mut engine, matrix.media);
    assert_eq!(final_state.definition, definition);
    let layer = final_state.items.iter().find(|item| item.id == items[0]).unwrap();
    assert_eq!(layer.parent, Some(group));
    assert_eq!(layer.channel, export);
    assert_eq!(layer.label, "Foreground");
    let encoded = to_sparse_project_json_pretty(&engine).unwrap();
    let mut restored = from_sparse_project_json::<AppNode>(&encoded).unwrap();
    assert_eq!(inspect(&mut restored, matrix.media), final_state);
}
