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
        source: serde_json::to_string(&reference).unwrap(),
        destination: rect,
        source_rect: UnitRect { width: 1., ..rect },
    };
    let preview = ChannelPreview {
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
    let preview = ChannelPreview {
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
