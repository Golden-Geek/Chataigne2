use super::screens::{screen, screen_edit};
use super::*;
use crate::{
    preview::{capture::capture_presentation, protocol::PreviewSource},
    screens::protocol::*,
};

#[test]
fn fullscreen_capture_and_plan_withdrawal_work_without_a_native_host() {
    let mut engine = engine();
    let channel = create(&mut engine, MediaKind::SolidColor);
    let (screen, _) = screen(&mut engine, channel);
    screen_edit(
        &mut engine,
        ScreenEdit::ConfigureScreen {
            screen,
            definition: ScreenDefinition {
                enabled: true,
                target: Some(ScreenTarget::Fullscreen {
                    display: "intended".into(),
                }),
            },
        },
    );
    let displays = [DisplayInfo {
        id: "intended".into(),
        name: "Monitor".into(),
        bounds: PhysicalRectangle {
            left: -320,
            top: 20,
            extent: Extent2D::new(320, 180).unwrap(),
        },
    }];
    let mut renderer = renderer();
    for (catalog, expected) in [(&displays[..], 1), (&[][..], 0), (&displays[..], 1)] {
        let mut capture = capture_presentation(&engine, &[], 8, 128, catalog).unwrap();
        assert_eq!(capture.outputs.len(), expected);
        if let Some(route) = capture.screens.first_mut() {
            route.output = Some(OutputId::new(2).unwrap());
        }
        let request = renderer
            .publish_screen_targets(capture.channels, vec![], capture.screens, None)
            .unwrap();
        installed(&renderer, request);
        assert_eq!(
            renderer.snapshot().resources.unwrap().active.unwrap().output_count,
            expected
        );
        assert_eq!(renderer.channels().len(), expected);
    }
    close(renderer);
}

fn configure(engine: &mut AppEngine, screen: ScreenId, name: &str, enabled: bool) {
    screen_edit(
        engine,
        ScreenEdit::ConfigureScreen {
            screen,
            definition: ScreenDefinition {
                enabled,
                target: Some(ScreenTarget::Spout {
                    sender: name.into(),
                    extent: Extent2D::new(320, 180).unwrap(),
                }),
            },
        },
    );
}

#[test]
fn authored_output_demand_survives_closed_editor_and_calibration_restores_shared_source() {
    let mut engine = engine();
    let channel = create(&mut engine, MediaKind::SolidColor);
    let (screen, surface) = screen(&mut engine, channel);
    configure(&mut engine, screen, "Output", true);
    let preview = [
        PreviewSource::Screen { screen },
        PreviewSource::Channel { reference: channel },
    ];
    let with_editor = capture_presentation(&engine, &preview, 8, 128, &[]).unwrap();
    assert_eq!(with_editor.outputs.len(), 1);
    assert_eq!(with_editor.screens.len(), 1);
    let mut closed = capture_presentation(&engine, &[], 8, 128, &[]).unwrap();
    assert_eq!(closed.outputs, with_editor.outputs);
    assert_eq!(closed.channels.channels.len(), 1);
    closed.screens[0].output = Some(OutputId::new(2).unwrap());
    let mut renderer = renderer();
    let request = renderer
        .publish_screen_targets(closed.channels, vec![], closed.screens, None)
        .unwrap();
    installed(&renderer, request);
    assert_eq!(renderer.channels().len(), 1);
    screen_edit(
        &mut engine,
        ScreenEdit::ConfigureSurface {
            reference: surface,
            definition: SurfaceDefinition {
                source: Some(channel),
                calibration: Calibration::Grid,
                ..Default::default()
            },
        },
    );
    let calibration = capture_presentation(&engine, &[], 8, 128, &[]).unwrap();
    assert_eq!(calibration.outputs.len(), 1);
    assert!(calibration.channels.channels.is_empty());
    assert!(calibration.screens[0].screen.has_content());
    configure(&mut engine, screen, "Output", false);
    assert!(!calibration.is_current());
    let disabled = capture_presentation(&engine, &[], 8, 128, &[]).unwrap();
    assert!(disabled.outputs.is_empty());
    assert!(disabled.screens.is_empty());
    assert!(disabled.channels.channels.is_empty());
    close(renderer);
}

#[test]
fn output_catalog_rejects_duplicate_names_and_enforces_bounds_before_native_creation() {
    let mut engine = engine();
    let channel = create(&mut engine, MediaKind::SolidColor);
    let (first, _) = screen(&mut engine, channel);
    let (second, _) = screen(&mut engine, channel);
    configure(&mut engine, first, "Same", true);
    let result = apply_project_intent(
        &mut engine,
        UiEditIntent::InvokeAppOperation {
            namespace: SCREEN_NAMESPACE.into(),
            operation: SCREEN_OPERATION.into(),
            payload: serde_json::to_value(ScreenEdit::ConfigureScreen {
                screen: second,
                definition: ScreenDefinition {
                    enabled: true,
                    target: Some(ScreenTarget::Spout {
                        sender: "Same".into(),
                        extent: Extent2D::new(320, 180).unwrap(),
                    }),
                },
            })
            .unwrap(),
        },
    );
    assert!(!result.success);
    assert_eq!(
        capture_presentation(&engine, &[], 8, 128, &[]).unwrap().outputs.len(),
        1
    );
    configure(&mut engine, second, "Different", true);
    assert!(crate::screens::rendering::capture_outputs(&engine, 1, 7, &[]).is_err());
    assert!(crate::screens::rendering::capture_outputs(&engine, 128, 1, &[]).is_err());
    let capture = capture_presentation(&engine, &[], 8, 128, &[]).unwrap();
    assert_eq!(capture.outputs.len(), 2);
    assert_eq!(capture.channels.channels.len(), 1);
}
