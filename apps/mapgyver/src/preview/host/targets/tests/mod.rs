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
    let mut disabled = capture_presentation(&engine, &[], 8, 128, &[]).unwrap();
    assert!(registry.assign(&mut disabled).unwrap().is_empty());
    configure(&mut engine, screen, "MapGyver test", true);
    assert!(!disabled.is_current());
    let mut active = capture_presentation(&engine, &[], 8, 128, &[]).unwrap();
    let first = registry.assign(&mut active).unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(active.screens[0].output, Some(first[0].id));
    assert_ne!(first[0].id, OutputId::new(1).unwrap());
    assert_eq!(first[0].descriptor.color_encoding, ColorEncoding::Srgb);
    assert_eq!(first[0].descriptor.alpha_mode, AlphaMode::Straight);
    configure(&mut engine, screen, "MapGyver test", false);
    let mut disabled = capture_presentation(&engine, &[], 8, 128, &[]).unwrap();
    assert!(registry.assign(&mut disabled).unwrap().is_empty());
    configure(&mut engine, screen, "MapGyver renamed", true);
    let mut renamed = capture_presentation(&engine, &[], 8, 128, &[]).unwrap();
    let renamed_targets = registry.assign(&mut renamed).unwrap();
    assert_eq!(first[0].id, renamed_targets[0].id);
    assert_ne!(first, renamed_targets);
    let ScreenEditResult::ScreenCreated { screen: second, .. } =
        edit(&mut engine, ScreenEdit::CreateScreen { label: None })
    else {
        panic!()
    };
    configure(&mut engine, second, "Second", true);
    let mut before = capture_presentation(&engine, &[], 8, 128, &[]).unwrap();
    let before = registry.assign(&mut before).unwrap();
    edit(
        &mut engine,
        ScreenEdit::ReorderScreen {
            screen: second,
            after: None,
        },
    );
    let mut after = capture_presentation(&engine, &[], 8, 128, &[]).unwrap();
    assert_eq!(before, registry.assign(&mut after).unwrap());
    edit(&mut engine, ScreenEdit::RemoveScreen { screen });
    let mut removed = capture_presentation(&engine, &[], 8, 128, &[]).unwrap();
    let targets = registry.assign(&mut removed).unwrap();
    assert_eq!(targets.len(), 1);
    assert_ne!(targets[0].id, first[0].id);
    assert_eq!(registry.ids.len(), 1);
}

#[test]
fn rectangle_and_spout_share_output_admission_without_windows_on_the_actor() {
    let mut engine: AppEngine = create_new_project_engine().unwrap();
    engine.apply_edits().unwrap();
    let ScreenEditResult::ScreenCreated { screen, .. } = edit(&mut engine, ScreenEdit::CreateScreen { label: None })
    else {
        panic!()
    };
    let bounds = PhysicalRectangle {
        left: -640,
        top: -20,
        extent: Extent2D::new(640, 360).unwrap(),
    };
    edit(
        &mut engine,
        ScreenEdit::ConfigureScreen {
            screen,
            definition: ScreenDefinition {
                enabled: true,
                target: Some(ScreenTarget::Rectangle { bounds }),
            },
        },
    );
    let mut capture = capture_presentation(&engine, &[], 8, 128, &[]).unwrap();
    let mut registry = Targets::default();
    let targets = registry.assign(&mut capture).unwrap();
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].kind, ResolvedTarget::Rectangle(bounds));
    assert_eq!(targets[0].descriptor.alpha_mode, AlphaMode::Opaque);
    assert_eq!(capture.screens[0].output, Some(targets[0].id));
    let ScreenEditResult::ScreenCreated { screen: second, .. } =
        edit(&mut engine, ScreenEdit::CreateScreen { label: None })
    else {
        panic!()
    };
    configure(&mut engine, second, "Concurrent", true);
    let mut mixed = capture_presentation(&engine, &[], 8, 128, &[]).unwrap();
    assert!(!capture.is_current());
    let mixed = registry.assign(&mut mixed).unwrap();
    assert_eq!(mixed.len(), 2);
    assert_eq!(mixed[0], targets[0]);
    assert!(matches!(mixed[1].kind, ResolvedTarget::Spout { .. }));
    assert!(crate::screens::rendering::capture_outputs(&engine, 128, 1, &[]).is_err());
}

#[test]
fn fullscreen_resolves_exact_identity_and_recovers_without_primary_or_geometry_fallback() {
    use crate::{preview::protocol::PreviewSource, screens::protocol::DisplayInfo};
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
                calibration: Calibration::Grid,
                ..Default::default()
            },
        },
    );
    edit(
        &mut engine,
        ScreenEdit::ConfigureScreen {
            screen,
            definition: ScreenDefinition {
                enabled: true,
                target: Some(ScreenTarget::Fullscreen {
                    display: "wanted".into(),
                }),
            },
        },
    );
    let bounds = PhysicalRectangle {
        left: -1920,
        top: -80,
        extent: Extent2D::new(1920, 1080).unwrap(),
    };
    let wanted = DisplayInfo {
        id: "wanted".into(),
        name: "Same label".into(),
        bounds,
    };
    let other = DisplayInfo {
        id: "other".into(),
        ..wanted.clone()
    };
    let mut registry = Targets::default();
    let mut active = capture_presentation(
        &engine,
        &[PreviewSource::Screen { screen }],
        8,
        128,
        &[other.clone(), wanted.clone()],
    )
    .unwrap();
    assert_eq!(active.screens.len(), 1);
    assert_eq!(active.screens[0].extent, bounds.extent);
    let target = registry.assign(&mut active).unwrap();
    assert_eq!(
        target[0].kind,
        ResolvedTarget::Fullscreen {
            display: "wanted".into(),
            bounds
        }
    );
    let mut reordered = capture_presentation(&engine, &[], 8, 128, &[wanted.clone(), other.clone()]).unwrap();
    assert_eq!(target, registry.assign(&mut reordered).unwrap());
    let mut missing = capture_presentation(&engine, &[], 8, 128, &[other]).unwrap();
    assert!(registry.assign(&mut missing).unwrap().is_empty());
    assert!(
        missing.screens.is_empty(),
        "missing output must not keep source/pass demand"
    );
    assert!(
        missing.statuses(&[])[0]
            .message
            .as_ref()
            .unwrap()
            .contains("unavailable")
    );
    let moved = DisplayInfo {
        bounds: PhysicalRectangle {
            left: 3840,
            top: 40,
            extent: Extent2D::new(2560, 1440).unwrap(),
        },
        ..wanted
    };
    let mut recovered = capture_presentation(&engine, &[], 8, 128, std::slice::from_ref(&moved)).unwrap();
    let recovered = registry.assign(&mut recovered).unwrap();
    assert_eq!(recovered[0].id, target[0].id);
    assert_eq!(recovered[0].descriptor.extent, moved.bounds.extent);
    assert_eq!(
        recovered[0].kind,
        ResolvedTarget::Fullscreen {
            display: "wanted".into(),
            bounds: moved.bounds
        }
    );
    let ScreenEditResult::Manager { snapshot } = edit(&mut engine, ScreenEdit::InspectManager) else {
        panic!()
    };
    assert_eq!(
        snapshot.screens[0].definition.target,
        Some(ScreenTarget::Fullscreen {
            display: "wanted".into()
        })
    );
    let ScreenEditResult::ScreenCreated { screen: healthy, .. } =
        edit(&mut engine, ScreenEdit::CreateScreen { label: None })
    else {
        panic!()
    };
    configure(&mut engine, healthy, "Healthy output", true);
    let oversized = DisplayInfo {
        bounds: PhysicalRectangle {
            left: 0,
            top: 0,
            extent: Extent2D::new(8193, 64).unwrap(),
        },
        ..moved
    };
    let partial = capture_presentation(&engine, &[], 8, 128, &[oversized]).unwrap();
    assert_eq!(
        partial.outputs.len(),
        1,
        "unsupported fullscreen must not block the healthy Spout"
    );
    assert_eq!(partial.outputs[0].screen, healthy);
    assert!(
        partial
            .statuses(&[])
            .iter()
            .any(|status| status.message.as_ref().is_some_and(|message| message.contains("8192")))
    );
    edit(
        &mut engine,
        ScreenEdit::ConfigureScreen {
            screen,
            definition: ScreenDefinition {
                enabled: false,
                target: Some(ScreenTarget::Fullscreen {
                    display: "wanted".into(),
                }),
            },
        },
    );
    let preview_display = DisplayInfo {
        id: "wanted".into(),
        name: "Monitor".into(),
        bounds: PhysicalRectangle {
            left: 2560,
            top: 0,
            extent: Extent2D::new(2560, 1600).unwrap(),
        },
    };
    let preview = capture_presentation(
        &engine,
        &[PreviewSource::Screen { screen }],
        8,
        128,
        std::slice::from_ref(&preview_display),
    )
    .unwrap();
    let route = preview
        .screens
        .iter()
        .find(|route| route.screen.id() == screen)
        .unwrap();
    assert_eq!(
        route.extent, preview_display.bounds.extent,
        "disabled output preview still uses the intended display aspect/extent"
    );
    assert!(route.output.is_none());
}
