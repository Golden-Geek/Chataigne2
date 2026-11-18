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
    overlay: Option<TextureId>,
) -> Result<RenderOperation, String> {
    let source_aspect = intrinsic.width() as f64 / intrinsic.height() as f64;
    let target_aspect = target.width() as f64 / target.height() as f64;
    let direct = mode == MediaFitMode::Stretch || source_aspect == target_aspect;
    if direct && overlay.is_none() {
        return Ok(RenderOperation::Sample {
            source,
            filter: SampleFilter::Linear,
        });
    }
    let mapping = if direct {
        None
    } else {
        let (crop, quad) = geometry(intrinsic, target, mode);
        Some(ProjectiveMapping::new(crop, quad).map_err(|error| error.to_string())?)
    };
    let opacity = Opacity::new(1.0).map_err(|error| error.to_string())?;
    let layers = std::iter::once(source)
        .chain(overlay)
        .map(|texture| Layer {
            source: texture.into(),
            opacity,
            opacity_envelope: None,
            blend: BlendMode::Normal,
            filter: SampleFilter::Linear,
            mapping,
        })
        .collect();
    Ok(RenderOperation::Composite { layers })
}

/// Content geometry inside a normalized placement box; reused without an intermediate render pass.
pub(super) fn geometry(intrinsic: Extent2D, target: Extent2D, mode: MediaFitMode) -> (UnitRect, Quad) {
    let source_aspect = intrinsic.width() as f64 / intrinsic.height() as f64;
    let target_aspect = target.width() as f64 / target.height() as f64;
    match mode {
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
        MediaFitMode::Stretch => (UnitRect::FULL, Quad::default()),
    }
}
