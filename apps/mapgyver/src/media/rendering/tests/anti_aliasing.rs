use super::*;
use crate::screens::{protocol::*, rendering::capture_screen};
use golden_video::plan::{RenderOperation, SampleFilter};

fn prepare(
    engine: &AppEngine,
    screen: ScreenId,
    previous: Option<&super::super::plan::Prepared>,
    limits: PlanLimits,
    next: &mut u64,
) -> Result<super::super::plan::Prepared, String> {
    let screen = capture_screen(engine, screen, 8).unwrap();
    let capture = capture_channels(engine, &screen.references(), 16).unwrap();
    let routes = [1, 2].map(|id| ScreenRoute {
        screen: screen.clone(),
        output: Some(OutputId::new(id).unwrap()),
        extent: Extent2D::new(64, 32).unwrap(),
    });
    let empty_channels = HashMap::new();
    let empty_screens = HashMap::new();
    let renderer = renderer();
    let result = super::super::plan::build(
        &capture,
        super::super::plan::Targets {
            routes: &[],
            preview: None,
            screens: &routes,
        },
        super::super::plan::Previous {
            channels: previous.map_or(&empty_channels, |previous| &previous.entries),
            screens: previous.map_or(&empty_screens, |previous| &previous.screens),
            preview_images: &Default::default(),
        },
        next,
        limits,
        super::super::plan::Sources {
            observations: &renderer.source_observations(),
            pictures: None,
            #[cfg(all(windows, feature = "native-video"))]
            shaders: None,
            #[cfg(feature = "video-files")]
            videos: None,
            calibration: &mut Default::default(),
        },
    );
    close(renderer);
    result
}

fn mode(engine: &mut AppEngine, screen: ScreenId, anti_aliasing: ScreenAntiAliasing) {
    let definition = crate::screens::inspect(engine)
        .unwrap()
        .screens
        .into_iter()
        .find(|item| item.id == screen)
        .unwrap()
        .definition;
    super::screens::screen_edit(
        engine,
        ScreenEdit::ConfigureScreen {
            screen,
            definition: ScreenDefinition {
                anti_aliasing,
                ..definition
            },
        },
    );
}

#[test]
fn smooth_resolves_once_and_raw_roundtrip_preserves_graph_and_texture_identities() {
    let mut engine = engine();
    let source = create(&mut engine, MediaKind::SolidColor);
    resize(&mut engine, source, 8);
    let (screen, _) = super::screens::screen(&mut engine, source);
    let limits = config().plan_limits;
    let mut next = 3;
    let raw = prepare(&engine, screen, None, limits, &mut next).unwrap();
    assert_eq!(raw.submission.plan.passes.len(), 2);
    mode(&mut engine, screen, ScreenAntiAliasing::Smooth2x);
    let smooth = prepare(&engine, screen, Some(&raw), limits, &mut next).unwrap();
    let passes = &smooth.submission.plan.passes;
    assert_eq!(passes.len(), 3);
    assert_eq!(smooth.entries.len(), raw.entries.len());
    assert_eq!(
        format!("{:?}", passes[0]),
        format!("{:?}", raw.submission.plan.passes[0]),
        "source producer unchanged"
    );
    assert_eq!(passes[1].descriptor.extent, Extent2D::new(128, 64).unwrap());
    assert_eq!(passes[2].descriptor.extent, Extent2D::new(64, 32).unwrap());
    assert_eq!(passes[2].output, raw.screens[&screen].target);
    assert!(
        matches!(passes[2].operation, RenderOperation::Sample { source, filter: SampleFilter::Linear } if source == passes[1].output)
    );
    assert_eq!(
        format!("{:?}", smooth.submission.plan.outputs),
        format!("{:?}", raw.submission.plan.outputs)
    );
    let RenderOperation::Composite { layers: raw_layers } = &raw.submission.plan.passes[1].operation else {
        panic!()
    };
    let RenderOperation::Composite { layers: smooth_layers } = &passes[1].operation else {
        panic!()
    };
    assert_eq!(
        format!("{raw_layers:?}"),
        format!("{smooth_layers:?}"),
        "normalized mapping and source sampling unchanged"
    );
    mode(&mut engine, screen, ScreenAntiAliasing::Raw);
    let restored = prepare(&engine, screen, Some(&smooth), limits, &mut next).unwrap();
    assert_eq!(
        format!("{:?}", restored.submission.plan),
        format!("{:?}", raw.submission.plan)
    );
    mode(&mut engine, screen, ScreenAntiAliasing::Smooth2x);
    let again = prepare(&engine, screen, Some(&restored), limits, &mut next).unwrap();
    assert_eq!(
        format!("{:?}", again.submission.plan),
        format!("{:?}", smooth.submission.plan)
    );
}

#[test]
fn smooth_rasterizes_calibration_at_double_density_with_unchanged_logical_layout() {
    let mut engine = engine();
    let source = create(&mut engine, MediaKind::SolidColor);
    let (screen, surface) = super::screens::screen(&mut engine, source);
    super::screens::screen_edit(
        &mut engine,
        ScreenEdit::ConfigureSurface {
            reference: surface,
            definition: SurfaceDefinition {
                calibration: Calibration::Grid,
                ..Default::default()
            },
        },
    );
    let mut next = 3;
    let limits = config().plan_limits;
    let raw = prepare(&engine, screen, None, limits, &mut next).unwrap();
    mode(&mut engine, screen, ScreenAntiAliasing::Smooth2x);
    let smooth = prepare(&engine, screen, Some(&raw), limits, &mut next).unwrap();
    assert!(raw.channels.is_empty(), "calibration needs no source producer");
    assert!(smooth.channels.is_empty());
    assert_eq!(raw.submission.plan.passes.len(), 1);
    assert_eq!(smooth.submission.plan.passes.len(), 2);
    assert_eq!(
        format!("{:?}", raw.submission.plan.passes[0].operation),
        format!("{:?}", smooth.submission.plan.passes[0].operation)
    );
    assert_eq!(
        smooth.submission.plan.passes[0].descriptor.extent,
        Extent2D::new(128, 64).unwrap()
    );
}

#[test]
fn four_density_resolves_all_subpixels_and_preserves_producers_and_output_identity() {
    let mut engine = engine();
    let source = create(&mut engine, MediaKind::SolidColor);
    resize(&mut engine, source, 8);
    let (screen, _) = super::screens::screen(&mut engine, source);
    let limits = config().plan_limits;
    let mut next = 3;
    let raw = prepare(&engine, screen, None, limits, &mut next).unwrap();
    mode(&mut engine, screen, ScreenAntiAliasing::Smooth4x);
    let smooth = prepare(&engine, screen, Some(&raw), limits, &mut next).unwrap();
    let passes = &smooth.submission.plan.passes;
    assert_eq!(passes.len(), 4);
    assert_eq!(
        format!("{:?}", passes[0]),
        format!("{:?}", raw.submission.plan.passes[0])
    );
    assert_eq!(passes[1].descriptor.extent, Extent2D::new(256, 128).unwrap());
    assert_eq!(passes[2].descriptor.extent, Extent2D::new(128, 64).unwrap());
    assert_eq!(passes[3].descriptor.extent, Extent2D::new(64, 32).unwrap());
    for index in [2, 3] {
        assert!(
            matches!(passes[index].operation, RenderOperation::Sample { source, filter: SampleFilter::Linear } if source == passes[index - 1].output)
        );
    }
    assert_eq!(passes[3].output, raw.screens[&screen].target);
    mode(&mut engine, screen, ScreenAntiAliasing::Smooth2x);
    let two = prepare(&engine, screen, Some(&smooth), limits, &mut next).unwrap();
    mode(&mut engine, screen, ScreenAntiAliasing::Raw);
    let restored = prepare(&engine, screen, Some(&two), limits, &mut next).unwrap();
    assert_eq!(
        format!("{:?}", restored.submission.plan),
        format!("{:?}", raw.submission.plan)
    );
    mode(&mut engine, screen, ScreenAntiAliasing::Smooth4x);
    let again = prepare(&engine, screen, Some(&restored), limits, &mut next).unwrap();
    assert_eq!(
        format!("{:?}", again.submission.plan),
        format!("{:?}", smooth.submission.plan)
    );
}

#[test]
fn four_density_tiles_within_existing_dimensions_and_rejects_pass_and_memory_overruns() {
    let mut engine = engine();
    let source = create(&mut engine, MediaKind::SolidColor);
    resize(&mut engine, source, 8);
    let (screen, _) = super::screens::screen(&mut engine, source);
    mode(&mut engine, screen, ScreenAntiAliasing::Smooth4x);
    let mut limits = config().plan_limits;
    limits.max_texture_dimension = 128;
    let prepared = prepare(&engine, screen, None, limits, &mut 3).unwrap();
    let passes = &prepared.submission.plan.passes;
    assert_eq!(
        passes.len(),
        8,
        "one shared producer, two three-pass tiles, one assembly"
    );
    assert!(
        passes
            .iter()
            .all(|pass| pass.descriptor.extent.width() <= 128 && pass.descriptor.extent.height() <= 128)
    );
    let RenderOperation::Regions { regions, .. } = &passes.last().unwrap().operation else {
        panic!()
    };
    assert_eq!(regions.len(), 2);
    assert_eq!([regions[0].destination.x, regions[1].destination.x], [0., 0.5]);
    assert!(
        regions
            .iter()
            .all(|region| region.destination.width == 0.5 && region.destination.height == 1.)
    );
    limits.max_passes = 7;
    assert!(prepare(&engine, screen, None, limits, &mut 3).is_err());
    limits.max_passes = 8;
    limits.max_texture_bytes = 64 * 32 * 4;
    assert!(prepare(&engine, screen, None, limits, &mut 3).is_err());
}

#[test]
fn smooth_is_rejected_without_fallback_at_existing_pass_dimension_and_byte_limits() {
    let mut engine = engine();
    let source = create(&mut engine, MediaKind::SolidColor);
    resize(&mut engine, source, 8);
    let (screen, _) = super::screens::screen(&mut engine, source);
    let mut limits = config().plan_limits;
    limits.max_passes = 2;
    assert!(prepare(&engine, screen, None, limits, &mut 3).is_ok());
    mode(&mut engine, screen, ScreenAntiAliasing::Smooth2x);
    assert!(prepare(&engine, screen, None, limits, &mut 3).is_err());
    limits = config().plan_limits;
    limits.max_texture_dimension = 64;
    assert!(prepare(&engine, screen, None, limits, &mut 3).is_err());
    mode(&mut engine, screen, ScreenAntiAliasing::Raw);
    assert!(prepare(&engine, screen, None, limits, &mut 3).is_ok());
    limits = config().plan_limits;
    limits.max_texture_bytes = (8 * 2 + 64 * 32) * 4;
    assert!(prepare(&engine, screen, None, limits, &mut 3).is_ok());
    mode(&mut engine, screen, ScreenAntiAliasing::Smooth2x);
    assert!(prepare(&engine, screen, None, limits, &mut 3).is_err());
    limits.max_texture_bytes += 128 * 64 * 4;
    assert!(prepare(&engine, screen, None, limits, &mut 3).is_ok());
}
