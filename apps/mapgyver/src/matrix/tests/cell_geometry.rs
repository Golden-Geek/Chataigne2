use super::*;
use crate::media::spatial::protocol::{SpatialRect, SpatialResizeHandle};
use golden_video::{Extent2D, plan::Point};

fn arranged(source: Option<MediaKind>) -> (AppEngine, MediaId, LayerId, ColumnId, CellId, Option<MediaId>) {
    let mut engine = fixture();
    let media = create(&mut engine, MediaKind::Matrix).media;
    let row = layer(layers(&mut engine, media, None, 1)[0]);
    let column = columns(&mut engine, media, 1)[0];
    let source = source.map(|kind| {
        let MediaEditResult::MatrixMediaCreated { media: source, .. } = edit(
            &mut engine,
            media,
            MatrixEdit::CreateCellMedia {
                layer: row,
                column,
                media_kind: kind,
            },
        ) else {
            panic!()
        };
        source
    });
    edit(
        &mut engine,
        media,
        MatrixEdit::ConfigureCell {
            layer: row,
            column,
            settings: CellSettings {
                position: [10.25, -20.5],
                opacity: 0.25,
                enabled: false,
                fit_mode: MediaFitMode::Fill,
                ..Default::default()
            },
        },
    );
    let cell = inspect(&mut engine, media)
        .cells
        .iter()
        .find(|cell| cell.layer == row && cell.definition.column == column)
        .unwrap()
        .id;
    (engine, media, row, column, cell, source)
}
fn settings(engine: &mut AppEngine, media: MediaId, cell: CellId) -> CellSettings {
    inspect(engine, media)
        .cells
        .into_iter()
        .find(|entry| entry.id == cell)
        .unwrap()
        .definition
        .settings
}
fn translate(origin: [f32; 2], delta: [f32; 2]) -> CellGeometryEdit {
    CellGeometryEdit::Translate {
        origin,
        start: Point { x: 0.0, y: 0.0 },
        point: Point {
            x: delta[0],
            y: delta[1],
        },
    }
}
fn resize(origin: SpatialRect, handle: SpatialResizeHandle, delta: [f32; 2]) -> CellGeometryEdit {
    CellGeometryEdit::Resize {
        origin,
        handle,
        start: Point { x: 0.0, y: 0.0 },
        point: Point {
            x: delta[0],
            y: delta[1],
        },
    }
}
fn geometry(engine: &mut AppEngine, media: MediaId, cell: CellId, edit_value: CellGeometryEdit) -> CellSettings {
    let MediaEditResult::MatrixCellGeometryChanged {
        media: returned,
        cell: changed,
        settings,
    } = edit(engine, media, MatrixEdit::EditCellGeometry { cell, edit: edit_value })
    else {
        panic!()
    };
    assert_eq!((returned, changed), (media, cell));
    settings
}

#[test]
fn unavailable_and_unconfigured_cells_translate_without_freezing_natural_size_and_group_history_exactly() {
    for kind in [None, Some(MediaKind::Picture)] {
        let (mut engine, media, row, _, cell, _) = arranged(kind);
        crate::matrix::inspect_cells(
            &engine,
            MatrixCellLayoutQuery {
                media,
                layer: row,
                selected: Some(cell),
                node: None,
                start: 0,
                count: 1,
            },
        )
        .unwrap();
        let id = engine.node_id_by_uuid(cell.0).unwrap();
        let AppNode::MatrixCell(node) = engine.nodes.get(id).unwrap() else {
            panic!()
        };
        let version = node.source_version();
        let original = settings(&mut engine, media, cell);
        let before = to_sparse_project_json_pretty(&engine).unwrap();
        let history = engine.undo_len();
        assert!(
            apply_project_intent(
                &mut engine,
                UiEditIntent::BeginEdit {
                    client_edit_id: "cell-drag".into(),
                    label: Some("Move Cell".into())
                }
            )
            .success
        );
        for step in 1..=100 {
            let after = geometry(
                &mut engine,
                media,
                cell,
                translate(original.position, [step as f32, -3.0]),
            );
            assert_eq!(
                after,
                CellSettings {
                    position: [original.position[0] + step as f32, original.position[1] - 3.0],
                    ..original
                }
            );
        }
        assert!(
            apply_project_intent(
                &mut engine,
                UiEditIntent::EndEdit {
                    client_edit_id: "cell-drag".into()
                }
            )
            .success
        );
        assert_eq!(engine.undo_len(), history + 1);
        let AppNode::MatrixCell(node) = engine.nodes.get(id).unwrap() else {
            panic!()
        };
        assert_eq!(node.source_version(), version);
        let after = to_sparse_project_json_pretty(&engine).unwrap();
        assert!(engine.undo().unwrap());
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
        assert!(engine.redo().unwrap());
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), after);
        let saved = to_sparse_project_json_pretty(&engine).unwrap();
        let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
        assert_eq!(settings(&mut loaded, media, cell), settings(&mut engine, media, cell));
        let before = to_sparse_project_json_pretty(&engine).unwrap();
        let history = engine.undo_len();
        let ack = invoke(
            &mut engine,
            MediaEdit::Matrix {
                media,
                edit: MatrixEdit::EditCellGeometry {
                    cell,
                    edit: resize(
                        SpatialRect {
                            position: original.position,
                            size: Extent2D::new(8, 4).unwrap(),
                        },
                        SpatialResizeHandle::Right,
                        [2.0, 0.0],
                    ),
                },
            },
        );
        assert!(!ack.success);
        assert_eq!(engine.undo_len(), history);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    }
}

#[test]
fn cell_resize_validates_natural_source_basis_then_authors_independent_size_and_all_opposite_anchors() {
    for handle in [
        SpatialResizeHandle::TopLeft,
        SpatialResizeHandle::Top,
        SpatialResizeHandle::TopRight,
        SpatialResizeHandle::Right,
        SpatialResizeHandle::BottomRight,
        SpatialResizeHandle::Bottom,
        SpatialResizeHandle::BottomLeft,
        SpatialResizeHandle::Left,
    ] {
        let (mut engine, media, row, column, cell, source) = arranged(Some(MediaKind::SolidColor));
        let source = source.unwrap();
        apply(
            &mut engine,
            MediaEdit::ConfigureMediaResolution {
                media: source,
                size: SizePolicy::Fixed {
                    extent: Extent2D::new(8, 4).unwrap(),
                },
            },
        );
        let original = settings(&mut engine, media, cell);
        let origin = SpatialRect {
            position: original.position,
            size: Extent2D::new(8, 4).unwrap(),
        };
        let history = engine.undo_len();
        assert_eq!(
            geometry(&mut engine, media, cell, resize(origin, handle, [0.0; 2])),
            original
        );
        assert_eq!(
            engine.undo_len(),
            history,
            "clicking a resize handle without motion does not freeze natural size"
        );
        let before = to_sparse_project_json_pretty(&engine).unwrap();
        let after = geometry(&mut engine, media, cell, resize(origin, handle, [2.4, 1.4]));
        let extent = after.size.unwrap();
        let direction = handle.position();
        for axis in 0..2 {
            let old = [8, 4][axis] as f32;
            let size = [extent.width(), extent.height()][axis] as f32;
            let opposite = (1.0 - direction[axis]) as f32;
            assert!((after.position[axis] + opposite * size - origin.position[axis] - opposite * old).abs() < 0.0001);
            if direction[axis] == 0.5 {
                assert_eq!(size, old);
            }
        }
        assert_eq!(after.opacity, original.opacity);
        assert_eq!(after.fit_mode, original.fit_mode);
        assert_eq!(after.enabled, original.enabled);
        assert_eq!(
            crate::media::authoring::media(&engine, source).unwrap().1,
            MediaKind::SolidColor
        );
        assert!(engine.undo().unwrap());
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
        assert!(engine.redo().unwrap());
        edit(&mut engine, media, MatrixEdit::DuplicateColumn { column });
        assert!(
            inspect(&mut engine, media)
                .cells
                .iter()
                .filter(|cell| cell.layer == row)
                .all(|cell| cell.definition.settings == after)
        );
    }
    let (mut engine, media, _, _, cell, source) = arranged(Some(MediaKind::SolidColor));
    let source = source.unwrap();
    apply(
        &mut engine,
        MediaEdit::ConfigureMediaResolution {
            media: source,
            size: SizePolicy::Fixed {
                extent: Extent2D::new(16, 4).unwrap(),
            },
        },
    );
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    let old = SpatialRect {
        position: settings(&mut engine, media, cell).position,
        size: Extent2D::new(8, 4).unwrap(),
    };
    assert!(
        !invoke(
            &mut engine,
            MediaEdit::Matrix {
                media,
                edit: MatrixEdit::EditCellGeometry {
                    cell,
                    edit: resize(old, SpatialResizeHandle::Right, [2.0, 0.0])
                }
            }
        )
        .success
    );
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    let after = geometry(
        &mut engine,
        media,
        cell,
        resize(
            SpatialRect {
                size: Extent2D::new(16, 4).unwrap(),
                ..old
            },
            SpatialResizeHandle::Right,
            [2.0, 0.0],
        ),
    );
    assert_eq!(after.size, Some(Extent2D::new(18, 4).unwrap()));
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    assert!(
        !invoke(
            &mut engine,
            MediaEdit::Matrix {
                media,
                edit: MatrixEdit::EditCellGeometry {
                    cell,
                    edit: resize(
                        SpatialRect {
                            position: after.position,
                            size: after.size.unwrap()
                        },
                        SpatialResizeHandle::Right,
                        [-100.0, 0.0]
                    )
                }
            }
        )
        .success
    );
    assert_eq!(engine.undo_len(), history);
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
}

#[test]
fn cell_geometry_rejects_foreign_kind_and_finite_overflow_atomically() {
    let (mut engine, media, row, _, cell, _) = arranged(None);
    let foreign = create(&mut engine, MediaKind::Matrix).media;
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    let max = f32::MAX;
    for (owner, target, edit) in [
        (foreign, cell, translate([0.0; 2], [1.0; 2])),
        (media, CellId(row.0), translate([0.0; 2], [1.0; 2])),
        (
            media,
            cell,
            CellGeometryEdit::Translate {
                origin: [max; 2],
                start: Point { x: -max, y: -max },
                point: Point { x: max, y: max },
            },
        ),
    ] {
        assert!(
            !invoke(
                &mut engine,
                MediaEdit::Matrix {
                    media: owner,
                    edit: MatrixEdit::EditCellGeometry { cell: target, edit }
                }
            )
            .success
        );
        assert_eq!(engine.undo_len(), history);
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    }
}
