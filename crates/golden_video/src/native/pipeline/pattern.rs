use crate::plan::canvas::{CANVAS_LABEL_BYTES, MAX_CANVAS_TEXT_BYTES, TextSource};
use crate::plan::{CanvasPrimitive, CanvasShape, MAX_CANVAS_PRIMITIVES, Pattern};
pub(super) mod frame;
mod storage;
use frame::{AsciiLine, FrameText};
pub(super) use storage::PatternBuffer;

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
    text: [[u32; 4]; MAX_CANVAS_TEXT_BYTES / 16],
}

impl PatternParameters {
    #[cfg(test)]
    pub(super) fn new(pattern: &Pattern, mapped: bool) -> Self {
        Self::with_frame(pattern, mapped, FrameText::default())
    }
    pub(super) fn with_frame(pattern: &Pattern, mapped: bool, frame: FrameText) -> Self {
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
                    text: [[0; 4]; MAX_CANVAS_TEXT_BYTES / 16],
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
                text: [[0; 4]; MAX_CANVAS_TEXT_BYTES / 16],
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
                    text: [[0; 4]; MAX_CANVAS_TEXT_BYTES / 16],
                };
                let mut offset = 0;
                for (target, primitive) in result.primitives.iter_mut().zip(canvas.primitives()) {
                    *target = PrimitiveParameters::new(primitive, canvas.monotonic_nanos);
                    if let Some(text) = &primitive.text {
                        let line;
                        let bytes = match text.source {
                            TextSource::LocalTime => {
                                line = AsciiLine::clock(frame);
                                &line.bytes[..line.len]
                            }
                            TextSource::FrameRate => {
                                line = AsciiLine::fps(frame);
                                &line.bytes[..line.len]
                            }
                            _ => &text.bytes,
                        };
                        target.metadata[2] = offset as u32;
                        target.metadata[3] = if text.source == TextSource::Tiles {
                            CANVAS_LABEL_BYTES as u32
                        } else {
                            bytes.len() as u32
                        };
                        if target.geometry[1][2] != 0. {
                            target.geometry[1][0] = (1. - bytes.len() as f32 * target.geometry[0][2]) * 0.5;
                        }
                        for &byte in bytes {
                            result.text[offset / 16][(offset / 4) % 4] |= u32::from(byte) << ((offset % 4) * 8);
                            offset += 1;
                        }
                    }
                }
                result
            }
        }
    }
}

const _: () = assert!(size_of::<PrimitiveParameters>() == 208);
const _: () = assert!(size_of::<PatternParameters>() == 15008);
const _: () = assert!(std::mem::offset_of!(PatternParameters, colors) == 32);
const _: () = assert!(std::mem::offset_of!(PatternParameters, primitives) == 160);
const _: () = assert!(std::mem::offset_of!(PatternParameters, text) == 6816);
