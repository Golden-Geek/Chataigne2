use super::drop_sources::source;
use super::*;
use crate::preview::protocol::PreviewSource;

fn read(engine: &mut AppEngine, reference: SurfaceReference) -> ScreenEditorPage {
    let ScreenEditResult::Editor { page } = apply(
        engine,
        ScreenEdit::InspectEditor {
            query: ScreenEditorQuery {
                editor: ScreenEditorKind::Surface,
                selected: Some(reference.surface.0),
                document: None,
                locked: None,
                retained: None,
                surface: None,
            },
        },
    ) else {
        panic!()
    };
    *page
}

#[test]
fn source_free_patterns_restore_the_exact_source_and_crop_without_query_history() {
    let mut engine = fixture();
    let (screen, reference) = create(&mut engine);
    let extent = golden_video::Extent2D::new(96, 64).unwrap();
    apply(
        &mut engine,
        ScreenEdit::ConfigureScreen {
            screen,
            definition: ScreenDefinition {
                target: Some(ScreenTarget::Spout {
                    sender: "Preview test".into(),
                    extent,
                }),
                ..Default::default()
            },
        },
    );
    let leaf = source(&mut engine);
    for assigned in [None, Some(leaf)] {
        let original = SurfaceDefinition {
            source: assigned,
            crop: UnitRect {
                x: 0.2,
                y: 0.1,
                width: 0.6,
                height: 0.7,
            },
            ..Default::default()
        };
        for mode in [
            Calibration::Off,
            Calibration::ColorBars,
            Calibration::Grid,
            Calibration::Off,
        ] {
            let definition = SurfaceDefinition {
                calibration: mode,
                ..original.clone()
            };
            apply(
                &mut engine,
                ScreenEdit::ConfigureSurface {
                    reference,
                    definition: definition.clone(),
                },
            );
            let history = engine.ui_history_state();
            let page = read(&mut engine, reference);
            let expected = if mode == Calibration::Off {
                assigned.map(|reference| PreviewSource::Channel { reference })
            } else {
                Some(PreviewSource::Calibration { mode, extent })
            };
            assert_eq!(page.source_preview, expected);
            assert_eq!(page.surface.unwrap().definition, definition);
            assert_eq!(engine.ui_history_state(), history);
        }
    }
}
