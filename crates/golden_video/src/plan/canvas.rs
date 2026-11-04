//! Bounded app-neutral procedural display lists, evaluated in one compositing draw.
use super::UnitRect;
use crate::{LinearColor, Opacity};
use std::{sync::Arc, time::Duration};
mod text;
pub(crate) use text::TextSource;
pub use text::{CANVAS_LABEL_BYTES, CanvasText, CanvasTextStyle, MAX_CANVAS_TEXT_BYTES};

/// Maximum primitive count in one admitted procedural canvas.
pub const MAX_CANVAS_PRIMITIVES: usize = 32;

/// Geometry evaluated in a primitive's local normalized rectangle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum CanvasShape {
    /// A filled rectangle.
    Rectangle,
    /// Alternating rectangular cells, including partial edge cells.
    Checkerboard,
    /// Thin horizontal and vertical cell boundaries.
    Grid,
    /// An ellipse touching the rectangle's four limiting edges.
    Ellipse,
    /// A finite line segment with an explicit coordinate aspect.
    Line,
    /// Eight vertical palette entries.
    Bars,
    /// An immutable bitmap sampled inside the rectangle.
    Image,
    /// Fixed-font text, optionally tiled or resolved from render-boundary telemetry.
    Text,
}

/// A finite color or shared-clock rainbow; product layout and timing policy remain app-owned.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasPaint {
    pub(crate) color: LinearColor,
    pub(crate) rainbow: Option<(Duration, [f32; 2])>,
}
impl CanvasPaint {
    /// A premultiplied linear solid color.
    pub fn solid(color: LinearColor) -> Self {
        Self { color, rainbow: None }
    }
    /// A moving hue field. Each slope component is hue cycles across the local rectangle.
    pub fn rainbow(period: Duration, slope: [f32; 2], alpha: Opacity) -> Option<Self> {
        (!period.is_zero()
            && period <= Duration::from_secs(86_400)
            && slope.iter().all(|value| value.is_finite() && value.abs() <= 16.0))
        .then(|| Self {
            color: LinearColor::from_straight(1., 1., 1., alpha.get()).expect("checked alpha"),
            rainbow: Some((period, slope)),
        })
    }
}

/// An admitted shape; geometry and paint are immutable after construction.
#[derive(Clone, Debug, PartialEq)]
pub struct CanvasPrimitive {
    pub(crate) shape: CanvasShape,
    pub(crate) bounds: UnitRect,
    pub(crate) geometry: [[f32; 4]; 2],
    pub(crate) paint: CanvasPaint,
    pub(crate) colors: [LinearColor; 8],
    pub(crate) text: Option<CanvasText>,
}
impl CanvasPrimitive {
    fn new(shape: CanvasShape, bounds: UnitRect, paint: CanvasPaint) -> Option<Self> {
        bounds.is_valid().then_some(Self {
            shape,
            bounds,
            geometry: [[0.; 4]; 2],
            paint,
            colors: [LinearColor::TRANSPARENT; 8],
            text: None,
        })
    }
    /// A filled rectangle.
    pub fn rectangle(bounds: UnitRect, paint: CanvasPaint) -> Option<Self> {
        Self::new(CanvasShape::Rectangle, bounds, paint)
    }
    /// Alternating cells. Fractional counts preserve square cells at arbitrary output aspects.
    pub fn checkerboard(bounds: UnitRect, cells: [f32; 2], colors: [LinearColor; 2]) -> Option<Self> {
        if !cells
            .iter()
            .all(|value| value.is_finite() && (1.0 / 16384.0..=4096.0).contains(value))
        {
            return None;
        }
        let mut result = Self::new(CanvasShape::Checkerboard, bounds, CanvasPaint::solid(colors[0]))?;
        result.geometry[0][..2].copy_from_slice(&cells);
        result.colors[..2].copy_from_slice(&colors);
        Some(result)
    }
    /// Cell boundaries with half-widths measured in local rectangle fractions.
    pub fn grid(bounds: UnitRect, cells: [f32; 2], half_width: [f32; 2], paint: CanvasPaint) -> Option<Self> {
        if !cells
            .iter()
            .all(|value| value.is_finite() && (1.0 / 16384.0..=4096.0).contains(value))
            || !half_width
                .iter()
                .all(|value| value.is_finite() && *value > 0. && *value < 0.5)
        {
            return None;
        }
        let mut result = Self::new(CanvasShape::Grid, bounds, paint)?;
        result.geometry[0] = [cells[0], cells[1], half_width[0], half_width[1]];
        Some(result)
    }
    /// An ellipse outline; half-width is relative to its normalized radius.
    pub fn ellipse(bounds: UnitRect, half_width: f32, paint: CanvasPaint) -> Option<Self> {
        if !half_width.is_finite() || half_width <= 0. || half_width >= 0.5 {
            return None;
        }
        let mut result = Self::new(CanvasShape::Ellipse, bounds, paint)?;
        result.geometry[0][0] = half_width;
        Some(result)
    }
    /// A line in local coordinates. Aspect scales X into Y units before measuring distance.
    pub fn line(
        bounds: UnitRect,
        start: [f32; 2],
        end: [f32; 2],
        half_width: f32,
        aspect: f32,
        paint: CanvasPaint,
    ) -> Option<Self> {
        if start
            .iter()
            .chain(&end)
            .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
            || start == end
            || !half_width.is_finite()
            || half_width <= 0.
            || half_width >= 0.5
            || !aspect.is_finite()
            || !(1.0 / 16384.0..=16384.0).contains(&aspect)
        {
            return None;
        }
        let delta = [(end[0] - start[0]) * aspect, end[1] - start[1]];
        if delta[0] * delta[0] + delta[1] * delta[1] < 1e-12 {
            return None;
        }
        let mut result = Self::new(CanvasShape::Line, bounds, paint)?;
        result.geometry[0] = [start[0], start[1], end[0], end[1]];
        result.geometry[1] = [half_width, aspect, 0., 0.];
        Some(result)
    }
    /// Eight ordered palette entries covering the rectangle.
    pub fn bars(bounds: UnitRect, colors: [LinearColor; 8]) -> Option<Self> {
        let mut result = Self::new(CanvasShape::Bars, bounds, CanvasPaint::solid(colors[0]))?;
        result.colors = colors;
        Some(result)
    }
    /// Place the canvas's bound bitmap, preserving its declared color/alpha sampling contract.
    pub fn image(bounds: UnitRect, opacity: Opacity) -> Option<Self> {
        Self::new(
            CanvasShape::Image,
            bounds,
            CanvasPaint::solid(LinearColor::from_straight(1., 1., 1., opacity.get()).expect("checked opacity")),
        )
    }
    /// A bounded text run or tiled label field, evaluated in the same draw as other shapes.
    pub fn text(bounds: UnitRect, text: CanvasText, style: CanvasTextStyle, paint: CanvasPaint) -> Option<Self> {
        if !style.valid(&text) {
            return None;
        }
        let mut result = Self::new(CanvasShape::Text, bounds, paint)?;
        result.geometry = [
            [style.cells[0], style.cells[1], style.glyph[0], style.glyph[1]],
            [style.inset[0], style.inset[1], u8::from(style.centered) as f32, 0.],
        ];
        result.text = Some(text);
        Some(result)
    }
    /// The admitted normalized placement.
    pub fn bounds(&self) -> UnitRect {
        self.bounds
    }
    /// The geometric operation, independent of app policy.
    pub fn shape(&self) -> CanvasShape {
        self.shape
    }
}

/// An immutable bounded display list. Sampling its shared clock clones only its storage lease.
#[derive(Clone, Debug)]
pub struct CanvasPattern {
    primitives: Arc<[CanvasPrimitive]>,
    image: Option<Arc<crate::image::ImageData>>,
    pub(crate) monotonic_nanos: u64,
}
impl CanvasPattern {
    /// Admit a nonempty list before retaining it or submitting native work.
    pub fn new(primitives: Vec<CanvasPrimitive>) -> Option<Self> {
        Self::assemble(primitives, None)
    }
    /// Bind one immutable drawing asset. It is not a media producer or an external endpoint.
    pub fn with_image(primitives: Vec<CanvasPrimitive>, image: Arc<crate::image::ImageData>) -> Option<Self> {
        if !matches!(
            image.descriptor().format,
            crate::PixelFormat::Rgba8Unorm | crate::PixelFormat::Bgra8Unorm
        ) {
            return None;
        }
        Self::assemble(primitives, Some(image))
    }
    fn assemble(primitives: Vec<CanvasPrimitive>, image: Option<Arc<crate::image::ImageData>>) -> Option<Self> {
        if primitives.is_empty() || primitives.len() > MAX_CANVAS_PRIMITIVES {
            return None;
        }
        if primitives
            .iter()
            .filter_map(|item| item.text.as_ref())
            .map(CanvasText::reserved_bytes)
            .sum::<usize>()
            > MAX_CANVAS_TEXT_BYTES
        {
            return None;
        }
        let needs_image = primitives.iter().any(|primitive| primitive.shape == CanvasShape::Image);
        (needs_image == image.is_some()).then(|| Self {
            primitives: primitives.into(),
            image,
            monotonic_nanos: 0,
        })
    }
    /// Immutable bitmap lease retained through plan and native texture lifetime.
    pub fn image(&self) -> Option<&Arc<crate::image::ImageData>> {
        self.image.as_ref()
    }
    /// Read-only drawing order.
    pub fn primitives(&self) -> &[CanvasPrimitive] {
        &self.primitives
    }
    pub(crate) fn sample_time(&mut self, time: golden_time::TimeSnapshot) {
        self.monotonic_nanos = time.monotonic().as_nanos();
    }
    pub(crate) fn storage_identity(&self) -> usize {
        self.primitives.as_ptr() as usize
    }
    pub(crate) fn retained_bytes(&self) -> usize {
        self.primitives.len() * size_of::<CanvasPrimitive>()
            + 2 * size_of::<usize>()
            + self
                .primitives
                .iter()
                .filter_map(|item| item.text.as_ref())
                .map(CanvasText::retained_bytes)
                .sum::<usize>()
    }
}

impl PartialEq for CanvasPattern {
    fn eq(&self, other: &Self) -> bool {
        self.monotonic_nanos == other.monotonic_nanos
            && self.primitives == other.primitives
            && match (&self.image, &other.image) {
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                (None, None) => true,
                _ => false,
            }
    }
}
