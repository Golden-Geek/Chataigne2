use crate::plan::{CanvasPrimitive, CanvasShape, MAX_CANVAS_PRIMITIVES, Pattern};

#[cfg(test)]
mod tests;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct PrimitiveParameters {
    metadata: [u32; 4],
    bounds: [f32; 4],
    geometry: [[f32; 4]; 2],
    paint: [f32; 4],
    colors: [[f32; 4]; 8],
}
impl PrimitiveParameters {
    fn new(primitive: &CanvasPrimitive, nanos: u64) -> Self {
        let mut result = Self {
            metadata: [primitive.shape as u32, 0, 0, 0],
            bounds: [
                primitive.bounds.x,
                primitive.bounds.y,
                primitive.bounds.width,
                primitive.bounds.height,
            ],
            geometry: primitive.geometry,
            paint: [0.; 4],
            colors: primitive.colors.map(|color| color.components()),
        };
        if !matches!(primitive.shape, CanvasShape::Checkerboard | CanvasShape::Bars) {
            result.colors[0] = primitive.paint.color.components();
        }
        if let Some((period, slope)) = primitive.paint.rainbow {
            result.metadata[1] = 1;
            result.paint = [
                slope[0],
                slope[1],
                ((u128::from(nanos) % period.as_nanos()) as f64 / period.as_nanos() as f64) as f32,
                primitive.paint.color.components()[3],
            ];
        }
        result
    }
}

#[repr(C)]
pub(super) struct PatternParameters {
    kind: u32,
    mapped: u32,
    cells: [u32; 2],
    width: f32,
    reserved: [u32; 3],
    colors: [[f32; 4]; 8],
    primitives: [PrimitiveParameters; MAX_CANVAS_PRIMITIVES],
}

impl PatternParameters {
    pub(super) fn new(pattern: &Pattern, mapped: bool) -> Self {
        match pattern {
            Pattern::Grid(grid) => {
                let mut colors = [[0.0; 4]; 8];
                colors[0] = grid.foreground().components();
                colors[1] = grid.background().components();
                Self {
                    kind: 0,
                    mapped: u32::from(mapped),
                    cells: grid.cells(),
                    width: grid.width(),
                    reserved: [0; 3],
                    colors,
                    primitives: [PrimitiveParameters::default(); MAX_CANVAS_PRIMITIVES],
                }
            }
            Pattern::Bars(colors) => Self {
                kind: 1,
                mapped: u32::from(mapped),
                cells: [0; 2],
                width: 0.0,
                reserved: [0; 3],
                colors: colors.map(|color| color.components()),
                primitives: [PrimitiveParameters::default(); MAX_CANVAS_PRIMITIVES],
            },
            Pattern::Canvas(canvas) => {
                let mut result = Self {
                    kind: 2,
                    mapped: u32::from(mapped),
                    cells: [0; 2],
                    width: 0.,
                    reserved: [canvas.primitives().len() as u32, 0, 0],
                    colors: [[0.; 4]; 8],
                    primitives: [PrimitiveParameters::default(); MAX_CANVAS_PRIMITIVES],
                };
                for (target, primitive) in result.primitives.iter_mut().zip(canvas.primitives()) {
                    *target = PrimitiveParameters::new(primitive, canvas.monotonic_nanos);
                }
                result
            }
        }
    }
}

const _: () = assert!(size_of::<PrimitiveParameters>() == 208);
const _: () = assert!(size_of::<PatternParameters>() == 6816);
