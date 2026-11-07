use super::*;

#[test]
fn live_geometry_publishes_typed_changes_without_invalidating_every_screen_reader() {
    use golden_core::events::EventKind;
    let mut engine = fixture();
    let (_, reference) = create(&mut engine);
    engine.clear_ui_event_log();
    let result = apply(
        &mut engine,
        ScreenEdit::EditSurfaceGeometry {
            reference,
            edit: SurfaceGeometryEdit::QuadCorner {
                corner: 0,
                point: Point { x: 0.1, y: 0.2 },
            },
        },
    );
    assert!(matches!(result, ScreenEditResult::SurfaceChanged { .. }));
    let events = engine.ui_events_since(None);
    assert!(
        events
            .iter()
            .any(|event| matches!(&event.kind, EventKind::ParamChanged { .. }))
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(&event.kind, EventKind::Custom(custom) if custom.topic == SCREEN_NAMESPACE))
    );
    assert_eq!(definition(&engine, reference).quad.corners[0], Point { x: 0.1, y: 0.2 });
    engine.clear_ui_event_log();
    apply(
        &mut engine,
        ScreenEdit::RenameSurface {
            reference,
            label: "Changed source label".into(),
        },
    );
    assert!(
        engine
            .ui_events_since(None)
            .iter()
            .any(|event| matches!(&event.kind, EventKind::Custom(custom) if custom.topic == SCREEN_NAMESPACE)),
        "metadata changes must still refresh Screen rows"
    );
}

pub(super) fn session(engine: &mut AppEngine, begin: bool) {
    let intent = if begin {
        UiEditIntent::BeginEdit {
            client_edit_id: "corner-drag".into(),
            label: Some("Warp Surface".into()),
        }
    } else {
        UiEditIntent::EndEdit {
            client_edit_id: "corner-drag".into(),
        }
    };
    let ack = apply_project_intent(engine, intent);
    assert!(ack.success, "{ack:?}");
}

#[test]
fn corner_samples_preserve_other_fields_and_undo_redo_as_one_gesture() {
    let mut engine = fixture();
    let (_, reference) = create(&mut engine);
    let mut original = definition(&engine, reference);
    original.opacity = 0.4;
    original.crop = UnitRect {
        x: 0.1,
        y: 0.2,
        width: 0.7,
        height: 0.6,
    };
    original.calibration = Calibration::Grid;
    apply(
        &mut engine,
        ScreenEdit::ConfigureSurface {
            reference,
            definition: original.clone(),
        },
    );
    let history = engine.undo_len();
    session(&mut engine, true);
    for index in 1..=100 {
        let point = Point {
            x: index as f32 / 1000.0,
            y: 0.1,
        };
        let ScreenEditResult::SurfaceChanged { snapshot } = apply(
            &mut engine,
            ScreenEdit::EditSurfaceGeometry {
                reference,
                edit: SurfaceGeometryEdit::QuadCorner { corner: 0, point },
            },
        ) else {
            panic!()
        };
        let mut expected = original.clone();
        expected.quad.corners[0] = point;
        assert_eq!(snapshot.definition, expected);
        assert_eq!(definition(&engine, reference), expected);
    }
    let final_state = definition(&engine, reference);
    for (corner, point) in [
        (4, Point { x: 0.2, y: 0.2 }),
        (
            0,
            Point {
                x: f32::INFINITY,
                y: 0.0,
            },
        ),
    ] {
        let ack = invoke(
            &mut engine,
            ScreenEdit::EditSurfaceGeometry {
                reference,
                edit: SurfaceGeometryEdit::QuadCorner { corner, point },
            },
        );
        assert!(!ack.success);
        assert_eq!(definition(&engine, reference), final_state);
    }
    session(&mut engine, false);
    assert_eq!(engine.undo_len(), history + 1);
    assert!(engine.undo().unwrap());
    assert_eq!(definition(&engine, reference), original);
    assert!(engine.redo().unwrap());
    assert_eq!(definition(&engine, reference), final_state);
}

#[test]
fn unchanged_corner_has_no_history_and_ungrouped_edits_remain_independent() {
    let mut engine = fixture();
    let (_, reference) = create(&mut engine);
    let original = definition(&engine, reference);
    let history = engine.undo_len();
    session(&mut engine, true);
    apply(
        &mut engine,
        ScreenEdit::EditSurfaceGeometry {
            reference,
            edit: SurfaceGeometryEdit::QuadCorner {
                corner: 0,
                point: original.quad.corners[0],
            },
        },
    );
    session(&mut engine, false);
    assert_eq!(engine.undo_len(), history);
    for x in [0.1, 0.2] {
        apply(
            &mut engine,
            ScreenEdit::EditSurfaceGeometry {
                reference,
                edit: SurfaceGeometryEdit::QuadCorner {
                    corner: 0,
                    point: Point { x, y: 0.1 },
                },
            },
        );
    }
    assert_eq!(engine.undo_len(), history + 2);
    assert!(engine.undo().unwrap());
    assert_eq!(definition(&engine, reference).quad.corners[0].x, 0.1);
}
