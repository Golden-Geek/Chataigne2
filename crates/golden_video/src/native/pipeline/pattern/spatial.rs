//! Conservative fixed-size Canvas selection, prepared on the rendering worker.
use super::PrimitiveParameters;
use crate::plan::CanvasShape;

pub(super) const SIDE: usize = 16;
pub(super) const CELLS: usize = SIDE * SIDE;

pub(super) fn masks(primitives: &[PrimitiveParameters]) -> [u32; CELLS] {
    let mut result = [0; CELLS];
    for (index, primitive) in primitives.iter().enumerate() {
        let bit = 1u32 << index;
        if primitive.metadata[0] == CanvasShape::Text as u32 {
            text(&mut result, primitive, bit);
            continue;
        }
        let [x, y, width, height] = primitive.bounds.map(f64::from);
        for row in span(y, y + height) {
            for column in span(x, x + width) {
                if may_cover(primitive, column, row) {
                    result[row * SIDE + column] |= bit;
                }
            }
        }
    }
    result
}

fn text(result: &mut [u32; CELLS], primitive: &PrimitiveParameters, bit: u32) {
    let [x, y, width, height] = primitive.bounds.map(f64::from);
    let [columns, rows, advance, glyph_height] = primitive.geometry[0].map(f64::from);
    let [inset_x, inset_y, _, _] = primitive.geometry[1].map(f64::from);
    let run_width = f64::from(primitive.metadata[3]) * advance;
    for row in 0..rows.ceil() as usize {
        for column in 0..columns.ceil() as usize {
            let left = (column as f64 / columns + inset_x).max(0.);
            let top = (row as f64 / rows + inset_y).max(0.);
            let right = (column as f64 / columns + inset_x + run_width)
                .min((column + 1) as f64 / columns)
                .min(1.);
            let bottom = (row as f64 / rows + inset_y + glyph_height)
                .min((row + 1) as f64 / rows)
                .min(1.);
            if right < left || bottom < top {
                continue;
            }
            for cell_y in span(y + top * height, y + bottom * height) {
                for cell_x in span(x + left * width, x + right * width) {
                    result[cell_y * SIDE + cell_x] |= bit;
                }
            }
        }
    }
}

fn span(begin: f64, end: f64) -> std::ops::RangeInclusive<usize> {
    // Include adjacent bins at exact boundaries and tolerate the shader's f32 transforms.
    let cell = |value: f64| (value * SIDE as f64).floor().clamp(0., (SIDE - 1) as f64) as usize;
    cell(begin - 0.00001)..=cell(end + 0.00001)
}

fn may_cover(primitive: &PrimitiveParameters, column: usize, row: usize) -> bool {
    let kind = primitive.metadata[0];
    if kind != CanvasShape::Line as u32 && kind != CanvasShape::Ellipse as u32 {
        return true;
    }
    let [x, y, width, height] = primitive.bounds.map(f64::from);
    let center = [
        ((column as f64 + 0.5) / SIDE as f64 - x) / width,
        ((row as f64 + 0.5) / SIDE as f64 - y) / height,
    ];
    let half = [0.5 / SIDE as f64 / width, 0.5 / SIDE as f64 / height];
    if kind == CanvasShape::Ellipse as u32 {
        let distance = ((center[0] - 0.5) * 2.).hypot((center[1] - 0.5) * 2.);
        let radius = (half[0] * 2.).hypot(half[1] * 2.);
        return (distance - 1.).abs()
            <= radius + f64::from(primitive.geometry[0][0]) + 0.00001 * (1. + width.recip() + height.recip());
    }
    let [sx, sy, ex, ey] = primitive.geometry[0].map(f64::from);
    let [stroke, aspect, _, _] = primitive.geometry[1].map(f64::from);
    let delta = [(ex - sx) * aspect, ey - sy];
    let point = [(center[0] - sx) * aspect, center[1] - sy];
    let along =
        ((point[0] * delta[0] + point[1] * delta[1]) / (delta[0] * delta[0] + delta[1] * delta[1])).clamp(0., 1.);
    let distance = (point[0] - delta[0] * along).hypot(point[1] - delta[1] * along);
    distance
        <= (half[0] * aspect).hypot(half[1]) + stroke + 0.00001 * (1. + aspect.abs() + width.recip() + height.recip())
}
