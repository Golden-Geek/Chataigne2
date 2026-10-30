use super::*;
use crate::screens::{protocol::*, rendering::capture_screen};
use golden_video::plan::{Point, RenderOperation, UnitRect};

fn screen_edit(engine: &mut AppEngine, edit: ScreenEdit) -> ScreenEditResult {
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

fn screen(engine: &mut AppEngine, channel: ChannelReference) -> (ScreenId, SurfaceReference) {
    let ScreenEditResult::ScreenCreated { screen, surface } =
        screen_edit(engine, ScreenEdit::CreateScreen { label: None })
    else {
        panic!()
    };
    screen_edit(
        engine,
        ScreenEdit::ConfigureSurface {
            reference: surface,
            definition: SurfaceDefinition {
                source: Some(channel),
                crop: UnitRect {
                    x: 0.25,
                    y: 0.25,
                    width: 0.5,
                    height: 0.5,
                },
                opacity: 0.5,
                quad: Quad {
                    corners: [
                        Point { x: 0.25, y: 0.125 },
                        Point { x: 1.0, y: 0.125 },
                        Point {
                            x: 2.0 / 3.0,
                            y: 2.0 / 3.0,
                        },
                        Point {
                            x: 1.0 / 6.0,
                            y: 2.0 / 3.0,
                        },
                    ],
                },
                ..Default::default()
            },
        },
    );
    (screen, surface)
}

fn screen_routes(engine: &AppEngine, screen: ScreenId) -> Vec<ScreenRoute> {
    let capture = capture_screen(engine, screen, 8).unwrap();
    [1, 2]
        .into_iter()
        .map(|id| ScreenRoute {
            output: Some(OutputId::new(id).unwrap()),
            extent: Extent2D::new(320, 180).unwrap(),
            screen: capture.clone(),
        })
        .collect()
}

#[test]
fn composed_screen_consumers_share_one_channel_and_one_mapped_pass() {
    let mut engine = engine();
    let reference = create(&mut engine, MediaKind::SolidColor);
    let (screen, _) = screen(&mut engine, reference);
    let routes = screen_routes(&engine, screen);
    assert_eq!(routes[0].screen.references(), vec![reference]);
    let capture = capture_channels(&engine, &routes[0].screen.references(), 16).unwrap();
    let mut renderer = renderer();
    let prepared = super::super::plan::build(
        &capture,
        super::super::plan::Targets {
            routes: &[],
            preview: None,
            screens: &routes,
        },
        super::super::plan::Previous {
            channels: &HashMap::new(),
            screens: &HashMap::new(),
        },
        &mut 3,
        config().plan_limits,
        &renderer.source_observations(),
        None,
    )
    .unwrap();
    assert_eq!(prepared.submission.plan.passes.len(), 2);
    assert_eq!(
        prepared.submission.plan.outputs[0].source,
        prepared.submission.plan.outputs[1].source
    );
    let RenderOperation::Composite { layers } = &prepared.submission.plan.passes[1].operation else {
        panic!()
    };
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].source, prepared.channels[0].target);
    assert_eq!(layers[0].opacity.get(), 0.5);
    let coordinate = layers[0]
        .mapping
        .unwrap()
        .source_coordinate(Point { x: 0.5, y: 0.45 })
        .unwrap();
    assert!(coordinate.into_iter().all(|component| (component - 0.5).abs() < 1e-5));
    let request = renderer
        .publish_screen_targets(capture, Vec::new(), routes, None)
        .unwrap();
    installed(&renderer, request);
    assert_eq!(renderer.channels().len(), 1);
    assert_eq!(renderer.snapshot().resources.unwrap().active.unwrap().output_count, 2);
    close(renderer);
}

#[test]
fn mapping_edits_invalidate_capture_without_reallocating_screen_identity_and_delete_undo_preserves_routes() {
    let mut engine = engine();
    let source = create(&mut engine, MediaKind::SolidColor);
    let (screen, surface) = screen(&mut engine, source);
    let old_routes = screen_routes(&engine, screen);
    let mut renderer = renderer();
    let request = renderer
        .publish_screen_targets(
            capture_channels(&engine, &[source], 16).unwrap(),
            Vec::new(),
            old_routes.clone(),
            None,
        )
        .unwrap();
    installed(&renderer, request);
    let target = renderer.screen_entries[&screen];
    screen_edit(
        &mut engine,
        ScreenEdit::ConfigureSurface {
            reference: surface,
            definition: SurfaceDefinition {
                source: Some(source),
                opacity: 0.25,
                ..Default::default()
            },
        },
    );
    assert!(!old_routes[0].screen.is_current());
    assert!(
        renderer
            .publish_screen_targets(
                capture_channels(&engine, &[source], 16).unwrap(),
                Vec::new(),
                old_routes,
                None
            )
            .is_err()
    );
    assert_eq!(renderer.latest_request(), Some(request));
    for missing in [false, true, false] {
        if missing {
            edit(&mut engine, MediaEdit::RemoveMedia { media: source.media });
        } else if crate::media::channel(&engine, source).is_err() {
            assert!(engine.undo().unwrap());
        }
        let routes = screen_routes(&engine, screen);
        let request = renderer
            .publish_screen_targets(
                capture_channels(&engine, &routes[0].screen.references(), 16).unwrap(),
                Vec::new(),
                routes,
                None,
            )
            .unwrap();
        installed(&renderer, request);
        assert_eq!(renderer.screen_entries[&screen], target);
        assert_eq!(renderer.channels().len(), 1);
        assert_eq!(
            renderer.channels()[0].resolution.availability == crate::media::resolution::ChannelAvailability::Missing,
            missing
        );
    }
    close(renderer);
}

#[test]
fn mixed_project_screen_routes_are_rejected_before_replacing_the_live_renderer_project() {
    let mut engine = engine();
    let source = create(&mut engine, MediaKind::SolidColor);
    let (screen, _) = screen(&mut engine, source);
    let routes = screen_routes(&engine, screen);
    let mut renderer = renderer();
    let request = renderer
        .publish_screen_targets(
            capture_channels(&engine, &[source], 16).unwrap(),
            Vec::new(),
            routes.clone(),
            None,
        )
        .unwrap();
    installed(&renderer, request);
    let project = renderer.snapshot().project;
    let json = golden_core::app::to_sparse_project_json_pretty(&engine).unwrap();
    let mut loaded = golden_core::app::from_sparse_project_json::<crate::app::AppNode>(&json).unwrap();
    loaded.apply_edits().unwrap();
    assert!(
        renderer
            .publish_screen_targets(
                capture_channels(&loaded, &[source], 16).unwrap(),
                Vec::new(),
                routes,
                None
            )
            .is_err()
    );
    assert_eq!(renderer.snapshot().project, project);
    assert_eq!(renderer.latest_request(), Some(request));
    close(renderer);
}
