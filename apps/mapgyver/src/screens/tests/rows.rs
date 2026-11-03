use super::*;
use golden_video::Extent2D;
fn row(engine: &mut AppEngine, node: golden_core::model::NodeUuid) -> ScreenRowSnapshot {
    let ScreenEditResult::Row { snapshot } = apply(engine, ScreenEdit::InspectRow { node }) else {
        panic!()
    };
    snapshot.expect("mounted fixture row")
}
#[test]
fn row_reads_are_constant_size_read_only_and_follow_targets_source_renames_and_missing_sources() {
    let mut engine = fixture();
    let (screen, surface) = create(&mut engine);
    let history = engine.undo_len();
    let initial = row(&mut engine, screen.0);
    assert_eq!(engine.undo_len(), history);
    assert!(initial.detail.starts_with("Spout"));
    assert_eq!(row(&mut engine, surface.surface.0).detail, "No source");
    let source = super::drop_sources::source(&mut engine);
    let mut definition = definition(&engine, surface);
    definition.source = Some(source);
    apply(
        &mut engine,
        ScreenEdit::ConfigureSurface {
            reference: surface,
            definition,
        },
    );
    let first = row(&mut engine, surface.surface.0);
    assert!(first.detail.contains("Solid Color"), "{first:?}");
    super::drop_sources::media(
        &mut engine,
        crate::media::protocol::MediaEdit::RenameMedia {
            media: source.media,
            label: "Renamed media".into(),
        },
    );
    assert!(
        row(&mut engine, surface.surface.0)
            .detail
            .starts_with("Renamed media /")
    );
    let extent = Extent2D::new(320, 240).unwrap();
    apply(
        &mut engine,
        ScreenEdit::ConfigureScreen {
            screen,
            definition: ScreenDefinition {
                enabled: false,
                target: Some(ScreenTarget::Rectangle {
                    bounds: PhysicalRectangle {
                        left: -320,
                        top: 12,
                        extent,
                    },
                }),
            },
        },
    );
    let next = row(&mut engine, screen.0);
    assert_eq!(next.detail, "Rectangle · -320, 12");
    assert_eq!(next.extent, Some(extent));
    assert!(!next.enabled);
    assert!(!invoke(&mut engine, ScreenEdit::InspectRow { node: source.media.0 }).success);
    super::drop_sources::media(
        &mut engine,
        crate::media::protocol::MediaEdit::RemoveMedia { media: source.media },
    );
    assert!(!row(&mut engine, surface.surface.0).detail.contains("Renamed media"));
    let history = engine.undo_len();
    for node in [source.media.0, golden_core::model::NodeUuid(uuid::Uuid::new_v4())] {
        assert!(matches!(
            apply(&mut engine, ScreenEdit::InspectRow { node }),
            ScreenEditResult::Row { snapshot: None }
        ));
    }
    assert_eq!(
        engine.undo_len(),
        history,
        "missing rows are read-only, including after deletion"
    );
}

#[test]
fn mounted_row_reads_survive_removal_and_undo_without_creating_history() {
    let mut engine = fixture();
    let (screen, surface) = create(&mut engine);
    let original = row(&mut engine, surface.surface.0);
    apply(&mut engine, ScreenEdit::RemoveScreen { screen });
    let history = engine.ui_history_state();
    for node in [screen.0, surface.surface.0] {
        assert!(matches!(
            apply(&mut engine, ScreenEdit::InspectRow { node }),
            ScreenEditResult::Row { snapshot: None }
        ));
    }
    assert_eq!(engine.ui_history_state(), history);
    assert!(engine.undo().unwrap());
    assert_eq!(row(&mut engine, surface.surface.0), original);
}
