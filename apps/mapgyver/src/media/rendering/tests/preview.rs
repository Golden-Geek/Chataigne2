use super::*;
use golden_io::RevisionSource;
use golden_video::plan::{RenderOperation, UnitRect};
use golden_video::preview::{PreviewSnapshot, ResolvedView};
use golden_video::{AlphaMode, ColorEncoding, PixelFormat, TextureId};

#[test]
fn preview_reuses_channel_textures_and_withdraws_geometry_without_revoking_other_outputs() {
    let mut engine = engine();
    let reference = create(&mut engine, MediaKind::SolidColor);
    resize(&mut engine, reference, 2);
    let capture = capture_channels(&engine, &[reference], 8).unwrap();
    let revision = RevisionSource::default();
    let reader = revision.reader();
    let rect = UnitRect {
        x: 0.,
        y: 0.,
        width: 0.5,
        height: 1.,
    };
    let view = ResolvedView {
        id: uuid::Uuid::new_v4(),
        source: serde_json::to_string(&crate::preview::protocol::PreviewSource::Channel { reference }).unwrap(),
        destination: rect,
        source_rect: UnitRect { width: 1., ..rect },
    };
    let preview = PreviewLayout {
        output: OutputId::new(9).unwrap(),
        descriptor: golden_video::TextureDescriptor {
            extent: Extent2D::new(8, 8).unwrap(),
            format: PixelFormat::Rgba8Unorm,
            color_encoding: ColorEncoding::Linear,
            alpha_mode: AlphaMode::Premultiplied,
        },
        layout: PreviewSnapshot {
            generation: reader.current(),
            views: vec![
                view.clone(),
                ResolvedView {
                    id: uuid::Uuid::new_v4(),
                    ..view
                },
            ],
            guard: reader.guard_at(reader.current()),
        },
    };
    let renderer = renderer();
    let mut next = 3;
    let prepared = super::super::plan::build(
        &capture,
        super::super::plan::Targets {
            screens: &[],
            routes: &routes(reference),
            preview: Some(&preview),
        },
        super::super::plan::Previous {
            channels: &HashMap::new(),
            screens: &HashMap::new(),
        },
        &mut next,
        config().plan_limits,
        &renderer.source_observations(),
        None,
    )
    .unwrap();
    assert_eq!(prepared.channels.len(), 1);
    assert_eq!(prepared.submission.plan.passes.len(), 4);
    assert_eq!(prepared.submission.plan.outputs.len(), 3);
    let pass = prepared
        .submission
        .plan
        .passes
        .iter()
        .find(|pass| pass.output == TextureId::new(1).unwrap())
        .unwrap();
    let RenderOperation::Regions { regions, visibility } = &pass.operation else {
        panic!()
    };
    assert_eq!(regions.len(), 2);
    assert_eq!(regions[0].source, regions[1].source);
    assert!(visibility.as_ref().unwrap().is_current());
    revision.advance();
    assert!(!visibility.as_ref().unwrap().is_current());
    assert!(prepared.submission.guards.iter().all(|guard| guard.is_current()));
    close(renderer);
}

#[test]
fn malformed_preview_reference_cannot_publish_a_plan() {
    let revision = RevisionSource::default();
    let reader = revision.reader();
    let preview = PreviewLayout {
        output: OutputId::new(1).unwrap(),
        descriptor: golden_video::TextureDescriptor {
            extent: Extent2D::new(8, 8).unwrap(),
            format: PixelFormat::Rgba8Unorm,
            color_encoding: ColorEncoding::Linear,
            alpha_mode: AlphaMode::Premultiplied,
        },
        layout: PreviewSnapshot {
            generation: reader.current(),
            views: vec![ResolvedView {
                id: uuid::Uuid::new_v4(),
                source: "not a channel reference".into(),
                destination: UnitRect {
                    x: 0.,
                    y: 0.,
                    width: 1.,
                    height: 1.,
                },
                source_rect: UnitRect {
                    x: 0.,
                    y: 0.,
                    width: 1.,
                    height: 1.,
                },
            }],
            guard: reader.guard_at(reader.current()),
        },
    };
    let engine = engine();
    let mut renderer = renderer();
    assert!(
        renderer
            .publish_targets(capture_channels(&engine, &[], 8).unwrap(), vec![], Some(preview))
            .is_err()
    );
    assert!(renderer.latest_request().is_none());
    close(renderer);
}

fn screen_edit(
    engine: &mut AppEngine,
    edit: crate::screens::protocol::ScreenEdit,
) -> crate::screens::protocol::ScreenEditResult {
    use crate::screens::protocol::*;
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

#[test]
fn mixed_screen_and_channel_views_share_sources_and_invalidate_on_surface_edits_and_delete_undo() {
    use crate::preview::{capture::capture_preview, protocol::PreviewSource};
    use crate::screens::protocol::*;
    let mut engine = engine();
    let reference = create(&mut engine, MediaKind::SolidColor);
    let ScreenEditResult::ScreenCreated { screen, surface } =
        screen_edit(&mut engine, ScreenEdit::CreateScreen { label: None })
    else {
        panic!()
    };
    screen_edit(
        &mut engine,
        ScreenEdit::ConfigureSurface {
            reference: surface,
            definition: SurfaceDefinition {
                source: Some(reference),
                ..Default::default()
            },
        },
    );
    let source = PreviewSource::Screen { screen };
    let sources = vec![PreviewSource::Channel { reference }, source, source];
    let capture = capture_preview(&engine, &sources, 8, 8).unwrap();
    assert_eq!(capture.screens.len(), 1);
    assert!(capture.screens[0].output.is_none());
    let revision = RevisionSource::default();
    let reader = revision.reader();
    let preview = PreviewLayout {
        output: OutputId::new(1).unwrap(),
        descriptor: golden_video::TextureDescriptor {
            extent: Extent2D::new(8, 8).unwrap(),
            format: PixelFormat::Rgba8Unorm,
            color_encoding: ColorEncoding::Linear,
            alpha_mode: AlphaMode::Premultiplied,
        },
        layout: PreviewSnapshot {
            generation: 0,
            guard: reader.guard_at(0),
            views: sources
                .iter()
                .enumerate()
                .map(|(index, source)| ResolvedView {
                    id: uuid::Uuid::new_v4(),
                    source: serde_json::to_string(source).unwrap(),
                    destination: UnitRect {
                        x: index as f32 * 0.25,
                        y: 0.0,
                        width: 0.25,
                        height: 1.0,
                    },
                    source_rect: UnitRect::FULL,
                })
                .collect(),
        },
    };
    let mut renderer = renderer();
    let prepared = super::super::plan::build(
        &capture.channels,
        super::super::plan::Targets {
            routes: &[],
            screens: &capture.screens,
            preview: Some(&preview),
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
    assert_eq!(prepared.channels.len(), 1);
    assert_eq!(prepared.screens.len(), 1);
    assert_eq!(prepared.submission.plan.passes.len(), 5);
    assert_eq!(prepared.submission.plan.outputs.len(), 1);
    let RenderOperation::Regions { regions, .. } = &prepared
        .submission
        .plan
        .passes
        .iter()
        .find(|pass| pass.output == TextureId::new(1).unwrap())
        .unwrap()
        .operation
    else {
        panic!()
    };
    assert_eq!(regions.len(), 3);
    assert_ne!(regions[0].source, regions[1].source);
    assert_eq!(regions[1].source, regions[2].source);
    let request = renderer
        .publish_screen_targets(
            capture.channels.clone(),
            Vec::new(),
            capture.screens.clone(),
            Some(preview),
        )
        .unwrap();
    installed(&renderer, request);
    assert_eq!(renderer.snapshot().resources.unwrap().active.unwrap().output_count, 1);
    screen_edit(
        &mut engine,
        ScreenEdit::ConfigureSurface {
            reference: surface,
            definition: SurfaceDefinition {
                source: Some(reference),
                opacity: 0.5,
                ..Default::default()
            },
        },
    );
    assert!(!capture.is_current());
    assert!(capture.channels.is_current());
    assert!(prepared.submission.guards.iter().any(|guard| !guard.is_current()));
    screen_edit(&mut engine, ScreenEdit::RemoveScreen { screen });
    let missing = capture_preview(&engine, &sources, 8, 8).unwrap();
    assert!(missing.screens.is_empty());
    assert!(
        missing
            .statuses(renderer.channels())
            .iter()
            .any(|status| status.source == source && status.error.is_some())
    );
    assert!(engine.undo().unwrap());
    assert!(!missing.is_current());
    assert_eq!(capture_preview(&engine, &sources, 8, 8).unwrap().screens.len(), 1);
    close(renderer);
}

#[test]
fn unsupported_screen_preview_is_isolated_and_recovers_on_an_authored_revision() {
    use crate::preview::{capture::capture_preview, protocol::PreviewSource};
    use crate::screens::protocol::*;
    let mut engine = engine();
    let reference = create(&mut engine, MediaKind::SolidColor);
    let ScreenEditResult::ScreenCreated { screen, surface } =
        screen_edit(&mut engine, ScreenEdit::CreateScreen { label: None })
    else {
        panic!()
    };
    screen_edit(
        &mut engine,
        ScreenEdit::ConfigureSurface {
            reference: surface,
            definition: SurfaceDefinition {
                source: Some(reference),
                calibration: Calibration::Grid,
                ..Default::default()
            },
        },
    );
    let sources = [PreviewSource::Channel { reference }, PreviewSource::Screen { screen }];
    let capture = capture_preview(&engine, &sources, 8, 8).unwrap();
    assert!(capture.screens.is_empty());
    let mut renderer = renderer();
    let request = renderer.publish(capture.channels.clone(), routes(reference)).unwrap();
    installed(&renderer, request);
    let statuses = capture.statuses(renderer.channels());
    assert!(
        statuses
            .iter()
            .any(|status| status.source == sources[0] && status.error.is_none() && status.message.is_none())
    );
    assert!(
        statuses
            .iter()
            .any(|status| status.source == sources[1] && status.error.is_some())
    );
    screen_edit(
        &mut engine,
        ScreenEdit::ConfigureSurface {
            reference: surface,
            definition: SurfaceDefinition {
                source: Some(reference),
                ..Default::default()
            },
        },
    );
    assert!(!capture.is_current());
    let recovered = capture_preview(&engine, &sources, 8, 8).unwrap();
    assert_eq!(recovered.screens.len(), 1);
    assert!(
        recovered
            .statuses(renderer.channels())
            .iter()
            .all(|status| status.error.is_none())
    );
    assert!(capture_preview(&engine, &sources, 1, 8).is_err());
    let exceeded = capture_preview(&engine, &sources, 8, 0).unwrap();
    assert!(exceeded.screens.is_empty());
    assert!(
        exceeded
            .statuses(renderer.channels())
            .iter()
            .any(|status| status.source == sources[1] && status.error.is_some())
    );
    close(renderer);
}
