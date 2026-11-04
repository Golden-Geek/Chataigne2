//! Media source sizing policy. Golden owns checked geometry and GPU execution.
use crate::media::protocol::MediaFitMode;
use golden_video::{
    Extent2D, Opacity, TextureId,
    plan::{BlendMode, Layer, Point, ProjectiveMapping, Quad, RenderOperation, SampleFilter, UnitRect},
};

pub(super) fn operation(
    source: TextureId,
    intrinsic: Extent2D,
    target: Extent2D,
    mode: MediaFitMode,
    filter: SampleFilter,
) -> Result<RenderOperation, String> {
    let source_aspect = intrinsic.width() as f64 / intrinsic.height() as f64;
    let target_aspect = target.width() as f64 / target.height() as f64;
    if mode == MediaFitMode::Stretch || source_aspect == target_aspect {
        return Ok(RenderOperation::Sample { source, filter });
    }
    let (crop, quad) = match mode {
        MediaFitMode::Fit => {
            let width = (source_aspect / target_aspect).min(1.0) as f32;
            let height = (target_aspect / source_aspect).min(1.0) as f32;
            let x = (1.0 - width) / 2.0;
            let y = (1.0 - height) / 2.0;
            (
                UnitRect::FULL,
                Quad {
                    corners: [
                        Point { x, y },
                        Point { x: x + width, y },
                        Point {
                            x: x + width,
                            y: y + height,
                        },
                        Point { x, y: y + height },
                    ],
                },
            )
        }
        MediaFitMode::Fill => {
            let width = (target_aspect / source_aspect).min(1.0) as f32;
            let height = (source_aspect / target_aspect).min(1.0) as f32;
            (
                UnitRect {
                    x: (1.0 - width) / 2.0,
                    y: (1.0 - height) / 2.0,
                    width,
                    height,
                },
                Quad::default(),
            )
        }
        MediaFitMode::Stretch => unreachable!(),
    };
    Ok(RenderOperation::Composite {
        layers: vec![Layer {
            source: source.into(),
            opacity: Opacity::new(1.0).map_err(|error| error.to_string())?,
            opacity_envelope: None,
            blend: BlendMode::Normal,
            filter,
            mapping: Some(ProjectiveMapping::new(crop, quad).map_err(|error| error.to_string())?),
        }],
    })
}
