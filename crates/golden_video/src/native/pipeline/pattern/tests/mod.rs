use super::*;
use crate::{
    Opacity,
    plan::{CanvasPaint, CanvasPattern, UnitRect},
};
use std::time::Duration;

#[test]
fn text_packing_preserves_offsets_tiles_and_render_boundary_clock_and_fps() {
    use crate::plan::{CanvasText, CanvasTextStyle};
    let style = CanvasTextStyle {
        cells: [1.; 2],
        glyph: [0.01, 0.02],
        inset: [0.; 2],
        centered: true,
    };
    let paint = CanvasPaint::solid(crate::LinearColor::TRANSPARENT);
    let canvas = CanvasPattern::new(vec![
        CanvasPrimitive::text(
            UnitRect::FULL,
            CanvasText::literal("12345678901234567").unwrap(),
            style,
            paint,
        )
        .unwrap(),
        CanvasPrimitive::text(UnitRect::FULL, CanvasText::local_time(), style, paint).unwrap(),
        CanvasPrimitive::text(UnitRect::FULL, CanvasText::frame_rate(), style, paint).unwrap(),
        CanvasPrimitive::text(
            UnitRect::FULL,
            CanvasText::tiles(&["A1", "B1"]).unwrap(),
            CanvasTextStyle {
                cells: [2., 1.],
                centered: false,
                ..style
            },
            paint,
        )
        .unwrap(),
    ])
    .unwrap();
    let packed = PatternParameters::with_frame(
        &Pattern::Canvas(canvas.clone()),
        false,
        FrameText {
            local: [23, 59, 58, 7],
            fps: Some(49.96),
        },
    );
    let bytes: Vec<u8> = packed.text.into_iter().flatten().flat_map(u32::to_le_bytes).collect();
    assert_eq!(&bytes[..17], b"12345678901234567");
    assert_eq!(&bytes[17..29], b"23:59:58.007");
    assert_eq!(&bytes[29..37], b"50.0 FPS");
    assert_eq!(&bytes[37..53], b"A1\0\0\0\0\0\0B1\0\0\0\0\0\0");
    assert_eq!(packed.primitives[1].metadata[2..], [17, 12]);
    assert_eq!(packed.primitives[3].metadata[2..], [37, 8]);
    let unavailable = AsciiLine::fps(FrameText::default());
    assert_eq!(&unavailable.bytes[..unavailable.len], b"--.- FPS");
}

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
