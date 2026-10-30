use super::*;
use crate::screens::{protocol::*, rendering::capture_screen};
use golden_video::plan::{LayerSource, Pattern};
use golden_video::plan::{Point, RenderOperation, UnitRect};

pub(super) fn screen_edit(engine: &mut AppEngine, edit: ScreenEdit) -> ScreenEditResult {
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

pub(super) fn screen(engine: &mut AppEngine, channel: ChannelReference) -> (ScreenId, SurfaceReference) {
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
    assert_eq!(layers[0].source.texture(), Some(prepared.channels[0].target));
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

#[test]
fn calibration_needs_no_source_and_restores_the_authored_channel_crop_quad_and_opacity() {
    let mut engine = engine();
    let source = create(&mut engine, MediaKind::SolidColor);
    let (screen, surface) = screen(&mut engine, source);
    let original = crate::screens::inspect(&engine).unwrap().screens[0].surfaces[0]
        .definition
        .clone();
    let mut renderer = renderer();
    for mode in [Calibration::Grid, Calibration::ColorBars, Calibration::Off] {
        let definition = SurfaceDefinition {
            calibration: mode,
            ..original.clone()
        };
        screen_edit(
            &mut engine,
            ScreenEdit::ConfigureSurface {
                reference: surface,
                definition: definition.clone(),
            },
        );
        assert_eq!(
            crate::screens::inspect(&engine).unwrap().screens[0].surfaces[0].definition,
            definition
        );
        let captured = capture_screen(&engine, screen, 8).unwrap();
        let references = captured.references();
        assert_eq!(references, if mode == Calibration::Off { vec![source] } else { vec![] });
        let channels = capture_channels(&engine, &references, 8).unwrap();
        let screen_routes = vec![ScreenRoute {
            output: Some(OutputId::new(1).unwrap()),
            extent: Extent2D::new(320, 180).unwrap(),
            screen: captured,
        }];
        let prepared = super::super::plan::build(
            &channels,
            super::super::plan::Targets {
                routes: &[],
                screens: &screen_routes,
                preview: None,
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
        assert!(prepared.submission.plan.external_textures.is_empty());
        assert!(prepared.submission.source_demands.is_empty());
        let RenderOperation::Composite { layers } = &prepared.submission.plan.passes.last().unwrap().operation else {
            panic!()
        };
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].opacity.get(), original.opacity);
        assert_eq!(
            layers[0].mapping.unwrap().crop(),
            if mode == Calibration::Off {
                original.crop
            } else {
                UnitRect::FULL
            }
        );
        match (mode, layers[0].source) {
            (Calibration::Grid, LayerSource::Pattern(Pattern::Grid(_)))
            | (Calibration::ColorBars, LayerSource::Pattern(Pattern::Bars(_)))
            | (Calibration::Off, LayerSource::Texture(_)) => {}
            _ => panic!("wrong calibration source"),
        }
        let request = renderer
            .publish_screen_targets(channels, Vec::new(), screen_routes, None)
            .unwrap();
        installed(&renderer, request);
        assert_eq!(renderer.channels().len(), usize::from(mode == Calibration::Off));
    }
    assert_eq!(
        crate::screens::inspect(&engine).unwrap().screens[0].surfaces[0].definition,
        original
    );
    for mode in [Calibration::Grid, Calibration::ColorBars] {
        screen_edit(
            &mut engine,
            ScreenEdit::ConfigureSurface {
                reference: surface,
                definition: SurfaceDefinition {
                    source: None,
                    calibration: mode,
                    ..original.clone()
                },
            },
        );
        let capture = crate::preview::capture::capture_preview(
            &engine,
            &[crate::preview::protocol::PreviewSource::Screen { screen }],
            8,
            8,
        )
        .unwrap();
        assert!(capture.screens[0].screen.references().is_empty());
        assert!(
            capture
                .statuses(&[])
                .iter()
                .all(|status| status.error.is_none() && status.message.is_none())
        );
    }
    close(renderer);
}

#[test]
fn surface_reordering_preserves_each_procedural_or_texture_layer_alpha_and_mapping() {
    let mut engine = engine();
    let source = create(&mut engine, MediaKind::SolidColor);
    let (screen, first) = screen(&mut engine, source);
    let ScreenEditResult::SurfaceCreated { reference: second } =
        screen_edit(&mut engine, ScreenEdit::AddSurface { screen, label: None })
    else {
        panic!()
    };
    screen_edit(
        &mut engine,
        ScreenEdit::ConfigureSurface {
            reference: second,
            definition: SurfaceDefinition {
                source: None,
                calibration: Calibration::ColorBars,
                opacity: 0.25,
                ..Default::default()
            },
        },
    );
    let snapshot = crate::screens::inspect(&engine).unwrap();
    let layers = capture_screen(&engine, screen, 8)
        .unwrap()
        .layers(&HashMap::from([(source, TextureId::new(10).unwrap())]))
        .unwrap();
    assert!(matches!(layers[0].source, LayerSource::Texture(_)));
    assert!(matches!(layers[1].source, LayerSource::Pattern(Pattern::Bars(_))));
    assert_eq!(layers[1].opacity.get(), 0.25);
    screen_edit(
        &mut engine,
        ScreenEdit::ReorderSurface {
            reference: second,
            after: None,
        },
    );
    let layers = capture_screen(&engine, screen, 8)
        .unwrap()
        .layers(&HashMap::from([(source, TextureId::new(10).unwrap())]))
        .unwrap();
    assert!(matches!(layers[0].source, LayerSource::Pattern(Pattern::Bars(_))));
    assert!(matches!(layers[1].source, LayerSource::Texture(_)));
    assert_eq!(layers[0].opacity.get(), 0.25);
    assert_eq!(
        layers[1].opacity.get(),
        snapshot.screens[0].surfaces[0].definition.opacity
    );
    assert!(engine.undo().unwrap());
    assert_eq!(crate::screens::inspect(&engine).unwrap(), snapshot);
    assert_eq!(snapshot.screens[0].surfaces[0].reference, first);
}
