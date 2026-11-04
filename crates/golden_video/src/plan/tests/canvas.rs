use super::*;
use crate::plan::{CanvasPaint, CanvasPattern, CanvasPrimitive, MAX_CANVAS_PRIMITIVES};

struct FixedClock;
impl golden_time::Clock for FixedClock {
    fn now(&self) -> Result<golden_time::MonotonicTime, golden_time::TimeError> {
        Ok(golden_time::MonotonicTime::from_nanos(0))
    }
}

#[test]
fn render_sampling_uses_shared_time_without_copying_the_display_list_or_advancing_time() {
    let mut authority = golden_time::TimeAuthority::new(
        std::sync::Arc::new(FixedClock),
        golden_time::ShowPosition::from_nanos(0),
        golden_time::PlaybackRate::NORMAL,
    )
    .unwrap();
    let now = authority
        .sample()
        .unwrap()
        .at_monotonic(golden_time::MonotonicTime::from_nanos(3_000_000_000))
        .unwrap();
    let canvas = CanvasPattern::new(vec![
        CanvasPrimitive::rectangle(UnitRect::FULL, CanvasPaint::solid(LinearColor::TRANSPARENT)).unwrap(),
    ])
    .unwrap();
    let layer = Layer {
        source: LayerSource::Pattern(Pattern::Canvas(canvas.clone())),
        opacity: crate::Opacity::OPAQUE,
        opacity_envelope: None,
        blend: BlendMode::Normal,
        filter: SampleFilter::Nearest,
        mapping: None,
    };
    for _ in 0..3 {
        let sampled = layer.evaluated(now);
        let LayerSource::Pattern(Pattern::Canvas(value)) = sampled.source else {
            panic!()
        };
        assert_eq!(value.monotonic_nanos, 3_000_000_000);
        assert_eq!(value.storage_identity(), canvas.storage_identity());
    }
    assert_eq!(canvas.monotonic_nanos, 0);
}

#[test]
fn canvas_rejects_unbounded_nonfinite_and_singular_commands_before_retention() {
    let paint = CanvasPaint::solid(LinearColor::TRANSPARENT);
    let primitive = CanvasPrimitive::rectangle(UnitRect::FULL, paint).unwrap();
    assert!(CanvasPattern::new(vec![]).is_none());
    assert!(CanvasPattern::new(vec![primitive; MAX_CANVAS_PRIMITIVES]).is_some());
    assert!(CanvasPattern::new(vec![primitive; MAX_CANVAS_PRIMITIVES + 1]).is_none());
    assert!(CanvasPrimitive::grid(UnitRect::FULL, [f32::NAN, 1.], [0.1; 2], paint).is_none());
    assert!(CanvasPrimitive::line(UnitRect::FULL, [0.; 2], [0.; 2], 0.1, 1., paint).is_none());
    assert!(CanvasPrimitive::line(UnitRect::FULL, [0.; 2], [1e-30; 2], 0.1, 1., paint).is_none());
    assert!(CanvasPrimitive::ellipse(UnitRect::FULL, f32::INFINITY, paint).is_none());
    assert!(CanvasPaint::rainbow(std::time::Duration::ZERO, [1.; 2], crate::Opacity::OPAQUE).is_none());
    assert!(
        CanvasPaint::rainbow(
            std::time::Duration::from_secs(1),
            [f32::NAN, 1.],
            crate::Opacity::OPAQUE
        )
        .is_none()
    );
}

#[test]
fn shared_canvas_storage_is_charged_once() {
    let canvas = CanvasPattern::new(vec![
        CanvasPrimitive::rectangle(UnitRect::FULL, CanvasPaint::solid(LinearColor::TRANSPARENT)).unwrap(),
    ])
    .unwrap();
    let layer = Layer {
        source: LayerSource::Pattern(Pattern::Canvas(canvas.clone())),
        opacity: crate::Opacity::OPAQUE,
        opacity_envelope: None,
        blend: BlendMode::Normal,
        filter: SampleFilter::Nearest,
        mapping: None,
    };
    let mut plan = RenderPlan {
        passes: vec![RenderPass {
            id: pass_id(1),
            output: texture(1),
            descriptor: descriptor(4, 4),
            operation: RenderOperation::Composite {
                layers: vec![layer.clone(), layer],
            },
        }],
        outputs: vec![output(1, 1)],
        ..Default::default()
    };
    let shared_bytes = plan.retained_bytes().unwrap();
    let RenderOperation::Composite { layers } = &mut plan.passes[0].operation else {
        unreachable!()
    };
    layers[1].source = LayerSource::Pattern(Pattern::Canvas(
        CanvasPattern::new(canvas.primitives().to_vec()).unwrap(),
    ));
    assert_eq!(plan.retained_bytes().unwrap() - shared_bytes, canvas.retained_bytes());
}

#[test]
fn canvas_work_uses_the_existing_dependency_budget() {
    let primitive = CanvasPrimitive::rectangle(UnitRect::FULL, CanvasPaint::solid(LinearColor::TRANSPARENT)).unwrap();
    let mut plan = RenderPlan {
        passes: vec![RenderPass {
            id: pass_id(1),
            output: texture(1),
            descriptor: descriptor(4, 4),
            operation: RenderOperation::Composite {
                layers: vec![Layer {
                    source: LayerSource::Pattern(Pattern::Canvas(CanvasPattern::new(vec![primitive; 4]).unwrap())),
                    opacity: crate::Opacity::OPAQUE,
                    opacity_envelope: None,
                    blend: BlendMode::Normal,
                    filter: SampleFilter::Nearest,
                    mapping: None,
                }],
            },
        }],
        outputs: vec![output(1, 1)],
        ..Default::default()
    };
    let mut budget = limits();
    budget.max_dependencies = 3;
    assert!(plan.clone().compile(&budget).is_err());
    budget.max_dependencies = 4;
    assert!(std::mem::take(&mut plan).compile(&budget).is_ok());
}
