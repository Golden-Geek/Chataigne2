//! Checked ASCII text; the fixed public-domain bitmap font is compiled with the native shader.
use std::sync::Arc;

/// Maximum character storage in one canvas, independent of its pixel dimensions.
pub const MAX_CANVAS_TEXT_BYTES: usize = 8192;
/// Maximum bytes in a tiled label, including any zero padding.
pub const CANVAS_LABEL_BYTES: usize = 8;

/// Immutable text or a value resolved on the rendering worker.
#[derive(Clone, Debug, PartialEq)]
pub struct CanvasText {
    pub(crate) source: TextSource,
    pub(crate) bytes: Arc<[u8]>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TextSource {
    Literal,
    Tiles,
    LocalTime,
    FrameRate,
}
impl CanvasText {
    /// One line of printable ASCII, at most 64 characters.
    pub fn literal(text: &str) -> Option<Self> {
        (!text.is_empty() && text.len() <= 64 && printable(text)).then(|| Self {
            source: TextSource::Literal,
            bytes: text.as_bytes().into(),
        })
    }
    /// Row-major labels, at most 1024 entries of eight printable ASCII characters each.
    pub fn tiles(labels: &[&str]) -> Option<Self> {
        if labels.is_empty()
            || labels.len() > 1024
            || labels
                .iter()
                .any(|text| text.len() > CANVAS_LABEL_BYTES || !printable(text))
        {
            return None;
        }
        let mut bytes = vec![0; labels.len() * CANVAS_LABEL_BYTES];
        for (slot, text) in bytes.chunks_exact_mut(CANVAS_LABEL_BYTES).zip(labels) {
            slot[..text.len()].copy_from_slice(text.as_bytes());
        }
        Some(Self {
            source: TextSource::Tiles,
            bytes: bytes.into(),
        })
    }
    /// Local system time, `HH:MM:SS.mmm`; never evaluated by the engine actor.
    pub fn local_time() -> Self {
        Self {
            source: TextSource::LocalTime,
            bytes: Arc::from([]),
        }
    }
    /// Measured successful renderer submissions per second; unavailable until sampled.
    pub fn frame_rate() -> Self {
        Self {
            source: TextSource::FrameRate,
            bytes: Arc::from([]),
        }
    }
    pub(crate) fn reserved_bytes(&self) -> usize {
        match self.source {
            TextSource::LocalTime | TextSource::FrameRate => 64,
            _ => self.bytes.len(),
        }
    }
    pub(crate) fn retained_bytes(&self) -> usize {
        self.bytes.len() + 2 * size_of::<usize>()
    }
}
fn printable(text: &str) -> bool {
    text.bytes().all(|byte| (32..=126).contains(&byte))
}

/// Geometry in the primitive's local rectangle. Glyph dimensions include a one-pixel advance gap.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasTextStyle {
    /// Fractional tile counts preserve partial cells at the canvas edges.
    pub cells: [f32; 2],
    /// Character advance and height in local rectangle fractions (a 9 by 8 font cell).
    pub glyph: [f32; 2],
    /// Text origin relative to each tile's top-left, in local rectangle fractions.
    pub inset: [f32; 2],
    /// Center a single line horizontally; tiled labels always use their explicit inset.
    pub centered: bool,
}
impl CanvasTextStyle {
    pub(crate) fn valid(self, text: &CanvasText) -> bool {
        if !self.cells.iter().all(|v| v.is_finite() && *v > 0. && *v <= 32.)
            || !self.glyph.iter().all(|v| v.is_finite() && *v > 0. && *v <= 1.)
            || !self.inset.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v))
        {
            return false;
        }
        match text.source {
            TextSource::Tiles => {
                !self.centered
                    && text.bytes.len() / CANVAS_LABEL_BYTES
                        == self.cells[0].ceil() as usize * self.cells[1].ceil() as usize
            }
            _ => self.cells == [1.; 2],
        }
    }
}
