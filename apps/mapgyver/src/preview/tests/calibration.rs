use crate::{
    preview::{layout::PreviewLayout, protocol::PreviewSource},
    screens::protocol::Calibration,
};
use golden_io::RevisionSource;
use golden_video::runtime::PlanSubmission;
use golden_video::{
    AlphaMode, ColorEncoding, Extent2D, LinearColor, OutputId, PixelFormat, TextureDescriptor, TextureId,
};
use golden_video::{
    plan::{LayerSource, PlanLimits, RenderOperation, UnitRect},
    preview::{PreviewSnapshot, ResolvedView},
};
use std::{collections::HashMap, sync::Arc};

fn limits() -> PlanLimits {
    crate::preview::config::configuration(16 * 1024 * 1024, None)
        .video
        .plan_limits
}

fn layout(sources: &[PreviewSource]) -> PreviewLayout {
    let revision = RevisionSource::default();
    let reader = revision.reader();
    let guard = reader.guard_at(reader.current());
    PreviewLayout {
        output: OutputId::new(1).unwrap(),
        descriptor: TextureDescriptor {
            extent: Extent2D::new(128, 64).unwrap(),
            format: PixelFormat::Rgba8Unorm,
            color_encoding: ColorEncoding::Linear,
            alpha_mode: AlphaMode::Premultiplied,
        },
        layout: PreviewSnapshot::new(
            reader.current(),
            sources
                .iter()
                .enumerate()
                .map(|(index, source)| ResolvedView {
                    id: uuid::Uuid::new_v4(),
                    source: serde_json::to_string(source).unwrap(),
                    destination: UnitRect {
                        x: index as f32 / sources.len() as f32,
                        y: 0.,
                        width: 1. / sources.len() as f32,
                        height: 1.,
                    },
                    backdrop: UnitRect::FULL,
                    source_rect: UnitRect::FULL,
                    overlays: Arc::from([]),
                    overlay_scale: 1.,
                    overlay_capacity: 0,
                    tile_groups: None,
                })
                .collect(),
            guard,
            LinearColor::TRANSPARENT,
        ),
    }
}

#[test]
fn repeated_pattern_views_share_one_stable_native_pass_without_channels_or_external_inputs() {
    let source = PreviewSource::Calibration {
        mode: Calibration::ColorBars,
        extent: Extent2D::new(96, 64).unwrap(),
    };
    let preview = layout(&[source, source]);
    assert_eq!(preview.pass_count().unwrap(), PreviewLayout::PASS_COUNT + 1);
    assert!(preview.references().unwrap().is_empty());
    let mut first = PlanSubmission::new(Default::default());
    preview
        .append(
            &mut first,
            &HashMap::new(),
            &HashMap::new(),
            &mut Default::default(),
            None,
        )
        .unwrap();
    assert!(first.plan.external_textures.is_empty());
    let patterns = first.plan.passes.iter().filter(|pass| matches!(&pass.operation, RenderOperation::Composite { layers } if layers.iter().any(|layer| matches!(layer.source, LayerSource::Pattern(_))))).collect::<Vec<_>>();
    assert_eq!(patterns.len(), 1);
    assert_eq!(patterns[0].descriptor.extent, Extent2D::new(96, 64).unwrap());
    assert!(patterns[0].output.get() >= PreviewLayout::CALIBRATION_TEXTURE_BASE);
    let pattern_texture = patterns[0].output;
    let RenderOperation::Regions { regions, .. } = &first
        .plan
        .passes
        .iter()
        .find(|pass| pass.output == TextureId::new(1).unwrap())
        .unwrap()
        .operation
    else {
        panic!()
    };
    assert_eq!(regions.len(), 2);
    assert!(regions.iter().all(|region| region.source == pattern_texture));
    first.plan.compile(&limits()).unwrap();
    let mut second = PlanSubmission::new(Default::default());
    preview
        .append(
            &mut second,
            &HashMap::new(),
            &HashMap::new(),
            &mut Default::default(),
            None,
        )
        .unwrap();
    assert_eq!(second.plan.passes[0].output, pattern_texture);
}

#[test]
fn grid_uses_the_existing_native_canvas_pattern_and_original_admission_limits() {
    let source = PreviewSource::Calibration {
        mode: Calibration::Grid,
        extent: Extent2D::new(96, 64).unwrap(),
    };
    let preview = layout(&[source]);
    let mut submission = PlanSubmission::new(Default::default());
    preview
        .append(
            &mut submission,
            &HashMap::new(),
            &HashMap::new(),
            &mut Default::default(),
            None,
        )
        .unwrap();
    assert!(
        matches!(&submission.plan.passes[0].operation, RenderOperation::Composite { layers } if matches!(layers[0].source, LayerSource::Pattern(golden_video::plan::Pattern::Canvas(_))))
    );
    submission.plan.clone().compile(&limits()).unwrap();
    let limits = PlanLimits {
        max_passes: PreviewLayout::PASS_COUNT,
        ..limits()
    };
    assert!(submission.plan.compile(&limits).is_err());
    let invalid = layout(&[PreviewSource::Calibration {
        mode: Calibration::Off,
        extent: Extent2D::new(96, 64).unwrap(),
    }]);
    assert!(invalid.pass_count().is_err());
    let large = layout(&[PreviewSource::Calibration {
        mode: Calibration::Grid,
        extent: Extent2D::new(8193, 64).unwrap(),
    }]);
    assert!(large.maximum_dependencies().is_err());
}
