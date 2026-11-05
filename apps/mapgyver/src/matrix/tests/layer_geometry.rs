use super::*;
use crate::media::spatial::protocol::*;
use golden_video::{Extent2D, plan::Point};

fn origin() -> SpatialRect {
    SpatialRect {
        position: [10.25, -20.5],
        size: Extent2D::new(80, 40).unwrap(),
    }
}
fn request(layer: LayerId, mode: RectGeometryMode, point: [f32; 2]) -> MatrixEdit {
    MatrixEdit::EditLayerGeometry {
        layer,
        edit: RectGeometryEdit {
            origin: origin(),
            mode,
            start: Point { x: 20.0, y: 30.0 },
            point: Point {
                x: 20.0 + point[0],
                y: 30.0 + point[1],
            },
        },
    }
}
fn arranged() -> (AppEngine, MediaId, LayerId) {
    let mut engine = fixture();
    let media = create(&mut engine, MediaKind::Matrix).media;
    let MediaEditResult::MatrixCreated { items, .. } = edit(
        &mut engine,
        media,
        MatrixEdit::AddGroup {
            parent: None,
            label: None,
        },
    ) else {
        panic!()
    };
    let MatrixItemId::Group { id: parent } = items[0] else {
        panic!()
    };
    let row = layer(layers(&mut engine, media, Some(parent), 1)[0]);
    edit(
        &mut engine,
        media,
        MatrixEdit::ConfigureLayer {
            layer: row,
            settings: LayerSettings {
                position: origin().position,
                size: origin().size,
                duration_ms: 777,
            },
        },
    );
    (engine, media, row)
}

#[test]
fn every_layer_resize_handle_rounds_dimensions_and_preserves_the_opposite_anchor_and_unrelated_settings() {
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
        let (mut engine, media, row) = arranged();
        let before = to_sparse_project_json_pretty(&engine).unwrap();
        let history = engine.undo_len();
        let MediaEditResult::MatrixLayerGeometryChanged {
            media: returned,
            layer,
            settings,
        } = edit(
            &mut engine,
            media,
            request(row, RectGeometryMode::Resize { handle }, [5.4, 3.6]),
        )
        else {
            panic!()
        };
        assert_eq!((returned, layer), (media, row));
        assert_eq!(settings.duration_ms, 777);
        let direction = handle.position();
        let size = [settings.size.width(), settings.size.height()];
        for axis in 0..2 {
            let old = [80, 40][axis];
            if direction[axis] == 0.5 {
                assert_eq!(size[axis], old);
                assert_eq!(settings.position[axis], origin().position[axis]);
            } else {
                let opposite = (1.0 - direction[axis]) as f32;
                assert!(
                    (settings.position[axis] + opposite * size[axis] as f32
                        - origin().position[axis]
                        - opposite * old as f32)
                        .abs()
                        < 0.0001
                );
            }
        }
        assert_eq!(engine.undo_len(), history + 1);
        let saved = to_sparse_project_json_pretty(&engine).unwrap();
        let mut loaded = from_sparse_project_json::<AppNode>(&saved).unwrap();
        assert_eq!(
            inspect(&mut loaded, media)
                .items
                .iter()
                .find(|item| item.id == MatrixItemId::Layer { id: row })
                .unwrap()
                .layer,
            Some(settings)
        );
        assert!(engine.undo().unwrap());
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
        assert!(engine.redo().unwrap());
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), saved);
    }
}

#[test]
fn absolute_layer_samples_form_one_history_step_without_launching_and_invalid_or_foreign_targets_are_atomic() {
    let (mut engine, media, row) = arranged();
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    assert!(
        apply_project_intent(
            &mut engine,
            UiEditIntent::BeginEdit {
                client_edit_id: "layer-drag".into(),
                label: Some("Move Layer".into())
            }
        )
        .success
    );
    for step in 1..=100 {
        let MediaEditResult::MatrixLayerGeometryChanged { settings, .. } = edit(
            &mut engine,
            media,
            request(row, RectGeometryMode::Translate, [step as f32, -3.0]),
        ) else {
            panic!()
        };
        assert_eq!(
            settings.position,
            [origin().position[0] + step as f32, origin().position[1] - 3.0]
        );
        assert_eq!(settings.size, origin().size);
        assert_eq!(settings.duration_ms, 777);
    }
    assert!(
        apply_project_intent(
            &mut engine,
            UiEditIntent::EndEdit {
                client_edit_id: "layer-drag".into()
            }
        )
        .success
    );
    assert_eq!(engine.undo_len(), history + 1);
    assert!(engine.undo().unwrap());
    assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
    let foreign = create(&mut engine, MediaKind::Matrix).media;
    let group = inspect(&mut engine, media)
        .items
        .into_iter()
        .find_map(|item| match item.id {
            MatrixItemId::Group { id } => Some(id.0),
            _ => None,
        })
        .unwrap();
    let before = to_sparse_project_json_pretty(&engine).unwrap();
    let history = engine.undo_len();
    for (owner, target, mode, delta) in [
        (foreign, row, RectGeometryMode::Translate, [1.0, 1.0]),
        (media, LayerId(group), RectGeometryMode::Translate, [1.0, 1.0]),
        (
            media,
            row,
            RectGeometryMode::Resize {
                handle: SpatialResizeHandle::Right,
            },
            [-100.0, 0.0],
        ),
        (
            media,
            row,
            RectGeometryMode::Resize {
                handle: SpatialResizeHandle::Bottom,
            },
            [0.0, 20000.0],
        ),
        (media, row, RectGeometryMode::Translate, [f32::INFINITY, 0.0]),
    ] {
        assert!(
            !invoke(
                &mut engine,
                MediaEdit::Matrix {
                    media: owner,
                    edit: request(target, mode, delta)
                }
            )
            .success
        );
        assert_eq!(to_sparse_project_json_pretty(&engine).unwrap(), before);
        assert_eq!(engine.undo_len(), history);
    }
    let MediaEditResult::MatrixPlayback { snapshot } = edit(&mut engine, media, MatrixEdit::InspectPlayback) else {
        panic!()
    };
    assert_eq!(snapshot.desired_column, None);
}
