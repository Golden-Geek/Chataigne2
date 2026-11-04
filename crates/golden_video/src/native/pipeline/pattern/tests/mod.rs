use super::*;
use crate::{
    Opacity,
    plan::{CanvasPaint, CanvasPattern, UnitRect},
};
use std::time::Duration;

#[test]
fn canvas_constants_preserve_shape_order_alpha_and_wrapped_shared_clock_phase() {
    let paint = CanvasPaint::rainbow(Duration::from_secs(6), [1., -1.], Opacity::new(0.5).unwrap()).unwrap();
    let mut canvas = CanvasPattern::new(vec![
        CanvasPrimitive::grid(UnitRect::FULL, [16., 9.], [0.001, 0.002], paint).unwrap(),
    ])
    .unwrap();
    for nanos in [3_000_000_000, 6_003_000_000_000] {
        canvas.monotonic_nanos = nanos;
        let packed = PatternParameters::new(&Pattern::Canvas(canvas.clone()), true);
        assert_eq!((packed.kind, packed.mapped, packed.reserved[0]), (2, 1, 1));
        let primitive = packed.primitives[0];
        assert_eq!(primitive.metadata[..2], [CanvasShape::Grid as u32, 1]);
        assert_eq!(primitive.paint, [1., -1., 0.5, 0.5]);
        assert_eq!(primitive.geometry[0], [16., 9., 0.001, 0.002]);
        assert_eq!(primitive.colors[0], [0.5; 4]);
    }
}
