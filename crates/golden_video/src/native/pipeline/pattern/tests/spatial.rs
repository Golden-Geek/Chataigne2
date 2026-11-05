use super::super::spatial as selection;
use super::*;
use crate::plan::{CanvasText, CanvasTextStyle};

fn possible(p: &PrimitiveParameters, uv: [f32; 2]) -> bool {
    let local = [(uv[0] - p.bounds[0]) / p.bounds[2], (uv[1] - p.bounds[1]) / p.bounds[3]];
    if local.iter().any(|value| !(0. ..=1.).contains(value)) {
        return false;
    }
    let [x, y] = local;
    match p.metadata[0] {
        kind if kind == CanvasShape::Ellipse as u32 => {
            ((((x - 0.5) * 2.).powi(2) + ((y - 0.5) * 2.).powi(2)).sqrt() - 1.).abs() <= p.geometry[0][0]
        }
        kind if kind == CanvasShape::Line as u32 => {
            let [sx, sy, ex, ey] = p.geometry[0];
            let [stroke, aspect, _, _] = p.geometry[1];
            let delta = [(ex - sx) * aspect, ey - sy];
            let point = [(x - sx) * aspect, y - sy];
            let along = ((point[0] * delta[0] + point[1] * delta[1]) / (delta[0] * delta[0] + delta[1] * delta[1]))
                .clamp(0., 1.);
            ((point[0] - delta[0] * along).powi(2) + (point[1] - delta[1] * along).powi(2)).sqrt() <= stroke
        }
        kind if kind == CanvasShape::Text as u32 => {
            let [columns, rows, advance, height] = p.geometry[0];
            let tile = [(x * columns).floor(), (y * rows).floor()];
            if tile[0] >= columns.ceil() || tile[1] >= rows.ceil() {
                return false;
            }
            let local = [
                x - tile[0] / columns - p.geometry[1][0],
                y - tile[1] / rows - p.geometry[1][1],
            ];
            local[0] >= 0. && local[1] >= 0. && local[1] < height && (local[0] / advance).floor() < p.metadata[3] as f32
        }
        _ => true,
    }
}

#[test]
fn spatial_selection_contains_every_possible_geometry_and_text_sample_in_authored_order() {
    let paint = CanvasPaint::solid(crate::LinearColor::from_straight(1., 0., 0., 0.5).unwrap());
    let bounds = UnitRect {
        x: 0.125,
        y: 0.1875,
        width: 0.75,
        height: 0.625,
    };
    let mut primitives = vec![
        CanvasPrimitive::rectangle(bounds, paint).unwrap(),
        CanvasPrimitive::ellipse(bounds, 0.003, paint).unwrap(),
        CanvasPrimitive::line(UnitRect::FULL, [0., 0.], [1., 1.], 0.0001, 16. / 9., paint).unwrap(),
        CanvasPrimitive::line(bounds, [0., 1.], [1., 0.], 0.005, 0.25, paint).unwrap(),
        CanvasPrimitive::text(
            bounds,
            CanvasText::literal("CENTERED").unwrap(),
            CanvasTextStyle {
                cells: [1.; 2],
                glyph: [0.025, 0.04],
                inset: [0.; 2],
                centered: true,
            },
            paint,
        )
        .unwrap(),
        CanvasPrimitive::text(
            UnitRect::FULL,
            CanvasText::tiles(&["A1", "B1", "A2", "B2", "A3", "B3"]).unwrap(),
            CanvasTextStyle {
                cells: [1.75, 2.5],
                glyph: [0.01, 0.02],
                inset: [0.03, 0.04],
                centered: false,
            },
            paint,
        )
        .unwrap(),
        CanvasPrimitive::text(
            bounds,
            CanvasText::local_time(),
            CanvasTextStyle {
                cells: [1.; 2],
                glyph: [0.02, 0.04],
                inset: [0.; 2],
                centered: true,
            },
            paint,
        )
        .unwrap(),
    ];
    while primitives.len() < MAX_CANVAS_PRIMITIVES {
        let index = primitives.len();
        primitives.push(
            CanvasPrimitive::rectangle(
                UnitRect {
                    x: (index % 8) as f32 / 8.,
                    y: (index / 8) as f32 / 4.,
                    width: 0.04,
                    height: 0.04,
                },
                paint,
            )
            .unwrap(),
        );
    }
    let packed = PatternParameters::with_frame(
        &Pattern::Canvas(CanvasPattern::new(primitives).unwrap()),
        false,
        FrameText {
            local: [23, 59, 59, 999],
            fps: Some(60.),
        },
    );
    let mut rejected = 0;
    for y in 0..=512 {
        for x in 0..=512 {
            let uv = [x as f32 / 512., y as f32 / 512.];
            let cell = [
                ((uv[0] * selection::SIDE as f32) as usize).min(selection::SIDE - 1),
                ((uv[1] * selection::SIDE as f32) as usize).min(selection::SIDE - 1),
            ];
            let mask = packed.spatial[cell[1] * selection::SIDE + cell[0]];
            for (index, p) in packed.primitives.iter().enumerate() {
                if possible(p, uv) {
                    assert_ne!(mask & (1 << index), 0, "primitive {index} at {uv:?}");
                } else if mask & (1 << index) == 0 {
                    rejected += 1;
                }
            }
            let ordered = (0..MAX_CANVAS_PRIMITIVES)
                .filter(|index| mask & (1 << index) != 0)
                .collect::<Vec<_>>();
            let mut selected = Vec::new();
            let mut remaining = mask;
            while remaining != 0 {
                selected.push(remaining.trailing_zeros() as usize);
                remaining &= remaining - 1;
            }
            assert_eq!(selected, ordered);
        }
    }
    assert!(rejected > 1_000_000, "sparse spatial masks must actually omit work");
    assert_eq!(size_of_val(&packed.spatial), 1024);
}
