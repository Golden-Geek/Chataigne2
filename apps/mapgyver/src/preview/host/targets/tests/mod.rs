use super::*;
use crate::{app::AppEngine, preview::capture::capture_presentation, screens::protocol::*};
use golden_core::{app::create_new_project_engine, application::apply_project_intent, ui_sync::UiEditIntent};
use golden_video::Extent2D;

fn edit(engine: &mut AppEngine, edit: ScreenEdit) -> ScreenEditResult {
    let result = apply_project_intent(
        engine,
        UiEditIntent::InvokeAppOperation {
            namespace: SCREEN_NAMESPACE.into(),
            operation: SCREEN_OPERATION.into(),
            payload: serde_json::to_value(edit).unwrap(),
        },
    );
    assert!(result.success, "{result:?}");
    serde_json::from_value(result.result.unwrap().value).unwrap()
}

fn configure(engine: &mut AppEngine, screen: ScreenId, sender: &str, enabled: bool) {
    edit(
        engine,
        ScreenEdit::ConfigureScreen {
            screen,
            definition: ScreenDefinition {
                enabled,
                target: Some(ScreenTarget::Spout {
                    sender: sender.into(),
                    extent: Extent2D::new(64, 32).unwrap(),
                }),
            },
        },
    );
}

#[test]
fn outputs_without_editors_keep_stable_ids_through_toggle_rename_and_reorder() {
    let mut engine: AppEngine = create_new_project_engine().unwrap();
    engine.apply_edits().unwrap();
    let ScreenEditResult::ScreenCreated { screen, surface } =
        edit(&mut engine, ScreenEdit::CreateScreen { label: None })
    else {
        panic!()
    };
    edit(
        &mut engine,
        ScreenEdit::ConfigureSurface {
            reference: surface,
            definition: SurfaceDefinition {
                calibration: Calibration::ColorBars,
                ..Default::default()
            },
        },
    );
    configure(&mut engine, screen, "MapGyver test", false);
    let mut registry = Targets::default();
    let mut disabled = capture_presentation(&engine, &[], 8, 128).unwrap();
    assert!(registry.assign(&mut disabled).unwrap().is_empty());
    configure(&mut engine, screen, "MapGyver test", true);
    assert!(!disabled.is_current());
    let mut active = capture_presentation(&engine, &[], 8, 128).unwrap();
    let first = registry.assign(&mut active).unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(active.screens[0].output, Some(first[0].id));
    assert_ne!(first[0].id, OutputId::new(1).unwrap());
    assert_eq!(first[0].descriptor.color_encoding, ColorEncoding::Srgb);
    assert_eq!(first[0].descriptor.alpha_mode, AlphaMode::Straight);
    configure(&mut engine, screen, "MapGyver test", false);
    let mut disabled = capture_presentation(&engine, &[], 8, 128).unwrap();
    assert!(registry.assign(&mut disabled).unwrap().is_empty());
    configure(&mut engine, screen, "MapGyver renamed", true);
    let mut renamed = capture_presentation(&engine, &[], 8, 128).unwrap();
    let renamed_targets = registry.assign(&mut renamed).unwrap();
    assert_eq!(first[0].id, renamed_targets[0].id);
    assert_ne!(first, renamed_targets);
    let ScreenEditResult::ScreenCreated { screen: second, .. } =
        edit(&mut engine, ScreenEdit::CreateScreen { label: None })
    else {
        panic!()
    };
    configure(&mut engine, second, "Second", true);
    let mut before = capture_presentation(&engine, &[], 8, 128).unwrap();
    let before = registry.assign(&mut before).unwrap();
    edit(
        &mut engine,
        ScreenEdit::ReorderScreen {
            screen: second,
            after: None,
        },
    );
    let mut after = capture_presentation(&engine, &[], 8, 128).unwrap();
    assert_eq!(before, registry.assign(&mut after).unwrap());
    edit(&mut engine, ScreenEdit::RemoveScreen { screen });
    let mut removed = capture_presentation(&engine, &[], 8, 128).unwrap();
    let targets = registry.assign(&mut removed).unwrap();
    assert_eq!(targets.len(), 1);
    assert_ne!(targets[0].id, first[0].id);
    assert_eq!(registry.ids.len(), 1);
}
